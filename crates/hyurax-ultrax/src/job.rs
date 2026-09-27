// ✝ Neemias 4:6 — “Assim edificamos o muro, porque o povo tinha ânimo para trabalhar.”
//! JOB: um pedido de computação científica, dividido em unidades derivadas.
//!
//! Tradução de `reference/hyurax/job.py`, conferida por `vectors/job.json`.
//! O desenho está em `docs/COMPUTACAO-CIENTIFICA.md`, seção 2.5.
//!
//! - [`EspecificacaoDeJob`]: domínio, motor, especificação-modelo, unidades,
//!   nível de verificação, redundância, prazo, orçamento e descrição. O
//!   `JOB_ID` é o hash da codificação canônica.
//! - [`unidade`]: a unidade `i` se deriva na hora, sem ficar guardada: a
//!   especificação sai do modelo e a semente é
//!   `H(DOMINIO_UNIDADE || JOB_ID || u64 i)`.
//! - [`Intervalos`]: o progresso, em memória proporcional aos buracos.
//! - [`Resumo`]: soma dos resultados que não depende da ordem de chegada.
//! - [`Consumo`] e [`milicreditos`]: a contabilidade, versão 1.
//! - [`Liquidacao`]: a ponte para um pagamento, que hoje **não liquida**.

use std::fmt;

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{HASH_LEN, sha512};

use crate::trabalho::{self, ErroDeTrabalho, Especificacao, TipoDeTrabalho};

/// Domínio do `JOB_ID`.
pub const DOMINIO_JOB: &[u8] = dominio!("JOB-v1");
/// Domínio da semente de cada unidade.
pub const DOMINIO_UNIDADE: &[u8] = dominio!("UNIDADE-v1");
/// Domínio da folha do resumo aditivo.
pub const DOMINIO_RESUMO: &[u8] = dominio!("RESUMO-v1");

/// Versão da codificação do JOB.
pub const VERSAO: u8 = 1;
/// Maior descrição, em bytes UTF-8.
pub const DESCRICAO_MAX: usize = 2000;
/// Mais unidades que isso num JOB só, não: `2^40`, pouco mais de um trilhão.
pub const UNIDADES_MAX: u64 = 1 << 40;
/// Maior redundância (workers diferentes na mesma unidade).
pub const REDUNDANCIA_MAX: u8 = 7;
/// Créditos v1: um milicrédito a cada milhão de operações executadas e
/// conferidas (um crédito = 10^9 operações).
pub const OPERACOES_POR_MILICREDITO: u64 = 1_000_000;
/// Versão da fórmula de créditos.
pub const CREDITOS_VERSAO: u32 = 1;

/// Área de pesquisa de quem pede. Cada uma aponta os motores que a atendem;
/// as que não têm motor existem para o pedido ser recusado com o motivo
/// certo, e não para fingir que calculam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dominio {
    /// Triagem de moléculas.
    Molecular,
    /// Genética de populações.
    Genetica,
    /// Melhoramento de culturas.
    Agricultura,
    /// Rotas.
    Logistica,
    /// Alocação (mochila).
    Economia,
    /// Treino de rede neural.
    Ia,
    /// Matrizes e difusão.
    Matematica,
    /// Sem motor.
    Materiais,
    /// Sem motor.
    Energia,
    /// Sem motor.
    MeioAmbiente,
    /// Sem motor: não há modelo biológico de regeneração no programa.
    Regeneracao,
}

impl Dominio {
    /// Todos, na ordem do código.
    pub const TODOS: [Self; 11] = [
        Self::Molecular,
        Self::Genetica,
        Self::Agricultura,
        Self::Logistica,
        Self::Economia,
        Self::Ia,
        Self::Matematica,
        Self::Materiais,
        Self::Energia,
        Self::MeioAmbiente,
        Self::Regeneracao,
    ];

