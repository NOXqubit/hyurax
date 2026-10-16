//! O registro de nós (seção 8), o agente (seção 9), a telemetria (31) e o
//! pulso (32).
//!
//! O agente só fala de dentro para fora (HTTPS, pedido assinado): o
//! computador do fornecedor não abre porta nenhuma. A cada pulso ele conta o
//! que tem livre e recebe de volta o que parar e o que fazer.

use serde::Deserialize;
use serde_json::{Value, json};

use super::contas::Quem;
use super::{Amostra, Erro, Plataforma, Pulso, R};
use crate::auth;
use crate::estado::{Estado, No};
use crate::eventos::{Desfecho, EstadoAdm, EstadoTarefa, Evento};
use crate::modelo::{Hardware, Id, Limites, texto_curto, versao};
use crate::regras;
use crate::util::{de_hex_fixo, hex};

/// Pulso esperado a cada isso.
pub const PULSO_MS: u64 = 5_000;
/// Sem pulso por isso: instável.
pub const INSTAVEL_MS: u64 = 20_000;
/// Sem pulso por isso: offline.
pub const OFFLINE_MS: u64 = 60_000;
/// Código de vínculo vale por isso.
pub const VINCULO_MS: u64 = 15 * 60_000;
/// Amostras de telemetria guardadas por nó (30 min a cada 5 s).
const AMOSTRAS: usize = 360;
/// Computadores por fornecedor.
const NOS_POR_FORNECEDOR: usize = 50;

/// O estado de um nó para a tela (seção 8): `OFFLINE`, `UNSTABLE`, `IDLE`,
/// `BUSY`, `PAUSED`, `QUARANTINED`, `MAINTENANCE`, `BANNED` ou `REMOVED`.
pub fn estado_do_no(no: &No, pulso: Option<&Pulso>, abertas: usize, agora: u64) -> &'static str {
    if no.removido {
        return "REMOVED";
    }
    match no.adm {
        EstadoAdm::Banned => return "BANNED",
        EstadoAdm::Quarantined => return "QUARANTINED",
        EstadoAdm::Maintenance => return "MAINTENANCE",
        EstadoAdm::Normal => {}
    }
    let idade = pulso.map_or(u64::MAX, |p| agora.saturating_sub(p.ultimo_ms));
    if idade > OFFLINE_MS {
        return "OFFLINE";
    }
    if no.pausado || pulso.is_some_and(|p| p.pausado_local) {
        return "PAUSED";
    }
    if idade > INSTAVEL_MS {
        return "UNSTABLE";
    }
    if abertas > 0 { "BUSY" } else { "IDLE" }
}

/// Atribuições abertas de um nó.
pub fn abertas_do_no(e: &Estado, no: Id) -> Vec<Id> {
    e.atribuicoes.values().filter(|a| a.no == no && a.aberta()).map(|a| a.id).collect()
}

/// O que o agente manda no registro.
#[derive(Clone, Debug, Deserialize)]
pub struct PedidoDeRegistro {
    /// O código de vínculo que o fornecedor copiou do painel.
    pub codigo: String,
    /// Nome do computador.
    #[serde(default)]
    pub nome: String,
    /// Hardware.
    #[serde(default)]
    pub hardware: Hardware,
    /// Versão do agente.
    #[serde(default)]
    pub versao: String,
    /// Limites que o fornecedor escolheu no programa.
    #[serde(default)]
    pub limites: Option<Limites>,
}

/// Progresso de uma atribuição, pelo agente.
#[derive(Clone, Debug, Deserialize)]
pub struct Progresso {
    /// Atribuição.
    pub atribuicao: Id,
    /// Operações feitas.
    #[serde(default)]
    pub feitas: u64,
    /// Total.
    #[serde(default)]
    pub total: u64,
    /// Já começou a executar.
    #[serde(default)]
    pub executando: bool,
}

