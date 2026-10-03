// ✝ Eclesiastes 4:12 — “O cordão de três dobras não se quebra tão depressa.”
//! A computação científica entre nós (Etapa D de `docs/COMPUTACAO-CIENTIFICA.md`).
//!
//! Dois papéis, no mesmo nó:
//!
//! - **quem trabalha** (só com "aceitar trabalho da rede" ligado): recebe um
//!   pedido de unidade, valida (a especificação é decodificada e conferida;
//!   memória, prazo, tamanho do resultado e quantos pedidos por par têm teto),
//!   calcula pelo mesmo worker do ULTRAX, com os mesmos limites, e responde
//!   com o compromisso; quando quem pediu manda revelar, entrega o resultado e
//!   o registro de prova assinado;
//! - **quem pede** (JOB com verificação de nível 3 ou mais): manda cada unidade
//!   para `redundância − 1` workers de **outros nós** e calcula uma cópia aqui.
//!   Espera todos os compromissos antes de pedir a revelação (quem copiar o
//!   resultado de outro não sabe o compromisso a tempo), confere cada entrega
//!   (assinatura, entrada, hash do resultado, compromisso) e decide por
//!   maioria. O resultado da maioria ainda é conferido aqui, por recomputação,
//!   antes de entrar no JOB. Sem maioria, a unidade volta para a fila.
//!
//! A reputação de cada worker (entregas, verificadas, recusadas, divergentes)
//! é medida aqui e usada para escolher a quem pedir. Não é consenso de rede:
//! é o que este nó viu.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN};
use hyurax_net::Rede;
use hyurax_ultrax::job::Nivel;
use hyurax_ultrax::prova::RegistroDeProva;
use hyurax_ultrax::rede::{MensagemUltrax, RESULTADO_MAX, bit_do_tipo};
use hyurax_ultrax::reputacao::Reputacao;
use hyurax_ultrax::tarefa::Estado;
use hyurax_ultrax::trabalho::{self, Especificacao, TipoDeTrabalho, hash_da_entrada, hash_do_resultado};
use hyurax_ultrax::validador::{Desfecho, Parecer, compromisso, por_maioria, revelacao_confere};

use super::{Ciencia, EmVoo, Evento, PRIORIDADE, agora_ms};
use crate::util::hex;
use crate::ultrax::{DesfechoDeUnidade, PedidoDeUnidade};

/// Uma oferta vale por isso depois de vista.
const OFERTA_VALE_MS: u64 = 90_000;
/// Pedidos de outros nós em voo neste worker, por par.
pub(super) const REMOTAS_POR_PAR: usize = 2;
/// Pedidos de outros nós em voo neste worker, no total.
const REMOTAS_MAX: usize = 8;
/// Resultado guardado esperando a revelação, no máximo por isso.
const GUARDADO_MS: u64 = 10 * 60_000;
/// Uma oferta só vale se o horário assinado nela estiver perto do nosso:
/// oferta velha, guardada e repetida por outro par, não vale.
const OFERTA_FRESCA_MS: u64 = 5 * 60_000;
/// Par que recusou ou sumiu fica fora da escolha por isso.
const INDISPONIVEL_MS: u64 = 5 * 60_000;

/// Uma oferta vista de um par.
#[derive(Clone, Copy, Debug)]
pub(super) struct OfertaVista {
    pub(super) worker: [u8; PUBKEY_LEN],
    pub(super) tipos: u16,
    pub(super) memoria_mib: u32,
    pub(super) visto_ms: u64,
}

/// Uma unidade que outro nó pediu a este.
#[derive(Clone, Copy, Debug)]
pub(super) struct Remota {
    par: u64,
    pedido: u64,
    job: [u8; HASH_LEN],
    indice: u64,
    esp: Especificacao,
    semente: [u8; HASH_LEN],
    prazo_ms: u64,
    com_compromisso: bool,
}

/// Um resultado pronto, esperando a revelação.
#[derive(Clone, Debug)]
pub(super) struct Guardado {
    registro: Vec<u8>,
    assinatura: [u8; 64],
    resultado: Vec<u8>,
    desde_ms: u64,
}

/// Uma entrega conferida.
#[derive(Clone, Debug)]
pub(super) struct Entrega {
    worker: [u8; PUBKEY_LEN],
    hash: [u8; HASH_LEN],
    resultado: Vec<u8>,
    registro: RegistroDeProva,
    ms_calculo: u64,
    ms_verificacao: u64,
    memoria: u64,
    operacoes_verificacao: u64,
    tarefa: [u8; HASH_LEN],
    /// A assinatura do registro de prova, guardada como evidência do voto.
    assinatura: Option<[u8; 64]>,
}

/// Onde está um worker remoto nesta unidade.
#[derive(Clone, Debug)]
pub(super) enum Remoto {
    Esperando { par: u64, pedido: u64 },
    /// `assinatura`: do worker, sobre o compromisso para esta unidade (já
    /// conferida na chegada).
    Comprometido { par: u64, pedido: u64, worker: [u8; PUBKEY_LEN], compromisso: [u8; HASH_LEN], assinatura: [u8; 64] },
    Entregou(Box<Entrega>),
    Falhou { worker: Option<[u8; PUBKEY_LEN]>, motivo: String },
}

/// Uma unidade redundante (nível 3 ou mais), esperando os workers.
#[derive(Clone, Debug)]
pub(super) struct Redundante {
    esp: Especificacao,
    semente: [u8; HASH_LEN],
    prazo_ms: u64,
    local: Option<Result<Entrega, String>>,
    remotos: Vec<Remoto>,
    revelou: bool,
}

/// O estado da rede científica deste nó.
#[derive(Default)]
pub(super) struct EstadoDaRede {
    pub(super) rede: Option<Arc<Rede>>,
    pub(super) segredo: Option<[u8; SECRET_LEN]>,
    pub(super) ofertas: BTreeMap<u64, OfertaVista>,
    pub(super) fila: VecDeque<Remota>,
    pub(super) executando: BTreeMap<([u8; HASH_LEN], u64), Remota>,
    pub(super) guardados: BTreeMap<(u64, u64), Guardado>,
    pub(super) redundantes: BTreeMap<([u8; HASH_LEN], u64), Redundante>,
    pub(super) pedidos: BTreeMap<u64, ([u8; HASH_LEN], u64)>,
    pub(super) proximo_pedido: u64,
    pub(super) reputacao: BTreeMap<[u8; PUBKEY_LEN], Reputacao>,
    /// Pares que recusaram ou sumiram: fora da escolha até este instante.
    pub(super) indisponiveis: BTreeMap<u64, u64>,
    pub(super) recebidas: u64,
    pub(super) enviadas: u64,
    pub(super) ultima_oferta_ms: u64,
    /// Fila da thread que decide as unidades redundantes. A decisão pode
    /// refazer o cálculo (segundos) e roda fora da thread que lê o par: senão
    /// um resultado remoto segurava os blocos e as transações daquele par.
    pub(super) decisora: Option<std::sync::mpsc::SyncSender<([u8; HASH_LEN], u64)>>,
}

/// Unidades esperando decisão na fila da thread decisora. Cheia, a decisão
/// roda na hora, na thread de quem chegou (o par que inunda espera por ela).
const FILA_DE_DECISOES: usize = 256;