    /// Código na codificação canônica.
    pub fn codigo(self) -> u8 {
        match self {
            Self::Molecular => 1,
            Self::Genetica => 2,
            Self::Agricultura => 3,
            Self::Logistica => 4,
            Self::Economia => 5,
            Self::Ia => 6,
            Self::Matematica => 7,
            Self::Materiais => 8,
            Self::Energia => 9,
            Self::MeioAmbiente => 10,
            Self::Regeneracao => 11,
        }
    }

    /// O inverso de [`Self::codigo`].
    pub fn de_codigo(codigo: u8) -> Option<Self> {
        Self::TODOS.into_iter().find(|d| d.codigo() == codigo)
    }

    /// Nome curto, igual ao do gabarito.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Molecular => "molecular",
            Self::Genetica => "genetica",
            Self::Agricultura => "agricultura",
            Self::Logistica => "logistica",
            Self::Economia => "economia",
            Self::Ia => "ia",
            Self::Matematica => "matematica",
            Self::Materiais => "materiais",
            Self::Energia => "energia",
            Self::MeioAmbiente => "meio-ambiente",
            Self::Regeneracao => "regeneracao",
        }
    }

    /// Os motores que atendem. Vazio: domínio declarado, sem motor.
    pub fn motores(self) -> &'static [TipoDeTrabalho] {
        match self {
            Self::Molecular => &[TipoDeTrabalho::Triagem],
            Self::Genetica => &[TipoDeTrabalho::Genetica],
            Self::Agricultura => &[TipoDeTrabalho::Melhoramento],
            Self::Logistica => &[TipoDeTrabalho::Rotas],
            Self::Economia => &[TipoDeTrabalho::Mochila],
            Self::Ia => &[TipoDeTrabalho::Ia],
            Self::Matematica => &[TipoDeTrabalho::Matriz, TipoDeTrabalho::Difusao],
            Self::Materiais | Self::Energia | Self::MeioAmbiente | Self::Regeneracao => &[],
        }
    }
}

/// Os cinco níveis de verificação de `docs/COMPUTACAO-CIENTIFICA.md` (2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Nivel {
    /// RESULT_HASH e assinatura conferidos.
    Integridade,
    /// Recomputação (ou Freivalds) por quem pediu.
    Reexecucao,
    /// Maioria entre workers de nós diferentes.
    Concordancia,
    /// Algoritmo ou verificador diferente de quem produziu.
    Independente,
    /// Tudo acima, e o relatório com o que refaz cada unidade.
    Reprodutibilidade,
}

impl Nivel {
    /// Código `1..=5`.
    pub fn codigo(self) -> u8 {
        match self {
            Self::Integridade => 1,
            Self::Reexecucao => 2,
            Self::Concordancia => 3,
            Self::Independente => 4,
            Self::Reprodutibilidade => 5,
        }
    }

    /// O inverso de [`Self::codigo`].
    pub fn de_codigo(codigo: u8) -> Option<Self> {
        [Self::Integridade, Self::Reexecucao, Self::Concordancia, Self::Independente, Self::Reprodutibilidade]
            .into_iter()
            .find(|n| n.codigo() == codigo)
    }

    /// Nome para a tela.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Integridade => "integridade",
            Self::Reexecucao => "reexecução",
            Self::Concordancia => "concordância entre nós",
            Self::Independente => "verificação independente",
            Self::Reprodutibilidade => "reprodutibilidade",
        }
    }
}

/// Por que um JOB foi recusado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroDeJob {
    /// Domínio com código desconhecido.
    DominioDesconhecido(u8),
    /// Domínio declarado, sem motor.
    SemMotor(Dominio),
    /// O motor pedido não atende o domínio.
    MotorErrado(Dominio, TipoDeTrabalho),
    /// Campo fora da faixa.
    Faixa(&'static str),
    /// A especificação-modelo ou uma unidade derivada não valem.
    Trabalho(ErroDeTrabalho),
}