/// Telemetria do pulso.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Telemetria {
    /// CPU, em %.
    pub cpu: Option<f32>,
    /// Memória usada, em MiB.
    pub ram_usada_mib: Option<u64>,
    /// GPU, em %.
    pub gpu: Option<f32>,
    /// Memória da GPU usada.
    pub vram_mib: Option<u64>,
    /// Temperatura, em °C.
    pub temperatura: Option<f32>,
    /// Potência estimada.
    pub potencia_w: Option<f32>,
    /// Bytes de rede.
    #[serde(default)]
    pub rede_bytes: u64,
}

/// `null` vira o padrão (o `#[serde(default)]` só cobre o campo ausente).
fn nulo_e_padrao<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// O pulso do agente.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PedidoDePulso {
    /// Versão do agente.
    #[serde(default)]
    pub versao: String,
    /// Hardware (quando mudou).
    #[serde(default)]
    pub hardware: Option<Hardware>,
    /// Linhas de CPU livres para a plataforma.
    #[serde(default)]
    pub livres: u32,
    /// Memória livre para a plataforma, em MiB.
    #[serde(default)]
    pub ram_livre_mib: u64,
    /// Códigos dos motores que o agente executa.
    #[serde(default)]
    pub tipos: Vec<u8>,
    /// GPU disponível.
    #[serde(default)]
    pub gpu: bool,
    /// Pausado pelo dono no programa.
    #[serde(default)]
    pub pausado: bool,
    /// Atribuições recebidas e o progresso delas.
    #[serde(default)]
    pub progresso: Vec<Progresso>,
    /// Telemetria (pode faltar ou vir nula: as métricas do sistema ainda não leram nada).
    #[serde(default, deserialize_with = "nulo_e_padrao")]
    pub telemetria: Telemetria,
}

impl Plataforma {
    /// Confere um pedido assinado do agente: assinatura, janela de tempo,
    /// repetição e ritmo. Devolve a chave do worker.
    pub fn autenticar_agente(&self, metodo: &str, caminho: &str, chave: &str, tempo: &str, assinatura: &str, corpo: &[u8]) -> R<[u8; 32]> {
        let chave = de_hex_fixo::<32>(chave).ok_or_else(|| Erro { status: 401, mensagem: "cabeçalho X-Hyx-No inválido".into() })?;
        let assinatura = de_hex_fixo::<64>(assinatura).ok_or_else(|| Erro { status: 401, mensagem: "cabeçalho X-Hyx-Assinatura inválido".into() })?;
        let tempo: u64 = tempo.trim().parse().map_err(|_| Erro { status: 401, mensagem: "cabeçalho X-Hyx-Tempo inválido".into() })?;
        let agora = self.agora();
        regras::conferir_pedido(&chave, metodo, caminho, tempo, corpo, &assinatura, agora).map_err(|m| Erro { status: 401, mensagem: m.into() })?;
        // a mesma assinatura duas vezes é repetição (alguém copiou o pedido)
        let marca: [u8; 32] = hyurax_crypto::sha512(&assinatura).first_chunk::<32>().copied().unwrap_or([0; 32]);
        let repetido = {
            let mut v = self.vivo()?;
            let janela = regras::JANELA_MS.saturating_mul(2);
            while v.assinaturas_ordem.front().is_some_and(|(_, t)| agora.saturating_sub(*t) > janela) {
                if let Some((m, _)) = v.assinaturas_ordem.pop_front() {
                    v.assinaturas.remove(&m);
                }
            }
            match v.assinaturas.entry(marca) {
                std::collections::hash_map::Entry::Occupied(_) => true,
                std::collections::hash_map::Entry::Vacant(livre) => {
                    livre.insert(agora);
                    v.assinaturas_ordem.push_back((marca, agora));
                    false
                }
            }
        };
        if repetido {
            self.alertar_uma_vez("seguranca:repeticao", "media", &format!("chave:{}", hex(&chave)), "pedido de agente repetido (mesma assinatura)")?;
            return Err(Erro { status: 401, mensagem: "pedido repetido".into() });
        }
        if !self.ritmo(&format!("agente:{}", hex(&chave)), 240) {
            return Err(Erro::limite("pulsos demais"));
        }
        Ok(chave)
    }

