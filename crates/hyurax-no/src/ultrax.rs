// ✝ Colossenses 3:23 — “Tudo quanto fizerdes, fazei-o de todo o coração.”
//! ULTRAX no programa: o worker do modo **LAB**.
//!
//! O motor (tipos de trabalho, ciclo de vida, prova, validador, reputação e
//! Work Score) é o crate `hyurax-ultrax`. Aqui fica o que depende de máquina:
//!
//! - **gerador de tarefas (LAB)**: cria tarefas reais, com o tamanho ajustado
//!   ao ritmo medido desta máquina para cada uma levar alguns segundos;
//! - **gerenciador de recursos**: linhas, fatia da CPU e teto de memória. A
//!   memória de cada tarefa é reservada antes de ela começar; a CPU é limitada
//!   por pausas proporcionais ao tempo trabalhado, também durante a
//!   verificação; toda tarefa tem prazo e pode ser cancelada;
//! - **worker**: executa, assina o registro de prova, verifica, e passa por
//!   cada estado do ciclo de vida com horário;
//! - **tarefas-desafio**: a cada [`DESAFIO_A_CADA`] tarefas, uma cuja resposta
//!   já se sabe, porque veio do gabarito em Python (`vectors/utrax.json`). Se
//!   esta máquina errar, a tela avisa: pode ser defeito de hardware ou programa
//!   adulterado;
//! - **histórico e placar** em `PASTA/ultrax/`, que sobrevivem a reiniciar;
//! - **telemetria** com horário de cada passo, e o modo DEBUG, que também grava
//!   em arquivo;
//! - **auditoria**: `hyurax-no ultrax auditar` refaz as contas do histórico e
//!   confere id e assinatura de cada registro.
//!
//! **O que o modo LAB é, sem enfeite.** As tarefas nascem nesta máquina e são
//! conferidas por ela mesma, e ninguém de fora encomendou o trabalho: é carga
//! de teste (TESTNET WORKLOAD), não cliente. O que cada conferência pega:
//!
//! - **Freivalds** (matriz) usa outra conta, então pega erro passageiro e
//!   também defeito que se repete na multiplicação;
//! - **recomputação** (mochila, difusão) roda o mesmo código de novo, na mesma
//!   máquina: pega erro passageiro, **não** pega erro que se repete igual;
//! - **tarefa-desafio** compara com a resposta calculada fora desta máquina, e
//!   é ela que pega o erro que se repete.
//!
//! A reputação e o Work Score são medidas locais e não contam para rede
//! nenhuma. GPU não é usada. Quem tem o `no.chave` consegue assinar qualquer
//! registro: a auditoria prova coerência do histórico, não honestidade do dono.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN, ed25519_public_key, sha512};
use hyurax_ultrax::ia;
use hyurax_ultrax::job::semente_da_unidade;
use hyurax_ultrax::pontuacao::WorkScore;
use hyurax_ultrax::prova::{RegistroDeProva, chave_do_worker};
use hyurax_ultrax::reputacao::Reputacao;
use hyurax_ultrax::tarefa::{Estado, Instancia, Tarefa};
use hyurax_ultrax::trabalho::{
    self, ErroDeTrabalho, Especificacao, MetodoDeVerificacao, Recusa, TipoDeTrabalho, hash_da_entrada, hash_do_resultado,
};
use hyurax_ultrax::validador::Parecer;

use crate::painel::texto_json;
use crate::{de_hex, hex};

/// O modo de operação. TESTNET e REAL ainda não existem.
pub const MODO: &str = "LAB";
/// O selo que acompanha toda tarefa gerada aqui.
pub const SELO: &str = "TESTNET WORKLOAD";
/// Quanto tempo de cálculo uma tarefa deve levar, com a CPU inteira.
const SEGUNDOS_ALVO: f64 = 8.0;
/// Uma tarefa-desafio a cada tantas.
pub const DESAFIO_A_CADA: u64 = 10;
/// Tarefas esperando na fila, além das que estão rodando.
const FILA_EXTRA: usize = 1;
/// Quanto do histórico e da telemetria fica na memória, para o painel.
const HISTORICO_NA_MEMORIA: usize = 40;
const TELEMETRIA_NA_MEMORIA: usize = 120;
/// Tamanho a partir do qual o arquivo recomeça (o anterior vira `.1`).
const HISTORICO_MAX_BYTES: u64 = 4 * 1024 * 1024;
const TELEMETRIA_MAX_BYTES: u64 = 1024 * 1024;
/// Teto de memória, em MiB: padrão e faixa aceita.
pub const MEMORIA_PADRAO_MIB: u32 = 256;
pub const MEMORIA_MIN_MIB: u32 = 32;
pub const MEMORIA_MAX_MIB: u32 = 16 * 1024;
/// Ritmo inicial suposto, em operações por segundo, até medir o de verdade.
const RITMO_INICIAL: f64 = 20.0e6;
const RITMO_INICIAL_GPU: f64 = 50.0e6;
/// Quantas tarefas a GPU pode ter pendentes ao mesmo tempo (uma por janela aberta).
const GPU_TAREFAS_MAX: usize = 2;
/// Maior corpo aceito no resultado da GPU: uma matriz 1024 × 1024 em u32.
pub const GPU_RESULTADO_MAX: usize = 1024 * 1024 * 4;
/// Menor prazo de uma tarefa. O prazo cobre execução e verificação com a CPU
/// no menor limite permitido (10%), com folga de três vezes: baixar o limite
/// depois de criar a tarefa não a faz vencer.
const PRAZO_MINIMO_MS: u64 = 60_000;
const PRAZO_FOLGA: f64 = 3.0;
const USO_MINIMO: f64 = 0.10;
/// Pausas do limite de CPU só acontecem quando somam pelo menos isto: dormir
/// frações de milissegundo no Windows dorme bem mais que o pedido.
const PAUSA_MINIMA: Duration = Duration::from_millis(15);
const PAUSA_MAXIMA: Duration = Duration::from_secs(2);
/// Semente dos vetores do gabarito (`gen_vectors.vec_utrax`).
const SEMENTE_DOS_DESAFIOS: &[u8] = b"vetor utrax";

/// Uma tarefa de resposta conhecida.
struct Desafio {
    /// A semente que o gabarito usou nos vetores.
    semente: &'static [u8],
    tipo: TipoDeTrabalho,
    tamanho: u32,
    passos: u32,
    /// `RESULT_HASH` do resultado que o gabarito em Python calculou.
    esperado: &'static str,
}

/// Os maiores casos de `vectors/utrax.json` e de `vectors/ia.json`. O teste
/// `desafios_batem_com_os_vetores` confere cada hash contra os arquivos.
const DESAFIOS: [Desafio; 4] = [
    Desafio {
        semente: SEMENTE_DOS_DESAFIOS,
        tipo: TipoDeTrabalho::Matriz,
        tamanho: 16,
        passos: 0,
        esperado: "99c04ebd0b60b1f45a25db543baf9a711adc368c04bd546cf0d90b920268e32fd3cd7d3d4baa1c72f5a37cca20658163e30bd70567b9b824d16dd7f3b416a721",
    },
    Desafio {
        semente: SEMENTE_DOS_DESAFIOS,
        tipo: TipoDeTrabalho::Mochila,
        tamanho: 20,
        passos: 0,
        esperado: "fab8cbd7ae0f02316570aecec412b23647cc04d188c4764544a389d67ed2a8ef7b202787f5bb1eda76ed92d52ac3da17610b24b1b41b5762323c62c2804450b5",
    },
    Desafio {
        semente: SEMENTE_DOS_DESAFIOS,
        tipo: TipoDeTrabalho::Difusao,
        tamanho: 16,
        passos: 10,
        esperado: "8bf5147ea7293fe06e363b45cd40d27d1d5076187ab4cdbea5e936e5feb028a54f491be69c8ecff844df73a6928a87e9bbd4a84729e75a7f4be4f9acbb4984c9",
    },
    Desafio {
        semente: b"vetor ia",
        tipo: TipoDeTrabalho::Ia,
        tamanho: 16,
        passos: 25,
        esperado: "96dc8131d4839cff07f6cd34112e7508988a1b0dc15abb15df328337ed8068795157860565ed786891884b2835f43a345b4ab073d91f6e1ad92c7665b7beddee",
    },
];

/// O desafio do gabarito que tem exatamente esta especificação.
fn desafio_de(esp: &Especificacao) -> Option<&'static Desafio> {
    DESAFIOS.iter().find(|d| d.tipo == esp.tipo() && d.tamanho == esp.tamanho() && d.passos == esp.passos())
}

/// A semente de um desafio, a mesma do vetor do gabarito.
fn semente_do_desafio(esp: &Especificacao) -> [u8; HASH_LEN] {
    sha512(desafio_de(esp).map_or(SEMENTE_DOS_DESAFIOS, |d| d.semente))
}

fn agora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn indice(tipo: TipoDeTrabalho) -> usize {
    match tipo {
        TipoDeTrabalho::Matriz => 0,
        TipoDeTrabalho::Mochila => 1,
        TipoDeTrabalho::Difusao => 2,
        TipoDeTrabalho::Ia => 3,
        // os científicos têm ritmo e contagem próprios, fora dos quatro do LAB
        TipoDeTrabalho::Genetica => 4,
        TipoDeTrabalho::Melhoramento => 5,
        TipoDeTrabalho::Rotas => 6,
        TipoDeTrabalho::Triagem => 7,
    }
}

/// O melhor modelo de IA treinado nesta máquina até agora, e o último treino.
#[derive(Clone, Debug, Default)]
struct Modelo {
    /// Pesos e erro de validação (Q12) do melhor treino.
    melhor: Option<(ia::Pesos, u64)>,
    /// Tarefa que treinou o melhor.
    tarefa: u32,
    /// Treinos verificados.
    treinos: u64,
    /// Curva e erro do último treino.
    ultima_curva: Vec<u64>,
    ultimo_erro: Option<u64>,
}

impl Modelo {
    fn texto(&self) -> String {
        let mut t = String::from("# Melhor modelo de IA do ULTRAX (modo LAB): pesos e erro, codificados como no resultado da tarefa.\n");
        if let Some((pesos, erro)) = &self.melhor {
            let _ = writeln!(t, "melhor={}", hex(&ia::codificar(pesos, *erro)));
        }
        let _ = writeln!(t, "tarefa={}", self.tarefa);
        let _ = writeln!(t, "treinos={}", self.treinos);
        if let Some(e) = self.ultimo_erro {
            let _ = writeln!(t, "ultimo_erro={e}");
        }
        let curva: Vec<String> = self.ultima_curva.iter().map(u64::to_string).collect();
        let _ = writeln!(t, "ultima_curva={}", curva.join(","));
        t
    }

    fn de_texto(texto: &str) -> Self {
        let mut m = Self::default();
        for linha in texto.lines() {
            match linha.trim().split_once('=') {
                Some(("melhor", v)) => {
                    m.melhor = decodificar_hex(v.trim()).and_then(|b| ia::decodificar(&b));
                }
                Some(("tarefa", v)) => m.tarefa = v.trim().parse().unwrap_or(0),
                Some(("treinos", v)) => m.treinos = v.trim().parse().unwrap_or(0),
                Some(("ultimo_erro", v)) => m.ultimo_erro = v.trim().parse().ok(),
                Some(("ultima_curva", v)) => {
                    m.ultima_curva = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                }
                _ => {}
            }
        }
        m
    }
}

fn decodificar_hex(texto: &str) -> Option<Vec<u8>> {
    if !texto.len().is_multiple_of(2) {
        return None;
    }
    (0..texto.len())
        .step_by(2)
        .map(|i| texto.get(i..i.saturating_add(2)).and_then(|par| u8::from_str_radix(par, 16).ok()))
        .collect()
}

/// A curva em até `pontos` trechos, cada um com a média dos passos dele. O
/// erro de um lote só é ruidoso; a média do trecho mostra a tendência de
/// verdade, sem inventar ponto nenhum.
fn resumir_curva(curva: &[u64], pontos: usize) -> Vec<u64> {
    if curva.len() <= pontos || pontos == 0 {
        return curva.to_vec();
    }
    (0..pontos)
        .filter_map(|k| {
            let inicio = k.saturating_mul(curva.len()) / pontos;
            let fim = k.saturating_add(1).saturating_mul(curva.len()) / pontos;
            let trecho = curva.get(inicio..fim.max(inicio.saturating_add(1)))?;
            let soma = trecho.iter().fold(0u128, |s, &v| s.saturating_add(u128::from(v)));
            u64::try_from(soma / trecho.len().max(1) as u128).ok()
        })
        .collect()
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Uma unidade de JOB que o agendador entrega ao worker.
#[derive(Clone, Copy, Debug)]
pub struct PedidoDeUnidade {
    /// `JOB_ID`.
    pub job: [u8; HASH_LEN],
    /// Índice da unidade no JOB.
    pub indice: u64,
    /// A especificação da unidade, já derivada.
    pub especificacao: Especificacao,
    /// A semente da unidade, `H(DOMINIO_UNIDADE || JOB_ID || i)`.
    pub semente: [u8; HASH_LEN],
    /// Prioridade na fila (a LAB usa 100).
    pub prioridade: u8,
    /// Prazo da unidade, em milissegundos desde 1970.
    pub prazo_ms: u64,
}

/// Como uma unidade de JOB saiu do worker.
#[derive(Clone, Debug)]
pub struct DesfechoDeUnidade {
    /// `JOB_ID`.
    pub job: [u8; HASH_LEN],
    /// Índice da unidade.
    pub indice: u64,
    /// `TASK_ID` da tarefa que levou a unidade.
    pub tarefa: [u8; HASH_LEN],
    /// Estado em que ela parou. [`Estado::Recusada`] não é final: o worker
    /// repete uma vez antes de desistir.
    pub estado: Estado,
    /// Os bytes do resultado, só quando liquidada.
    pub resultado: Option<Vec<u8>>,
    /// O registro de prova assinado, quando houve execução.
    pub registro: Option<RegistroDeProva>,
    /// Operações gastas conferindo.
    pub operacoes_verificacao: u64,
    /// Cálculo puro da execução, em ms.
    pub ms_calculo: u64,
    /// Cálculo puro da conferência, em ms.
    pub ms_verificacao: u64,
    /// Memória reservada, em bytes.
    pub memoria: u64,
    /// O nome da GPU, quando a conta foi feita nela.
    pub gpu: Option<String>,
    /// O motivo, quando não foi liquidada.
    pub nota: String,
    /// Recusada duas vezes: o worker desistiu por erro que se repete.
    pub abandonada: bool,
}

/// Uma unidade de JOB rodando agora, com o progresso da fase atual.
#[derive(Clone, Copy, Debug)]
pub struct UnidadeAtiva {
    /// `JOB_ID`.
    pub job: [u8; HASH_LEN],
    /// Índice da unidade.
    pub indice: u64,
    /// Linha de CPU.
    pub linha: u32,
    /// Operações feitas na fase atual (execução ou verificação).
    pub feitas: u64,
    /// Total da fase atual.
    pub total: u64,
    /// Estado (executando ou verificando).
    pub estado: Estado,
    /// Quando começou, em ms desde 1970.
    pub inicio_ms: u64,
}

/// Quem entrega unidades de JOB ao worker e recebe como terminaram.
pub trait Agendador: Send + Sync {
    /// A próxima unidade para a fila, se houver.
    fn proxima(&self, agora_ms: u64) -> Option<PedidoDeUnidade>;
    /// A unidade começou a executar nesta linha.
    fn comecou(&self, job: &[u8; HASH_LEN], indice: u64, linha: u32, entrada: Option<[u8; HASH_LEN]>);
    /// A unidade terminou, foi recusada uma vez, cancelada ou venceu.
    fn terminou(&self, desfecho: DesfechoDeUnidade);
}

/// Uma tarefa na fila, com o que o worker precisa saber além dela.
struct NaFila {
    tarefa: Tarefa,
    /// Índice em [`DESAFIOS`], quando é tarefa-desafio.
    desafio: Option<usize>,
    /// Quantas vezes já foi recusada. Uma recusa volta para a fila uma vez,
    /// para separar defeito passageiro de erro que se repete.
    recusas: u8,
    /// A origem que entrou no `TASK_ID`, guardada para a auditoria refazer:
    /// 32 bytes sorteados na LAB, a semente inteira (64) numa unidade de JOB.
    origem: Vec<u8>,
    /// `(JOB_ID, índice)` quando é unidade de JOB.
    job: Option<([u8; HASH_LEN], u64)>,
}

/// O que a execução deixa para a conferência.
struct Conclusao {
    linha: u32,
    item: NaFila,
    esp: Especificacao,
    semente: [u8; HASH_LEN],
    entrada: Option<[u8; HASH_LEN]>,
    exec: trabalho::Execucao,
    inicio_ms: u64,
    ms_calculo: u64,
    memoria: u64,
    total: u64,
    feitas: Arc<AtomicU64>,
    /// O nome da GPU, quando a conta foi feita nela.
    gpu: Option<String>,
}

/// Uma tarefa entregue à GPU da página, esperando o resultado.
struct NaGpu {
    item: NaFila,
    semente: [u8; HASH_LEN],
    entrada: Option<[u8; HASH_LEN]>,
    memoria: u64,
    inicio_ms: u64,
    feitas: Arc<AtomicU64>,
    nome: String,
}

/// Uma tarefa rodando agora, para o painel.
struct Ativa {
    linha: u32,
    /// `Some(nome)` quando a conta está na GPU.
    gpu: Option<String>,
    numero: u32,
    id: [u8; HASH_LEN],
    especificacao: Especificacao,
    metodo: MetodoDeVerificacao,
    desafio: bool,
    estado: Estado,
    /// Operações da fase atual (execução ou verificação), e o total dela.
    feitas: Arc<AtomicU64>,
    total: u64,
    operacoes: u64,
    inicio_ms: u64,
    memoria: u64,
    entrada: Option<[u8; HASH_LEN]>,
    /// `(JOB_ID, índice)` quando é unidade de JOB.
    job: Option<([u8; HASH_LEN], u64)>,
}

/// Os números acumulados, gravados em `PASTA/ultrax/placar.txt`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Placar {
    /// Tarefas que chegaram a SETTLED.
    pub liquidadas: u64,
    /// Execuções recusadas na conferência.
    pub recusadas: u64,
    /// Canceladas (parou, mudou o limite, não coube na memória).
    pub canceladas: u64,
    /// Passaram do prazo.
    pub expiradas: u64,
    /// Tarefas-desafio certas e erradas.
    pub desafios_certos: u64,
    /// Tarefas-desafio erradas.
    pub desafios_errados: u64,
    /// Work Score (só tarefa verificada soma).
    pub score: WorkScore,
    /// Reputação local deste worker.
    pub reputacao: Reputacao,
    /// Tempo de cálculo, sem contar as pausas do limite, em milissegundos.
    pub ms_calculo: u64,
    /// Tempo de conferência.
    pub ms_verificacao: u64,
    /// Segundos com o ULTRAX ligado.
    pub segundos_ligado: u64,
    /// Tarefas verificadas por tipo: matriz, mochila, difusão.
    pub por_tipo: [u64; 4],
    /// Tarefas geradas desde sempre: é o que decide a vez da tarefa-desafio.
    pub geradas: u64,
    /// Recusadas duas vezes: erro que se repete, e não parada de quem manda.
    pub abandonadas: u64,
}

