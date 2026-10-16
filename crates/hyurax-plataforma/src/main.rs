// ✝ Neemias 4:6 — “O povo tinha ânimo para trabalhar.”
//! `hyurax-plataforma`: o servidor do HYURAX Compute (V1).
//!
//! ```text
//! hyurax-plataforma servir   --pasta DADOS [--porta 8790] [--endereco 0.0.0.0]
//!                            [--publico https://compute.exemplo] [--https] [--proxy 1] [--teste]
//! hyurax-plataforma admin    --pasta DADOS --email EMAIL [--nome NOME]
//! hyurax-plataforma conferir --pasta DADOS
//! hyurax-plataforma copia    --pasta DADOS --destino ARQUIVO
//! ```
//!
//! No contêiner, as mesmas opções vêm do ambiente: `HYX_PASTA`, `PORT`,
//! `HYX_PUBLICO`, `HYX_HTTPS=1`, `HYX_PROXY`, `HYX_TESTE=1` e, só na primeira
//! vez, `HYX_ADMIN_EMAIL` (cria o administrador e mostra a senha uma vez no
//! log; troque a senha logo depois de entrar).

use std::path::PathBuf;
use std::process::ExitCode;

use hyurax_plataforma::http::{Escuta, servir};
use hyurax_plataforma::{Opcoes, Plataforma, VERSAO};

fn opcao(args: &[String], nome: &str) -> Option<String> {
    args.iter().position(|a| a == nome).and_then(|i| args.get(i.saturating_add(1))).cloned()
}

fn bandeira(args: &[String], nome: &str) -> bool {
    args.iter().any(|a| a == nome)
}

fn ambiente(nome: &str) -> Option<String> {
    std::env::var(nome).ok().filter(|v| !v.trim().is_empty())
}

fn pasta(args: &[String]) -> Result<PathBuf, String> {
    opcao(args, "--pasta").or_else(|| ambiente("HYX_PASTA")).map(PathBuf::from).ok_or_else(|| "falta --pasta (ou HYX_PASTA)".into())
}

fn servir_cmd(args: &[String]) -> Result<(), String> {
    let pasta = pasta(args)?;
    let porta: u16 = opcao(args, "--porta").or_else(|| ambiente("PORT")).unwrap_or_else(|| "8790".into()).parse().map_err(|_| "porta inválida")?;
    let endereco = opcao(args, "--endereco").unwrap_or_else(|| "0.0.0.0".into());
    let modo_teste = bandeira(args, "--teste") || ambiente("HYX_TESTE").is_some_and(|v| v == "1");
    let https = bandeira(args, "--https") || ambiente("HYX_HTTPS").is_some_and(|v| v == "1");
    let saltos: usize = opcao(args, "--proxy").or_else(|| ambiente("HYX_PROXY")).unwrap_or_else(|| "0".into()).parse().map_err(|_| "--proxy: número de saltos")?;
    let publico = opcao(args, "--publico").or_else(|| ambiente("HYX_PUBLICO")).unwrap_or_else(|| format!("http://127.0.0.1:{porta}"));
    let a = Plataforma::abrir(&pasta, Opcoes { modo_teste, https, endereco_publico: publico.clone() })?;
    let p = a.plataforma;
    println!("HYURAX Compute {VERSAO}: {} eventos relidos de {}", a.eventos, pasta.display());
    if a.cauda_cortada {
        println!("aviso: a última linha do diário estava incompleta (queda no meio da escrita) e foi cortada");
    }
    if modo_teste {
        println!("AMBIENTE DE TESTE: pagamentos de teste confirmam na hora; nada aqui é dinheiro de verdade");
    }
    if let Some(email) = ambiente("HYX_ADMIN_EMAIL") {
        let tem_admin = p.ler(|e| e.usuarios.values().any(|u| u.papeis.admin)).map_err(|e| e.mensagem)?;
        if !tem_admin {
            let senha = p.criar_admin(&email, "Administrador").map_err(|e| e.mensagem)?;
            println!("administrador criado: {email}");
            println!("senha provisória (aparece só agora; troque depois de entrar): {senha}");
        }
    }
    p.iniciar_manutencao();
    println!("servindo em {endereco}:{porta} (público: {publico}, proxy: {saltos} salto(s))");
    servir(p, &Escuta { endereco: format!("{endereco}:{porta}"), saltos_de_proxy: saltos }).map_err(|e| format!("não escutei: {e}"))
}