impl fmt::Display for ErroDeJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DominioDesconhecido(c) => write!(f, "domínio desconhecido: {c}"),
            Self::SemMotor(d) => write!(f, "o domínio {} ainda não tem motor: nada é calculado", d.nome()),
            Self::MotorErrado(d, t) => write!(f, "o motor {} não atende o domínio {}", t.nome(), d.nome()),
            Self::Faixa(motivo) => write!(f, "{motivo}"),
            Self::Trabalho(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ErroDeJob {}

impl From<ErroDeTrabalho> for ErroDeJob {
    fn from(e: ErroDeTrabalho) -> Self {
        Self::Trabalho(e)
    }
}

impl From<hyurax_codec::CodecError> for ErroDeJob {
    fn from(e: hyurax_codec::CodecError) -> Self {
        Self::Trabalho(ErroDeTrabalho::Codec(e))
    }
}

/// O pedido, validado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EspecificacaoDeJob {
    dominio: Dominio,
    modelo: Especificacao,
    unidades: u64,
    nivel: Nivel,
    redundancia: u8,
    prazo_s: u64,
    orcamento_milicreditos: u64,
    descricao: String,
}

/// Campos de um JOB antes da validação.
#[derive(Clone, Debug)]
pub struct PedidoDeJob {
    /// Área de pesquisa.
    pub dominio: Dominio,
    /// Especificação-modelo: a unidade 0, e a base das outras.
    pub modelo: Especificacao,
    /// Quantas unidades.
    pub unidades: u64,
    /// Nível de verificação.
    pub nivel: Nivel,
    /// Workers diferentes por unidade.
    pub redundancia: u8,
    /// Prazo do JOB inteiro, em segundos (0 = sem prazo).
    pub prazo_s: u64,
    /// Orçamento em milicréditos (0 = sem teto, só no LAB).
    pub orcamento_milicreditos: u64,
    /// O que se quer descobrir, em texto.
    pub descricao: String,
}

impl EspecificacaoDeJob {
    /// Valida e monta.
    pub fn nova(p: PedidoDeJob) -> Result<Self, ErroDeJob> {
        let motores = p.dominio.motores();
        if motores.is_empty() {
            return Err(ErroDeJob::SemMotor(p.dominio));
        }
        if !motores.contains(&p.modelo.tipo()) {
            return Err(ErroDeJob::MotorErrado(p.dominio, p.modelo.tipo()));
        }
        if !(1..=UNIDADES_MAX).contains(&p.unidades) {
            return Err(ErroDeJob::Faixa("unidades fora da faixa (1 a 2^40)"));
        }
        if !(1..=REDUNDANCIA_MAX).contains(&p.redundancia) {
            return Err(ErroDeJob::Faixa("redundância fora da faixa (1 a 7)"));
        }
        if p.nivel >= Nivel::Concordancia && p.redundancia < 2 {
            return Err(ErroDeJob::Faixa("concordância entre nós exige redundância 2 ou mais"));
        }
        if p.descricao.len() > DESCRICAO_MAX {
            return Err(ErroDeJob::Faixa("descrição longa demais (máximo 2000 bytes)"));
        }
        if p.descricao.chars().any(|c| u32::from(c) < 0x20 && c != '\n' && c != '\t') {
            return Err(ErroDeJob::Faixa("descrição com caractere de controle"));
        }
        let job = Self {
            dominio: p.dominio,
            modelo: p.modelo,
            unidades: p.unidades,
            nivel: p.nivel,
            redundancia: p.redundancia,
            prazo_s: p.prazo_s,
            orcamento_milicreditos: p.orcamento_milicreditos,
            descricao: p.descricao,
        };
        // a última unidade precisa existir (a faixa da triagem cabe no catálogo)
        trabalho::derivar_unidade(&job.modelo, job.unidades.saturating_sub(1))?;
        Ok(job)
    }

