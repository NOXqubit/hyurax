//! Código de 6 dígitos do Google Authenticator (TOTP, RFC 6238), em Rust puro.
//!
//! O aplicativo do celular e este programa guardam o mesmo segredo de 20 bytes.
//! A cada 30 segundos os dois calculam o mesmo número de 6 dígitos a partir do
//! relógio, sem trocar nada pela internet. Nenhum servidor no meio: o Hyurax
//! não fala com o Google nem com ninguém para conferir o código.
//!
//! O que isto protege, honestamente: quem senta neste computador e sabe (ou vê)
//! a sua senha ainda não consegue enviar HYUR, porque falta o celular. O que
//! **não** protege: quem copia a pasta de dados leva o arquivo do segredo junto
//! e pode gerar os mesmos códigos. Segundo fator num computador só é isso; o
//! programa diz isso na tela em vez de prometer o que não cumpre.
//!
//! Tudo aqui é escrito na mão porque o projeto não aceita dependência nova:
//! SHA-1 (RFC 3174), HMAC (RFC 2104) e Base32 (RFC 4648). SHA-1 quebrado para
//! colisão não enfraquece o HMAC, e é o único hash que todo aplicativo de
//! autenticação aceita.

/// Quantos dígitos o código tem.
pub const DIGITOS: u32 = 6;
/// Quantos segundos cada código vale.
pub const PASSO_S: u64 = 30;
/// Tamanho do segredo, em bytes (160 bits, o do RFC 6238).
pub const SEGREDO_LEN: usize = 20;
/// Quantos passos de 30 s para trás e para frente ainda valem: o relógio do
/// celular nunca bate no milissegundo com o do computador.
const JANELA: i64 = 1;

const BLOCO: usize = 64;

/// SHA-1 de uma mensagem (RFC 3174).
fn sha1(dados: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    // Preenchimento: 0x80, zeros até faltarem 8 bytes, e o tamanho em bits.
    let mut mensagem = dados.to_vec();
    let bits = (dados.len() as u64).wrapping_mul(8);
    mensagem.push(0x80);
    while mensagem.len() % BLOCO != BLOCO - 8 {
        mensagem.push(0);
    }
    mensagem.extend_from_slice(&bits.to_be_bytes());

    for bloco in mensagem.as_chunks::<BLOCO>().0 {
        let mut w = [0u32; 80];
        for (slot, quatro) in w.iter_mut().zip(bloco.as_chunks::<4>().0) {
            *slot = u32::from_be_bytes(*quatro);
        }
        for i in 16..80 {
            // Toda leitura é de índice menor que `i`, que já foi preenchido.
            let pega = |k: usize| w.get(k).copied().unwrap_or(0);
            let v = pega(i - 3) ^ pega(i - 8) ^ pega(i - 14) ^ pega(i - 16);
            if let Some(slot) = w.get_mut(i) {
                *slot = v.rotate_left(1);
            }
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, palavra) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999_u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*palavra);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        let [h0, h1, h2, h3, h4] = h;
        h = [
            h0.wrapping_add(a),
            h1.wrapping_add(b),
            h2.wrapping_add(c),
            h3.wrapping_add(d),
            h4.wrapping_add(e),
        ];
    }

    let mut saida = [0u8; 20];
    for (destino, palavra) in saida.as_chunks_mut::<4>().0.iter_mut().zip(h.iter()) {
        *destino = palavra.to_be_bytes();
    }
    saida
}

/// HMAC-SHA1 (RFC 2104).
fn hmac_sha1(chave: &[u8], mensagem: &[u8]) -> [u8; 20] {
    let mut normal = [0u8; BLOCO];
    // Chave maior que o bloco entra pelo hash; menor, completada com zeros.
    let curta = if chave.len() > BLOCO { sha1(chave).to_vec() } else { chave.to_vec() };
    for (destino, origem) in normal.iter_mut().zip(curta.iter()) {
        *destino = *origem;
    }
    let mut dentro = Vec::with_capacity(BLOCO.saturating_add(mensagem.len()));
    let mut fora = Vec::with_capacity(BLOCO.saturating_add(20));
    for b in normal {
        dentro.push(b ^ 0x36);
        fora.push(b ^ 0x5c);
    }
    dentro.extend_from_slice(mensagem);
    fora.extend_from_slice(&sha1(&dentro));
    sha1(&fora)
}

