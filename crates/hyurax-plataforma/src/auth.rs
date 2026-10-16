//! Senhas, sessões, códigos e chaves (seções 6, 7 e 30 do documento da V1).
//!
//! - **Senha:** Argon2id (m = 19 MiB, t = 2, p = 1, a recomendação da OWASP),
//!   sal de 16 bytes, guardada como `argon2id$v=19$m=19456,t=2,p=1$sal$hash`.
//!   Nunca em texto puro, nunca no diário. No máximo duas contas de senha ao
//!   mesmo tempo: quem tenta milhares de logins não esgota a memória.
//! - **Token de sessão, chave de API, código de vínculo e de recuperação:**
//!   32 (ou menos) bytes do sistema, em base32. O que se guarda é o hash
//!   (SHA-512 com domínio próprio), e a comparação é em tempo constante.

use std::sync::{Condvar, Mutex};

use argon2::{Algorithm, Argon2, Block, Params, Version};
use hyurax_crypto::sha512;

use crate::util::{aleatorio, base32, de_hex, hex, iguais};

const M_KIB: u32 = 19_456;
const T: u32 = 2;
const P: u32 = 1;
const SAIDA: usize = 32;

/// Domínio do hash dos segredos (tokens, chaves e códigos).
pub const DOMINIO_SEGREDO: &[u8] = concat!(hyurax_identidade::raiz!(), "-PLATAFORMA-SEGREDO-v1").as_bytes();

/// Senhas que todo mundo tenta primeiro.
const COMUNS: [&str; 24] = [
    "1234567890",
    "12345678910",
    "123456789a",
    "0123456789",
    "1q2w3e4r5t",
    "qwertyuiop",
    "senha12345",
    "senha123456",
    "password123",
    "password1234",
    "abcdefghij",
    "abc1234567",
    "iloveyou12",
    "brasil2026",
    "flamengo123",
    "corinthians",
    "palmeiras1",
    "minhasenha",
    "mudar12345",
    "admin12345",
    "hyurax1234",
    "hyuraxcompute",
    "1111111111",
    "aaaaaaaaaa",
];

struct Fila {
    ocupadas: Mutex<u32>,
    livre: Condvar,
}

static FILA: Fila = Fila { ocupadas: Mutex::new(0), livre: Condvar::new() };

fn com_vaga<R>(f: impl FnOnce() -> R) -> R {
    if let Ok(mut n) = FILA.ocupadas.lock() {
        while *n >= 2 {
            match FILA.livre.wait(n) {
                Ok(g) => n = g,
                Err(_) => return f(),
            }
        }
        *n = n.saturating_add(1);
    }
    let r = f();
    if let Ok(mut n) = FILA.ocupadas.lock() {
        *n = n.saturating_sub(1);
    }
    FILA.livre.notify_one();
    r
}

fn argon(senha: &[u8], sal: &[u8], m: u32, t: u32, p: u32) -> Result<[u8; SAIDA], String> {
    let params = Params::new(m, t, p, Some(SAIDA)).map_err(|e| format!("parâmetros do Argon2: {e}"))?;
    let blocos = params.block_count();
    let mut memoria = Vec::new();
    memoria.try_reserve_exact(blocos).map_err(|_| "sem memória para conferir a senha".to_string())?;
    memoria.resize(blocos, Block::new());
    let mut saida = [0u8; SAIDA];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into_with_memory(senha, sal, &mut saida, &mut memoria)
        .map_err(|e| format!("Argon2: {e}"))?;
    Ok(saida)
}

/// O hash de uma senha nova.
pub fn hash_da_senha(senha: &str) -> Result<String, String> {
    let sal: [u8; 16] = aleatorio()?;
    let h = com_vaga(|| argon(senha.as_bytes(), &sal, M_KIB, T, P))?;
    Ok(format!("argon2id$v=19$m={M_KIB},t={T},p={P}${}${}", hex(&sal), hex(&h)))
}

