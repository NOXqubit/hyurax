//! O recibo de aluguel: o cliente assina quanto trabalho verificado o
//! fornecedor fez para um JOB, e quanto isso vale no preço anunciado.
//!
//! ```text
//! cliente[32] || fornecedor[32] || job[64] || u64 creditos_mili
//! || u64 preco_credito_mili || u64 total_mili || u64 instante_ms || assinatura[64]
//! ```
//!
//! `total_mili = creditos_mili · preco_credito_mili / 1000` (para baixo);
//! `assinatura = Ed25519(cliente, H(DOMINIO_RECIBO || tudo antes dela))`.
//! O recibo é acumulado: o mais novo de um JOB substitui os anteriores.

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN, SIGNATURE_LEN, ed25519_public_key, ed25519_sign, ed25519_verify, sha512};

use crate::{DOMINIO_RECIBO, ErroNuvem};

/// O valor de `creditos_mili` ao preço `preco_credito_mili` (milicréditos
/// por crédito), para baixo. `None` se não cabe em 64 bits.
pub fn total(creditos_mili: u64, preco_credito_mili: u64) -> Option<u64> {
    let t = u128::from(creditos_mili).checked_mul(u128::from(preco_credito_mili))?.checked_div(1000)?;
    u64::try_from(t).ok()
}

/// Um recibo de aluguel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recibo {
    /// Chave do worker do cliente (quem assina).
    pub cliente: [u8; PUBKEY_LEN],
    /// Chave do worker do fornecedor.
    pub fornecedor: [u8; PUBKEY_LEN],
    /// O JOB.
    pub job: [u8; HASH_LEN],
    /// Créditos verificados feitos pelo fornecedor neste JOB, até aqui.
    pub creditos_mili: u64,
    /// O preço do anúncio.
    pub preco_credito_mili: u64,
    /// O valor.
    pub total_mili: u64,
    /// Quando, em ms desde 1970.
    pub instante_ms: u64,
    /// Assinatura do cliente.
    pub assinatura: [u8; SIGNATURE_LEN],
}

impl Recibo {
    /// Os bytes assinados.
    ///
    /// # Errors
    /// O total não bate com créditos e preço.
    pub fn corpo(&self) -> Result<Vec<u8>, ErroNuvem> {
        if total(self.creditos_mili, self.preco_credito_mili) != Some(self.total_mili) {
            return Err(ErroNuvem::Invalida("total do recibo não bate com créditos e preço"));
        }
        let mut w = Writer::new();
        w.fixed(&self.cliente);
        w.fixed(&self.fornecedor);
        w.fixed(&self.job);
        w.u64(self.creditos_mili);
        w.u64(self.preco_credito_mili);
        w.u64(self.total_mili);
        w.u64(self.instante_ms);
        Ok(w.into_bytes())
    }

    fn mensagem(corpo: &[u8]) -> [u8; 64] {
        let mut m = Vec::with_capacity(DOMINIO_RECIBO.len().saturating_add(corpo.len()));
        m.extend_from_slice(DOMINIO_RECIBO);
        m.extend_from_slice(corpo);
        sha512(&m)
    }

    /// Monta e assina um recibo.
    ///
    /// # Errors
    /// Valor que não cabe em 64 bits.
    pub fn assinar(
        segredo: &[u8; SECRET_LEN],
        fornecedor: [u8; PUBKEY_LEN],
        job: [u8; HASH_LEN],
        creditos_mili: u64,
        preco_credito_mili: u64,
        instante_ms: u64,
    ) -> Result<Self, ErroNuvem> {
        let total_mili = total(creditos_mili, preco_credito_mili).ok_or(ErroNuvem::Invalida("valor do recibo grande demais"))?;
        let mut r = Self {
            cliente: ed25519_public_key(segredo),
            fornecedor,
            job,
            creditos_mili,
            preco_credito_mili,
            total_mili,
            instante_ms,
            assinatura: [0; SIGNATURE_LEN],
        };
        r.assinatura = ed25519_sign(segredo, &Self::mensagem(&r.corpo()?));
        Ok(r)
    }

    /// A assinatura confere com o cliente.
    pub fn confere(&self) -> bool {
        self.corpo().is_ok_and(|c| ed25519_verify(&self.cliente, &Self::mensagem(&c), &self.assinatura))
    }

    /// Os bytes inteiros.
    ///
    /// # Errors
    /// O total não bate.
    pub fn bytes(&self) -> Result<Vec<u8>, ErroNuvem> {
        let mut b = self.corpo()?;
        b.extend_from_slice(&self.assinatura);
        Ok(b)
    }

    /// Lê um recibo (a assinatura não é conferida aqui).
    ///
    /// # Errors
    /// Bytes que não fecham, ou total que não bate.
    pub fn ler(r: &mut Reader<'_>) -> Result<Self, ErroNuvem> {
        let rc = Self {
            cliente: r.fixed::<32>()?,
            fornecedor: r.fixed::<32>()?,
            job: r.fixed::<64>()?,
            creditos_mili: r.u64()?,
            preco_credito_mili: r.u64()?,
            total_mili: r.u64()?,
            instante_ms: r.u64()?,
            assinatura: r.fixed::<64>()?,
        };
        if total(rc.creditos_mili, rc.preco_credito_mili) != Some(rc.total_mili) {
            return Err(ErroNuvem::Invalida("total do recibo não bate com créditos e preço"));
        }
        Ok(rc)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn assina_e_confere() {
        let r = Recibo::assinar(&[1; 32], [2; 32], [3; 64], 2_500, 1_200, 9).unwrap();
        assert_eq!(r.total_mili, 3_000);
        assert!(r.confere());
        let mut torto = r.clone();
        torto.fornecedor = [4; 32];
        assert!(!torto.confere());
        assert_eq!(total(u64::MAX, u64::MAX), None);
    }
}
