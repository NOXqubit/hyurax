// ✝ Eclesiastes 4:9 — “Melhor é serem dois do que um, porque têm melhor paga do seu trabalho.”
//! Mensagens do ULTRAX entre nós (Etapa D de `docs/COMPUTACAO-CIENTIFICA.md`).
//!
//! Tradução de `reference/hyurax/rede_ultrax.py`, conferida por
//! `vectors/rede_ultrax.json`; o gabarito descreve cada campo.
//!
//! Transporte: o corpo de uma mensagem de rede de tipo [`TIPO_ULTRAX`]. Nós
//! que não conhecem o tipo decodificam como desconhecida e ignoram. Fora do
//! consenso: nada aqui muda bloco, transação ou cadeia.
//!
//! Quem recebe confere tudo de novo: a especificação é decodificada **e
//! validada** (faixas e custo do motor), os tamanhos têm teto, a oferta é
//! assinada pelo worker e o compromisso amarra resultado e worker. Nenhum
//! código vem pela rede: só os motores compilados no programa rodam.

use hyurax_codec::{CodecError, Reader, Writer};
use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN, SIGNATURE_LEN, ed25519_public_key, ed25519_sign, ed25519_verify, sha512};

use crate::trabalho::{ErroDeTrabalho, Especificacao};

/// Tipo da mensagem de rede que leva o ULTRAX ("UX").
pub const TIPO_ULTRAX: u16 = 0x5558;
/// Versão do corpo.
pub const VERSAO: u8 = 1;
/// Domínio da assinatura da oferta.
pub const DOMINIO_OFERTA: &[u8] = dominio!("OFERTA-v1");
/// Maior resultado que viaja pela rede.
pub const RESULTADO_MAX: usize = 1 << 20;
/// Maior motivo de recusa, em bytes.
pub const MOTIVO_MAX: usize = 500;
/// Maior registro de prova codificado.
pub const REGISTRO_MAX: usize = 4096;

/// Uma mensagem do ULTRAX entre nós.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MensagemUltrax {
    /// O worker se oferece, com o que aceita.
    Oferta {
        /// Chave do worker (a do registro de prova).
        worker: [u8; PUBKEY_LEN],
        /// Bit `código − 1` de cada tipo de trabalho aceito.
        tipos: u16,
        /// Linhas livres.
        linhas: u8,
        /// Memória que aceita reservar, em MiB.
        memoria_mib: u32,
        /// Quando a oferta foi feita, em ms desde 1970.
        instante_ms: u64,
        /// Assinatura do worker.
        assinatura: [u8; SIGNATURE_LEN],
    },
    /// Quem pede manda uma unidade.
    Pedido {
        /// Número de quem pede.
        pedido: u64,
        /// `JOB_ID`.
        job: [u8; HASH_LEN],
        /// Índice da unidade.
        indice: u64,
        /// A especificação, já validada na leitura.
        especificacao: Especificacao,
        /// A semente da unidade.
        semente: [u8; HASH_LEN],
        /// Prazo, em ms desde 1970.
        prazo_ms: u64,
        /// Mandar compromisso antes do resultado.
        com_compromisso: bool,
    },
    /// O worker não vai fazer (ou não fez).
    Recusa {
        /// Número de quem pede.
        pedido: u64,
        /// Por quê.
        motivo: String,
    },
    /// O worker se compromete com o hash do resultado, sem revelá-lo.
    Compromisso {
        /// Número de quem pede.
        pedido: u64,
        /// Chave do worker.
        worker: [u8; PUBKEY_LEN],
        /// `H(DOMINIO_COMPROMISSO || RESULT_HASH || worker)`.
        compromisso: [u8; HASH_LEN],
    },
    /// Quem pede já tem todos os compromissos: pode revelar.
    Revelar {
        /// Número de quem pede.
        pedido: u64,
    },
    /// O resultado, com o registro de prova assinado.
    Resultado {
        /// Número de quem pede.
        pedido: u64,
        /// O registro de prova codificado.
        registro: Vec<u8>,
        /// Assinatura do registro pelo worker.
        assinatura: [u8; SIGNATURE_LEN],
        /// Os bytes do resultado.
        resultado: Vec<u8>,
    },
    /// Quem pede não quer mais.
    Cancelar {
        /// Número de quem pede.
        pedido: u64,
    },
}

/// Por que uma mensagem não foi lida.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroDeRede {
    /// Bytes mal formados.
    Codec(CodecError),
    /// Campo fora da faixa, versão ou subtipo desconhecidos.
    Invalida(&'static str),
    /// A especificação do pedido não vale.
    Trabalho(ErroDeTrabalho),
}

