// ✝ Jeremias 32:15 — “Ainda se comprarão casas, e campos, e vinhas nesta terra.”
//! Auron — cabeçalho e bloco (seção 7 da AURON-SPEC-01).
//!
//! Tradução de `reference/auron/block.py`, conferida contra `vectors/genesis.json`
//! e `vectors/chain.json`.
//!
//! ```text
//! u16  version = 2
//! u64  height
//! [64] prev_hash
//! [64] merkle_root
//! [64] useful_root   = H("AURON-UPOW-PROOF-v1" || prova)
//! u64  timestamp
//! u32  bits
//! u64  nonce
//!                    total: 222 bytes
//!
//! bloco = cabeçalho || var_bytes(prova útil) || lista de transações
//! ```
//!
//! Dois hashes sobre o mesmo cabeçalho: `block_hash` (SHA-512, barato, é a
//! identidade) e `pow_hash` (Argon2id, caro, é a prova; fica no `auron-pow`).

#![forbid(unsafe_code)]

use std::fmt;

use auron_codec::{CodecError, Reader, Writer, merkle_root};
use auron_crypto::{HASH_LEN, sha512};
use auron_tx::{Coinbase, Tx, TxError, decode_tx_from};
use auron_usefulpow::{UsefulWorkError, UsefulWorkProof};

/// Versão do cabeçalho com `useful_root`.
pub const BLOCK_VERSION: u16 = 2;
/// Tamanho fixo do cabeçalho v2.
pub const HEADER_LEN: usize = 222;
/// `useful_root` de um cabeçalho montado sem prova.
pub const SEM_PROVA: [u8; HASH_LEN] = [0; HASH_LEN];

// O layout do auron-pow (onde ficam `bits` e o nonce) depende deste tamanho.
const _: () = assert!(HEADER_LEN == auron_pow::HEADER_LEN);

/// Bloco que não decodifica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockError {
    /// Leitura binária.
    Codec(CodecError),
    /// Transação malformada.
    Tx(TxError),
    /// Prova de trabalho útil malformada.
    Prova(UsefulWorkError),
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "{e}"),
            Self::Tx(e) => write!(f, "{e}"),
            Self::Prova(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BlockError {}

impl From<CodecError> for BlockError {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}
impl From<TxError> for BlockError {
    fn from(e: TxError) -> Self {
        match e {
            TxError::Codec(c) => Self::Codec(c),
            outro => Self::Tx(outro),
        }
    }
}
impl From<UsefulWorkError> for BlockError {
    fn from(e: UsefulWorkError) -> Self {
        Self::Prova(e)
    }
}

/// Cabeçalho v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockHeader {
    /// Versão do formato.
    pub version: u16,
    /// Altura.
    pub height: u64,
    /// `block_hash` do bloco anterior.
    pub prev_hash: [u8; HASH_LEN],
    /// Raiz de Merkle das transações codificadas por inteiro.
    pub merkle_root: [u8; HASH_LEN],
    /// Compromisso da prova de trabalho útil.
    pub useful_root: [u8; HASH_LEN],
    /// Segundos Unix.
    pub timestamp: u64,
    /// Alvo em forma compacta.
    pub bits: u32,
    /// Nonce do Argon2id.
    pub nonce: u64,
}

