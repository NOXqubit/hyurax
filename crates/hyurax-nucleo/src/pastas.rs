//! Onde o Hyurax guarda as coisas do usuário.
//!
//! Duas pastas, pela convenção de cada sistema:
//!
//! | | Windows | Linux | macOS |
//! |---|---|---|---|
//! | **configuração** (carteira, ajustes, termos, identidade do nó, segundo fator, sementes) | `%APPDATA%\Hyurax` | `$XDG_CONFIG_HOME/hyurax` (ou `~/.config/hyurax`) | `~/Library/Application Support/Hyurax` |
//! | **dados** (cadeia, ULTRAX, ciência, registros, cache da janela) | `%LOCALAPPDATA%\Hyurax` | `$XDG_DATA_HOME/hyurax` (ou `~/.local/share/hyurax`) | a mesma da configuração |
//!
//! A configuração é pequena e é do dono (a carteira está nela). Os dados são
//! grandes e se refazem: a cadeia baixa de novo da rede, e a ciência é o que
//! este computador calculou. No Windows, a configuração fica no perfil que
//! acompanha o usuário (`Roaming`), e os dados, no perfil local.
//!
//! Os programas **nunca** gravam na pasta de instalação.
//!
//! No terminal, `--pasta P` põe tudo numa pasta só (as duas iguais), que é o
//! que se quer num servidor ou num teste.

use std::path::{Path, PathBuf};

/// Nome da pasta do programa dentro das pastas do sistema.
const NOME: &str = "Hyurax";

/// As pastas do usuário.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pastas {
    /// Configuração e segredos.
    pub config: PathBuf,
    /// Dados do nó.
    pub dados: PathBuf,
}

impl Pastas {
    /// Tudo numa pasta só (terminal, servidor, testes).
    pub fn unica(pasta: impl Into<PathBuf>) -> Self {
        let p = pasta.into();
        Self { config: p.clone(), dados: p }
    }

    /// As pastas do sistema para este usuário.
    ///
    /// # Errors
    /// Sem as variáveis do perfil do usuário.
    pub fn do_sistema() -> Result<Self, String> {
        let var = |nome: &str| std::env::var_os(nome).filter(|v| !v.is_empty()).map(PathBuf::from);
        if cfg!(windows) {
            let config = var("APPDATA").ok_or("não achei a pasta APPDATA do usuário")?.join(NOME);
            let dados = var("LOCALAPPDATA").map_or_else(|| config.clone(), |l| l.join(NOME));
            Ok(Self { config, dados })
        } else if cfg!(target_os = "macos") {
            let p = var("HOME").ok_or("não achei a pasta HOME")?.join("Library").join("Application Support").join(NOME);
            Ok(Self::unica(p))
        } else {
            let casa = var("HOME").ok_or("não achei a pasta HOME")?;
            let config = var("XDG_CONFIG_HOME").unwrap_or_else(|| casa.join(".config")).join("hyurax");
            let dados = var("XDG_DATA_HOME").unwrap_or_else(|| casa.join(".local").join("share")).join("hyurax");
            Ok(Self { config, dados })
        }
    }

    /// Cria as duas pastas.
    ///
    /// # Errors
    /// Sem permissão ou disco cheio.
    pub fn criar(&self) -> Result<(), String> {
        for p in [&self.config, &self.dados, &self.registros()] {
            std::fs::create_dir_all(p).map_err(|e| format!("não consegui criar {}: {e}", p.display()))?;
        }
        Ok(())
    }

    /// O arquivo da carteira.
    pub fn carteira(&self) -> PathBuf {
        self.config.join("carteira.txt")
    }

    /// A pasta dos registros (logs).
    pub fn registros(&self) -> PathBuf {
        self.dados.join("registros")
    }

    /// O cache do navegador da janela (WebView2).
    pub fn navegador(&self) -> PathBuf {
        self.dados.join("navegador")
    }

    /// Trava a pasta de dados para este processo. Dois núcleos na mesma pasta
    /// (a janela e o terminal, por exemplo) gravariam a mesma cadeia por cima
    /// um do outro. A trava é do sistema operacional: sai sozinha quando o
    /// processo termina, mesmo se ele cair. Guarde o arquivo devolvido
    /// enquanto o núcleo viver.
    ///
    /// # Errors
    /// Outro Hyurax já usa esta pasta, ou sem permissão.
    pub fn travar(&self) -> Result<std::fs::File, String> {
        std::fs::create_dir_all(&self.dados).map_err(|e| format!("não consegui criar {}: {e}", self.dados.display()))?;
        let caminho = self.dados.join("em-uso.trava");
        let arquivo = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&caminho)
            .map_err(|e| format!("não consegui abrir {}: {e}", caminho.display()))?;
        match arquivo.try_lock() {
            Ok(()) => Ok(arquivo),
            Err(std::fs::TryLockError::WouldBlock) => Err(format!(
                "outro Hyurax já está usando a pasta de dados {} (o programa com janela ou outro terminal); feche o outro ou use --pasta",
                self.dados.display()
            )),
            Err(std::fs::TryLockError::Error(e)) => Err(format!("não consegui travar {}: {e}", caminho.display())),
        }
    }
}

/// O que a migração fez, para o registro.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Migracao {
    /// Itens movidos da pasta de configuração para a de dados.
    pub movidos: Vec<String>,
    /// Itens guardados em `arquivo-0.x` por não valerem mais na 1.0.
    pub arquivados: Vec<String>,
}

/// Marca gravada na pasta de dados depois da migração: com ela, nada é
/// movido de novo, e o que a 1.0 cria nunca vai para o arquivo.
const MARCA: &str = "versao-dos-dados.txt";