    /// Área de pesquisa.
    pub fn dominio(&self) -> Dominio {
        self.dominio
    }
    /// Especificação-modelo.
    pub fn modelo(&self) -> &Especificacao {
        &self.modelo
    }
    /// Número de unidades.
    pub fn unidades(&self) -> u64 {
        self.unidades
    }
    /// Nível de verificação.
    pub fn nivel(&self) -> Nivel {
        self.nivel
    }
    /// Workers diferentes por unidade.
    pub fn redundancia(&self) -> u8 {
        self.redundancia
    }
    /// Prazo, em segundos (0 = sem prazo).
    pub fn prazo_s(&self) -> u64 {
        self.prazo_s
    }
    /// Orçamento em milicréditos (0 = sem teto).
    pub fn orcamento_milicreditos(&self) -> u64 {
        self.orcamento_milicreditos
    }
    /// Descrição.
    pub fn descricao(&self) -> &str {
        &self.descricao
    }

    /// Codificação canônica: `u8 versão || u8 domínio || especificação || u64
    /// unidades || u8 nível || u8 redundância || u64 prazo || u64 orçamento ||
    /// string descrição`.
    pub fn codificar(&self) -> Result<Vec<u8>, ErroDeJob> {
        let mut w = Writer::new();
        w.u8(VERSAO);
        w.u8(self.dominio.codigo());
        self.modelo.codificar(&mut w);
        w.u64(self.unidades);
        w.u8(self.nivel.codigo());
        w.u8(self.redundancia);
        w.u64(self.prazo_s);
        w.u64(self.orcamento_milicreditos);
        w.string(&self.descricao)?;
        Ok(w.into_bytes())
    }

    /// O inverso de [`Self::codificar`], validando de novo.
    pub fn decodificar(dados: &[u8]) -> Result<Self, ErroDeJob> {
        let mut r = Reader::new(dados);
        if r.u8()? != VERSAO {
            return Err(ErroDeJob::Faixa("versão de JOB desconhecida"));
        }
        let codigo = r.u8()?;
        let dominio = Dominio::de_codigo(codigo).ok_or(ErroDeJob::DominioDesconhecido(codigo))?;
        let modelo = Especificacao::decodificar(&mut r)?;
        let unidades = r.u64()?;
        let nivel = Nivel::de_codigo(r.u8()?).ok_or(ErroDeJob::Faixa("nível de verificação desconhecido"))?;
        let redundancia = r.u8()?;
        let prazo_s = r.u64()?;
        let orcamento_milicreditos = r.u64()?;
        let descricao = r.string()?.to_string();
        r.finish()?;
        Self::nova(PedidoDeJob { dominio, modelo, unidades, nivel, redundancia, prazo_s, orcamento_milicreditos, descricao })
    }

    /// `JOB_ID = H(DOMINIO_JOB || codificação)`.
    pub fn id(&self) -> Result<[u8; HASH_LEN], ErroDeJob> {
        let mut dados = DOMINIO_JOB.to_vec();
        dados.extend_from_slice(&self.codificar()?);
        Ok(sha512(&dados))
    }

    /// Estimativa antes de rodar: operações de execução e de verificação
    /// (pelo teto do modelo de custo de cada motor, vezes a redundância) e
    /// os créditos que isso custa.
    pub fn estimativa(&self) -> Estimativa {
        let por_unidade = self.modelo.operacoes_maximas();
        let verificacao = trabalho::operacoes_de_verificacao(&self.modelo);
        let vezes = self.unidades.saturating_mul(u64::from(self.redundancia));
        let operacoes = por_unidade.saturating_mul(vezes);
        let operacoes_verificacao = verificacao.saturating_mul(vezes);
        Estimativa {
            operacoes,
            operacoes_verificacao,
            milicreditos: milicreditos(operacoes, operacoes_verificacao),
            memoria_por_unidade: self.modelo.memoria_bytes(),
        }
    }
}

/// O que um JOB deve custar, pelo teto do modelo de custo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Estimativa {
    /// Operações de execução, somando a redundância.
    pub operacoes: u64,
    /// Operações de verificação, somando a redundância.
    pub operacoes_verificacao: u64,
    /// Créditos v1, em milésimos.
    pub milicreditos: u64,
    /// Memória no pico de uma unidade.
    pub memoria_por_unidade: u64,
}

