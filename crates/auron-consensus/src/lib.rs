// ✝ Provérbios 16:11 — “O peso e a balança justos são do Senhor.”
//! Auron — regras numéricas do consenso (seções 8, 10, 11, 12, 15 e 17 da
//! AURON-SPEC-01).
//!
//! Tradução de `reference/auron/consensus.py`, conferida contra
//! `vectors/targets.json` e `vectors/emission.json`:
//!
//! - parâmetros de cada rede, num lugar só;
//! - alvo normalizado e trabalho de um bloco (`2^256 / (alvo + 1)`);
//! - ajuste de dificuldade LWMA-1, com a janela tornada não decrescente;
//! - median-time-past;
//! - recompensa por altura e emissão acumulada.
//!
//! Tudo em inteiro. As contas que passam de 256 bits usam [`U512`].

#![forbid(unsafe_code)]

mod u512;

use std::fmt;

pub use auron_pow::{alvo_de_bits, bits_de_alvo, normalizar_alvo};
use auron_pow::ParametrosPow;
use auron_types::MAX_SUPPLY;
use auron_usefulpow::ParametrosUteis;
pub use u512::U512;

/// Alvo de 256 bits em big-endian.
pub type Alvo = [u8; 32];

/// Recusa de regra numérica do consenso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsensusError(pub String);

impl fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConsensusError {}

fn erro(m: impl Into<String>) -> ConsensusError {
    ConsensusError(m.into())
}

/// Tudo o que distingue uma rede da outra (`ChainParams`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametrosRede {
    /// Nome, como nos vetores.
    pub nome: &'static str,
    /// Identificador binário da rede.
    pub magic: [u8; 4],
    /// Argon2id.
    pub pow: ParametrosPow,
    /// Alvo mais fácil permitido, já canônico.
    pub max_target: Alvo,
    /// Tempo desejado entre blocos, em segundos. Só o que o LWMA persegue.
    pub target_spacing: u64,
    /// Janela do LWMA, em blocos.
    pub lwma_window: usize,
    /// Recompensa inicial, em unidades (PROVISÓRIO).
    pub initial_reward: u64,
    /// Blocos entre halvings (PROVISÓRIO).
    pub halving_interval: u64,
    /// Tamanho máximo do bloco codificado.
    pub max_block_bytes: usize,
    /// Quanto um timestamp pode passar do relógio local.
    pub max_future_drift: u64,
    /// Blocos usados no median-time-past.
    pub median_time_span: usize,
    /// Blocos até a coinbase poder ser gasta.
    pub coinbase_maturity: u64,
    /// Parâmetros do trabalho útil.
    pub uteis: ParametrosUteis,
}

const AUR_UNIT: u64 = auron_types::AUR_UNIT;

impl ParametrosRede {
    /// Rede principal.
    pub const MAINNET: Self = Self {
        nome: "auron-mainnet",
        magic: *b"AURM",
        pow: ParametrosPow::MAINNET,
        max_target: ParametrosUteis::MAINNET.max_target,
        target_spacing: 120,
        lwma_window: 60,
        initial_reward: 50 * AUR_UNIT,
        halving_interval: 210_000,
        max_block_bytes: 1_000_000,
        max_future_drift: 120,
        median_time_span: 11,
        coinbase_maturity: 100,
        uteis: ParametrosUteis::MAINNET,
    };
    /// Rede de teste.
    pub const TESTNET: Self = Self {
        nome: "auron-testnet",
        magic: *b"AURT",
        pow: ParametrosPow::TESTNET,
        max_target: ParametrosUteis::TESTNET.max_target,
        coinbase_maturity: 20,
        uteis: ParametrosUteis::TESTNET,
        ..Self::MAINNET
    };
    /// Rede local de desenvolvimento.
    pub const REGTEST: Self = Self {
        nome: "auron-regtest",
        magic: *b"AURR",
        pow: ParametrosPow::REGTEST,
        max_target: ParametrosUteis::REGTEST.max_target,
        lwma_window: 30,
        coinbase_maturity: 2,
        uteis: ParametrosUteis::REGTEST,
        ..Self::MAINNET
    };

    /// Parâmetros pelo nome (`auron-mainnet` ou `mainnet`).
    pub fn da_rede(nome: &str) -> Option<Self> {
        match nome.trim_start_matches("auron-") {
            "mainnet" => Some(Self::MAINNET),
            "testnet" => Some(Self::TESTNET),
            "regtest" => Some(Self::REGTEST),
            _ => None,
        }
    }

    /// Soma da emissão até a recompensa zerar.
    pub fn total_emission(&self) -> u64 {
        let mut total = 0u64;
        let mut reward = self.initial_reward;
        while reward > 0 {
            total = total.saturating_add(reward.saturating_mul(self.halving_interval));
            reward /= 2;
        }
        total
    }
}

// ---------------------------------------------------------------------------
// Alvo e trabalho
// ---------------------------------------------------------------------------

