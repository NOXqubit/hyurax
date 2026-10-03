//! Cópia de segurança e restauração (Documento Mestre, fase 10: disaster
//! recovery).
//!
//! `criar` copia as duas pastas do usuário para `DESTINO/hyurax-copia-<unix>/`
//! (`config/` e `dados/`) com um `MANIFESTO.txt`: caminho, tamanho e SHA-512
//! de cada arquivo. `conferir` refaz as contas. `restaurar` confere primeiro,
//! só roda com o programa fechado (trava da pasta de dados) e nunca apaga:
//! o que já existia vira `ARQUIVO.antes-da-restauracao-<unix>`.
//!
//! Fica de fora o que se refaz sozinho ou não deve ser copiado: a chave de
//! sessão do painel, a trava, o cache do navegador e os registros (logs).
//!
//! **A cópia leva segredos.** A carteira vai cifrada (a senha continua
//! valendo); o segundo fator (`seguranca.txt`) e a identidade do nó
//! (`no.chave`) vão em texto, como estão no disco. Guarde a cópia como
//! guardaria a carteira.

use std::path::{Path, PathBuf};

use hyurax_crypto::sha512;

use crate::pastas::Pastas;
use crate::util::hex;

/// O manifesto, na raiz da cópia.
pub const MANIFESTO: &str = "MANIFESTO.txt";
/// O que não entra na cópia (nome do arquivo ou da pasta).
const FORA: &[&str] = &["painel.chave", "em-uso.trava", "navegador", "registros"];
/// Maior arquivo copiado (os maiores são a cadeia e os registros por unidade).
const ARQUIVO_MAX: u64 = 4 * 1024 * 1024 * 1024;

/// Uma linha do manifesto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// `config/...` ou `dados/...`, com `/`.
    pub caminho: String,
    /// Bytes.
    pub tamanho: u64,
    /// SHA-512, em hex.
    pub hash: String,
}

fn listar(raiz: &Path, prefixo: &str, saida: &mut Vec<(PathBuf, String)>) -> Result<(), String> {
    let Ok(entradas) = std::fs::read_dir(raiz) else { return Ok(()) };
    for e in entradas.flatten() {
        let nome = e.file_name().to_string_lossy().into_owned();
        if FORA.contains(&nome.as_str()) || nome.contains(".antes-da-restauracao-") || nome.ends_with(".tmp") {
            continue;
        }
        let caminho = e.path();
        let rel = format!("{prefixo}/{nome}");
        let tipo = e.file_type().map_err(|x| x.to_string())?;
        if tipo.is_dir() {
            listar(&caminho, &rel, saida)?;
        } else if tipo.is_file() {
            saida.push((caminho, rel));
        }
    }
    Ok(())
}

fn hash_do_arquivo(caminho: &Path) -> Result<(u64, String), String> {
    let bytes = std::fs::read(caminho).map_err(|e| format!("não consegui ler {}: {e}", caminho.display()))?;
    Ok((u64::try_from(bytes.len()).unwrap_or(u64::MAX), hex(&sha512(&bytes))))
}

/// Cria a cópia em `destino`. Devolve a pasta criada e os itens.
///
/// # Errors
/// Disco, ou arquivo grande demais.
pub fn criar(pastas: &Pastas, destino: &Path, agora_unix: u64) -> Result<(PathBuf, Vec<Item>), String> {
    let raiz = destino.join(format!("hyurax-copia-{agora_unix}"));
    if raiz.exists() {
        return Err(format!("{} já existe", raiz.display()));
    }
    let mut arquivos = Vec::new();
    listar(&pastas.config, "config", &mut arquivos)?;
    // pastas iguais (terminal com --pasta): não copia duas vezes
    if pastas.dados != pastas.config {
        listar(&pastas.dados, "dados", &mut arquivos)?;
    }
    let mut itens = Vec::new();
    for (origem, rel) in arquivos {
        let tamanho = std::fs::metadata(&origem).map(|m| m.len()).unwrap_or(0);
        if tamanho > ARQUIVO_MAX {
            return Err(format!("{} passa de 4 GiB", origem.display()));
        }
        let alvo = raiz.join(&rel);
        if let Some(p) = alvo.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("não consegui criar {}: {e}", p.display()))?;
        }
        std::fs::copy(&origem, &alvo).map_err(|e| format!("não consegui copiar {}: {e}", origem.display()))?;
        // o hash é da cópia: se o arquivo mudou durante a cópia, vale o que foi copiado
        let (tamanho, hash) = hash_do_arquivo(&alvo)?;
        itens.push(Item { caminho: rel, tamanho, hash });
    }
    let mut texto = format!("# Hyurax {}: cópia de segurança. caminho tamanho sha512\n", crate::VERSAO);
    for i in &itens {
        texto.push_str(&format!("{} {} {}\n", i.caminho, i.tamanho, i.hash));
    }
    crate::arquivos::gravar_privado(&raiz.join(MANIFESTO), &texto)?;
    Ok((raiz, itens))
}

/// Lê o manifesto de uma cópia.
///
/// # Errors
/// Manifesto ausente ou ilegível, ou caminho que sai da cópia.
pub fn ler_manifesto(copia: &Path) -> Result<Vec<Item>, String> {
    let texto = std::fs::read_to_string(copia.join(MANIFESTO)).map_err(|e| format!("sem {MANIFESTO} em {}: {e}", copia.display()))?;
    texto
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            // o caminho pode ter espaço: tamanho e hash são as duas últimas palavras
            let (resto, hash) = l.rsplit_once(' ').ok_or("linha do manifesto ilegível")?;
            let (caminho, tamanho) = resto.rsplit_once(' ').ok_or("linha do manifesto ilegível")?;
            let seguro = (caminho.starts_with("config/") || caminho.starts_with("dados/"))
                && !caminho.split('/').any(|p| p == ".." || p.is_empty() || p.contains(':') || p.contains('\\'));
            if !seguro {
                return Err(format!("caminho recusado no manifesto: {caminho}"));
            }
            Ok(Item { caminho: caminho.to_string(), tamanho: tamanho.parse().map_err(|_| "tamanho ilegível")?, hash: hash.to_string() })
        })
        .collect()
}

