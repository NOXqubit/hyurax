//! O serviço: o diário, o estado e o que não vai para o diário (pulsos,
//! telemetria, limites de ritmo), e as operações que a API chama.
//!
//! Uma trava só (`nucleo`) guarda diário e estado juntos: toda operação que
//! muda alguma coisa decide olhando o estado, grava os eventos no diário (com
//! `sync`) e só então aplica. Duas operações nunca se cruzam no meio.
//!
//! A ordem das travas é sempre `nucleo` → `vivo`. Nenhum código pega `vivo` e
//! depois `nucleo`.

pub mod admin;
pub mod agendador;
pub mod contas;
pub mod financeiro;
pub mod fraude;
pub mod nos;
pub mod tarefas;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::diario::Diario;
use crate::dinheiro::{Lancamento, Linha, Motivo, normalizar};
use crate::estado::Estado;
use crate::eventos::Evento;
use crate::modelo::Id;
use crate::util::agora_ms;

/// Um erro que vira resposta HTTP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Erro {
    /// Status HTTP.
    pub status: u16,
    /// Mensagem para quem pediu.
    pub mensagem: String,
}

impl Erro {
    /// 400.
    pub fn pedido(m: impl Into<String>) -> Self {
        Self { status: 400, mensagem: m.into() }
    }
    /// 401.
    pub fn sem_sessao() -> Self {
        Self { status: 401, mensagem: "entre na sua conta".into() }
    }
    /// 403.
    pub fn proibido(m: impl Into<String>) -> Self {
        Self { status: 403, mensagem: m.into() }
    }
    /// 404.
    pub fn nao_achado(m: impl Into<String>) -> Self {
        Self { status: 404, mensagem: m.into() }
    }
    /// 409.
    pub fn conflito(m: impl Into<String>) -> Self {
        Self { status: 409, mensagem: m.into() }
    }
    /// 429.
    pub fn limite(m: impl Into<String>) -> Self {
        Self { status: 429, mensagem: m.into() }
    }
    /// 503.
    pub fn indisponivel(m: impl Into<String>) -> Self {
        Self { status: 503, mensagem: m.into() }
    }
    /// 500: o detalhe vai para o log do servidor, não para a resposta.
    pub fn interno(detalhe: &str) -> Self {
        eprintln!("[erro interno] {detalhe}");
        Self { status: 500, mensagem: "erro interno; tente de novo em instantes".into() }
    }
}

impl From<String> for Erro {
    fn from(m: String) -> Self {
        Self::pedido(m)
    }
}

impl From<&str> for Erro {
    fn from(m: &str) -> Self {
        Self::pedido(m)
    }
}

/// Resultado das operações.
pub type R<T> = Result<T, Erro>;

/// Como o servidor foi aberto.
#[derive(Clone, Debug)]
pub struct Opcoes {
    /// Ambiente de teste: pagamento de teste confirma na hora. A interface
    /// mostra uma faixa "ambiente de teste" em todas as telas.
    pub modo_teste: bool,
    /// O site é servido por HTTPS (cookie `Secure`, HSTS).
    pub https: bool,
    /// Endereço público (para as instruções do agente).
    pub endereco_publico: String,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self { modo_teste: false, https: false, endereco_publico: "http://127.0.0.1:8790".into() }
    }
}

/// Diário e estado, sob a mesma trava.
pub struct Nucleo {
    /// O diário.
    pub diario: Diario,
    /// O estado.
    pub estado: Estado,
}

/// Uma amostra de telemetria (seção 31): só números da máquina.
#[derive(Clone, Copy, Debug, Default)]
pub struct Amostra {
    /// Quando.
    pub t: u64,
    /// CPU da máquina, em %.
    pub cpu: Option<f32>,
    /// Memória usada, em MiB.
    pub ram_usada_mib: Option<u64>,
    /// GPU, em %.
    pub gpu: Option<f32>,
    /// Memória da GPU usada, em MiB.
    pub vram_mib: Option<u64>,
    /// Temperatura da CPU, em °C (quando o sistema informa).
    pub temperatura: Option<f32>,
    /// Potência estimada, em W.
    pub potencia_w: Option<f32>,
    /// Bytes de rede do agente desde o último pulso.
    pub rede_bytes: u64,
    /// Unidades rodando.
    pub ativas: u32,
}

