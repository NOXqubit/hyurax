// ✝ Provérbios 6:6 — “Vai ter com a formiga, ó preguiçoso; olha para os seus caminhos e sê sábio.”
//! Auron — prova de trabalho Argon2id (seções 8 e 9 da AURON-SPEC-01).
//!
//! Este crate existe para o **celular minerar**. O gabarito em Python leva
//! minutos por avaliação de 32 MiB; aqui a mesma conta sai em uma fração de
//! segundo, e o código compila no Termux sem compilador C.
//!
//! Três escolhas pensando no celular:
//!
//! 1. **Memória alocada uma vez.** Cada tentativa usa 32 MiB. Pedir e devolver
//!    32 MiB ao sistema a cada nonce desperdiça tempo e fragmenta a memória de
//!    um aparelho que tem pouca. A [`Calculadora`] guarda o bloco de memória e
//!    reaproveita.
//! 2. **Uma linha de execução por núcleo escolhido.** O parâmetro `faixas`
//!    do consenso é 1, então o ganho vem de tentar nonces diferentes em
//!    paralelo. Cada linha gasta 32 MiB; quem minera escolhe quantas.
//! 3. **Parada e pausa.** [`minerar`] obedece a uma bandeira de parada e aceita
//!    uma pausa entre tentativas, para o aparelho não esquentar.
//!
//! O que este crate NÃO faz: validar bloco. Ele calcula e confere o Argon2id de
//! um cabeçalho já montado. A prova de trabalho útil (seção 9A) e o resto da
//! validação continuam no gabarito até serem migrados.

#![forbid(unsafe_code)]

use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use argon2::{Algorithm, Argon2, Block, Params, Version};

/// Sal fixo e público do PoW: `"AURON-POW-v1"` completado com zeros até 16 bytes.
pub const POW_SALT: [u8; 16] = *b"AURON-POW-v1\0\0\0\0";

/// Tamanho do `pow_hash`, em bytes.
pub const POW_HASH_LEN: usize = 32;

/// Tamanho do cabeçalho v2 (seção 7).
pub const HEADER_LEN: usize = 222;

/// O nonce é o último campo do cabeçalho: `u64` big-endian.
pub const NONCE_OFFSET: usize = HEADER_LEN - 8;

/// O campo `bits` fica logo antes do nonce: `u32` big-endian.
pub const BITS_OFFSET: usize = NONCE_OFFSET - 4;

/// Parâmetros do Argon2id de uma rede. Espelha `ChainParams.pow_*` do gabarito.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametrosPow {
    /// Memória por tentativa, em KiB.
    pub memoria_kib: u32,
    /// Passadas sobre a memória.
    pub tempo: u32,
    /// Faixas (lanes) do Argon2id. É consenso; não é o número de núcleos.
    pub faixas: u32,
}

impl ParametrosPow {
    /// Rede principal: 32 MiB.
    pub const MAINNET: Self = Self { memoria_kib: 32 * 1024, tempo: 1, faixas: 1 };
    /// Rede de teste: 32 MiB.
    pub const TESTNET: Self = Self { memoria_kib: 32 * 1024, tempo: 1, faixas: 1 };
    /// Rede local de desenvolvimento: 32 KiB, só para os testes andarem.
    pub const REGTEST: Self = Self { memoria_kib: 32, tempo: 1, faixas: 1 };

    /// Parâmetros pelo nome da rede, como aparece nos vetores (`auron-mainnet`)
    /// ou curto (`mainnet`).
    pub fn da_rede(nome: &str) -> Option<Self> {
        match nome.trim_start_matches("auron-") {
            "mainnet" => Some(Self::MAINNET),
            "testnet" => Some(Self::TESTNET),
            "regtest" => Some(Self::REGTEST),
            _ => None,
        }
    }

    /// Memória que uma linha de mineração ocupa, em bytes.
    pub fn memoria_bytes(&self) -> u64 {
        u64::from(self.memoria_kib).saturating_mul(1024)
    }
}

/// Erro da prova de trabalho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroPow {
    /// O Argon2id recusou os parâmetros ou a entrada.
    Argon2(String),
    /// Cabeçalho com tamanho diferente de [`HEADER_LEN`].
    TamanhoDoCabecalho {
        /// Quantos bytes vieram.
        veio: usize,
    },
    /// `bits` que não representa um alvo válido e canônico.
    AlvoCompacto(&'static str),
    /// Não sobrou memória para mais uma linha de mineração.
    SemMemoria,
}