impl EstadoDaRede {
    /// Pares com oferta válida que aceitam este trabalho, dos de melhor
    /// reputação para os de pior.
    fn candidatos(&self, esp: &Especificacao, agora: u64) -> Vec<(u64, [u8; PUBKEY_LEN])> {
        let memoria = esp.memoria_bytes().div_ceil(1024 * 1024);
        let ocupados = |par: u64| self.redundantes.values().flat_map(|r| &r.remotos).filter(|x| matches!(x, Remoto::Esperando { par: p, .. } | Remoto::Comprometido { par: p, .. } if *p == par)).count();
        let mut v: Vec<(u64, [u8; PUBKEY_LEN], u64)> = self
            .ofertas
            .iter()
            .filter(|(_, o)| agora.saturating_sub(o.visto_ms) < OFERTA_VALE_MS)
            .filter(|(_, o)| o.tipos & bit_do_tipo(esp.tipo()) != 0 && u64::from(o.memoria_mib) >= memoria)
            .filter(|(par, _)| ocupados(**par) < REMOTAS_POR_PAR)
            .filter(|(par, _)| self.indisponiveis.get(par).is_none_or(|ate| *ate <= agora))
            .map(|(par, o)| (*par, o.worker, self.reputacao.get(&o.worker).map_or(500, Reputacao::nota)))
            .collect();
        v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        v.into_iter().map(|(p, w, _)| (p, w)).collect()
    }
}

impl Ciencia {
    /// Liga a computação científica na rede P2P deste nó. `aceitar` diz se
    /// este nó calcula unidades que outros pedirem.
    pub fn ligar_rede(self: &Arc<Self>, rede: &Arc<Rede>, segredo_do_worker: [u8; SECRET_LEN], aceitar: bool) {
        if let Ok(mut r) = self.rede.lock() {
            r.rede = Some(Arc::clone(rede));
            r.segredo = Some(segredo_do_worker);
        }
        self.aceitar_rede.store(aceitar, Ordering::Relaxed);
        self.iniciar_decisora();
        let c = Arc::downgrade(self);
        rede.ao_receber_ultrax(Arc::new(move |par, _identidade, corpo| {
            if let Some(c) = c.upgrade() {
                c.tratar_ultrax(par, corpo);
            }
        }));
    }

    /// A thread que decide as unidades redundantes (uma por ciência). Ela só
    /// guarda uma referência fraca: quando a ciência acaba, a fila fecha e a
    /// thread termina.
    fn iniciar_decisora(self: &Arc<Self>) {
        let Ok(mut r) = self.rede.lock() else { return };
        if r.decisora.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::sync_channel::<([u8; HASH_LEN], u64)>(FILA_DE_DECISOES);
        let fraca = Arc::downgrade(self);
        let criada = std::thread::Builder::new().name("ciencia-decisora".into()).spawn(move || {
            while let Ok((job, indice)) = rx.recv() {
                let Some(c) = fraca.upgrade() else { return };
                c.decidir(&job, indice);
            }
        });
        if criada.is_ok() {
            r.decisora = Some(tx);
        }
    }

    /// Decide a unidade na thread decisora; sem ela (testes) ou com a fila
    /// cheia, decide aqui mesmo.
    fn decidir_depois(&self, job: [u8; HASH_LEN], indice: u64) {
        let fila = self.rede.lock().ok().and_then(|r| r.decisora.clone());
        if fila.is_none_or(|tx| tx.try_send((job, indice)).is_err()) {
            self.decidir(&job, indice);
        }
    }

    /// Liga ou desliga aceitar trabalho de outros nós.
    pub fn aceitar_da_rede(&self, aceitar: bool) {
        self.aceitar_rede.store(aceitar, Ordering::Relaxed);
    }

    /// Este nó aceita trabalho de outros nós?
    pub fn aceita_da_rede(&self) -> bool {
        self.aceitar_rede.load(Ordering::Relaxed)
    }

    pub(super) fn enviar(&self, par: u64, m: &MensagemUltrax) {
        let rede = self.rede.lock().ok().and_then(|r| r.rede.clone());
        if let (Some(rede), Ok(corpo)) = (rede, m.codificar())
            && rede.enviar_ultrax(par, corpo)
            && let Ok(mut r) = self.rede.lock()
        {
            r.enviadas = r.enviadas.saturating_add(1);
        }
    }

    /// Manda a oferta deste worker a todos os pares (se aceita trabalho) e
    /// limpa o que venceu.
    pub(super) fn manutencao_rede(&self, agora: u64) {
        let (rede, segredo, ofertar) = match self.rede.lock() {
            Ok(mut r) => {
                let ofertar = agora.saturating_sub(r.ultima_oferta_ms) >= 10_000;
                if ofertar {
                    r.ultima_oferta_ms = agora;
                }
                (r.rede.clone(), r.segredo, ofertar)
            }
            Err(_) => return,
        };
        let Some(rede) = rede else { return };
        if ofertar
            && self.aceitar_rede.load(Ordering::Relaxed)
            && let (Some(segredo), Some(u)) = (segredo, self.ultrax())
            && u.ligado.load(Ordering::Relaxed)
        {
            let tipos = TipoDeTrabalho::TODOS.iter().fold(0u16, |t, tipo| t | bit_do_tipo(*tipo));
            let linhas = u8::try_from(u.linhas.load(Ordering::Relaxed)).unwrap_or(u8::MAX);
            let oferta = MensagemUltrax::oferta(&segredo, tipos, linhas, u.memoria_mib.load(Ordering::Relaxed), agora);
            if let Ok(corpo) = oferta.codificar() {
                rede.difundir_ultrax(&corpo);
            }
        }
        let mut vencidas = Vec::new();
        if let Ok(mut r) = self.rede.lock() {
            r.ofertas.retain(|_, o| agora.saturating_sub(o.visto_ms) < OFERTA_VALE_MS);
            r.guardados.retain(|_, g| agora.saturating_sub(g.desde_ms) < GUARDADO_MS);
            // unidade redundante vencida: quem não entregou falhou
            let mut sumiram = Vec::new();
            for ((job, indice), u) in r.redundantes.iter_mut() {
                if agora > u.prazo_ms {
                    for x in &mut u.remotos {
                        if let Remoto::Esperando { par, .. } | Remoto::Comprometido { par, .. } = x {
                            sumiram.push(*par);
                            *x = Remoto::Falhou { worker: None, motivo: "prazo vencido sem entrega".into() };
                        }
                    }
                    if u.local.is_none() {
                        u.local = Some(Err("prazo vencido aqui".into()));
                    }
                    vencidas.push((*job, *indice));
                }
            }
            for par in sumiram {
                r.indisponiveis.insert(par, agora.saturating_add(INDISPONIVEL_MS));
            }
            r.indisponiveis.retain(|_, ate| *ate > agora);
        }
        // na decisora: a conferência não segura o vigia (checkpoints, prazos)
        for (job, indice) in vencidas {
            self.decidir_depois(job, indice);
        }
    }

