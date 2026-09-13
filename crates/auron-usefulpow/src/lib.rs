// ✝ Provérbios 14:23 — “Em todo trabalho há proveito; meras palavras, porém, levam à penúria.”
//! Auron — trabalho útil dentro do consenso (seção 9A da AURON-SPEC-01).
//!
//! Tradução de `reference/auron/usefulpow.py`, conferida contra
//! `vectors/usefulpow.json`. As mensagens de recusa são as do Python, letra por
//! letra: o vetor grava o motivo, e o Rust precisa recusar pelo mesmo motivo.
//!
//! Família 1, `MATRIX-FREIVALDS-V1`:
//!
//! - **tarefa:** `C = A · B`, matrizes `n×n` com entradas em `[0, 999]`;
//! - **instância:** derivada de magic, altura, `prev_hash` e minerador;
//! - **prova:** `C` inteira, em `u32` big-endian;
//! - **conferência:** faixa de cada entrada e 4 rodadas de Freivalds.
//!
//! Aritmética inteira sem sinal, sem ponto flutuante. Os limites que garantem
//! que nenhuma conta estoura estão escritos ao lado de cada uma.

#![forbid(unsafe_code)]

use std::fmt;

use auron_codec::{CodecError, Reader, Writer};
use auron_crypto::{ADDRESS_LEN, HASH_LEN, sha512, xof};

/// Família `MATRIX-FREIVALDS-V1`.
pub const FAMILY_MATRIX_FREIVALDS: u8 = 1;
/// Versão do formato da prova.
pub const PROOF_VERSION: u16 = 1;

/// Domínio da semente da instância.
pub const DOMAIN_TASK: &[u8] = b"AURON-UPOW-TASK-v1";
/// Domínio do compromisso que vai no cabeçalho (`useful_root`).
pub const DOMAIN_COMMIT: &[u8] = b"AURON-UPOW-PROOF-v1";
/// Domínio do desafio de Freivalds.
pub const DOMAIN_CHALLENGE: &[u8] = b"AURON-UPOW-CHALLENGE-v1";
/// Domínio das matrizes (o mesmo gerador do UTRAX, seção 18).
pub const DOMAIN_INSTANCE: &[u8] = b"AURON-UTRAX-INSTANCE-v1";

/// Bytes por entrada do resultado.
pub const ENTRY_BYTES: usize = 4;
/// Bits de cada coordenada do vetor de desafio.
pub const CHALLENGE_BITS: u32 = 20;
/// Entradas de `A` e `B` ficam em `[0, MATRIX_ENTRY_MAX)`.
pub const MATRIX_ENTRY_MAX: u32 = 1000;
/// Maior lado que o gerador aceita (igual a `utrax.MATRIX_MAX_SIZE`).
pub const MATRIX_MAX_SIZE: u32 = 1024;

/// 2^(k/3) para k = 0, 1, 2, em milésimos.
const RAIZ_CUBICA_DE_2_MILESIMOS: [u64; 3] = [1000, 1260, 1587];

/// Parâmetros de rede que o trabalho útil usa. Espelha `ChainParams`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametrosUteis {
    /// Identificador da rede.
    pub magic: [u8; 4],
    /// Alvo mais fácil permitido, big-endian.
    pub max_target: [u8; 32],
    /// Menor lado exigido.
    pub useful_size_min: u32,
    /// Lado exigido na dificuldade mínima.
    pub useful_size_base: u32,
    /// Maior lado exigido.
    pub useful_size_max: u32,
    /// Rodadas de Freivalds.
    pub useful_rounds: u32,
}