impl Placar {
    fn texto(&self) -> String {
        let r = &self.reputacao;
        let mut t = String::from("# Placar do ULTRAX (modo LAB). Medida local; não é dinheiro nem HYX.\n");
        for (nome, v) in [
            ("liquidadas", self.liquidadas),
            ("recusadas", self.recusadas),
            ("canceladas", self.canceladas),
            ("expiradas", self.expiradas),
            ("desafios_certos", self.desafios_certos),
            ("desafios_errados", self.desafios_errados),
            ("operacoes_verificadas", self.score.operacoes_verificadas),
            ("operacoes_sem_credito", self.score.operacoes_sem_credito),
            ("ms_calculo", self.ms_calculo),
            ("ms_verificacao", self.ms_verificacao),
            ("segundos_ligado", self.segundos_ligado),
            ("matriz", self.por_tipo[0]),
            ("mochila", self.por_tipo[1]),
            ("difusao", self.por_tipo[2]),
            ("ia", self.por_tipo[3]),
            ("geradas", self.geradas),
            ("abandonadas", self.abandonadas),
            ("rep_enviadas", r.enviadas),
            ("rep_verificadas", r.verificadas),
            ("rep_recusadas", r.recusadas),
            ("rep_divergentes", r.divergentes),
            ("rep_disputadas", r.disputadas),
            ("rep_segundos_ativo", r.segundos_ativo),
        ] {
            let _ = writeln!(t, "{nome}={v}");
        }
        t
    }

    fn de_texto(texto: &str) -> Self {
        let mut p = Self::default();
        for linha in texto.lines() {
            let Some((nome, valor)) = linha.trim().split_once('=') else { continue };
            let Ok(v) = valor.trim().parse::<u64>() else { continue };
            let campo = match nome {
                "liquidadas" => &mut p.liquidadas,
                "recusadas" => &mut p.recusadas,
                "canceladas" => &mut p.canceladas,
                "expiradas" => &mut p.expiradas,
                "desafios_certos" => &mut p.desafios_certos,
                "desafios_errados" => &mut p.desafios_errados,
                "operacoes_verificadas" => &mut p.score.operacoes_verificadas,
                "operacoes_sem_credito" => &mut p.score.operacoes_sem_credito,
                "ms_calculo" => &mut p.ms_calculo,
                "ms_verificacao" => &mut p.ms_verificacao,
                "segundos_ligado" => &mut p.segundos_ligado,
                "matriz" => &mut p.por_tipo[0],
                "mochila" => &mut p.por_tipo[1],
                "difusao" => &mut p.por_tipo[2],
                "ia" => &mut p.por_tipo[3],
                "geradas" => &mut p.geradas,
                "abandonadas" => &mut p.abandonadas,
                "rep_enviadas" => &mut p.reputacao.enviadas,
                "rep_verificadas" => &mut p.reputacao.verificadas,
                "rep_recusadas" => &mut p.reputacao.recusadas,
                "rep_divergentes" => &mut p.reputacao.divergentes,
                "rep_disputadas" => &mut p.reputacao.disputadas,
                "rep_segundos_ativo" => &mut p.reputacao.segundos_ativo,
                _ => continue,
            };
            *campo = v;
        }
        p
    }
}

/// Uma linha do histórico recente, para o painel.
#[derive(Clone)]
struct Lembranca {
    numero: u32,
    tipo: TipoDeTrabalho,
    resumo: String,
    metodo: MetodoDeVerificacao,
    desafio: bool,
    estado: Estado,
    operacoes: u64,
    ms_calculo: u64,
    fim_ms: u64,
    resultado: Option<[u8; HASH_LEN]>,
    nota: String,
}

/// Um passo registrado na telemetria.
struct Marca {
    ms: u64,
    tarefa: u32,
    evento: &'static str,
    detalhe: String,
}