/// O último pulso de um nó.
#[derive(Clone, Debug, Default)]
pub struct Pulso {
    /// Quando chegou.
    pub ultimo_ms: u64,
    /// Linhas de CPU livres para a plataforma.
    pub livres: u32,
    /// Memória livre para a plataforma, em MiB.
    pub ram_livre_mib: u64,
    /// Motores que o agente executa (bit = código do motor).
    pub tipos: u16,
    /// GPU disponível agora.
    pub gpu: bool,
    /// O agente está pausado pelo dono?
    pub pausado_local: bool,
    /// Progresso de cada atribuição em voo: (atribuição, feitas, total).
    pub progresso: Vec<(Id, u64, u64)>,
    /// Telemetria recente.
    pub telemetria: VecDeque<Amostra>,
    /// Pulsos seguidos que chegaram (para a estabilidade).
    pub seguidos: u64,
}

/// Um balde de ritmo (token bucket).
#[derive(Clone, Copy, Debug)]
struct Balde {
    fichas_mil: u64,
    ultimo_ms: u64,
}

/// Contadores do servidor para o painel de saúde.
#[derive(Clone, Debug, Default)]
pub struct Saude {
    /// Pedidos atendidos.
    pub pedidos: u64,
    /// Respostas 5xx.
    pub erros_5xx: u64,
    /// Respostas 4xx.
    pub erros_4xx: u64,
    /// Latências recentes, em ms.
    pub latencias: VecDeque<u32>,
    /// Última cópia do diário.
    pub ultima_copia_ms: u64,
    /// Erro da última cópia.
    pub erro_copia: String,
    /// Eventos que não puderam ser aplicados (deveria ser zero).
    pub falhas_de_aplicacao: u64,
}

/// O que não vai para o diário.
#[derive(Default)]
pub struct Vivo {
    /// Último pulso de cada nó.
    pub pulsos: HashMap<Id, Pulso>,
    /// Última atividade de cada sessão (hash do token).
    pub atividade: HashMap<String, u64>,
    /// Falhas de login por chave (e-mail ou IP): (falhas, bloqueado até, janela desde).
    pub falhas_login: HashMap<String, (u32, u64, u64)>,
    baldes: HashMap<String, Balde>,
    /// Assinaturas de agente já vistas (contra repetição), com a hora.
    pub assinaturas: HashMap<[u8; 32], u64>,
    /// Ordem das assinaturas, para expirar.
    pub assinaturas_ordem: VecDeque<([u8; 32], u64)>,
    /// Cadastros por IP (horários).
    pub cadastros: HashMap<String, Vec<u64>>,
    /// Tarefas criadas por cliente (horários).
    pub criacoes: HashMap<Id, Vec<u64>>,
    /// Última vez que cada cliente foi atendido pelo agendador.
    pub atendido: HashMap<Id, u64>,
    /// Atribuições que o agente deve parar, por nó.
    pub cancelar: HashMap<Id, Vec<Id>>,
    /// Unidade que esperou por um nó mais adequado, desde quando.
    pub adiado: HashMap<(Id, u64), u64>,
    /// Última varredura de fraude.
    pub ultima_varredura: u64,
    /// Saúde do servidor.
    pub saude: Saude,
}

/// A plataforma.
pub struct Plataforma {
    pasta: PathBuf,
    nucleo: Mutex<Nucleo>,
    vivo: Mutex<Vivo>,
    /// Como foi aberta.
    pub opcoes: Opcoes,
    relogio: Arc<dyn Fn() -> u64 + Send + Sync>,
    /// Quando o processo abriu.
    pub inicio_ms: u64,
    encerrar: AtomicBool,
    degradada: AtomicBool,
}

/// O resultado de abrir.
pub struct Abertura {
    /// A plataforma.
    pub plataforma: Arc<Plataforma>,
    /// Eventos relidos.
    pub eventos: u64,
    /// A cauda incompleta foi cortada.
    pub cauda_cortada: bool,
}

impl Plataforma {
    /// Abre a pasta: trava, relê o diário inteiro e reconstrói o estado.
    pub fn abrir(pasta: &Path, opcoes: Opcoes) -> Result<Abertura, String> {
        Self::abrir_com_relogio(pasta, opcoes, Arc::new(agora_ms))
    }

