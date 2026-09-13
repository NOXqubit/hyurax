//! Inteiro sem sinal de 512 bits, só com o que o consenso usa.
//!
//! Por que não uma biblioteca: são cinco operações (somar, multiplicar e
//! dividir por um `u64`, dividir por outro inteiro grande, comparar), e cada
//! dependência nova é código de consenso que não conferimos. 512 bits dão folga
//! de sobra: a maior conta do LWMA (média dos alvos × tempo ponderado) passa
//! pouco de 280 bits.

use core::cmp::Ordering;

/// 8 palavras de 64 bits, a menos significativa primeiro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct U512([u64; 8]);

impl U512 {
    /// Zero.
    pub const ZERO: Self = Self([0; 8]);

    /// A partir de um `u64`.
    pub const fn from_u64(v: u64) -> Self {
        Self([v, 0, 0, 0, 0, 0, 0, 0])
    }

    /// `2^bit`, com `bit < 512`.
    pub fn potencia_de_dois(bit: u32) -> Self {
        let mut p = [0u64; 8];
        if let Some(palavra) = p.get_mut((bit / 64) as usize) {
            *palavra = 1u64 << (bit % 64);
        }
        Self(p)
    }

    /// A partir de 32 bytes big-endian.
    pub fn from_be32(bytes: &[u8; 32]) -> Self {
        let mut p = [0u64; 8];
        for (palavra, pedaco) in p.iter_mut().zip(bytes.rchunks_exact(8)) {
            let mut b = [0u8; 8];
            b.copy_from_slice(pedaco);
            *palavra = u64::from_be_bytes(b);
        }
        Self(p)
    }

    /// Para 32 bytes big-endian; `None` se não couber em 256 bits.
    pub fn to_be32(&self) -> Option<[u8; 32]> {
        if self.0.iter().skip(4).any(|&w| w != 0) {
            return None;
        }
        let mut saida = [0u8; 32];
        for (pedaco, palavra) in saida.rchunks_exact_mut(8).zip(self.0.iter()) {
            pedaco.copy_from_slice(&palavra.to_be_bytes());
        }
        Some(saida)
    }

    /// É zero?
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }

    /// Soma; `None` se passar de 512 bits.
    pub fn checked_add(&self, outro: &Self) -> Option<Self> {
        let mut r = [0u64; 8];
        let mut vai_um = false;
        for ((destino, &a), &b) in r.iter_mut().zip(&self.0).zip(&outro.0) {
            let (s1, c1) = a.overflowing_add(b);
            let (s2, c2) = s1.overflowing_add(u64::from(vai_um));
            *destino = s2;
            vai_um = c1 || c2;
        }
        (!vai_um).then_some(Self(r))
    }

    /// Multiplica por um `u64`; `None` se passar de 512 bits.
    pub fn checked_mul_u64(&self, m: u64) -> Option<Self> {
        let mut r = [0u64; 8];
        let mut carrega = 0u128;
        for (destino, &a) in r.iter_mut().zip(&self.0) {
            let prod = u128::from(a)
                .wrapping_mul(u128::from(m))
                .wrapping_add(carrega);
            *destino = prod as u64;
            carrega = prod >> 64;
        }
        (carrega == 0).then_some(Self(r))
    }

    /// Divide por um `u64` não nulo, truncando.
    pub fn div_u64(&self, d: u64) -> Option<Self> {
        if d == 0 {
            return None;
        }
        let mut r = [0u64; 8];
        let mut resto = 0u128;
        for (destino, &a) in r.iter_mut().zip(&self.0).rev() {
            let atual = (resto << 64) | u128::from(a);
            *destino = atual.checked_div(u128::from(d)).unwrap_or(0) as u64;
            resto = atual.checked_rem(u128::from(d)).unwrap_or(0);
        }
        Some(Self(r))
    }

    /// Resto da divisão por um `u64` não nulo.
    pub fn rem_u64(&self, d: u64) -> Option<u64> {
        if d == 0 {
            return None;
        }
        let mut resto = 0u128;
        for &a in self.0.iter().rev() {
            resto = ((resto << 64) | u128::from(a)).checked_rem(u128::from(d)).unwrap_or(0);
        }
        Some(resto as u64)
    }

    fn bit(&self, i: u32) -> bool {
        self.0.get((i / 64) as usize).is_some_and(|w| (w >> (i % 64)) & 1 == 1)
    }

    fn shl1(&self) -> Self {
        let mut r = [0u64; 8];
        let mut entra = 0u64;
        for (destino, &a) in r.iter_mut().zip(&self.0) {
            *destino = (a << 1) | entra;
            entra = a >> 63;
        }
        Self(r)
    }

    fn sub_assumindo_maior(&self, outro: &Self) -> Self {
        let mut r = [0u64; 8];
        let mut pede = false;
        for ((destino, &a), &b) in r.iter_mut().zip(&self.0).zip(&outro.0) {
            let (d1, e1) = a.overflowing_sub(b);
            let (d2, e2) = d1.overflowing_sub(u64::from(pede));
            *destino = d2;
            pede = e1 || e2;
        }
        Self(r)
    }

    /// Divisão inteira por outro inteiro grande, bit a bit. Lenta, e basta:
    /// só roda uma vez por bloco.
    pub fn div(&self, d: &Self) -> Option<Self> {
        if d.is_zero() {
            return None;
        }
        let mut q = [0u64; 8];
        let mut resto = Self::ZERO;
        for i in (0..512u32).rev() {
            resto = resto.shl1();
            if self.bit(i)
                && let Some(w) = resto.0.first_mut()
            {
                *w |= 1;
            }
            if resto >= *d {
                resto = resto.sub_assumindo_maior(d);
                if let Some(w) = q.get_mut((i / 64) as usize) {
                    *w |= 1u64 << (i % 64);
                }
            }
        }
        Some(Self(q))
    }

    /// Decimal, para comparar com os vetores (que gravam trabalho em decimal).
    pub fn to_decimal(&self) -> String {
        if self.is_zero() {
            return "0".into();
        }
        const BASE: u64 = 10_000_000_000_000_000_000; // 10^19
        let mut partes = Vec::new();
        let mut v = *self;
        while !v.is_zero() {
            partes.push(v.rem_u64(BASE).unwrap_or(0));
            v = v.div_u64(BASE).unwrap_or(Self::ZERO);
        }
        let mut texto = partes.last().map(u64::to_string).unwrap_or_default();
        for parte in partes.iter().rev().skip(1) {
            texto.push_str(&format!("{parte:019}"));
        }
        texto
    }
}

impl PartialOrd for U512 {
    fn partial_cmp(&self, outro: &Self) -> Option<Ordering> {
        Some(self.cmp(outro))
    }
}

impl Ord for U512 {
    fn cmp(&self, outro: &Self) -> Ordering {
        self.0.iter().rev().cmp(outro.0.iter().rev())
    }
}
