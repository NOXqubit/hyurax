//! As mensagens da nuvem entre os nós, tipo de rede [`crate::TIPO_NUVEM`].
//! Corpo: `u8 VERSAO || u8 subtipo || campos`, sem sobra. Quem manda já
//! provou a identidade no aperto Noise: só o dono (a identidade que mandou
//! `Guardar`) busca, desafia ou apaga um fragmento.

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::HASH_LEN;

use crate::anuncio::Anuncio;
use crate::recibo::Recibo;
use crate::{ErroNuvem, FRAGMENTO_MAX, K_MAX, M_MAX, PARTE_MAX, VERSAO, ler_texto, texto_limitado};

/// Maior motivo de recusa, em bytes.
pub const MOTIVO_MAX: usize = 200;

/// O fragmento de que a mensagem fala: arquivo e índice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Alvo {
    /// Identificador do arquivo (sorteado pelo dono).
    pub arquivo: [u8; 32],
    /// Índice do fragmento (`0..k+m`).
    pub indice: u8,
}

/// Uma mensagem da nuvem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MensagemNuvem {
    /// Uma máquina no mercado.
    Anuncio(Box<Anuncio>),
    /// "Vou mandar este fragmento; guarde-o para mim."
    Guardar {
        /// O fragmento.
        alvo: Alvo,
        /// Tamanho, em bytes.
        tamanho: u32,
        /// SHA-512 do fragmento.
        hash: [u8; HASH_LEN],
    },
    /// Um pedaço do fragmento, nos dois sentidos.
    Parte {
        /// O fragmento.
        alvo: Alvo,
        /// Onde o pedaço começa.
        deslocamento: u32,
        /// Os bytes (até [`PARTE_MAX`]).
        dados: Vec<u8>,
    },
    /// Resultado da guarda.
    Guardado {
        /// O fragmento.
        alvo: Alvo,
        /// Guardou e conferiu o hash.
        ok: bool,
        /// Por que não.
        motivo: String,
    },
    /// "Prove que ainda guarda o fragmento."
    Desafio {
        /// O fragmento.
        alvo: Alvo,
        /// Sorteado por quem desafia.
        nonce: [u8; 32],
    },
    /// A prova de guarda.
    Prova {
        /// O fragmento.
        alvo: Alvo,
        /// O nonce do desafio.
        nonce: [u8; 32],
        /// `H(DOMINIO_PROVA || nonce || fragmento)`.
        h: [u8; HASH_LEN],
    },
    /// "Me devolva o fragmento."
    Buscar(Alvo),
    /// Resposta ao `Buscar`; com `tem`, as partes vêm em seguida.
    Entrega {
        /// O fragmento.
        alvo: Alvo,
        /// Ainda guarda.
        tem: bool,
        /// Tamanho, em bytes.
        tamanho: u32,
        /// SHA-512 do fragmento.
        hash: [u8; HASH_LEN],
    },
    /// "Pode apagar."
    Apagar(Alvo),
    /// Recibo de aluguel.
    Recibo(Box<Recibo>),
}

const ANUNCIO: u8 = 1;
const GUARDAR: u8 = 2;
const PARTE: u8 = 3;
const GUARDADO: u8 = 4;
const DESAFIO: u8 = 5;
const PROVA: u8 = 6;
const BUSCAR: u8 = 7;
const ENTREGA: u8 = 8;
const APAGAR: u8 = 9;
const RECIBO: u8 = 10;

fn escrever_alvo(w: &mut Writer, a: &Alvo) -> Result<(), ErroNuvem> {
    if usize::from(a.indice) >= K_MAX.saturating_add(M_MAX) {
        return Err(ErroNuvem::Invalida("índice de fragmento fora da faixa"));
    }
    w.fixed(&a.arquivo);
    w.u8(a.indice);
    Ok(())
}

fn ler_alvo(r: &mut Reader<'_>) -> Result<Alvo, ErroNuvem> {
    let arquivo = r.fixed::<32>()?;
    let indice = r.u8()?;
    if usize::from(indice) >= K_MAX.saturating_add(M_MAX) {
        return Err(ErroNuvem::Invalida("índice de fragmento fora da faixa"));
    }
    Ok(Alvo { arquivo, indice })
}

fn ler_marca(r: &mut Reader<'_>) -> Result<bool, ErroNuvem> {
    match r.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ErroNuvem::Invalida("marca inválida")),
    }
}

