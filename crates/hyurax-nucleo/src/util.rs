//! Conversões pequenas usadas no programa inteiro: hexadecimal, HYX em texto,
//! relógio e texto JSON.

use std::fmt::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

/// 1 HYX = 100 000 000 unidades (seção 1 da especificação).
pub const HYX_UNIDADE: u128 = 100_000_000;

/// Bytes em hexadecimal minúsculo.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len().saturating_mul(2));
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Hexadecimal de tamanho exato para bytes.
pub fn de_hex<const N: usize>(texto: &str) -> Option<[u8; N]> {
    let texto = texto.trim();
    if texto.len() != N.checked_mul(2)? {
        return None;
    }
    let mut saida = [0u8; N];
    for (i, byte) in saida.iter_mut().enumerate() {
        let ini = i.checked_mul(2)?;
        *byte = u8::from_str_radix(texto.get(ini..ini.checked_add(2)?)?, 16).ok()?;
    }
    Some(saida)
}

/// Unidades em HYX com as 8 casas: `150000000` vira `"1.50000000"`.
pub fn hyx(unidades: u128) -> String {
    format!("{}.{:08}", unidades / HYX_UNIDADE, unidades % HYX_UNIDADE)
}

/// `"1.5"` vira 150 000 000 unidades. No máximo 8 casas; nunca ponto flutuante.
///
/// # Errors
/// Texto que não é número decimal com até 8 casas, ou que estoura.
pub fn unidades_de_hyx(texto: &str) -> Result<u64, String> {
    let erro = || format!("valor inválido: {texto} (use ponto, até 8 casas, por exemplo 1.5)");
    let (inteiro, fracao) = texto.trim().split_once('.').unwrap_or((texto.trim(), ""));
    if inteiro.is_empty() || fracao.len() > 8 || !inteiro.chars().chain(fracao.chars()).all(|c| c.is_ascii_digit()) {
        return Err(erro());
    }
    let inteiro: u64 = inteiro.parse().map_err(|_| erro())?;
    let fracao: u64 = format!("{fracao:0<8}").parse().map_err(|_| erro())?;
    inteiro.checked_mul(100_000_000).and_then(|v| v.checked_add(fracao)).ok_or_else(erro)
}

/// Segundos desde 1970.
pub fn agora_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Milissegundos desde 1970.
pub fn agora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Base64 padrão (RFC 4648, com `=`).
pub fn base64(bytes: &[u8]) -> String {
    const TABELA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let letra = |i: u32| char::from(TABELA.get((i & 63) as usize).copied().unwrap_or(b'A'));
    let mut s = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for bloco in bytes.chunks(3) {
        let b0 = u32::from(bloco.first().copied().unwrap_or(0));
        let b1 = u32::from(bloco.get(1).copied().unwrap_or(0));
        let b2 = u32::from(bloco.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        s.push(letra(n >> 18));
        s.push(letra(n >> 12));
        s.push(if bloco.len() > 1 { letra(n >> 6) } else { '=' });
        s.push(if bloco.len() > 2 { letra(n) } else { '=' });
    }
    s
}

/// Texto como literal JSON, com aspas e escape.
pub fn texto_json(s: &str) -> String {
    serde_json::Value::String(s.to_owned()).to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn hyx_vai_e_volta_sem_ponto_flutuante() {
        assert_eq!(unidades_de_hyx("1.5").unwrap(), 150_000_000);
        assert_eq!(unidades_de_hyx("0.00000001").unwrap(), 1);
        assert_eq!(hyx(150_000_000), "1.50000000");
        assert!(unidades_de_hyx("1.123456789").is_err(), "nove casas");
        assert!(unidades_de_hyx("-1").is_err());
        assert!(unidades_de_hyx("1e3").is_err());
        assert!(unidades_de_hyx("184467440738").is_err(), "estoura o u64");
    }

    #[test]
    fn hex_vai_e_volta() {
        assert_eq!(hex(&[0, 0xab, 0xff]), "00abff");
        assert_eq!(de_hex::<3>("00abff"), Some([0, 0xab, 0xff]));
        assert_eq!(de_hex::<3>("00abf"), None);
        assert_eq!(de_hex::<3>("zzabff"), None);
    }

    #[test]
    fn base64_da_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn json_escapa_aspas_barras_e_controle() {
        assert_eq!(texto_json("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }
}