// `max_target` de cada rede, como o gabarito imprime (`ChainParams.max_target`).
const ALVO_MAINNET: [u8; 32] = [
    0x00, 0x00, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const ALVO_TESTNET: [u8; 32] = [
    0x00, 0xff, 0xff, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const ALVO_REGTEST: [u8; 32] = [
    0x3f, 0xff, 0xff, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

impl ParametrosUteis {
    /// Rede principal.
    pub const MAINNET: Self = Self {
        magic: *b"AURM",
        max_target: ALVO_MAINNET,
        useful_size_min: 32,
        useful_size_base: 48,
        useful_size_max: 256,
        useful_rounds: 4,
    };
    /// Rede de teste.
    pub const TESTNET: Self = Self {
        magic: *b"AURT",
        max_target: ALVO_TESTNET,
        useful_size_min: 32,
        useful_size_base: 48,
        useful_size_max: 256,
        useful_rounds: 4,
    };
    /// Rede local de desenvolvimento: matrizes pequenas, mesma regra.
    pub const REGTEST: Self = Self {
        magic: *b"AURR",
        max_target: ALVO_REGTEST,
        useful_size_min: 4,
        useful_size_base: 6,
        useful_size_max: 16,
        useful_rounds: 4,
    };

    /// Parâmetros pelo nome da rede (`auron-mainnet` ou `mainnet`).
    pub fn da_rede(nome: &str) -> Option<Self> {
        match nome.trim_start_matches("auron-") {
            "mainnet" => Some(Self::MAINNET),
            "testnet" => Some(Self::TESTNET),
            "regtest" => Some(Self::REGTEST),
            _ => None,
        }
    }
}

/// Prova de trabalho útil malformada, ou entrada inválida para a tarefa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsefulWorkError {
    /// A codificação da prova não fecha.
    Codec(CodecError),
    /// `prev_hash` com tamanho diferente de 64 bytes.
    PrevHashTamanho,
    /// Endereço do minerador com tamanho diferente de 20 bytes.
    MineradorTamanho,
    /// Lado fora de `[1, MATRIX_MAX_SIZE]`.
    TamanhoDeMatriz(u32),
}

impl fmt::Display for UsefulWorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "{e}"),
            Self::PrevHashTamanho => write!(f, "prev_hash com tamanho inválido"),
            Self::MineradorTamanho => write!(f, "endereço do minerador com tamanho inválido"),
            Self::TamanhoDeMatriz(n) => write!(f, "tamanho de matriz fora da faixa: {n}"),
        }
    }
}

impl std::error::Error for UsefulWorkError {}

impl From<CodecError> for UsefulWorkError {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

/// A prova que viaja no corpo do bloco.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsefulWorkProof {
    /// Família do trabalho.
    pub family: u8,
    /// Lado das matrizes.
    pub n: u32,
    /// `C`, `n·n` entradas `u32` big-endian, linha a linha.
    pub result: Vec<u8>,
    /// Versão do formato.
    pub version: u16,
}

impl UsefulWorkProof {
    /// `u8 family || u16 version || u32 n || var_bytes result`.
    pub fn encode(&self) -> Result<Vec<u8>, UsefulWorkError> {
        let mut w = Writer::new();
        w.u8(self.family);
        w.u16(self.version);
        w.u32(self.n);
        w.var_bytes(&self.result)?;
        Ok(w.into_bytes())
    }

    /// Decodifica exigindo que não sobre byte.
    pub fn decode(dados: &[u8]) -> Result<Self, UsefulWorkError> {
        let mut r = Reader::new(dados);
        let family = r.u8()?;
        let version = r.u16()?;
        let n = r.u32()?;
        let result = r.var_bytes()?.to_vec();
        r.finish()?;
        Ok(Self { family, n, result, version })
    }

    /// `useful_root = SHA-512(DOMAIN_COMMIT || prova codificada)`.
    pub fn commitment(&self) -> Result<[u8; HASH_LEN], UsefulWorkError> {
        let mut dados = DOMAIN_COMMIT.to_vec();
        dados.extend(self.encode()?);
        Ok(sha512(&dados))
    }
}

// ---------------------------------------------------------------------------
// Dificuldade do trabalho útil
// ---------------------------------------------------------------------------

/// `256 − bit_length(alvo)`.
fn bits_de_trabalho(alvo: &[u8; 32]) -> u32 {
    let mut zeros = 0u32;
    for byte in alvo {
        if *byte == 0 {
            zeros = zeros.saturating_add(8);
        } else {
            return zeros.saturating_add(byte.leading_zeros());
        }
    }
    zeros
}

