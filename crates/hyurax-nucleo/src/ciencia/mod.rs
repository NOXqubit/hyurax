// ✝ Provérbios 25:2 — “A glória de Deus é encobrir o negócio; mas a glória dos reis é esquadrinhá-lo.”
//! Computação científica no nó: o agendador de JOBs.
//!
//! Etapa C de `docs/COMPUTACAO-CIENTIFICA.md`. O motor, o JOB, as unidades,
//! os intervalos, o resumo e os créditos são do crate `hyurax-ultrax`; aqui
//! fica o que depende de disco e de relógio:
//!
//! - **fila de JOBs**: validação, estimativa de custo e tempo, pausa, retomada
//!   e cancelamento;
//! - **despacho**: cada unidade vira uma tarefa do worker que já existe
//!   (`ultrax.rs`), com a semente do JOB. Execução, limites de CPU e memória,
//!   prazo, prova assinada e conferência são os mesmos da LAB;
//! - **repetição**: unidade cancelada ou vencida volta para a fila; unidade
//!   que o worker abandonou (recusada duas vezes) é tentada de novo, e depois
//!   de [`FALHAS_POR_UNIDADE`] abandonos fica registrada como falha, sem
//!   travar o resto do JOB;
//! - **checkpoint**: o progresso vai para o disco a cada poucos segundos, com
//!   hash de integridade e a cópia anterior guardada. Ao reabrir, o JOB
//!   continua de onde parou;
//! - **eventos**: cada passo vira uma linha com os campos da especificação
//!   (JOB, unidade, nó, tipo, horário, hashes, tempo, uso e verificação);
//! - **relatório**: ver `relatorio.rs`.
//!
//! **O que o nível de verificação quer dizer aqui.** Um nó sozinho entrega os
//! níveis 1 e 2: integridade e reexecução, na própria máquina. Os níveis 3 em
//! diante pedem workers de **outros nós**; sem eles, o JOB fica esperando
//! ("aguardando nós"), e não é mostrado como conferido por quem não conferiu.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, sha512};
use hyurax_ultrax::agregador::Agregador;
use hyurax_ultrax::job::{Consumo, EspecificacaoDeJob, Intervalos, Nivel, PedidoDeJob, Resumo, unidade};
use hyurax_ultrax::tarefa::Estado;
use hyurax_ultrax::trabalho::{self, TipoDeTrabalho};

use crate::util::texto_json;
use crate::ultrax::{Agendador, DesfechoDeUnidade, PedidoDeUnidade, Ultrax};
use crate::util::{de_hex, hex};

pub mod bancada;
mod rede;
pub mod relatorio;

/// Versão do esquema da pasta `PASTA/ciencia/`.
pub const VERSAO_DA_PASTA: u32 = 1;
/// Versão do arquivo de checkpoint.
const VERSAO_CHECKPOINT: u32 = 1;
/// Unidades de um JOB na fila ou rodando ao mesmo tempo.
const EM_VOO_POR_JOB: usize = 8;
/// Abandonos do worker (cada um = duas recusas) antes de a unidade virar falha.
pub const FALHAS_POR_UNIDADE: u8 = 2;
/// Eventos guardados na memória para a tela.
const EVENTOS_NA_MEMORIA: usize = 4000;
/// Checkpoint de JOB com mudança, no máximo a cada isso.
const CHECKPOINT_A_CADA: Duration = Duration::from_secs(2);
/// O registro por unidade (`unidades.jsonl`) para de crescer aqui; o resumo
/// aditivo e o agregador continuam cobrindo todas.
const REGISTRO_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Prazo mínimo de uma unidade.
const PRAZO_MINIMO_MS: u64 = 120_000;
/// Prioridade das unidades de JOB na fila (a LAB usa 100).
const PRIORIDADE: u8 = 150;

fn agora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Onde um JOB está.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoDoJob {
    /// Distribuindo e calculando unidades (quando o ULTRAX está ligado).
    Rodando,
    /// Parado pelo dono; retoma de onde parou.
    Pausado,
    /// Pede concordância entre nós (nível 3 ou mais) e não há outros nós.
    AguardandoNos,
    /// Todas as unidades conferidas ou registradas como falha.
    Concluido,
    /// Cancelado pelo dono.
    Cancelado,
    /// O prazo do JOB venceu antes do fim.
    Vencido,
    /// O consumo chegou no orçamento.
    SemOrcamento,
}

impl EstadoDoJob {
    fn codigo(self) -> u8 {
        match self {
            Self::Rodando => 1,
            Self::Pausado => 2,
            Self::AguardandoNos => 3,
            Self::Concluido => 4,
            Self::Cancelado => 5,
            Self::Vencido => 6,
            Self::SemOrcamento => 7,
        }
    }

    fn de_codigo(c: u8) -> Option<Self> {
        [
            Self::Rodando,
            Self::Pausado,
            Self::AguardandoNos,
            Self::Concluido,
            Self::Cancelado,
            Self::Vencido,
            Self::SemOrcamento,
        ]
        .into_iter()
        .find(|e| e.codigo() == c)
    }

    /// Nome para a tela e o relatório.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Rodando => "RUNNING",
            Self::Pausado => "PAUSED",
            Self::AguardandoNos => "WAITING FOR NODES",
            Self::Concluido => "COMPLETED",
            Self::Cancelado => "CANCELLED",
            Self::Vencido => "EXPIRED",
            Self::SemOrcamento => "OUT OF BUDGET",
        }
    }

    /// Não volta a rodar.
    pub fn e_final(self) -> bool {
        matches!(self, Self::Concluido | Self::Cancelado | Self::Vencido)
    }
}

/// Uma unidade despachada e ainda sem desfecho.
#[derive(Clone, Copy, Debug)]
struct EmVoo {
    despachada_ms: u64,
    inicio_ms: u64,
    linha: Option<u32>,
    entrada: Option<[u8; HASH_LEN]>,
}

/// Um JOB na memória, com o progresso.
pub(crate) struct Job {
    pub(crate) esp: EspecificacaoDeJob,
    pub(crate) id: [u8; HASH_LEN],
    pub(crate) criado_ms: u64,
    pub(crate) estado: EstadoDoJob,
    pub(crate) motivo: String,
    pub(crate) inicio_ms: u64,
    pub(crate) fim_ms: u64,
    pub(crate) cursor: u64,
    pub(crate) feitas: Intervalos,
    pub(crate) abandonadas: Intervalos,
    pub(crate) falhas: BTreeMap<u64, u8>,
    em_voo: BTreeMap<u64, EmVoo>,
    pub(crate) resumo: Resumo,
    pub(crate) agregador: Agregador,
    pub(crate) consumo: Consumo,
    pub(crate) workers: BTreeSet<[u8; PUBKEY_LEN]>,
    pub(crate) recusas: u64,
    pub(crate) repetidas: u64,
    pub(crate) registros: u64,
    pub(crate) registro_cortado: bool,
    sujo: bool,
    ultimo_checkpoint: Instant,
}

impl Job {
    fn novo(esp: EspecificacaoDeJob, id: [u8; HASH_LEN], criado_ms: u64) -> Self {
        let estado = if esp.nivel() >= Nivel::Concordancia { EstadoDoJob::AguardandoNos } else { EstadoDoJob::Rodando };
        let agregador = Agregador::novo(esp.modelo());
        Self {
            esp,
            id,
            criado_ms,
            estado,
            motivo: if estado == EstadoDoJob::AguardandoNos {
                "nível de verificação pede workers de outros nós; este nó está sozinho".into()
            } else {
                String::new()
            },
            inicio_ms: 0,
            fim_ms: 0,
            cursor: 0,
            feitas: Intervalos::new(),
            abandonadas: Intervalos::new(),
            falhas: BTreeMap::new(),
            em_voo: BTreeMap::new(),
            resumo: Resumo::default(),
            agregador,
            consumo: Consumo::default(),
            workers: BTreeSet::new(),
            recusas: 0,
            repetidas: 0,
            registros: 0,
            registro_cortado: false,
            sujo: true,
            ultimo_checkpoint: Instant::now(),
        }
    }

    /// Unidades com desfecho definitivo: conferidas ou registradas como falha.
    pub(crate) fn resolvidas(&self) -> u64 {
        self.feitas.concluidas().saturating_add(self.abandonadas.concluidas())
    }

    fn progresso(&self) -> f64 {
        self.feitas.concluidas() as f64 / self.esp.unidades().max(1) as f64
    }

    /// A próxima unidade que falta, sem as que estão em voo nem as
    /// abandonadas. Procura do cursor para a frente e, se não achar, do
    /// começo (uma unidade que voltou para a fila fica antes do cursor).
    fn proxima_faltante(&self) -> Option<u64> {
        let total = self.esp.unidades();
        let procurar = |inicio: u64| -> Option<u64> {
            let mut i = inicio;
            loop {
                let k = self.feitas.primeira_faltante(i, total)?;
                if let Some(fim) = self.abandonadas.fim_da_faixa(k) {
                    i = fim;
                } else if self.em_voo.contains_key(&k) {
                    i = k.saturating_add(1);
                } else {
                    return Some(k);
                }
            }
        };
        procurar(self.cursor).or_else(|| if self.cursor > 0 { procurar(0) } else { None })
    }
}

/// Um evento, com os campos pedidos pela especificação. `None` vira `null`
/// no JSON: é dado que este nó não mede, e não um zero inventado.
#[derive(Clone, Debug, Default)]
pub struct Evento {
    /// Nome, em maiúsculas (`WORK_UNIT_VERIFIED`).
    pub evento: &'static str,
    /// `JOB_ID`.
    pub job: Option<[u8; HASH_LEN]>,
    /// Índice da unidade.
    pub unidade: Option<u64>,
    /// `WORKER_ID` de quem executou ou deste nó.
    pub no: Option<[u8; PUBKEY_LEN]>,
    /// Tipo de trabalho.
    pub tipo: Option<TipoDeTrabalho>,
    /// O que foi feito, em texto.
    pub operacao: String,
    /// Progresso do JOB, de 0 a 1.
    pub progresso: Option<f64>,
    /// INPUT_HASH.
    pub entrada: Option<[u8; HASH_LEN]>,
    /// RESULT_HASH.
    pub resultado: Option<[u8; HASH_LEN]>,
    /// Tempo de cálculo puro, em ms.
    pub ms_execucao: Option<u64>,
    /// Ciclo de trabalho da linha de CPU, em %: cálculo ÷ relógio.
    pub cpu: Option<f64>,
    /// Ciclo de trabalho da GPU pelo ULTRAX, em %.
    pub gpu: Option<f64>,
    /// Memória reservada, em bytes.
    pub ram: Option<u64>,
    /// Operações por segundo de cálculo.
    pub vazao: Option<f64>,
    /// PENDING, PASSED, FAILED, NOT_JUDGED.
    pub verificacao: Option<&'static str>,
}