/// Uma unidade, derivada do JOB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unidade {
    /// O índice `i`.
    pub indice: u64,
    /// A especificação desta unidade.
    pub especificacao: Especificacao,
    /// `H(DOMINIO_UNIDADE || JOB_ID || u64 i)`.
    pub semente: [u8; HASH_LEN],
}

/// A semente da unidade `indice` do JOB `job_id`.
pub fn semente_da_unidade(job_id: &[u8; HASH_LEN], indice: u64) -> [u8; HASH_LEN] {
    let mut dados = DOMINIO_UNIDADE.to_vec();
    dados.extend_from_slice(job_id);
    dados.extend_from_slice(&indice.to_be_bytes());
    sha512(&dados)
}

/// Deriva a unidade `indice`. Nada é guardado: o custo é o mesmo para a
/// unidade 0 e para a unidade um bilhão.
pub fn unidade(job: &EspecificacaoDeJob, job_id: &[u8; HASH_LEN], indice: u64) -> Result<Unidade, ErroDeJob> {
    if indice >= job.unidades {
        return Err(ErroDeJob::Faixa("unidade além do fim do JOB"));
    }
    Ok(Unidade { indice, especificacao: trabalho::derivar_unidade(&job.modelo, indice)?, semente: semente_da_unidade(job_id, indice) })
}

// ---------------------------------------------------------------------------
// Progresso
// ---------------------------------------------------------------------------

/// Intervalos `[a, b)` concluídos: ordenados, disjuntos e sem encostar (dois
/// que se tocam viram um). A memória cresce com o número de buracos, não
/// com o de unidades.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Intervalos {
    faixas: Vec<(u64, u64)>,
}

impl Intervalos {
    /// Vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// As faixas, em ordem.
    pub fn faixas(&self) -> &[(u64, u64)] {
        &self.faixas
    }

    /// Marca `[a, b)` como concluído. `a >= b` não faz nada.
    pub fn inserir(&mut self, a: u64, b: u64) {
        if a >= b {
            return;
        }
        // primeira faixa que termina em `a` ou depois (pode encostar)
        let inicio = self.faixas.partition_point(|&(_, y)| y < a);
        // primeira faixa que começa depois de `b` (não encosta)
        let fim = self.faixas.partition_point(|&(x, _)| x <= b);
        let (mut novo_a, mut novo_b) = (a, b);
        if let Some(juntas) = self.faixas.get(inicio..fim) {
            if let (Some(&(x, _)), Some(&(_, y))) = (juntas.first(), juntas.last()) {
                novo_a = novo_a.min(x);
                novo_b = novo_b.max(y);
            }
            self.faixas.splice(inicio..fim, [(novo_a, novo_b)]);
        }
    }

    /// Marca uma unidade.
    pub fn inserir_um(&mut self, i: u64) {
        self.inserir(i, i.saturating_add(1));
    }

    /// `i` já foi concluída?
    pub fn contem(&self, i: u64) -> bool {
        self.fim_da_faixa(i).is_some()
    }

    /// Se `i` está numa faixa, onde ela termina.
    pub fn fim_da_faixa(&self, i: u64) -> Option<u64> {
        let k = self.faixas.partition_point(|&(_, y)| y <= i);
        self.faixas.get(k).filter(|&&(x, _)| x <= i).map(|&(_, y)| y)
    }

    /// Quantas unidades concluídas.
    pub fn concluidas(&self) -> u64 {
        self.faixas.iter().fold(0u64, |t, &(x, y)| t.saturating_add(y.saturating_sub(x)))
    }

    /// A primeira unidade em `[desde, total)` que falta, se houver.
    pub fn primeira_faltante(&self, desde: u64, total: u64) -> Option<u64> {
        let mut i = desde;
        let k = self.faixas.partition_point(|&(_, y)| y <= i);
        if let Some(&(x, y)) = self.faixas.get(k)
            && x <= i
        {
            i = y;
        }
        (i < total).then_some(i)
    }