impl fmt::Display for ErroPow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argon2(e) => write!(f, "Argon2id recusou a entrada: {e}"),
            Self::TamanhoDoCabecalho { veio } => {
                write!(f, "cabeçalho deve ter {HEADER_LEN} bytes, veio {veio}")
            }
            Self::AlvoCompacto(motivo) => write!(f, "{motivo}"),
            Self::SemMemoria => write!(f, "sem memória para a linha de mineração"),
        }
    }
}

impl std::error::Error for ErroPow {}

fn erro_argon2(e: argon2::Error) -> ErroPow {
    ErroPow::Argon2(e.to_string())
}

// ---------------------------------------------------------------------------
// Argon2id
// ---------------------------------------------------------------------------

/// Calcula `pow_hash` reaproveitando a mesma memória a cada chamada.
pub struct Calculadora {
    argon: Argon2<'static>,
    memoria: Vec<Block>,
}

impl Calculadora {
    /// Prepara a calculadora e reserva a memória de uma tentativa.
    pub fn nova(parametros: ParametrosPow) -> Result<Self, ErroPow> {
        let params = Params::new(
            parametros.memoria_kib,
            parametros.tempo,
            parametros.faixas,
            Some(POW_HASH_LEN),
        )
        .map_err(erro_argon2)?;
        let blocos = params.block_count();
        let mut memoria = Vec::new();
        // try_reserve: num celular sem memória, é erro que se mostra, não
        // o processo morto pelo sistema.
        memoria.try_reserve_exact(blocos).map_err(|_| ErroPow::SemMemoria)?;
        memoria.resize(blocos, Block::new());
        Ok(Self { argon: Argon2::new(Algorithm::Argon2id, Version::V0x13, params), memoria })
    }

    /// `pow_hash = Argon2id(cabeçalho, POW_SALT)`, 32 bytes.
    pub fn pow_hash(&mut self, cabecalho: &[u8]) -> Result<[u8; POW_HASH_LEN], ErroPow> {
        if cabecalho.len() != HEADER_LEN {
            return Err(ErroPow::TamanhoDoCabecalho { veio: cabecalho.len() });
        }
        let mut saida = [0u8; POW_HASH_LEN];
        self.argon
            .hash_password_into_with_memory(cabecalho, &POW_SALT, &mut saida, &mut self.memoria)
            .map_err(erro_argon2)?;
        Ok(saida)
    }
}

/// O hash bate o alvo? Comparação numérica: os dois são inteiros de 256 bits
/// em big-endian, e para arrays do mesmo tamanho a ordem dos bytes é a ordem
/// dos números.
pub fn bate_alvo(hash: &[u8; POW_HASH_LEN], alvo: &[u8; POW_HASH_LEN]) -> bool {
    hash <= alvo
}

// ---------------------------------------------------------------------------
// Alvo compacto (seção 8)
// ---------------------------------------------------------------------------

/// Desempacota `bits` num alvo de 256 bits, com as mesmas recusas do gabarito
/// (`consensus.compact_to_target`), inclusive a exigência de forma canônica.
pub fn alvo_de_bits(bits: u32) -> Result<[u8; POW_HASH_LEN], ErroPow> {
    let [tamanho, m0, m1, m2] = bits.to_be_bytes();
    if m0 & 0x80 != 0 {
        return Err(ErroPow::AlvoCompacto("bit de sinal ligado no alvo compacto"));
    }
    if (m0, m1, m2) == (0, 0, 0) {
        return Err(ErroPow::AlvoCompacto("mantissa zero"));
    }

    // valor = mantissa · 256^(tamanho − 3). O byte k da mantissa (k = 0 é o
    // mais alto) cai na posição, contada do fim, tamanho − 1 − k. Posição
    // negativa é o deslocamento para a direita descartando o byte; posição
    // de 32 em diante com byte diferente de zero não cabe em 256 bits.
    let mut alvo = [0u8; POW_HASH_LEN];
    const ULTIMO_INDICE: i64 = 31;
    for (k, byte) in [0i64, 1, 2].into_iter().zip([m0, m1, m2]) {
        let posicao = i64::from(tamanho).saturating_sub(1).saturating_sub(k);
        if byte == 0 || posicao < 0 {
            continue;
        }
        // posição acima de 31 dá índice negativo, e o try_from recusa
        let indice = ULTIMO_INDICE.saturating_sub(posicao);
        match usize::try_from(indice).ok().and_then(|i| alvo.get_mut(i)) {
            Some(celula) => *celula = byte,
            None => return Err(ErroPow::AlvoCompacto("alvo fora da faixa")),
        }
    }
    if alvo == [0u8; POW_HASH_LEN] {
        return Err(ErroPow::AlvoCompacto("alvo fora da faixa"));
    }
    if bits_de_alvo(&alvo) != bits {
        return Err(ErroPow::AlvoCompacto("alvo compacto não está na forma canônica"));
    }
    Ok(alvo)
}