impl BlockHeader {
    /// Os 222 bytes do cabeçalho.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut w = Writer::new();
        w.u16(self.version);
        w.u64(self.height);
        w.fixed(&self.prev_hash);
        w.fixed(&self.merkle_root);
        w.fixed(&self.useful_root);
        w.u64(self.timestamp);
        w.u32(self.bits);
        w.u64(self.nonce);
        let bytes = w.into_bytes();
        let mut saida = [0u8; HEADER_LEN];
        // os campos somam exatamente HEADER_LEN; conferido pelo teste de vetor
        for (destino, origem) in saida.iter_mut().zip(bytes) {
            *destino = origem;
        }
        saida
    }

    /// Lê de dentro de um fluxo maior.
    pub fn decode_from(r: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(Self {
            version: r.u16()?,
            height: r.u64()?,
            prev_hash: r.fixed()?,
            merkle_root: r.fixed()?,
            useful_root: r.fixed()?,
            timestamp: r.u64()?,
            bits: r.u32()?,
            nonce: r.u64()?,
        })
    }

    /// Lê um cabeçalho isolado, sem aceitar byte sobrando.
    pub fn decode(dados: &[u8]) -> Result<Self, CodecError> {
        let mut r = Reader::new(dados);
        let h = Self::decode_from(&mut r)?;
        r.finish()?;
        Ok(h)
    }

    /// Identidade do bloco: `SHA-512(cabeçalho)`.
    pub fn block_hash(&self) -> [u8; HASH_LEN] {
        sha512(&self.encode())
    }

    /// Alvo decodificado dos `bits`, com as recusas do consenso.
    pub fn target(&self) -> Result<[u8; 32], auron_pow::ErroPow> {
        auron_pow::alvo_de_bits(self.bits)
    }

    /// Cópia com outro nonce.
    pub fn with_nonce(&self, nonce: u64) -> Self {
        Self { nonce, ..*self }
    }
}

/// Bloco: cabeçalho, prova de trabalho útil e transações.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// Cabeçalho.
    pub header: BlockHeader,
    /// Transações; a primeira precisa ser a coinbase.
    pub transactions: Vec<Tx>,
    /// Prova de trabalho útil. `None` decodifica, e a validação recusa.
    pub useful_proof: Option<UsefulWorkProof>,
}

impl Block {
    /// Codificação canônica.
    pub fn encode(&self) -> Result<Vec<u8>, BlockError> {
        let mut w = Writer::new();
        w.fixed(&self.header.encode());
        let prova = match &self.useful_proof {
            Some(p) => p.encode()?,
            None => Vec::new(),
        };
        w.var_bytes(&prova)?;
        // Lista: contagem u32 e cada transação em sequência, os mesmos bytes
        // de `codec.enc_list`. A contagem é conferida antes de escrever.
        let quantidade = u32::try_from(self.transactions.len())
            .map_err(|_| CodecError::ListaLongaDemais)?;
        w.u32(quantidade);
        let mut saida = w.into_bytes();
        for tx in &self.transactions {
            saida.extend(tx.encode()?);
        }
        Ok(saida)
    }

    /// Lê um bloco inteiro, sem aceitar byte sobrando.
    pub fn decode(dados: &[u8]) -> Result<Self, BlockError> {
        let mut r = Reader::new(dados);
        let header = BlockHeader::decode_from(&mut r)?;
        let bruto = r.var_bytes()?;
        let useful_proof =
            if bruto.is_empty() { None } else { Some(UsefulWorkProof::decode(bruto)?) };
        let transactions = r.read_list(|r| decode_tx_from(r).map_err(BlockError::from))?;
        r.finish()?;
        Ok(Self { header, transactions, useful_proof })
    }

    /// Identidade do bloco.
    pub fn block_hash(&self) -> [u8; HASH_LEN] {
        self.header.block_hash()
    }

    /// Folhas da árvore: cada transação inteira, assinatura inclusive.
    pub fn merkle_leaves(&self) -> Result<Vec<Vec<u8>>, TxError> {
        self.transactions.iter().map(Tx::encode).collect()
    }

    /// Raiz de Merkle calculada das transações.
    pub fn computed_merkle_root(&self) -> Result<[u8; HASH_LEN], TxError> {
        Ok(merkle_root(&self.merkle_leaves()?))
    }

    /// A coinbase, que precisa ser a primeira transação.
    pub fn coinbase(&self) -> Result<&Coinbase, &'static str> {
        match self.transactions.first() {
            None => Err("bloco sem transações"),
            Some(Tx::Coinbase(c)) => Ok(c),
            Some(Tx::Transfer(_)) => Err("a primeira transação não é coinbase"),
        }
    }

    /// Soma das taxas das transferências, saturando no teto do `u64`.
    pub fn total_fees(&self) -> u64 {
        self.transactions
            .iter()
            .skip(1)
            .map(|tx| match tx {
                Tx::Transfer(t) => t.fee,
                Tx::Coinbase(_) => 0,
            })
            .fold(0u64, u64::saturating_add)
    }
}
