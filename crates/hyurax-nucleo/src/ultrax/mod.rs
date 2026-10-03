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
//!   já se sabe, porque veio do gabarito em Python (`vectors/ultrax.json`). Se
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
//! nenhuma. Quem tem o `no.chave` consegue assinar qualquer registro: a
//! auditoria prova coerência do histórico, não honestidade do dono.
//!
//! Arquivos: [`lab`] (gerador), [`worker`] (linhas de CPU e limitador), [`gpu`]
//! (backend WebGL 2), [`disco`] (placar, histórico, telemetria), [`estado`]
//! (JSON), [`historico`] (formato do histórico) e [`auditoria`].

mod auditoria;
mod disco;
mod estado;
mod gpu;
mod historico;
mod lab;
mod worker;
#[cfg(test)]
mod testes;

pub use auditoria::{Auditoria, auditar};
use auditoria::operacoes_da_instancia;
use historico::*;
use worker::*;

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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

use crate::util::texto_json;
use crate::util::{de_hex, hex};

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
/// Semente dos vetores do gabarito (`gen_vectors.vec_ultrax`).
const SEMENTE_DOS_DESAFIOS: &[u8] = b"vetor ultrax";

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

/// Os maiores casos de `vectors/ultrax.json` e de `vectors/ia.json`. O teste
/// `desafios_batem_com_os_vetores` confere cada hash contra os arquivos.
const DESAFIOS: [Desafio; 4] = [
    Desafio {
        semente: SEMENTE_DOS_DESAFIOS,
        tipo: TipoDeTrabalho::Matriz,
        tamanho: 16,
        passos: 0,
        esperado: "d1febe844db50e441a218ee2eeae042b9df13d0f60c612bfbf30d9d8b59f474700e91d805176c7ecaea42ce989b0a7213fe325e6dc6c71e065a4b81a7fd5f8e4",
    },
    Desafio {
        semente: SEMENTE_DOS_DESAFIOS,
        tipo: TipoDeTrabalho::Mochila,
        tamanho: 20,
        passos: 0,
        esperado: "067b6ac73dd2e7808e3041fcb1b22a5e13f3ae0ec7872d934287e4e57141554e3e7bc3f218e6b8bc04a67739b2473d573d3ce228488f00d50737af8d3f9f2675",
    },
    Desafio {
        semente: SEMENTE_DOS_DESAFIOS,
        tipo: TipoDeTrabalho::Difusao,
        tamanho: 16,
        passos: 10,
        esperado: "869c21ae7546e98ab3e425bcb4ccc8465add4d9ce6add916cb2bb6b509363e5bf7e47bfc4471fcea578e5040abe19494bc253aefe4383b9dd95676eb937b4454",
    },
    Desafio {
        semente: b"vetor ia",
        tipo: TipoDeTrabalho::Ia,
        tamanho: 16,
        passos: 25,
        esperado: "d4ed6cdc8d654d42b1d9f61b6ee2a9b767f70547c0c40df71f4124040507b4e588c47e48e687fcf499f5e0e198eb1308e9e0c75e09f36a235a620a023c8d2313",
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
    /// A assinatura do registro pelo worker.
    pub assinatura: Option<[u8; 64]>,
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
    /// A próxima unidade de um tipo só (a GPU pede as de matriz).
    fn proxima_do_tipo(&self, agora_ms: u64, tipo: TipoDeTrabalho) -> Option<PedidoDeUnidade> {
        let _ = (agora_ms, tipo);
        None
    }
    /// A unidade começou a executar nesta linha.
    fn comecou(&self, job: &[u8; HASH_LEN], indice: u64, linha: u32, entrada: Option<[u8; HASH_LEN]>);
    /// A unidade terminou, foi recusada uma vez, cancelada ou venceu.
    fn terminou(&self, desfecho: DesfechoDeUnidade);
    /// Há JOB com unidade esperando a vez. Enquanto houver, a LAB (carga de
    /// teste) não gera nada: o trabalho pedido vem primeiro.
    fn tem_trabalho(&self) -> bool;
    /// O JOB ainda quer esta unidade? `false` depois de cancelado, vencido ou
    /// concluído: a unidade na fila não começa, e a que roda para.
    fn ainda_quer(&self, job: &[u8; HASH_LEN], indice: u64) -> bool {
        let _ = (job, indice);
        true
    }
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

/// Uma linha do histórico recente.
#[derive(Clone, Debug)]
pub struct Lembranca {
    /// Número da tarefa nesta máquina.
    pub numero: u32,
    /// O tipo de trabalho.
    pub tipo: TipoDeTrabalho,
    /// Resumo da especificação.
    pub resumo: String,
    /// Como foi conferida.
    pub metodo: MetodoDeVerificacao,
    /// Tarefa-desafio (resposta conhecida).
    pub desafio: bool,
    /// Estado final.
    pub estado: Estado,
    /// Operações feitas.
    pub operacoes: u64,
    /// Tempo de cálculo, em ms.
    pub ms_calculo: u64,
    /// Quando terminou.
    pub fim_ms: u64,
    /// `RESULT_HASH`.
    pub resultado: Option<[u8; HASH_LEN]>,
    /// Observação (motivo de recusa, por exemplo).
    pub nota: String,
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

/// Uma unidade de JOB: o `JOB_ID` e o índice.
pub type UnidadeDoJob = ([u8; HASH_LEN], u64);

/// De onde veio uma amostra: a tarefa, a unidade de JOB (se for), a linha e o
/// recurso que calculou.
#[derive(Clone, Debug)]
pub struct ContextoDaAmostra {
    /// Número da tarefa nesta máquina.
    pub tarefa: u32,
    /// `JOB_ID` e índice da unidade, quando a tarefa é unidade de JOB.
    pub job: Option<([u8; HASH_LEN], u64)>,
    /// Linha de CPU (ou 0 na GPU).
    pub linha: u32,
    /// `CPU` ou `GPU`.
    pub recurso: &'static str,
    /// Tarefa-desafio (resposta conhecida).
    pub desafio: bool,
    /// Resumo da especificação.
    pub resumo: String,
    /// A especificação: tamanho, passos e parâmetros (as rotas precisam da
    /// instância para desenhar as cidades).
    pub especificacao: Especificacao,
}

impl ContextoDaAmostra {
    /// Em JSON.
    pub fn json(&self) -> String {
        let (job, unidade) = match self.job {
            Some((j, i)) => (format!("\"{}\"", hex(&j)), i.to_string()),
            None => ("null".into(), "null".into()),
        };
        format!(
            "{{\"tarefa\":{},\"job\":{job},\"unidade\":{unidade},\"linha\":{},\"recurso\":\"{}\",\"lab\":{},\"desafio\":{},\"resumo\":{},\"tamanho\":{},\"passos\":{},\"parametros\":[{}]}}",
            self.tarefa,
            self.linha,
            self.recurso,
            self.job.is_none(),
            self.desafio,
            texto_json(&self.resumo),
            self.especificacao.tamanho(),
            self.especificacao.passos(),
            self.especificacao.parametros().iter().map(u32::to_string).collect::<Vec<_>>().join(",")
        )
    }
}

/// Recebe as amostras dos motores (o barramento do núcleo).
pub type SaidaDeAmostras = Arc<dyn Fn(&ContextoDaAmostra, hyurax_ultrax::observador::Amostra) + Send + Sync>;
/// Recebe cada passo de cada tarefa: (tarefa, evento, detalhe).
pub type SaidaDeTarefas = Arc<dyn Fn(u32, &'static str, &str) + Send + Sync>;

/// Intervalo mínimo entre duas amostras da mesma linha.
const INTERVALO_DAS_AMOSTRAS: Duration = Duration::from_millis(150);

/// O worker LAB e tudo o que ele mede.
pub struct Ultrax {
    pasta: PathBuf,
    segredo: [u8; SECRET_LEN],
    worker: [u8; PUBKEY_LEN],
    nucleos: u32,
    /// O dono ligou o ULTRAX.
    pub ligado: AtomicBool,
    /// Fim das threads (orquestrador e linhas): só o benchmark usa, num
    /// ULTRAX temporário que precisa sumir depois.
    encerrado: AtomicBool,
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
    /// Acorda uma linha parada quando algo entra na fila.
    tem_na_fila: Condvar,
    /// Acorda o gerador quando a fila anda (uma linha pegou ou terminou
    /// uma tarefa), em vez de esperar o próximo segundo.
    pede_mais: Mutex<bool>,
    pede_mais_cv: Condvar,
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
    /// Quem recebe as amostras dos motores.
    saida_de_amostras: Mutex<Option<SaidaDeAmostras>>,
    /// Quem recebe os passos de cada tarefa.
    saida_de_tarefas: Mutex<Option<SaidaDeTarefas>>,
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
            encerrado: AtomicBool::new(false),
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
            tem_na_fila: Condvar::new(),
            pede_mais: Mutex::new(false),
            pede_mais_cv: Condvar::new(),
            ativas: Mutex::new(Vec::new()),
            placar: Mutex::new(placar),
            historico: Mutex::new(VecDeque::new()),
            telemetria: Mutex::new(VecDeque::new()),
            ritmo: Mutex::new([RITMO_INICIAL; 8]),
            agendador: Mutex::new(None),
            modelo: Mutex::new(Modelo::default()),
            aviso,
            saida_de_amostras: Mutex::new(None),
            saida_de_tarefas: Mutex::new(None),
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

    /// Termina as threads deste ULTRAX (orquestrador e linhas) de vez.
    pub fn encerrar_threads(&self) {
        self.ligado.store(false, Ordering::Relaxed);
        self.encerrado.store(true, Ordering::Relaxed);
        self.tem_na_fila.notify_all();
        self.pedir_mais();
    }

    pub(super) fn encerrado(&self) -> bool {
        self.encerrado.load(Ordering::Relaxed)
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

    /// O ritmo de um tipo, só se já foi medido nesta abertura (senão, `None`:
    /// o valor inicial é um chute, e a tela não o chama de medido).
    pub fn ritmo_medido(&self, tipo: TipoDeTrabalho) -> Option<f64> {
        Some(self.ritmo_de(tipo)).filter(|r| (*r - RITMO_INICIAL).abs() > f64::EPSILON)
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
}
