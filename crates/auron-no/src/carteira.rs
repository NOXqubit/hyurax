//! Arquivo de carteira com senha (formato `AURON-CARTEIRA-v2`).
//!
//! O segredo Ed25519 nunca fica em texto no disco:
//!
//! - a senha vira uma chave de 32 bytes com **Argon2id** (64 MiB, 3 passadas,
//!   sal aleatório de 16 bytes). Os parâmetros ficam gravados no arquivo, para
//!   um aumento futuro não quebrar carteiras antigas;
//! - o segredo é cifrado com **ChaCha20-Poly1305**, nonce aleatório de 12 bytes;
//! - o dado associado é `AURON-CARTEIRA-v2 || endereço`: trocar o endereço do
//!   arquivo faz a abertura falhar, então ninguém engana a pessoa sobre de quem
//!   é a carteira.
//!
//! Quem rouba o arquivo precisa adivinhar a senha, e cada tentativa custa
//! 64 MiB e algumas centenas de milissegundos. Senha curta continua fraca: por
//! isso o mínimo de 10 caracteres.
//!
//! O formato antigo (`segredo=` em texto) ainda é lido, para poder ser
//! convertido com `carteira cifrar`.

use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, Tag};

use auron_crypto::{ADDRESS_LEN, SECRET_LEN, address_from_ed25519_pubkey, ed25519_public_key};

/// Nome do formato, na primeira linha útil do arquivo.
pub const FORMATO: &str = "AURON-CARTEIRA-v2";
/// Menor senha aceita.
pub const SENHA_MINIMA: usize = 10;

const MEMORIA_KIB: u32 = 64 * 1024;
const PASSADAS: u32 = 3;
const FAIXAS: u32 = 1;
/// Teto de memória aceito ao ABRIR um arquivo: um arquivo adulterado não pode
/// mandar o programa reservar gigabytes.
const MEMORIA_MAXIMA_KIB: u32 = 1024 * 1024;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn de_hex<const N: usize>(texto: &str) -> Option<[u8; N]> {
    let texto = texto.trim();
    if texto.len() != N.checked_mul(2)? {
        return None;
    }
    let mut saida = [0u8; N];
    for (i, byte) in saida.iter_mut().enumerate() {
        *byte = u8::from_str_radix(texto.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(saida)
}

fn campo<'a>(texto: &'a str, nome: &str) -> Option<&'a str> {
    texto.lines().find_map(|l| l.trim().strip_prefix(nome)?.strip_prefix('='))
}

fn derivar(senha: &str, sal: &[u8; 16], memoria_kib: u32, passadas: u32, faixas: u32) -> Result<[u8; 32], String> {
    if memoria_kib > MEMORIA_MAXIMA_KIB {
        return Err(format!("arquivo pede {memoria_kib} KiB de memória; recusado"));
    }
    let params = Params::new(memoria_kib, passadas, faixas, Some(32)).map_err(|e| format!("parâmetros do Argon2id: {e}"))?;
    let mut memoria = Vec::new();
    memoria.try_reserve_exact(params.block_count()).map_err(|_| "sem memória para abrir a carteira".to_string())?;
    memoria.resize(params.block_count(), Block::new());
    let mut chave = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into_with_memory(senha.as_bytes(), sal, &mut chave, &mut memoria)
        .map_err(|e| format!("Argon2id: {e}"))?;
    Ok(chave)
}

fn dado_associado(endereco: &[u8; ADDRESS_LEN]) -> Vec<u8> {
    let mut aad = FORMATO.as_bytes().to_vec();
    aad.extend_from_slice(endereco);
    aad
}

/// Confere a regra mínima de senha.
///
/// # Errors
/// Senha curta demais.
pub fn senha_aceitavel(senha: &str) -> Result<(), String> {
    if senha.chars().count() < SENHA_MINIMA {
        return Err(format!("a senha precisa de pelo menos {SENHA_MINIMA} caracteres"));
    }
    Ok(())
}

