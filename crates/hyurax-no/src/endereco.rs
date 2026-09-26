// ✝ Provérbios 22:28 — “Não removas os limites antigos que fizeram teus pais.”
//! O endereço como as pessoas veem: Bech32m (BIP-350), com dígito verificador.
//!
//! Por dentro nada muda: o endereço continua sendo os 20 bytes da seção 4 da
//! especificação, e o consenso nem sabe que este formato existe. Muda o que se
//! lê, copia e digita:
//!
//! - **dígito verificador:** um erro de digitação de até 4 caracteres é pego
//!   com certeza, e qualquer outro erro quase sempre (chance de passar de
//!   1 em 1 bilhão). Com o hexadecimal de antes, uma letra trocada mandava HYX
//!   para um endereço que ninguém controla;
//! - **a rede no prefixo:** `hyx1…` na principal, `thyx1…` na de teste,
//!   `rhyx1…` na local. Colar um endereço de teste onde se espera um da rede
//!   principal é recusado;
//! - **sem diferença de maiúscula:** o QR fica menor, e ditar fica mais fácil.
//!
//! O hexadecimal de 40 dígitos continua aceito na entrada, para quem já tem
//! endereço anotado; ele só não tem a proteção do verificador.

use hyurax_crypto::ADDRESS_LEN;

const ALFABETO: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32M: u32 = 0x2bc8_30a3;
const GERADOR: [u32; 5] = [0x3b6a_57b2, 0x2650_8e6d, 0x1ea1_19fa, 0x3d42_33dd, 0x2a14_62b3];

/// O prefixo do endereço em cada rede.
pub fn prefixo(nome_da_rede: &str) -> &'static str {
    match nome_da_rede.trim_start_matches("hyurax-") {
        "mainnet" => "hyx",
        "testnet" => "thyx",
        _ => "rhyx",
    }
}

fn polimodo(valores: &[u8]) -> u32 {
    let mut c: u32 = 1;
    for &v in valores {
        let topo = c >> 25;
        c = ((c & 0x01ff_ffff) << 5) ^ u32::from(v);
        for (i, g) in GERADOR.iter().enumerate() {
            if (topo >> i) & 1 == 1 {
                c ^= g;
            }
        }
    }
    c
}

fn expandir(hrp: &str) -> Vec<u8> {
    let mut v: Vec<u8> = hrp.bytes().map(|b| b >> 5).collect();
    v.push(0);
    v.extend(hrp.bytes().map(|b| b & 31));
    v
}

/// Reagrupa bits (de 8 em 8 para 5 em 5, ou o contrário).
fn reagrupar(dados: &[u8], de: u32, para: u32, completar: bool) -> Option<Vec<u8>> {
    let (mut acumulado, mut bits) = (0u32, 0u32);
    let maximo = (1u32 << para) - 1;
    let mut saida = Vec::new();
    for &v in dados {
        if u32::from(v) >> de != 0 {
            return None;
        }
        acumulado = (acumulado << de) | u32::from(v);
        bits += de;
        while bits >= para {
            bits -= para;
            saida.push(u8::try_from((acumulado >> bits) & maximo).ok()?);
        }
    }
    if completar {
        if bits > 0 {
            saida.push(u8::try_from((acumulado << (para - bits)) & maximo).ok()?);
        }
    } else if bits >= de || (acumulado << (para - bits)) & maximo != 0 {
        return None;
    }
    Some(saida)
}

/// Codifica qualquer carga em Bech32m com o prefixo dado.
fn codificar(hrp: &str, carga: &[u8]) -> Option<String> {
    let dados = reagrupar(carga, 8, 5, true)?;
    let mut valores = expandir(hrp);
    valores.extend(&dados);
    valores.extend([0u8; 6]);
    let verificador = polimodo(&valores) ^ BECH32M;
    let mut s = String::with_capacity(hrp.len() + 1 + dados.len() + 6);
    s.push_str(hrp);
    s.push('1');
    for d in dados.iter().copied().chain((0..6).map(|i| u8::try_from((verificador >> (5 * (5 - i))) & 31).unwrap_or(0))) {
        s.push(char::from(*ALFABETO.get(usize::from(d))?));
    }
    Some(s)
}

