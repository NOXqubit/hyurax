//! `hyurax-no planos chave|emitir|conferir`: para quem vende os planos
//! (docs/MONETIZACAO.md). O voucher vai para o cliente colar na tela Planos;
//! o programa dele confere a assinatura com a chave de planos embutida.

use std::path::PathBuf;

use hyurax_nucleo::planos;
use hyurax_nucleo::util::{agora_ms, data_curta, de_hex, hex};
use hyurax_nuvem::plano::{self, CATALOGO, Voucher};

const DIA_MS: u64 = 86_400_000;

fn ler_segredo(caminho: &str) -> Result<[u8; 32], String> {
    let texto = std::fs::read_to_string(caminho).map_err(|e| format!("não consegui ler a chave: {e}"))?;
    let linha = texto.lines().find(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()).unwrap_or("");
    de_hex::<32>(linha.trim()).ok_or_else(|| "arquivo de chave ilegível".to_string())
}

fn mostrar(v: &Voucher) {
    let nome = plano::do_catalogo(v.plano).map_or("?", |p| p.nome);
    println!("  plano {nome} · série {} · de {} até {} (exclusive)", v.serie, data_curta(v.inicio_ms), data_curta(v.fim_ms));
    println!("  para o worker {}", hex(&v.beneficiario));
    println!("  {} créditos/mês · {} GiB gerenciados · comissão {}%", v.creditos_mes, v.armazenamento_gib, f64::from(v.comissao_bp) / 100.0);
}

/// `hyurax-no planos ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no planos chave|emitir|conferir|catalogo (veja hyurax-no ajuda)")?;
    let mut campos: Vec<(String, String)> = Vec::new();
    let mut it = resto.iter();
    while let Some(nome) = it.next() {
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        campos.push((nome.clone(), valor.clone()));
    }
    let campo = |n: &str| campos.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    let exige = |n: &str| campo(n).ok_or_else(|| format!("falta {n}"));
    match sub.as_str() {
        "catalogo" => {
            for p in &CATALOGO {
                println!(
                    "{:>2} {:<11} R$ {:>7.2}/mês · R$ {:>8.2}/ano · {:>10} créditos/mês · {:>5} GiB · comissão {}%",
                    p.id,
                    p.nome,
                    p.mensal_centavos as f64 / 100.0,
                    p.anual_centavos as f64 / 100.0,
                    p.creditos_mes,
                    p.armazenamento_gib,
                    f64::from(p.comissao_bp) / 100.0
                );
            }
            Ok(())
        }
        "chave" => {
            // nasce fora do repositório e nunca é sobrescrita
            let saida = PathBuf::from(exige("--saida")?);
            if saida.exists() {
                return Err(format!("{} já existe: uma chave de planos não se sobrescreve", saida.display()));
            }
            if let Some(pasta) = saida.parent() {
                std::fs::create_dir_all(pasta).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
            }
            let segredo = hyurax_net::entropia::entropia_do_sistema()?;
            let texto = format!(
                "# Segredo de PLANOS do Hyurax. Quem tem este arquivo emite vouchers de plano\n\
                 # que todo programa aceita. Guarde uma cópia fora deste computador e NUNCA\n\
                 # ponha no repositório.\n{}\n",
                hex(&segredo)
            );
            hyurax_nucleo::arquivos::gravar_privado(&saida, &texto)?;
            println!("Segredo gravado em {}", saida.display());
            println!("Chave pública (vai para rede/chave-de-planos.pub):");
            println!("{}", hex(&hyurax_crypto::ed25519_public_key(&segredo)));
            Ok(())
        }
        "emitir" => {
            let segredo = ler_segredo(&exige("--chave")?)?;
            let embutida = planos::chave_do_emissor().ok_or("este programa foi montado sem a chave de planos")?;
            if hyurax_crypto::ed25519_public_key(&segredo) != embutida {
                return Err("esta chave não é a embutida no programa: os clientes recusariam o voucher".into());
            }
            let nome = exige("--plano")?.to_lowercase();
            let p = CATALOGO
                .iter()
                .find(|p| p.nome.to_lowercase() == nome && p.id != plano::COMUNIDADE)
                .ok_or("plano desconhecido (pro, equipe ou empresa)")?;
            let para = de_hex::<32>(exige("--para")?.trim()).ok_or("--para precisa do código do worker do cliente (64 dígitos, tela Planos)")?;
            let dias: u64 = campo("--dias").map_or(Ok(30), |d| d.parse().map_err(|_| "--dias precisa ser número"))?;
            let serie: u32 = exige("--serie")?.parse().map_err(|_| "--serie precisa ser número")?;
            let inicio = agora_ms();
            let v = Voucher::emitir(&segredo, p.id, para, inicio, inicio.saturating_add(dias.saturating_mul(DIA_MS)), serie).map_err(|e| e.to_string())?;
            // confere como o programa do cliente vai conferir
            if !v.vale(&embutida, &para, inicio) {
                return Err("o voucher emitido não confere (defeito)".into());
            }
            println!("Voucher emitido:");
            mostrar(&v);
            println!();
            println!("{}", v.texto().map_err(|e| e.to_string())?);
            Ok(())
        }
        "conferir" => {
            let v = Voucher::ler_texto(&exige("--voucher")?).map_err(|e| e.to_string())?;
            mostrar(&v);
            match planos::chave_do_emissor() {
                Some(c) if v.assinatura_confere(&c) => {
                    let agora = agora_ms();
                    let estado = if agora < v.inicio_ms {
                        "ainda não começou"
                    } else if agora >= v.fim_ms {
                        "vencido"
                    } else {
                        "valendo"
                    };
                    println!("Assinatura confere com a chave de planos do projeto: {estado}.");
                    Ok(())
                }
                Some(_) => Err("a assinatura NÃO confere com a chave de planos do projeto".into()),
                None => Err("este programa foi montado sem a chave de planos".into()),
            }
        }
        outro => Err(format!("planos {outro}? use chave, emitir, conferir ou catalogo")),
    }
}