impl Evento {
    /// Uma linha JSON, com o número de sequência e o horário.
    pub fn json(&self, seq: u64, instante_ms: u64) -> String {
        let hx = |v: &Option<[u8; HASH_LEN]>| v.map_or("null".to_string(), |h| format!("\"{}\"", hex(&h)));
        let num = |v: Option<f64>| v.filter(|x| x.is_finite()).map_or("null".to_string(), |x| format!("{x:.3}"));
        let inteiro = |v: Option<u64>| v.map_or("null".to_string(), |x| x.to_string());
        format!(
            "{{\"seq\":{seq},\"event\":\"{}\",\"job_id\":{},\"work_unit_id\":{},\"unit_index\":{},\"node_id\":{},\"workload_type\":{},\
             \"operation\":{},\"timestamp\":{instante_ms},\"progress\":{},\"input_hash\":{},\"result_hash\":{},\"execution_time_ms\":{},\
             \"cpu_usage\":{},\"gpu_usage\":{},\"vram_bytes\":null,\"ram_bytes\":{},\"throughput\":{},\"verification_status\":{}}}",
            self.evento,
            hx(&self.job),
            match (self.job, self.unidade) {
                (Some(j), Some(i)) => format!("\"{}#{i}\"", hex(j.get(..8).unwrap_or_default())),
                _ => "null".into(),
            },
            inteiro(self.unidade),
            self.no.map_or("null".to_string(), |n| format!("\"{}\"", hex(&n))),
            self.tipo.map_or("null".to_string(), |t| format!("\"{}\"", t.nome())),
            texto_json(&self.operacao),
            num(self.progresso),
            hx(&self.entrada),
            hx(&self.resultado),
            inteiro(self.ms_execucao),
            num(self.cpu),
            num(self.gpu),
            inteiro(self.ram),
            num(self.vazao),
            self.verificacao.map_or("null".to_string(), |v| format!("\"{v}\"")),
        )
    }
}

/// O agendador de JOBs deste nó.
pub struct Ciencia {
    pasta: PathBuf,
    no: [u8; PUBKEY_LEN],
    versao: &'static str,
    pub(crate) jobs: Mutex<Vec<Job>>,
    eventos: Mutex<VecDeque<String>>,
    seq: AtomicU64,
    /// Fim da thread vigia (só o benchmark, com uma ciência temporária).
    vigia_encerrada: std::sync::atomic::AtomicBool,
    /// Arquivos só-acréscimo abertos, com buffer e bytes já no disco. Esvaziados
    /// a cada segundo e ao fim de cada JOB: abrir e fechar a cada evento custava
    /// uns 7 ms por unidade no disco desta máquina.
    escrita: Mutex<BTreeMap<PathBuf, (std::io::BufWriter<std::fs::File>, u64)>>,
    ultrax: Mutex<Weak<Ultrax>>,
    /// Pasta de versão mais nova que este programa: só leitura, nada roda.
    somente_leitura: Option<String>,
    /// ULTRA BENCHMARK pedido pela tela: rodando agora, e o último resultado.
    pub(crate) benchmark: Mutex<(bool, String)>,
    /// A computação entre nós: ofertas, pedidos, redundância e reputação.
    rede: Mutex<rede::EstadoDaRede>,
    /// Este nó calcula unidades que outros nós pedem.
    aceitar_rede: AtomicBool,
    /// Quem mais recebe cada evento (o barramento do núcleo).
    saida: Mutex<Option<SaidaDeEventos>>,
}

/// Recebe cada evento da ciência, já em JSON.
pub type SaidaDeEventos = Arc<dyn Fn(&str) + Send + Sync>;

/// A situação de um JOB num instante.
#[derive(Clone, Debug)]
pub struct Situacao {
    /// `JOB_ID`.
    pub id: [u8; HASH_LEN],
    /// Estado.
    pub estado: EstadoDoJob,
    /// Por que parou (vazio se não parou).
    pub motivo: String,
    /// Unidades conferidas.
    pub feitas: u64,
    /// Unidades registradas como falha.
    pub falhas: u64,
    /// Unidades do JOB.
    pub total: u64,
    /// O motor.
    pub tipo: TipoDeTrabalho,
    /// O resumo consolidado do que já foi conferido.
    pub agregado: String,
}

impl Situacao {
    fn de(j: &Job) -> Self {
        Self {
            id: j.id,
            estado: j.estado,
            motivo: j.motivo.clone(),
            feitas: j.feitas.concluidas(),
            falhas: j.abandonadas.concluidas(),
            total: j.esp.unidades(),
            tipo: j.esp.modelo().tipo(),
            agregado: j.agregador.texto(),
        }
    }
}

/// Lê um arquivo inteiro, se existir.
fn ler(caminho: &Path) -> Option<Vec<u8>> {
    std::fs::read(caminho).ok()
}

/// Grava de forma atômica: arquivo temporário e troca de nome, guardando a
/// versão anterior em `.anterior`.
fn gravar_atomico(caminho: &Path, dados: &[u8]) -> Result<(), String> {
    let temporario = caminho.with_extension("tmp");
    let mut f = std::fs::File::create(&temporario).map_err(|e| format!("não consegui criar {}: {e}", temporario.display()))?;
    f.write_all(dados).and_then(|()| f.sync_all()).map_err(|e| format!("não consegui gravar {}: {e}", temporario.display()))?;
    drop(f);
    if caminho.exists() {
        let _ = std::fs::rename(caminho, caminho.with_extension("anterior"));
    }
    std::fs::rename(&temporario, caminho).map_err(|e| format!("não consegui trocar {}: {e}", caminho.display()))
}

impl Ciencia {
    /// Abre (ou cria) `PASTA/ciencia`, migra o esquema se preciso e carrega os
    /// JOBs com o progresso do último checkpoint.
    pub fn abrir(dados: &Path, no: [u8; PUBKEY_LEN], versao: &'static str) -> Result<Arc<Self>, String> {
        let pasta = dados.join("ciencia");
        std::fs::create_dir_all(pasta.join("jobs")).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
        let somente_leitura = migrar(&pasta)?;
        let c = Arc::new(Self {
            pasta,
            no,
            versao,
            jobs: Mutex::new(Vec::new()),
            eventos: Mutex::new(VecDeque::new()),
            seq: AtomicU64::new(0),
            vigia_encerrada: std::sync::atomic::AtomicBool::new(false),
            escrita: Mutex::new(BTreeMap::new()),
            ultrax: Mutex::new(Weak::new()),
            somente_leitura,
            benchmark: Mutex::new((false, String::new())),
            rede: Mutex::new(rede::EstadoDaRede::default()),
            aceitar_rede: AtomicBool::new(false),
            saida: Mutex::new(None),
        });
        c.carregar();
        Ok(c)
    }

    /// Liga no worker: ele passa a pedir unidades a este agendador, e o
    /// agendador passa a ler o progresso das unidades rodando.
    pub fn ligar(self: &Arc<Self>, ultrax: &Arc<Ultrax>) {
        if let Ok(mut u) = self.ultrax.lock() {
            *u = Arc::downgrade(ultrax);
        }
        let agendador: Arc<dyn Agendador> = Arc::clone(self) as Arc<dyn Agendador>;
        ultrax.ligar_agendador(agendador);
        let c = Arc::clone(self);
        std::thread::spawn(move || c.vigiar());
    }

    /// Termina a thread vigia de vez.
    pub fn encerrar_vigia(&self) {
        self.vigia_encerrada.store(true, Ordering::Relaxed);
    }

    fn ultrax(&self) -> Option<Arc<Ultrax>> {
        self.ultrax.lock().ok().and_then(|u| u.upgrade())
    }

    /// Pasta de um JOB.
    pub fn pasta_do_job(&self, id: &[u8; HASH_LEN]) -> PathBuf {
        self.pasta.join("jobs").join(hex(id.get(..16).unwrap_or_default()))
    }

