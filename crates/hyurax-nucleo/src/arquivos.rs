//! Leitura e gravação de arquivos do usuário.

use std::io::Write;
use std::path::Path;

/// Lê um arquivo de texto.
///
/// # Errors
/// Arquivo ausente ou ilegível, com o caminho na mensagem.
pub fn ler(arquivo: &Path) -> Result<String, String> {
    std::fs::read_to_string(arquivo).map_err(|e| format!("não consegui ler {}: {e}", arquivo.display()))
}

/// Grava num arquivo temporário ao lado, manda o sistema descarregar no disco
/// (`sync_all`) e só então troca de nome: quem lê nunca pega o arquivo pela
/// metade, e uma queda de energia deixa o antigo ou o novo inteiro, nunca um
/// arquivo vazio com o nome certo.
///
/// # Errors
/// Sem permissão ou disco cheio.
pub fn gravar_atomico(arquivo: &Path, conteudo: &[u8]) -> Result<(), String> {
    gravar(arquivo, conteudo, false)
}

/// Como [`gravar_atomico`], para arquivos com segredo (carteira, identidade do
/// nó, segundo fator, chave do painel): no Linux e no macOS o temporário já
/// nasce legível só pelo dono, antes de receber o primeiro byte.
///
/// # Errors
/// Sem permissão ou disco cheio.
pub fn gravar_privado(arquivo: &Path, conteudo: &str) -> Result<(), String> {
    gravar(arquivo, conteudo.as_bytes(), true)
}

fn gravar(arquivo: &Path, conteudo: &[u8], privado: bool) -> Result<(), String> {
    let erro = |e: std::io::Error| format!("não consegui gravar {}: {e}", arquivo.display());
    let temporario = arquivo.with_extension("tmp");
    let mut opcoes = std::fs::OpenOptions::new();
    opcoes.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if privado {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = privado;
    let mut f = opcoes.open(&temporario).map_err(erro)?;
    f.write_all(conteudo).map_err(erro)?;
    f.sync_all().map_err(erro)?;
    drop(f);
    std::fs::rename(&temporario, arquivo).map_err(erro)?;
    // a troca de nome também precisa chegar ao disco (no Windows, o NTFS
    // grava o diário dos metadados sozinho; abrir uma pasta não é permitido)
    #[cfg(unix)]
    if let Some(pasta) = arquivo.parent()
        && let Ok(d) = std::fs::File::open(pasta)
    {
        let _ = d.sync_all();
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn grava_por_cima_sem_sobrar_temporario() {
        let p = std::env::temp_dir().join(format!("hyurax-arquivos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        let a = p.join("carteira.txt");
        gravar_privado(&a, "um").unwrap();
        gravar_privado(&a, "dois").unwrap();
        assert_eq!(ler(&a).unwrap(), "dois");
        assert!(!a.with_extension("tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&a).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&p);
    }
}