    /// O que chega de outro nó.
    fn tratar_ultrax(&self, par: u64, corpo: &[u8]) {
        let Ok(m) = MensagemUltrax::decodificar(corpo) else { return };
        if let Ok(mut r) = self.rede.lock() {
            r.recebidas = r.recebidas.saturating_add(1);
        }
        match m {
            MensagemUltrax::Oferta { worker, tipos, memoria_mib, instante_ms, .. } => {
                let agora = agora_ms();
                let fresca = agora.abs_diff(instante_ms) <= OFERTA_FRESCA_MS;
                if fresca
                    && m.oferta_confere()
                    && let Ok(mut r) = self.rede.lock()
                {
                    // a mesma chave de worker em dois pares: vale a do primeiro
                    // (quem repete a oferta de outro não toma o lugar dele)
                    let de_outro = r.ofertas.iter().any(|(p, o)| *p != par && o.worker == worker && agora.saturating_sub(o.visto_ms) < OFERTA_VALE_MS);
                    if !de_outro {
                        r.ofertas.insert(par, OfertaVista { worker, tipos, memoria_mib, visto_ms: agora });
                    }
                }
            }
            MensagemUltrax::Pedido { pedido, job, indice, especificacao, semente, prazo_ms, com_compromisso } => {
                let motivo = self.aceitar_pedido(Remota { par, pedido, job, indice, esp: especificacao, semente, prazo_ms, com_compromisso });
                if let Some(motivo) = motivo {
                    self.enviar(par, &MensagemUltrax::Recusa { pedido, motivo });
                }
            }
            MensagemUltrax::Revelar { pedido } => {
                let guardado = self.rede.lock().ok().and_then(|mut r| r.guardados.remove(&(par, pedido)));
                if let Some(g) = guardado {
                    self.enviar(par, &MensagemUltrax::Resultado { pedido, registro: g.registro, assinatura: g.assinatura, resultado: g.resultado });
                }
            }
            MensagemUltrax::Cancelar { pedido } => {
                if let Ok(mut r) = self.rede.lock() {
                    r.fila.retain(|x| !(x.par == par && x.pedido == pedido));
                    r.guardados.remove(&(par, pedido));
                }
            }
            MensagemUltrax::Recusa { pedido, motivo } => {
                // recusa não pesa na reputação de ninguém, mas o par sai da
                // escolha por um tempo (e a unidade não conta como falha)
                if let Ok(mut r) = self.rede.lock() {
                    r.indisponiveis.insert(par, agora_ms().saturating_add(INDISPONIVEL_MS));
                }
                self.do_remoto(par, pedido, |x| {
                    *x = Remoto::Falhou { worker: None, motivo: format!("recusou: {motivo}") };
                });
            }
            MensagemUltrax::Compromisso { pedido, worker, compromisso, assinatura } => {
                // o worker do compromisso tem de ser o da oferta deste par, e a
                // assinatura dele tem de ser para a unidade deste pedido
                let (ofertado, unidade) = self
                    .rede
                    .lock()
                    .map(|r| (r.ofertas.get(&par).map(|o| o.worker), r.pedidos.get(&pedido).copied()))
                    .unwrap_or((None, None));
                let assinado = unidade.is_some_and(|(job, indice)| {
                    hyurax_crypto::ed25519_verify(&worker, &hyurax_ultrax::rede::mensagem_do_compromisso(&job, indice, pedido, &compromisso), &assinatura)
                });
                self.do_remoto(par, pedido, |x| {
                    if let Remoto::Esperando { par, pedido } = *x {
                        *x = if ofertado != Some(worker) {
                            Remoto::Falhou { worker: None, motivo: "compromisso com um worker que não é o da oferta deste par".into() }
                        } else if !assinado {
                            Remoto::Falhou { worker: None, motivo: "a assinatura do compromisso não confere".into() }
                        } else {
                            Remoto::Comprometido { par, pedido, worker, compromisso, assinatura }
                        };
                    }
                });
            }
            MensagemUltrax::Resultado { pedido, registro, assinatura, resultado } => {
                self.resultado_remoto(par, pedido, &registro, &assinatura, resultado);
            }
        }
    }

    /// Quem trabalha: aceita (entra na fila) ou devolve o motivo da recusa.
    fn aceitar_pedido(&self, remota: Remota) -> Option<String> {
        if !self.aceitar_rede.load(Ordering::Relaxed) {
            return Some("este nó não aceita trabalho da rede".into());
        }
        let Some(u) = self.ultrax() else { return Some("o ULTRAX deste nó não está pronto".into()) };
        if !u.ligado.load(Ordering::Relaxed) {
            return Some("o ULTRAX deste nó está desligado".into());
        }
        let teto = u64::from(u.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
        if remota.esp.memoria_bytes() > teto {
            return Some(format!("pede {} MiB; o teto deste worker é {} MiB", remota.esp.memoria_bytes() / 1_048_576, teto / 1_048_576));
        }
        if trabalho::tamanho_maximo_do_resultado(&remota.esp) > RESULTADO_MAX as u64 {
            return Some("o resultado passaria do teto de 1 MiB da rede".into());
        }
        if remota.prazo_ms <= agora_ms() {
            return Some("o prazo já venceu".into());
        }
        if remota.semente != hyurax_ultrax::job::semente_da_unidade(&remota.job, remota.indice) {
            return Some("a semente não é a desta unidade do JOB".into());
        }
        if self.jobs.lock().is_ok_and(|j| j.iter().any(|x| x.id == remota.job)) {
            return Some("este nó já calcula este JOB".into());
        }
        let Ok(mut r) = self.rede.lock() else { return Some("estado da rede travado".into()) };
        let mesma = |x: &Remota| x.job == remota.job && x.indice == remota.indice;
        if r.fila.iter().any(mesma) || r.executando.values().any(mesma) {
            return Some("já calculo esta unidade para outro pedido".into());
        }
        let do_par = r.fila.iter().filter(|x| x.par == remota.par).count().saturating_add(r.executando.values().filter(|x| x.par == remota.par).count());
        if do_par >= REMOTAS_POR_PAR || r.fila.len().saturating_add(r.executando.len()) >= REMOTAS_MAX {
            return Some("worker ocupado".into());
        }
        r.fila.push_back(remota);
        None
    }

    /// Quem trabalha: a próxima unidade pedida por outro nó, para o worker local.
    pub(super) fn proxima_remota(&self, agora: u64) -> Option<PedidoDeUnidade> {
        let mut vencidas = Vec::new();
        let escolhida = {
            let mut r = self.rede.lock().ok()?;
            let mut escolhida = None;
            while let Some(x) = r.fila.pop_front() {
                if x.prazo_ms <= agora {
                    vencidas.push(x);
                    continue;
                }
                r.executando.insert((x.job, x.indice), x);
                escolhida = Some(x);
                break;
            }
            escolhida
        };
        for x in vencidas {
            self.enviar(x.par, &MensagemUltrax::Recusa { pedido: x.pedido, motivo: "o prazo venceu na fila".into() });
        }
        let x = escolhida?;
        self.emitir(Evento {
            evento: "REMOTE_UNIT_ACCEPTED",
            job: None,
            unidade: Some(x.indice),
            no: Some(self.no),
            tipo: Some(x.esp.tipo()),
            operacao: format!("unidade {} de um JOB de outro nó ({}…): {} {}", x.indice, hex(x.job.get(..8).unwrap_or_default()), x.esp.tipo().descricao(), x.esp.resumo()),
            ..Evento::default()
        });
        Some(PedidoDeUnidade { job: x.job, indice: x.indice, especificacao: x.esp, semente: x.semente, prioridade: PRIORIDADE, prazo_ms: x.prazo_ms })
    }

    /// Quem trabalha: o worker local terminou uma unidade de outro nó.
    /// Devolve `false` se a unidade não era de outro nó.
    pub(super) fn terminou_remota(&self, d: &DesfechoDeUnidade) -> bool {
        let remota = match self.rede.lock() {
            Ok(mut r) => {
                if d.estado == Estado::Recusada {
                    // o worker repete uma vez; espera o desfecho final
                    return r.executando.contains_key(&(d.job, d.indice));
                }
                r.executando.remove(&(d.job, d.indice))
            }
            Err(_) => return false,
        };
        let Some(x) = remota else { return false };
        match (d.estado, d.registro.as_ref(), d.assinatura, d.resultado.as_ref()) {
            (Estado::Liquidada, Some(registro), Some(assinatura), Some(resultado)) => {
                let guardado = Guardado { registro: registro.codificar(), assinatura, resultado: resultado.clone(), desde_ms: agora_ms() };
                if x.com_compromisso {
                    let c = compromisso(&registro.resultado, &registro.worker);
                    let segredo = match self.rede.lock() {
                        Ok(mut r) => {
                            r.guardados.insert((x.par, x.pedido), guardado);
                            r.segredo
                        }
                        Err(_) => None,
                    };
                    // assinado pela mesma chave do registro de prova: revelar
                    // outra coisa depois deixa a prova contra este worker
                    let m = match segredo {
                        Some(s) if hyurax_crypto::ed25519_public_key(&s) == registro.worker => {
                            MensagemUltrax::compromisso(&s, &x.job, x.indice, x.pedido, c)
                        }
                        _ => MensagemUltrax::Recusa { pedido: x.pedido, motivo: "sem a chave do worker para assinar o compromisso".into() },
                    };
                    self.enviar(x.par, &m);
                } else {
                    self.enviar(
                        x.par,
                        &MensagemUltrax::Resultado { pedido: x.pedido, registro: guardado.registro, assinatura, resultado: guardado.resultado },
                    );
                }
            }
            _ => {
                let motivo = if d.nota.is_empty() { format!("parou em {}", d.estado.nome()) } else { d.nota.clone() };
                self.enviar(x.par, &MensagemUltrax::Recusa { pedido: x.pedido, motivo });
            }
        }
        true
    }

    /// Quem pede: despacha uma unidade redundante (a cópia local volta como
    /// pedido para o worker daqui). `None` se não há workers de fora bastante.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn despachar_redundante(
        &self,
        job: [u8; HASH_LEN],
        indice: u64,
        esp: Especificacao,
        semente: [u8; HASH_LEN],
        redundancia: u8,
        prazo_ms: u64,
        agora: u64,
    ) -> Option<Vec<(u64, MensagemUltrax)>> {
        let precisa = usize::from(redundancia.saturating_sub(1));
        let mut r = self.rede.lock().ok()?;
        if trabalho::tamanho_maximo_do_resultado(&esp) > RESULTADO_MAX as u64 {
            return None;
        }
        let candidatos = r.candidatos(&esp, agora);
        if candidatos.len() < precisa {
            return None;
        }
        let mut envios = Vec::new();
        let mut remotos = Vec::new();
        for (par, _) in candidatos.into_iter().take(precisa) {
            r.proximo_pedido = r.proximo_pedido.saturating_add(1);
            let pedido = r.proximo_pedido;
            r.pedidos.insert(pedido, (job, indice));
            remotos.push(Remoto::Esperando { par, pedido });
            envios.push((par, MensagemUltrax::Pedido { pedido, job, indice, especificacao: esp, semente, prazo_ms, com_compromisso: true }));
        }
        r.redundantes.insert((job, indice), Redundante { esp, semente, prazo_ms, local: None, remotos, revelou: false });
        Some(envios)
    }