/// O código de um contador (HOTP, RFC 4226).
fn codigo_do_contador(segredo: &[u8], contador: u64) -> u32 {
    let mac = hmac_sha1(segredo, &contador.to_be_bytes());
    // Truncamento dinâmico: os 4 bits finais dizem de onde tirar 31 bits.
    let inicio = usize::from(mac.last().copied().unwrap_or(0) & 0x0f);
    let mut quatro = [0u8; 4];
    for (destino, origem) in quatro.iter_mut().zip(mac.iter().skip(inicio)) {
        *destino = *origem;
    }
    let numero = u32::from_be_bytes(quatro) & 0x7fff_ffff;
    numero % 10_u32.pow(DIGITOS)
}

/// O código que vale agora, com zeros à esquerda.
#[must_use]
pub fn codigo(segredo: &[u8], agora_unix: u64) -> String {
    let n = codigo_do_contador(segredo, agora_unix / PASSO_S);
    format!("{n:0width$}", width = DIGITOS as usize)
}

/// Confere o código digitado, aceitando 30 s de folga para cada lado.
///
/// A comparação é dígito a dígito em tempo constante para o tamanho certo: não
/// vaza quantos dígitos iniciais acertaram.
#[must_use]
pub fn confere(segredo: &[u8], agora_unix: u64, digitado: &str) -> bool {
    let limpo: String = digitado.chars().filter(|c| c.is_ascii_digit()).collect();
    if limpo.len() != DIGITOS as usize {
        return false;
    }
    let passo = (agora_unix / PASSO_S) as i64;
    let mut bateu = false;
    for delta in -JANELA..=JANELA {
        let Ok(contador) = u64::try_from(passo.saturating_add(delta)) else { continue };
        let esperado = format!("{:0width$}", codigo_do_contador(segredo, contador), width = DIGITOS as usize);
        let mut iguais = 1u8;
        for (a, b) in esperado.bytes().zip(limpo.bytes()) {
            iguais &= u8::from(a == b);
        }
        bateu |= iguais == 1;
    }
    bateu
}

const ALFABETO: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Base32 sem preenchimento (RFC 4648), como os aplicativos de autenticação
/// esperam ver o segredo.
#[must_use]
pub fn base32(dados: &[u8]) -> String {
    let mut saida = String::new();
    let mut acumulado: u32 = 0;
    let mut bits: u32 = 0;
    for b in dados {
        acumulado = (acumulado << 8) | u32::from(*b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let indice = ((acumulado >> bits) & 0x1f) as usize;
            saida.push(char::from(ALFABETO.get(indice).copied().unwrap_or(b'A')));
        }
    }
    if bits > 0 {
        let indice = ((acumulado << (5 - bits)) & 0x1f) as usize;
        saida.push(char::from(ALFABETO.get(indice).copied().unwrap_or(b'A')));
    }
    saida
}

/// Lê um segredo em Base32 (para quem digita um segredo que já tem).
///
/// Ignora espaços, traços e o preenchimento `=`; devolve `None` em letra que
/// não existe no alfabeto.
#[must_use]
pub fn de_base32(texto: &str) -> Option<Vec<u8>> {
    let mut saida = Vec::new();
    let mut acumulado: u32 = 0;
    let mut bits: u32 = 0;
    for c in texto.chars() {
        if c.is_whitespace() || c == '-' || c == '=' {
            continue;
        }
        let letra = c.to_ascii_uppercase() as u8;
        let indice = ALFABETO.iter().position(|a| *a == letra)?;
        acumulado = (acumulado << 5) | (indice as u32);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            saida.push(((acumulado >> bits) & 0xff) as u8);
        }
    }
    if saida.is_empty() { None } else { Some(saida) }
}