/// Quem recebe os avisos que merecem ir para o fluxo de eventos do painel.
pub type Aviso = Box<dyn Fn(&'static str, String) + Send + Sync>;

/// O worker LAB e tudo o que ele mede.
pub struct Ultrax {
    pasta: PathBuf,
    segredo: [u8; SECRET_LEN],
    worker: [u8; PUBKEY_LEN],
    nucleos: u32,
    /// O dono ligou o ULTRAX.
    pub ligado: AtomicBool,
    /// Gera trabalho LAB quando não há unidade de JOB. Desligado, a fila só
    /// recebe unidades de JOB (é o que `hyurax-no ciencia rodar` usa).
    pub lab: AtomicBool,
    /// Quantas linhas trabalham ao mesmo tempo.
    pub linhas: AtomicU32,
    /// Fatia da CPU de cada linha, de 10 a 100 por cento.
    pub uso_cpu: AtomicU32,
    /// Teto de memória somando todas as tarefas, em MiB.
    pub memoria_mib: AtomicU32,
    /// Telemetria detalhada, também em arquivo.
    pub debug: AtomicBool,
    /// A GPU da página (WebGL2) também trabalha.
    pub gpu_ligada: AtomicBool,
    /// Fatia de tempo da GPU, de 10 a 100 por cento.
    pub gpu_uso: AtomicU32,
    gpu: Mutex<Vec<NaGpu>>,
    gpu_nome: Mutex<String>,
    ritmo_gpu: Mutex<f64>,
    sequencia: AtomicU64,
    reservada: AtomicU64,
    /// Trechos de ciclo encerrados nesta abertura (para o terminal imprimir),
    /// e tarefas que chegaram a um estado final.
    encerrados: AtomicU64,
    finais: AtomicU64,
    /// Uma escrita de cada vez nos arquivos, para linhas não se misturarem.
    escrita: Mutex<()>,
    fila: Mutex<VecDeque<NaFila>>,
    ativas: Mutex<Vec<Ativa>>,
    placar: Mutex<Placar>,
    historico: Mutex<VecDeque<Lembranca>>,
    telemetria: Mutex<VecDeque<Marca>>,
    /// Operações por segundo de cálculo puro, por tipo (média que esquece devagar).
    ritmo: Mutex<[f64; 8]>,
    /// Quem entrega unidades de JOB, quando há.
    agendador: Mutex<Option<Arc<dyn Agendador>>>,
    /// O melhor modelo de IA e o último treino, gravados em `modelo.txt`.
    modelo: Mutex<Modelo>,
    aviso: Aviso,
}

/// Ajustes de partida.
pub struct Partida {
    /// Começa ligado.
    pub ligado: bool,
    /// Linhas.
    pub linhas: u32,
    /// Fatia da CPU.
    pub uso_cpu: u32,
    /// Teto de memória.
    pub memoria_mib: u32,
    /// Telemetria em arquivo.
    pub debug: bool,
    /// A GPU da página também trabalha.
    pub gpu: bool,
    /// Fatia de tempo da GPU.
    pub gpu_uso: u32,
}

impl Ultrax {
    /// Abre o worker na pasta de dados do nó. O segredo é o da identidade do
    /// nó; a chave de assinatura do worker é derivada dele.
    pub fn abrir(dados: &Path, segredo_do_no: &[u8; SECRET_LEN], nucleos: u32, partida: &Partida, aviso: Aviso) -> Arc<Self> {
        let pasta = dados.join("ultrax");
        let _ = std::fs::create_dir_all(&pasta);
        let segredo = chave_do_worker(segredo_do_no);
        let placar = std::fs::read_to_string(pasta.join("placar.txt")).map(|t| Placar::de_texto(&t)).unwrap_or_default();
        let nucleos = nucleos.max(1);
        let u = Arc::new(Self {
            worker: ed25519_public_key(&segredo),
            segredo,
            nucleos,
            ligado: AtomicBool::new(partida.ligado),
            lab: AtomicBool::new(true),
            linhas: AtomicU32::new(partida.linhas.clamp(1, nucleos)),
            uso_cpu: AtomicU32::new(partida.uso_cpu.clamp(10, 100)),
            memoria_mib: AtomicU32::new(partida.memoria_mib.clamp(MEMORIA_MIN_MIB, MEMORIA_MAX_MIB)),
            debug: AtomicBool::new(partida.debug),
            gpu_ligada: AtomicBool::new(partida.gpu),
            gpu_uso: AtomicU32::new(partida.gpu_uso.clamp(10, 100)),
            gpu: Mutex::new(Vec::new()),
            gpu_nome: Mutex::new(String::new()),
            ritmo_gpu: Mutex::new(RITMO_INICIAL_GPU),
            sequencia: AtomicU64::new(placar.geradas),
            reservada: AtomicU64::new(0),
            encerrados: AtomicU64::new(0),
            finais: AtomicU64::new(0),
            escrita: Mutex::new(()),
            fila: Mutex::new(VecDeque::new()),
            ativas: Mutex::new(Vec::new()),
            placar: Mutex::new(placar),
            historico: Mutex::new(VecDeque::new()),
            telemetria: Mutex::new(VecDeque::new()),
            ritmo: Mutex::new([RITMO_INICIAL; 8]),
            agendador: Mutex::new(None),
            modelo: Mutex::new(Modelo::default()),
            aviso,
            pasta,
        });
        u.carregar_historico_recente();
        if let (Ok(texto), Ok(mut m)) = (std::fs::read_to_string(u.pasta.join("modelo.txt")), u.modelo.lock()) {
            *m = Modelo::de_texto(&texto);
        }
        u
    }

    /// Põe o orquestrador e as linhas para rodar. As linhas acima do ajuste
    /// ficam dormindo; mudar o ajuste acorda ou adormece sem reiniciar nada.
    pub fn iniciar(self: &Arc<Self>) {
        if self.ligado.load(Ordering::Relaxed) {
            (self.aviso)("ultrax", format!("ULTRAX ligado (modo {MODO}, {SELO}): trabalho de teste gerado nesta máquina"));
        }
        {
            let u = Arc::clone(self);
            std::thread::spawn(move || u.orquestrar());
        }
        for i in 0..self.nucleos {
            let u = Arc::clone(self);
            std::thread::spawn(move || u.linha(i));
        }
    }

    /// Liga o agendador de JOBs: dali em diante, a fila pede unidades a ele
    /// antes de gerar trabalho LAB.
    pub fn ligar_agendador(&self, agendador: Arc<dyn Agendador>) {
        if let Ok(mut a) = self.agendador.lock() {
            *a = Some(agendador);
        }
    }

    fn agendador(&self) -> Option<Arc<dyn Agendador>> {
        self.agendador.lock().ok().and_then(|a| a.clone())
    }

    /// Tarefas na fila ou rodando (para esperar tudo terminar).
    pub fn ocupado(&self) -> bool {
        self.fila.lock().is_ok_and(|f| !f.is_empty()) || self.ativas.lock().is_ok_and(|a| !a.is_empty())
    }

    /// As unidades de JOB rodando agora. É o que a tela usa para mostrar o
    /// progresso real de cada unidade.
    pub fn unidades_ativas(&self) -> Vec<UnidadeAtiva> {
        self.ativas.lock().map_or_else(
            |_| Vec::new(),
            |a| {
                a.iter()
                    .filter_map(|x| {
                        x.job.map(|(job, indice)| UnidadeAtiva {
                            job,
                            indice,
                            linha: x.linha,
                            feitas: x.feitas.load(Ordering::Relaxed).min(x.total),
                            total: x.total,
                            estado: x.estado,
                            inicio_ms: x.inicio_ms,
                        })
                    })
                    .collect()
            },
        )
    }

    /// Fatia da CPU de cada linha, em %.
    pub fn uso_cpu(&self) -> u32 {
        self.uso_cpu.load(Ordering::Relaxed)
    }

    /// Ritmo medido (operações por segundo de cálculo puro) de um tipo.
    pub fn ritmo_de(&self, tipo: TipoDeTrabalho) -> f64 {
        self.ritmo.lock().map_or(RITMO_INICIAL, |r| r.get(indice(tipo)).copied().unwrap_or(RITMO_INICIAL))
    }

    /// `WORKER_ID`: a chave pública Ed25519 deste worker.
    pub fn worker(&self) -> [u8; PUBKEY_LEN] {
        self.worker
    }

    /// Cópia do placar.
    pub fn placar(&self) -> Placar {
        self.placar.lock().map(|p| *p).unwrap_or_default()
    }

    /// Liga ou desliga. Desligar interrompe o que está rodando: a tarefa em
    /// curso vira CANCELLED, sem julgamento, e a fila é descartada.
    pub fn ligar(&self, ligar: bool) {
        if self.ligado.swap(ligar, Ordering::Relaxed) == ligar {
            return;
        }
        if ligar {
            (self.aviso)("ultrax", format!("ULTRAX ligado (modo {MODO}, {SELO}): trabalho de teste gerado nesta máquina"));
        } else {
            self.descartar_fila("o ULTRAX foi desligado");
            self.cancelar_gpu("o ULTRAX foi desligado");
            (self.aviso)("ultrax", "ULTRAX desligado".into());
        }
    }

    /// Muda linhas, fatia da CPU, teto de memória, o modo DEBUG e a GPU (ligada
    /// e fatia de tempo). Valores fora da faixa são trazidos para dentro dela.
    pub fn ajustar_tudo(
        &self,
        linhas: Option<u32>,
        uso_cpu: Option<u32>,
        memoria_mib: Option<u32>,
        debug: Option<bool>,
        gpu: Option<bool>,
        gpu_uso: Option<u32>,
    ) {
        if let Some(u) = gpu_uso {
            self.gpu_uso.store(u.clamp(10, 100), Ordering::Relaxed);
        }
        if let Some(g) = gpu
            && self.gpu_ligada.swap(g, Ordering::Relaxed) != g
        {
            if g {
                (self.aviso)("ultrax", "GPU ligada no ULTRAX: a janela do programa manda contas para a GPU, e a CPU confere cada uma".into());
            } else {
                self.cancelar_gpu("a GPU foi desligada");
            }
        }
        if let Some(n) = linhas {
            let novo = n.clamp(1, self.nucleos);
            if self.linhas.swap(novo, Ordering::Relaxed) != novo {
                // a fila foi dimensionada para a fatia de memória de cada linha
                self.descartar_fila("o número de linhas mudou");
            }
        }
        if let Some(u) = uso_cpu {
            self.uso_cpu.store(u.clamp(10, 100), Ordering::Relaxed);
        }
        if let Some(m) = memoria_mib {
            let novo = m.clamp(MEMORIA_MIN_MIB, MEMORIA_MAX_MIB);
            if self.memoria_mib.swap(novo, Ordering::Relaxed) != novo {
                // o que já está na fila foi dimensionado para o teto antigo
                self.descartar_fila("o teto de memória mudou");
            }
        }
        if let Some(d) = debug {
            self.debug.store(d, Ordering::Relaxed);
        }
    }

    // -----------------------------------------------------------------------
    // Gerador de tarefas (LAB)
    // -----------------------------------------------------------------------

    fn orquestrar(self: Arc<Self>) {
        let mut ultimo_gravado = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            if self.ligado.load(Ordering::Relaxed) {
                if let Ok(mut p) = self.placar.lock() {
                    p.segundos_ligado = p.segundos_ligado.saturating_add(1);
                    p.reputacao.segundos_ativo = p.reputacao.segundos_ativo.saturating_add(1);
                }
                self.expirar_na_fila();
                self.expirar_na_gpu();
                let quer = (self.linhas.load(Ordering::Relaxed) as usize).saturating_add(FILA_EXTRA);
                while self.ligado.load(Ordering::Relaxed) && self.fila.lock().map_or(usize::MAX, |f| f.len()) < quer {
                    // unidade de JOB primeiro; a LAB só preenche o que sobra
                    let proxima = self.agendador().and_then(|a| a.proxima(agora_ms()));
                    let item = match proxima {
                        Some(pedido) => self.unidade_de_job(pedido),
                        None if self.lab.load(Ordering::Relaxed) => self.gerar(agora_ms()),
                        None => break,
                    };
                    match item {
                        Ok(item) => self.enfileirar(item, false),
                        Err(e) => {
                            self.marcar(0, "GENERATOR ERROR", e);
                            break;
                        }
                    }
                }
            }
            // grava mesmo desligado: o tempo e os cancelamentos do desligar também contam
            if ultimo_gravado.elapsed() >= Duration::from_secs(30) {
                self.gravar_placar();
                ultimo_gravado = Instant::now();
            }
        }
    }

    /// Põe na fila, se o ULTRAX ainda estiver ligado; senão, cancela. A
    /// conferência e o empilhamento acontecem com a fila travada, então nada
    /// entra depois de desligar.
    fn enfileirar(&self, item: NaFila, na_frente: bool) {
        let sobrou = match self.fila.lock() {
            Ok(mut f) if self.ligado.load(Ordering::Relaxed) => {
                if na_frente {
                    f.push_front(item);
                } else {
                    f.push_back(item);
                }
                None
            }
            _ => Some(item),
        };
        if let Some(mut item) = sobrou {
            let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), "o ULTRAX foi desligado");
            self.encerrar(&item, None, 0, "o ULTRAX foi desligado");
        }
    }

    /// Cria uma tarefa e já a põe em QUEUED.
    fn gerar(&self, agora: u64) -> Result<NaFila, String> {
        let k = self.sequencia.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut p) = self.placar.lock() {
            p.geradas = p.geradas.max(k.saturating_add(1));
        }
        let mut origem = [0u8; 32];
        hyurax_net::entropia::preencher(&mut origem)?;
        let e_desafio = k % DESAFIO_A_CADA == DESAFIO_A_CADA - 1;
        let (especificacao, metodo, instancia, prioridade, desafio) = if e_desafio {
            let qual = usize::try_from((k / DESAFIO_A_CADA) % DESAFIOS.len() as u64).unwrap_or(0);
            let d = DESAFIOS.get(qual).ok_or("desafio inexistente")?;
            let esp = Especificacao::nova(d.tipo, d.tamanho, d.passos).map_err(|e| e.to_string())?;
            (esp, MetodoDeVerificacao::ResultadoEsperado, Instancia::Compartilhada, 200, Some(qual))
        } else {
            // os desafios não tiram a vez de ninguém: a rotação conta só as normais
            let normais = k.saturating_sub(k / DESAFIO_A_CADA);
            let tipo = TipoDeTrabalho::ROTACAO_LAB
                .get(usize::try_from(normais % TipoDeTrabalho::ROTACAO_LAB.len() as u64).unwrap_or(0))
                .copied()
                .unwrap_or(TipoDeTrabalho::Matriz);
            let esp = self.dimensionar(tipo)?;
            (esp, tipo.metodo(), Instancia::PorWorker, 100, None)
        };
        let estimado = self.segundos_estimados(&especificacao);
        let prazo = agora.saturating_add(PRAZO_MINIMO_MS.max((estimado / USO_MINIMO * PRAZO_FOLGA * 1000.0) as u64));
        let mut tarefa = Tarefa::nova(especificacao, metodo, instancia, &origem, prioridade, agora, prazo)
            .map_err(|e| e.to_string())?;
        let numero = tarefa.numero();
        let rotulo = if desafio.is_some() { format!("{SELO} · tarefa-desafio de resposta conhecida") } else { SELO.to_string() };
        self.marcar(
            numero,
            "TASK CREATED",
            format!(
                "{} {} · {} · verificação: {} · {rotulo}",
                especificacao.tipo().descricao(),
                especificacao.resumo(),
                especificacao.tipo().categoria().nome(),
                metodo.nome()
            ),
        );
        tarefa.avancar(Estado::NaFila, agora_ms(), "").map_err(|e| e.to_string())?;
        self.marcar(numero, "TASK QUEUED", String::new());
        Ok(NaFila { tarefa, desafio, recusas: 0, origem: origem.to_vec(), job: None })
    }

    /// Transforma uma unidade de JOB numa tarefa, já em QUEUED. A semente é a
    /// da unidade ([`Instancia::DeJob`]), e o método é o do tipo.
    fn unidade_de_job(&self, p: PedidoDeUnidade) -> Result<NaFila, String> {
        let agora = agora_ms();
        let metodo = p.especificacao.tipo().metodo();
        let mut tarefa = Tarefa::nova(p.especificacao, metodo, Instancia::DeJob, &p.semente, p.prioridade, agora, p.prazo_ms)
            .map_err(|e| e.to_string())?;
        let numero = tarefa.numero();
        self.marcar(
            numero,
            "TASK CREATED",
            format!(
                "JOB {}… unidade {} · {} {} · {} · verificação: {}",
                curto(&p.job),
                p.indice,
                p.especificacao.tipo().descricao(),
                p.especificacao.resumo(),
                p.especificacao.tipo().categoria().nome(),
                metodo.nome()
            ),
        );
        tarefa.avancar(Estado::NaFila, agora_ms(), "").map_err(|e| e.to_string())?;
        self.marcar(numero, "TASK QUEUED", String::new());
        Ok(NaFila { tarefa, desafio: None, recusas: 0, origem: p.semente.to_vec(), job: Some((p.job, p.indice)) })
    }

    /// Execução e verificação, com a CPU inteira, no ritmo medido.
    fn segundos_estimados(&self, esp: &Especificacao) -> f64 {
        let ritmo = self.ritmo.lock().map_or(RITMO_INICIAL, |r| r.get(indice(esp.tipo())).copied().unwrap_or(RITMO_INICIAL));
        let operacoes = esp.operacoes_maximas().saturating_add(trabalho::operacoes_de_verificacao(esp));
        operacoes as f64 / ritmo.max(1.0)
    }

    /// O tamanho que cabe em [`SEGUNDOS_ALVO`] de cálculo neste ritmo, e na
    /// fatia de memória de uma linha.
    fn dimensionar(&self, tipo: TipoDeTrabalho) -> Result<Especificacao, String> {
        let ritmo = self.ritmo.lock().map_or(RITMO_INICIAL, |r| r.get(indice(tipo)).copied().unwrap_or(RITMO_INICIAL));
        let alvo = (ritmo * SEGUNDOS_ALVO).max(1.0);
        let linhas = u64::from(self.linhas.load(Ordering::Relaxed).max(1));
        let orcamento = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024) / linhas;
        let cabe = |esp: &Especificacao| esp.memoria_bytes() <= orcamento;
        let erro = |e: ErroDeTrabalho| e.to_string();
        let esp = match tipo {
            TipoDeTrabalho::Ia => {
                // lote de 32; os passos enchem o tempo-alvo
                let lote = 32u32;
                let por_passo = f64::from(lote) * 368.0 + 193.0;
                let mut passos = ((alvo / por_passo) as u32).clamp(16, ia::PASSOS_MAX);
                loop {
                    let esp = Especificacao::nova(tipo, lote, passos).map_err(erro)?;
                    if cabe(&esp) || passos <= 16 {
                        break esp;
                    }
                    passos = passos.saturating_mul(3) / 4;
                }
            }
            TipoDeTrabalho::Matriz => {
                let mut n = (alvo.cbrt() as u32).clamp(32, trabalho::MATRIZ_LADO_MAX);
                loop {
                    let esp = Especificacao::nova(tipo, n, 0).map_err(erro)?;
                    if cabe(&esp) || n <= 32 {
                        break esp;
                    }
                    n = n.saturating_mul(7) / 8;
                }
            }
            TipoDeTrabalho::Mochila => {
                // operações ≈ n · 25n
                let mut n = ((alvo / 25.0).sqrt() as u32).clamp(16, trabalho::MOCHILA_ITENS_MAX);
                loop {
                    let esp = Especificacao::nova(tipo, n, 0).map_err(erro)?;
                    if cabe(&esp) || n <= 16 {
                        break esp;
                    }
                    n = n.saturating_mul(7) / 8;
                }
            }
            TipoDeTrabalho::Difusao => {
                let mut g = 128u32;
                // sobe para a grade maior uma vez só; depois, só desce até caber
                let mut subiu = false;
                loop {
                    let celulas = f64::from(g) * f64::from(g);
                    let passos = ((alvo / celulas) as u32).clamp(1, trabalho::DIFUSAO_PASSOS_MAX);
                    if passos == trabalho::DIFUSAO_PASSOS_MAX && g < trabalho::DIFUSAO_GRADE_MAX && !subiu {
                        g = trabalho::DIFUSAO_GRADE_MAX;
                        subiu = true;
                        continue;
                    }
                    let esp = Especificacao::nova(tipo, g, passos).map_err(erro)?;
                    if cabe(&esp) || g <= 16 {
                        break esp;
                    }
                    g = g.saturating_mul(3) / 4;
                }
            }
            // os tipos científicos só rodam por JOB, com os parâmetros de quem pediu
            TipoDeTrabalho::Genetica | TipoDeTrabalho::Melhoramento | TipoDeTrabalho::Rotas | TipoDeTrabalho::Triagem => {
                return Err(format!("{} só roda por JOB, não no rodízio LAB", tipo.nome()));
            }
        };
        if cabe(&esp) {
            Ok(esp)
        } else {
            Err(format!("o teto de memória não comporta nem a menor tarefa de {}", tipo.descricao()))
        }
    }

    fn expirar_na_fila(&self) {
        let agora = agora_ms();
        let vencidas: Vec<NaFila> = match self.fila.lock() {
            Ok(mut f) => {
                let (vencidas, ficam): (Vec<_>, Vec<_>) = f.drain(..).partition(|t| t.tarefa.vencida(agora));
                f.extend(ficam);
                vencidas
            }
            Err(_) => return,
        };
        for mut item in vencidas {
            let _ = item.tarefa.avancar(Estado::Expirada, agora, "prazo vencido na fila");
            self.encerrar(&item, None, 0, "prazo vencido na fila");
        }
    }

    fn descartar_fila(&self, motivo: &str) {
        let itens: Vec<NaFila> = self.fila.lock().map(|mut f| f.drain(..).collect()).unwrap_or_default();
        for mut item in itens {
            let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), motivo);
            self.encerrar(&item, None, 0, motivo);
        }
    }

    // -----------------------------------------------------------------------
    // Worker
    // -----------------------------------------------------------------------

    fn linha(self: Arc<Self>, i: u32) {
        loop {
            if !self.ligado.load(Ordering::Relaxed) || i >= self.linhas.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(300));
                continue;
            }
            let item = self.fila.lock().ok().and_then(|mut f| f.pop_front());
            match item {
                Some(item) => self.processar(i, item),
                None => std::thread::sleep(Duration::from_millis(200)),
            }
        }
    }

    /// Reserva memória para uma tarefa, se couber no teto.
    fn reservar(&self, bytes: u64) -> bool {
        let teto = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
        self.reservada
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |r| r.checked_add(bytes).filter(|&t| t <= teto))
            .is_ok()
    }

    fn liberar(&self, bytes: u64) {
        let _ = self.reservada.fetch_update(Ordering::AcqRel, Ordering::Acquire, |r| Some(r.saturating_sub(bytes)));
    }

    /// Leva uma tarefa da fila até o fim do ciclo, ou até ser interrompida.
    #[allow(clippy::too_many_lines)]
    fn processar(&self, linha: u32, mut item: NaFila) {
        let numero = item.tarefa.numero();
        let esp = item.tarefa.especificacao;
        let agora = agora_ms();
        if item.tarefa.vencida(agora) {
            let _ = item.tarefa.avancar(Estado::Expirada, agora, "prazo vencido na fila");
            self.encerrar(&item, None, 0, "prazo vencido na fila");
            return;
        }

        // Gerenciador de recursos: a memória é reservada antes de atribuir, ou
        // a tarefa não roda.
        let memoria = esp.memoria_bytes();
        if !self.reservar(memoria) {
            let teto = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
            if memoria > teto {
                let motivo = format!("pede {:.1} MiB e o teto é {:.0} MiB", mib(memoria), mib(teto));
                let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), motivo.clone());
                self.encerrar(&item, None, 0, &motivo);
            } else {
                // cabe no teto, mas outra linha está usando: espera a vez na fila
                self.enfileirar(item, false);
                std::thread::sleep(Duration::from_millis(500));
            }
            return;
        }
        if item.tarefa.avancar(Estado::Atribuida, agora_ms(), format!("linha {linha}")).is_err() {
            self.liberar(memoria);
            return;
        }
        self.marcar(numero, "TASK ASSIGNED", format!("linha {linha} · worker {}", curto(&self.worker)));
        self.marcar(
            numero,
            "RESOURCE ALLOCATED",
            format!(
                "{:.1} MiB reservados · CPU até {}% · 1 núcleo · GPU não usada",
                mib(memoria),
                self.uso_cpu.load(Ordering::Relaxed)
            ),
        );

        let semente = match item.desafio {
            Some(_) => semente_do_desafio(&esp),
            None => item.tarefa.semente_para(&self.worker),
        };
        let entrada = hash_da_entrada(&esp, &semente).ok();
        let total = operacoes_da_instancia(&esp, &semente);
        let feitas = Arc::new(AtomicU64::new(0));
        if let Ok(mut a) = self.ativas.lock() {
            a.push(Ativa {
                linha,
                gpu: None,
                numero,
                id: item.tarefa.id,
                especificacao: esp,
                metodo: item.tarefa.metodo,
                desafio: item.desafio.is_some(),
                estado: Estado::Executando,
                feitas: Arc::clone(&feitas),
                total,
                operacoes: 0,
                inicio_ms: agora_ms(),
                memoria,
                entrada,
                job: item.job,
            });
        }
        let _ = item.tarefa.avancar(Estado::Executando, agora_ms(), "");
        self.marcar(numero, "WORK STARTED", format!("{} {}", esp.tipo().descricao(), esp.resumo()));
        if let (Some((job, i)), Some(a)) = (item.job, self.agendador()) {
            a.comecou(&job, i, linha, entrada);
        }

        let inicio_ms = agora_ms();
        let mut controle = Controle::novo(self, linha, item.tarefa.prazo_ms, &feitas);
        let execucao = trabalho::executar(&esp, &semente, &mut |ops| controle.passo(ops));
        let ms_calculo = u64::try_from(controle.trabalhando.as_millis()).unwrap_or(u64::MAX);
        let exec = match execucao {
            Ok(exec) => exec,
            Err(e) => {
                let (estado, motivo) = controle.desfecho(&e);
                let _ = item.tarefa.avancar(estado, agora_ms(), motivo.clone());
                self.sair(numero, memoria);
                self.somar_tempo(ms_calculo, 0);
                self.encerrar(&item, None, ms_calculo, &motivo);
                return;
            }
        };
        self.concluir(Conclusao { linha, item, esp, semente, entrada, exec, inicio_ms, ms_calculo, memoria, total, feitas, gpu: None });
    }

    /// Depois da execução, na CPU ou na GPU: assinar, conferir e liquidar.
    #[allow(clippy::too_many_lines)]
    fn concluir(&self, c: Conclusao) {
        let Conclusao { linha, mut item, esp, semente, entrada, mut exec, inicio_ms, ms_calculo, memoria, total, feitas, gpu } = c;
        let numero = item.tarefa.numero();
        let fim_ms = agora_ms();
        if ms_calculo > 50 {
            let ops_s = exec.operacoes as f64 / (ms_calculo as f64 / 1000.0);
            if gpu.is_some() {
                if let Ok(mut r) = self.ritmo_gpu.lock() {
                    *r = *r * 0.5 + ops_s * 0.5;
                }
            } else if let Ok(mut r) = self.ritmo.lock()
                && let Some(v) = r.get_mut(indice(esp.tipo()))
            {
                *v = *v * 0.5 + ops_s * 0.5;
            }
        }
        let onde = gpu.as_deref().map_or_else(|| "de cálculo".to_string(), |nome| format!("na GPU ({nome}), no relógio"));
        self.marcar(
            numero,
            "WORK COMPLETED",
            format!(
                "{} operações em {:.2} s {onde} ({:.1} M/s) · {:.2} s no relógio",
                exec.operacoes,
                ms_calculo as f64 / 1000.0,
                exec.operacoes as f64 / (ms_calculo.max(1) as f64 * 1000.0),
                (fim_ms.saturating_sub(inicio_ms)) as f64 / 1000.0
            ),
        );

        // Proof engine: o registro assinado pela chave do worker.
        let registro = RegistroDeProva {
            tarefa: item.tarefa.id,
            entrada: entrada.unwrap_or([0; HASH_LEN]),
            especificacao: esp,
            metodo: item.tarefa.metodo,
            resultado: hash_do_resultado(&exec.resultado),
            operacoes: exec.operacoes,
            worker: self.worker,
            inicio_ms,
            fim_ms,
        };
        let assinatura = registro.assinar(&self.segredo);
        let _ = item.tarefa.avancar(Estado::Enviada, agora_ms(), "");
        self.marcar(numero, "RESULT SUBMITTED", format!("RESULT_HASH {}…", curto(&registro.resultado)));

        // Verification engine.
        let _ = item.tarefa.avancar(Estado::Verificando, agora_ms(), "");
        let total_verificacao = match (item.desafio, esp.tipo()) {
            (Some(_), _) => 1,
            // a mochila refaz a programação dinâmica da instância, o mesmo total da execução
            (None, TipoDeTrabalho::Mochila) => total,
            (None, _) => trabalho::operacoes_de_verificacao(&esp),
        };
        self.mudar_fase(numero, Estado::Verificando, total_verificacao);
        self.marcar(
            numero,
            "VERIFICATION STARTED",
            format!(
                "{} · autoconferência na CPU desta máquina ({MODO}){}",
                item.tarefa.metodo.nome(),
                if gpu.is_some() { " · o resultado veio da GPU" } else { "" }
            ),
        );
        let comeco_verificacao = Instant::now();
        // a conferência é sempre na CPU, com outro algoritmo: é o que pega o erro da GPU
        let mut controle = Controle::novo(self, linha, item.tarefa.prazo_ms, &feitas);
        let julgamento: Result<Parecer, ErroDeTrabalho> = if !assinatura.is_some_and(|a| registro.assinatura_confere(&a)) {
            Ok(Parecer::Recusado(Recusa("o registro de prova não tem assinatura válida deste worker".into())))
        } else if let Some(d) = item.desafio.and_then(|q| DESAFIOS.get(q)) {
            feitas.store(1, Ordering::Relaxed);
            Ok(if hex(&registro.resultado) == d.esperado {
                Parecer::Aceito
            } else {
                Parecer::Recusado(Recusa(
                    "a resposta difere da que o gabarito conhece: esta máquina calculou errado (defeito de hardware ou programa alterado)".into(),
                ))
            })
        } else {
            trabalho::verificar_controlado(&esp, &semente, &exec.resultado, &mut |ops| controle.passo(ops)).map(|r| match r {
                Ok(()) => Parecer::Aceito,
                Err(recusa) => Parecer::Recusado(recusa),
            })
        };
        // cálculo puro, como o da execução: as pausas do limite de CPU ficam de fora
        let ms_verificacao = if item.desafio.is_some() {
            u64::try_from(comeco_verificacao.elapsed().as_millis()).unwrap_or(u64::MAX)
        } else {
            u64::try_from(controle.trabalhando.as_millis()).unwrap_or(u64::MAX)
        };
        self.sair(numero, memoria);
        self.somar_tempo(ms_calculo, ms_verificacao);
        let parecer = match julgamento {
            Ok(p) => p,
            Err(e) => {
                let (estado, motivo) = controle.desfecho(&e);
                let motivo = format!("{motivo}; o resultado ficou sem julgamento");
                let _ = item.tarefa.avancar(estado, agora_ms(), motivo.clone());
                self.encerrar(&item, Some((&registro, assinatura)), ms_calculo, &motivo);
                return;
            }
        };
        if let Ok(mut p) = self.placar.lock() {
            p.reputacao.registrar(&parecer);
        }

        match parecer {
            Parecer::Aceito => {
                let _ = item.tarefa.avancar(Estado::Verificada, agora_ms(), "");
                self.marcar(
                    numero,
                    "VERIFICATION PASSED",
                    format!("{} · {:.2} s de cálculo · autoconferência nesta máquina ({MODO})", item.tarefa.metodo.nome(), ms_verificacao as f64 / 1000.0),
                );
                if let Ok(mut p) = self.placar.lock() {
                    p.score.somar(exec.operacoes, true);
                    p.liquidadas = p.liquidadas.saturating_add(1);
                    if let Some(c) = p.por_tipo.get_mut(indice(esp.tipo())) {
                        *c = c.saturating_add(1);
                    }
                    if item.desafio.is_some() {
                        p.desafios_certos = p.desafios_certos.saturating_add(1);
                    }
                }
                let pontos = WorkScore { operacoes_verificadas: exec.operacoes, operacoes_sem_credito: 0 }.texto();
                let _ = item.tarefa.avancar(Estado::Liquidada, agora_ms(), format!("+{pontos} de Work Score"));
                self.marcar(numero, "SETTLEMENT COMPLETED", format!("+{pontos} de Work Score · medida de contribuição, sem valor em HYX"));
                if esp.tipo() == TipoDeTrabalho::Ia && item.desafio.is_none() && item.job.is_none() {
                    self.guardar_modelo(numero, &exec);
                }
                self.encerrar(&item, Some((&registro, assinatura)), ms_calculo, "");
                if let (Some((job, indice)), Some(a)) = (item.job, self.agendador()) {
                    a.terminou(DesfechoDeUnidade {
                        job,
                        indice,
                        tarefa: item.tarefa.id,
                        estado: Estado::Liquidada,
                        resultado: Some(std::mem::take(&mut exec.resultado)),
                        registro: Some(registro.clone()),
                        operacoes_verificacao: total_verificacao,
                        ms_calculo,
                        ms_verificacao,
                        memoria,
                        gpu: gpu.clone(),
                        nota: String::new(),
                        abandonada: false,
                    });
                }
            }
            Parecer::Recusado(recusa) => {
                let _ = item.tarefa.avancar(Estado::Recusada, agora_ms(), recusa.0.clone());
                self.marcar(numero, "VERIFICATION FAILED", recusa.0.clone());
                let segunda = item.recusas > 0;
                if let Ok(mut p) = self.placar.lock() {
                    p.score.somar(exec.operacoes, false);
                    p.recusadas = p.recusadas.saturating_add(1);
                    // o desafio conta uma vez, pelo desfecho: errado é errar as duas vezes
                    if segunda && item.desafio.is_some() {
                        p.desafios_errados = p.desafios_errados.saturating_add(1);
                    }
                }
                (self.aviso)("ultrax", format!("tarefa #{numero} recusada na conferência: {recusa}"));
                self.encerrar(&item, Some((&registro, assinatura)), ms_calculo, &recusa.0);
                if let (Some((job, indice)), Some(a)) = (item.job, self.agendador()) {
                    a.terminou(DesfechoDeUnidade {
                        job,
                        indice,
                        tarefa: item.tarefa.id,
                        estado: Estado::Recusada,
                        resultado: None,
                        registro: Some(registro.clone()),
                        operacoes_verificacao: total_verificacao,
                        ms_calculo,
                        ms_verificacao,
                        memoria,
                        gpu: gpu.clone(),
                        nota: recusa.0.clone(),
                        abandonada: false,
                    });
                }
                item.recusas = item.recusas.saturating_add(1);
                if segunda {
                    let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), "recusada duas vezes");
                    self.encerrar(&item, None, 0, "recusada duas vezes: erro que se repete");
                } else if item.tarefa.avancar(Estado::NaFila, agora_ms(), "repetida para separar defeito passageiro de erro sistemático").is_ok() {
                    // de novo, uma vez: erro que some era passageiro; erro que volta é sistemático
                    self.marcar(numero, "TASK QUEUED", "repetida uma vez depois da recusa".into());
                    self.enfileirar(item, true);
                }
            }
            // O worker LAB só julga por conferência; maioria e repetição não acontecem aqui.
            Parecer::Divergente | Parecer::Repetida => {}
        }
    }

    /// Um treino verificado: vira o último treino, e o melhor se errar menos.
    fn guardar_modelo(&self, numero: u32, exec: &trabalho::Execucao) {
        let Some((pesos, erro)) = ia::decodificar(&exec.resultado) else { return };
        let texto = match self.modelo.lock() {
            Ok(mut m) => {
                m.treinos = m.treinos.saturating_add(1);
                m.ultima_curva = resumir_curva(&exec.curva, 96);
                m.ultimo_erro = Some(erro);
                if m.melhor.as_ref().is_none_or(|(_, e)| erro < *e) {
                    m.melhor = Some((pesos, erro));
                    m.tarefa = numero;
                    let base = ia::Base::embutida();
                    self.marcar(
                        numero,
                        "MODEL IMPROVED",
                        format!("novo melhor modelo: erro típico de {:.2} em log S", ia::rmse_em_logs(erro, base) as f64 / 1000.0),
                    );
                }
                m.texto()
            }
            Err(_) => return,
        };
        let _vez = self.escrita.lock();
        let arquivo = self.pasta.join("modelo.txt");
        let temporario = arquivo.with_extension("tmp");
        if std::fs::write(&temporario, texto).is_ok() {
            let _ = std::fs::rename(&temporario, &arquivo);
        }
    }

    /// O painel da IA: o melhor modelo, o último treino e uma molécula de
    /// validação com a solubilidade medida e a prevista pelo melhor modelo.
    /// A molécula muda a cada 8 segundos.
    fn json_ia(&self) -> String {
        let base = ia::Base::embutida();
        let Ok(m) = self.modelo.lock() else { return "null".into() };
        let (_, validacao) = base.divisao();
        let vez = usize::try_from(agora_ms() / 8000).unwrap_or(0);
        let amostra = validacao.get(vez % validacao.len().max(1)).and_then(|&k| base.moleculas.get(k));
        let mut j = String::with_capacity(2048);
        let _ = write!(
            j,
            "{{\"moleculas\":{},\"validacao\":{},\"treinos\":{},\"tarefa\":{},\"melhor_erro\":{},\"melhor_rmse_mili\":{},\
             \"ultimo_rmse_mili\":{},\"ultima_curva\":[{}],\"desvio_mili\":{},\"descritores\":[{}],\"amostra\":",
            base.moleculas.len(),
            validacao.len(),
            m.treinos,
            m.tarefa,
            m.melhor.as_ref().map_or("null".into(), |(_, e)| e.to_string()),
            m.melhor.as_ref().map_or("null".into(), |(_, e)| ia::rmse_em_logs(*e, base).to_string()),
            m.ultimo_erro.map_or("null".into(), |e| ia::rmse_em_logs(e, base).to_string()),
            m.ultima_curva.iter().map(|&c| ia::rmse_em_logs(c, base).to_string()).collect::<Vec<_>>().join(","),
            base.desvio_mili,
            ia::DESCRITORES.iter().map(|d| texto_json(d)).collect::<Vec<_>>().join(","),
        );
        match amostra {
            Some(mol) => {
                let previsto = m.melhor.as_ref().map(|(p, _)| base.logs_de(ia::prever(p, &mol.x).0));
                let _ = write!(
                    j,
                    "{{\"id\":{},\"nome\":{},\"formula\":{},\"smiles\":{},\"massa_mili\":{},\"medido_mili\":{},\"previsto_mili\":{}}}}}",
                    texto_json(&mol.id),
                    texto_json(&mol.nome),
                    texto_json(&mol.formula),
                    texto_json(&mol.smiles),
                    mol.massa_mili,
                    mol.logs_mili,
                    previsto.map_or("null".into(), |v| v.to_string()),
                );
            }
            None => j.push_str("null}"),
        }
        j
    }

    fn json_gpu(&self) -> String {
        format!(
            "{{\"ligada\":{},\"uso\":{},\"nome\":{},\"tarefas\":{},\"ritmo\":{:.0}}}",
            self.gpu_ligada.load(Ordering::Relaxed),
            self.gpu_uso.load(Ordering::Relaxed),
            texto_json(&self.gpu_nome.lock().map(|g| g.clone()).unwrap_or_default()),
            self.gpu.lock().map_or(0, |g| g.len()),
            self.ritmo_gpu.lock().map_or(0.0, |r| *r),
        )
    }

    fn somar_tempo(&self, ms_calculo: u64, ms_verificacao: u64) {
        if let Ok(mut p) = self.placar.lock() {
            p.ms_calculo = p.ms_calculo.saturating_add(ms_calculo);
            p.ms_verificacao = p.ms_verificacao.saturating_add(ms_verificacao);
        }
    }

    fn mudar_fase(&self, numero: u32, estado: Estado, total: u64) {
        if let Ok(mut a) = self.ativas.lock()
            && let Some(x) = a.iter_mut().find(|x| x.numero == numero)
        {
            x.operacoes = x.feitas.swap(0, Ordering::Relaxed);
            x.estado = estado;
            x.total = total.max(1);
        }
    }

    fn sair(&self, numero: u32, memoria: u64) {
        if let Ok(mut a) = self.ativas.lock() {
            a.retain(|x| x.numero != numero);
        }
        self.liberar(memoria);
    }

    /// Fecha um trecho do ciclo: assina o veredito, grava no histórico, conta
    /// os encerramentos sem julgamento, grava o placar e guarda a lembrança
    /// para o painel.
    fn encerrar(&self, item: &NaFila, registro: Option<(&RegistroDeProva, Option<[u8; 64]>)>, ms_calculo: u64, nota: &str) {
        let t = &item.tarefa;
        match t.estado {
            // recusada duas vezes é erro que se repete, não parada de quem manda
            Estado::Cancelada if item.recusas >= 2 => {
                if let Ok(mut p) = self.placar.lock() {
                    p.abandonadas = p.abandonadas.saturating_add(1);
                }
                self.marcar(t.numero(), "TASK ABANDONED", nota.to_string());
            }
            Estado::Cancelada => {
                if let Ok(mut p) = self.placar.lock() {
                    p.canceladas = p.canceladas.saturating_add(1);
                }
                self.marcar(t.numero(), "TASK CANCELLED", nota.to_string());
            }
            Estado::Expirada => {
                if let Ok(mut p) = self.placar.lock() {
                    p.expiradas = p.expiradas.saturating_add(1);
                }
                self.marcar(t.numero(), "TASK EXPIRED", nota.to_string());
            }
            _ => {}
        }
        // o veredito vale para este registro: trocar o estado no arquivo depois quebra a assinatura
        let veredito = registro.and_then(|(r, _)| r.assinar_veredito(&self.segredo, t.estado.nome()));
        let linha = linha_do_historico(item, registro, veredito, ms_calculo, nota);
        self.gravar_no_arquivo("historico.txt", &linha, HISTORICO_MAX_BYTES);
        let lembranca = Lembranca {
            numero: t.numero(),
            tipo: t.especificacao.tipo(),
            resumo: t.especificacao.resumo(),
            metodo: t.metodo,
            desafio: item.desafio.is_some(),
            estado: t.estado,
            operacoes: registro.map_or(0, |(r, _)| r.operacoes),
            ms_calculo,
            fim_ms: agora_ms(),
            resultado: registro.map(|(r, _)| r.resultado),
            nota: nota.to_string(),
        };
        if let Ok(mut h) = self.historico.lock() {
            h.push_front(lembranca);
            h.truncate(HISTORICO_NA_MEMORIA);
            // soma com o histórico travado: quem lê os dois juntos nunca vê um sem o outro
            self.encerrados.fetch_add(1, Ordering::Relaxed);
        }
        if t.estado.e_final() {
            self.finais.fetch_add(1, Ordering::Relaxed);
        }
        self.gravar_placar();
        // liquidada e recusada avisam em concluir(), com o resultado na mão
        if matches!(t.estado, Estado::Cancelada | Estado::Expirada)
            && let (Some((job, indice)), Some(a)) = (item.job, self.agendador())
        {
            a.terminou(DesfechoDeUnidade {
                job,
                indice,
                tarefa: t.id,
                estado: t.estado,
                resultado: None,
                registro: registro.map(|(r, _)| r.clone()),
                operacoes_verificacao: 0,
                ms_calculo,
                ms_verificacao: 0,
                memoria: 0,
                gpu: None,
                nota: nota.to_string(),
                abandonada: item.recusas >= 2,
            });
        }
    }

    // -----------------------------------------------------------------------
    // GPU: a página faz a conta, a CPU confere
    // -----------------------------------------------------------------------

    /// Lado da matriz que cabe em [`SEGUNDOS_ALVO`] no ritmo medido da GPU.
    fn dimensionar_gpu(&self) -> Result<Especificacao, String> {
        let ritmo = self.ritmo_gpu.lock().map_or(RITMO_INICIAL_GPU, |r| *r);
        let teto = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
        let mut n = ((ritmo * SEGUNDOS_ALVO).cbrt() as u32).clamp(64, trabalho::MATRIZ_LADO_MAX);
        loop {
            let esp = Especificacao::nova(TipoDeTrabalho::Matriz, n, 0).map_err(|e| e.to_string())?;
            if esp.memoria_bytes() <= teto || n <= 64 {
                return Ok(esp);
            }
            n = n.saturating_mul(7) / 8;
        }
    }

    /// A página pede uma tarefa para a GPU dela. Devolve `(número, lado)`.
    pub fn gpu_pegar(&self, nome: &str) -> Result<(u32, u32), String> {
        if !self.ligado.load(Ordering::Relaxed) || !self.gpu_ligada.load(Ordering::Relaxed) {
            return Err("a GPU está desligada no ULTRAX".into());
        }
        let nome: String = nome.chars().filter(|c| !c.is_control()).take(120).collect();
        let nome = if nome.trim().is_empty() { "GPU".to_string() } else { nome };
        if let Ok(mut g) = self.gpu_nome.lock() {
            g.clone_from(&nome);
        }
        if self.gpu.lock().map_or(usize::MAX, |g| g.len()) >= GPU_TAREFAS_MAX {
            return Err("a GPU já tem tarefa em curso".into());
        }
        let esp = self.dimensionar_gpu()?;
        let agora = agora_ms();
        let mut origem = [0u8; 32];
        hyurax_net::entropia::preencher(&mut origem)?;
        let estimado = esp.operacoes_maximas() as f64 / self.ritmo_gpu.lock().map_or(RITMO_INICIAL_GPU, |r| *r).max(1.0);
        let prazo = agora.saturating_add(PRAZO_MINIMO_MS.max((estimado / USO_MINIMO * PRAZO_FOLGA * 1000.0) as u64));
        let mut tarefa = Tarefa::nova(esp, MetodoDeVerificacao::Freivalds, Instancia::PorWorker, &origem, 100, agora, prazo)
            .map_err(|e| e.to_string())?;
        let numero = tarefa.numero();
        self.marcar(
            numero,
            "TASK CREATED",
            format!("{} {} · {} · na GPU · verificação: {} na CPU · {SELO}", esp.tipo().descricao(), esp.resumo(), esp.tipo().categoria().nome(), tarefa.metodo.nome()),
        );
        let memoria = esp.memoria_bytes();
        if !self.reservar(memoria) {
            return Err("sem memória livre no teto do ULTRAX agora".into());
        }
        let _ = tarefa.avancar(Estado::NaFila, agora_ms(), "");
        let _ = tarefa.avancar(Estado::Atribuida, agora_ms(), format!("GPU: {nome}"));
        self.marcar(numero, "TASK ASSIGNED", format!("GPU: {nome} · worker {}", curto(&self.worker)));
        self.marcar(
            numero,
            "RESOURCE ALLOCATED",
            format!("{:.1} MiB reservados · GPU até {}% · WebGL2", mib(memoria), self.gpu_uso.load(Ordering::Relaxed)),
        );
        let _ = tarefa.avancar(Estado::Executando, agora_ms(), "");
        self.marcar(numero, "WORK STARTED", format!("{} {} na GPU", esp.tipo().descricao(), esp.resumo()));
        let semente = tarefa.semente_para(&self.worker);
        let entrada = hash_da_entrada(&esp, &semente).ok();
        let feitas = Arc::new(AtomicU64::new(0));
        if let Ok(mut a) = self.ativas.lock() {
            a.push(Ativa {
                linha: 0,
                gpu: Some(nome.clone()),
                numero,
                id: tarefa.id,
                especificacao: esp,
                metodo: tarefa.metodo,
                desafio: false,
                estado: Estado::Executando,
                feitas: Arc::clone(&feitas),
                total: esp.operacoes_maximas(),
                operacoes: 0,
                inicio_ms: agora_ms(),
                memoria,
                entrada,
                job: None,
            });
        }
        let item = NaFila { tarefa, desafio: None, recusas: 0, origem: origem.to_vec(), job: None };
        if let Ok(mut g) = self.gpu.lock() {
            g.push(NaGpu { item, semente, entrada, memoria, inicio_ms: agora_ms(), feitas, nome });
        }
        Ok((numero, esp.tamanho()))
    }

    /// As matrizes `A` e `B` de uma tarefa da GPU, em u32 little-endian, nessa ordem.
    pub fn gpu_entrada(&self, numero: u32) -> Option<Vec<u8>> {
        let (semente, n) = self
            .gpu
            .lock()
            .ok()?
            .iter()
            .find(|g| g.item.tarefa.numero() == numero)
            .map(|g| (g.semente, g.item.tarefa.especificacao.tamanho()))?;
        let (a, b) = trabalho::matrizes(n, &semente).ok()?;
        Some(a.iter().chain(&b).flat_map(|v| v.to_le_bytes()).collect())
    }

    /// Quantas linhas de `C` a GPU já calculou.
    pub fn gpu_progresso(&self, numero: u32, linhas: u64) {
        if let Ok(g) = self.gpu.lock()
            && let Some(x) = g.iter().find(|g| g.item.tarefa.numero() == numero)
        {
            let n = u64::from(x.item.tarefa.especificacao.tamanho());
            x.feitas.store(linhas.min(n).saturating_mul(n).saturating_mul(n), Ordering::Relaxed);
        }
    }

    /// O resultado da GPU chega: `C` em u32 little-endian. A CPU confere por
    /// Freivalds antes de creditar qualquer coisa.
    pub fn gpu_resultado(&self, numero: u32, bytes: &[u8]) -> Result<(), String> {
        let x = self
            .gpu
            .lock()
            .ok()
            .and_then(|mut g| g.iter().position(|x| x.item.tarefa.numero() == numero).map(|k| g.remove(k)))
            .ok_or("tarefa de GPU desconhecida ou já encerrada")?;
        let esp = x.item.tarefa.especificacao;
        // u32 da GPU para o formato do resultado (i64 little-endian); tamanho
        // errado segue assim mesmo, e a conferência recusa com o motivo
        let resultado: Vec<u8> = bytes.as_chunks::<4>().0.iter().flat_map(|c| i64::from(u32::from_le_bytes(*c)).to_le_bytes()).collect();
        let ms = agora_ms().saturating_sub(x.inicio_ms);
        let exec = trabalho::Execucao { resultado, operacoes: esp.operacoes_fixas().unwrap_or(0), curva: Vec::new() };
        let total = exec.operacoes;
        self.concluir(Conclusao {
            linha: 0,
            item: x.item,
            esp,
            semente: x.semente,
            entrada: x.entrada,
            exec,
            inicio_ms: x.inicio_ms,
            ms_calculo: ms,
            memoria: x.memoria,
            total,
            feitas: x.feitas,
            gpu: Some(x.nome),
        });
        Ok(())
    }

    /// A página desistiu da tarefa (parou, fechou a aba, deu erro no WebGL).
    pub fn gpu_cancelar(&self, numero: u32, motivo: &str) {
        let achada = self
            .gpu
            .lock()
            .ok()
            .and_then(|mut g| g.iter().position(|x| x.item.tarefa.numero() == numero).map(|k| g.remove(k)));
        if let Some(x) = achada {
            self.fechar_gpu(x, Estado::Cancelada, motivo);
        }
    }

    fn cancelar_gpu(&self, motivo: &str) {
        let todas: Vec<NaGpu> = self.gpu.lock().map(|mut g| g.drain(..).collect()).unwrap_or_default();
        for x in todas {
            self.fechar_gpu(x, Estado::Cancelada, motivo);
        }
    }

    fn expirar_na_gpu(&self) {
        let agora = agora_ms();
        let vencidas: Vec<NaGpu> = match self.gpu.lock() {
            Ok(mut g) => {
                let (vencidas, ficam): (Vec<_>, Vec<_>) = g.drain(..).partition(|x| x.item.tarefa.vencida(agora));
                g.extend(ficam);
                vencidas
            }
            Err(_) => return,
        };
        for x in vencidas {
            self.fechar_gpu(x, Estado::Expirada, "prazo vencido: a janela parou de mandar o resultado da GPU");
        }
    }

    fn fechar_gpu(&self, mut x: NaGpu, estado: Estado, motivo: &str) {
        let numero = x.item.tarefa.numero();
        let _ = x.item.tarefa.avancar(estado, agora_ms(), motivo);
        self.sair(numero, x.memoria);
        let ms = agora_ms().saturating_sub(x.inicio_ms);
        self.encerrar(&x.item, None, ms, motivo);
    }

    // -----------------------------------------------------------------------
    // Disco e telemetria
    // -----------------------------------------------------------------------

    fn gravar_placar(&self) {
        let texto = self.placar().texto();
        let arquivo = self.pasta.join("placar.txt");
        let temporario = arquivo.with_extension("tmp");
        if std::fs::write(&temporario, texto).is_ok() {
            let _ = std::fs::rename(&temporario, &arquivo);
        }
    }

    /// Acrescenta uma linha, de uma escrita só; passando do tamanho, o arquivo
    /// atual vira `.1` (e o `.1` anterior se perde: o histórico guarda as
    /// tarefas mais recentes, não todas desde sempre).
    fn gravar_no_arquivo(&self, nome: &str, linha: &str, maximo: u64) {
        let _vez = self.escrita.lock();
        let arquivo = self.pasta.join(nome);
        if std::fs::metadata(&arquivo).is_ok_and(|m| m.len() > maximo) {
            let _ = std::fs::rename(&arquivo, self.pasta.join(format!("{nome}.1")));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&arquivo) {
            let mut bytes = linha.as_bytes().to_vec();
            bytes.push(b'\n');
            let _ = f.write_all(&bytes);
        }
    }

    fn carregar_historico_recente(&self) {
        let Ok(texto) = std::fs::read_to_string(self.pasta.join("historico.txt")) else { return };
        let mut recentes = VecDeque::new();
        for linha in texto.lines().rev().take(HISTORICO_NA_MEMORIA) {
            if let Some(r) = Registro::ler(linha) {
                recentes.push_back(Lembranca {
                    numero: r.numero(),
                    tipo: r.especificacao.tipo(),
                    resumo: r.especificacao.resumo(),
                    metodo: r.metodo,
                    desafio: r.desafio,
                    estado: r.estado,
                    operacoes: r.operacoes,
                    ms_calculo: r.ms_calculo,
                    fim_ms: r.fim_ms.unwrap_or(r.criada_ms),
                    resultado: r.resultado,
                    nota: r.nota.clone(),
                });
            }
        }
        if let Ok(mut h) = self.historico.lock() {
            *h = recentes;
        }
    }

    fn marcar(&self, tarefa: u32, evento: &'static str, detalhe: String) {
        let ms = agora_ms();
        if self.debug.load(Ordering::Relaxed) {
            let linha = format!("{ms} {MODO} #{tarefa:08} {evento} {detalhe}");
            self.gravar_no_arquivo("telemetria.log", &linha, TELEMETRIA_MAX_BYTES);
        }
        if let Ok(mut t) = self.telemetria.lock() {
            t.push_front(Marca { ms, tarefa, evento, detalhe });
            t.truncate(TELEMETRIA_NA_MEMORIA);
        }
    }

    // -----------------------------------------------------------------------
    // Estado para o painel
    // -----------------------------------------------------------------------

    /// O estado inteiro em JSON, para `/api/estado`.
    #[allow(clippy::too_many_lines)]
    pub fn json(&self) -> String {
        let mut j = String::with_capacity(8 * 1024);
        let p = self.placar();
        let fila = self.fila.lock().map_or(0, |f| f.len());
        let _ = write!(
            j,
            "{{\"modo\":\"{MODO}\",\"selo\":\"{SELO}\",\"ligado\":{},\"linhas\":{},\"nucleos\":{},\"uso_cpu\":{},\
             \"memoria_mib\":{},\"reservada_mib\":{:.1},\"gpu\":{},\"debug\":{},\"worker\":\"{}\",\"fila\":{fila},\
             \"desafio_a_cada\":{DESAFIO_A_CADA},\"verificador\":\"esta máquina (autoconferência LAB)\",",
            self.ligado.load(Ordering::Relaxed),
            self.linhas.load(Ordering::Relaxed),
            self.nucleos,
            self.uso_cpu.load(Ordering::Relaxed),
            self.memoria_mib.load(Ordering::Relaxed),
            mib(self.reservada.load(Ordering::Relaxed)),
            self.json_gpu(),
            self.debug.load(Ordering::Relaxed),
            hex(&self.worker),
        );
        j.push_str("\"ativas\":[");
        if let Ok(a) = self.ativas.lock() {
            for (k, x) in a.iter().enumerate() {
                if k > 0 {
                    j.push(',');
                }
                let feitas = x.feitas.load(Ordering::Relaxed).min(x.total);
                let _ = write!(
                    j,
                    "{{\"linha\":{},\"dispositivo\":{},\"numero\":{},\"id\":\"{}\",\"categoria\":\"{}\",\"tipo\":\"{}\",\"descricao\":{},\
                     \"tamanho\":{},\"passos\":{},\"resumo\":{},\"metodo\":{},\"desafio\":{},\"estado\":\"{}\",\"feitas\":{feitas},\"total\":{},\
                     \"operacoes\":{},\"inicio\":{},\"memoria_mib\":{:.1},\"entrada\":\"{}\",\"job\":{},\"indice\":{}}}",
                    x.linha,
                    texto_json(x.gpu.as_deref().map_or("CPU", |_| "GPU")),
                    x.numero,
                    hex(&x.id),
                    x.especificacao.tipo().categoria().nome(),
                    x.especificacao.tipo().nome(),
                    texto_json(x.especificacao.tipo().descricao()),
                    x.especificacao.tamanho(),
                    x.especificacao.passos(),
                    texto_json(&x.especificacao.resumo()),
                    texto_json(x.metodo.nome()),
                    x.desafio,
                    x.estado.nome(),
                    x.total,
                    x.operacoes,
                    x.inicio_ms,
                    mib(x.memoria),
                    x.entrada.map(|e| hex(&e)).unwrap_or_default(),
                    x.job.map_or("null".to_string(), |(j, _)| format!("\"{}\"", hex(&j))),
                    x.job.map_or("null".to_string(), |(_, i)| i.to_string()),
                );
            }
        }
        let r = &p.reputacao;
        let _ = write!(
            j,
            "],\"placar\":{{\"liquidadas\":{},\"recusadas\":{},\"canceladas\":{},\"expiradas\":{},\
             \"desafios_certos\":{},\"desafios_errados\":{},\"work_score\":\"{}\",\"operacoes_verificadas\":{},\
             \"operacoes_sem_credito\":{},\"ms_calculo\":{},\"ms_verificacao\":{},\"segundos_ligado\":{},\
             \"matriz\":{},\"mochila\":{},\"difusao\":{},\"ia\":{},\"enviadas\":{},\"verificadas\":{},\"divergentes\":{},\
             \"disputadas\":{},\"nota\":{},\"taxa\":{},\"geradas\":{},\"abandonadas\":{}}},",
            p.liquidadas,
            p.recusadas,
            p.canceladas,
            p.expiradas,
            p.desafios_certos,
            p.desafios_errados,
            p.score.texto(),
            p.score.operacoes_verificadas,
            p.score.operacoes_sem_credito,
            p.ms_calculo,
            p.ms_verificacao,
            p.segundos_ligado,
            p.por_tipo[0],
            p.por_tipo[1],
            p.por_tipo[2],
            p.por_tipo[3],
            r.enviadas,
            r.verificadas,
            r.divergentes,
            r.disputadas,
            r.nota(),
            r.taxa_de_acerto().map_or("null".to_string(), |t| t.to_string()),
            p.geradas,
            p.abandonadas,
        );
        let _ = write!(j, "\"ia\":{},", self.json_ia());
        j.push_str("\"historico\":[");
        if let Ok(h) = self.historico.lock() {
            for (k, x) in h.iter().enumerate() {
                if k > 0 {
                    j.push(',');
                }
                let _ = write!(
                    j,
                    "{{\"numero\":{},\"tipo\":\"{}\",\"categoria\":\"{}\",\"resumo\":{},\"metodo\":{},\"desafio\":{},\
                     \"estado\":\"{}\",\"operacoes\":{},\"ms_calculo\":{},\"fim\":{},\"resultado\":\"{}\",\"nota\":{}}}",
                    x.numero,
                    x.tipo.nome(),
                    x.tipo.categoria().nome(),
                    texto_json(&x.resumo),
                    texto_json(x.metodo.nome()),
                    x.desafio,
                    x.estado.nome(),
                    x.operacoes,
                    x.ms_calculo,
                    x.fim_ms,
                    x.resultado.map(|r| hex(&r)).unwrap_or_default(),
                    texto_json(&x.nota),
                );
            }
        }
        j.push_str("],\"telemetria\":[");
        if let Ok(t) = self.telemetria.lock() {
            for (k, m) in t.iter().enumerate() {
                if k > 0 {
                    j.push(',');
                }
                let _ = write!(
                    j,
                    "{{\"ms\":{},\"tarefa\":{},\"evento\":\"{}\",\"detalhe\":{}}}",
                    m.ms,
                    m.tarefa,
                    m.evento,
                    texto_json(&m.detalhe)
                );
            }
        }
        j.push_str("]}");
        j
    }
}