impl MensagemNuvem {
    /// O nome do subtipo (para registro e testes).
    pub fn subtipo(&self) -> &'static str {
        match self {
            Self::Anuncio(_) => "anuncio",
            Self::Guardar { .. } => "guardar",
            Self::Parte { .. } => "parte",
            Self::Guardado { .. } => "guardado",
            Self::Desafio { .. } => "desafio",
            Self::Prova { .. } => "prova",
            Self::Buscar(_) => "buscar",
            Self::Entrega { .. } => "entrega",
            Self::Apagar(_) => "apagar",
            Self::Recibo(_) => "recibo",
        }
    }

    /// O corpo da mensagem de rede.
    ///
    /// # Errors
    /// Campo fora da faixa.
    pub fn codificar(&self) -> Result<Vec<u8>, ErroNuvem> {
        let mut w = Writer::new();
        w.u8(VERSAO);
        match self {
            Self::Anuncio(a) => {
                w.u8(ANUNCIO);
                w.raw(&a.bytes()?);
            }
            Self::Guardar { alvo, tamanho, hash } => {
                if !(1..=FRAGMENTO_MAX).contains(tamanho) {
                    return Err(ErroNuvem::Invalida("fragmento grande demais"));
                }
                w.u8(GUARDAR);
                escrever_alvo(&mut w, alvo)?;
                w.u32(*tamanho);
                w.fixed(hash);
            }
            Self::Parte { alvo, deslocamento, dados } => {
                if dados.is_empty() || dados.len() > PARTE_MAX {
                    return Err(ErroNuvem::Invalida("parte de tamanho inválido"));
                }
                w.u8(PARTE);
                escrever_alvo(&mut w, alvo)?;
                w.u32(*deslocamento);
                w.var_bytes(dados)?;
            }
            Self::Guardado { alvo, ok, motivo } => {
                w.u8(GUARDADO);
                escrever_alvo(&mut w, alvo)?;
                w.u8(u8::from(*ok));
                texto_limitado(&mut w, motivo, MOTIVO_MAX)?;
            }
            Self::Desafio { alvo, nonce } => {
                w.u8(DESAFIO);
                escrever_alvo(&mut w, alvo)?;
                w.fixed(nonce);
            }
            Self::Prova { alvo, nonce, h } => {
                w.u8(PROVA);
                escrever_alvo(&mut w, alvo)?;
                w.fixed(nonce);
                w.fixed(h);
            }
            Self::Buscar(alvo) => {
                w.u8(BUSCAR);
                escrever_alvo(&mut w, alvo)?;
            }
            Self::Entrega { alvo, tem, tamanho, hash } => {
                if *tamanho > FRAGMENTO_MAX {
                    return Err(ErroNuvem::Invalida("fragmento grande demais"));
                }
                w.u8(ENTREGA);
                escrever_alvo(&mut w, alvo)?;
                w.u8(u8::from(*tem));
                w.u32(*tamanho);
                w.fixed(hash);
            }
            Self::Apagar(alvo) => {
                w.u8(APAGAR);
                escrever_alvo(&mut w, alvo)?;
            }
            Self::Recibo(r) => {
                w.u8(RECIBO);
                w.raw(&r.bytes()?);
            }
        }
        Ok(w.into_bytes())
    }

    /// Decodifica uma mensagem; recusa o que não for exatamente uma mensagem
    /// válida.
    ///
    /// # Errors
    /// Versão ou subtipo desconhecidos, campo fora da faixa, bytes que não
    /// fecham ou sobram.
    pub fn decodificar(dados: &[u8]) -> Result<Self, ErroNuvem> {
        let mut r = Reader::new(dados);
        if r.u8()? != VERSAO {
            return Err(ErroNuvem::Invalida("versão de mensagem da nuvem desconhecida"));
        }
        let m = match r.u8()? {
            ANUNCIO => Self::Anuncio(Box::new(Anuncio::ler(&mut r)?)),
            GUARDAR => {
                let alvo = ler_alvo(&mut r)?;
                let tamanho = r.u32()?;
                if !(1..=FRAGMENTO_MAX).contains(&tamanho) {
                    return Err(ErroNuvem::Invalida("fragmento grande demais"));
                }
                Self::Guardar { alvo, tamanho, hash: r.fixed::<64>()? }
            }
            PARTE => {
                let alvo = ler_alvo(&mut r)?;
                let deslocamento = r.u32()?;
                let corpo = r.var_bytes()?;
                if corpo.is_empty() || corpo.len() > PARTE_MAX {
                    return Err(ErroNuvem::Invalida("parte de tamanho inválido"));
                }
                Self::Parte { alvo, deslocamento, dados: corpo.to_vec() }
            }
            GUARDADO => {
                let alvo = ler_alvo(&mut r)?;
                let ok = ler_marca(&mut r)?;
                Self::Guardado { alvo, ok, motivo: ler_texto(&mut r, MOTIVO_MAX)? }
            }
            DESAFIO => {
                let alvo = ler_alvo(&mut r)?;
                Self::Desafio { alvo, nonce: r.fixed::<32>()? }
            }
            PROVA => {
                let alvo = ler_alvo(&mut r)?;
                let nonce = r.fixed::<32>()?;
                Self::Prova { alvo, nonce, h: r.fixed::<64>()? }
            }
            BUSCAR => Self::Buscar(ler_alvo(&mut r)?),
            ENTREGA => {
                let alvo = ler_alvo(&mut r)?;
                let tem = ler_marca(&mut r)?;
                let tamanho = r.u32()?;
                if tamanho > FRAGMENTO_MAX {
                    return Err(ErroNuvem::Invalida("fragmento grande demais"));
                }
                Self::Entrega { alvo, tem, tamanho, hash: r.fixed::<64>()? }
            }
            APAGAR => Self::Apagar(ler_alvo(&mut r)?),
            RECIBO => Self::Recibo(Box::new(Recibo::ler(&mut r)?)),
            _ => return Err(ErroNuvem::Invalida("subtipo de mensagem da nuvem desconhecido")),
        };
        r.finish()?;
        Ok(m)
    }
}