/// O endereço `otpauth://` que o aplicativo do celular lê pelo QR Code.
///
/// `conta` é só um rótulo dentro do aplicativo. Vai limpo de caracteres que
/// estragariam o endereço.
#[must_use]
pub fn uri(segredo: &[u8], conta: &str) -> String {
    let rotulo: String = conta
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(32)
        .collect();
    let rotulo = if rotulo.is_empty() { "conta".to_string() } else { rotulo };
    format!(
        "otpauth://totp/Hyurax:{rotulo}?secret={}&issuer=Hyurax&algorithm=SHA1&digits={DIGITOS}&period={PASSO_S}",
        base32(segredo)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    fn hexa(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn sha1_nos_vetores_do_rfc_3174() {
        assert_eq!(hexa(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hexa(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hexa(&sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        // 1 000 000 de "a": pega erro no contador de bits acima de 2^20.
        let muitos = vec![b'a'; 1_000_000];
        assert_eq!(hexa(&sha1(&muitos)), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
        // Exatamente um bloco menos um byte, e o bloco cheio: as bordas do preenchimento.
        assert_eq!(hexa(&sha1(&[b'a'; 55])), "c1c8bbdc22796e28c0e15163d20899b65621d65a");
        assert_eq!(hexa(&sha1(&[b'a'; 56])), "c2db330f6083854c99d4b5bfb6e8f29f201be699");
    }

    #[test]
    fn hmac_sha1_nos_vetores_do_rfc_2202() {
        assert_eq!(hexa(&hmac_sha1(&[0x0b; 20], b"Hi There")), "b617318655057264e28bc0b6fb378c8ef146be00");
        assert_eq!(hexa(&hmac_sha1(b"Jefe", b"what do ya want for nothing?")), "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79");
        // Chave maior que o bloco: entra pelo hash antes.
        assert_eq!(
            hexa(&hmac_sha1(&[0xaa; 80], b"Test Using Larger Than Block-Size Key - Hash Key First")),
            "aa4ae5e15272d00e95705637ce8a3b55ed402112"
        );
    }

    #[test]
    fn totp_nos_vetores_do_rfc_6238() {
        // O RFC usa o segredo ASCII "12345678901234567890" e mostra 8 dígitos;
        // 6 dígitos é o mesmo número, cortado.
        let segredo = b"12345678901234567890";
        for (tempo, oito) in [
            (59_u64, "94287082"),
            (1_111_111_109, "07081804"),
            (1_111_111_111, "14050471"),
            (1_234_567_890, "89005924"),
            (2_000_000_000, "69279037"),
        ] {
            let seis = &oito[oito.len() - 6..];
            assert_eq!(codigo(segredo, tempo), seis, "tempo {tempo}");
        }
    }

    #[test]
    fn a_janela_aceita_o_codigo_de_antes_e_de_depois_e_nada_mais() {
        let segredo = [0x42u8; SEGREDO_LEN];
        let agora = 1_700_000_000u64;
        assert!(confere(&segredo, agora, &codigo(&segredo, agora)));
        assert!(confere(&segredo, agora, &codigo(&segredo, agora - PASSO_S)));
        assert!(confere(&segredo, agora, &codigo(&segredo, agora + PASSO_S)));
        assert!(!confere(&segredo, agora, &codigo(&segredo, agora + PASSO_S * 3)));
        // Espaço no meio é perdoado; tamanho errado e letra não são.
        let certo = codigo(&segredo, agora);
        assert!(confere(&segredo, agora, &format!("{} {}", &certo[..3], &certo[3..])));
        assert!(!confere(&segredo, agora, &certo[..5]));
        assert!(!confere(&segredo, agora, "abcdef"));
        assert!(!confere(&segredo, agora, ""));
    }

    #[test]
    fn base32_nos_vetores_do_rfc_4648_e_de_volta() {
        assert_eq!(base32(b"f"), "MY");
        assert_eq!(base32(b"fo"), "MZXQ");
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32(b"12345678901234567890"), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(de_base32("MZXW6YTBOI").unwrap(), b"foobar");
        assert_eq!(de_base32("mzxw 6ytb-oi=").unwrap(), b"foobar");
        assert!(de_base32("MZXW1").is_none()); // 1 não existe no alfabeto
        assert!(de_base32("").is_none());
    }

    #[test]
    fn o_endereco_otpauth_leva_o_segredo_e_limpa_o_rotulo() {
        let u = uri(&[0x0bu8; SEGREDO_LEN], "PC-01 do João/;?");
        assert!(u.starts_with("otpauth://totp/Hyurax:PC-01doJoo?secret=BMFQWCYL"), "{u}");
        assert!(u.contains("digits=6") && u.contains("period=30") && u.contains("algorithm=SHA1"));
    }
}