    /// Quantos workers de fora aceitariam este trabalho agora.
    pub(super) fn workers_de_fora(&self, esp: &Especificacao, agora: u64) -> usize {
        self.rede.lock().map_or(0, |r| r.candidatos(esp, agora).len())
    }

    /// Quem pede: o worker local terminou a cópia de uma unidade redundante.
    /// Devolve `false` se a unidade não era redundante.
    pub(super) fn terminou_redundante(&self, d: &DesfechoDeUnidade) -> bool {
        if d.estado == Estado::Recusada {
            return self.rede.lock().is_ok_and(|r| r.redundantes.contains_key(&(d.job, d.indice)));
        }
        {
            let Ok(mut r) = self.rede.lock() else { return false };
            let Some(u) = r.redundantes.get_mut(&(d.job, d.indice)) else { return false };
            u.local = Some(match (d.estado, d.registro.as_ref(), d.resultado.as_ref()) {
                (Estado::Liquidada, Some(reg), Some(res)) => Ok(Entrega {
                    worker: reg.worker,
                    hash: reg.resultado,
                    resultado: res.clone(),
                    registro: reg.clone(),
                    ms_calculo: d.ms_calculo,
                    ms_verificacao: d.ms_verificacao,
                    memoria: d.memoria,
                    operacoes_verificacao: d.operacoes_verificacao,
                    tarefa: d.tarefa,
                    assinatura: d.assinatura,
                }),
                _ => Err(if d.nota.is_empty() { d.estado.nome().to_string() } else { d.nota.clone() }),
            });
        }
        self.decidir(&d.job, d.indice);
        true
    }

    fn do_remoto(&self, par: u64, pedido: u64, mudar: impl FnOnce(&mut Remoto)) {
        let unidade = {
            let Ok(mut r) = self.rede.lock() else { return };
            let Some(&(job, indice)) = r.pedidos.get(&pedido) else { return };
            let Some(u) = r.redundantes.get_mut(&(job, indice)) else { return };
            let Some(x) = u.remotos.iter_mut().find(|x| matches!(x, Remoto::Esperando { par: p, pedido: q } | Remoto::Comprometido { par: p, pedido: q, .. } if *p == par && *q == pedido)) else {
                return;
            };
            mudar(x);
            (job, indice)
        };
        self.decidir_depois(unidade.0, unidade.1);
    }

    /// Quem pede: confere uma entrega de outro nó.
    fn resultado_remoto(&self, par: u64, pedido: u64, registro: &[u8], assinatura: &[u8; 64], resultado: Vec<u8>) {
        let conferido = RegistroDeProva::decodificar(registro).map_err(|e| e.to_string()).and_then(|reg| {
            if !reg.assinatura_confere(assinatura) {
                return Err("assinatura do registro de prova não confere".into());
            }
            if hash_do_resultado(&resultado) != reg.resultado {
                return Err("os bytes entregues não têm o hash do registro".into());
            }
            Ok(reg)
        });
        self.do_remoto(par, pedido, |x| {
            let Remoto::Comprometido { worker, compromisso: c, assinatura: assinatura_do_compromisso, .. } = x.clone() else {
                *x = Remoto::Falhou { worker: None, motivo: "resultado sem compromisso antes".into() };
                return;
            };
            // Falha que o worker não assinou não pesa nele: quem repassa a
            // oferta de outro não consegue sujar a reputação do dono da chave.
            *x = match conferido {
                Ok(reg) if reg.worker != worker => Remoto::Falhou { worker: None, motivo: "o registro é de outro worker".into() },
                // o compromisso e o registro são assinados pelo mesmo worker
                // e não fecham: a prova é dele, e a falha pesa nele
                Ok(reg) if !revelacao_confere(&c, &reg.resultado, &reg.worker) => Remoto::Falhou {
                    worker: Some(worker),
                    motivo: format!(
                        "revelou um resultado diferente do compromisso que assinou (compromisso {}…, assinatura {}…)",
                        hex(c.get(..8).unwrap_or_default()),
                        hex(assinatura_do_compromisso.get(..8).unwrap_or_default())
                    ),
                },
                Ok(reg) => Remoto::Entregou(Box::new(Entrega {
                    worker: reg.worker,
                    hash: reg.resultado,
                    resultado,
                    // o tempo é declarado pelo outro nó: não entra como CPU deste JOB
                    ms_calculo: 0,
                    ms_verificacao: 0,
                    memoria: 0,
                    operacoes_verificacao: 0,
                    tarefa: reg.tarefa,
                    assinatura: Some(*assinatura),
                    registro: reg,
                })),
                Err(motivo) => Remoto::Falhou { worker: None, motivo },
            };
        });
    }