fn curto(bytes: &[u8]) -> String {
    hex(bytes.get(..8).unwrap_or(bytes))
}

/// Limite de CPU, prazo e botão de parar, aplicados a cada pedaço de trabalho.
struct Controle<'a> {
    u: &'a Ultrax,
    linha: u32,
    prazo_ms: u64,
    feitas: &'a AtomicU64,
    marco: Instant,
    /// Tempo de cálculo puro, sem as pausas.
    trabalhando: Duration,
    /// Pausa devida e ainda não dormida.
    divida: Duration,
    motivo: Option<&'static str>,
}

impl<'a> Controle<'a> {
    fn novo(u: &'a Ultrax, linha: u32, prazo_ms: u64, feitas: &'a AtomicU64) -> Self {
        Self { u, linha, prazo_ms, feitas, marco: Instant::now(), trabalhando: Duration::ZERO, divida: Duration::ZERO, motivo: None }
    }

    /// Chamado a cada pedaço. Devolve `false` para interromper.
    fn passo(&mut self, ops: u64) -> bool {
        self.feitas.fetch_add(ops, Ordering::Relaxed);
        let gasto = self.marco.elapsed();
        self.trabalhando = self.trabalhando.saturating_add(gasto);
        if let Some(motivo) = self.parar() {
            self.motivo = Some(motivo);
            return false;
        }
        let uso = f64::from(self.u.uso_cpu.load(Ordering::Relaxed).clamp(10, 100));
        if uso < 100.0 {
            // com 25%, cada segundo de cálculo pede três de descanso
            self.divida = self.divida.saturating_add(gasto.mul_f64(100.0 / uso - 1.0));
            if self.divida >= PAUSA_MINIMA {
                let pedido = self.divida.min(PAUSA_MAXIMA);
                let comeco = Instant::now();
                // dorme em fatias, para o botão de parar responder logo
                while comeco.elapsed() < pedido {
                    if let Some(motivo) = self.parar() {
                        self.motivo = Some(motivo);
                        return false;
                    }
                    std::thread::sleep((pedido.saturating_sub(comeco.elapsed())).min(Duration::from_millis(100)));
                }
                self.divida = self.divida.saturating_sub(comeco.elapsed());
            }
        }
        self.marco = Instant::now();
        true
    }