/// Empacota um alvo não nulo em `bits` (`consensus.target_to_compact`).
fn bits_de_alvo(alvo: &[u8; POW_HASH_LEN]) -> u32 {
    let inicio = alvo.iter().position(|&b| b != 0).unwrap_or(POW_HASH_LEN);
    let significativos = alvo.get(inicio..).unwrap_or(&[]);
    let mut tamanho = significativos.len();
    let mut mantissa = [0u8; 3];
    let alto_ligado = significativos.first().is_some_and(|b| b & 0x80 != 0);
    if alto_ligado {
        // bit alto colidiria com o sinal: empurra um byte de zero na frente
        tamanho = tamanho.saturating_add(1);
        for (destino, origem) in mantissa.iter_mut().skip(1).zip(significativos) {
            *destino = *origem;
        }
    } else {
        for (destino, origem) in mantissa.iter_mut().zip(significativos) {
            *destino = *origem;
        }
    }
    let [a, b, c] = mantissa;
    u32::from_be_bytes([u8::try_from(tamanho).unwrap_or(u8::MAX), a, b, c])
}

/// Lê o campo `bits` de um cabeçalho v2.
pub fn bits_do_cabecalho(cabecalho: &[u8]) -> Result<u32, ErroPow> {
    let campo = cabecalho
        .get(BITS_OFFSET..NONCE_OFFSET)
        .filter(|_| cabecalho.len() == HEADER_LEN)
        .ok_or(ErroPow::TamanhoDoCabecalho { veio: cabecalho.len() })?;
    let mut b = [0u8; 4];
    b.copy_from_slice(campo);
    Ok(u32::from_be_bytes(b))
}

/// Número médio de tentativas para bater o alvo: `2^256 / (alvo + 1)`.
///
/// Em ponto flutuante de propósito: serve só para mostrar estimativa de tempo
/// na tela. Nada de consenso passa por aqui.
pub fn tentativas_esperadas(alvo: &[u8; POW_HASH_LEN]) -> f64 {
    let valor = alvo.iter().fold(0f64, |acc, &b| acc * 256.0 + f64::from(b));
    2f64.powi(256) / (valor + 1.0)
}

// ---------------------------------------------------------------------------
// Busca de nonce
// ---------------------------------------------------------------------------

/// Como a busca deve se comportar.
#[derive(Clone, Copy, Debug)]
pub struct ConfigMineracao {
    /// Linhas de execução. Cada uma usa a memória de uma tentativa.
    pub linhas: u32,
    /// Primeiro nonce da busca.
    pub nonce_inicial: u64,
    /// Máximo de tentativas somando todas as linhas (`None` = sem limite).
    pub limite: Option<u64>,
    /// Pausa depois de cada tentativa, para o aparelho não esquentar.
    pub pausa: Duration,
}

/// Nonce encontrado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Achado {
    /// O nonce que bate o alvo.
    pub nonce: u64,
    /// O `pow_hash` do cabeçalho com esse nonce.
    pub hash: [u8; POW_HASH_LEN],
}

/// Resultado de uma busca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resultado {
    /// `Some` se achou.
    pub achado: Option<Achado>,
    /// Tentativas feitas por todas as linhas.
    pub tentativas: u64,
}

/// Escreve o nonce no cabeçalho.
pub fn com_nonce(cabecalho: &mut [u8], nonce: u64) -> Result<(), ErroPow> {
    let veio = cabecalho.len();
    let campo = cabecalho
        .get_mut(NONCE_OFFSET..HEADER_LEN)
        .filter(|_| veio == HEADER_LEN)
        .ok_or(ErroPow::TamanhoDoCabecalho { veio })?;
    campo.copy_from_slice(&nonce.to_be_bytes());
    Ok(())
}