    /// Quem pede: com o que já chegou, pede a revelação ou decide.
    pub(super) fn decidir(&self, job: &[u8; HASH_LEN], indice: u64) {
        let mut revelar = Vec::new();
        let decisao = {
            let Ok(mut r) = self.rede.lock() else { return };
            let Some(u) = r.redundantes.get_mut(&(*job, indice)) else { return };
            // todos os compromissos chegaram (ou falharam) e a cópia local acabou: revela
            let todos_comprometidos = u.remotos.iter().all(|x| !matches!(x, Remoto::Esperando { .. }));
            if !u.revelou && todos_comprometidos && u.local.is_some() {
                u.revelou = true;
                for x in &u.remotos {
                    if let Remoto::Comprometido { par, pedido, .. } = x {
                        revelar.push((*par, *pedido));
                    }
                }
            }
            let terminou = u.local.is_some() && u.remotos.iter().all(|x| matches!(x, Remoto::Entregou(_) | Remoto::Falhou { .. }));
            if terminou {
                // os números de pedido desta unidade não servem mais: sem isto o
                // mapa crescia um par de entradas por unidade, para sempre
                r.pedidos.retain(|_, v| *v != (*job, indice));
                r.redundantes.remove(&(*job, indice))
            } else {
                None
            }
        };
        for (par, pedido) in revelar {
            self.enviar(par, &MensagemUltrax::Revelar { pedido });
        }
        let Some(u) = decisao else { return };
        self.decidir_unidade(job, indice, u);
    }