    fn parar(&self) -> Option<&'static str> {
        if !self.u.ligado.load(Ordering::Relaxed) {
            Some("o ULTRAX foi desligado")
        } else if self.linha >= self.u.linhas.load(Ordering::Relaxed) {
            Some("a linha foi desligada nos ajustes")
        } else if agora_ms() > self.prazo_ms {
            Some("prazo vencido")
        } else if self.u.reservada.load(Ordering::Relaxed)
            > u64::from(self.u.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024)
        {
            // baixaram o teto com tarefas rodando: elas param, e a memória volta
            Some("o teto de memória baixou")
        } else {
            None
        }
    }

    /// O estado e o motivo de uma interrupção.
    fn desfecho(&self, e: &ErroDeTrabalho) -> (Estado, String) {
        match (e, self.motivo) {
            (ErroDeTrabalho::Cancelado, Some("prazo vencido")) => (Estado::Expirada, "prazo vencido".into()),
            (ErroDeTrabalho::Cancelado, Some(motivo)) => (Estado::Cancelada, motivo.into()),
            (outro, _) => (Estado::Cancelada, outro.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Histórico em texto: uma linha por trecho de ciclo encerrado
// ---------------------------------------------------------------------------

/// Texto sem espaço nem quebra, para caber numa linha `chave=valor`.
fn escapar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '%' => s.push_str("%25"),
            ' ' => s.push_str("%20"),
            '\n' => s.push_str("%0A"),
            '\r' => s.push_str("%0D"),
            '\t' => s.push_str("%09"),
            outro => s.push(outro),
        }
    }
    s
}