/// Confere cada arquivo da cópia contra o manifesto. Devolve os problemas
/// (vazio: a cópia está inteira).
///
/// # Errors
/// Manifesto ausente ou ilegível.
pub fn conferir(copia: &Path) -> Result<Vec<String>, String> {
    let mut problemas = Vec::new();
    for i in ler_manifesto(copia)? {
        match hash_do_arquivo(&copia.join(&i.caminho)) {
            Ok((tamanho, hash)) if tamanho == i.tamanho && hash == i.hash => {}
            Ok(_) => problemas.push(format!("{}: conteúdo diferente do manifesto", i.caminho)),
            Err(e) => problemas.push(e),
        }
    }
    Ok(problemas)
}

/// Restaura a cópia nas pastas. Só com o programa fechado; confere antes.
/// Devolve quantos arquivos voltaram e quantos já existiam (guardados ao lado).
///
/// # Errors
/// Programa aberto, cópia que não confere, ou disco.
pub fn restaurar(copia: &Path, pastas: &Pastas, agora_unix: u64) -> Result<(usize, usize), String> {
    let _trava = pastas.travar()?;
    let problemas = conferir(copia)?;
    if !problemas.is_empty() {
        return Err(format!("a cópia não confere, nada foi restaurado:\n  {}", problemas.join("\n  ")));
    }
    let (mut voltaram, mut guardados) = (0usize, 0usize);
    for i in ler_manifesto(copia)? {
        let (base, rel) = match i.caminho.split_once('/') {
            Some(("config", r)) => (&pastas.config, r),
            Some(("dados", r)) => (&pastas.dados, r),
            _ => continue,
        };
        let alvo = base.join(rel);
        if let Some(p) = alvo.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("não consegui criar {}: {e}", p.display()))?;
        }
        if alvo.exists() {
            let mut antes = alvo.clone().into_os_string();
            antes.push(format!(".antes-da-restauracao-{agora_unix}"));
            std::fs::rename(&alvo, PathBuf::from(antes)).map_err(|e| format!("não consegui guardar {}: {e}", alvo.display()))?;
            guardados = guardados.saturating_add(1);
        }
        std::fs::copy(copia.join(&i.caminho), &alvo).map_err(|e| format!("não consegui restaurar {}: {e}", alvo.display()))?;
        voltaram = voltaram.saturating_add(1);
    }
    Ok((voltaram, guardados))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn copia_confere_restaura_e_pega_adulteracao() {
        let base = std::env::temp_dir().join(format!("hyurax-copia-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let pastas = Pastas { config: base.join("cfg"), dados: base.join("dados") };
        std::fs::create_dir_all(pastas.config.join("sub")).unwrap();
        std::fs::create_dir_all(pastas.dados.join("ciencia")).unwrap();
        std::fs::write(pastas.config.join("carteira.txt"), "cifrada").unwrap();
        std::fs::write(pastas.config.join("painel.chave"), "sessao").unwrap();
        std::fs::write(pastas.dados.join("ciencia").join("reputacao.txt"), "rep").unwrap();
        std::fs::create_dir_all(pastas.dados.join("navegador")).unwrap();
        std::fs::write(pastas.dados.join("navegador").join("cache"), "x").unwrap();

        let (copia, itens) = criar(&pastas, &base.join("destino"), 7).unwrap();
        let nomes: Vec<&str> = itens.iter().map(|i| i.caminho.as_str()).collect();
        assert!(nomes.contains(&"config/carteira.txt") && nomes.contains(&"dados/ciencia/reputacao.txt"));
        assert!(!nomes.iter().any(|n| n.contains("painel.chave") || n.contains("navegador")), "{nomes:?}");
        assert!(conferir(&copia).unwrap().is_empty());

        // perdeu a carteira: a restauração devolve, e o que existia fica ao lado
        std::fs::remove_file(pastas.config.join("carteira.txt")).unwrap();
        std::fs::write(pastas.dados.join("ciencia").join("reputacao.txt"), "nova").unwrap();
        assert_eq!(restaurar(&copia, &pastas, 9).unwrap(), (2, 1));
        assert_eq!(std::fs::read_to_string(pastas.config.join("carteira.txt")).unwrap(), "cifrada");
        assert!(pastas.dados.join("ciencia").join("reputacao.txt.antes-da-restauracao-9").exists());

        // cópia adulterada: nada é restaurado
        std::fs::write(copia.join("config").join("carteira.txt"), "outra").unwrap();
        assert_eq!(conferir(&copia).unwrap().len(), 1);
        assert!(restaurar(&copia, &pastas, 10).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn manifesto_nao_escapa_da_copia() {
        let base = std::env::temp_dir().join(format!("hyurax-copia-mal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        for ruim in ["config/../../fora 1 aa", "outra/x 1 aa", "config/C:\\x 1 aa", "dados//x 1 aa"] {
            std::fs::write(base.join(MANIFESTO), format!("{ruim}\n")).unwrap();
            assert!(ler_manifesto(&base).is_err(), "{ruim}");
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
