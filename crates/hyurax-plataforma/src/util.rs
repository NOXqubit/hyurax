//! Pequenas ferramentas: hexadecimal, base32, bytes aleatórios e relógio.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milissegundos desde 1970.
pub fn agora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Põe `valor` em `chave` de um objeto JSON (sem índice que possa entrar em pânico).
pub fn por(objeto: &mut serde_json::Value, chave: &str, valor: serde_json::Value) {
    if let Some(o) = objeto.as_object_mut() {
        o.insert(chave.to_string(), valor);
    }
}

/// Bytes em hexadecimal minúsculo.
pub fn hex(b: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len().saturating_mul(2));
    for &x in b {
        s.push(char::from(D.get(usize::from(x >> 4)).copied().unwrap_or(b'0')));
        s.push(char::from(D.get(usize::from(x & 15)).copied().unwrap_or(b'0')));
    }
    s
}

/// Hexadecimal de volta em bytes.
pub fn de_hex(texto: &str) -> Option<Vec<u8>> {
    let t = texto.trim().as_bytes();
    if !t.len().is_multiple_of(2) {
        return None;
    }
    let digito = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c.wrapping_sub(b'0')),
            b'a'..=b'f' => Some(c.wrapping_sub(b'a').wrapping_add(10)),
            b'A'..=b'F' => Some(c.wrapping_sub(b'A').wrapping_add(10)),
            _ => None,
        }
    };
    t.as_chunks::<2>().0.iter().map(|&[a, b]| Some((digito(a)? << 4) | digito(b)?)).collect()
}

/// Hexadecimal de tamanho fixo.
pub fn de_hex_fixo<const N: usize>(texto: &str) -> Option<[u8; N]> {
    de_hex(texto)?.try_into().ok()
}

/// Base32 sem preenchimento (RFC 4648, alfabeto minúsculo), para chaves e
/// tokens que pessoas copiam.
pub fn base32(dados: &[u8]) -> String {
    const A: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut s = String::new();
    let mut acumulado: u32 = 0;
    let mut bits: u32 = 0;
    for &b in dados {
        acumulado = (acumulado << 8) | u32::from(b);
        bits = bits.saturating_add(8);
        while bits >= 5 {
            bits = bits.saturating_sub(5);
            let i = (acumulado >> bits) & 31;
            s.push(char::from(A.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or(b'a')));
        }
        acumulado &= (1u32 << bits).wrapping_sub(1);
    }
    if bits > 0 {
        let i = (acumulado << (5u32.saturating_sub(bits))) & 31;
        s.push(char::from(A.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or(b'a')));
    }
    s
}

/// `n` bytes imprevisíveis, da entropia do sistema.
pub fn aleatorio<const N: usize>() -> Result<[u8; N], String> {
    let mut b = [0u8; N];
    hyurax_net::entropia::preencher(&mut b)?;
    Ok(b)
}

/// Um número imprevisível em `[0, 10000)`, para os sorteios em pontos-base.
pub fn sorteio_bp() -> Result<u32, String> {
    let b: [u8; 8] = aleatorio()?;
    Ok(u32::try_from(u64::from_be_bytes(b) % 10_000).unwrap_or(0))
}

/// Compara dois textos em tempo que não depende de onde diferem.
pub fn iguais(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Data e hora em UTC: `2026-10-16 14:03`.
pub fn data_hora(ms: u64) -> String {
    let s = ms / 1000;
    let (dias, resto) = (s / 86_400, s % 86_400);
    let (a, m, d) = civil(i64::try_from(dias).unwrap_or(0));
    format!("{a:04}-{m:02}-{d:02} {:02}:{:02}", resto / 3600, (resto % 3600) / 60)
}

/// `AAAA-MM` do instante.
pub fn mes(ms: u64) -> String {
    let (a, m, _) = civil(i64::try_from(ms / 86_400_000).unwrap_or(0));
    format!("{a:04}-{m:02}")
}

/// Dias desde 1970 para ano, mês e dia (algoritmo de Howard Hinnant).
#[allow(clippy::arithmetic_side_effects)]
fn civil(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn hex_e_base32() {
        assert_eq!(hex(&[0, 0xab, 0xff]), "00abff");
        assert_eq!(de_hex("00ABff").unwrap(), vec![0, 0xab, 0xff]);
        assert_eq!(de_hex("0"), None);
        assert_eq!(de_hex("zz"), None);
        // RFC 4648, seção 10 (em minúsculas e sem '=')
        assert_eq!(base32(b"f"), "my");
        assert_eq!(base32(b"fooba"), "mzxw6ytb");
        assert_eq!(base32(b"foobar"), "mzxw6ytboi");
    }

    #[test]
    fn datas() {
        assert_eq!(data_hora(0), "1970-01-01 00:00");
        assert_eq!(data_hora(1_791_000_000_000), "2026-10-03 04:00");
        assert_eq!(mes(1_791_000_000_000), "2026-10");
        assert!(iguais(b"abc", b"abc") && !iguais(b"abc", b"abd") && !iguais(b"ab", b"abc"));
    }
}
