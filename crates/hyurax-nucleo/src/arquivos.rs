//! Leitura e gravação de arquivos do usuário.

use std::path::Path;

/// Lê um arquivo de texto.
///
/// # Errors
/// Arquivo ausente ou ilegível, com o caminho na mensagem.
pub fn ler(arquivo: &Path) -> Result<String, String> {
    std::fs::read_to_string(arquivo).map_err(|e| format!("não consegui ler {}: {e}", arquivo.display()))
}

/// Grava num arquivo temporário ao lado e troca de nome: quem lê nunca pega o
/// arquivo pela metade, e uma queda de energia deixa o antigo inteiro.
///
/// # Errors
/// Sem permissão ou disco cheio.
pub fn gravar_atomico(arquivo: &Path, conteudo: &[u8]) -> Result<(), String> {
    let temporario = arquivo.with_extension("tmp");
    std::fs::write(&temporario, conteudo).map_err(|e| format!("não consegui gravar {}: {e}", arquivo.display()))?;
    std::fs::rename(&temporario, arquivo).map_err(|e| format!("não consegui gravar {}: {e}", arquivo.display()))
}

/// Como [`gravar_atomico`], para arquivos com segredo (carteira, identidade do
/// nó): no Linux e no macOS, só o dono lê.
///
/// # Errors
/// Sem permissão ou disco cheio.
pub fn gravar_privado(arquivo: &Path, conteudo: &str) -> Result<(), String> {
    gravar_atomico(arquivo, conteudo.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(arquivo, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
