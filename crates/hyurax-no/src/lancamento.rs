//! `hyurax-no lancamento chave|assinar|conferir`: para quem publica uma
//! versão. A atualização segura do programa só instala o que a chave de
//! lançamento assinou (ver `hyurax_nucleo::atualizacao`).

use std::path::PathBuf;

use hyurax_nucleo::atualizacao::{self, Manifesto};
use hyurax_nucleo::util::{de_hex, hex};

/// `hyurax-no lancamento ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no lancamento chave|assinar|conferir (veja hyurax-no ajuda)")?;
    let mut campos: Vec<(String, String)> = Vec::new();
    let mut it = resto.iter();
    while let Some(nome) = it.next() {
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        campos.push((nome.clone(), valor.clone()));
    }
    let campo = |n: &str| campos.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    let exige = |n: &str| campo(n).ok_or_else(|| format!("falta {n}"));
    match sub.as_str() {
        "chave" => {
            // a chave nasce fora do repositório e nunca é sobrescrita
            let saida = PathBuf::from(exige("--saida")?);
            if saida.exists() {
                return Err(format!("{} já existe: uma chave de lançamento não se sobrescreve", saida.display()));
            }
            if let Some(pasta) = saida.parent() {
                std::fs::create_dir_all(pasta).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
            }
            let segredo = hyurax_net::entropia::entropia_do_sistema()?;
            let texto = format!(
                "# Segredo de lançamento do Hyurax / Ultrax. Quem tem este arquivo assina\n\
                 # atualizações que todo programa instalado aceita. Guarde uma cópia fora\n\
                 # deste computador e NUNCA ponha no repositório.\n{}\n",
                hex(&segredo)
            );
            hyurax_nucleo::arquivos::gravar_privado(&saida, &texto)?;
            println!("Segredo gravado em {}", saida.display());
            println!("Chave pública (vai para rede/chave-de-lancamento.pub):");
            println!("{}", hex(&atualizacao::publica_de(&segredo)));
            Ok(())
        }
        "assinar" => {
            let texto = std::fs::read_to_string(exige("--chave")?).map_err(|e| format!("não consegui ler a chave: {e}"))?;
            let linha = texto.lines().find(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()).unwrap_or("");
            let segredo = de_hex::<32>(linha.trim()).ok_or("arquivo de chave ilegível")?;
            if atualizacao::chave_do_projeto().is_some_and(|c| c != atualizacao::publica_de(&segredo)) {
                return Err("esta chave não é a embutida no programa: as instalações recusariam o manifesto".into());
            }
            let versao = exige("--versao")?;
            let instalador = PathBuf::from(exige("--instalador")?);
            let nome = instalador.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("instalador sem nome")?;
            let endereco = campo("--endereco").unwrap_or_else(|| format!("{}v{versao}/{nome}", atualizacao::PREFIXO_DO_INSTALADOR));
            let m = atualizacao::manifesto_de(&instalador, &versao, &endereco, &campo("--notas").unwrap_or_default())?;
            let saida = PathBuf::from(campo("--saida").unwrap_or_else(|| "atualizacao.txt".into()));
            let assinado = m.assinar(&segredo);
            // confere o que acabou de assinar, como uma instalação conferiria
            Manifesto::conferir(&assinado, &atualizacao::publica_de(&segredo))?;
            std::fs::write(&saida, assinado).map_err(|e| format!("não consegui gravar {}: {e}", saida.display()))?;
            println!("Manifesto assinado: {} (versão {versao}, {} bytes, endereço {endereco})", saida.display(), m.tamanho);
            println!("Publique-o na Release v{versao}, ao lado do instalador.");
            Ok(())
        }
        "conferir" => {
            let chave = atualizacao::chave_do_projeto().ok_or("este programa foi montado sem chave de lançamento")?;
            let arquivo = exige("--arquivo")?;
            let texto = std::fs::read_to_string(&arquivo).map_err(|e| format!("não consegui ler {arquivo}: {e}"))?;
            let m = Manifesto::conferir(&texto, &chave)?;
            println!("Manifesto confere: versão {}, {} ({} bytes)", m.versao, m.instalador, m.tamanho);
            if let Some(i) = campo("--instalador") {
                atualizacao::conferir_arquivo(&m, &PathBuf::from(i))?;
                println!("O instalador confere com o manifesto (tamanho e SHA-512).");
            }
            Ok(())
        }
        outro => Err(format!("subcomando desconhecido: {outro} (chave, assinar ou conferir)")),
    }
}