    /// Gera um código de vínculo para o fornecedor registrar um computador.
    pub fn criar_vinculo(&self, q: &Quem) -> R<Value> {
        q.fornecedor()?;
        let (codigo, hash) = auth::codigo_de_vinculo().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        let ativos = n.estado.nos.values().filter(|x| x.dono == q.usuario && !x.removido).count();
        if ativos >= NOS_POR_FORNECEDOR {
            return Err(Erro::conflito(format!("limite de {NOS_POR_FORNECEDOR} computadores por conta")));
        }
        let t = self.agora();
        let expira = t.saturating_add(VINCULO_MS);
        self.registrar(&mut n, vec![Evento::LinkCodeIssued { codigo: hash, usuario: q.usuario, expira, t }])?;
        Ok(json!({ "codigo": codigo, "expira_ms": expira, "plataforma": self.opcoes.endereco_publico }))
    }

    /// Registra um computador (o agente, com o código de vínculo).
    pub fn registrar_no(&self, chave: [u8; 32], p: PedidoDeRegistro) -> R<Value> {
        let codigo = auth::hash_do_segredo(&auth::normalizar_codigo(&p.codigo));
        let mut n = self.nucleo()?;
        let t = self.agora();
        if let Some(&id) = n.estado.no_por_chave.get(&chave) {
            // o mesmo computador vinculando de novo: devolve o número que já tem
            let no = n.estado.nos.get(&id).ok_or_else(|| Erro::interno("nó sumiu"))?;
            return Ok(json!({ "no": id, "nome": no.nome, "ja_registrado": true }));
        }
        let v = n.estado.vinculos.get(&codigo).cloned().filter(|v| !v.usado && v.expira_ms > t).ok_or_else(|| Erro::proibido("código de vínculo inválido, usado ou vencido; gere outro no painel"))?;
        let dono = n.estado.usuarios.get(&v.usuario).filter(|u| !u.bloqueado && u.papeis.fornecedor).map(|u| u.id).ok_or_else(|| Erro::proibido("a conta do fornecedor não está ativa"))?;
        let hardware = p.hardware.limpo();
        let id = n.estado.seq.no;
        let nome = {
            let x = texto_curto(&p.nome, 60);
            if x.is_empty() { format!("Computador {id}") } else { x }
        };
        let limites = p.limites.map_or_else(Limites::default, |l| limitar(l, &hardware));
        // o mesmo hardware em donos diferentes é sinal de conta duplicada
        let impressao = (hardware.cpu_modelo.clone(), hardware.nucleos, hardware.ram_mib, hardware.gpu_modelo.clone());
        let parecido = n
            .estado
            .nos
            .values()
            .any(|x| x.dono != dono && !x.removido && (x.hardware.cpu_modelo.clone(), x.hardware.nucleos, x.hardware.ram_mib, x.hardware.gpu_modelo.clone()) == impressao && impressao.0.len() > 3);
        let versao_txt = texto_curto(&p.versao, 20);
        let mut eventos = vec![Evento::NodeRegistered { id, dono, chave: hex(&chave), nome: nome.clone(), hardware, versao: versao_txt, codigo, limites, t }];
        if parecido {
            eventos.push(Evento::SecurityAlert {
                id: n.estado.seq.alerta,
                tipo: "fraude:hardware-repetido".into(),
                severidade: "baixa".into(),
                alvo: format!("no:{id}"),
                detalhe: "o mesmo modelo de hardware já está registrado por outra conta (pode ser coincidência)".into(),
                t,
            });
        }
        self.registrar(&mut n, eventos)?;
        Ok(json!({ "no": id, "nome": nome, "limites": limites_json(&limites), "ja_registrado": false }))
    }