    /// Como [`Self::abrir`], com um relógio de fora (para os testes).
    pub fn abrir_com_relogio(pasta: &Path, opcoes: Opcoes, relogio: Arc<dyn Fn() -> u64 + Send + Sync>) -> Result<Abertura, String> {
        let (diario, registros, cauda_cortada) = Diario::abrir(pasta)?;
        let mut estado = Estado::novo();
        let eventos = u64::try_from(registros.len()).unwrap_or(u64::MAX);
        for r in &registros {
            estado.aplicar(&r.ev).map_err(|e| format!("o diário não reconstrói o estado no evento {}: {e}", r.n))?;
        }
        for sub in ["resultados", "entregas", "copias"] {
            std::fs::create_dir_all(pasta.join(sub)).map_err(|e| format!("não criei {sub}: {e}"))?;
        }
        let inicio_ms = relogio();
        let p = Arc::new(Self {
            pasta: pasta.to_path_buf(),
            nucleo: Mutex::new(Nucleo { diario, estado }),
            vivo: Mutex::new(Vivo::default()),
            opcoes,
            relogio,
            inicio_ms,
            encerrar: AtomicBool::new(false),
            degradada: AtomicBool::new(false),
        });
        Ok(Abertura { plataforma: p, eventos, cauda_cortada })
    }

    /// Agora, pelo relógio da plataforma.
    pub fn agora(&self) -> u64 {
        (self.relogio)()
    }