fn admin_cmd(args: &[String]) -> Result<(), String> {
    let pasta = pasta(args)?;
    let email = opcao(args, "--email").ok_or("falta --email")?;
    let nome = opcao(args, "--nome").unwrap_or_else(|| "Administrador".into());
    let p = Plataforma::abrir(&pasta, Opcoes::default())?.plataforma;
    let senha = p.criar_admin(&email, &nome).map_err(|e| e.mensagem)?;
    println!("administrador: {email}");
    println!("senha (aparece só agora; troque depois de entrar): {senha}");
    Ok(())
}

fn conferir_cmd(args: &[String]) -> Result<(), String> {
    let pasta = pasta(args)?;
    let leitura = hyurax_plataforma::diario::ler(&pasta.join("diario.hyx"))?;
    let mut estado = hyurax_plataforma::estado::Estado::novo();
    for r in &leitura.registros {
        estado.aplicar(&r.ev).map_err(|e| format!("evento {}: {e}", r.n))?;
    }
    println!("diário confere: {} eventos, corrente de hash íntegra", leitura.registros.len());
    if leitura.cauda_cortada {
        println!("aviso: a última linha está incompleta (seria cortada ao abrir)");
    }
    for m in [hyurax_plataforma::dinheiro::Moeda::Brl, hyurax_plataforma::dinheiro::Moeda::Usd] {
        println!("livro em {}: soma {} (tem de ser zero)", m.codigo(), estado.livro.soma(m));
    }
    println!("usuários {}, nós {}, tarefas {}, pagamentos {}", estado.usuarios.len(), estado.nos.len(), estado.tarefas.len(), estado.pagamentos.len());
    println!("selo (guarde fora do servidor): {}", hyurax_plataforma::util::hex(&leitura.ultimo));
    Ok(())
}

fn copia_cmd(args: &[String]) -> Result<(), String> {
    let pasta = pasta(args)?;
    let destino = PathBuf::from(opcao(args, "--destino").ok_or("falta --destino")?);
    // lê sem travar: o servidor pode estar rodando; a cópia confere sozinha
    let bytes = std::fs::read(pasta.join("diario.hyx")).map_err(|e| format!("não li o diário: {e}"))?;
    std::fs::write(&destino, &bytes).map_err(|e| format!("não gravei {}: {e}", destino.display()))?;
    let l = hyurax_plataforma::diario::ler(&destino)?;
    println!("cópia em {}: {} eventos conferidos{}", destino.display(), l.registros.len(), if l.cauda_cortada { " (a última linha estava sendo escrita e ficou de fora)" } else { "" });
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let resto: Vec<String> = args.iter().skip(2).cloned().collect();
    let r = match args.get(1).map(String::as_str) {
        Some("servir") | None if ambiente("HYX_PASTA").is_some() || args.get(1).is_some() => servir_cmd(&resto),
        Some("admin") => admin_cmd(&resto),
        Some("conferir") => conferir_cmd(&resto),
        Some("copia") => copia_cmd(&resto),
        Some("versao" | "--version") => {
            println!("hyurax-plataforma {VERSAO}");
            Ok(())
        }
        _ => {
            println!("hyurax-plataforma {VERSAO} — HYURAX Compute (V1)\n");
            println!("  servir   --pasta DADOS [--porta 8790] [--publico URL] [--https] [--proxy N] [--teste]");
            println!("  admin    --pasta DADOS --email EMAIL [--nome NOME]");
            println!("  conferir --pasta DADOS");
            println!("  copia    --pasta DADOS --destino ARQUIVO");
            Ok(())
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::FAILURE
        }
    }
}
