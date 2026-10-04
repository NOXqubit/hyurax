// ✝ Atos 2:44 — “Todos os que criam estavam juntos e tinham tudo em comum.”
//! Nuvem Hyurax: a nuvem são os próprios nós (`docs/NUVEM.md`).
//!
//! Tradução de `reference/hyurax/nuvem.py`, conferida por
//! `vectors/nuvem.json`; o gabarito descreve cada campo. Fora do consenso:
//! nada aqui muda bloco, transação ou cadeia. Créditos de computação não são
//! dinheiro nem HYX.
//!
//! - [`codigo`]: Reed–Solomon sobre GF(2^8): quaisquer `k` de `k + m`
//!   fragmentos reconstroem o arquivo (já cifrado pelo dono).
//! - [`anuncio`]: a máquina que se oferece no mercado, assinada pelo worker.
//! - [`recibo`]: o cliente assina quanto trabalho verificado o fornecedor fez.
//! - [`mensagem`]: o que viaja entre os nós, tipo de rede [`TIPO_NUVEM`].
//! - [`livro`]: o encadeamento por hash do livro de contas e a partilha de um
//!   consumo entre provedor, plataforma e reserva.

use hyurax_codec::CodecError;

/// Domínio de assinatura e hash da nuvem: `HYURAX-NUVEM-<sufixo>`.
macro_rules! dominio {
    ($sufixo:literal) => {
        concat!(hyurax_identidade::raiz!(), "-NUVEM-", $sufixo).as_bytes()
    };
}

pub mod anuncio;
pub mod codigo;
pub mod livro;
pub mod mensagem;
pub mod recibo;

/// Tipo da mensagem de rede da nuvem ("NV").
pub const TIPO_NUVEM: u16 = 0x4E56;
/// Versão do corpo.
pub const VERSAO: u8 = 1;
/// Domínio da assinatura do anúncio.
pub const DOMINIO_ANUNCIO: &[u8] = dominio!("ANUNCIO-v1");
/// Domínio da prova de guarda.
pub const DOMINIO_PROVA: &[u8] = dominio!("PROVA-v1");
/// Domínio da assinatura do recibo.
pub const DOMINIO_RECIBO: &[u8] = dominio!("RECIBO-v1");
/// Domínio do encadeamento do livro.
pub const DOMINIO_LIVRO: &[u8] = dominio!("LIVRO-v1");
/// Maior pedaço de fragmento numa mensagem.
pub const PARTE_MAX: usize = 256 * 1024;
/// Maior fragmento.
pub const FRAGMENTO_MAX: u32 = 64 * 1024 * 1024;
/// Maior número de fragmentos de dados.
pub const K_MAX: usize = 32;
/// Maior número de fragmentos de paridade.
pub const M_MAX: usize = 32;

/// Dado malformado ou fora da faixa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroNuvem {
    /// Bytes que não fecham.
    Codec(CodecError),
    /// Campo fora da faixa, versão ou subtipo desconhecidos.
    Invalida(&'static str),
}

impl std::fmt::Display for ErroNuvem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "{e}"),
            Self::Invalida(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ErroNuvem {}

impl From<CodecError> for ErroNuvem {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

/// Texto UTF-8 com teto em bytes, como `str` do codec.
pub(crate) fn texto_limitado(w: &mut hyurax_codec::Writer, texto: &str, maximo: usize) -> Result<(), ErroNuvem> {
    if texto.len() > maximo {
        return Err(ErroNuvem::Invalida("texto longo demais"));
    }
    Ok(w.string(texto)?)
}

/// Lê um `str` do codec com teto em bytes.
pub(crate) fn ler_texto(r: &mut hyurax_codec::Reader<'_>, maximo: usize) -> Result<String, ErroNuvem> {
    let t = r.string()?;
    if t.len() > maximo {
        return Err(ErroNuvem::Invalida("texto longo demais"));
    }
    Ok(t.to_string())
}