    /// O pulso: guarda telemetria, manda parar o que deve parar e entrega
    /// trabalho novo, se o nó puder receber.
    #[allow(clippy::too_many_lines)]
    pub fn pulso(&self, chave: [u8; 32], p: PedidoDePulso) -> R<Value> {
        let agora = self.agora();
        let mut n = self.nucleo()?;
        let id = *n.estado.no_por_chave.get(&chave).ok_or_else(|| Erro { status: 410, mensagem: "este computador não está registrado; vincule de novo pelo painel".into() })?;
        let no = n.estado.nos.get(&id).cloned().ok_or_else(|| Erro::interno("nó sumiu"))?;
        if no.adm == EstadoAdm::Banned {
            return Err(Erro::proibido("este computador foi banido da plataforma"));
        }
        let mut eventos = Vec::new();
        if !no.online {
            eventos.push(Evento::NodePresence { no: id, online: true, t: agora });
        }
        if let Some(h) = p.hardware.clone() {
            let h = h.limpo();
            let versao_txt = texto_curto(&p.versao, 20);
            if h != no.hardware || versao_txt != no.versao {
                eventos.push(Evento::NodeUpdated { no: id, hardware: h, versao: versao_txt, t: agora });
            }
        }
        // recebidas e começadas
        let mut primeiras = Vec::new();
        for pr in &p.progresso {
            if let Some(a) = n.estado.atribuicoes.get(&pr.atribuicao).filter(|a| a.no == id && a.aberta())
                && ((!a.recebida) || (pr.executando && !a.executando))
            {
                eventos.push(Evento::AssignmentProgress { atribuicao: a.id, executando: pr.executando, t: agora });
                primeiras.push((a.tarefa, pr.executando));
            }
        }
        // a tarefa anda de estado com a primeira confirmação e a primeira execução
        for (tarefa, executando) in primeiras {
            if let Some(x) = n.estado.tarefas.get(&tarefa) {
                let novo = match x.estado {
                    EstadoTarefa::Scheduled if executando => Some(EstadoTarefa::Running),
                    EstadoTarefa::Scheduled => Some(EstadoTarefa::Dispatched),
                    EstadoTarefa::Dispatched if executando => Some(EstadoTarefa::Running),
                    _ => None,
                };
                if let Some(e) = novo
                    && !eventos.iter().any(|ev| matches!(ev, Evento::TaskState { tarefa: t2, estado, .. } if *t2 == tarefa && *estado == e))
                {
                    eventos.push(Evento::TaskState { tarefa, estado: e, motivo: String::new(), por: 0, t: agora });
                }
            }
        }
        self.registrar(&mut n, std::mem::take(&mut eventos))?;
        // o que o agente deve parar: canceladas aqui e atribuições que ele
        // diz ter mas que já fecharam (prazo, tarefa cancelada)
        let mut cancelar: Vec<Id> = {
            let mut v = self.vivo()?;
            v.cancelar.remove(&id).unwrap_or_default()
        };
        for pr in &p.progresso {
            let fechada = n.estado.atribuicoes.get(&pr.atribuicao).is_none_or(|a| a.no != id || !a.aberta());
            if fechada && !cancelar.contains(&pr.atribuicao) {
                cancelar.push(pr.atribuicao);
            }
        }
        if n.estado.config.parada_de_emergencia {
            for a in abertas_do_no(&n.estado, id) {
                if !cancelar.contains(&a) {
                    cancelar.push(a);
                }
            }
        }
        // guarda o pulso
        let mut tipos: u16 = 0;
        for &c in &p.tipos {
            if (1..=15).contains(&c) {
                tipos |= 1u16.wrapping_shl(u32::from(c));
            }
        }
        {
            let mut v = self.vivo()?;
            let pulso = v.pulsos.entry(id).or_insert_with(Pulso::default);
            let seguido = agora.saturating_sub(pulso.ultimo_ms) <= INSTAVEL_MS;
            pulso.seguidos = if seguido { pulso.seguidos.saturating_add(1) } else { 1 };
            pulso.ultimo_ms = agora;
            pulso.livres = p.livres.min(no.limites.linhas);
            pulso.ram_livre_mib = p.ram_livre_mib.min(no.limites.ram_mib);
            pulso.tipos = tipos;
            pulso.gpu = p.gpu && no.limites.gpu;
            pulso.pausado_local = p.pausado;
            pulso.progresso = p.progresso.iter().map(|x| (x.atribuicao, x.feitas, x.total)).collect();
            let tel = &p.telemetria;
            pulso.telemetria.push_back(Amostra {
                t: agora,
                cpu: tel.cpu.filter(|x| x.is_finite()).map(|x| x.clamp(0.0, 100.0)),
                ram_usada_mib: tel.ram_usada_mib,
                gpu: tel.gpu.filter(|x| x.is_finite()).map(|x| x.clamp(0.0, 100.0)),
                vram_mib: tel.vram_mib,
                temperatura: tel.temperatura.filter(|x| x.is_finite() && (-40.0..=150.0).contains(x)),
                potencia_w: tel.potencia_w.filter(|x| x.is_finite() && (0.0..=5000.0).contains(x)),
                rede_bytes: tel.rede_bytes,
                ativas: u32::try_from(p.progresso.len()).unwrap_or(u32::MAX),
            });
            while pulso.telemetria.len() > AMOSTRAS {
                pulso.telemetria.pop_front();
            }
        }
        // estado e motivo de não receber trabalho
        let no_atual = n.estado.nos.get(&id).cloned().ok_or_else(|| Erro::interno("nó sumiu"))?;
        let minima = n.estado.config.versao_minima_agente.clone();
        let desatualizado = versao(&p.versao).zip(versao(&minima)).is_none_or(|(a, b)| a < b);
        let (estado_txt, mensagem, pode) = if n.estado.config.parada_de_emergencia {
            ("ONLINE", "parada de emergência: nenhum trabalho até a equipe liberar", false)
        } else if no_atual.adm == EstadoAdm::Quarantined {
            ("QUARANTINED", "em quarentena: resultados divergentes; a equipe vai analisar", false)
        } else if no_atual.adm == EstadoAdm::Maintenance {
            ("MAINTENANCE", "em manutenção pela equipe", false)
        } else if no_atual.removido {
            ("REMOVED", "computador removido do painel", false)
        } else if no_atual.pausado || p.pausado {
            ("PAUSED", "pausado pelo dono", false)
        } else if desatualizado {
            ("ONLINE", "atualize o programa para receber trabalho", false)
        } else if n.estado.usuarios.get(&no_atual.dono).is_none_or(|u| u.bloqueado) {
            ("ONLINE", "a conta do fornecedor está bloqueada", false)
        } else {
            ("ONLINE", "", true)
        };
        let atribuicoes = if pode { self.despachar(&mut n, id, agora)? } else { Vec::new() };
        let abertas = abertas_do_no(&n.estado, id).len();
        let estado_txt = if estado_txt == "ONLINE" { if abertas > 0 { "BUSY" } else { "IDLE" } } else { estado_txt };
        Ok(json!({
            "no": id,
            "estado": estado_txt,
            "mensagem": mensagem,
            "cancelar": cancelar,
            "versao_minima": minima,
            "atualizar": desatualizado,
            "atribuicoes": atribuicoes,
            "limites": limites_json(&no_atual.limites),
            "proximo_pulso_ms": PULSO_MS,
        }))
    }