/// Lê um texto Bech32m qualquer: prefixo e dados de 5 bits, já conferidos.
fn ler_bech32m(texto: &str) -> Result<(String, Vec<u8>), String> {
    let t = texto.trim();
    if t.len() > 90 || t.len() < 8 {
        return Err("endereço com tamanho errado".into());
    }
    if t.chars().any(|c| c.is_ascii_lowercase()) && t.chars().any(|c| c.is_ascii_uppercase()) {
        return Err("endereço mistura maiúsculas e minúsculas".into());
    }
    let t = t.to_ascii_lowercase();
    let separador = t.rfind('1').ok_or("endereço sem o separador 1")?;
    let (hrp, resto) = t.split_at(separador);
    let resto = resto.get(1..).unwrap_or("");
    if hrp.is_empty() || resto.len() < 6 {
        return Err("endereço incompleto".into());
    }
    let mut dados = Vec::with_capacity(resto.len());
    for c in resto.bytes() {
        let v = ALFABETO.iter().position(|&a| a == c).ok_or_else(|| format!("caractere que não existe em endereço: {}", char::from(c)))?;
        dados.push(u8::try_from(v).map_err(|_| "caractere inválido")?);
    }
    let mut valores = expandir(hrp);
    valores.extend(&dados);
    if polimodo(&valores) != BECH32M {
        return Err("o dígito verificador não confere: tem erro de digitação no endereço".into());
    }
    dados.truncate(dados.len().saturating_sub(6));
    Ok((hrp.to_string(), dados))
}

/// O endereço de 20 bytes no formato com verificador, para a rede dada.
pub fn mostrar(endereco: &[u8; ADDRESS_LEN], nome_da_rede: &str) -> String {
    codificar(prefixo(nome_da_rede), endereco).unwrap_or_default()
}

/// Lê um endereço digitado ou colado: o formato com verificador da rede certa,
/// ou o hexadecimal antigo de 40 dígitos.
pub fn ler(texto: &str, nome_da_rede: &str) -> Result<[u8; ADDRESS_LEN], String> {
    let t = texto.trim();
    if t.len() == ADDRESS_LEN * 2 && t.bytes().all(|b| b.is_ascii_hexdigit()) {
        return crate::de_hex(t).ok_or_else(|| "endereço hexadecimal inválido".into());
    }
    let (hrp, dados) = ler_bech32m(t)?;
    let esperado = prefixo(nome_da_rede);
    if hrp != esperado {
        let de_qual = match hrp.as_str() {
            "hyx" => "da rede principal",
            "thyx" => "da rede de teste",
            "rhyx" => "da rede local",
            _ => "de outra coisa, não do Hyurax",
        };
        return Err(format!("este endereço é {de_qual}; aqui a rede espera um que comece com {esperado}1"));
    }
    let bytes = reagrupar(&dados, 5, 8, false).ok_or("endereço com bits que sobram")?;
    <[u8; ADDRESS_LEN]>::try_from(bytes.as_slice()).map_err(|_| "endereço com tamanho errado".to_string())
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    /// Vetores válidos do BIP-350 (bech32m): o verificador tem de aceitar.
    #[test]
    fn vetores_do_bip350() {
        for v in [
            "A1LQFN3A",
            "a1lqfn3a",
            "an83characterlonghumanreadablepartthatcontainsthetheexcludedcharactersbioandnumber11sg7hg6",
            "abcdef1l7aum6echk45nj3s0wdvt2fg8x9yrzpqzd3ryx",
            "split1checkupstagehandshakeupstreamerranterredcaperredlc445v",
            "?1v759aa",
        ] {
            assert!(ler_bech32m(v).is_ok(), "{v}");
        }
        // e recusar: verificador de bech32 (o antigo), caractere fora, mistura de caixa
        for v in ["abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw", "a1lqfn3b", "A1lqfn3a"] {
            assert!(ler_bech32m(v).is_err(), "{v}");
        }
    }

    #[test]
    fn ida_e_volta_nas_tres_redes() {
        let e = [0xc0u8, 0xde, 0x48, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0x1b, 0x0e];
        for rede in ["hyurax-mainnet", "hyurax-testnet", "hyurax-regtest"] {
            let texto = mostrar(&e, rede);
            assert!(texto.starts_with(prefixo(rede)), "{texto}");
            assert_eq!(ler(&texto, rede), Ok(e));
            assert_eq!(ler(&texto.to_uppercase(), rede), Ok(e), "maiúsculas também valem");
        }
        assert_eq!(mostrar(&e, "hyurax-testnet").len(), 4 + 1 + 32 + 6);
    }

    #[test]
    fn um_caractere_trocado_e_pego() {
        let e = [7u8; 20];
        let certo = mostrar(&e, "testnet");
        for posicao in 5..certo.len() {
            let mut errado: Vec<u8> = certo.bytes().collect();
            errado[posicao] = if errado[posicao] == b'q' { b'p' } else { b'q' };
            let errado = String::from_utf8(errado).unwrap();
            assert!(ler(&errado, "testnet").is_err(), "trocar a posição {posicao} passou: {errado}");
        }
    }

    #[test]
    fn rede_errada_e_recusada() {
        let e = [7u8; 20];
        let principal = mostrar(&e, "mainnet");
        let erro = ler(&principal, "testnet").unwrap_err();
        assert!(erro.contains("rede principal"), "{erro}");
    }

    #[test]
    fn hexadecimal_antigo_continua_valendo() {
        let e = [0xabu8; 20];
        assert_eq!(ler(&"ab".repeat(20), "testnet"), Ok(e));
    }
}
