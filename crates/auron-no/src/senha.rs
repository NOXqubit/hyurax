//! Ler a senha sem mostrar na tela, sem biblioteca externa.
//!
//! - Linux e Android (Termux): desliga o eco do terminal com `stty -echo`.
//! - Windows: `Read-Host -AsSecureString` do PowerShell, que já esconde.
//!
//! Para uso em script existe a variável `AURON_SENHA`. Ela é conveniente e
//! menos segura: fica no ambiente do processo e, às vezes, no histórico do
//! terminal.

use std::io::Write;

/// Nome da variável de ambiente com a senha, para scripts.
pub const VARIAVEL: &str = "AURON_SENHA";

/// Pede a senha ao usuário.
///
/// # Errors
/// Terminal indisponível ou leitura interrompida.
pub fn ler(pergunta: &str) -> Result<String, String> {
    if let Ok(senha) = std::env::var(VARIAVEL) {
        return Ok(senha);
    }
    ler_escondido(pergunta)
}

#[cfg(unix)]
fn ler_escondido(pergunta: &str) -> Result<String, String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};
    let tty = || std::fs::File::open("/dev/tty").map(Stdio::from);
    eprint!("{pergunta}: ");
    let _ = std::io::stderr().flush();
    let escondeu = tty().ok().and_then(|t| Command::new("stty").arg("-echo").stdin(t).status().ok()).is_some_and(|s| s.success());
    let mut linha = String::new();
    let lido = std::io::stdin().lock().read_line(&mut linha);
    if escondeu && let Ok(t) = tty() {
        let _ = Command::new("stty").arg("echo").stdin(t).status();
    }
    eprintln!();
    lido.map_err(|e| format!("não consegui ler a senha: {e}"))?;
    Ok(linha.trim_end_matches(['\r', '\n']).to_string())
}

#[cfg(windows)]
fn ler_escondido(pergunta: &str) -> Result<String, String> {
    use std::process::{Command, Stdio};
    let _ = std::io::stderr().flush();
    let script = format!(
        "$s = Read-Host '{}' -AsSecureString; \
         $p = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($s); \
         try {{ [Console]::Out.Write([Runtime.InteropServices.Marshal]::PtrToStringBSTR($p)) }} \
         finally {{ [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($p) }}",
        pergunta.replace('\'', "")
    );
    let saida = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("não consegui pedir a senha pelo PowerShell: {e}"))?;
    if !saida.status.success() {
        return Err("leitura da senha interrompida".into());
    }
    String::from_utf8(saida.stdout).map_err(|_| "senha com caracteres inválidos".into())
}