/// Lado `n` exigido para um bloco com este alvo (`useful_work_size`).
pub fn useful_work_size(target: &[u8; 32], p: &ParametrosUteis) -> u32 {
    let delta = bits_de_trabalho(target).saturating_sub(bits_de_trabalho(&p.max_target));
    let dobras = delta / 3;
    let resto = usize::try_from(delta % 3).unwrap_or(0);
    // base · 2^dobras, saturando: acima de 2^32 já passou do máximo de longe
    let n = u64::from(p.useful_size_base)
        .checked_shl(dobras)
        .filter(|v| v >> dobras == u64::from(p.useful_size_base))
        .unwrap_or(u64::MAX);
    let fator = RAIZ_CUBICA_DE_2_MILESIMOS.get(resto).copied().unwrap_or(1000);
    let n = n.saturating_mul(fator) / 1000;
    let limitado = n.clamp(u64::from(p.useful_size_min), u64::from(p.useful_size_max));
    u32::try_from(limitado).unwrap_or(p.useful_size_max)
}

// ---------------------------------------------------------------------------
// Tarefa, solução e conferência
// ---------------------------------------------------------------------------

/// Semente da instância: `H(DOMAIN_TASK || magic || u64 altura || prev_hash || minerador)`.
pub fn task_seed(
    p: &ParametrosUteis,
    height: u64,
    prev_hash: &[u8],
    miner: &[u8],
) -> Result<[u8; HASH_LEN], UsefulWorkError> {
    if prev_hash.len() != HASH_LEN {
        return Err(UsefulWorkError::PrevHashTamanho);
    }
    if miner.len() != ADDRESS_LEN {
        return Err(UsefulWorkError::MineradorTamanho);
    }
    let mut dados = Vec::new();
    dados.extend_from_slice(DOMAIN_TASK);
    dados.extend_from_slice(&p.magic);
    dados.extend_from_slice(&height.to_be_bytes());
    dados.extend_from_slice(prev_hash);
    dados.extend_from_slice(miner);
    Ok(sha512(&dados))
}

/// `count` inteiros em `[0, modulo)`, 4 bytes big-endian cada, do XOF.
fn inteiros_da_semente(semente: &[u8], count: usize, modulo: u32, dominio: &[u8]) -> Vec<u32> {
    let bruto = xof(semente, count.saturating_mul(4), dominio);
    bruto
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| u32::from_be_bytes(c).checked_rem(modulo).unwrap_or(0))
        .collect()
}

/// Matrizes `A` e `B`, linha a linha (`utrax.generate_matrices`).
pub fn generate_matrices(semente: &[u8], n: u32) -> Result<(Vec<u32>, Vec<u32>), UsefulWorkError> {
    if !(1..=MATRIX_MAX_SIZE).contains(&n) {
        return Err(UsefulWorkError::TamanhoDeMatriz(n));
    }
    let lado = n as usize;
    let metade = lado.saturating_mul(lado);
    let mut valores = inteiros_da_semente(semente, metade.saturating_mul(2), MATRIX_ENTRY_MAX, DOMAIN_INSTANCE);
    let b = valores.split_off(metade);
    Ok((valores, b))
}

/// `C = A · B`. Cada entrada é soma de no máximo 1024 produtos de 999·999,
/// menos de 2^30: cabe em `u32`, somada em `u64` por folga.
///
/// Ordem i-k-j: percorre `B` linha a linha, que é a ordem da memória, e deixa a
/// conta bem mais rápida no celular do que a ordem i-j-k dos livros.
fn multiplicar(a: &[u32], b: &[u32], n: usize) -> Vec<u32> {
    let mut acumulado = vec![0u64; n];
    let mut c = Vec::with_capacity(n.saturating_mul(n));
    for linha_a in a.chunks_exact(n) {
        acumulado.fill(0);
        for (&aik, linha_b) in linha_a.iter().zip(b.chunks_exact(n)) {
            let aik = u64::from(aik);
            for (soma, &bkj) in acumulado.iter_mut().zip(linha_b) {
                *soma = soma.wrapping_add(aik.wrapping_mul(u64::from(bkj)));
            }
        }
        c.extend(acumulado.iter().map(|&v| u32::try_from(v).unwrap_or(u32::MAX)));
    }
    c
}

