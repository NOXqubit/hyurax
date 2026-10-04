//! O livro de contas: cada lançamento carrega o hash do anterior, então
//! apagar, trocar ou reordenar uma linha quebra todas as seguintes.
//!
//! ```text
//! hash = H(DOMINIO_LIVRO || anterior[64] || u64 seq || u64 instante
//!          || str tipo || u32 conta || job[64] || contraparte[32] || u64 valor_mili
//!          || str nota)
//! ```
//!
//! Créditos de computação, não dinheiro: o livro registra consumo e a
//! partilha dele, para auditoria. Nada aqui paga ninguém.

use hyurax_codec::Writer;
use hyurax_crypto::{HASH_LEN, sha512};

use crate::{DOMINIO_LIVRO, ErroNuvem, texto_limitado};

/// Maior nota de um lançamento, em bytes.
pub const NOTA_MAX: usize = 200;

/// Os tipos de lançamento.
pub const TIPOS: [&str; 6] = ["consumo", "provedor", "plataforma", "reserva", "credito", "recibo"];

/// Um lançamento, sem o próprio hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lancamento {
    /// Número de ordem (1, 2, ...).
    pub seq: u64,
    /// Quando (segundos Unix).
    pub instante: u64,
    /// Um de [`TIPOS`].
    pub tipo: String,
    /// Conta de cliente (0: o dono do nó).
    pub conta: u32,
    /// JOB (zeros: nenhum).
    pub job: [u8; HASH_LEN],
    /// A outra parte (chave do worker; zeros: nenhuma).
    pub contraparte: [u8; 32],
    /// Valor, em milicréditos.
    pub valor_mili: u64,
    /// Texto livre curto.
    pub nota: String,
}

impl Lancamento {
    /// O hash deste lançamento, encadeado ao `anterior`.
    ///
    /// # Errors
    /// Tipo desconhecido ou nota longa demais.
    pub fn hash(&self, anterior: &[u8; HASH_LEN]) -> Result<[u8; HASH_LEN], ErroNuvem> {
        if !TIPOS.contains(&self.tipo.as_str()) {
            return Err(ErroNuvem::Invalida("tipo de lançamento desconhecido"));
        }
        let mut w = Writer::new();
        w.raw(DOMINIO_LIVRO);
        w.fixed(anterior);
        w.u64(self.seq);
        w.u64(self.instante);
        w.string(&self.tipo)?;
        w.u32(self.conta);
        w.fixed(&self.job);
        w.fixed(&self.contraparte);
        w.u64(self.valor_mili);
        texto_limitado(&mut w, &self.nota, NOTA_MAX)?;
        Ok(sha512(&w.into_bytes()))
    }
}

/// A partilha de um consumo: `(provedor, plataforma, reserva)`. A reserva
/// fica com o resto, então as três somam sempre o total.
///
/// # Errors
/// Percentuais que passam de 100.
pub fn repartir(total_mili: u64, provedor_pct: u8, plataforma_pct: u8) -> Result<(u64, u64, u64), ErroNuvem> {
    if u16::from(provedor_pct).saturating_add(u16::from(plataforma_pct)) > 100 {
        return Err(ErroNuvem::Invalida("percentuais inválidos"));
    }
    let parte = |pct: u8| u64::try_from(u128::from(total_mili).saturating_mul(u128::from(pct)) / 100).unwrap_or(0);
    let provedor = parte(provedor_pct);
    let plataforma = parte(plataforma_pct);
    Ok((provedor, plataforma, total_mili.saturating_sub(provedor).saturating_sub(plataforma)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn soma_sempre_o_total() {
        for t in [0, 1, 7, 999, u64::MAX] {
            let (a, b, c) = repartir(t, 80, 15).unwrap();
            assert_eq!(u128::from(a) + u128::from(b) + u128::from(c), u128::from(t));
        }
        assert!(repartir(1, 90, 20).is_err());
    }
}