/// Trabalho esperado para bater o alvo: `2^256 / (alvo + 1)`.
pub fn target_to_work(alvo: &Alvo) -> Result<U512, ConsensusError> {
    let a = U512::from_be32(alvo);
    if a.is_zero() {
        return Err(erro("alvo deve ser positivo"));
    }
    let divisor = a.checked_add(&U512::from_u64(1)).ok_or_else(|| erro("alvo fora da faixa"))?;
    U512::potencia_de_dois(256).div(&divisor).ok_or_else(|| erro("alvo fora da faixa"))
}

// ---------------------------------------------------------------------------
// Retarget — LWMA-1
// ---------------------------------------------------------------------------

fn para_alvo(v: &U512) -> Result<Alvo, ConsensusError> {
    v.to_be32().ok_or_else(|| erro("alvo acima do limite representável"))
}

fn normalizar(v: &U512) -> Result<Alvo, ConsensusError> {
    normalizar_alvo(&para_alvo(v)?).map_err(|e| erro(e.to_string()))
}

/// Alvo do próximo bloco (`next_target`).
///
/// `timestamps` e `targets` são dos últimos blocos, do mais antigo para o mais
/// recente, com o mesmo tamanho.
pub fn next_target(
    timestamps: &[u64],
    targets: &[Alvo],
    p: &ParametrosRede,
) -> Result<Alvo, ConsensusError> {
    let n = timestamps.len();
    if n != targets.len() {
        return Err(erro("timestamps e targets com tamanhos diferentes"));
    }
    let maximo = U512::from_be32(&p.max_target);
    let Some(ultimo) = targets.last() else {
        return Ok(p.max_target);
    };
    if n == 1 {
        return normalizar(&U512::from_be32(ultimo).min(maximo));
    }

    let janela = n.saturating_sub(1).min(p.lwma_window);
    let ts = timestamps.get(n.saturating_sub(janela.saturating_add(1))..).unwrap_or(&[]);
    let tg = targets.get(n.saturating_sub(janela)..).unwrap_or(&[]);
    let janela_u64 = janela as u64;
    let espaco = p.target_spacing;
    let k = janela_u64
        .saturating_mul(janela_u64.saturating_add(1))
        .checked_div(2)
        .unwrap_or(0)
        .saturating_mul(espaco);

    // Janela não decrescente: horário para trás vira tempo zero (seção 10).
    let mut ponderado = 0u64;
    let mut maior = ts.first().copied().unwrap_or(0);
    for (i, &t) in (1u64..).zip(ts.iter().skip(1)) {
        let anterior = maior;
        maior = maior.max(t);
        let resolucao = maior.saturating_sub(anterior).min(espaco.saturating_mul(6));
        ponderado = ponderado.saturating_add(resolucao.saturating_mul(i));
    }
    ponderado = ponderado.max(k / 3);

    let mut soma = U512::ZERO;
    for alvo in tg {
        soma = soma.checked_add(&U512::from_be32(alvo)).ok_or_else(|| erro("soma de alvos estourou"))?;
    }
    let media = soma.div_u64(janela_u64).ok_or_else(|| erro("janela vazia"))?;
    let mut candidato = media
        .checked_mul_u64(ponderado)
        .and_then(|v| v.div_u64(k))
        .ok_or_else(|| erro("conta do retarget estourou"))?;

    let anterior = U512::from_be32(tg.last().unwrap_or(ultimo));
    let metade = anterior.div_u64(2).unwrap_or(U512::ZERO);
    let dobro = anterior.checked_mul_u64(2).ok_or_else(|| erro("alvo anterior estourou"))?;
    candidato = candidato.max(metade).min(dobro);
    if candidato.is_zero() {
        candidato = U512::from_u64(1);
    }
    normalizar(&candidato.min(maximo))
}

/// Mediana dos últimos `median_time_span` timestamps.
pub fn median_time_past(timestamps: &[u64], p: &ParametrosRede) -> u64 {
    let inicio = timestamps.len().saturating_sub(p.median_time_span);
    let mut recentes = timestamps.get(inicio..).unwrap_or(&[]).to_vec();
    recentes.sort_unstable();
    recentes.get(recentes.len() / 2).copied().unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Emissão
// ---------------------------------------------------------------------------

/// Recompensa do bloco na altura dada, com halving.
pub fn block_reward(altura: u64, p: &ParametrosRede) -> u64 {
    let halvings = altura.checked_div(p.halving_interval).unwrap_or(u64::MAX);
    if halvings >= 64 {
        return 0;
    }
    p.initial_reward >> halvings
}

/// Total emitido até a altura dada, inclusive. Nunca passa de `MAX_SUPPLY`.
pub fn cumulative_emission(altura: u64, p: &ParametrosRede) -> u64 {
    let mut total = 0u64;
    let mut restante = u128::from(altura) + 1;
    for era in 0..64u32 {
        let recompensa = p.initial_reward >> era;
        if restante == 0 || recompensa == 0 {
            break;
        }
        let quantos = restante.min(u128::from(p.halving_interval));
        total = total.saturating_add(recompensa.saturating_mul(quantos as u64));
        restante = restante.saturating_sub(quantos);
    }
    total.min(MAX_SUPPLY)
}