fn desescapar(texto: &str) -> String {
    texto.replace("%20", " ").replace("%0A", "\n").replace("%0D", "\r").replace("%09", "\t").replace("%25", "%")
}

fn linha_do_historico(
    item: &NaFila,
    registro: Option<(&RegistroDeProva, Option<[u8; 64]>)>,
    veredito: Option<[u8; 64]>,
    ms_calculo: u64,
    nota: &str,
) -> String {
    let t = &item.tarefa;
    let esp = t.especificacao;
    let mut l = String::with_capacity(1024);
    let _ = write!(
        l,
        "v=1 modo={MODO} selo={} numero={} id={} estado={} tipo={} tamanho={} passos={} metodo={} instancia={} desafio={} \
         origem={} prioridade={} criada={} prazo={}",
        escapar(SELO),
        t.numero(),
        hex(&t.id),
        t.estado.nome(),
        esp.tipo().codigo(),
        esp.tamanho(),
        esp.passos(),
        t.metodo.codigo(),
        t.instancia.codigo(),
        u8::from(item.desafio.is_some()),
        hex(&item.origem),
        t.prioridade,
        t.criada_ms,
        t.prazo_ms,
    );
    if !esp.parametros().is_empty() {
        let lista: Vec<String> = esp.parametros().iter().map(u32::to_string).collect();
        let _ = write!(l, " parametros={}", lista.join(","));
    }
    if let Some((job, indice)) = item.job {
        let _ = write!(l, " job={} indice={indice}", hex(&job));
    }
    if let Some((r, assinatura)) = registro {
        let _ = write!(
            l,
            " entrada={} resultado={} operacoes={} worker={} inicio={} fim={} assinatura={} veredito={}",
            hex(&r.entrada),
            hex(&r.resultado),
            r.operacoes,
            hex(&r.worker),
            r.inicio_ms,
            r.fim_ms,
            assinatura.map(|a| hex(&a)).unwrap_or_default(),
            veredito.map(|a| hex(&a)).unwrap_or_default(),
        );
    }
    let eventos: Vec<String> = t.eventos.iter().map(|e| format!("{}@{}", e.estado.nome(), e.instante_ms)).collect();
    let _ = write!(l, " ms_calculo={ms_calculo} eventos={} nota={}", eventos.join(","), escapar(nota));
    l
}

/// Uma linha do histórico, lida de volta.
#[derive(Clone, Debug)]
struct Registro {
    id: [u8; HASH_LEN],
    estado: Estado,
    especificacao: Especificacao,
    metodo: MetodoDeVerificacao,
    instancia: Instancia,
    desafio: bool,
    origem: Vec<u8>,
    prioridade: u8,
    criada_ms: u64,
    prazo_ms: u64,
    entrada: Option<[u8; HASH_LEN]>,
    resultado: Option<[u8; HASH_LEN]>,
    operacoes: u64,
    worker: Option<[u8; PUBKEY_LEN]>,
    inicio_ms: Option<u64>,
    fim_ms: Option<u64>,
    assinatura: Option<[u8; 64]>,
    veredito: Option<[u8; 64]>,
    ms_calculo: u64,
    nota: String,
    job: Option<[u8; HASH_LEN]>,
    indice: Option<u64>,
}

fn estado_de_nome(nome: &str) -> Option<Estado> {
    use Estado::*;
    [Criada, NaFila, Atribuida, Executando, Enviada, Verificando, Verificada, Recusada, Liquidada, Cancelada, Expirada]
        .into_iter()
        .find(|e| e.nome() == nome)
}

impl Registro {
    fn ler(linha: &str) -> Option<Self> {
        let campo = |nome: &str| {
            linha.split(' ').find_map(|par| par.split_once('=').filter(|(k, _)| *k == nome).map(|(_, v)| v))
        };
        let num = |nome: &str| campo(nome).and_then(|v| v.parse::<u64>().ok());
        if campo("v") != Some("1") {
            return None;
        }
        let tipo = TipoDeTrabalho::de_codigo(u8::try_from(num("tipo")?).ok()?)?;
        let parametros: Vec<u32> = match campo("parametros") {
            Some(lista) => lista.split(',').map(|v| v.parse::<u32>().ok()).collect::<Option<Vec<u32>>>()?,
            None => Vec::new(),
        };
        let especificacao = Especificacao::nova_com(
            tipo,
            u32::try_from(num("tamanho")?).ok()?,
            u32::try_from(num("passos")?).ok()?,
            &parametros,
        )
        .ok()?;
        Some(Self {
            id: de_hex(campo("id")?)?,
            estado: estado_de_nome(campo("estado")?)?,
            especificacao,
            metodo: MetodoDeVerificacao::de_codigo(u8::try_from(num("metodo")?).ok()?)?,
            instancia: Instancia::de_codigo(u8::try_from(num("instancia")?).ok()?)?,
            desafio: num("desafio")? == 1,
            origem: decodificar_hex(campo("origem")?)?,
            prioridade: u8::try_from(num("prioridade")?).ok()?,
            criada_ms: num("criada")?,
            prazo_ms: num("prazo")?,
            entrada: campo("entrada").and_then(de_hex),
            resultado: campo("resultado").and_then(de_hex),
            operacoes: num("operacoes").unwrap_or(0),
            worker: campo("worker").and_then(de_hex),
            inicio_ms: num("inicio"),
            fim_ms: num("fim"),
            assinatura: campo("assinatura").and_then(de_hex),
            veredito: campo("veredito").and_then(de_hex),
            ms_calculo: num("ms_calculo").unwrap_or(0),
            nota: campo("nota").map(desescapar).unwrap_or_default(),
            job: campo("job").and_then(de_hex),
            indice: num("indice"),
        })
    }