/// A senha confere com o hash guardado?
pub fn senha_confere(senha: &str, guardado: &str) -> bool {
    let partes: Vec<&str> = guardado.split('$').collect();
    let (Some(&"argon2id"), Some(&"v=19"), Some(params), Some(sal), Some(h)) = (partes.first(), partes.get(1), partes.get(2), partes.get(3), partes.get(4)) else {
        return false;
    };
    let mut m = 0;
    let mut t = 0;
    let mut p = 0;
    for par in params.split(',') {
        match par.split_once('=') {
            Some(("m", v)) => m = v.parse().unwrap_or(0),
            Some(("t", v)) => t = v.parse().unwrap_or(0),
            Some(("p", v)) => p = v.parse().unwrap_or(0),
            _ => return false,
        }
    }
    // só os parâmetros que esta versão grava: um hash com m enorme no diário
    // não vira um jeito de travar o servidor
    if (m, t, p) != (M_KIB, T, P) {
        return false;
    }
    let (Some(sal), Some(esperado)) = (de_hex(sal), de_hex(h)) else { return false };
    com_vaga(|| argon(senha.as_bytes(), &sal, m, t, p)).is_ok_and(|calc| iguais(&calc, &esperado))
}

/// Uma senha só para manter o tempo da resposta igual quando o e-mail não
/// existe (quem tenta descobrir contas pelo tempo não descobre).
pub fn gastar_tempo_como_login(senha: &str) {
    let _ = com_vaga(|| argon(senha.as_bytes(), &[0u8; 16], M_KIB, T, P));
}

/// Regras da senha: de 10 a 128 caracteres, não pode ser só repetição nem
/// estar na lista das mais comuns, nem conter o e-mail.
pub fn validar_senha(senha: &str, email: &str) -> Result<(), &'static str> {
    let n = senha.chars().count();
    if n < 10 {
        return Err("a senha precisa de pelo menos 10 caracteres");
    }
    if n > 128 {
        return Err("a senha passa de 128 caracteres");
    }
    let minuscula = senha.to_lowercase();
    if COMUNS.contains(&minuscula.as_str()) {
        return Err("essa senha está entre as mais usadas; escolha outra");
    }
    let mut distintos: Vec<char> = minuscula.chars().collect();
    distintos.sort_unstable();
    distintos.dedup();
    if distintos.len() < 4 {
        return Err("a senha repete poucos caracteres; misture mais");
    }
    if let Some(local) = email.split('@').next().filter(|l| l.len() >= 4)
        && minuscula.contains(&local.to_lowercase())
    {
        return Err("a senha não pode conter o seu e-mail");
    }
    Ok(())
}

/// Confere e normaliza um e-mail.
pub fn validar_email(email: &str) -> Option<String> {
    let e = email.trim().to_lowercase();
    let (local, dominio) = e.split_once('@')?;
    let ok = !local.is_empty()
        && local.len() <= 64
        && dominio.len() <= 190
        && dominio.contains('.')
        && !dominio.starts_with('.')
        && !dominio.ends_with('.')
        && !dominio.contains("..")
        && e.chars().all(|c| c.is_ascii_alphanumeric() || "@.-_+".contains(c))
        && dominio.chars().all(|c| c.is_ascii_alphanumeric() || ".-".contains(c));
    ok.then_some(e)
}

/// Hash de um segredo (token, chave, código).
pub fn hash_do_segredo(segredo: &str) -> String {
    let mut dados = DOMINIO_SEGREDO.to_vec();
    dados.extend_from_slice(segredo.as_bytes());
    hex(&sha512(&dados))
}

/// Um token de sessão novo: `(token, hash)`.
pub fn token_de_sessao() -> Result<(String, String), String> {
    let b: [u8; 32] = aleatorio()?;
    let t = format!("hyxs_{}", base32(&b));
    let h = hash_do_segredo(&t);
    Ok((t, h))
}

/// Uma chave de API nova: `(chave, prefixo, hash)`. A chave é mostrada uma
/// vez; depois só o prefixo aparece.
pub fn chave_de_api() -> Result<(String, String, String), String> {
    let b: [u8; 32] = aleatorio()?;
    let c = format!("hyx_{}", base32(&b));
    let prefixo = c.chars().take(12).collect();
    let h = hash_do_segredo(&c);
    Ok((c, prefixo, h))
}