/// `M · r`. Com `M ≤ n·998²` (resultado), `r < 2^20` e `n ≤ 1024`, cada soma
/// fica abaixo de 2^60: cabe em `u64`. Em `A·(B·r)` o termo interno é menor
/// que 2^40 e `A ≤ 999`, então também abaixo de 2^60.
fn aplicar(m: &[u64], r: &[u64], n: usize) -> Vec<u64> {
    m.chunks_exact(n)
        .map(|linha| linha.iter().zip(r).map(|(&x, &y)| x.wrapping_mul(y)).fold(0u64, u64::wrapping_add))
        .collect()
}

/// Faz o trabalho: calcula `C = A · B` com o `n` exigido. Custo O(n³).
pub fn solve(
    p: &ParametrosUteis,
    height: u64,
    prev_hash: &[u8],
    miner: &[u8],
    target: &[u8; 32],
) -> Result<UsefulWorkProof, UsefulWorkError> {
    let n = useful_work_size(target, p);
    let (a, b) = generate_matrices(&task_seed(p, height, prev_hash, miner)?, n)?;
    let c = multiplicar(&a, &b, n as usize);
    Ok(UsefulWorkProof {
        family: FAMILY_MATRIX_FREIVALDS,
        n,
        result: c.iter().flat_map(|v| v.to_be_bytes()).collect(),
        version: PROOF_VERSION,
    })
}

fn desafio(semente: &[u8], result: &[u8], n: usize, rodadas: usize) -> Vec<u32> {
    let mut entrada = semente.to_vec();
    entrada.extend_from_slice(result);
    inteiros_da_semente(&sha512(&entrada), rodadas.saturating_mul(n), 1 << CHALLENGE_BITS, DOMAIN_CHALLENGE)
}

/// Confere a prova sem refazer o trabalho. Custo O(n² · rodadas).
///
/// Devolve `Ok(())` ou `Err(motivo)`, com o mesmo texto do gabarito. A ordem das
/// checagens é a do gabarito, porque ela decide qual motivo aparece.
pub fn verify(
    proof: &UsefulWorkProof,
    p: &ParametrosUteis,
    height: u64,
    prev_hash: &[u8],
    miner: &[u8],
    target: &[u8; 32],
) -> Result<(), String> {
    if proof.family != FAMILY_MATRIX_FREIVALDS {
        return Err(format!("família de trabalho útil desconhecida: {}", proof.family));
    }
    if proof.version != PROOF_VERSION {
        return Err(format!("versão de prova desconhecida: {}", proof.version));
    }
    let esperado = useful_work_size(target, p);
    if proof.n != esperado {
        return Err(format!("tamanho do trabalho {} difere do exigido {esperado}", proof.n));
    }
    let n = esperado as usize;
    if Some(proof.result.len()) != n.checked_mul(n).and_then(|q| q.checked_mul(ENTRY_BYTES)) {
        return Err("resultado com tamanho que não fecha n·n".into());
    }
    let semente = task_seed(p, height, prev_hash, miner).map_err(|e| e.to_string())?;

    let c: Vec<u64> = proof
        .result
        .as_chunks::<ENTRY_BYTES>()
        .0
        .iter()
        .map(|&e| u64::from(u32::from_be_bytes(e)))
        .collect();
    // Faixa antes de qualquer conta: é o que garante os limites de `aplicar`.
    let maior_entrada = u64::from(MATRIX_ENTRY_MAX.saturating_sub(1));
    let limite = u64::from(esperado).saturating_mul(maior_entrada.saturating_mul(maior_entrada));
    if c.iter().any(|&v| v > limite) {
        return Err("resultado fora da faixa possível de um produto honesto".into());
    }

    let (a, b) = generate_matrices(&semente, esperado).map_err(|e| e.to_string())?;
    let a: Vec<u64> = a.into_iter().map(u64::from).collect();
    let b: Vec<u64> = b.into_iter().map(u64::from).collect();
    let rodadas = p.useful_rounds as usize;
    let valores = desafio(&semente, &proof.result, n, rodadas);
    for (numero, r) in (1u32..).zip(valores.chunks_exact(n).take(rodadas)) {
        let r: Vec<u64> = r.iter().copied().map(u64::from).collect();
        if aplicar(&a, &aplicar(&b, &r, n), n) != aplicar(&c, &r, n) {
            return Err(format!("o resultado não confere na rodada {numero} de Freivalds"));
        }
    }
    Ok(())
}
