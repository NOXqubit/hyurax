// ✝ Provérbios 25:25 — “Como água fresca para a alma sedenta, tais são as boas novas de terra distante.”
//! Auron — formato das mensagens da rede entre nós (`AURON-WIRE-v1`, seção 21).
//!
//! Só o **formato** e as **regras de decisão** são consenso de rede: dois nós
//! precisam concordar bit a bit sobre o que é um quadro válido. O transporte
//! (sockets, threads) fica de fora e não está aqui.
//!
//! Conferido contra `vectors/wire.json`. Este crate não faz entrada nem saída;
//! decodifica e codifica bytes que outra camada recebe e envia.
//!
//! ```text
//! quadro = [4] magic || u16 versão || u16 tipo || u32 tamanho || [tamanho] corpo
//! ```
//!
//! O tamanho vem antes do corpo e é conferido contra [`MAX_FRAME_BODY`] ANTES de
//! alocar: um quadro que anuncia 4 GiB é recusado sem reservar 4 GiB.

#![forbid(unsafe_code)]

use std::fmt;

use auron_block::{Block, BlockError, BlockHeader, HEADER_LEN};
use auron_codec::{CodecError, Reader, Writer};
use auron_tx::{Transfer, Tx, TxError};

/// Versão do protocolo de rede.
pub const PROTOCOL_VERSION: u16 = 1;
/// Maior corpo de quadro aceito: 2 MiB.
pub const MAX_FRAME_BODY: u32 = 2 * 1024 * 1024;
/// Teto de cabeçalhos numa mensagem `HEADERS`.
pub const MAX_HEADERS: u32 = 2000;
/// Teto de endereços numa mensagem `ADDRS`.
pub const MAX_ADDRS: u32 = 1000;
/// Teto de hashes num `GET_BLOCKS`.
pub const MAX_GET_BLOCKS: u32 = 2000;

const TIPO_HELLO: u16 = 1;
const TIPO_HELLO_ACK: u16 = 2;
const TIPO_GET_HEADERS: u16 = 3;
const TIPO_HEADERS: u16 = 4;
const TIPO_GET_BLOCKS: u16 = 5;
const TIPO_BLOCK: u16 = 6;
const TIPO_GET_ADDRS: u16 = 7;
const TIPO_ADDRS: u16 = 8;
const TIPO_TX: u16 = 9;
const TIPO_PING: u16 = 10;
const TIPO_PONG: u16 = 11;

/// Hash de bloco (SHA-512).
pub type Hash = [u8; 64];

/// Recusa ao ler um quadro. O texto é o que os vetores gravam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// Faltam bytes para o cabeçalho ou o corpo do quadro.
    QuadroIncompleto,
    /// Magic de outra rede.
    MagicErrado,
    /// Versão de protocolo que este nó não fala.
    VersaoDesconhecida(u16),
    /// Corpo maior que [`MAX_FRAME_BODY`].
    CorpoGrandeDemais,
    /// O corpo não casa com o formato do tipo declarado.
    CorpoInvalido(String),
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuadroIncompleto => f.write_str("quadro incompleto"),
            Self::MagicErrado => f.write_str("magic da rede não confere"),
            Self::VersaoDesconhecida(v) => write!(f, "versão de protocolo desconhecida: {v}"),
            Self::CorpoGrandeDemais => f.write_str("corpo do quadro maior que o máximo"),
            Self::CorpoInvalido(m) => write!(f, "corpo de mensagem inválido: {m}"),
        }
    }
}

impl std::error::Error for WireError {}

impl From<CodecError> for WireError {
    fn from(e: CodecError) -> Self {
        Self::CorpoInvalido(e.to_string())
    }
}
impl From<TxError> for WireError {
    fn from(e: TxError) -> Self {
        Self::CorpoInvalido(e.to_string())
    }
}
impl From<BlockError> for WireError {
    fn from(e: BlockError) -> Self {
        Self::CorpoInvalido(e.to_string())
    }
}

/// A ponta de um nó, anunciada no aperto de mão.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ponta {
    /// Versão de protocolo do nó.
    pub protocolo: u16,
    /// Magic da rede do nó.
    pub magic: [u8; 4],
    /// Altura da ponta.
    pub altura: u64,
    /// Trabalho acumulado, 32 bytes big-endian.
    pub trabalho: [u8; 32],
    /// Nonce aleatório da conexão.
    pub nonce: u64,
}

/// Um endereço de outro nó, para descoberta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnderecoDeRede {
    /// 4 (IPv4) ou 6 (IPv6).
    pub familia: u8,
    /// Bytes do IP.
    pub ip: Vec<u8>,
    /// Porta.
    pub porta: u16,
    /// Quando foi visto pela última vez (segundos Unix).
    pub visto_em: u64,
}