    fn numero(&self) -> u32 {
        u32::from_be_bytes(self.id.first_chunk::<4>().copied().unwrap_or_default()) % 100_000_000
    }

    fn prova(&self) -> Option<RegistroDeProva> {
        Some(RegistroDeProva {
            tarefa: self.id,
            entrada: self.entrada?,
            especificacao: self.especificacao,
            metodo: self.metodo,
            resultado: self.resultado?,
            operacoes: self.operacoes,
            worker: self.worker?,
            inicio_ms: self.inicio_ms?,
            fim_ms: self.fim_ms?,
        })
    }
}

// ---------------------------------------------------------------------------
// Auditoria
// ---------------------------------------------------------------------------

/// O que a auditoria achou.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Auditoria {
    /// Linhas lidas.
    pub registros: u64,
    /// Linhas que não se leem.
    pub ilegiveis: u64,
    /// `TASK_ID` refeito a partir dos campos e igual ao gravado.
    pub ids_certos: u64,
    /// Registros de prova com assinatura válida, do worker deste nó.
    pub assinaturas_certas: u64,
    /// Registros com prova (execução terminada).
    pub com_prova: u64,
    /// Vereditos assinados que conferem com o estado gravado.
    pub vereditos_certos: u64,
    /// Tarefas refeitas do zero.
    pub refeitas: u64,
    /// Refeitas com o mesmo resultado e as mesmas operações.
    pub refeitas_iguais: u64,
    /// O placar foi comparado com o histórico (só dá quando o histórico está inteiro).
    pub placar_conferido: bool,
    /// Descrição de cada problema achado.
    pub problemas: Vec<String>,
    /// Limites do que a auditoria prova, para ninguém ler mais do que ela diz.
    pub avisos: Vec<String>,
}

/// O worker deste nó, pela `no.chave` da pasta, sem criar arquivo nenhum.
fn worker_da_pasta(dados: &Path) -> Option<[u8; PUBKEY_LEN]> {
    let texto = std::fs::read_to_string(dados.join("no.chave")).ok()?;
    let segredo: [u8; SECRET_LEN] = texto.lines().find_map(|l| l.trim().strip_prefix("segredo=")).and_then(de_hex)?;
    Some(ed25519_public_key(&chave_do_worker(&segredo)))
}

/// As operações que a instância exige pelo modelo de custo do tipo.
fn operacoes_da_instancia(esp: &Especificacao, semente: &[u8]) -> u64 {
    match esp.tipo() {
        TipoDeTrabalho::Mochila => trabalho::instancia_mochila(esp.tamanho(), semente)
            .map_or(esp.operacoes_maximas(), |i| u64::from(esp.tamanho()).saturating_mul(u64::from(i.capacidade).saturating_add(1))),
        _ => esp.operacoes_maximas(),
    }
}

/// Confere tudo o que uma linha afirma, menos refazer a conta.
fn conferir_linha(r: &Registro, t: Option<&Tarefa>, esperado: Option<[u8; PUBKEY_LEN]>, a: &mut Auditoria) {
    let n = r.numero();
    if t.is_some_and(|t| t.id == r.id) {
        a.ids_certos = a.ids_certos.saturating_add(1);
    } else {
        a.problemas.push(format!("#{n:08}: o TASK_ID não confere com os campos (registro alterado)"));
    }
    // desafio só é desafio se for exatamente um dos casos do gabarito
    let desafio = desafio_de(&r.especificacao);
    if r.desafio {
        if desafio.is_none() || r.metodo != MetodoDeVerificacao::ResultadoEsperado || r.instancia != Instancia::Compartilhada {
            a.problemas.push(format!("#{n:08}: marcada como desafio, mas não é um desafio do gabarito"));
        }
    } else if r.instancia == Instancia::DeJob {
        // a semente de uma unidade de JOB é função do JOB e do índice, e só dele
        let presa = match (r.job, r.indice) {
            (Some(job), Some(i)) => r.origem == semente_da_unidade(&job, i).to_vec(),
            _ => false,
        };
        if !presa {
            a.problemas.push(format!("#{n:08}: unidade de JOB cuja semente não é a do JOB e índice gravados"));
        }
    } else if r.instancia != Instancia::PorWorker {
        a.problemas.push(format!("#{n:08}: tarefa normal com instância compartilhada (copiável)"));
    }
    let depois_da_entrega = matches!(
        r.estado,
        Estado::Enviada | Estado::Verificando | Estado::Verificada | Estado::Recusada | Estado::Liquidada
    );
    let Some(prova) = r.prova() else {
        if depois_da_entrega {
            a.problemas.push(format!("#{n:08}: estado {} sem registro de prova", r.estado.nome()));
        }
        return;
    };
    a.com_prova = a.com_prova.saturating_add(1);
    if esperado.is_some_and(|w| w != prova.worker) {
        a.problemas.push(format!("#{n:08}: assinado por outro worker, não pelo deste nó"));
    } else if r.assinatura.is_some_and(|s| prova.assinatura_confere(&s)) {
        a.assinaturas_certas = a.assinaturas_certas.saturating_add(1);
    } else {
        a.problemas.push(format!("#{n:08}: assinatura do registro de prova não confere"));
    }
    if r.veredito.is_some_and(|v| prova.veredito_confere(r.estado.nome(), &v)) {
        a.vereditos_certos = a.vereditos_certos.saturating_add(1);
    } else {
        a.problemas.push(format!("#{n:08}: o veredito assinado não confere com o estado {}", r.estado.nome()));
    }
    // as operações são a base do Work Score: precisam ser as do modelo de custo
    let semente = if r.desafio { semente_do_desafio(&r.especificacao) } else { t.map_or([0; HASH_LEN], |t| t.semente_para(&prova.worker)) };
    let devidas = operacoes_da_instancia(&r.especificacao, &semente);
    if r.operacoes != devidas {
        a.problemas.push(format!("#{n:08}: declara {} operações, e a tarefa tem {devidas}", r.operacoes));
    }
    if r.desafio && r.estado == Estado::Liquidada && desafio.is_some_and(|d| Some(d.esperado.to_string()) != r.resultado.map(|x| hex(&x))) {
        a.problemas.push(format!("#{n:08}: desafio liquidado com resposta diferente da do gabarito"));
    }
}

/// Lê o histórico, confere cada linha (id, worker, assinatura, veredito,
/// operações, desafio) e refaz do zero até `amostra` tarefas liquidadas,
/// escolhidas ao acaso. Com o histórico inteiro, compara também o placar.
pub fn auditar(dados: &Path, amostra: usize) -> Result<Auditoria, String> {
    let pasta = dados.join("ultrax");
    let anterior = std::fs::read_to_string(pasta.join("historico.txt.1")).ok();
    let mut texto = anterior.clone().unwrap_or_default();
    texto.push_str(&std::fs::read_to_string(pasta.join("historico.txt")).map_err(|e| format!("sem histórico em {}: {e}", pasta.display()))?);
    let mut a = Auditoria::default();
    let esperado = worker_da_pasta(dados);
    if esperado.is_none() {
        a.avisos.push("sem no.chave na pasta: não dá para saber se o worker dos registros é o deste nó".into());
    }
    a.avisos.push("quem tem o no.chave consegue assinar qualquer registro: a auditoria prova que o histórico é coerente, não que o dono é honesto".into());
    let mut liquidadas = Vec::new();
    let mut ids_liquidados: Vec<[u8; HASH_LEN]> = Vec::new();
    let (mut soma_liquidadas, mut soma_operacoes) = (0u64, 0u64);
    for linha in texto.lines().filter(|l| !l.trim().is_empty()) {
        a.registros = a.registros.saturating_add(1);
        let Some(r) = Registro::ler(linha) else {
            a.ilegiveis = a.ilegiveis.saturating_add(1);
            a.problemas.push(format!("linha {} ilegível", a.registros));
            continue;
        };
        let refeita = Tarefa::nova(r.especificacao, r.metodo, r.instancia, &r.origem, r.prioridade, r.criada_ms, r.prazo_ms).ok();
        conferir_linha(&r, refeita.as_ref(), esperado, &mut a);
        if r.estado == Estado::Liquidada {
            if ids_liquidados.contains(&r.id) {
                a.problemas.push(format!("#{:08}: liquidada mais de uma vez", r.numero()));
                continue;
            }
            ids_liquidados.push(r.id);
            soma_liquidadas = soma_liquidadas.saturating_add(1);
            soma_operacoes = soma_operacoes.saturating_add(r.operacoes);
            if let Some(t) = refeita {
                liquidadas.push((r, t));
            }
        }
    }
    // o placar não é assinado: só dá para conferir contra o histórico inteiro
    if anterior.is_none() {
        let placar = std::fs::read_to_string(pasta.join("placar.txt")).map(|t| Placar::de_texto(&t)).unwrap_or_default();
        a.placar_conferido = true;
        if placar.liquidadas != soma_liquidadas || placar.score.operacoes_verificadas != soma_operacoes {
            a.problemas.push(format!(
                "placar.txt diz {} liquidadas e {} operações verificadas; o histórico soma {soma_liquidadas} e {soma_operacoes}",
                placar.liquidadas, placar.score.operacoes_verificadas
            ));
        }
    } else {
        a.avisos.push("o histórico já recomeçou uma vez (historico.txt.1): o placar não pode ser comparado com ele".into());
    }
    // amostra ao acaso, sem repetir
    let mut escolhidas = Vec::new();
    while escolhidas.len() < amostra.min(liquidadas.len()) {
        let mut sorteio = [0u8; 8];
        hyurax_net::entropia::preencher(&mut sorteio)?;
        let k = usize::try_from(u64::from_be_bytes(sorteio) % liquidadas.len() as u64).unwrap_or(0);
        if !escolhidas.contains(&k) {
            escolhidas.push(k);
        }
    }
    for k in escolhidas {
        let Some((r, t)) = liquidadas.get(k) else { continue };
        let Some(worker) = r.worker else { continue };
        let semente = if r.desafio { semente_do_desafio(&r.especificacao) } else { t.semente_para(&worker) };
        a.refeitas = a.refeitas.saturating_add(1);
        match trabalho::executar(&r.especificacao, &semente, &mut |_| true) {
            Ok(exec) if Some(hash_do_resultado(&exec.resultado)) == r.resultado && exec.operacoes == r.operacoes => {
                a.refeitas_iguais = a.refeitas_iguais.saturating_add(1);
            }
            Ok(_) => a.problemas.push(format!("#{:08}: refeita do zero, deu outro resultado", r.numero())),
            Err(e) => a.problemas.push(format!("#{:08}: não consegui refazer: {e}", r.numero())),
        }
    }
    Ok(a)
}

// ---------------------------------------------------------------------------
// Terminal: `hyurax-no ultrax lab` e `hyurax-no ultrax auditar`
// ---------------------------------------------------------------------------

