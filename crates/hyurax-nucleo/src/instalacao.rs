//! A instalação no Windows: onde o programa mora e o manifesto do que o
//! instalador pôs no disco.
//!
//! O instalador (`hyurax-instalador`) grava, ao lado do `Hyurax.exe`, o
//! arquivo `instalacao.txt`: versão, rede, a chave de "Aplicativos" do
//! Windows, os atalhos e os arquivos do programa. A desinstalação
//! (`Hyurax.exe --desinstalar`) lê esse manifesto e apaga exatamente o que
//! está nele — nunca uma pasta inteira às cegas. Configuração, carteira e
//! dados do usuário ficam em outras pastas ([`crate::pastas`]) e nunca
//! aparecem aqui.
//!
//! O formato é texto, uma entrada por linha (`campo=valor`), para a pessoa
//! poder ler e conferir.

use std::path::{Path, PathBuf};

/// O nome do manifesto, na pasta do programa.
pub const MANIFESTO: &str = "instalacao.txt";

/// A chave de "Aplicativos" (desinstalar), só para o usuário atual.
pub const CHAVE_PADRAO: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Hyurax";

/// O nome que aparece em "Aplicativos" e nos atalhos.
pub const NOME: &str = "Hyurax / Ultrax";

/// Os arquivos que o instalador põe na pasta do programa.
pub const ARQUIVOS: &[&str] = &["Hyurax.exe", "WebView2Loader.dll", "TERMOS-DE-USO.txt", "LEIA-ME.txt", "Hyurax.ico", MANIFESTO];

/// A pasta do programa: `%LOCALAPPDATA%\Programs\Hyurax` (sem administrador).
///
/// # Errors
/// Sem `LOCALAPPDATA` no ambiente.
pub fn pasta_do_programa() -> Result<PathBuf, String> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("não achei a pasta LOCALAPPDATA do usuário")?;
    Ok(PathBuf::from(base).join("Programs").join("Hyurax"))
}

/// O que o instalador registrou.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifesto {
    pub versao: String,
    pub rede: String,
    pub chave: String,
    pub atalhos: Vec<PathBuf>,
    pub arquivos: Vec<String>,
}

impl Manifesto {
    /// O manifesto em texto.
    pub fn texto(&self) -> String {
        let mut t = String::from(
            "# Hyurax / Ultrax: o que o instalador pôs neste computador.\n\
             # A desinstalação apaga só o que está listado aqui. A carteira, a\n\
             # configuração e os dados ficam em outras pastas e não são tocados.\n",
        );
        for (campo, valor) in [("produto", NOME), ("versao", &self.versao), ("rede", &self.rede), ("chave", &self.chave)] {
            t.push_str(campo);
            t.push('=');
            t.push_str(valor);
            t.push('\n');
        }
        for a in &self.atalhos {
            t.push_str("atalho=");
            t.push_str(&a.display().to_string());
            t.push('\n');
        }
        for a in &self.arquivos {
            t.push_str("arquivo=");
            t.push_str(a);
            t.push('\n');
        }
        t
    }

