// ✝ Eclesiastes 12:13 — “De tudo o que se tem ouvido, o fim é.”
//! Consolidação dos resultados de um JOB, com memória constante.
//!
//! Cada unidade conferida entra com o índice e os bytes do resultado. O
//! agregador guarda só o que o relatório precisa (o melhor resultado, somas,
//! contagens), nunca a lista de resultados, então um JOB de um bilhão de
//! unidades cabe no mesmo espaço que um de dez.
//!
//! **Não depende da ordem de chegada.** Empate escolhe o menor índice, e somas
//! são somas: dois nós que consolidaram o mesmo conjunto de unidades chegam
//! no mesmo agregador, byte a byte. Uma unidade que entra duas vezes é erro de
//! quem chama (o agendador só soma unidade nova, pelos intervalos).
//!
//! Tudo em inteiros; média e desvio só viram número decimal na hora de
//! mostrar.

use hyurax_codec::{Reader, Writer};

use crate::ia;
use crate::job::ErroDeJob;
use crate::trabalho::{MOCHILA_RESULTADO_LEN, TipoDeTrabalho};

/// Versão da codificação do agregador.
pub const VERSAO: u8 = 1;

/// O estado consolidado de um JOB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Agregador {
    /// Matriz e difusão: o resultado é conferido e contado; o valor
    /// científico está na verificação, não num número a consolidar.
    Contagem {
        /// Unidades somadas.
        unidades: u64,
    },
    /// Mochila: o maior valor ótimo entre as instâncias, e de qual unidade.
    Mochila {
        /// Unidades somadas.
        unidades: u64,
        /// `(valor ótimo, índice)` da melhor; empate pelo menor índice.
        melhor: Option<(u64, u64)>,
        /// Soma dos valores ótimos, para a média.
        soma: u128,
    },
    /// Treino de IA: o menor erro de validação e de qual unidade.
    Ia {
        /// Unidades somadas.
        unidades: u64,
        /// `(erro Q12, índice)` do melhor treino; empate pelo menor índice.
        melhor: Option<(u64, u64)>,
        /// Soma e soma dos quadrados dos erros, para média e desvio.
        soma: u128,
        /// Soma dos quadrados.
        soma_quadrados: u128,
    },
}

impl Agregador {
    /// Vazio, para o tipo.
    pub fn novo(tipo: TipoDeTrabalho) -> Self {
        match tipo {
            TipoDeTrabalho::Mochila => Self::Mochila { unidades: 0, melhor: None, soma: 0 },
            TipoDeTrabalho::Ia => Self::Ia { unidades: 0, melhor: None, soma: 0, soma_quadrados: 0 },
            TipoDeTrabalho::Matriz
            | TipoDeTrabalho::Difusao
            | TipoDeTrabalho::Genetica
            | TipoDeTrabalho::Melhoramento
            | TipoDeTrabalho::Rotas
            | TipoDeTrabalho::Triagem => Self::Contagem { unidades: 0 },
        }
    }

    /// Unidades somadas até aqui.
    pub fn unidades(&self) -> u64 {
        match self {
            Self::Contagem { unidades } | Self::Mochila { unidades, .. } | Self::Ia { unidades, .. } => *unidades,
        }
    }

    /// Soma uma unidade conferida.
    pub fn somar(&mut self, indice: u64, resultado: &[u8]) -> Result<(), ErroDeJob> {
        match self {
            Self::Contagem { unidades } => {
                *unidades = unidades.saturating_add(1);
            }
            Self::Mochila { unidades, melhor, soma } => {
                if resultado.len() != MOCHILA_RESULTADO_LEN {
                    return Err(ErroDeJob::Faixa("resultado de mochila com tamanho errado"));
                }
                let valor = resultado
                    .first_chunk::<8>()
                    .map(|b| u64::from_be_bytes(*b))
                    .ok_or(ErroDeJob::Faixa("resultado de mochila curto"))?;
                *unidades = unidades.saturating_add(1);
                *soma = soma.saturating_add(u128::from(valor));
                if melhor.is_none_or(|(v, i)| valor > v || (valor == v && indice < i)) {
                    *melhor = Some((valor, indice));
                }
            }
            Self::Ia { unidades, melhor, soma, soma_quadrados } => {
                let (_, erro) = ia::decodificar(resultado).ok_or(ErroDeJob::Faixa("resultado de IA ilegível"))?;
                *unidades = unidades.saturating_add(1);
                *soma = soma.saturating_add(u128::from(erro));
                *soma_quadrados = soma_quadrados.saturating_add(u128::from(erro).saturating_mul(u128::from(erro)));
                if melhor.is_none_or(|(e, i)| erro < e || (erro == e && indice < i)) {
                    *melhor = Some((erro, indice));
                }
            }
        }
        Ok(())
    }

    /// Codificação canônica, para o checkpoint.
    pub fn codificar(&self, w: &mut Writer) {
        w.u8(VERSAO);
        let opcional = |w: &mut Writer, par: &Option<(u64, u64)>| match par {
            Some((a, b)) => {
                w.u8(1);
                w.u64(*a);
                w.u64(*b);
            }
            None => w.u8(0),
        };
        let u128_ = |w: &mut Writer, v: u128| w.raw(&v.to_be_bytes());
        match self {
            Self::Contagem { unidades } => {
                w.u8(1);
                w.u64(*unidades);
            }
            Self::Mochila { unidades, melhor, soma } => {
                w.u8(2);
                w.u64(*unidades);
                opcional(w, melhor);
                u128_(w, *soma);
            }
            Self::Ia { unidades, melhor, soma, soma_quadrados } => {
                w.u8(3);
                w.u64(*unidades);
                opcional(w, melhor);
                u128_(w, *soma);
                u128_(w, *soma_quadrados);
            }
        }
    }