/// `hyurax-no ultrax ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no ultrax lab|auditar --pasta dados")?;
    let mut pasta = PathBuf::from("dados-hyurax");
    let mut tarefas: u64 = 0;
    let mut amostra: usize = 5;
    let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
    let mut partida = Partida { ligado: true, linhas: 1, uso_cpu: 100, memoria_mib: MEMORIA_PADRAO_MIB, debug: false, gpu: false, gpu_uso: 50 };
    let mut it = resto.iter();
    while let Some(nome) = it.next() {
        if nome == "--debug" {
            partida.debug = true;
            continue;
        }
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        let numero = || valor.parse::<u64>().map_err(|_| format!("{nome} precisa ser número"));
        match nome.as_str() {
            "--pasta" => pasta = PathBuf::from(valor),
            "--tarefas" => tarefas = numero()?,
            "--amostra" => amostra = usize::try_from(numero()?).unwrap_or(usize::MAX),
            "--linhas" => partida.linhas = u32::try_from(numero()?).unwrap_or(1),
            "--uso-cpu" => partida.uso_cpu = u32::try_from(numero()?).unwrap_or(100),
            "--memoria-mib" => partida.memoria_mib = u32::try_from(numero()?).unwrap_or(MEMORIA_PADRAO_MIB),
            _ => return Err(format!("opção desconhecida: {nome}")),
        }
    }
    match sub.as_str() {
        "auditar" => {
            let a = auditar(&pasta, amostra)?;
            println!("Auditoria do ULTRAX em {}", pasta.join("ultrax").display());
            println!("  registros:            {} ({} ilegíveis)", a.registros, a.ilegiveis);
            println!("  TASK_ID conferido:    {}", a.ids_certos);
            println!("  assinaturas válidas:  {} de {} com prova", a.assinaturas_certas, a.com_prova);
            println!("  vereditos assinados:  {} de {} com prova", a.vereditos_certos, a.com_prova);
            println!("  refeitas do zero:     {} iguais de {} sorteadas", a.refeitas_iguais, a.refeitas);
            println!("  placar conferido:     {}", if a.placar_conferido { "sim, contra o histórico inteiro" } else { "não" });
            for aviso in &a.avisos {
                println!("  aviso: {aviso}");
            }
            for p in &a.problemas {
                println!("  PROBLEMA: {p}");
            }
            if a.problemas.is_empty() { Ok(()) } else { Err(format!("{} problema(s) na auditoria", a.problemas.len())) }
        }
        "lab" => {
            std::fs::create_dir_all(&pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
            let identidade = crate::identidade_na_pasta(&pasta)?;
            let u = Ultrax::abrir(&pasta, identidade.segredo(), nucleos, &partida, Box::new(|tipo, texto| println!("  [{tipo}] {texto}")));
            println!(
                "ULTRAX, modo {MODO} ({SELO}): tarefas geradas e conferidas nesta máquina.\n  worker {}\n  {} linha(s), CPU até {}%, memória até {} MiB, GPU não usada",
                hex(&u.worker()),
                u.linhas.load(Ordering::Relaxed),
                u.uso_cpu.load(Ordering::Relaxed),
                u.memoria_mib.load(Ordering::Relaxed)
            );
            let antes = u.placar();
            u.iniciar();
            let mut impressos = 0u64;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let encerrados = u.encerrados.load(Ordering::Relaxed);
                if encerrados > impressos {
                    let novos = usize::try_from(encerrados.saturating_sub(impressos)).unwrap_or(usize::MAX);
                    let linhas: Vec<Lembranca> =
                        u.historico.lock().map(|h| h.iter().take(novos).cloned().collect()).unwrap_or_default();
                    for x in linhas.iter().rev() {
                        println!(
                            "  #{:08} {:<10} {:<30} {:<9} {:>13} operações {:>7.2} s{}",
                            x.numero,
                            x.tipo.nome(),
                            x.resumo,
                            x.estado.nome(),
                            x.operacoes,
                            x.ms_calculo as f64 / 1000.0,
                            if x.nota.is_empty() { String::new() } else { format!("  · {}", x.nota) }
                        );
                    }
                    impressos = encerrados;
                }
                if tarefas > 0 && u.finais.load(Ordering::Relaxed) >= tarefas {
                    // para de gerar, cancela a fila e espera as outras linhas fecharem o que têm
                    u.ligar(false);
                    let limite = Instant::now();
                    while u.ativas.lock().is_ok_and(|a| !a.is_empty()) && limite.elapsed() < Duration::from_secs(10) {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    u.gravar_placar();
                    let p = u.placar();
                    println!(
                        "Pronto: {} liquidada(s), {} recusada(s), {} cancelada(s). Work Score acumulado {} (medida local; não é HYX), reputação local {}/1000",
                        p.liquidadas.saturating_sub(antes.liquidadas),
                        p.recusadas.saturating_sub(antes.recusadas),
                        p.canceladas.saturating_sub(antes.canceladas),
                        p.score.texto(),
                        p.reputacao.nota()
                    );
                    return Ok(());
                }
            }
        }
        outro => Err(format!("subcomando desconhecido: {outro} (use lab ou auditar)")),
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hyurax-ultrax-teste-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn worker(nome: &str, partida: &Partida) -> (Arc<Ultrax>, PathBuf) {
        let p = pasta(nome);
        let u = Ultrax::abrir(&p, &[9; 32], 2, partida, Box::new(|_, _| {}));
        // ritmo baixo: o gerador faz tarefas pequenas, que o modo debug roda rápido
        *u.ritmo.lock().unwrap() = [1.0e5; 8];
        (u, p)
    }

    fn partida() -> Partida {
        Partida { ligado: true, linhas: 1, uso_cpu: 100, memoria_mib: MEMORIA_PADRAO_MIB, debug: false, gpu: false, gpu_uso: 50 }
    }

    #[test]
    fn desafios_batem_com_os_vetores() {
        let ler = |nome: &str| {
            let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", nome].iter().collect();
            std::fs::read_to_string(caminho).unwrap()
        };
        let texto = ler("utrax.json") + &ler("ia.json");
        for d in &DESAFIOS {
            let esp = Especificacao::nova(d.tipo, d.tamanho, d.passos).unwrap();
            let exec = trabalho::executar(&esp, &semente_do_desafio(&esp), &mut |_| true).unwrap();
            // o resultado calculado aqui está, em hexadecimal, dentro do arquivo do gabarito
            assert!(texto.contains(&hex(&exec.resultado)), "{} fora dos vetores", esp.resumo());
            assert_eq!(hex(&hash_do_resultado(&exec.resultado)), d.esperado, "{}", esp.resumo());
        }
    }

    #[test]
    fn uma_tarefa_passa_pelo_ciclo_inteiro() {
        let (u, p) = worker("ciclo", &partida());
        let item = u.gerar(agora_ms()).unwrap();
        let numero = item.tarefa.numero();
        u.processar(0, item);
        let placar = u.placar();
        assert_eq!(placar.liquidadas, 1);
        assert_eq!(placar.reputacao.verificadas, 1);
        assert!(placar.score.operacoes_verificadas > 0);
        let eventos: Vec<&str> = u.telemetria.lock().unwrap().iter().rev().filter(|m| m.tarefa == numero).map(|m| m.evento).collect();
        assert_eq!(
            eventos,
            [
                "TASK CREATED",
                "TASK QUEUED",
                "TASK ASSIGNED",
                "RESOURCE ALLOCATED",
                "WORK STARTED",
                "WORK COMPLETED",
                "RESULT SUBMITTED",
                "VERIFICATION STARTED",
                "VERIFICATION PASSED",
                "SETTLEMENT COMPLETED"
            ]
        );
        // o histórico foi gravado e a auditoria confere id, assinatura e refaz a conta
        let a = auditar(&p, 10).unwrap();
        assert_eq!((a.registros, a.ids_certos, a.assinaturas_certas, a.refeitas_iguais), (1, 1, 1, 1), "{a:?}");
        assert!(a.problemas.is_empty(), "{:?}", a.problemas);
        // e o placar sobrevive a reabrir
        let reaberto = Ultrax::abrir(&p, &[9; 32], 2, &partida(), Box::new(|_, _| {}));
        assert_eq!(reaberto.placar().liquidadas, 1);
        assert_eq!(reaberto.historico.lock().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn a_decima_tarefa_e_desafio_e_passa() {
        let (u, p) = worker("desafio", &partida());
        u.sequencia.store(DESAFIO_A_CADA - 1, Ordering::Relaxed);
        let item = u.gerar(agora_ms()).unwrap();
        assert!(item.desafio.is_some());
        assert_eq!(item.tarefa.metodo, MetodoDeVerificacao::ResultadoEsperado);
        u.processar(0, item);
        assert_eq!(u.placar().desafios_certos, 1);
        assert!(auditar(&p, 1).unwrap().problemas.is_empty());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn auditoria_pega_historico_alterado() {
        let (u, p) = worker("adulterado", &partida());
        let item = u.gerar(agora_ms()).unwrap();
        u.processar(0, item);
        let arquivo = p.join("ultrax").join("historico.txt");
        let texto = std::fs::read_to_string(&arquivo).unwrap();
        // inflar as operações para ganhar Work Score
        let operacoes = Registro::ler(texto.lines().next().unwrap()).unwrap().operacoes;
        std::fs::write(&arquivo, texto.replace(&format!("operacoes={operacoes}"), &format!("operacoes={}", operacoes * 10))).unwrap();
        let a = auditar(&p, 0).unwrap();
        assert_eq!(a.assinaturas_certas, 0);
        assert!(a.problemas.iter().any(|x| x.contains("assinatura")), "{:?}", a.problemas);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn teto_de_memoria_cancela_o_que_nao_cabe() {
        let (u, p) = worker("memoria", &Partida { memoria_mib: MEMORIA_MIN_MIB, ..partida() });
        // uma matriz 1024 pede 48 MiB, acima do teto de 32
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 1024, 0).unwrap();
        let mut tarefa = Tarefa::nova(esp, MetodoDeVerificacao::Freivalds, Instancia::PorWorker, b"x", 0, agora_ms(), agora_ms() + 60_000).unwrap();
        tarefa.avancar(Estado::NaFila, agora_ms(), "").unwrap();
        u.processar(0, NaFila { tarefa, desafio: None, recusas: 0, origem: vec![0; 32], job: None });
        let placar = u.placar();
        assert_eq!((placar.canceladas, placar.liquidadas), (1, 0));
        assert_eq!(u.reservada.load(Ordering::Relaxed), 0, "nada fica reservado");
        // e o gerador dimensiona dentro do teto
        let esp = u.dimensionar(TipoDeTrabalho::Matriz).unwrap();
        assert!(esp.memoria_bytes() <= u64::from(MEMORIA_MIN_MIB) * 1024 * 1024);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn desligar_cancela_sem_julgar() {
        let (u, p) = worker("desligar", &partida());
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 256, 0).unwrap();
        let mut tarefa = Tarefa::nova(esp, MetodoDeVerificacao::Freivalds, Instancia::PorWorker, b"y", 0, agora_ms(), agora_ms() + 60_000).unwrap();
        tarefa.avancar(Estado::NaFila, agora_ms(), "").unwrap();
        u.ligado.store(false, Ordering::Relaxed);
        u.processar(0, NaFila { tarefa, desafio: None, recusas: 0, origem: vec![0; 32], job: None });
        let placar = u.placar();
        assert_eq!((placar.canceladas, placar.recusadas, placar.reputacao.recusadas), (1, 0, 0), "parar não é errar");
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn limite_de_cpu_descansa_na_proporcao() {
        let (u, p) = worker("cpu", &Partida { uso_cpu: 50, ..partida() });
        let feitas = AtomicU64::new(0);
        let mut c = Controle::novo(&u, 0, agora_ms() + 60_000, &feitas);
        let comeco = Instant::now();
        for _ in 0..20 {
            let t = Instant::now();
            while t.elapsed() < Duration::from_millis(10) {
                std::hint::spin_loop();
            }
            assert!(c.passo(1));
        }
        let parede = comeco.elapsed().as_secs_f64();
        let calculo = c.trabalhando.as_secs_f64();
        // 50%: o relógio anda perto do dobro do cálculo (com folga para o agendador)
        assert!(parede > calculo * 1.6, "parede {parede:.3} s, cálculo {calculo:.3} s");
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn difusao_com_pouca_memoria_por_linha_nao_trava() {
        let p = pasta("difusao");
        let partida = Partida { memoria_mib: MEMORIA_MIN_MIB, linhas: 16, ..partida() };
        let u = Ultrax::abrir(&p, &[9; 32], 16, &partida, Box::new(|_, _| {}));
        // máquina rápida: os passos batem no máximo, e 2 MiB por linha não cabem a grade 256
        *u.ritmo.lock().unwrap() = [1.0e10; 8];
        let esp = u.dimensionar(TipoDeTrabalho::Difusao).unwrap();
        assert!(esp.memoria_bytes() <= u64::from(MEMORIA_MIN_MIB) * 1024 * 1024 / 16, "{}", esp.resumo());
        let _ = std::fs::remove_dir_all(p);
    }

    /// Uma pasta com uma tarefa liquidada e a `no.chave` do worker que a fez.
    fn pasta_auditavel(nome: &str) -> (PathBuf, PathBuf) {
        let (u, p) = worker(nome, &partida());
        std::fs::write(p.join("no.chave"), format!("segredo={}
", hex(&[9u8; 32]))).unwrap();
        let item = u.gerar(agora_ms()).unwrap();
        u.processar(0, item);
        let historico = p.join("ultrax").join("historico.txt");
        let a = auditar(&p, 1).unwrap();
        assert!(a.problemas.is_empty(), "a pasta honesta passa: {:?}", a.problemas);
        assert!(a.placar_conferido);
        assert_eq!((a.vereditos_certos, a.refeitas_iguais), (1, 1));
        (p, historico)
    }

    fn trocar_no_historico(historico: &Path, de: &str, para: &str) {
        let texto = std::fs::read_to_string(historico).unwrap();
        assert!(texto.contains(de), "{de} não está no histórico");
        std::fs::write(historico, texto.replacen(de, para, 1)).unwrap();
    }

    #[test]
    fn auditoria_pega_estado_trocado() {
        let (p, historico) = pasta_auditavel("estado");
        trocar_no_historico(&historico, "estado=SETTLED", "estado=REJECTED");
        let a = auditar(&p, 0).unwrap();
        assert!(a.problemas.iter().any(|x| x.contains("veredito")), "{:?}", a.problemas);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn auditoria_pega_desafio_falso_e_linha_repetida() {
        let (p, historico) = pasta_auditavel("desafio-falso");
        let linha = std::fs::read_to_string(&historico).unwrap();
        std::fs::write(&historico, format!("{linha}{linha}")).unwrap();
        trocar_no_historico(&historico, "desafio=0", "desafio=1");
        let a = auditar(&p, 0).unwrap();
        assert!(a.problemas.iter().any(|x| x.contains("não é um desafio do gabarito")), "{:?}", a.problemas);
        assert!(a.problemas.iter().any(|x| x.contains("mais de uma vez")), "{:?}", a.problemas);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn auditoria_pega_outro_worker_e_placar_inflado() {
        let (p, _) = pasta_auditavel("worker");
        std::fs::write(p.join("no.chave"), format!("segredo={}
", hex(&[8u8; 32]))).unwrap();
        let placar = p.join("ultrax").join("placar.txt");
        let texto = std::fs::read_to_string(&placar).unwrap();
        std::fs::write(&placar, texto.replace("liquidadas=1", "liquidadas=500")).unwrap();
        let a = auditar(&p, 0).unwrap();
        assert!(a.problemas.iter().any(|x| x.contains("outro worker")), "{:?}", a.problemas);
        assert!(a.problemas.iter().any(|x| x.contains("placar.txt")), "{:?}", a.problemas);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn contagem_de_recusa_dupla() {
        let (u, p) = worker("recusa", &partida());
        // um desafio com a resposta esperada errada faz esta máquina "errar" sempre
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap();
        let mut tarefa = Tarefa::nova(esp, MetodoDeVerificacao::ResultadoEsperado, Instancia::Compartilhada, b"z", 0, agora_ms(), agora_ms() + 60_000).unwrap();
        tarefa.avancar(Estado::NaFila, agora_ms(), "").unwrap();
        // desafio de índice 0 é a matriz 16; esta tarefa é 8, então a resposta nunca bate
        u.processar(0, NaFila { tarefa, desafio: Some(0), recusas: 0, origem: vec![0; 32], job: None });
        let item = u.fila.lock().unwrap().pop_front().expect("volta para a fila uma vez");
        u.processar(0, item);
        let placar = u.placar();
        assert_eq!(
            (placar.recusadas, placar.abandonadas, placar.canceladas, placar.desafios_errados, placar.liquidadas),
            (2, 1, 0, 1, 0),
            "{placar:?}"
        );
        assert_eq!(u.finais.load(Ordering::Relaxed), 1, "uma tarefa, um final");
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn treino_de_ia_guarda_o_melhor_modelo() {
        let (u, p) = worker("ia", &partida());
        // a quarta tarefa normal é a de IA
        u.sequencia.store(3, Ordering::Relaxed);
        let item = u.gerar(agora_ms()).unwrap();
        assert_eq!(item.tarefa.especificacao.tipo(), TipoDeTrabalho::Ia);
        u.processar(0, item);
        assert_eq!(u.placar().por_tipo[3], 1);
        let json = u.json();
        assert!(json.contains("\"treinos\":1"), "{json}");
        assert!(json.contains("\"previsto_mili\":-") || json.contains("\"previsto_mili\":"), "{json}");
        // e o modelo sobrevive a reabrir
        let reaberto = Ultrax::abrir(&p, &[9; 32], 2, &partida(), Box::new(|_, _| {}));
        assert!(reaberto.modelo.lock().unwrap().melhor.is_some());
        assert!(auditar(&p, 1).unwrap().problemas.is_empty());
        let _ = std::fs::remove_dir_all(p);
    }

    /// A GPU de mentira: faz a conta na CPU e manda como a página mandaria.
    fn fazer_na_gpu(u: &Ultrax, adulterar: bool) -> u32 {
        let (numero, n) = u.gpu_pegar("GPU de teste").unwrap();
        let entrada = u.gpu_entrada(numero).unwrap();
        let v: Vec<u64> = entrada.as_chunks::<4>().0.iter().map(|c| u64::from(u32::from_le_bytes(*c))).collect();
        let n = n as usize;
        let (a, b) = v.split_at(n * n);
        let mut c = vec![0u32; n * n];
        for i in 0..n {
            for k in 0..n {
                for j in 0..n {
                    c[i * n + j] += (a[i * n + k] * b[k * n + j]) as u32;
                }
            }
        }
        if adulterar {
            c[7] ^= 1;
        }
        u.gpu_progresso(numero, n as u64);
        let bytes: Vec<u8> = c.iter().flat_map(|x| x.to_le_bytes()).collect();
        u.gpu_resultado(numero, &bytes).unwrap();
        numero
    }

    #[test]
    fn gpu_honesta_e_creditada_e_adulterada_e_recusada() {
        let (u, p) = worker("gpu", &Partida { gpu: true, ..partida() });
        *u.ritmo_gpu.lock().unwrap() = 1.0e5;
        fazer_na_gpu(&u, false);
        assert_eq!(u.placar().liquidadas, 1);
        assert_eq!(u.reservada.load(Ordering::Relaxed), 0);
        fazer_na_gpu(&u, true);
        let placar = u.placar();
        assert_eq!(placar.recusadas, 1, "a CPU pegou o erro da GPU");
        assert!(auditar(&p, 1).unwrap().problemas.is_empty());
        // desligada, não entrega tarefa
        u.ajustar_tudo(None, None, None, None, Some(false), None);
        assert!(u.gpu_pegar("x").is_err());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn gpu_esquecida_vence_e_libera_a_memoria() {
        let (u, p) = worker("gpu-vence", &Partida { gpu: true, ..partida() });
        *u.ritmo_gpu.lock().unwrap() = 1.0e5;
        let (numero, _) = u.gpu_pegar("x").unwrap();
        u.gpu_cancelar(numero, "a janela fechou");
        assert_eq!(u.placar().canceladas, 1);
        assert_eq!(u.reservada.load(Ordering::Relaxed), 0);
        assert!(u.gpu_resultado(numero, &[0; 16]).is_err(), "encerrada não aceita resultado");
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn escapar_ida_e_volta() {
        let t = "a b%c\nd\te";
        assert_eq!(desescapar(&escapar(t)), t);
        assert!(!escapar(t).contains(' '));
    }
}