    /// `u32 n || n × (u64 a || u64 b)`.
    pub fn codificar(&self, w: &mut Writer) -> Result<(), ErroDeJob> {
        w.u32(u32::try_from(self.faixas.len()).map_err(|_| ErroDeJob::Faixa("faixas demais para codificar"))?);
        for &(a, b) in &self.faixas {
            w.u64(a);
            w.u64(b);
        }
        Ok(())
    }

    /// O inverso de [`Self::codificar`], recusando faixas fora de ordem,
    /// vazias ou encostadas (a forma canônica é uma só).
    pub fn decodificar(r: &mut Reader<'_>) -> Result<Self, ErroDeJob> {
        let n = r.u32()?;
        let mut faixas = Vec::new();
        let mut anterior: Option<u64> = None;
        for _ in 0..n {
            let (a, b) = (r.u64()?, r.u64()?);
            if a >= b || anterior.is_some_and(|fim| a <= fim) {
                return Err(ErroDeJob::Faixa("intervalos fora da forma canônica"));
            }
            anterior = Some(b);
            faixas.push((a, b));
        }
        Ok(Self { faixas })
    }
}

/// Resumo aditivo dos resultados: soma, módulo `2^512`, de
/// `H(DOMINIO_RESUMO || u64 i || RESULT_HASH)`. Sai igual em qualquer ordem
/// de chegada, e dois nós que consolidaram o mesmo conjunto chegam no mesmo
/// valor. Não é prova forte contra quem escolhe resultados de propósito: para
/// isso o relatório guarda o RESULT_HASH de cada unidade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resumo {
    soma: [u8; HASH_LEN],
    /// Quantas unidades entraram.
    pub unidades: u64,
}

impl Default for Resumo {
    fn default() -> Self {
        Self { soma: [0; HASH_LEN], unidades: 0 }
    }
}

impl Resumo {
    /// Acrescenta uma unidade.
    pub fn somar(&mut self, indice: u64, hash_do_resultado: &[u8; HASH_LEN]) {
        let mut dados = DOMINIO_RESUMO.to_vec();
        dados.extend_from_slice(&indice.to_be_bytes());
        dados.extend_from_slice(hash_do_resultado);
        let folha = sha512(&dados);
        // soma big-endian com vai-um, da direita para a esquerda
        let mut vai = 0u16;
        for (s, f) in self.soma.iter_mut().rev().zip(folha.iter().rev()) {
            let t = u16::from(*s).saturating_add(u16::from(*f)).saturating_add(vai);
            let [alto, baixo] = t.to_be_bytes();
            *s = baixo;
            vai = u16::from(alto);
        }
        self.unidades = self.unidades.saturating_add(1);
    }

    /// Os 64 bytes da soma.
    pub fn bytes(&self) -> [u8; HASH_LEN] {
        self.soma
    }

    /// Recria de bytes guardados.
    pub fn de_bytes(soma: [u8; HASH_LEN], unidades: u64) -> Self {
        Self { soma, unidades }
    }
}

// ---------------------------------------------------------------------------
// Contabilidade
// ---------------------------------------------------------------------------

/// O que um JOB consumiu. Tudo é registrado; só as operações viram crédito
/// na v1 (ver [`milicreditos`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Consumo {
    /// Unidades conferidas.
    pub unidades: u64,
    /// Operações executadas, pelo modelo de custo, das unidades conferidas.
    pub operacoes: u64,
    /// Operações gastas conferindo.
    pub operacoes_verificacao: u64,
    /// Milissegundos de CPU (medidos pelo limitador).
    pub cpu_ms: u64,
    /// Milissegundos de GPU (medidos pela janela).
    pub gpu_ms: u64,
    /// Memória reservada vezes tempo, em MiB·s.
    pub memoria_mib_s: u64,
    /// Bytes que passaram pela rede por causa do JOB.
    pub bytes_rede: u64,
}

