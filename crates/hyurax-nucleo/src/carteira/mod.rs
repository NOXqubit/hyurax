//! A carteira: o arquivo cifrado, o endereço, a senha, o envio de HYX e o
//! segundo fator.

pub mod arquivo;
pub mod endereco;
pub mod envio;
pub mod seguranca;
pub mod senha;
pub mod servico;
pub mod totp;

use std::path::Path;

use hyurax_crypto::SECRET_LEN;
use hyurax_net::entropia::preencher;

/// Cifra um segredo com a senha, com sal e nonce tirados do gerador do nó.
///
/// # Errors
/// Senha fraca ou gerador indisponível.
pub fn cifrar_segredo(segredo: &[u8; SECRET_LEN], senha: &str) -> Result<String, String> {
    let mut aleatorio = [0u8; 28];
    preencher(&mut aleatorio)?;
    arquivo::cifrar(segredo, senha, &aleatorio)
}

/// Grava a carteira sem nunca deixar um arquivo pela metade, e só para o dono.
///
/// # Errors
/// Sem permissão ou disco cheio.
pub fn gravar(destino: &Path, conteudo: &str) -> Result<(), String> {
    crate::arquivos::gravar_privado(destino, conteudo)
}