    /// O inverso de [`Self::codificar`].
    pub fn decodificar(r: &mut Reader<'_>) -> Result<Self, ErroDeJob> {
        if r.u8()? != VERSAO {
            return Err(ErroDeJob::Faixa("versão de agregador desconhecida"));
        }
        let opcional = |r: &mut Reader<'_>| -> Result<Option<(u64, u64)>, ErroDeJob> {
            Ok(match r.u8()? {
                0 => None,
                1 => Some((r.u64()?, r.u64()?)),
                _ => return Err(ErroDeJob::Faixa("marcador inválido no agregador")),
            })
        };
        let u128_ = |r: &mut Reader<'_>| -> Result<u128, ErroDeJob> { Ok(u128::from_be_bytes(r.fixed::<16>()?)) };
        Ok(match r.u8()? {
            1 => Self::Contagem { unidades: r.u64()? },
            2 => Self::Mochila { unidades: r.u64()?, melhor: opcional(r)?, soma: u128_(r)? },
            3 => Self::Ia { unidades: r.u64()?, melhor: opcional(r)?, soma: u128_(r)?, soma_quadrados: u128_(r)? },
            _ => return Err(ErroDeJob::Faixa("tipo de agregador desconhecido")),
        })
    }

    /// Resumo em texto para a tela e o relatório.
    pub fn texto(&self) -> String {
        match self {
            Self::Contagem { unidades } => format!("{unidades} unidade(s) conferida(s)"),
            Self::Mochila { unidades, melhor, soma } => match melhor {
                Some((v, i)) => format!(
                    "{unidades} instância(s) resolvidas no ótimo; maior valor ótimo {v} (unidade {i}); média {:.1}",
                    media(*soma, *unidades)
                ),
                None => "nenhuma instância ainda".into(),
            },
            Self::Ia { unidades, melhor, soma, soma_quadrados } => match melhor {
                Some((e, i)) => format!(
                    "{unidades} treino(s); menor erro de validação {:.4} (unidade {i}); média {:.4} ± {:.4} (desvio entre treinos), em unidades normalizadas Q12",
                    *e as f64 / 4096.0,
                    media(*soma, *unidades) / 4096.0,
                    desvio(*soma, *soma_quadrados, *unidades) / 4096.0
                ),
                None => "nenhum treino ainda".into(),
            },
        }
    }
}

/// Média de uma soma inteira, só para mostrar.
pub fn media(soma: u128, n: u64) -> f64 {
    if n == 0 { 0.0 } else { soma as f64 / n as f64 }
}

/// Desvio-padrão amostral a partir de soma e soma dos quadrados, só para
/// mostrar.
pub fn desvio(soma: u128, soma_quadrados: u128, n: u64) -> f64 {
    if n < 2 {
        return 0.0;
    }
    let n = n as f64;
    let m = soma as f64 / n;
    ((soma_quadrados as f64 - n * m * m) / (n - 1.0)).max(0.0).sqrt()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod testes {
    use super::*;

    fn mochila(valor: u64) -> Vec<u8> {
        let mut r = valor.to_be_bytes().to_vec();
        r.extend_from_slice(&[0; 32]);
        r
    }

    #[test]
    fn mesma_consolidacao_em_qualquer_ordem() {
        let valores = [5u64, 9, 3, 9, 7];
        let mut a = Agregador::novo(TipoDeTrabalho::Mochila);
        let mut b = Agregador::novo(TipoDeTrabalho::Mochila);
        for (i, v) in valores.iter().enumerate() {
            a.somar(i as u64, &mochila(*v)).unwrap();
        }
        for (i, v) in valores.iter().enumerate().rev() {
            b.somar(i as u64, &mochila(*v)).unwrap();
        }
        assert_eq!(a, b);
        assert!(matches!(a, Agregador::Mochila { melhor: Some((9, 1)), .. }), "empate fica com o menor índice");
    }

    #[test]
    fn vai_e_volta_pelo_checkpoint() {
        let mut a = Agregador::novo(TipoDeTrabalho::Mochila);
        a.somar(3, &mochila(42)).unwrap();
        for ag in [a, Agregador::novo(TipoDeTrabalho::Matriz), Agregador::novo(TipoDeTrabalho::Ia)] {
            let mut w = Writer::new();
            ag.codificar(&mut w);
            let bytes = w.into_bytes();
            let mut r = Reader::new(&bytes);
            assert_eq!(Agregador::decodificar(&mut r).unwrap(), ag);
            r.finish().unwrap();
        }
    }

    #[test]
    fn resultado_ilegivel_e_recusado() {
        let mut a = Agregador::novo(TipoDeTrabalho::Mochila);
        assert!(a.somar(0, &[1, 2, 3]).is_err());
        let mut i = Agregador::novo(TipoDeTrabalho::Ia);
        assert!(i.somar(0, &[0; 5]).is_err());
        assert_eq!(a.unidades() + i.unidades(), 0, "recusado não conta");
    }
}