impl std::fmt::Display for ErroDeRede {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "{e}"),
            Self::Invalida(m) => write!(f, "{m}"),
            Self::Trabalho(e) => write!(f, "{e}"),
        }
    }
}

impl From<CodecError> for ErroDeRede {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

/// O que a assinatura da oferta cobre.
pub fn mensagem_da_oferta(worker: &[u8; PUBKEY_LEN], tipos: u16, linhas: u8, memoria_mib: u32, instante_ms: u64) -> [u8; HASH_LEN] {
    let mut dados = DOMINIO_OFERTA.to_vec();
    dados.extend_from_slice(worker);
    dados.extend_from_slice(&tipos.to_be_bytes());
    dados.push(linhas);
    dados.extend_from_slice(&memoria_mib.to_be_bytes());
    dados.extend_from_slice(&instante_ms.to_be_bytes());
    sha512(&dados)
}

impl MensagemUltrax {
    /// Uma oferta assinada pela chave do worker.
    pub fn oferta(segredo: &[u8; SECRET_LEN], tipos: u16, linhas: u8, memoria_mib: u32, instante_ms: u64) -> Self {
        let worker = ed25519_public_key(segredo);
        let assinatura = ed25519_sign(segredo, &mensagem_da_oferta(&worker, tipos, linhas, memoria_mib, instante_ms));
        Self::Oferta { worker, tipos, linhas, memoria_mib, instante_ms, assinatura }
    }

    /// A oferta foi assinada pelo worker que ela nomeia? Outras mensagens: `false`.
    pub fn oferta_confere(&self) -> bool {
        match self {
            Self::Oferta { worker, tipos, linhas, memoria_mib, instante_ms, assinatura } => {
                ed25519_verify(worker, &mensagem_da_oferta(worker, *tipos, *linhas, *memoria_mib, *instante_ms), assinatura)
            }
            _ => false,
        }
    }

    /// O número do pedido, nas mensagens que têm.
    pub fn pedido(&self) -> Option<u64> {
        match self {
            Self::Oferta { .. } => None,
            Self::Pedido { pedido, .. }
            | Self::Recusa { pedido, .. }
            | Self::Compromisso { pedido, .. }
            | Self::Revelar { pedido }
            | Self::Resultado { pedido, .. }
            | Self::Cancelar { pedido } => Some(*pedido),
        }
    }

    /// Os bytes do corpo.
    pub fn codificar(&self) -> Result<Vec<u8>, ErroDeRede> {
        let mut w = Writer::new();
        w.u8(VERSAO);
        match self {
            Self::Oferta { worker, tipos, linhas, memoria_mib, instante_ms, assinatura } => {
                w.u8(1);
                w.fixed(worker);
                w.u16(*tipos);
                w.u8(*linhas);
                w.u32(*memoria_mib);
                w.u64(*instante_ms);
                w.fixed(assinatura);
            }
            Self::Pedido { pedido, job, indice, especificacao, semente, prazo_ms, com_compromisso } => {
                w.u8(2);
                w.u64(*pedido);
                w.fixed(job);
                w.u64(*indice);
                especificacao.codificar(&mut w);
                w.fixed(semente);
                w.u64(*prazo_ms);
                w.u8(u8::from(*com_compromisso));
            }
            Self::Recusa { pedido, motivo } => {
                if motivo.len() > MOTIVO_MAX {
                    return Err(ErroDeRede::Invalida("motivo longo demais"));
                }
                w.u8(3);
                w.u64(*pedido);
                w.string(motivo)?;
            }
            Self::Compromisso { pedido, worker, compromisso } => {
                w.u8(4);
                w.u64(*pedido);
                w.fixed(worker);
                w.fixed(compromisso);
            }
            Self::Revelar { pedido } => {
                w.u8(5);
                w.u64(*pedido);
            }
            Self::Resultado { pedido, registro, assinatura, resultado } => {
                if resultado.len() > RESULTADO_MAX || registro.len() > REGISTRO_MAX {
                    return Err(ErroDeRede::Invalida("resultado ou registro grande demais"));
                }
                w.u8(6);
                w.u64(*pedido);
                w.var_bytes(registro)?;
                w.fixed(assinatura);
                w.var_bytes(resultado)?;
            }
            Self::Cancelar { pedido } => {
                w.u8(7);
                w.u64(*pedido);
            }
        }
        Ok(w.into_bytes())
    }