/// Leva o que as versões 0.x guardavam para o lugar da 1.0. Não apaga nada.
///
/// - Com as pastas separadas (programa com janela), a cadeia e o cache da
///   janela saem da configuração e vão para os dados.
/// - O histórico do ULTRAX e os JOBs da ciência das 0.x foram assinados com
///   os rótulos antigos (`UTRAX-…`). A 1.0 não consegue mais conferi-los,
///   então eles vão para `arquivo-0.x`, com um aviso, em vez de aparecerem
///   como trabalho conferido. O registro antigo vai junto.
///
/// # Errors
/// Falha ao mover.
pub fn migrar_da_0x(p: &Pastas) -> Result<Migracao, String> {
    let mut m = Migracao::default();
    let marca = p.dados.join(MARCA);
    if marca.exists() {
        return Ok(m);
    }
    let mover = |de: &Path, para: &Path| -> Result<(), String> {
        if let Some(pai) = para.parent() {
            std::fs::create_dir_all(pai).map_err(|e| format!("não consegui criar {}: {e}", pai.display()))?;
        }
        std::fs::rename(de, para).map_err(|e| format!("não consegui mover {} para {}: {e}", de.display(), para.display()))
    };
    if let Ok(itens) = std::fs::read_dir(&p.config) {
        for item in itens.flatten() {
            let nome = item.file_name().to_string_lossy().into_owned();
            let de = item.path();
            let destino = if nome == "ultrax" || nome == "ciencia" || nome == "hyurax.log" {
                Some((p.dados.join("arquivo-0.x").join(&nome), true))
            } else if p.config != p.dados && (nome.ends_with(".cadeia") || nome == "navegador") {
                Some((p.dados.join(&nome), false))
            } else {
                None
            };
            if let Some((para, arquivado)) = destino
                && !para.exists()
            {
                mover(&de, &para)?;
                if arquivado { m.arquivados.push(nome) } else { m.movidos.push(nome) }
            }
        }
    }
    if !m.arquivados.is_empty() {
        let leia = p.dados.join("arquivo-0.x").join("LEIA-ME.txt");
        let _ = std::fs::write(
            leia,
            "Histórico das versões 0.x do Hyurax (ULTRAX, JOBs da ciência e registro).
             Foi guardado aqui, e não apagado, na passagem para a 1.0.
             A 1.0 usa rótulos de domínio novos (HYURAX-ULTRAX-...), então não confere mais
             as assinaturas destes registros e não os mostra como trabalho conferido.
             Pode apagar esta pasta quando quiser. A carteira NÃO está aqui.
",
        );
    }
    std::fs::create_dir_all(&p.dados).map_err(|e| format!("não consegui criar {}: {e}", p.dados.display()))?;
    std::fs::write(&marca, "1
").map_err(|e| format!("não consegui gravar {}: {e}", marca.display()))?;
    m.arquivados.sort();
    m.movidos.sort();
    Ok(m)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn tmp(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hyurax-pastas-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn migracao_move_a_cadeia_arquiva_o_ultrax_e_nao_toca_na_carteira() {
        let raiz = tmp("migra");
        let p = Pastas { config: raiz.join("roaming"), dados: raiz.join("local") };
        std::fs::create_dir_all(p.config.join("ultrax")).unwrap();
        std::fs::create_dir_all(p.config.join("navegador")).unwrap();
        std::fs::write(p.config.join("carteira.txt"), "formato=HYURAX-CARTEIRA-v2\n").unwrap();
        std::fs::write(p.config.join("hyurax-testnet.cadeia"), b"cadeia").unwrap();
        std::fs::write(p.config.join("ultrax").join("historico.txt"), b"x").unwrap();
        let m = migrar_da_0x(&p).unwrap();
        assert!(p.config.join("carteira.txt").exists(), "a carteira fica onde está");
        assert!(p.dados.join("hyurax-testnet.cadeia").exists());
        assert!(p.dados.join("navegador").is_dir());
        assert!(p.dados.join("arquivo-0.x").join("ultrax").join("historico.txt").exists());
        assert!(p.dados.join("arquivo-0.x").join("LEIA-ME.txt").exists());
        assert_eq!(m.arquivados, vec!["ultrax".to_string()]);
        // de novo: nada a fazer
        assert_eq!(migrar_da_0x(&p).unwrap(), Migracao::default());
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn a_pasta_de_dados_so_aceita_um_nucleo_por_vez() {
        let raiz = tmp("trava");
        let p = Pastas::unica(&raiz);
        let primeira = p.travar().unwrap();
        assert!(p.travar().unwrap_err().contains("outro Hyurax"));
        drop(primeira);
        assert!(p.travar().is_ok(), "soltou quando o dono fechou");
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn pasta_unica_arquiva_o_ultrax_antigo_uma_vez_so() {
        let raiz = tmp("unica");
        std::fs::create_dir_all(raiz.join("ultrax")).unwrap();
        std::fs::write(raiz.join("hyurax-regtest.cadeia"), b"c").unwrap();
        let m = migrar_da_0x(&Pastas::unica(&raiz)).unwrap();
        assert_eq!(m.arquivados, vec!["ultrax".to_string()]);
        assert!(m.movidos.is_empty(), "com uma pasta só, a cadeia fica");
        assert!(raiz.join("hyurax-regtest.cadeia").exists());
        // o ULTRAX que a 1.0 criar depois não vai para o arquivo
        std::fs::create_dir_all(raiz.join("ultrax")).unwrap();
        assert_eq!(migrar_da_0x(&Pastas::unica(&raiz)).unwrap(), Migracao::default());
        assert!(raiz.join("ultrax").is_dir());
        let _ = std::fs::remove_dir_all(raiz);
    }
}