/// Uma mensagem da rede.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// Abre a conversa.
    Hello(Ponta),
    /// Fecha o aperto de mão, ecoando o nonce recebido.
    HelloAck {
        /// A ponta de quem responde.
        ponta: Ponta,
        /// Eco do nonce que veio no `Hello`.
        eco: u64,
    },
    /// Pede cabeçalhos a partir de um hash conhecido.
    GetHeaders {
        /// Último hash em comum.
        inicio: Hash,
        /// Quantos cabeçalhos, no máximo.
        quantidade: u32,
    },
    /// Cabeçalhos em resposta.
    Headers(Vec<BlockHeader>),
    /// Pede os blocos inteiros destes hashes.
    GetBlocks(Vec<Hash>),
    /// Um bloco, como resposta ou anúncio.
    Block(Box<Block>),
    /// Pede endereços de outros nós.
    GetAddrs,
    /// Endereços em resposta.
    Addrs(Vec<EnderecoDeRede>),
    /// Espalha uma transação para o mempool.
    Tx(Box<Transfer>),
    /// Vivo?
    Ping(u64),
    /// Resposta ao ping.
    Pong(u64),
    /// Tipo que este nó não conhece: guardado para ser ignorado, não recusado.
    Desconhecida {
        /// O número do tipo.
        tipo: u16,
        /// O corpo, sem interpretar.
        corpo: Vec<u8>,
    },
}

fn escreve_ponta(w: &mut Writer, p: &Ponta) {
    w.u16(p.protocolo);
    w.fixed(&p.magic);
    w.u64(p.altura);
    w.fixed(&p.trabalho);
    w.u64(p.nonce);
}

fn le_ponta(r: &mut Reader<'_>) -> Result<Ponta, WireError> {
    Ok(Ponta {
        protocolo: r.u16()?,
        magic: r.fixed()?,
        altura: r.u64()?,
        trabalho: r.fixed()?,
        nonce: r.u64()?,
    })
}

impl Message {
    fn tipo(&self) -> u16 {
        match self {
            Self::Hello(_) => TIPO_HELLO,
            Self::HelloAck { .. } => TIPO_HELLO_ACK,
            Self::GetHeaders { .. } => TIPO_GET_HEADERS,
            Self::Headers(_) => TIPO_HEADERS,
            Self::GetBlocks(_) => TIPO_GET_BLOCKS,
            Self::Block(_) => TIPO_BLOCK,
            Self::GetAddrs => TIPO_GET_ADDRS,
            Self::Addrs(_) => TIPO_ADDRS,
            Self::Tx(_) => TIPO_TX,
            Self::Ping(_) => TIPO_PING,
            Self::Pong(_) => TIPO_PONG,
            Self::Desconhecida { tipo, .. } => *tipo,
        }
    }

    /// O corpo da mensagem (sem o cabeçalho do quadro).
    pub fn corpo(&self) -> Result<Vec<u8>, WireError> {
        let mut w = Writer::new();
        match self {
            Self::Hello(p) => escreve_ponta(&mut w, p),
            Self::HelloAck { ponta, eco } => {
                escreve_ponta(&mut w, ponta);
                w.u64(*eco);
            }
            Self::GetHeaders { inicio, quantidade } => {
                w.fixed(inicio);
                w.u32(*quantidade);
            }
            Self::Headers(cabecalhos) => {
                w.list(cabecalhos, |w, c| {
                    w.raw(&c.encode());
                    Ok::<(), CodecError>(())
                })?;
            }
            Self::GetBlocks(hashes) => {
                w.list(hashes, |w, hs| {
                    w.fixed(hs);
                    Ok::<(), CodecError>(())
                })?;
            }
            Self::Block(bloco) => w.raw(&bloco.encode()?),
            Self::GetAddrs => {}
            Self::Addrs(enderecos) => {
                w.list(enderecos, |w, a| {
                    w.u8(a.familia);
                    w.var_bytes(&a.ip)?;
                    w.u16(a.porta);
                    w.u64(a.visto_em);
                    Ok::<(), CodecError>(())
                })?;
            }
            Self::Tx(tx) => w.raw(&tx.encode()?),
            Self::Ping(n) | Self::Pong(n) => w.u64(*n),
            Self::Desconhecida { corpo, .. } => w.raw(corpo),
        }
        Ok(w.into_bytes())
    }