    /// Lê o manifesto. Recusa nome de arquivo com caminho (só nomes soltos
    /// na pasta do programa) e atalho que não seja `.lnk`.
    ///
    /// # Errors
    /// Linha inválida, ou faltando versão ou chave.
    pub fn ler(texto: &str) -> Result<Self, String> {
        let mut m = Self::default();
        for linha in texto.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let (campo, valor) = linha.split_once('=').ok_or_else(|| format!("linha sem '=': {linha}"))?;
            match campo {
                "produto" => {}
                "versao" => valor.clone_into(&mut m.versao),
                "rede" => valor.clone_into(&mut m.rede),
                "chave" => {
                    if !valor.starts_with(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Hyurax") {
                        return Err(format!("chave fora do lugar do Hyurax: {valor}"));
                    }
                    valor.clone_into(&mut m.chave);
                }
                "atalho" => {
                    let p = PathBuf::from(valor);
                    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("lnk")) {
                        return Err(format!("atalho que não é .lnk: {valor}"));
                    }
                    m.atalhos.push(p);
                }
                "arquivo" => {
                    if valor.is_empty() || valor.contains(['/', '\\', ':']) || valor == "." || valor == ".." {
                        return Err(format!("arquivo com caminho: {valor}"));
                    }
                    m.arquivos.push(valor.to_string());
                }
                outro => return Err(format!("campo desconhecido: {outro}")),
            }
        }
        if m.versao.is_empty() || m.chave.is_empty() {
            return Err("manifesto sem versão ou sem chave".into());
        }
        Ok(m)
    }

    /// Lê o manifesto da pasta do programa.
    ///
    /// # Errors
    /// Sem manifesto, ou manifesto inválido.
    pub fn da_pasta(pasta: &Path) -> Result<Self, String> {
        let t = std::fs::read_to_string(pasta.join(MANIFESTO)).map_err(|e| format!("sem {MANIFESTO} em {}: {e}", pasta.display()))?;
        Self::ler(&t)
    }
}

/// A versão instalada, se houver (para o instalador dizer "atualizar").
pub fn versao_instalada(pasta: &Path) -> Option<String> {
    Manifesto::da_pasta(pasta).ok().map(|m| m.versao)
}

/// Apaga os arquivos do manifesto e os atalhos. Devolve o que não conseguiu
/// apagar (arquivo em uso, por exemplo). A pasta só sai se ficar vazia.
pub fn remover_arquivos(pasta: &Path, m: &Manifesto, menos: &[&str]) -> Vec<String> {
    let mut falhas = Vec::new();
    for a in &m.atalhos {
        if a.exists()
            && let Err(e) = std::fs::remove_file(a)
        {
            falhas.push(format!("{}: {e}", a.display()));
        }
    }
    for nome in m.arquivos.iter().filter(|n| !menos.iter().any(|x| x.eq_ignore_ascii_case(n))) {
        let p = pasta.join(nome);
        if p.exists()
            && let Err(e) = std::fs::remove_file(&p)
        {
            falhas.push(format!("{}: {e}", p.display()));
        }
    }
    falhas
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn exemplo() -> Manifesto {
        Manifesto {
            versao: "1.0.0".into(),
            rede: "testnet".into(),
            chave: CHAVE_PADRAO.into(),
            atalhos: vec![PathBuf::from(r"C:\x\Hyurax.lnk")],
            arquivos: ARQUIVOS.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn ida_e_volta() {
        let m = exemplo();
        assert_eq!(Manifesto::ler(&m.texto()).unwrap(), m);
    }

    #[test]
    fn recusa_o_que_poderia_apagar_fora_da_pasta() {
        let base = exemplo().texto();
        for ruim in ["arquivo=..\\carteira.txt", "arquivo=C:\\Windows\\x.dll", "arquivo=a/b", "arquivo=..", "atalho=C:\\x\\carteira.txt", "chave=HKCU\\Software\\Outro", "lixo"] {
            assert!(Manifesto::ler(&format!("{base}{ruim}\n")).is_err(), "aceitou {ruim}");
        }
    }

    #[test]
    fn remove_so_o_listado_e_preserva_o_resto() {
        let pasta = std::env::temp_dir().join(format!("hyurax-instalacao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        for a in ARQUIVOS {
            std::fs::write(pasta.join(a), b"x").unwrap();
        }
        std::fs::write(pasta.join("meu-arquivo.txt"), b"do dono").unwrap();
        let mut m = exemplo();
        m.atalhos.clear();
        let falhas = remover_arquivos(&pasta, &m, &["Hyurax.exe"]);
        assert!(falhas.is_empty(), "{falhas:?}");
        assert!(pasta.join("Hyurax.exe").exists(), "o exe em uso fica para depois");
        assert!(pasta.join("meu-arquivo.txt").exists(), "arquivo fora do manifesto não sai");
        assert!(!pasta.join("WebView2Loader.dll").exists());
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