    /// O agente desistiu de uma atribuição (memória, erro que se repete).
    pub fn falha_do_agente(&self, chave: [u8; 32], atribuicao: Id, motivo: &str) -> R<()> {
        let mut n = self.nucleo()?;
        let id = *n.estado.no_por_chave.get(&chave).ok_or_else(|| Erro { status: 410, mensagem: "computador não registrado".into() })?;
        let a = n.estado.atribuicoes.get(&atribuicao).filter(|a| a.no == id && a.aberta()).ok_or_else(|| Erro::nao_achado("atribuição não está aberta neste computador"))?;
        let tarefa = a.tarefa;
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::AssignmentClosed { atribuicao, desfecho: Desfecho::Abandonada, motivo: texto_curto(motivo, 200), t }])?;
        self.depois_de_mudar_unidade(&mut n, tarefa)?;
        Ok(())
    }

    /// O resumo que o programa do fornecedor mostra.
    pub fn resumo_do_agente(&self, chave: [u8; 32]) -> R<Value> {
        let agora = self.agora();
        let n = self.nucleo()?;
        let id = *n.estado.no_por_chave.get(&chave).ok_or_else(|| Erro { status: 410, mensagem: "computador não registrado".into() })?;
        let no = n.estado.nos.get(&id).ok_or_else(|| Erro::interno("nó sumiu"))?;
        let v = self.vivo()?;
        let abertas = abertas_do_no(&n.estado, id).len();
        let estado = estado_do_no(no, v.pulsos.get(&id), abertas, agora);
        let dono = n.estado.usuarios.get(&no.dono);
        let moeda = dono.map_or(crate::dinheiro::Moeda::Brl, |u| u.moeda);
        let livro = &n.estado.livro;
        Ok(json!({
            "no": id,
            "nome": no.nome,
            "estado": estado,
            "reputacao": no.historico.pontuacao(),
            "faixa": regras::faixa(no.historico.pontuacao()),
            "entregas": no.historico.entregas,
            "aceitas": no.historico.aceitas,
            "incorretas": no.historico.incorretas,
            "abertas": abertas,
            "ganhos": no.ganhos.iter().map(|(m, c)| json!({ "moeda": m.codigo(), "centavos": c, "texto": m.formatar(*c) })).collect::<Vec<_>>(),
            "conta": dono.map(|u| json!({
                "pendente": moeda.formatar(livro.saldo(crate::dinheiro::Conta::FornecedorPendente(u.id), moeda)),
                "disponivel": moeda.formatar(livro.saldo(crate::dinheiro::Conta::FornecedorDisponivel(u.id), moeda)),
            })),
        }))
    }

    /// Os computadores do fornecedor.
    pub fn meus_nos(&self, q: &Quem) -> R<Value> {
        q.fornecedor()?;
        let agora = self.agora();
        let n = self.nucleo()?;
        let v = self.vivo()?;
        let lista: Vec<Value> = n.estado.nos.values().filter(|x| x.dono == q.usuario && !x.removido).map(|x| json_do_no(&n.estado, x, v.pulsos.get(&x.id), agora, false)).collect();
        Ok(json!({ "nos": lista }))
    }

    /// Um computador, com a telemetria e as últimas unidades.
    pub fn detalhe_do_no(&self, q: &Quem, id: Id) -> R<Value> {
        let agora = self.agora();
        let n = self.nucleo()?;
        let no = n.estado.nos.get(&id).filter(|x| x.dono == q.usuario || q.papeis.admin).ok_or_else(|| Erro::nao_achado("computador não encontrado"))?;
        let v = self.vivo()?;
        let pulso = v.pulsos.get(&id);
        let mut j = json_do_no(&n.estado, no, pulso, agora, true);
        let mut recentes: Vec<&crate::estado::Atribuicao> = n.estado.atribuicoes.values().filter(|a| a.no == id).collect();
        recentes.sort_by_key(|a| std::cmp::Reverse(a.id));
        crate::util::por(&mut j, "unidades", Value::Array(
            recentes
                .into_iter()
                .take(40)
                .map(|a| {
                    let tipo = n.estado.tarefas.get(&a.tarefa).map(|t| t.pedido.tipo.clone()).unwrap_or_default();
                    json!({
                        "atribuicao": a.id, "tarefa": a.tarefa, "unidade": a.unidade, "tipo": tipo,
                        "emitida_ms": a.emitida_ms, "prazo_ms": a.prazo_ms,
                        "desfecho": a.desfecho.as_ref().map(|d| desfecho_nome(d.0)),
                        "ms_calculo": a.entrega.as_ref().map(|e| e.ms_calculo),
                    })
                })
                .collect(),
        ));
        Ok(j)
    }

    /// O fornecedor muda nome, região ou limites.
    pub fn ajustar_no(&self, q: &Quem, id: Id, nome: Option<String>, regiao: Option<String>, limites: Option<Limites>) -> R<Value> {
        q.fornecedor()?;
        let mut n = self.nucleo()?;
        let no = n.estado.nos.get(&id).filter(|x| x.dono == q.usuario && !x.removido).cloned().ok_or_else(|| Erro::nao_achado("computador não encontrado"))?;
        let nome = nome.map_or(no.nome.clone(), |x| texto_curto(&x, 60));
        if nome.is_empty() {
            return Err(Erro::pedido("o nome não pode ficar vazio"));
        }
        let regiao = regiao.map_or(no.regiao.clone(), |x| texto_curto(&x, 24).to_uppercase());
        let limites = limites.map_or(no.limites, |l| limitar(l, &no.hardware));
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::NodeSettings { no: id, nome, regiao, limites, t }])?;
        Ok(json!({ "ok": true, "limites": limites_json(&limites) }))
    }

    /// O fornecedor pausa ou retoma; pausar cancela o que está em voo.
    pub fn pausar_no(&self, q: &Quem, id: Id, pausar: bool) -> R<()> {
        q.fornecedor()?;
        let mut n = self.nucleo()?;
        n.estado.nos.get(&id).filter(|x| x.dono == q.usuario && !x.removido).ok_or_else(|| Erro::nao_achado("computador não encontrado"))?;
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::NodePaused { no: id, pausado: pausar, t }])?;
        if pausar {
            self.cancelar_do_no(&mut n, id, "pausado pelo dono")?;
        }
        Ok(())
    }

    /// O fornecedor tira o computador da plataforma.
    pub fn remover_no(&self, q: &Quem, id: Id) -> R<()> {
        q.fornecedor()?;
        let mut n = self.nucleo()?;
        n.estado.nos.get(&id).filter(|x| x.dono == q.usuario && !x.removido).ok_or_else(|| Erro::nao_achado("computador não encontrado"))?;
        self.cancelar_do_no(&mut n, id, "computador removido")?;
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::NodeRemoved { no: id, por: q.usuario, t }])
    }

    /// Cancela as atribuições abertas de um nó e manda o agente parar.
    pub fn cancelar_do_no(&self, n: &mut super::Nucleo, no: Id, motivo: &str) -> R<()> {
        let abertas = abertas_do_no(&n.estado, no);
        if abertas.is_empty() {
            return Ok(());
        }
        let t = self.agora();
        let tarefas: Vec<Id> = abertas.iter().filter_map(|a| n.estado.atribuicoes.get(a).map(|x| x.tarefa)).collect();
        let eventos = abertas.iter().map(|&a| Evento::AssignmentClosed { atribuicao: a, desfecho: Desfecho::Cancelada, motivo: motivo.into(), t }).collect();
        self.registrar(n, eventos)?;
        self.vivo()?.cancelar.entry(no).or_default().extend(abertas);
        for tarefa in tarefas {
            self.depois_de_mudar_unidade(n, tarefa)?;
        }
        Ok(())
    }

    pub(crate) fn vigiar_presenca(&self) -> R<()> {
        let agora = self.agora();
        let mut n = self.nucleo()?;
        let parados: Vec<Id> = {
            let v = self.vivo()?;
            n.estado.nos.values().filter(|x| x.online && v.pulsos.get(&x.id).is_none_or(|p| agora.saturating_sub(p.ultimo_ms) > OFFLINE_MS)).map(|x| x.id).collect()
        };
        if parados.is_empty() {
            return Ok(());
        }
        let eventos = parados.iter().map(|&no| Evento::NodePresence { no, online: false, t: agora }).collect();
        self.registrar(&mut n, eventos)
    }
}