    /// Lê um corpo, sem sobra, validando a especificação do pedido.
    pub fn decodificar(dados: &[u8]) -> Result<Self, ErroDeRede> {
        let mut r = Reader::new(dados);
        if r.u8()? != VERSAO {
            return Err(ErroDeRede::Invalida("versão de mensagem do ULTRAX desconhecida"));
        }
        let m = match r.u8()? {
            1 => Self::Oferta {
                worker: r.fixed()?,
                tipos: r.u16()?,
                linhas: r.u8()?,
                memoria_mib: r.u32()?,
                instante_ms: r.u64()?,
                assinatura: r.fixed()?,
            },
            2 => {
                let (pedido, job, indice) = (r.u64()?, r.fixed()?, r.u64()?);
                let especificacao = Especificacao::decodificar(&mut r).map_err(ErroDeRede::Trabalho)?;
                let (semente, prazo_ms) = (r.fixed()?, r.u64()?);
                let com_compromisso = match r.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(ErroDeRede::Invalida("marca de compromisso inválida")),
                };
                Self::Pedido { pedido, job, indice, especificacao, semente, prazo_ms, com_compromisso }
            }
            3 => {
                let pedido = r.u64()?;
                let motivo = r.string()?.to_string();
                if motivo.len() > MOTIVO_MAX {
                    return Err(ErroDeRede::Invalida("motivo longo demais"));
                }
                Self::Recusa { pedido, motivo }
            }
            4 => Self::Compromisso { pedido: r.u64()?, worker: r.fixed()?, compromisso: r.fixed()? },
            5 => Self::Revelar { pedido: r.u64()? },
            6 => {
                let pedido = r.u64()?;
                let registro = r.var_bytes()?.to_vec();
                let assinatura = r.fixed()?;
                let resultado = r.var_bytes()?.to_vec();
                if resultado.len() > RESULTADO_MAX || registro.len() > REGISTRO_MAX {
                    return Err(ErroDeRede::Invalida("resultado ou registro grande demais"));
                }
                Self::Resultado { pedido, registro, assinatura, resultado }
            }
            7 => Self::Cancelar { pedido: r.u64()? },
            _ => return Err(ErroDeRede::Invalida("subtipo de mensagem do ULTRAX desconhecido")),
        };
        r.finish()?;
        Ok(m)
    }
}

/// O bit de um tipo de trabalho na oferta.
pub fn bit_do_tipo(tipo: crate::trabalho::TipoDeTrabalho) -> u16 {
    1u16.wrapping_shl(u32::from(tipo.codigo().saturating_sub(1)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;
    use crate::trabalho::TipoDeTrabalho;

    #[test]
    fn oferta_forjada_nao_confere() {
        let o = MensagemUltrax::oferta(&[3; 32], 0xff, 2, 256, 1_000);
        assert!(o.oferta_confere());
        let MensagemUltrax::Oferta { worker, tipos, linhas, instante_ms, assinatura, .. } = o else { unreachable!() };
        let forjada = MensagemUltrax::Oferta { worker, tipos, linhas, memoria_mib: 4096, instante_ms, assinatura };
        assert!(!forjada.oferta_confere(), "mudar a memória quebra a assinatura");
    }

    #[test]
    fn pedido_com_especificacao_invalida_e_recusado_na_leitura() {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 64, 0).unwrap();
        let bom = MensagemUltrax::Pedido { pedido: 1, job: [1; 64], indice: 2, especificacao: esp, semente: [4; 64], prazo_ms: 5, com_compromisso: true };
        let mut bytes = bom.codificar().unwrap();
        assert_eq!(MensagemUltrax::decodificar(&bytes).unwrap(), bom);
        // lado da matriz 64 -> 2000 (acima do máximo 1024), no mesmo lugar dos bytes
        let posicao = 2 + 8 + 64 + 8 + 1;
        bytes[posicao..posicao + 4].copy_from_slice(&2000u32.to_be_bytes());
        assert!(matches!(MensagemUltrax::decodificar(&bytes), Err(ErroDeRede::Trabalho(_))));
        let mut sobra = bom.codificar().unwrap();
        sobra.push(0);
        assert!(MensagemUltrax::decodificar(&sobra).is_err(), "sobra no fim");
    }

    #[test]
    fn resultado_grande_demais_nao_sai_nem_entra() {
        let grande = MensagemUltrax::Resultado { pedido: 1, registro: vec![], assinatura: [0; 64], resultado: vec![0; RESULTADO_MAX + 1] };
        assert!(grande.codificar().is_err());
        assert_eq!(bit_do_tipo(TipoDeTrabalho::Matriz), 1);
        assert_eq!(bit_do_tipo(TipoDeTrabalho::Triagem), 1 << 7);
    }
}