    fn decidir_unidade(&self, job_id: &[u8; HASH_LEN], indice: u64, u: Redundante) {
        let mut entregas: Vec<Entrega> = Vec::new();
        let mut falhas: Vec<(Option<[u8; PUBKEY_LEN]>, String)> = Vec::new();
        match u.local {
            Some(Ok(e)) => entregas.push(e),
            Some(Err(m)) => falhas.push((None, format!("cópia local: {m}"))),
            None => {}
        }
        for x in u.remotos {
            match x {
                Remoto::Entregou(e) => entregas.push(*e),
                Remoto::Falhou { worker, motivo } => falhas.push((worker, motivo)),
                _ => {}
            }
        }
        // a entrada de cada entrega tem de ser a da unidade: senão calculou outra coisa
        let entrada = hash_da_entrada(&u.esp, &u.semente).ok();
        let teto = u.esp.operacoes_maximas();
        entregas.retain(|e| {
            let ok = Some(e.registro.entrada) == entrada && e.registro.especificacao == u.esp;
            if !ok {
                falhas.push((Some(e.worker), "entrada ou especificação diferentes da unidade".into()));
                return false;
            }
            // operações declaradas acima do teto da especificação: o registro
            // (assinado pelo worker) mente sobre o trabalho, e os créditos sairiam dele
            if e.registro.operacoes > teto {
                falhas.push((Some(e.worker), format!("declarou {} operações; o teto desta unidade é {teto}", e.registro.operacoes)));
                return false;
            }
            true
        });
        let (redundancia, tipo) = self
            .jobs
            .lock()
            .ok()
            .and_then(|j| j.iter().find(|x| &x.id == job_id).map(|x| (x.esp.redundancia(), x.esp.modelo().tipo())))
            .unwrap_or((2, u.esp.tipo()));
        let quorum = usize::from(redundancia) / 2 + 1;
        let votos: Vec<([u8; PUBKEY_LEN], [u8; HASH_LEN])> = entregas.iter().map(|e| (e.worker, e.hash)).collect();
        let decisao = por_maioria(&votos, quorum);
        let aceitos: Vec<[u8; PUBKEY_LEN]> = decisao.pareceres.iter().filter(|(_, p)| *p == Parecer::Aceito).map(|(w, _)| *w).collect();
        let vencedora = entregas.iter().find(|e| aceitos.contains(&e.worker)).cloned();
        // conferência independente do resultado da maioria, aqui, antes de dar
        // nota a quem votou nele: maioria combinada num resultado errado não ganha
        let conferencia = match (&decisao.desfecho, &vencedora) {
            (Desfecho::Verificado { .. }, Some(e)) => Some(trabalho::verificar(&u.esp, &u.semente, &e.resultado)),
            _ => None,
        };
        // reputação: o que este nó viu de cada worker. As conferências da
        // minoria rodam antes de pegar a trava da rede (podem levar segundos).
        let vistos: Vec<([u8; PUBKEY_LEN], Parecer)> = decisao
            .pareceres
            .iter()
            .map(|(worker, parecer)| {
                let visto = match (&conferencia, parecer) {
                    // votou no resultado que a conferência recusou: recusa, com a evidência
                    (Some(Err(recusa)), Parecer::Aceito) => Parecer::Recusado(recusa.clone()),
                    // a minoria, com a maioria recusada, é conferida uma a uma
                    (Some(Err(_)), Parecer::Divergente) => match entregas.iter().find(|e| e.worker == *worker) {
                        Some(e) => match trabalho::verificar(&u.esp, &u.semente, &e.resultado) {
                            Ok(()) => Parecer::Aceito,
                            Err(recusa) => Parecer::Recusado(recusa),
                        },
                        None => parecer.clone(),
                    },
                    _ => parecer.clone(),
                };
                (*worker, visto)
            })
            .collect();
        if let Ok(mut r) = self.rede.lock() {
            for (worker, visto) in &vistos {
                r.reputacao.entry(*worker).or_default().registrar(visto);
            }
            for (worker, motivo) in &falhas {
                if let Some(w) = worker {
                    r.reputacao.entry(*w).or_default().registrar(&Parecer::Recusado(hyurax_ultrax::trabalho::Recusa(motivo.clone())));
                }
            }
        }
        let diagnostico = format!(
            "{} entrega(s), {} falha(s){}",
            entregas.len(),
            falhas.len(),
            falhas.iter().map(|(_, m)| format!("; {m}")).collect::<String>()
        );
        match (decisao.desfecho, vencedora) {
            (Desfecho::Verificado { aceitos: k }, Some(e)) => {
                // conferência independente aqui, antes de entrar no JOB
                if let Some(Err(_)) = conferencia {
                    self.voltar_para_a_fila(job_id, indice, tipo, format!("a maioria ({k}) concordou num resultado que a conferência daqui recusou; {diagnostico}"), true);
                    return;
                }
                // a evidência do consenso: o registro e a assinatura de cada voto
                // aceito, para uma auditoria refazer a conta depois
                let caminho = self.pasta_do_job(job_id).join("consenso.jsonl");
                for v in entregas.iter().filter(|x| aceitos.contains(&x.worker)) {
                    let linha = format!(
                        "{{\"indice\":{indice},\"worker\":\"{}\",\"resultado\":\"{}\",\"registro\":\"{}\",\"assinatura\":{}}}",
                        hex(&v.worker),
                        hex(&v.hash),
                        hex(&v.registro.codificar()),
                        v.assinatura.map_or("null".to_string(), |a| format!("\"{}\"", hex(&a)))
                    );
                    self.acrescentar(&caminho, &linha, None);
                }
                let rotulo: &'static str = match (k, votos.len()) {
                    (2, 2) => "CONSENSUS 2/2",
                    (2, 3) => "CONSENSUS 2/3",
                    (3, 3) => "CONSENSUS 3/3",
                    _ => "CONSENSUS",
                };
                let outros: Vec<[u8; PUBKEY_LEN]> = aceitos.iter().filter(|w| **w != e.worker).copied().collect();
                let d = DesfechoDeUnidade {
                    job: *job_id,
                    indice,
                    tarefa: e.tarefa,
                    estado: Estado::Liquidada,
                    resultado: Some(e.resultado.clone()),
                    registro: Some(e.registro.clone()),
                    assinatura: None,
                    operacoes_verificacao: e.operacoes_verificacao,
                    ms_calculo: e.ms_calculo,
                    ms_verificacao: e.ms_verificacao,
                    memoria: e.memoria,
                    gpu: None,
                    nota: diagnostico.clone(),
                    abandonada: false,
                };
                let mut eventos = Vec::new();
                let mut terminou_job = false;
                if let Ok(mut jobs) = self.jobs.lock()
                    && let Some(job) = jobs.iter_mut().find(|j| &j.id == job_id)
                {
                    let voo: Option<EmVoo> = job.em_voo.remove(&indice);
                    if !job.feitas.contem(indice) {
                        self.consolidar(job, &d, voo, rotulo, &outros, &mut eventos);
                        eventos.push(Evento {
                            evento: "WORK_UNIT_CONSENSUS",
                            job: Some(*job_id),
                            unidade: Some(indice),
                            no: Some(self.no),
                            tipo: Some(tipo),
                            operacao: format!("{k} de {} workers de nós diferentes concordaram; {diagnostico}", votos.len()),
                            resultado: Some(e.hash),
                            verificacao: Some(rotulo),
                            ..Evento::default()
                        });
                    }
                    terminou_job = self.fechar_se_acabou(job, &mut eventos);
                }
                for ev in eventos {
                    self.emitir(ev);
                }
                if terminou_job {
                    let _ = crate::ciencia::relatorio::gravar(self, job_id);
                }
            }
            _ => {
                // só divergência de verdade (resultados diferentes) conta como
                // falha da unidade; faltar voto (recusa, sumiço) só a devolve
                let distintos: std::collections::BTreeSet<[u8; HASH_LEN]> = votos.iter().map(|(_, h)| *h).collect();
                let divergiu = distintos.len() >= 2;
                self.voltar_para_a_fila(job_id, indice, tipo, format!("sem maioria entre workers de nós diferentes; {diagnostico}"), divergiu);
            }
        }
    }

    fn voltar_para_a_fila(&self, job_id: &[u8; HASH_LEN], indice: u64, tipo: TipoDeTrabalho, motivo: String, contar: bool) {
        let mut eventos = Vec::new();
        let mut terminou_job = false;
        if let Ok(mut jobs) = self.jobs.lock()
            && let Some(job) = jobs.iter_mut().find(|j| &j.id == job_id)
        {
            job.em_voo.remove(&indice);
            job.cursor = job.cursor.min(indice);
            let n = job.falhas.entry(indice).or_insert(0);
            if contar {
                *n = n.saturating_add(1);
            }
            let abandonar = *n >= super::FALHAS_POR_UNIDADE;
            if abandonar {
                job.abandonadas.inserir_um(indice);
            } else {
                job.repetidas = job.repetidas.saturating_add(1);
            }
            job.sujo = true;
            eventos.push(Evento {
                evento: if abandonar { "WORK_UNIT_FAILED" } else { "WORK_UNIT_DIVERGENT" },
                job: Some(*job_id),
                unidade: Some(indice),
                no: Some(self.no),
                tipo: Some(tipo),
                operacao: if abandonar { format!("{motivo}; fica registrada como falha") } else { format!("{motivo}; volta para a fila") },
                verificacao: Some("DISPUTED"),
                ..Evento::default()
            });
            terminou_job = self.fechar_se_acabou(job, &mut eventos);
        }
        for ev in eventos {
            self.emitir(ev);
        }
        if terminou_job {
            let _ = crate::ciencia::relatorio::gravar(self, job_id);
        }
    }

    /// Reputação dos workers que este nó viu, para a tela e o relatório.
    pub fn json_rede(&self) -> String {
        let Ok(r) = self.rede.lock() else { return "{}".into() };
        let agora = agora_ms();
        let ofertas: Vec<String> = r
            .ofertas
            .iter()
            .filter(|(_, o)| agora.saturating_sub(o.visto_ms) < OFERTA_VALE_MS)
            .map(|(par, o)| format!("{{\"par\":{par},\"worker\":\"{}\",\"memoria_mib\":{},\"visto\":{}}}", hex(&o.worker), o.memoria_mib, o.visto_ms))
            .collect();
        let reputacao: Vec<String> = r
            .reputacao
            .iter()
            .map(|(w, rep)| {
                format!(
                    "{{\"worker\":\"{}\",\"enviadas\":{},\"verificadas\":{},\"recusadas\":{},\"divergentes\":{},\"nota\":{}}}",
                    hex(w),
                    rep.enviadas,
                    rep.verificadas,
                    rep.recusadas,
                    rep.divergentes,
                    rep.nota()
                )
            })
            .collect();
        format!(
            "{{\"ligada\":{},\"aceitar\":{},\"ofertas\":[{}],\"reputacao\":[{}],\"remotas_na_fila\":{},\"remotas_executando\":{},\"redundantes\":{},\"mensagens_recebidas\":{},\"mensagens_enviadas\":{}}}",
            r.rede.is_some(),
            self.aceitar_rede.load(Ordering::Relaxed),
            ofertas.join(","),
            reputacao.join(","),
            r.fila.len(),
            r.executando.len(),
            r.redundantes.len(),
            r.recebidas,
            r.enviadas
        )
    }
}

/// O nível pede workers de outros nós?
pub(super) fn pede_outros_nos(nivel: Nivel) -> bool {
    nivel >= Nivel::Concordancia
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::expect_used)]
mod testes {
    use super::*;