/// Os limites do fornecedor, presos ao hardware que o nó tem.
pub fn limitar(l: Limites, h: &Hardware) -> Limites {
    let nucleos = if h.nucleos == 0 { 64 } else { h.nucleos };
    let ram = if h.ram_mib == 0 { 1 << 20 } else { h.ram_mib };
    Limites { linhas: l.linhas.clamp(1, nucleos), ram_mib: l.ram_mib.clamp(256, ram), gpu: l.gpu && !h.gpu_modelo.is_empty() }
}

/// Os limites em JSON.
pub fn limites_json(l: &Limites) -> Value {
    json!({ "linhas": l.linhas, "ram_mib": l.ram_mib, "gpu": l.gpu })
}

/// O nome de um desfecho.
pub fn desfecho_nome(d: Desfecho) -> &'static str {
    match d {
        Desfecho::Aceita => "aceita",
        Desfecho::Recusada => "recusada",
        Desfecho::Divergente => "divergente",
        Desfecho::Vencida => "vencida",
        Desfecho::Abandonada => "abandonada",
        Desfecho::Cancelada => "cancelada",
        Desfecho::Dispensada => "dispensada",
    }
}

/// Um nó em JSON, para o painel do fornecedor e do administrador.
pub fn json_do_no(e: &Estado, no: &No, pulso: Option<&Pulso>, agora: u64, com_telemetria: bool) -> Value {
    let abertas = abertas_do_no(e, no.id).len();
    let estado = estado_do_no(no, pulso, abertas, agora);
    let h = &no.hardware;
    let p = no.historico.pontuacao();
    let online_total = no.online_total_ms.saturating_add(if no.online { agora.saturating_sub(no.presenca_desde_ms) } else { 0 });
    let vida = agora.saturating_sub(no.criado_ms).max(1);
    let ultima = pulso.and_then(|x| x.telemetria.back().copied());
    let mut j = json!({
        "id": no.id,
        "dono": no.dono,
        "nome": no.nome,
        "estado": estado,
        "regiao": no.regiao,
        "versao": no.versao,
        "hardware": {
            "cpu_modelo": h.cpu_modelo, "nucleos": h.nucleos, "ram_mib": h.ram_mib,
            "gpu_modelo": h.gpu_modelo, "gpu_vram_mib": h.gpu_vram_mib, "disco_livre_mib": h.disco_livre_mib, "so": h.so,
        },
        "limites": limites_json(&no.limites),
        "reputacao": { "pontuacao": p, "faixa": regras::faixa(p), "entregas": no.historico.entregas, "aceitas": no.historico.aceitas,
                       "verificadas": no.historico.verificadas, "falhas": no.historico.falhas, "incorretas": no.historico.incorretas },
        "disponibilidade_pct": (online_total as f64 / vida as f64 * 100.0).min(100.0),
        "ultimo_pulso_ms": pulso.map(|x| x.ultimo_ms),
        "abertas": abertas,
        "ganhos": no.ganhos.iter().map(|(m, c)| json!({ "moeda": m.codigo(), "centavos": c, "texto": m.formatar(*c) })).collect::<Vec<_>>(),
        "motivo": no.motivo_adm,
        "criado_ms": no.criado_ms,
        "agora": ultima.map(amostra_json),
    });
    if com_telemetria {
        crate::util::por(&mut j, "telemetria", Value::Array(pulso.map(|x| x.telemetria.iter().copied().map(amostra_json).collect()).unwrap_or_default()));
        crate::util::por(&mut j, "progresso", Value::Array(pulso.map(|x| x.progresso.iter().map(|(a, f, t)| json!({ "atribuicao": a, "feitas": f, "total": t })).collect()).unwrap_or_default()));
    }
    j
}

fn amostra_json(a: Amostra) -> Value {
    json!({
        "t": a.t, "cpu": a.cpu, "ram_usada_mib": a.ram_usada_mib, "gpu": a.gpu, "vram_mib": a.vram_mib,
        "temperatura": a.temperatura, "potencia_w": a.potencia_w, "rede_bytes": a.rede_bytes, "ativas": a.ativas,
    })
}