    fn do_corpo(tipo: u16, corpo: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader::new(corpo);
        let msg = match tipo {
            TIPO_HELLO => Self::Hello(le_ponta(&mut r)?),
            TIPO_HELLO_ACK => Self::HelloAck { ponta: le_ponta(&mut r)?, eco: r.u64()? },
            TIPO_GET_HEADERS => Self::GetHeaders { inicio: r.fixed()?, quantidade: r.u32()? },
            TIPO_HEADERS => {
                let brutos = r.read_list(|r| r.take(HEADER_LEN).map(<[u8]>::to_vec))?;
                let mut cabecalhos = Vec::with_capacity(brutos.len());
                for bruto in brutos {
                    cabecalhos.push(BlockHeader::decode(&bruto)?);
                }
                Self::Headers(cabecalhos)
            }
            TIPO_GET_BLOCKS => Self::GetBlocks(r.read_list(|r| r.fixed::<64>())?),
            TIPO_BLOCK => {
                // O corpo inteiro é o bloco; o próprio Block::decode já exige
                // que feche sem sobra.
                Self::Block(Box::new(Block::decode(corpo).map_err(|e| WireError::CorpoInvalido(e.to_string()))?))
            }
            TIPO_GET_ADDRS => Self::GetAddrs,
            TIPO_ADDRS => Self::Addrs(r.read_list(|r| {
                Ok::<_, CodecError>(EnderecoDeRede {
                    familia: r.u8()?,
                    ip: r.var_bytes()?.to_vec(),
                    porta: r.u16()?,
                    visto_em: r.u64()?,
                })
            })?),
            TIPO_TX => {
                // decode_tx já exige que o corpo inteiro seja a transação.
                match auron_tx::decode_tx(corpo)? {
                    Tx::Transfer(t) => Self::Tx(Box::new(t)),
                    Tx::Coinbase(_) => {
                        return Err(WireError::CorpoInvalido("coinbase não se espalha na rede".into()));
                    }
                }
            }
            TIPO_PING => Self::Ping(r.u64()?),
            TIPO_PONG => Self::Pong(r.u64()?),
            outro => return Ok(Self::Desconhecida { tipo: outro, corpo: corpo.to_vec() }),
        };
        // Os tipos conhecidos têm formato exato: sobra de bytes é recusa.
        if !matches!(msg, Self::Block(_) | Self::Tx(_)) {
            r.finish()?;
        }
        Ok(msg)
    }
}

/// Serializa uma mensagem num quadro pronto para a rede.
pub fn encode_frame(magic: &[u8; 4], msg: &Message) -> Result<Vec<u8>, WireError> {
    let corpo = msg.corpo()?;
    let tamanho = u32::try_from(corpo.len()).map_err(|_| WireError::CorpoGrandeDemais)?;
    if tamanho > MAX_FRAME_BODY {
        return Err(WireError::CorpoGrandeDemais);
    }
    let mut w = Writer::new();
    w.fixed(magic);
    w.u16(PROTOCOL_VERSION);
    w.u16(msg.tipo());
    w.u32(tamanho);
    let mut saida = w.into_bytes();
    saida.extend(corpo);
    Ok(saida)
}

/// O que sobra depois de ler um quadro do começo de um fluxo.
#[derive(Debug)]
pub struct QuadroLido<'a> {
    /// A mensagem decodificada.
    pub message: Message,
    /// Os bytes ainda não consumidos (o próximo quadro, se houver).
    pub resto: &'a [u8],
}

/// Lê um quadro do começo de `dados`, conferindo magic, versão e teto de
/// tamanho ANTES de tocar no corpo.
///
/// `QuadroIncompleto` não é erro fatal: significa "faltam bytes, espere mais da
/// rede". Os outros erros derrubam a conexão.
pub fn decode_frame<'a>(esperado_magic: &[u8; 4], dados: &'a [u8]) -> Result<QuadroLido<'a>, WireError> {
    // cabeçalho do quadro: 4 + 2 + 2 + 4 = 12 bytes
    let (cabecalho, apos_cabecalho) = dados.split_at_checked(12).ok_or(WireError::QuadroIncompleto)?;
    let mut r = Reader::new(cabecalho);
    let magic: [u8; 4] = r.fixed()?;
    if &magic != esperado_magic {
        return Err(WireError::MagicErrado);
    }
    let versao = r.u16()?;
    if versao != PROTOCOL_VERSION {
        return Err(WireError::VersaoDesconhecida(versao));
    }
    let tipo = r.u16()?;
    let tamanho = r.u32()?;
    if tamanho > MAX_FRAME_BODY {
        return Err(WireError::CorpoGrandeDemais);
    }
    let (corpo, resto) = apos_cabecalho
        .split_at_checked(tamanho as usize)
        .ok_or(WireError::QuadroIncompleto)?;
    Ok(QuadroLido { message: Message::do_corpo(tipo, corpo)?, resto })
}