impl Consumo {
    /// Soma outro consumo a este.
    pub fn somar(&mut self, outro: &Self) {
        self.unidades = self.unidades.saturating_add(outro.unidades);
        self.operacoes = self.operacoes.saturating_add(outro.operacoes);
        self.operacoes_verificacao = self.operacoes_verificacao.saturating_add(outro.operacoes_verificacao);
        self.cpu_ms = self.cpu_ms.saturating_add(outro.cpu_ms);
        self.gpu_ms = self.gpu_ms.saturating_add(outro.gpu_ms);
        self.memoria_mib_s = self.memoria_mib_s.saturating_add(outro.memoria_mib_s);
        self.bytes_rede = self.bytes_rede.saturating_add(outro.bytes_rede);
    }

    /// Créditos v1 deste consumo, em milésimos.
    pub fn milicreditos(&self) -> u64 {
        milicreditos(self.operacoes, self.operacoes_verificacao)
    }
}

/// Créditos de computação v1, em milésimos: um milicrédito por milhão de
/// operações executadas e conferidas. Tempo, memória e rede ficam de fora da
/// v1: o tempo depende da máquina, e contá-lo premiaria a máquina lenta.
pub fn milicreditos(operacoes: u64, operacoes_verificacao: u64) -> u64 {
    operacoes.saturating_add(operacoes_verificacao) / OPERACOES_POR_MILICREDITO
}

/// O que aconteceu na liquidação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Liquidado {
    /// Nada foi pago, e o motivo.
    NaoLiquidado(&'static str),
}

/// A ponte entre contabilidade e pagamento. A computação e a contabilidade
/// não sabem que HYX existe; só uma implementação desta interface saberia.
pub trait Liquidacao {
    /// Liquida o consumo de um JOB.
    fn liquidar(&self, job: &[u8; HASH_LEN], consumo: &Consumo) -> Liquidado;
}

/// A única liquidação que existe hoje: nenhuma. Crédito não vira HYX sem
/// regra econômica publicada.
#[derive(Clone, Copy, Debug, Default)]
pub struct NaoLiquida;