    /// A pasta de dados.
    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    /// Trava diário e estado.
    pub fn nucleo(&self) -> R<MutexGuard<'_, Nucleo>> {
        self.nucleo.lock().map_err(|_| Erro::interno("trava do núcleo envenenada"))
    }

    /// Trava o que não vai para o diário.
    pub fn vivo(&self) -> R<MutexGuard<'_, Vivo>> {
        self.vivo.lock().map_err(|_| Erro::interno("trava do vivo envenenada"))
    }

    /// Lê o estado sem mudar nada.
    pub fn ler<T>(&self, f: impl FnOnce(&Estado) -> T) -> R<T> {
        Ok(f(&self.nucleo()?.estado))
    }

    /// Grava os eventos no diário e aplica. Sem diário gravado, nada muda.
    pub fn registrar(&self, n: &mut Nucleo, eventos: Vec<Evento>) -> R<()> {
        if eventos.is_empty() {
            return Ok(());
        }
        if self.degradada.load(Ordering::Relaxed) {
            return Err(Erro::indisponivel("a plataforma está em modo de leitura até a equipe conferir o diário"));
        }
        let t = self.agora();
        n.diario.gravar(t, &eventos).map_err(|e| Erro::interno(&e))?;
        for ev in &eventos {
            if let Err(e) = n.estado.aplicar(ev) {
                // o diário já tem o evento: o estado na memória ficou atrás.
                // Para de aceitar escrita até alguém olhar (reabrir relê tudo).
                self.degradada.store(true, Ordering::Relaxed);
                if let Ok(mut v) = self.vivo.lock() {
                    v.saude.falhas_de_aplicacao = v.saude.falhas_de_aplicacao.saturating_add(1);
                }
                return Err(Erro::interno(&format!("evento {} não aplicou: {e}", ev.nome())));
            }
        }
        Ok(())
    }

    /// Está em modo de leitura por falha de aplicação?
    pub fn degradada(&self) -> bool {
        self.degradada.load(Ordering::Relaxed)
    }

    /// Um lançamento com o próximo número.
    pub fn lancamento(e: &Estado, deslocamento: u64, t: u64, motivo: Motivo, referencia: String, linhas: Vec<Linha>) -> Evento {
        Evento::Ledger(Lancamento { id: e.seq.lancamento.saturating_add(deslocamento), t, motivo, referencia, linhas: normalizar(linhas) })
    }

    /// Ritmo: `limite` pedidos por minuto na chave `chave`. `false` = passou do limite.
    pub fn ritmo(&self, chave: &str, limite_por_min: u32) -> bool {
        let agora = self.agora();
        let Ok(mut v) = self.vivo.lock() else { return true };
        let cheio = u64::from(limite_por_min).saturating_mul(1000);
        let b = v.baldes.entry(chave.to_string()).or_insert(Balde { fichas_mil: cheio, ultimo_ms: agora });
        let passou = agora.saturating_sub(b.ultimo_ms);
        // repõe limite/min, isto é, limite·1000 fichas-mil a cada 60.000 ms
        let repor = passou.saturating_mul(u64::from(limite_por_min)) / 60;
        b.fichas_mil = b.fichas_mil.saturating_add(repor).min(cheio);
        b.ultimo_ms = agora;
        if b.fichas_mil >= 1000 {
            b.fichas_mil = b.fichas_mil.saturating_sub(1000);
            true
        } else {
            false
        }
    }

    /// Anota um pedido atendido (para o painel de saúde).
    pub fn anotar_pedido(&self, status: u16, ms: u32) {
        if let Ok(mut v) = self.vivo.lock() {
            let s = &mut v.saude;
            s.pedidos = s.pedidos.saturating_add(1);
            if status >= 500 {
                s.erros_5xx = s.erros_5xx.saturating_add(1);
            } else if status >= 400 {
                s.erros_4xx = s.erros_4xx.saturating_add(1);
            }
            s.latencias.push_back(ms);
            if s.latencias.len() > 2000 {
                s.latencias.pop_front();
            }
            // limpa baldes velhos de vez em quando
            if s.pedidos.is_multiple_of(5000) {
                let agora = self.agora();
                v.baldes.retain(|_, b| agora.saturating_sub(b.ultimo_ms) < 600_000);
            }
        }
    }

    /// Pede para o laço de manutenção parar.
    pub fn encerrar(&self) {
        self.encerrar.store(true, Ordering::Relaxed);
    }

    /// Liga o laço de manutenção numa linha própria (a cada 2 s).
    pub fn iniciar_manutencao(self: &Arc<Self>) {
        let fraco = Arc::downgrade(self);
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let Some(p) = fraco.upgrade() else { return };
                if p.encerrar.load(Ordering::Relaxed) {
                    return;
                }
                if let Err(e) = p.manutencao() {
                    eprintln!("[manutenção] {}", e.mensagem);
                }
            }
        });
    }

    /// Uma passada de manutenção: prazos, presença, conferências esquecidas,
    /// tarefas que terminaram, liberação de ganhos, resultados vencidos e a
    /// cópia diária do diário. O laço chama a cada 2 s; os testes chamam à mão.
    pub fn manutencao(&self) -> R<()> {
        self.vencer_prazos()?;
        self.vigiar_presenca()?;
        self.fechar_tarefas()?;
        self.liberar_ganhos()?;
        self.apagar_resultados_vencidos()?;
        self.encerrar_sessoes_vencidas()?;
        self.varredura_de_fraude()?;
        self.copia_diaria()?;
        Ok(())
    }

    /// Caminho do resultado aceito de uma unidade.
    pub fn arquivo_do_resultado(&self, tarefa: Id, unidade: u64) -> PathBuf {
        self.pasta.join("resultados").join(tarefa.to_string()).join(format!("{unidade}.bin"))
    }

    /// Caminho de uma entrega ainda não julgada.
    pub fn arquivo_da_entrega(&self, atribuicao: Id) -> PathBuf {
        self.pasta.join("entregas").join(format!("{atribuicao}.bin"))
    }

    fn copia_diaria(&self) -> R<()> {
        let agora = self.agora();
        {
            let v = self.vivo()?;
            if agora.saturating_sub(v.saude.ultima_copia_ms) < 6 * 3_600_000 {
                return Ok(());
            }
        }
        let destino = self.pasta.join("copias").join(format!("diario-{}.hyx", crate::util::data_hora(agora).replace([' ', ':'], "-")));
        let r = self.nucleo()?.diario.copiar(&destino);
        let mut v = self.vivo()?;
        v.saude.ultima_copia_ms = agora;
        match r {
            Ok(_) => {
                v.saude.erro_copia.clear();
                drop(v);
                // guarda as 28 mais novas (sete dias, quatro por dia)
                if let Ok(lista) = std::fs::read_dir(self.pasta.join("copias")) {
                    let mut nomes: Vec<PathBuf> = lista.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "hyx")).collect();
                    nomes.sort();
                    let sobra = nomes.len().saturating_sub(28);
                    for velho in nomes.into_iter().take(sobra) {
                        let _ = std::fs::remove_file(velho);
                    }
                }
            }
            Err(e) => v.saude.erro_copia = e,
        }
        Ok(())
    }
}