    /// A pasta `ciencia` deste nó.
    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    /// Versão do programa, para o relatório.
    pub fn versao(&self) -> &'static str {
        self.versao
    }

    // -----------------------------------------------------------------------
    // Pedidos do dono
    // -----------------------------------------------------------------------

    /// Estimativa antes de submeter: operações, créditos, memória e o tempo
    /// no ritmo medido desta máquina (com uma linha de CPU inteira).
    pub fn estimar(&self, pedido: PedidoDeJob) -> Result<String, String> {
        let esp = EspecificacaoDeJob::nova(pedido).map_err(|e| e.to_string())?;
        Ok(self.json_estimativa(&esp))
    }

    /// A estimativa de um JOB (operações, memória, tempo), em JSON.
    pub fn json_estimativa(&self, esp: &EspecificacaoDeJob) -> String {
        let e = esp.estimativa();
        let medido = self.ultrax().and_then(|u| u.ritmo_medido(esp.modelo().tipo()));
        let ritmo = self.ultrax().map_or(20.0e6, |u| u.ritmo_de(esp.modelo().tipo())).max(1.0);
        let linhas = self.ultrax().map_or(1, |u| u.linhas.load(Ordering::Relaxed).max(1));
        let segundos = e.operacoes.saturating_add(e.operacoes_verificacao) as f64 / ritmo;
        format!(
            "{{\"operacoes\":{},\"operacoes_verificacao\":{},\"milicreditos\":{},\"memoria_por_unidade\":{},\
             \"segundos_uma_linha\":{segundos:.1},\"segundos_estimados\":{:.1},\"linhas\":{linhas},\"ritmo\":{ritmo:.0},\"ritmo_medido\":{},\
             \"nota\":\"teto do modelo de custo de cada motor; sem contar pausas do limite de CPU\"}}",
            e.operacoes,
            e.operacoes_verificacao,
            e.milicreditos,
            e.memoria_por_unidade,
            segundos / f64::from(linhas),
            medido.is_some(),
        )
    }

    /// Submete um JOB. Devolve o `JOB_ID`.
    pub fn submeter(&self, pedido: PedidoDeJob) -> Result<[u8; HASH_LEN], String> {
        if let Some(motivo) = &self.somente_leitura {
            return Err(motivo.clone());
        }
        let esp = EspecificacaoDeJob::nova(pedido).map_err(|e| e.to_string())?;
        let id = esp.id().map_err(|e| e.to_string())?;
        // uma unidade que não cabe no teto de memória do ULTRAX nunca rodaria
        if let Some(u) = self.ultrax.lock().ok().and_then(|w| w.upgrade()) {
            let teto = u64::from(u.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
            let pede = unidade(&esp, &id, 0).map(|x| x.especificacao.memoria_bytes()).unwrap_or(0);
            if pede > teto {
                return Err(format!(
                    "cada unidade pede {:.1} MiB e o teto de memória do ULTRAX é {:.0} MiB: aumente o teto (ULTRAX) ou diminua o tamanho",
                    pede as f64 / 1_048_576.0,
                    teto as f64 / 1_048_576.0
                ));
            }
        }
        let codificado = esp.codificar().map_err(|e| e.to_string())?;
        let mut jobs = self.jobs.lock().map_err(|_| "trava dos JOBs quebrada".to_string())?;
        if jobs.iter().any(|j| j.id == id) {
            return Err("este JOB já foi submetido (mesmo JOB_ID)".into());
        }
        let criado = agora_ms();
        let pasta = self.pasta_do_job(&id);
        std::fs::create_dir_all(&pasta).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
        gravar_atomico(&pasta.join("job.bin"), &codificado)?;
        let job = Job::novo(esp, id, criado);
        let operacao = format!(
            "{} · {} unidade(s) de {} {} · verificação {} · redundância {}",
            job.esp.dominio().nome(),
            job.esp.unidades(),
            job.esp.modelo().tipo().descricao(),
            job.esp.modelo().resumo(),
            job.esp.nivel().nome(),
            job.esp.redundancia()
        );
        self.gravar_checkpoint(&job)?;
        let aguardando = job.estado == EstadoDoJob::AguardandoNos;
        jobs.push(job);
        drop(jobs);
        self.emitir(Evento { evento: "JOB_CREATED", job: Some(id), no: Some(self.no), operacao, progresso: Some(0.0), ..Evento::default() });
        if aguardando {
            self.emitir(Evento {
                evento: "JOB_WAITING_FOR_NODES",
                job: Some(id),
                operacao: "concordância entre nós pedida; nenhum outro nó oferece workers ainda".into(),
                ..Evento::default()
            });
        }
        Ok(id)
    }

    /// Pausa, retoma ou cancela.
    pub fn mudar(&self, id: &[u8; HASH_LEN], acao: &str) -> Result<(), String> {
        let mut jobs = self.jobs.lock().map_err(|_| "trava dos JOBs quebrada".to_string())?;
        let job = jobs.iter_mut().find(|j| &j.id == id).ok_or("JOB não encontrado")?;
        if job.estado.e_final() {
            return Err(format!("o JOB já terminou ({})", job.estado.nome()));
        }
        let (novo, evento) = match acao {
            "pausar" => (EstadoDoJob::Pausado, "JOB_PAUSED"),
            "retomar" if job.esp.nivel() >= Nivel::Concordancia => (EstadoDoJob::AguardandoNos, "JOB_WAITING_FOR_NODES"),
            "retomar" => (EstadoDoJob::Rodando, "JOB_RESUMED"),
            "cancelar" => (EstadoDoJob::Cancelado, "JOB_CANCELLED"),
            _ => return Err("ação desconhecida".into()),
        };
        job.estado = novo;
        job.motivo = match acao {
            "pausar" => "pausado pelo dono".into(),
            "cancelar" => "cancelado pelo dono".into(),
            _ => String::new(),
        };
        if novo == EstadoDoJob::SemOrcamento || novo.e_final() {
            job.fim_ms = agora_ms();
        }
        job.sujo = true;
        let progresso = job.progresso();
        let _ = self.gravar_checkpoint(job);
        drop(jobs);
        self.emitir(Evento { evento, job: Some(*id), no: Some(self.no), progresso: Some(progresso), ..Evento::default() });
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Persistência
    // -----------------------------------------------------------------------

    fn carregar(&self) {
        let Ok(entradas) = std::fs::read_dir(self.pasta.join("jobs")) else { return };
        let mut carregados = Vec::new();
        for entrada in entradas.flatten() {
            let pasta = entrada.path();
            let Some(codificado) = ler(&pasta.join("job.bin")) else { continue };
            let Ok(esp) = EspecificacaoDeJob::decodificar(&codificado) else {
                self.emitir(Evento {
                    evento: "JOB_UNREADABLE",
                    operacao: format!("{}: job.bin ilegível; ignorado", pasta.display()),
                    ..Evento::default()
                });
                continue;
            };
            let Ok(id) = esp.id() else { continue };
            let mut job = Job::novo(esp, id, 0);
            let lido = ["checkpoint.bin", "checkpoint.anterior"]
                .iter()
                .find_map(|nome| ler(&pasta.join(nome)).and_then(|b| ler_checkpoint(&b, &mut job).ok().map(|()| *nome)));
            match lido {
                Some("checkpoint.bin") => {}
                Some(_) => self.emitir(Evento {
                    evento: "CHECKPOINT_RECOVERED",
                    job: Some(id),
                    operacao: "o checkpoint mais novo estava corrompido; voltou o anterior".into(),
                    ..Evento::default()
                }),
                None => {
                    job.motivo = "checkpoint ilegível: o progresso recomeçou do zero".into();
                    self.emitir(Evento {
                        evento: "CHECKPOINT_LOST",
                        job: Some(id),
                        operacao: job.motivo.clone(),
                        ..Evento::default()
                    });
                }
            }
            // o que estava em voo quando o programa fechou volta para a fila
            job.cursor = 0;
            job.sujo = false;
            carregados.push(job);
        }
        carregados.sort_by_key(|j| j.criado_ms);
        if let Ok(mut jobs) = self.jobs.lock() {
            *jobs = carregados;
        }
    }

    fn gravar_checkpoint(&self, job: &Job) -> Result<(), String> {
        let bytes = escrever_checkpoint(job).map_err(|e| e.to_string())?;
        gravar_atomico(&self.pasta_do_job(&job.id).join("checkpoint.bin"), &bytes)
    }

    fn acrescentar(&self, caminho: &Path, linha: &str, maximo: Option<u64>) -> bool {
        let Ok(mut abertos) = self.escrita.lock() else { return false };
        if !abertos.contains_key(caminho) {
            let tamanho = std::fs::metadata(caminho).map_or(0, |d| d.len());
            let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(caminho) else { return false };
            abertos.insert(caminho.to_path_buf(), (std::io::BufWriter::new(f), tamanho));
        }
        let Some((f, bytes)) = abertos.get_mut(caminho) else { return false };
        if maximo.is_some_and(|m| *bytes >= m) {
            return false;
        }
        let ok = f.write_all(linha.as_bytes()).and_then(|()| f.write_all(b"\n")).is_ok();
        *bytes = bytes.saturating_add(u64::try_from(linha.len()).unwrap_or(u64::MAX)).saturating_add(1);
        ok
    }

    /// Ao fechar o programa: grava o checkpoint de todo JOB que mudou e
    /// esvazia os registros. Sem isto, o que aconteceu nos últimos 2 s se
    /// perdia, e a unidade consolidada era refeita na próxima abertura.
    pub fn encerrar(&self) {
        if let Ok(mut jobs) = self.jobs.lock() {
            for job in jobs.iter_mut().filter(|j| j.sujo) {
                if self.gravar_checkpoint(job).is_ok() {
                    job.sujo = false;
                }
            }
        }
        self.esvaziar(true);
    }

    /// Esvazia os buffers para o disco. `fechar` também fecha os arquivos
    /// (ao fim de um JOB, para o relatório ler tudo).
    pub(crate) fn esvaziar(&self, fechar: bool) {
        if let Ok(mut abertos) = self.escrita.lock() {
            for (f, _) in abertos.values_mut() {
                let _ = f.flush();
            }
            if fechar {
                abertos.clear();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Eventos
    // -----------------------------------------------------------------------

    /// Registra um evento: na memória, para a tela, e no `eventos.jsonl` do
    /// JOB, para a reprodução.
    pub fn emitir(&self, e: Evento) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        let linha = e.json(seq, agora_ms());
        self.repassar(&linha);
        if let Some(job) = e.job {
            // os passos de cada unidade param no teto; os do JOB sempre entram.
            // Sem a pasta do JOB (apagada por fora), abrir falha e nada é gravado.
            let teto = e.unidade.map(|_| REGISTRO_MAX_BYTES);
            self.acrescentar(&self.pasta_do_job(&job).join("eventos.jsonl"), &linha, teto);
        }
        if let Ok(mut ev) = self.eventos.lock() {
            ev.push_back(linha);
            while ev.len() > EVENTOS_NA_MEMORIA {
                ev.pop_front();
            }
        }
    }

    /// Eventos com número de sequência maior que `desde` (os da memória).
    pub fn eventos_desde(&self, desde: u64) -> String {
        let mut j = format!("{{\"ultimo\":{},\"eventos\":[", self.seq.load(Ordering::Relaxed));
        if let Ok(ev) = self.eventos.lock() {
            let mut primeiro = true;
            for linha in ev.iter() {
                let seq = linha
                    .strip_prefix("{\"seq\":")
                    .and_then(|r| r.split(',').next())
                    .and_then(|n| n.parse::<u64>().ok())
                    .unwrap_or(0);
                if seq > desde {
                    if !primeiro {
                        j.push(',');
                    }
                    primeiro = false;
                    j.push_str(linha);
                }
            }
        }
        j.push_str("]}");
        j
    }

    /// Todos os eventos gravados de um JOB e o registro por unidade, para a
    /// reprodução: exatamente os arquivos, sem nada montado.
    pub fn historico_do_job(&self, id: &[u8; HASH_LEN]) -> Option<String> {
        let pasta = self.pasta_do_job(id);
        let eventos = std::fs::read_to_string(pasta.join("eventos.jsonl")).ok()?;
        let unidades = std::fs::read_to_string(pasta.join("unidades.jsonl")).unwrap_or_default();
        let lista = |t: &str| t.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join(",");
        Some(format!("{{\"eventos\":[{}],\"unidades\":[{}]}}", lista(&eventos), lista(&unidades)))
    }

    // -----------------------------------------------------------------------
    // Vigia: checkpoint, prazo e progresso das unidades rodando
    // -----------------------------------------------------------------------

    fn vigiar(self: Arc<Self>) {
        let mut ultimo_progresso: BTreeMap<([u8; HASH_LEN], u64), u64> = BTreeMap::new();
        loop {
            std::thread::sleep(Duration::from_millis(1000));
            if self.vigia_encerrada.load(Ordering::Relaxed) {
                return;
            }
            let agora = agora_ms();
            self.manutencao(agora);
            self.progresso_das_ativas(agora, &mut ultimo_progresso);
        }
    }

    /// Prazo dos JOBs e checkpoint dos que mudaram.
    pub(crate) fn manutencao(&self, agora: u64) {
        self.esvaziar(false);
        self.manutencao_rede(agora);
        {
            let mut vencidos = Vec::new();
            if let Ok(mut jobs) = self.jobs.lock() {
                for job in jobs.iter_mut() {
                    let prazo = job.esp.prazo_s();
                    if prazo > 0
                        && !job.estado.e_final()
                        && agora > job.criado_ms.saturating_add(prazo.saturating_mul(1000))
                    {
                        job.estado = EstadoDoJob::Vencido;
                        job.motivo = format!("o prazo de {prazo} s venceu com {} de {} unidades", job.feitas.concluidas(), job.esp.unidades());
                        job.fim_ms = agora;
                        job.sujo = true;
                        vencidos.push((job.id, job.motivo.clone()));
                    }
                    if job.sujo && job.ultimo_checkpoint.elapsed() >= CHECKPOINT_A_CADA {
                        if self.gravar_checkpoint(job).is_ok() {
                            job.sujo = false;
                        }
                        job.ultimo_checkpoint = Instant::now();
                    }
                }
            }
            for (id, motivo) in vencidos {
                self.emitir(Evento { evento: "JOB_EXPIRED", job: Some(id), operacao: motivo, ..Evento::default() });
                let _ = crate::ciencia::relatorio::gravar(self, &id);
            }
        }
    }

    /// Progresso real das unidades rodando: um evento por unidade quando muda.
    fn progresso_das_ativas(&self, agora: u64, ultimo_progresso: &mut BTreeMap<([u8; HASH_LEN], u64), u64>) {
        {
            if let Some(u) = self.ultrax() {
                // sem `cpu` no evento: o limite do ULTRAX é ajuste, não medida;
                // o uso real de CPU vem das métricas do sistema
                let ativas = u.unidades_ativas();
                ultimo_progresso.retain(|chave, _| ativas.iter().any(|a| (a.job, a.indice) == *chave));
                for a in ativas {
                    let crate::ultrax::UnidadeAtiva { job, indice, linha, feitas, total, estado, inicio_ms: inicio } = a;
                    if ultimo_progresso.get(&(job, indice)) == Some(&feitas) {
                        continue;
                    }
                    ultimo_progresso.insert((job, indice), feitas);
                    let tipo = self.jobs.lock().ok().and_then(|j| j.iter().find(|x| x.id == job).map(|x| x.esp.modelo().tipo()));
                    let segundos = agora.saturating_sub(inicio) as f64 / 1000.0;
                    self.emitir_na_memoria(Evento {
                        evento: "WORK_UNIT_PROGRESS",
                        job: Some(job),
                        unidade: Some(indice),
                        no: Some(self.no),
                        tipo,
                        operacao: format!("{} · linha {linha} · {feitas} de {total} operações", estado.nome()),
                        progresso: Some(feitas as f64 / total.max(1) as f64),
                        vazao: (segundos > 0.0).then(|| feitas as f64 / segundos),
                        verificacao: Some(if estado == Estado::Verificando { "VERIFYING" } else { "PENDING" }),
                        ..Evento::default()
                    });
                }
            }
        }
    }

    /// Evento só para a tela (progresso fino): não vai para o arquivo, que
    /// guarda os passos que a reprodução usa.
    fn emitir_na_memoria(&self, e: Evento) {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        let linha = e.json(seq, agora_ms());
        self.repassar(&linha);
        if let Ok(mut ev) = self.eventos.lock() {
            ev.push_back(linha);
            while ev.len() > EVENTOS_NA_MEMORIA {
                ev.pop_front();
            }
        }
    }

    /// A situação de um JOB, para quem acompanha pelo terminal.
    pub fn situacao(&self, id: &[u8; HASH_LEN]) -> Option<Situacao> {
        self.jobs.lock().ok()?.iter().find(|j| &j.id == id).map(Situacao::de)
    }

    /// A situação de todos os JOBs da pasta, na ordem em que foram criados.
    pub fn situacoes(&self) -> Vec<Situacao> {
        self.jobs.lock().map(|j| j.iter().map(Situacao::de).collect()).unwrap_or_default()
    }

    /// A especificação de um JOB.
    pub fn especificacao(&self, id: &[u8; HASH_LEN]) -> Option<EspecificacaoDeJob> {
        self.jobs.lock().ok()?.iter().find(|j| &j.id == id).map(|j| j.esp.clone())
    }

    /// Manda cada evento também para `saida` (o barramento do núcleo).
    pub fn ao_emitir(&self, saida: SaidaDeEventos) {
        if let Ok(mut s) = self.saida.lock() {
            *s = Some(saida);
        }
    }

    fn repassar(&self, linha: &str) {
        let saida = self.saida.lock().ok().and_then(|s| s.clone());
        if let Some(f) = saida {
            f(linha);
        }
    }

    // -----------------------------------------------------------------------
    // Para a tela
    // -----------------------------------------------------------------------

    /// Lista dos JOBs, com o progresso.
    pub fn json(&self) -> String {
        let mut j = String::from("{\"jobs\":[");
        if let Ok(jobs) = self.jobs.lock() {
            for (k, job) in jobs.iter().enumerate().rev() {
                if k.saturating_add(1) < jobs.len() {
                    j.push(',');
                }
                j.push_str(&json_do_job(job, false));
            }
        }
        let _ = write!(
            j,
            "],\"versao_da_pasta\":{VERSAO_DA_PASTA},\"somente_leitura\":{},\"no\":\"{}\",\"rede\":{},\"dominios\":[",
            self.somente_leitura.as_deref().map_or("null".to_string(), texto_json),
            hex(&self.no),
            self.json_rede()
        );
        for (k, d) in hyurax_ultrax::job::Dominio::TODOS.iter().enumerate() {
            if k > 0 {
                j.push(',');
            }
            let motores: Vec<String> = d.motores().iter().map(|t| format!("{{\"codigo\":{},\"nome\":\"{}\",\"descricao\":{}}}", t.codigo(), t.nome(), texto_json(t.descricao()))).collect();
            let _ = write!(j, "{{\"codigo\":{},\"nome\":\"{}\",\"motores\":[{}]}}", d.codigo(), d.nome(), motores.join(","));
        }
        j.push_str("]}");
        j
    }

    /// Um JOB inteiro, com o agregador e a estimativa.
    pub fn json_job(&self, id: &[u8; HASH_LEN]) -> Option<String> {
        let jobs = self.jobs.lock().ok()?;
        let job = jobs.iter().find(|j| &j.id == id)?;
        let mut j = json_do_job(job, true);
        j.pop();
        let _ = write!(j, ",\"estimativa\":{}}}", self.json_estimativa(&job.esp));
        Some(j)
    }

    /// O `JOB_ID` completo a partir do começo em hexadecimal (16 ou mais
    /// dígitos), se for único.
    pub fn achar(&self, prefixo: &str) -> Option<[u8; HASH_LEN]> {
        let prefixo = prefixo.to_ascii_lowercase();
        if prefixo.len() < 16 {
            return None;
        }
        let jobs = self.jobs.lock().ok()?;
        let mut achados = jobs.iter().filter(|j| hex(&j.id).starts_with(&prefixo));
        let primeiro = achados.next()?.id;
        achados.next().is_none().then_some(primeiro)
    }
}

fn json_do_job(job: &Job, completo: bool) -> String {
    let esp = &job.esp;
    let modelo = esp.modelo();
    let parametros: Vec<String> = modelo.parametros().iter().map(u32::to_string).collect();
    let c = &job.consumo;
    let mut j = format!(
        "{{\"id\":\"{}\",\"dominio\":\"{}\",\"tipo\":\"{}\",\"tipo_codigo\":{},\"descricao_tipo\":{},\"categoria\":\"{}\",\"resumo\":{},\
         \"tamanho\":{},\"passos\":{},\"parametros\":[{}],\"unidades\":{},\"nivel\":{},\"nivel_nome\":{},\"redundancia\":{},\
         \"prazo_s\":{},\"orcamento_milicreditos\":{},\"descricao\":{},\"criado\":{},\"inicio\":{},\"fim\":{},\"estado\":\"{}\",\
         \"motivo\":{},\"feitas\":{},\"falhas\":{},\"em_voo\":{},\"recusas\":{},\"repetidas\":{},\"progresso\":{:.6},\
         \"agregado\":{},\"resumo_hash\":\"{}\",\"consumo\":{{\"unidades\":{},\"operacoes\":{},\"operacoes_verificacao\":{},\
         \"cpu_ms\":{},\"gpu_ms\":{},\"memoria_mib_s\":{},\"bytes_rede\":{},\"milicreditos\":{}}},\"nos\":{}",
        hex(&job.id),
        esp.dominio().nome(),
        modelo.tipo().nome(),
        modelo.tipo().codigo(),
        texto_json(modelo.tipo().descricao()),
        modelo.tipo().categoria().nome(),
        texto_json(&modelo.resumo()),
        modelo.tamanho(),
        modelo.passos(),
        parametros.join(","),
        esp.unidades(),
        esp.nivel().codigo(),
        texto_json(esp.nivel().nome()),
        esp.redundancia(),
        esp.prazo_s(),
        esp.orcamento_milicreditos(),
        texto_json(esp.descricao()),
        job.criado_ms,
        job.inicio_ms,
        job.fim_ms,
        job.estado.nome(),
        texto_json(&job.motivo),
        job.feitas.concluidas(),
        job.abandonadas.concluidas(),
        job.em_voo.len(),
        job.recusas,
        job.repetidas,
        job.progresso(),
        texto_json(&job.agregador.texto()),
        hex(&job.resumo.bytes()),
        c.unidades,
        c.operacoes,
        c.operacoes_verificacao,
        c.cpu_ms,
        c.gpu_ms,
        c.memoria_mib_s,
        c.bytes_rede,
        c.milicreditos(),
        job.workers.len(),
    );
    if completo {
        let em_voo: Vec<String> = job
            .em_voo
            .iter()
            .map(|(i, v)| {
                format!(
                    "{{\"indice\":{i},\"despachada\":{},\"inicio\":{},\"linha\":{},\"entrada\":{}}}",
                    v.despachada_ms,
                    v.inicio_ms,
                    v.linha.map_or("null".to_string(), |l| l.to_string()),
                    v.entrada.map_or("null".to_string(), |e| format!("\"{}\"", hex(&e)))
                )
            })
            .collect();
        let faixas: Vec<String> = job.feitas.faixas().iter().take(200).map(|(a, b)| format!("[{a},{b}]")).collect();
        let workers: Vec<String> = job.workers.iter().map(|w| format!("\"{}\"", hex(w))).collect();
        let falhas: Vec<String> = job.abandonadas.faixas().iter().take(200).map(|(a, b)| format!("[{a},{b}]")).collect();
        let _ = write!(
            j,
            ",\"em_voo_lista\":[{}],\"faixas_feitas\":[{}],\"faixas_total\":{},\"faixas_falhas\":[{}],\"workers\":[{}],\"registro_cortado\":{},\"dados\":{}",
            em_voo.join(","),
            faixas.join(","),
            job.feitas.faixas().len(),
            falhas.join(","),
            workers.join(","),
            job.registro_cortado,
            job.agregador.json()
        );
    }
    j.push('}');
    j
}

impl Ciencia {
    /// Se todas as unidades têm desfecho, fecha o JOB (checkpoint e evento).
    /// Devolve `true` quando fechou agora; aí o relatório vai para o disco.
    fn fechar_se_acabou(&self, job: &mut Job, eventos: &mut Vec<Evento>) -> bool {
        // pausado ou sem orçamento também fecha: as unidades que estavam em voo
        // terminaram, e não falta nenhuma
        let aberto = matches!(job.estado, EstadoDoJob::Rodando | EstadoDoJob::Pausado | EstadoDoJob::SemOrcamento | EstadoDoJob::AguardandoNos);
        if !aberto || job.resolvidas() < job.esp.unidades() {
            return false;
        }
        job.estado = EstadoDoJob::Concluido;
        job.fim_ms = agora_ms();
        job.motivo = if job.abandonadas.concluidas() > 0 {
            format!("{} unidade(s) registradas como falha", job.abandonadas.concluidas())
        } else {
            String::new()
        };
        let _ = self.gravar_checkpoint(job);
        job.sujo = false;
        eventos.push(Evento {
            evento: "JOB_COMPLETED",
            job: Some(job.id),
            no: Some(self.no),
            tipo: Some(job.esp.modelo().tipo()),
            operacao: job.agregador.texto(),
            progresso: Some(job.progresso()),
            resultado: Some(job.resumo.bytes()),
            verificacao: Some(if job.abandonadas.concluidas() > 0 { "FAILED" } else { "PASSED" }),
            ..Evento::default()
        });
        true
    }

    /// Soma ao JOB uma unidade conferida: agregador, resumo, intervalos,
    /// consumo, registro por unidade e o evento. `verificacao` diz como foi
    /// conferida (PASSED numa máquina; CONSENSUS k/n entre nós), e
    /// `outros_workers` são os que concordaram além de quem entregou.
    fn consolidar(
        &self,
        job: &mut Job,
        d: &DesfechoDeUnidade,
        voo: Option<EmVoo>,
        verificacao: &'static str,
        outros_workers: &[[u8; PUBKEY_LEN]],
        eventos: &mut Vec<Evento>,
    ) {
        let base = Evento {
            job: Some(d.job),
            unidade: Some(d.indice),
            no: Some(d.registro.as_ref().map_or(self.no, |r| r.worker)),
            tipo: Some(job.esp.modelo().tipo()),
            entrada: d.registro.as_ref().map(|r| r.entrada),
            resultado: d.registro.as_ref().map(|r| r.resultado),
            ms_execucao: Some(d.ms_calculo),
            ram: Some(d.memoria),
            ..Evento::default()
        };
            if job.feitas.contem(d.indice) {
                eventos.push(Evento {
                    evento: "WORK_UNIT_DUPLICATE",
                    operacao: "unidade já conferida; o segundo resultado não entra".into(),
                    ..base
                });
            } else {
                let resultado = d.resultado.as_deref().unwrap_or_default();
                // sem registro de prova não há o que somar; com ele, o agregador decide
                let somado = match d.registro.as_ref() {
                    Some(r) => job.agregador.somar(d.indice, resultado).map(|()| r),
                    None => Err(hyurax_ultrax::job::ErroDeJob::Faixa("unidade liquidada sem registro de prova")),
                };
                match somado {
                    Ok(r) => {
                        job.resumo.somar(d.indice, &r.resultado);
                        job.feitas.inserir_um(d.indice);
                        job.workers.insert(r.worker);
                    for w in outros_workers {
                        job.workers.insert(*w);
                    }
                        let ms_total = d.ms_calculo.saturating_add(d.ms_verificacao);
                        job.consumo.somar(&Consumo {
                            unidades: 1,
                            operacoes: r.operacoes,
                            operacoes_verificacao: d.operacoes_verificacao,
                            cpu_ms: if d.gpu.is_some() { d.ms_verificacao } else { ms_total },
                            gpu_ms: if d.gpu.is_some() { d.ms_calculo } else { 0 },
                            memoria_mib_s: d.memoria.saturating_mul(ms_total) / (1024 * 1024 * 1000),
                            bytes_rede: 0,
                        });
                        let relogio = r.fim_ms.saturating_sub(r.inicio_ms).max(1);
                        let vazao = r.operacoes as f64 / (d.ms_calculo.max(1) as f64 / 1000.0);
                        let registro_linha = format!(
                            "{{\"indice\":{},\"tarefa\":\"{}\",\"worker\":\"{}\",\"linha\":{},\"despachada\":{},\"inicio\":{},\"fim\":{},\
                             \"verificada\":{},\"entrada\":\"{}\",\"resultado\":\"{}\",\"operacoes\":{},\"operacoes_verificacao\":{},\
                             \"ms_calculo\":{},\"ms_verificacao\":{},\"memoria\":{},\"gpu\":{},\"verificacao\":\"{verificacao}\",\"metodo\":{},\"programa\":\"{}\"}}",
                            d.indice,
                            hex(&d.tarefa),
                            hex(&r.worker),
                            voo.and_then(|v| v.linha).map_or("null".to_string(), |l| l.to_string()),
                            voo.map_or(0, |v| v.despachada_ms),
                            r.inicio_ms,
                            r.fim_ms,
                            agora_ms(),
                            hex(&r.entrada),
                            hex(&r.resultado),
                            r.operacoes,
                            d.operacoes_verificacao,
                            d.ms_calculo,
                            d.ms_verificacao,
                            d.memoria,
                            d.gpu.as_deref().map_or("null".to_string(), texto_json),
                            texto_json(r.metodo.nome()),
                            self.versao,
                        );
                        if !job.registro_cortado {
                            let caminho = self.pasta_do_job(&job.id).join("unidades.jsonl");
                            if self.acrescentar(&caminho, &registro_linha, Some(REGISTRO_MAX_BYTES)) {
                                job.registros = job.registros.saturating_add(1);
                            } else {
                                job.registro_cortado = true;
                            }
                        }
                        eventos.push(Evento {
                            evento: "WORK_UNIT_VERIFIED",
                            operacao: format!("{} · {} operações conferidas", r.metodo.nome(), r.operacoes),
                            progresso: Some(job.progresso()),
                            cpu: Some(d.ms_calculo as f64 * 100.0 / relogio as f64).map(|c| c.min(100.0)),
                            vazao: Some(vazao),
                            verificacao: Some("PASSED"),
                            ..base
                        });
                    }
                    Err(e) => {
                        // o worker conferiu, mas o consolidador não entende o resultado: é defeito, não sucesso
                        *job.falhas.entry(d.indice).or_insert(0) = FALHAS_POR_UNIDADE;
                        job.abandonadas.inserir_um(d.indice);
                        eventos.push(Evento {
                            evento: "WORK_UNIT_FAILED",
                            operacao: format!("resultado conferido que o consolidador recusou: {e}"),
                            verificacao: Some("FAILED"),
                            ..base
                        });
                    }
                }
            }
    }
}

// ---------------------------------------------------------------------------
// O worker pede e devolve unidades
// ---------------------------------------------------------------------------

impl Ciencia {
    /// A próxima unidade para o worker. Com `tipo`, só desse tipo (a GPU pede
    /// as de matriz); sem, primeiro as que outros nós pediram.
    fn proxima_filtrada(&self, agora: u64, tipo: Option<TipoDeTrabalho>) -> Option<PedidoDeUnidade> {
        if self.somente_leitura.is_some() {
            return None;
        }
        // primeiro o que outros nós pediram e este aceitou: tem prazo (só na CPU)
        if tipo.is_none()
            && let Some(remota) = self.proxima_remota(agora)
        {
            return Some(remota);
        }
        let ritmo = |t| self.ultrax().map_or(20.0e6, |u| u.ritmo_de(t)).max(1.0);
        let mut despachada = None;
        let mut sem_orcamento = Vec::new();
        let mut mudou_para: Vec<([u8; HASH_LEN], &'static str, String)> = Vec::new();
        let mut envios = Vec::new();
        {
            let mut jobs = self.jobs.lock().ok()?;
            // JOB que pede outros nós volta a rodar quando há workers de fora bastante (e para, quando some)
            for job in jobs.iter_mut().filter(|j| rede::pede_outros_nos(j.esp.nivel())) {
                let precisa = usize::from(job.esp.redundancia().saturating_sub(1));
                let de_fora = self.workers_de_fora(job.esp.modelo(), agora);
                if job.estado == EstadoDoJob::AguardandoNos && de_fora >= precisa {
                    job.estado = EstadoDoJob::Rodando;
                    job.motivo = String::new();
                    job.sujo = true;
                    mudou_para.push((job.id, "JOB_RESUMED", format!("{de_fora} worker(s) de outros nós oferecem trabalho")));
                } else if job.estado == EstadoDoJob::Rodando && de_fora < precisa && job.em_voo.is_empty() {
                    job.estado = EstadoDoJob::AguardandoNos;
                    job.motivo = format!("precisa de {precisa} worker(s) de outros nós; há {de_fora}");
                    job.sujo = true;
                    mudou_para.push((job.id, "JOB_WAITING_FOR_NODES", job.motivo.clone()));
                }
            }
            for job in jobs.iter_mut().filter(|j| j.estado == EstadoDoJob::Rodando) {
                if tipo.is_some_and(|t| job.esp.modelo().tipo() != t) {
                    continue;
                }
                let orcamento = job.esp.orcamento_milicreditos();
                if orcamento > 0 && job.consumo.milicreditos() >= orcamento {
                    job.estado = EstadoDoJob::SemOrcamento;
                    job.motivo = format!("consumo de {} milicréditos chegou no orçamento de {orcamento}", job.consumo.milicreditos());
                    job.sujo = true;
                    sem_orcamento.push((job.id, job.motivo.clone()));
                    continue;
                }
                if job.em_voo.len() >= EM_VOO_POR_JOB {
                    continue;
                }
                let Some(i) = job.proxima_faltante() else { continue };
                let Ok(u) = unidade(&job.esp, &job.id, i) else { continue };
                let segundos = u.especificacao.operacoes_maximas().saturating_add(trabalho::operacoes_de_verificacao(&u.especificacao))
                    as f64
                    / ritmo(u.especificacao.tipo());
                // prazo folgado: o limite de CPU pode ir a 10%, e a fila pode esperar
                let prazo_ms = agora.saturating_add(PRAZO_MINIMO_MS.max((segundos * 30.0 * 1000.0) as u64));
                if rede::pede_outros_nos(job.esp.nivel()) {
                    // a mesma unidade vai para workers de outros nós; a cópia daqui também conta
                    match self.despachar_redundante(job.id, i, u.especificacao, u.semente, job.esp.redundancia(), prazo_ms, agora) {
                        Some(e) => envios.extend(e),
                        None => continue,
                    }
                }
                job.em_voo.insert(i, EmVoo { despachada_ms: agora, inicio_ms: 0, linha: None, entrada: None });
                job.cursor = i.saturating_add(1);
                if job.inicio_ms == 0 {
                    job.inicio_ms = agora;
                }
                job.sujo = true;
                let progresso = job.progresso();
                despachada = Some((
                    PedidoDeUnidade {
                        job: job.id,
                        indice: i,
                        especificacao: u.especificacao,
                        semente: u.semente,
                        prioridade: PRIORIDADE,
                        prazo_ms,
                    },
                    progresso,
                ));
                break;
            }
        }
        for (id, motivo) in sem_orcamento {
            self.emitir(Evento { evento: "JOB_OUT_OF_BUDGET", job: Some(id), operacao: motivo, ..Evento::default() });
        }
        for (id, evento, motivo) in mudou_para {
            self.emitir(Evento { evento, job: Some(id), no: Some(self.no), operacao: motivo, ..Evento::default() });
        }
        for (par, m) in envios {
            self.enviar(par, &m);
        }
        let (pedido, progresso) = despachada?;
        self.emitir(Evento {
            evento: "WORK_UNIT_ASSIGNED",
            job: Some(pedido.job),
            unidade: Some(pedido.indice),
            no: Some(self.no),
            tipo: Some(pedido.especificacao.tipo()),
            operacao: format!("{} {}", pedido.especificacao.tipo().descricao(), pedido.especificacao.resumo()),
            progresso: Some(progresso),
            ram: Some(pedido.especificacao.memoria_bytes()),
            verificacao: Some("PENDING"),
            ..Evento::default()
        });
        Some(pedido)
    }
}

impl Agendador for Ciencia {
    fn ainda_quer(&self, job: &[u8; HASH_LEN], _indice: u64) -> bool {
        // unidade de outro nó (JOB que não é daqui) segue o pedido dele
        self.jobs.lock().map_or(true, |jobs| !jobs.iter().any(|j| &j.id == job && j.estado.e_final()))
    }

    fn tem_trabalho(&self) -> bool {
        if self.rede.lock().is_ok_and(|r| !r.fila.is_empty()) {
            return true;
        }
        self.jobs.lock().is_ok_and(|jobs| {
            jobs.iter().any(|j| {
                j.estado == EstadoDoJob::Rodando
                    && j.resolvidas().saturating_add(u64::try_from(j.em_voo.len()).unwrap_or(u64::MAX)) < j.esp.unidades()
            })
        })
    }

    fn proxima(&self, agora: u64) -> Option<PedidoDeUnidade> {
        self.proxima_filtrada(agora, None)
    }

    fn proxima_do_tipo(&self, agora: u64, tipo: TipoDeTrabalho) -> Option<PedidoDeUnidade> {
        self.proxima_filtrada(agora, Some(tipo))
    }
    fn comecou(&self, job: &[u8; HASH_LEN], indice: u64, linha: u32, entrada: Option<[u8; HASH_LEN]>) {
        let tipo = self.jobs.lock().ok().and_then(|mut jobs| {
            let j = jobs.iter_mut().find(|j| &j.id == job)?;
            if let Some(v) = j.em_voo.get_mut(&indice) {
                v.inicio_ms = agora_ms();
                v.linha = Some(linha);
                v.entrada = entrada;
            }
            Some(j.esp.modelo().tipo())
        });
        self.emitir(Evento {
            evento: "WORK_UNIT_STARTED",
            job: Some(*job),
            unidade: Some(indice),
            no: Some(self.no),
            tipo,
            operacao: format!("linha {linha} de CPU"),
            entrada,
            verificacao: Some("PENDING"),
            ..Evento::default()
        });
    }

    fn terminou(&self, d: DesfechoDeUnidade) {
        // unidade de outro nó: a resposta vai para ele, não para um JOB daqui
        if self.terminou_remota(&d) {
            return;
        }
        // cópia local de uma unidade redundante: espera os outros workers
        if self.terminou_redundante(&d) {
            return;
        }
        let mut eventos: Vec<Evento> = Vec::new();
        let terminou_job;
        {
            let Ok(mut jobs) = self.jobs.lock() else { return };
            let Some(job) = jobs.iter_mut().find(|j| j.id == d.job) else { return };
            if job.estado.e_final() {
                // cancelado, vencido ou concluído: o que ainda chegar não muda o
                // JOB (nem consumo, nem créditos, nem o checkpoint)
                job.em_voo.remove(&d.indice);
                return;
            }
            let tipo = job.esp.modelo().tipo();
            let voo = job.em_voo.get(&d.indice).copied();
            let base = Evento {
                job: Some(d.job),
                unidade: Some(d.indice),
                no: Some(d.registro.as_ref().map_or(self.no, |r| r.worker)),
                tipo: Some(tipo),
                entrada: d.registro.as_ref().map(|r| r.entrada),
                resultado: d.registro.as_ref().map(|r| r.resultado),
                ms_execucao: Some(d.ms_calculo),
                ram: Some(d.memoria),
                ..Evento::default()
            };
            match d.estado {
                Estado::Liquidada => {
                    job.em_voo.remove(&d.indice);
                    self.consolidar(job, &d, voo, "PASSED", &[], &mut eventos);
                }
                Estado::Recusada => {
                    job.recusas = job.recusas.saturating_add(1);
                    eventos.push(Evento {
                        evento: "WORK_UNIT_REJECTED",
                        operacao: format!("{}; o worker repete uma vez", d.nota),
                        verificacao: Some("FAILED"),
                        ..base
                    });
                }
                Estado::Cancelada | Estado::Expirada => {
                    job.em_voo.remove(&d.indice);
                    job.cursor = job.cursor.min(d.indice);
                    if d.abandonada {
                        let n = job.falhas.entry(d.indice).or_insert(0);
                        *n = n.saturating_add(1);
                        if *n >= FALHAS_POR_UNIDADE {
                            job.abandonadas.inserir_um(d.indice);
                            eventos.push(Evento {
                                evento: "WORK_UNIT_FAILED",
                                operacao: format!("recusada {} vezes: fica registrada como falha", u32::from(FALHAS_POR_UNIDADE) * 2),
                                verificacao: Some("FAILED"),
                                ..base
                            });
                        } else {
                            job.repetidas = job.repetidas.saturating_add(1);
                            eventos.push(Evento {
                                evento: "WORK_UNIT_RETRY",
                                operacao: format!("{}; volta para a fila", d.nota),
                                verificacao: Some("FAILED"),
                                ..base
                            });
                        }
                    } else {
                        job.repetidas = job.repetidas.saturating_add(1);
                        eventos.push(Evento {
                            evento: "WORK_UNIT_REQUEUED",
                            operacao: format!("{} ({}); volta para a fila", d.nota, d.estado.nome()),
                            verificacao: Some("NOT_JUDGED"),
                            ..base
                        });
                    }
                }
                _ => {}
            }
            job.sujo = true;
            terminou_job = self.fechar_se_acabou(job, &mut eventos);
        }
        for e in eventos {
            self.emitir(e);
        }
        if terminou_job {
            let _ = crate::ciencia::relatorio::gravar(self, &d.job);
        }
    }
}

// ---------------------------------------------------------------------------
// Esquema e checkpoint
// ---------------------------------------------------------------------------

/// Confere a versão da pasta e migra. Devolve `Some(motivo)` quando a pasta é
/// de um programa mais novo: aí nada roda, para não estragar o que ele gravou.
fn migrar(pasta: &Path) -> Result<Option<String>, String> {
    let arquivo = pasta.join("VERSAO");
    let atual = std::fs::read_to_string(&arquivo).ok().and_then(|t| t.trim().parse::<u32>().ok());
    match atual {
        None => {
            std::fs::write(&arquivo, format!("{VERSAO_DA_PASTA}\n")).map_err(|e| format!("não consegui gravar {}: {e}", arquivo.display()))?;
            Ok(None)
        }
        Some(v) if v == VERSAO_DA_PASTA => Ok(None),
        Some(v) if v > VERSAO_DA_PASTA => Ok(Some(format!(
            "a pasta de ciência é da versão {v}, mais nova que este programa ({VERSAO_DA_PASTA}); os JOBs ficam só para leitura"
        ))),
        Some(v) => {
            // migração de versão antiga: copia antes, sempre
            let backup = pasta.join(format!("backup-v{v}"));
            copiar_pasta(&pasta.join("jobs"), &backup.join("jobs")).map_err(|e| format!("não consegui fazer o backup antes de migrar: {e}"))?;
            let _ = std::fs::copy(&arquivo, backup.join("VERSAO"));
            // (não há migração de dados entre versões ainda: a v1 é a primeira)
            std::fs::write(&arquivo, format!("{VERSAO_DA_PASTA}\n")).map_err(|e| format!("não consegui gravar {}: {e}", arquivo.display()))?;
            Ok(None)
        }
    }
}

fn copiar_pasta(de: &Path, para: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(para)?;
    for entrada in std::fs::read_dir(de)? {
        let entrada = entrada?;
        let destino = para.join(entrada.file_name());
        if entrada.file_type()?.is_dir() {
            copiar_pasta(&entrada.path(), &destino)?;
        } else {
            std::fs::copy(entrada.path(), destino)?;
        }
    }
    Ok(())
}

fn escrever_checkpoint(job: &Job) -> Result<Vec<u8>, hyurax_ultrax::job::ErroDeJob> {
    let mut w = Writer::new();
    w.u32(VERSAO_CHECKPOINT);
    w.fixed(&job.id);
    w.u8(job.estado.codigo());
    w.string(&job.motivo)?;
    w.u64(job.criado_ms);
    w.u64(job.inicio_ms);
    w.u64(job.fim_ms);
    job.feitas.codificar(&mut w)?;
    job.abandonadas.codificar(&mut w)?;
    w.u32(u32::try_from(job.falhas.len()).unwrap_or(u32::MAX));
    for (i, n) in &job.falhas {
        w.u64(*i);
        w.u8(*n);
    }
    w.fixed(&job.resumo.bytes());
    w.u64(job.resumo.unidades);
    let c = &job.consumo;
    for v in [c.unidades, c.operacoes, c.operacoes_verificacao, c.cpu_ms, c.gpu_ms, c.memoria_mib_s, c.bytes_rede] {
        w.u64(v);
    }
    job.agregador.codificar(&mut w);
    w.u32(u32::try_from(job.workers.len()).unwrap_or(u32::MAX));
    for worker in &job.workers {
        w.fixed(worker);
    }
    for v in [job.recusas, job.repetidas, job.registros] {
        w.u64(v);
    }
    w.u8(u8::from(job.registro_cortado));
    let mut bytes = w.into_bytes();
    let integridade = sha512(&bytes);
    bytes.extend_from_slice(&integridade);
    Ok(bytes)
}

fn ler_checkpoint(bytes: &[u8], job: &mut Job) -> Result<(), String> {
    let corpo_len = bytes.len().checked_sub(HASH_LEN).ok_or("checkpoint curto")?;
    let (corpo, integridade) = bytes.split_at_checked(corpo_len).ok_or("checkpoint curto")?;
    if sha512(corpo).as_slice() != integridade {
        return Err("hash de integridade do checkpoint não confere".into());
    }
    let mut r = Reader::new(corpo);
    let erro = |e: hyurax_codec::CodecError| e.to_string();
    if r.u32().map_err(erro)? != VERSAO_CHECKPOINT {
        return Err("versão de checkpoint desconhecida".into());
    }
    if r.fixed::<HASH_LEN>().map_err(erro)? != job.id {
        return Err("checkpoint de outro JOB".into());
    }
    let estado = EstadoDoJob::de_codigo(r.u8().map_err(erro)?).ok_or("estado desconhecido")?;
    let motivo = r.string().map_err(erro)?.to_string();
    let (criado, inicio, fim) = (r.u64().map_err(erro)?, r.u64().map_err(erro)?, r.u64().map_err(erro)?);
    let feitas = Intervalos::decodificar(&mut r).map_err(|e| e.to_string())?;
    let abandonadas = Intervalos::decodificar(&mut r).map_err(|e| e.to_string())?;
    let mut falhas = BTreeMap::new();
    for _ in 0..r.u32().map_err(erro)? {
        falhas.insert(r.u64().map_err(erro)?, r.u8().map_err(erro)?);
    }
    let resumo = Resumo::de_bytes(r.fixed::<HASH_LEN>().map_err(erro)?, r.u64().map_err(erro)?);
    let mut c = [0u64; 7];
    for v in &mut c {
        *v = r.u64().map_err(erro)?;
    }
    let [unidades, operacoes, operacoes_verificacao, cpu_ms, gpu_ms, memoria_mib_s, bytes_rede] = c;
    let agregador = Agregador::decodificar(&mut r).map_err(|e| e.to_string())?;
    let mut workers = BTreeSet::new();
    for _ in 0..r.u32().map_err(erro)? {
        workers.insert(r.fixed::<PUBKEY_LEN>().map_err(erro)?);
    }
    let (recusas, repetidas, registros) = (r.u64().map_err(erro)?, r.u64().map_err(erro)?, r.u64().map_err(erro)?);
    let cortado = r.u8().map_err(erro)? == 1;
    r.finish().map_err(erro)?;
    if resumo.unidades != feitas.concluidas() || agregador.unidades() != feitas.concluidas() {
        return Err("checkpoint incoerente: resumo, agregador e intervalos não contam o mesmo".into());
    }
    job.estado = estado;
    job.motivo = motivo;
    job.criado_ms = criado;
    job.inicio_ms = inicio;
    job.fim_ms = fim;
    job.feitas = feitas;
    job.abandonadas = abandonadas;
    job.falhas = falhas;
    job.resumo = resumo;
    job.consumo = Consumo { unidades, operacoes, operacoes_verificacao, cpu_ms, gpu_ms, memoria_mib_s, bytes_rede };
    job.agregador = agregador;
    job.workers = workers;
    job.recusas = recusas;
    job.repetidas = repetidas;
    job.registros = registros;
    job.registro_cortado = cortado;
    Ok(())
}

/// `JOB_ID` a partir de hexadecimal completo.
pub fn id_de_hex(texto: &str) -> Option<[u8; HASH_LEN]> {
    de_hex(texto)
}

/// Monta o pedido de JOB a partir do formulário do painel. Campos:
/// `dominio`, `tipo` (códigos), `tamanho`, `passos`, `parametros` (lista com
/// vírgula), `unidades`, `nivel`, `redundancia`, `prazo_s`,
/// `orcamento_milicreditos` e `descricao`.
pub fn pedido_do_formulario(campos: &[(String, String)]) -> Result<PedidoDeJob, String> {
    let campo = |nome: &str| campos.iter().find(|(k, _)| k == nome).map(|(_, v)| v.trim().to_string());
    let num = |nome: &str, padrao: Option<u64>| -> Result<u64, String> {
        match campo(nome) {
            Some(v) if !v.is_empty() => v.parse::<u64>().map_err(|_| format!("{nome}: número inválido")),
            _ => padrao.ok_or(format!("falta o campo {nome}")),
        }
    };
    let pequeno = |nome: &str, padrao: Option<u64>| -> Result<u32, String> {
        u32::try_from(num(nome, padrao)?).map_err(|_| format!("{nome}: grande demais"))
    };
    let codigo = |nome: &str, padrao: Option<u64>| -> Result<u8, String> {
        u8::try_from(num(nome, padrao)?).map_err(|_| format!("{nome}: código inválido"))
    };
    let dominio = hyurax_ultrax::job::Dominio::de_codigo(codigo("dominio", None)?).ok_or("domínio desconhecido")?;
    let tipo = TipoDeTrabalho::de_codigo(codigo("tipo", None)?).ok_or("motor desconhecido")?;
    let parametros: Vec<u32> = match campo("parametros").filter(|v| !v.is_empty()) {
        Some(lista) => lista
            .split(',')
            .map(|v| v.trim().parse::<u32>().map_err(|_| format!("parâmetro inválido: {v}")))
            .collect::<Result<_, _>>()?,
        None => Vec::new(),
    };
    let modelo = trabalho::Especificacao::nova_com(tipo, pequeno("tamanho", None)?, pequeno("passos", Some(0))?, &parametros)
        .map_err(|e| e.to_string())?;
    Ok(PedidoDeJob {
        dominio,
        modelo,
        unidades: num("unidades", Some(1))?,
        nivel: Nivel::de_codigo(codigo("nivel", Some(2))?).ok_or("nível de verificação desconhecido")?,
        redundancia: codigo("redundancia", Some(1))?,
        prazo_s: num("prazo_s", Some(0))?,
        orcamento_milicreditos: num("orcamento_milicreditos", Some(0))?,
        descricao: campo("descricao").unwrap_or_default(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod testes {
    use super::*;
    use hyurax_ultrax::job::Dominio;
    use hyurax_ultrax::prova::RegistroDeProva;
    use hyurax_ultrax::trabalho::{Especificacao, MetodoDeVerificacao};

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hyurax-ciencia-teste-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn pedido(unidades: u64, lado: u32) -> PedidoDeJob {
        PedidoDeJob {
            dominio: Dominio::Matematica,
            modelo: Especificacao::nova(TipoDeTrabalho::Matriz, lado, 0).unwrap(),
            unidades,
            nivel: Nivel::Reexecucao,
            redundancia: 1,
            prazo_s: 0,
            orcamento_milicreditos: 0,
            descricao: "teste".into(),
        }
    }

    /// Desfecho fabricado, para testar o agendador sem o worker.
    fn liquidada(job: [u8; 64], indice: u64) -> DesfechoDeUnidade {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap();
        DesfechoDeUnidade {
            job,
            indice,
            tarefa: sha512(&indice.to_be_bytes()),
            estado: Estado::Liquidada,
            resultado: Some(vec![1, 2, 3]),
            assinatura: None,
            registro: Some(RegistroDeProva {
                tarefa: [0; 64],
                entrada: [1; 64],
                especificacao: esp,
                metodo: MetodoDeVerificacao::Freivalds,
                resultado: sha512(&indice.to_be_bytes()),
                operacoes: 512,
                worker: [9; 32],
                inicio_ms: 1,
                fim_ms: 2,
            }),
            operacoes_verificacao: 100,
            ms_calculo: 1,
            ms_verificacao: 1,
            memoria: 1024,
            gpu: None,
            nota: String::new(),
            abandonada: false,
        }
    }

    fn cancelada(job: [u8; 64], indice: u64, abandonada: bool) -> DesfechoDeUnidade {
        DesfechoDeUnidade {
            estado: Estado::Cancelada,
            resultado: None,
            registro: None,
            abandonada,
            nota: "teste".into(),
            ..liquidada(job, indice)
        }
    }

    fn estado(c: &Ciencia, id: &[u8; 64]) -> (EstadoDoJob, u64, u64) {
        let j = c.jobs.lock().unwrap();
        let x = j.iter().find(|x| &x.id == id).unwrap();
        (x.estado, x.feitas.concluidas(), x.abandonadas.concluidas())
    }

    /// A GPU recebe as unidades de JOB de matriz, a CPU confere o que volta,
    /// e a unidade fecha no JOB como se tivesse rodado na CPU.
    #[test]
    fn unidade_de_matriz_vai_para_a_gpu_e_fecha_no_job() {
        let p = pasta("gpu-job");
        let partida = crate::ultrax::Partida { ligado: true, linhas: 1, uso_cpu: 100, memoria_mib: 256, debug: false, gpu: true, gpu_uso: 100 };
        let u = Ultrax::abrir(&p, &[9; 32], 2, &partida, Box::new(|_, _| {}));
        u.lab.store(false, Ordering::Relaxed);
        let c = Ciencia::abrir(&p, u.worker(), "teste").unwrap();
        c.ligar(&u);
        let id = c.submeter(pedido(2, 16)).unwrap();
        for _ in 0..2 {
            let (numero, n, job) = u.gpu_pegar("GPU de teste").unwrap();
            assert_eq!(job.map(|(j, _)| j), Some(id), "a GPU recebeu a unidade do JOB");
            assert_eq!(n, 16);
            let entrada = u.gpu_entrada(numero).unwrap();
            let v: Vec<u64> = entrada.as_chunks::<4>().0.iter().map(|x| u64::from(u32::from_le_bytes(*x))).collect();
            let n = n as usize;
            let (a, b) = v.split_at(n * n);
            let mut m = vec![0u32; n * n];
            for i in 0..n {
                for k in 0..n {
                    for j in 0..n {
                        m[i * n + j] += (a[i * n + k] * b[k * n + j]) as u32;
                    }
                }
            }
            let bytes: Vec<u8> = m.iter().flat_map(|x| x.to_le_bytes()).collect();
            u.gpu_resultado(numero, &bytes).unwrap();
        }
        assert_eq!(estado(&c, &id), (EstadoDoJob::Concluido, 2, 0));
        // sem unidade pendente e sem LAB, a GPU não recebe nada
        assert!(u.gpu_pegar("GPU de teste").is_err());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn despacha_no_maximo_em_voo_e_conclui_com_as_unidades() {
        let p = pasta("despacho");
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        let id = c.submeter(pedido(20, 8)).unwrap();
        let mut despachadas = Vec::new();
        while let Some(u) = c.proxima(1) {
            despachadas.push(u.indice);
        }
        assert_eq!(despachadas, (0..EM_VOO_POR_JOB as u64).collect::<Vec<_>>(), "no máximo {EM_VOO_POR_JOB} em voo");
        for i in despachadas {
            c.terminou(liquidada(id, i));
        }
        while let Some(u) = c.proxima(1) {
            c.terminou(liquidada(id, u.indice));
        }
        assert_eq!(estado(&c, &id), (EstadoDoJob::Concluido, 20, 0));
        for arquivo in ["relatorio.json", "relatorio.csv", "relatorio.pdf", "unidades.jsonl", "eventos.jsonl", "checkpoint.bin"] {
            assert!(c.pasta_do_job(&id).join(arquivo).exists(), "{arquivo}");
        }
        let csv = std::fs::read_to_string(c.pasta_do_job(&id).join("relatorio.csv")).unwrap();
        assert_eq!(csv.lines().count(), 21, "cabeçalho e uma linha por unidade");
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn job_cancelado_nao_quer_mais_as_unidades_nem_recebe_resultado() {
        let p = pasta("cancelado");
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        let id = c.submeter(pedido(4, 8)).unwrap();
        let a = c.proxima(1).unwrap().indice;
        assert!(c.ainda_quer(&id, a));
        c.mudar(&id, "cancelar").unwrap();
        assert!(!c.ainda_quer(&id, a), "o worker para a unidade de JOB cancelado");
        // o resultado que chega depois não muda o JOB
        c.terminou(liquidada(id, a));
        assert_eq!(estado(&c, &id), (EstadoDoJob::Cancelado, 0, 0));
        assert!(c.jobs.lock().unwrap().iter().find(|j| j.id == id).unwrap().em_voo.is_empty());
        // JOB de outro nó (que não é daqui): segue o pedido dele
        assert!(c.ainda_quer(&[9; 64], 0));
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn job_pausado_fecha_quando_a_ultima_unidade_em_voo_termina() {
        let p = pasta("pausado");
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        let id = c.submeter(pedido(1, 8)).unwrap();
        let a = c.proxima(1).unwrap().indice;
        c.mudar(&id, "pausar").unwrap();
        c.terminou(liquidada(id, a));
        assert_eq!(estado(&c, &id), (EstadoDoJob::Concluido, 1, 0), "pausado com tudo feito vira concluído");
        assert!(c.pasta_do_job(&id).join("relatorio.json").exists());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn unidade_maior_que_o_teto_de_memoria_e_recusada_na_submissao() {
        let p = pasta("teto");
        let partida = crate::ultrax::Partida { ligado: false, linhas: 1, uso_cpu: 100, memoria_mib: 32, debug: false, gpu: false, gpu_uso: 50 };
        let u = Ultrax::abrir(&p, &[9; 32], 1, &partida, Box::new(|_, _| {}));
        let c = Ciencia::abrir(&p, u.worker(), "teste").unwrap();
        c.ligar(&u);
        let erro = c.submeter(pedido(2, 1024)).unwrap_err();
        assert!(erro.contains("teto de memória"), "{erro}");
        assert!(c.submeter(pedido(2, 16)).is_ok(), "a que cabe entra");
        u.encerrar_threads();
        c.encerrar_vigia();
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn encerrar_grava_o_checkpoint_do_que_acabou_de_mudar() {
        let p = pasta("encerrar");
        let id = {
            let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
            let id = c.submeter(pedido(3, 8)).unwrap();
            let a = c.proxima(1).unwrap().indice;
            c.terminou(liquidada(id, a));
            // fecha logo depois: sem esperar o vigia de 2 s
            c.encerrar();
            id
        };
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        assert_eq!(estado(&c, &id).1, 1, "a unidade consolidada sobreviveu ao fechamento");
        let _ = std::fs::remove_dir_all(p);
    }

    /// De ponta a ponta, pelas threads de verdade do worker (orquestrador e
    /// linhas), sem desfecho fabricado: o JOB sai da fila, cada unidade é
    /// calculada, conferida e consolidada, e o JOB fecha.
    #[test]
    fn job_na_cpu_pelas_linhas_do_worker_ate_concluir() {
        let p = pasta("ponta-a-ponta");
        let partida = crate::ultrax::Partida { ligado: true, linhas: 2, uso_cpu: 100, memoria_mib: 256, debug: false, gpu: false, gpu_uso: 50 };
        let u = Ultrax::abrir(&p, &[9; 32], 2, &partida, Box::new(|_, _| {}));
        u.lab.store(false, Ordering::Relaxed);
        let c = Ciencia::abrir(&p, u.worker(), "teste").unwrap();
        c.ligar(&u);
        let id = c.submeter(pedido(6, 16)).unwrap();
        u.iniciar();
        let inicio = std::time::Instant::now();
        while estado(&c, &id).0 != EstadoDoJob::Concluido && inicio.elapsed() < Duration::from_secs(180) {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(estado(&c, &id), (EstadoDoJob::Concluido, 6, 0), "o JOB não fechou pelas linhas do worker");
        let linhas = std::fs::read_to_string(c.pasta_do_job(&id).join("unidades.jsonl")).unwrap();
        assert_eq!(linhas.lines().count(), 6, "uma linha por unidade, com o registro conferido");
        assert!(linhas.lines().all(|l| l.contains("\"verificacao\":\"PASSED\"") && l.contains("\"programa\":\"teste\"")));
        // nada ficou preso: memória devolvida e nenhuma tarefa ativa
        let espera = std::time::Instant::now();
        while u.ocupado() && espera.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(u.memoria_reservada(), 0);
        u.encerrar_threads();
        c.encerrar_vigia();
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn unidade_cancelada_volta_e_abandonada_duas_vezes_vira_falha() {
        let p = pasta("falha");
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        let id = c.submeter(pedido(3, 8)).unwrap();
        let a = c.proxima(1).unwrap().indice;
        c.terminou(cancelada(id, a, false));
        assert_eq!(c.proxima(1).unwrap().indice, a, "cancelada sem julgamento volta para a fila");
        c.terminou(cancelada(id, a, true));
        assert_eq!(c.proxima(1).unwrap().indice, a, "primeiro abandono: tenta de novo");
        c.terminou(cancelada(id, a, true));
        assert_eq!(estado(&c, &id).2, 1, "segundo abandono: falha registrada");
        while let Some(u) = c.proxima(1) {
            assert_ne!(u.indice, a, "a unidade com falha não volta");
            c.terminou(liquidada(id, u.indice));
        }
        assert_eq!(estado(&c, &id), (EstadoDoJob::Concluido, 2, 1));
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn resultado_repetido_nao_entra_duas_vezes() {
        let p = pasta("duplicada");
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        let id = c.submeter(pedido(5, 8)).unwrap();
        let i = c.proxima(1).unwrap().indice;
        c.terminou(liquidada(id, i));
        c.terminou(liquidada(id, i));
        let j = c.jobs.lock().unwrap();
        let x = j.iter().find(|x| x.id == id).unwrap();
        assert_eq!((x.feitas.concluidas(), x.resumo.unidades, x.agregador.unidades()), (1, 1, 1));
        drop(j);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn retoma_do_checkpoint_e_recupera_do_anterior_se_o_novo_estragar() {
        let p = pasta("retomar");
        let id = {
            let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
            let id = c.submeter(pedido(10, 8)).unwrap();
            for _ in 0..4 {
                let u = c.proxima(1).unwrap();
                c.terminou(liquidada(id, u.indice));
            }
            // em voo quando "fecha": não pode sumir nem contar como feita
            let _ = c.proxima(1).unwrap();
            let jobs = c.jobs.lock().unwrap();
            let job = jobs.iter().find(|j| j.id == id).unwrap();
            c.gravar_checkpoint(job).unwrap();
            // um segundo checkpoint, para existir o `.anterior`
            c.gravar_checkpoint(job).unwrap();
            id
        };
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        assert_eq!(estado(&c, &id), (EstadoDoJob::Rodando, 4, 0), "voltou com as 4 feitas");
        let mut resto = Vec::new();
        while let Some(u) = c.proxima(1) {
            resto.push(u.indice);
            c.terminou(liquidada(id, u.indice));
        }
        resto.sort_unstable();
        assert_eq!(resto, vec![4, 5, 6, 7, 8, 9], "a unidade em voo voltou para a fila");
        assert_eq!(estado(&c, &id).0, EstadoDoJob::Concluido);
        let cp = c.pasta_do_job(&id).join("checkpoint.bin");
        drop(c);

        // estraga o checkpoint mais novo: volta o anterior
        let mut bytes = std::fs::read(&cp).unwrap();
        let meio = bytes.len() / 2;
        bytes[meio] ^= 0xFF;
        std::fs::write(&cp, bytes).unwrap();
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        assert!(c.eventos_desde(0).contains("CHECKPOINT_RECOVERED"));
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn orcamento_prazo_e_concordancia_param_o_despacho() {
        let p = pasta("limites");
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        // orçamento de 1 milicrédito = 1 milhão de operações; cada unidade fabricada gasta 612
        let mut barato = pedido(100_000, 8);
        barato.orcamento_milicreditos = 1;
        let id = c.submeter(barato).unwrap();
        let mut feitas = 0;
        while let Some(u) = c.proxima(1) {
            c.terminou(liquidada(id, u.indice));
            feitas += 1;
            assert!(feitas < 5000, "o orçamento devia ter parado o JOB");
        }
        assert_eq!(estado(&c, &id).0, EstadoDoJob::SemOrcamento);

        let mut curto = pedido(10, 16);
        curto.prazo_s = 1;
        let id = c.submeter(curto).unwrap();
        c.manutencao(agora_ms() + 5_000);
        assert_eq!(estado(&c, &id).0, EstadoDoJob::Vencido);

        let mut entre_nos = pedido(10, 32);
        entre_nos.nivel = Nivel::Concordancia;
        entre_nos.redundancia = 3;
        let id = c.submeter(entre_nos).unwrap();
        assert_eq!(estado(&c, &id).0, EstadoDoJob::AguardandoNos);
        assert!(c.proxima(1).is_none(), "sem outros nós, nada é despachado nem fingido");
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn pasta_de_versao_futura_fica_so_para_leitura() {
        let p = pasta("futura");
        std::fs::create_dir_all(p.join("ciencia")).unwrap();
        std::fs::write(p.join("ciencia").join("VERSAO"), "99\n").unwrap();
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        assert!(c.submeter(pedido(1, 8)).is_err());
        assert!(c.proxima(1).is_none());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn carga_de_vinte_mil_unidades_pelo_agendador() {
        let p = pasta("carga");
        let c = Ciencia::abrir(&p, [7; 32], "teste").unwrap();
        let id = c.submeter(pedido(20_000, 8)).unwrap();
        let inicio = Instant::now();
        let mut n = 0u64;
        let mut ja_voltou = std::collections::BTreeSet::new();
        while let Some(u) = c.proxima(1) {
            // algumas voltam para a fila uma vez antes de terminar
            if u.indice % 97 == 0 && ja_voltou.insert(u.indice) {
                c.terminou(cancelada(id, u.indice, false));
            } else {
                c.terminou(liquidada(id, u.indice));
            }
            n += 1;
        }
        let (e, feitas, falhas) = estado(&c, &id);
        assert_eq!((e, feitas, falhas), (EstadoDoJob::Concluido, 20_000, 0));
        let j = c.jobs.lock().unwrap();
        let x = j.iter().find(|x| x.id == id).unwrap();
        assert_eq!(x.feitas.faixas(), &[(0, 20_000)], "tudo numa faixa só");
        eprintln!("20.000 unidades pelo agendador em {:.2} s ({n} despachos)", inicio.elapsed().as_secs_f64());
        drop(j);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn formulario_vira_pedido() {
        let campos: Vec<(String, String)> = [("dominio", "7"), ("tipo", "1"), ("tamanho", "64"), ("unidades", "12"), ("descricao", "matrizes")]
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        let p = pedido_do_formulario(&campos).unwrap();
        assert_eq!((p.unidades, p.nivel, p.redundancia), (12, Nivel::Reexecucao, 1));
        let mut ruim = campos.clone();
        ruim.push(("parametros".into(), "1,2".into()));
        assert!(pedido_do_formulario(&ruim).is_err(), "matriz não leva parâmetros");
    }
}