/// Um código de vínculo de computador: `HYX-ABCD-EFGH`, válido por pouco tempo.
pub fn codigo_de_vinculo() -> Result<(String, String), String> {
    let b: [u8; 5] = aleatorio()?;
    let s = base32(&b).to_uppercase();
    let c = format!("HYX-{}-{}", s.get(..4).unwrap_or(""), s.get(4..8).unwrap_or(""));
    let h = hash_do_segredo(&c);
    Ok((c, h))
}

/// Normaliza o que a pessoa digitou do código de vínculo.
pub fn normalizar_codigo(texto: &str) -> String {
    let limpo: String = texto.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_uppercase();
    let corpo = limpo.strip_prefix("HYX").unwrap_or(&limpo);
    format!("HYX-{}-{}", corpo.get(..4).unwrap_or(""), corpo.get(4..8).unwrap_or(""))
}

/// Oito códigos de recuperação: `(códigos, hashes)`.
pub fn codigos_de_recuperacao() -> Result<(Vec<String>, Vec<String>), String> {
    let mut codigos = Vec::with_capacity(8);
    let mut hashes = Vec::with_capacity(8);
    for _ in 0..8 {
        let b: [u8; 8] = aleatorio()?;
        let s = base32(&b);
        let c = format!("{}-{}-{}", s.get(..4).unwrap_or(""), s.get(4..8).unwrap_or(""), s.get(8..12).unwrap_or(""));
        hashes.push(hash_do_segredo(&c));
        codigos.push(c);
    }
    Ok((codigos, hashes))
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn senha_ida_e_volta() {
        let h = hash_da_senha("cavalo-bateria-grampo").unwrap();
        assert!(h.starts_with("argon2id$v=19$m=19456,t=2,p=1$"));
        assert!(!h.contains("cavalo"));
        assert!(senha_confere("cavalo-bateria-grampo", &h));
        assert!(!senha_confere("cavalo-bateria-grampO", &h));
        // parâmetros fora do que esta versão grava não são conferidos
        assert!(!senha_confere("cavalo-bateria-grampo", &h.replace("m=19456", "m=4194304")));
        assert!(!senha_confere("x", "lixo"));
    }

    #[test]
    fn regras_da_senha_e_do_email() {
        assert!(validar_senha("curta", "a@b.com").is_err());
        assert!(validar_senha("1234567890", "a@b.com").is_err());
        assert!(validar_senha("abababababab", "a@b.com").is_err());
        assert!(validar_senha("joaosilva-2026!", "joaosilva@x.com").is_err());
        assert!(validar_senha("ponte-azul-77-rio", "joao@x.com").is_ok());
        assert_eq!(validar_email(" Joao@Exemplo.COM ").as_deref(), Some("joao@exemplo.com"));
        assert_eq!(validar_email("sem-arroba"), None);
        assert_eq!(validar_email("a@b"), None);
        assert_eq!(validar_email("a@b..com"), None);
        assert_eq!(validar_email("a<script>@b.com"), None);
    }

    #[test]
    fn segredos_tem_formato_e_hash() {
        let (t, h) = token_de_sessao().unwrap();
        assert!(t.starts_with("hyxs_") && t.len() > 50);
        assert_eq!(hash_do_segredo(&t), h);
        let (c, p, h) = chave_de_api().unwrap();
        assert!(c.starts_with("hyx_") && c.starts_with(&p) && p.len() == 12);
        assert_eq!(hash_do_segredo(&c), h);
        let (v, _) = codigo_de_vinculo().unwrap();
        assert_eq!(v.len(), 13);
        assert_eq!(normalizar_codigo(&v.to_lowercase().replace('-', " ")), v);
        let (cs, hs) = codigos_de_recuperacao().unwrap();
        assert_eq!(cs.len(), 8);
        assert_eq!(hash_do_segredo(&cs[0]), hs[0]);
    }
}