/// Monta o texto do arquivo de carteira cifrado.
///
/// `aleatorio` precisa trazer 28 bytes imprevisíveis: 16 de sal e 12 de nonce.
///
/// # Errors
/// Senha fraca ou falha do Argon2id.
pub fn cifrar(segredo: &[u8; SECRET_LEN], senha: &str, aleatorio: &[u8; 28]) -> Result<String, String> {
    senha_aceitavel(senha)?;
    let (sal, nonce) = aleatorio.split_at(16);
    let sal: [u8; 16] = sal.try_into().map_err(|_| "sal")?;
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| "nonce")?;
    let endereco = address_from_ed25519_pubkey(&ed25519_public_key(segredo));
    let chave = derivar(senha, &sal, MEMORIA_KIB, PASSADAS, FAIXAS)?;
    let mut dados = *segredo;
    let etiqueta = ChaCha20Poly1305::new(&Key::from(chave))
        .encrypt_in_place_detached(&Nonce::from(nonce), &dado_associado(&endereco), &mut dados)
        .map_err(|_| "falha ao cifrar".to_string())?;
    Ok(format!(
        "# Carteira Auron de TESTE, cifrada com senha.\n\
         # Sem a senha, este arquivo não gasta nada. Sem este arquivo E a senha,\n\
         # o saldo fica perdido: guarde uma cópia e não esqueça a senha.\n\
         # A rede pública não existe e o AUR não tem valor.\n\
         formato={FORMATO}\n\
         endereco={}\n\
         kdf=argon2id\n\
         memoria_kib={MEMORIA_KIB}\n\
         passadas={PASSADAS}\n\
         faixas={FAIXAS}\n\
         sal={}\n\
         nonce={}\n\
         cifrado={}\n\
         etiqueta={}\n",
        hex(&endereco),
        hex(&sal),
        hex(&nonce),
        hex(&dados),
        hex(etiqueta.as_slice()),
    ))
}

/// Se o arquivo é do formato antigo, com o segredo em texto.
pub fn e_formato_antigo(texto: &str) -> bool {
    campo(texto, "segredo").is_some() && campo(texto, "formato").is_none()
}

/// O endereço declarado no arquivo, sem pedir senha.
///
/// # Errors
/// Arquivo sem endereço legível.
pub fn endereco(texto: &str) -> Result<[u8; ADDRESS_LEN], String> {
    if e_formato_antigo(texto) {
        let segredo: [u8; SECRET_LEN] = campo(texto, "segredo").and_then(de_hex).ok_or("segredo inválido no arquivo")?;
        return Ok(address_from_ed25519_pubkey(&ed25519_public_key(&segredo)));
    }
    campo(texto, "endereco").and_then(de_hex).ok_or_else(|| "arquivo de carteira sem endereço".to_string())
}