impl Liquidacao for NaoLiquida {
    fn liquidar(&self, _job: &[u8; HASH_LEN], _consumo: &Consumo) -> Liquidado {
        Liquidado::NaoLiquidado("créditos não viram HYX: não há regra econômica publicada")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod testes {
    use super::*;

    fn matriz(unidades: u64) -> EspecificacaoDeJob {
        EspecificacaoDeJob::nova(PedidoDeJob {
            dominio: Dominio::Matematica,
            modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 64, 0).unwrap(),
            unidades,
            nivel: Nivel::Reexecucao,
            redundancia: 1,
            prazo_s: 0,
            orcamento_milicreditos: 0,
            descricao: "teste".into(),
        })
        .unwrap()
    }

    #[test]
    fn job_vai_e_volta_e_o_id_muda_com_qualquer_campo() {
        let j = matriz(1000);
        let volta = EspecificacaoDeJob::decodificar(&j.codificar().unwrap()).unwrap();
        assert_eq!(volta, j);
        assert_ne!(matriz(1001).id().unwrap(), j.id().unwrap());
    }

    #[test]
    fn dominio_sem_motor_e_motor_errado_sao_recusados() {
        let base = |dominio| PedidoDeJob {
            dominio,
            modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap(),
            unidades: 1,
            nivel: Nivel::Reexecucao,
            redundancia: 1,
            prazo_s: 0,
            orcamento_milicreditos: 0,
            descricao: String::new(),
        };
        for d in [Dominio::Materiais, Dominio::Energia, Dominio::MeioAmbiente, Dominio::Regeneracao] {
            assert_eq!(EspecificacaoDeJob::nova(base(d)), Err(ErroDeJob::SemMotor(d)));
        }
        assert!(matches!(EspecificacaoDeJob::nova(base(Dominio::Genetica)), Err(ErroDeJob::MotorErrado(..))));
        let mut p = base(Dominio::Matematica);
        p.nivel = Nivel::Concordancia;
        assert!(EspecificacaoDeJob::nova(p.clone()).is_err(), "concordância com um worker só");
        p.redundancia = 3;
        assert!(EspecificacaoDeJob::nova(p).is_ok());
    }

    #[test]
    fn unidade_deriva_sem_guardar_e_a_semente_muda_com_o_indice() {
        let j = matriz(UNIDADES_MAX);
        let id = j.id().unwrap();
        let u0 = unidade(&j, &id, 0).unwrap();
        let ultima = unidade(&j, &id, UNIDADES_MAX - 1).unwrap();
        assert_ne!(u0.semente, ultima.semente);
        assert_eq!(u0.especificacao, *j.modelo());
        assert!(unidade(&j, &id, UNIDADES_MAX).is_err());
    }

    #[test]
    fn intervalos_fundem_e_acham_o_buraco() {
        let mut v = Intervalos::new();
        v.inserir(10, 20);
        v.inserir(30, 40);
        v.inserir(20, 30); // encosta nas duas
        assert_eq!(v.faixas(), &[(10, 40)]);
        v.inserir(0, 5);
        v.inserir_um(5);
        assert_eq!(v.faixas(), &[(0, 6), (10, 40)]);
        assert!(v.contem(5) && !v.contem(6) && v.contem(39) && !v.contem(40));
        assert_eq!(v.primeira_faltante(0, 100), Some(6));
        assert_eq!(v.primeira_faltante(12, 100), Some(40));
        assert_eq!(v.primeira_faltante(40, 40), None);
        assert_eq!(v.concluidas(), 36);
        let mut w = Writer::new();
        v.codificar(&mut w).unwrap();
        let bytes = w.into_bytes();
        assert_eq!(Intervalos::decodificar(&mut Reader::new(&bytes)).unwrap(), v);
    }

    #[test]
    fn intervalos_com_um_milhao_de_unidades_fora_de_ordem_ficam_numa_faixa() {
        // 1.000.000 de unidades concluídas numa ordem embaralhada (passo
        // coprimo com o total): a memória termina numa faixa só.
        let total = 1_000_000u64;
        let mut v = Intervalos::new();
        let mut i = 0u64;
        for _ in 0..total {
            v.inserir_um(i);
            i = (i + 7919) % total;
        }
        assert_eq!(v.faixas(), &[(0, total)]);
        assert_eq!(v.concluidas(), total);
        assert_eq!(v.primeira_faltante(0, total), None);
    }

    #[test]
    fn intervalos_decodificar_recusa_forma_nao_canonica() {
        for faixas in [vec![(5u64, 5u64)], vec![(0, 10), (10, 20)], vec![(10, 20), (0, 5)]] {
            let mut w = Writer::new();
            w.u32(faixas.len() as u32);
            for (a, b) in faixas {
                w.u64(a);
                w.u64(b);
            }
            let bytes = w.into_bytes();
            assert!(Intervalos::decodificar(&mut Reader::new(&bytes)).is_err());
        }
    }

    #[test]
    fn resumo_nao_depende_da_ordem() {
        let hashes: Vec<[u8; HASH_LEN]> = (0u8..50).map(|k| sha512(&[k])).collect();
        let mut a = Resumo::default();
        let mut b = Resumo::default();
        for (i, h) in hashes.iter().enumerate() {
            a.somar(i as u64, h);
        }
        for (i, h) in hashes.iter().enumerate().rev() {
            b.somar(i as u64, h);
        }
        assert_eq!(a, b);
        let mut c = Resumo::default();
        for (i, h) in hashes.iter().enumerate() {
            c.somar(i as u64, if i == 7 { &hashes[8] } else { h });
        }
        assert_ne!(a.bytes(), c.bytes(), "um resultado trocado muda o resumo");
    }

    #[test]
    fn creditos_v1_e_liquidacao_que_nao_paga() {
        assert_eq!(milicreditos(999_999, 0), 0);
        assert_eq!(milicreditos(1_000_000_000, 1_000_000_000), 2000);
        let j = matriz(10);
        let e = j.estimativa();
        assert_eq!(e.operacoes, 64 * 64 * 64 * 10);
        assert!(matches!(NaoLiquida.liquidar(&j.id().unwrap(), &Consumo::default()), Liquidado::NaoLiquidado(_)));
    }
}