    #[test]
    fn candidatos_respeitam_tipo_memoria_ocupacao_e_validade() {
        let mut r = EstadoDaRede::default();
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 64, 0).unwrap();
        r.ofertas.insert(1, OfertaVista { worker: [1; 32], tipos: bit_do_tipo(TipoDeTrabalho::Matriz), memoria_mib: 256, visto_ms: 1_000 });
        r.ofertas.insert(2, OfertaVista { worker: [2; 32], tipos: bit_do_tipo(TipoDeTrabalho::Rotas), memoria_mib: 256, visto_ms: 1_000 });
        r.ofertas.insert(3, OfertaVista { worker: [3; 32], tipos: 0xff, memoria_mib: 0, visto_ms: 1_000 });
        r.ofertas.insert(4, OfertaVista { worker: [4; 32], tipos: 0xff, memoria_mib: 256, visto_ms: 1_000 });
        let c: Vec<u64> = r.candidatos(&esp, 2_000).into_iter().map(|(p, _)| p).collect();
        assert_eq!(c, vec![1, 4], "o 2 não faz matriz e o 3 não tem memória");
        assert!(r.candidatos(&esp, 1_000 + OFERTA_VALE_MS + 1).is_empty(), "oferta velha não vale");
        // o worker 4 com nota melhor passa na frente
        let mut boa = Reputacao::default();
        for _ in 0..10 {
            boa.registrar(&Parecer::Aceito);
        }
        r.reputacao.insert([4; 32], boa);
        let c: Vec<u64> = r.candidatos(&esp, 2_000).into_iter().map(|(p, _)| p).collect();
        assert_eq!(c, vec![4, 1]);
    }

    // -----------------------------------------------------------------------
    // Fraude: workers de outros nós simulados, com chaves de verdade. A rede
    // não está ligada (as mensagens de saída se perdem); as de entrada chegam
    // por `tratar_ultrax`, codificadas como viriam do fio.
    // -----------------------------------------------------------------------

    use crate::ciencia::EstadoDoJob;
    use crate::ultrax::Agendador;
    use hyurax_crypto::ed25519_public_key;
    use hyurax_ultrax::job::{Dominio, PedidoDeJob};
    use hyurax_ultrax::validador::compromisso;

    const LOCAL: [u8; SECRET_LEN] = [0x10; SECRET_LEN];
    const W1: [u8; SECRET_LEN] = [0x11; SECRET_LEN];
    const W2: [u8; SECRET_LEN] = [0x12; SECRET_LEN];

    /// Como cada worker se comporta.
    #[derive(Clone, Copy, PartialEq)]
    enum Jeito {
        Honesto,
        /// Troca um byte do resultado, e compromete-se com o hash trocado.
        Adultera,
        /// Compromete-se com um hash e revela outro resultado.
        TrocaNaRevelacao,
    }

    type Pacote = (RegistroDeProva, [u8; 64], Vec<u8>);

    /// O que um worker entregaria: registro, assinatura, resultado.
    fn entrega(esp: &Especificacao, semente: &[u8], segredo: &[u8; SECRET_LEN], jeito: Jeito) -> Pacote {
        let mut resultado = trabalho::executar(esp, semente, &mut |_| true).unwrap().resultado;
        if jeito == Jeito::Adultera {
            resultado[0] ^= 1;
        }
        let registro = RegistroDeProva {
            tarefa: [5; 64],
            entrada: hash_da_entrada(esp, semente).unwrap(),
            especificacao: *esp,
            metodo: esp.tipo().metodo(),
            resultado: hash_do_resultado(&resultado),
            operacoes: 1,
            worker: ed25519_public_key(segredo),
            inicio_ms: 1,
            fim_ms: 2,
        };
        let assinatura = registro.assinar(segredo).unwrap();
        (registro, assinatura, resultado)
    }

    /// Um JOB de 1 unidade, nível 3, redundância 3: a cópia daqui e dois
    /// workers de fora. Devolve a reputação vista de cada um, o estado do
    /// JOB, as unidades feitas e os eventos gravados.
    fn rodada(nome: &str, jeitos: [Jeito; 3]) -> (Vec<Reputacao>, EstadoDoJob, u64, Vec<String>) {
        rodada_com(nome, jeitos, false)
    }

    /// `decisora`: a decisão roda na thread decisora, como no programa ligado
    /// na rede, e não na thread que entregou a última mensagem.
    fn rodada_com(nome: &str, jeitos: [Jeito; 3], decisora: bool) -> (Vec<Reputacao>, EstadoDoJob, u64, Vec<String>) {
        let pasta = std::env::temp_dir().join(format!("hyurax-ciencia-rede-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let c = Ciencia::abrir(&pasta, ed25519_public_key(&LOCAL), "teste").unwrap();
        if decisora {
            c.iniciar_decisora();
        }
        let id = c
            .submeter(PedidoDeJob {
                dominio: Dominio::Matematica,
                modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap(),
                unidades: 1,
                nivel: Nivel::Concordancia,
                redundancia: 3,
                prazo_s: 0,
                orcamento_milicreditos: 0,
                descricao: "fraude".into(),
            })
            .unwrap();
        let agora = agora_ms();
        {
            let mut r = c.rede.lock().unwrap();
            for (par, s) in [(1u64, W1), (2, W2)] {
                r.ofertas.insert(par, OfertaVista { worker: ed25519_public_key(&s), tipos: 0xffff, memoria_mib: 256, visto_ms: agora });
            }
        }
        let p = c.proxima(agora).expect("com dois workers de fora, a unidade sai");
        let pedidos: BTreeMap<u64, u64> = {
            let r = c.rede.lock().unwrap();
            let u = r.redundantes.get(&(id, 0)).expect("unidade redundante registrada");
            u.remotos
                .iter()
                .map(|x| match x {
                    Remoto::Esperando { par, pedido } => (*par, *pedido),
                    _ => panic!("esperando"),
                })
                .collect()
        };
        assert_eq!(pedidos.len(), 2, "um pedido para cada worker de fora");

        // a cópia daqui
        let (reg, _, res) = entrega(&p.especificacao, &p.semente, &LOCAL, jeitos[0]);
        c.terminou(DesfechoDeUnidade {
            job: id,
            indice: 0,
            tarefa: reg.tarefa,
            estado: Estado::Liquidada,
            resultado: Some(res),
            registro: Some(reg),
            assinatura: None,
            operacoes_verificacao: 1,
            ms_calculo: 1,
            ms_verificacao: 1,
            memoria: 1,
            gpu: None,
            nota: String::new(),
            abandonada: false,
        });
        // os de fora: primeiro todos os compromissos, depois as revelações
        let entregas: Vec<(u64, u64, Pacote, Jeito, [u8; SECRET_LEN])> = [(1u64, W1, jeitos[1]), (2, W2, jeitos[2])]
            .into_iter()
            .map(|(par, s, j)| (par, pedidos[&par], entrega(&p.especificacao, &p.semente, &s, j), j, s))
            .collect();
        for (par, pedido, (reg, _, _), jeito, s) in &entregas {
            let prometido = if *jeito == Jeito::TrocaNaRevelacao { hash_do_resultado(b"outra coisa") } else { reg.resultado };
            let m = MensagemUltrax::compromisso(s, &id, 0, *pedido, compromisso(&prometido, &reg.worker));
            c.tratar_ultrax(*par, &m.codificar().unwrap());
        }
        // com a decisora, o pedido de revelação também sai na thread dela
        let ate = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while decisora && !c.rede.lock().unwrap().redundantes.get(&(id, 0)).is_some_and(|u| u.revelou) && std::time::Instant::now() < ate {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(c.rede.lock().unwrap().redundantes.get(&(id, 0)).is_some_and(|u| u.revelou), "com todos os compromissos, pede a revelação");
        for (par, pedido, (reg, assinatura, res), _, _) in entregas {
            let m = MensagemUltrax::Resultado { pedido, registro: reg.codificar(), assinatura, resultado: res };
            c.tratar_ultrax(par, &m.codificar().unwrap());
        }
        if decisora {
            let ate = std::time::Instant::now() + std::time::Duration::from_secs(30);
            while c.rede.lock().unwrap().redundantes.contains_key(&(id, 0))
                || !c.jobs.lock().unwrap().iter().any(|x| x.id == id && x.estado.e_final())
            {
                assert!(std::time::Instant::now() < ate, "a decisora não decidiu a unidade");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
        let reps = {
            let r = c.rede.lock().unwrap();
            [LOCAL, W1, W2].iter().map(|s| r.reputacao.get(&ed25519_public_key(s)).cloned().unwrap_or_default()).collect()
        };
        let (estado, feitas) = {
            let j = c.jobs.lock().unwrap();
            let x = j.iter().find(|x| x.id == id).unwrap();
            (x.estado, x.feitas.concluidas())
        };
        let eventos = c.eventos.lock().unwrap().iter().cloned().collect();
        let _ = std::fs::remove_dir_all(&pasta);
        (reps, estado, feitas, eventos)
    }

    #[test]
    fn tres_honestos_concordam_3_de_3() {
        let (reps, estado, feitas, eventos) = rodada("honestos", [Jeito::Honesto; 3]);
        assert_eq!((estado, feitas), (EstadoDoJob::Concluido, 1));
        assert!(reps.iter().all(|r| r.verificadas == 1 && r.divergentes == 0 && r.recusadas == 0));
        assert!(eventos.iter().any(|e| e.contains("CONSENSUS 3/3")));
    }

    #[test]
    fn decisora_decide_fora_da_thread_do_par() {
        let (reps, estado, feitas, eventos) = rodada_com("decisora", [Jeito::Honesto, Jeito::Honesto, Jeito::Adultera], true);
        assert_eq!((estado, feitas), (EstadoDoJob::Concluido, 1));
        assert_eq!((reps[2].verificadas, reps[2].divergentes), (0, 1));
        assert!(eventos.iter().any(|e| e.contains("CONSENSUS 2/3")));
    }

    #[test]
    fn worker_que_adultera_perde_na_maioria_e_leva_divergencia() {
        let (reps, estado, feitas, eventos) = rodada("adultera", [Jeito::Honesto, Jeito::Honesto, Jeito::Adultera]);
        assert_eq!((estado, feitas), (EstadoDoJob::Concluido, 1), "2 de 3 honestos bastam");
        assert_eq!((reps[0].verificadas, reps[1].verificadas), (1, 1));
        assert_eq!((reps[2].verificadas, reps[2].divergentes), (0, 1), "quem adulterou fica marcado");
        assert!(eventos.iter().any(|e| e.contains("CONSENSUS 2/3")));
    }

    #[test]
    fn conluio_de_dois_e_pego_pela_conferencia_local() {
        // os dois de fora combinam o mesmo resultado falso: é maioria, mas a
        // conferência daqui recusa, a unidade volta para a fila e eles levam recusa
        let (reps, estado, feitas, eventos) = rodada("conluio", [Jeito::Honesto, Jeito::Adultera, Jeito::Adultera]);
        assert_eq!(feitas, 0, "o resultado combinado não entra no JOB");
        assert_ne!(estado, EstadoDoJob::Concluido);
        assert_eq!((reps[1].recusadas, reps[2].recusadas), (1, 1), "os dois do conluio levam recusa com evidência");
        assert_eq!((reps[1].verificadas, reps[2].verificadas), (0, 0));
        assert_eq!((reps[0].verificadas, reps[0].divergentes), (1, 0), "a cópia honesta, conferida, não é punida");
        assert!(eventos.iter().any(|e| e.contains("WORK_UNIT_DIVERGENT") && e.contains("a confer")));
    }

    #[test]
    fn oferta_velha_ou_repetida_por_outro_par_nao_entra() {
        let pasta = std::env::temp_dir().join(format!("hyurax-ciencia-oferta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let c = Ciencia::abrir(&pasta, ed25519_public_key(&LOCAL), "teste").unwrap();
        let agora = agora_ms();
        let velha = MensagemUltrax::oferta(&W1, 0xffff, 1, 256, agora.saturating_sub(OFERTA_FRESCA_MS + 60_000));
        c.tratar_ultrax(1, &velha.codificar().unwrap());
        assert!(c.rede.lock().unwrap().ofertas.is_empty(), "oferta de horário velho não vale");
        let fresca = MensagemUltrax::oferta(&W1, 0xffff, 1, 256, agora);
        c.tratar_ultrax(1, &fresca.codificar().unwrap());
        // outro par repete a mesma oferta assinada: não toma o lugar do primeiro
        c.tratar_ultrax(2, &fresca.codificar().unwrap());
        let r = c.rede.lock().unwrap();
        assert_eq!(r.ofertas.keys().copied().collect::<Vec<_>>(), vec![1]);
        drop(r);
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn pedido_com_semente_que_nao_e_da_unidade_e_recusado() {
        let pasta = std::env::temp_dir().join(format!("hyurax-ciencia-semente-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let partida = crate::ultrax::Partida { ligado: true, linhas: 1, uso_cpu: 100, memoria_mib: 256, debug: false, gpu: false, gpu_uso: 50 };
        let u = crate::ultrax::Ultrax::abrir(&pasta, &[9; 32], 1, &partida, Box::new(|_, _| {}));
        let c = Ciencia::abrir(&pasta, ed25519_public_key(&LOCAL), "teste").unwrap();
        c.ligar(&u);
        c.aceitar_da_rede(true);
        let job = [3u8; HASH_LEN];
        let remota = |semente| Remota {
            par: 1,
            pedido: 1,
            job,
            indice: 0,
            esp: Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap(),
            semente,
            prazo_ms: agora_ms() + 60_000,
            com_compromisso: true,
        };
        let motivo = c.aceitar_pedido(remota([7; HASH_LEN])).unwrap();
        assert!(motivo.contains("semente"), "{motivo}");
        assert!(c.aceitar_pedido(remota(hyurax_ultrax::job::semente_da_unidade(&job, 0))).is_none(), "a semente certa entra");
        // o mesmo pedido de novo (outro par, mesma unidade): recusado
        let mut outra = remota(hyurax_ultrax::job::semente_da_unidade(&job, 0));
        outra.par = 2;
        assert!(c.aceitar_pedido(outra).unwrap().contains("já calculo"));
        u.encerrar_threads();
        c.encerrar_vigia();
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn revelacao_que_nao_bate_com_o_compromisso_falha() {
        let (reps, estado, _, eventos) = rodada("revelacao", [Jeito::Honesto, Jeito::Honesto, Jeito::TrocaNaRevelacao]);
        assert_eq!(estado, EstadoDoJob::Concluido, "sobram 2 de 3 honestos");
        // a entrega trocada não conta, e pesa no worker: o compromisso e o
        // registro são assinados por ele e não fecham
        assert_eq!((reps[2].verificadas, reps[2].recusadas), (0, 1));
        assert!(eventos.iter().any(|e| e.contains("CONSENSUS") && e.contains("compromisso que assinou")));
    }

    #[test]
    fn compromisso_com_assinatura_de_outra_unidade_nao_entra() {
        // a assinatura de um compromisso vale para o JOB e o índice do pedido;
        // reaproveitada em outro pedido, não confere
        let valor = compromisso(&hash_do_resultado(b"r"), &ed25519_public_key(&W1));
        let m = MensagemUltrax::compromisso(&W1, &[1; 64], 0, 7, valor);
        assert!(m.compromisso_confere(&[1; 64], 0));
        assert!(!m.compromisso_confere(&[1; 64], 1));
        assert!(!m.compromisso_confere(&[2; 64], 0));
    }
}