/// Abre a carteira e devolve o segredo.
///
/// # Errors
/// Senha errada, arquivo alterado ou formato desconhecido. Senha errada e
/// arquivo alterado dão a mesma mensagem, de propósito.
pub fn abrir(texto: &str, senha: &str) -> Result<[u8; SECRET_LEN], String> {
    if e_formato_antigo(texto) {
        return campo(texto, "segredo").and_then(de_hex).ok_or_else(|| "segredo inválido no arquivo".to_string());
    }
    if campo(texto, "formato") != Some(FORMATO) {
        return Err("formato de carteira desconhecido".into());
    }
    if campo(texto, "kdf") != Some("argon2id") {
        return Err("derivação de chave desconhecida".into());
    }
    let numero = |nome: &str| -> Result<u32, String> {
        campo(texto, nome).and_then(|v| v.parse().ok()).ok_or_else(|| format!("campo {nome} inválido"))
    };
    let endereco_declarado = endereco(texto)?;
    let sal: [u8; 16] = campo(texto, "sal").and_then(de_hex).ok_or("sal inválido")?;
    let nonce: [u8; 12] = campo(texto, "nonce").and_then(de_hex).ok_or("nonce inválido")?;
    let mut dados: [u8; SECRET_LEN] = campo(texto, "cifrado").and_then(de_hex).ok_or("cifrado inválido")?;
    let etiqueta: [u8; 16] = campo(texto, "etiqueta").and_then(de_hex).ok_or("etiqueta inválida")?;

    let chave = derivar(senha, &sal, numero("memoria_kib")?, numero("passadas")?, numero("faixas")?)?;
    ChaCha20Poly1305::new(&Key::from(chave))
        .decrypt_in_place_detached(&Nonce::from(nonce), &dado_associado(&endereco_declarado), &mut dados, &Tag::from(etiqueta))
        .map_err(|_| "senha errada, ou arquivo de carteira alterado".to_string())?;
    // Conferência final: o segredo aberto precisa gerar o endereço declarado.
    if address_from_ed25519_pubkey(&ed25519_public_key(&dados)) != endereco_declarado {
        return Err("senha errada, ou arquivo de carteira alterado".into());
    }
    Ok(dados)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    const SEGREDO: [u8; 32] = [0x21; 32];
    const ALEATORIO: [u8; 28] = [0x5a; 28];
    const SENHA: &str = "cavalo correto bateria";

    #[test]
    fn abre_com_a_senha_certa() {
        let texto = cifrar(&SEGREDO, SENHA, &ALEATORIO).unwrap();
        assert!(!texto.contains(&hex(&SEGREDO)), "o segredo apareceu em texto");
        assert_eq!(abrir(&texto, SENHA).unwrap(), SEGREDO);
        assert_eq!(endereco(&texto).unwrap(), address_from_ed25519_pubkey(&ed25519_public_key(&SEGREDO)));
    }

    #[test]
    fn senha_errada_nao_abre() {
        let texto = cifrar(&SEGREDO, SENHA, &ALEATORIO).unwrap();
        assert!(abrir(&texto, "cavalo correto baterIa").is_err());
    }

    #[test]
    fn trocar_o_endereco_do_arquivo_nao_engana() {
        let texto = cifrar(&SEGREDO, SENHA, &ALEATORIO).unwrap();
        let outro = hex(&[0x99u8; 20]);
        let linha = texto.lines().find(|l| l.starts_with("endereco=")).unwrap();
        let adulterado = texto.replace(linha, &format!("endereco={outro}"));
        assert!(abrir(&adulterado, SENHA).is_err());
    }

    #[test]
    fn cifrado_alterado_nao_abre() {
        let texto = cifrar(&SEGREDO, SENHA, &ALEATORIO).unwrap();
        let linha = texto.lines().find(|l| l.starts_with("cifrado=")).unwrap();
        let mut bytes: Vec<char> = linha.chars().collect();
        let ultimo = bytes.len() - 1;
        bytes[ultimo] = if bytes[ultimo] == '0' { '1' } else { '0' };
        let adulterado = texto.replace(linha, &bytes.into_iter().collect::<String>());
        assert!(abrir(&adulterado, SENHA).is_err());
    }

    #[test]
    fn memoria_absurda_no_arquivo_e_recusada() {
        let texto = cifrar(&SEGREDO, SENHA, &ALEATORIO).unwrap();
        let adulterado = texto.replace("memoria_kib=65536", "memoria_kib=4000000000");
        assert!(abrir(&adulterado, SENHA).unwrap_err().contains("recusado"));
    }

    #[test]
    fn senha_curta_e_recusada() {
        assert!(cifrar(&SEGREDO, "curta", &ALEATORIO).is_err());
    }

    #[test]
    fn formato_antigo_ainda_e_lido() {
        let antigo = format!("segredo={}\nendereco=qualquer\n", hex(&SEGREDO));
        assert!(e_formato_antigo(&antigo));
        assert_eq!(abrir(&antigo, "").unwrap(), SEGREDO);
    }
}