/// Procura um nonce que faça o `pow_hash` bater o alvo do próprio cabeçalho.
///
/// A linha `i` de `n` tenta `nonce_inicial + i`, `+ i + n`, `+ i + 2n`...,
/// então duas linhas nunca repetem trabalho. Com uma linha só a busca é
/// sequencial a partir de `nonce_inicial`, igual ao `Chain.mine` do gabarito.
///
/// `parar` pode ser ligada de fora (outra thread, sinal do sistema); a busca
/// termina na tentativa seguinte. `progresso` recebe a contagem ao vivo.
pub fn minerar(
    cabecalho: &[u8],
    parametros: ParametrosPow,
    config: ConfigMineracao,
    parar: &AtomicBool,
    progresso: &AtomicU64,
) -> Result<Resultado, ErroPow> {
    if cabecalho.len() != HEADER_LEN {
        return Err(ErroPow::TamanhoDoCabecalho { veio: cabecalho.len() });
    }
    let alvo = alvo_de_bits(bits_do_cabecalho(cabecalho)?)?;
    let linhas = config.linhas.max(1);

    // Reserva a memória de todas as linhas ANTES de começar: se não couber,
    // o erro aparece na hora, e não depois de minutos minerando.
    let calculadoras = (0..linhas)
        .map(|_| Calculadora::nova(parametros))
        .collect::<Result<Vec<_>, _>>()?;

    let achado: Mutex<Option<Achado>> = Mutex::new(None);
    let falha: Mutex<Option<ErroPow>> = Mutex::new(None);
    let parar_local = AtomicBool::new(false);
    let deve_parar = || parar.load(Ordering::Relaxed) || parar_local.load(Ordering::Relaxed);

    std::thread::scope(|escopo| {
        for (i, mut calc) in (0u64..).zip(calculadoras) {
            let (achado, falha, parar_local, deve_parar) = (&achado, &falha, &parar_local, &deve_parar);
            let mut meu = cabecalho.to_vec();
            escopo.spawn(move || {
                let mut nonce = config.nonce_inicial.wrapping_add(i);
                while !deve_parar() {
                    let feitas = progresso.fetch_add(1, Ordering::Relaxed).saturating_add(1);
                    if config.limite.is_some_and(|lim| feitas > lim) {
                        progresso.fetch_sub(1, Ordering::Relaxed);
                        parar_local.store(true, Ordering::Relaxed);
                        break;
                    }
                    let tentativa = com_nonce(&mut meu, nonce).and_then(|()| calc.pow_hash(&meu));
                    match tentativa {
                        Ok(hash) if bate_alvo(&hash, &alvo) => {
                            if let Ok(mut vaga) = achado.lock() {
                                // com várias linhas, fica o menor nonce achado
                                if vaga.is_none_or(|a| nonce < a.nonce) {
                                    *vaga = Some(Achado { nonce, hash });
                                }
                            }
                            parar_local.store(true, Ordering::Relaxed);
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            if let Ok(mut vaga) = falha.lock() {
                                vaga.get_or_insert(e);
                            }
                            parar_local.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                    nonce = nonce.wrapping_add(u64::from(linhas));
                    if !config.pausa.is_zero() {
                        std::thread::sleep(config.pausa);
                    }
                }
            });
        }
    });

    if let Some(e) = falha.into_inner().ok().flatten() {
        return Err(e);
    }
    Ok(Resultado {
        achado: achado.into_inner().ok().flatten(),
        tentativas: progresso.load(Ordering::Relaxed),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn alvo_compacto_recusa_forma_invalida() {
        assert_eq!(
            alvo_de_bits(0x0480_0000),
            Err(ErroPow::AlvoCompacto("bit de sinal ligado no alvo compacto"))
        );
        assert_eq!(alvo_de_bits(0x0400_0000), Err(ErroPow::AlvoCompacto("mantissa zero")));
        // 0x0100_0001 dá alvo zero depois do deslocamento
        assert_eq!(alvo_de_bits(0x0100_0001), Err(ErroPow::AlvoCompacto("alvo fora da faixa")));
        // 0x0200_1234 representa 0x12, mas a forma canônica é 0x0112_0000
        assert_eq!(
            alvo_de_bits(0x0200_1234),
            Err(ErroPow::AlvoCompacto("alvo compacto não está na forma canônica"))
        );
        // 34 bytes de tamanho com mantissa cheia não cabem em 256 bits
        assert_eq!(alvo_de_bits(0x2201_0000), Err(ErroPow::AlvoCompacto("alvo fora da faixa")));
    }

    #[test]
    fn nonce_vai_no_fim_do_cabecalho() {
        let mut c = vec![0u8; HEADER_LEN];
        com_nonce(&mut c, 0x0102_0304_0506_0708).unwrap();
        assert_eq!(&c[NONCE_OFFSET..], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(com_nonce(&mut c[..10], 1).is_err());
    }

    #[test]
    fn limite_de_tentativas_e_respeitado() {
        let mut c = vec![0u8; HEADER_LEN];
        // alvo mínimo (1): na prática nunca bate
        c[BITS_OFFSET..NONCE_OFFSET].copy_from_slice(&0x0101_0000u32.to_be_bytes());
        let config = ConfigMineracao {
            linhas: 3,
            nonce_inicial: 0,
            limite: Some(10),
            pausa: Duration::ZERO,
        };
        let r = minerar(&c, ParametrosPow::REGTEST, config, &AtomicBool::new(false), &AtomicU64::new(0))
            .unwrap();
        assert_eq!(r.achado, None);
        assert_eq!(r.tentativas, 10);
    }
}
