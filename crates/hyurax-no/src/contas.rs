//! `hyurax-no contas criar|listar|revogar|limite`: as contas da API externa
//! (ver `hyurax_nucleo::contas`).

use hyurax_nucleo::contas::Contas;
use hyurax_nucleo::util::agora_unix;

use crate::opcoes;

/// `hyurax-no contas ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no contas criar|listar|revogar|limite (veja hyurax-no ajuda)")?;
    // os campos daqui; o resto (--pasta, --rede) vai para as opções comuns
    let (mut nome, mut creditos, mut id) = (None::<String>, None::<u64>, None::<u32>);
    let mut comuns = Vec::new();
    let mut it = resto.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--nome" => nome = it.next().cloned(),
            "--creditos" => creditos = Some(it.next().and_then(|v| v.parse().ok()).ok_or("--creditos precisa ser número (milicréditos)")?),
            "--id" => id = Some(it.next().and_then(|v| v.parse().ok()).ok_or("--id precisa ser número")?),
            _ => comuns.push(arg.clone()),
        }
    }
    let o = opcoes::ler(&comuns)?;
    let contas = Contas::abrir(Some(o.no.pastas.config.clone()));
    match sub.as_str() {
        "criar" => {
            let (c, chave) = contas.criar(&nome.ok_or("falta --nome")?, creditos.unwrap_or(0), agora_unix())?;
            println!("conta {} ({}) com limite de {} milicréditos", c.id, c.nome, c.limite_milicreditos);
            println!("chave de acesso (aparece só agora): {chave}");
            println!("use: Authorization: Bearer {chave}");
            Ok(())
        }
        "listar" => {
            for c in contas.listar() {
                println!(
                    "{:>3}  {:<30} limite {:>10} milicréditos  {} JOB(s){}",
                    c.id,
                    c.nome,
                    c.limite_milicreditos,
                    contas.jobs_da_conta(c.id).len(),
                    if c.revogada { "  REVOGADA" } else { "" }
                );
            }
            Ok(())
        }
        "revogar" => {
            contas.revogar(id.ok_or("falta --id")?)?;
            println!("revogada: a chave para de valer na hora, inclusive no programa aberto");
            Ok(())
        }
        "limite" => {
            contas.mudar_limite(id.ok_or("falta --id")?, creditos.ok_or("falta --creditos")?)?;
            println!("limite trocado");
            Ok(())
        }
        outro => Err(format!("contas {outro}: use criar, listar, revogar ou limite")),
    }
}
