//! `hyurax-no copia criar|conferir|restaurar`: cópia de segurança das pastas
//! do usuário (ver `hyurax_nucleo::copia`).

use std::path::PathBuf;

use hyurax_nucleo::util::agora_unix;

use crate::opcoes;

/// `hyurax-no copia ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no copia criar --destino PASTA | conferir --origem COPIA | restaurar --origem COPIA")?;
    let (mut destino, mut origem) = (None::<PathBuf>, None::<PathBuf>);
    let mut comuns = Vec::new();
    let mut it = resto.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--destino" => destino = it.next().map(PathBuf::from),
            "--origem" => origem = it.next().map(PathBuf::from),
            _ => comuns.push(arg.clone()),
        }
    }
    let o = opcoes::ler(&comuns)?;
    match sub.as_str() {
        "criar" => {
            let (pasta, itens) = hyurax_nucleo::copia::criar(&o.no.pastas, &destino.ok_or("falta --destino")?, agora_unix())?;
            let bytes: u64 = itens.iter().map(|i| i.tamanho).sum();
            println!("cópia em {} ({} arquivos, {:.1} MiB)", pasta.display(), itens.len(), bytes as f64 / 1_048_576.0);
            println!("ela leva a carteira (cifrada), o segundo fator e a identidade do nó: guarde como guardaria a carteira");
            Ok(())
        }
        "conferir" => {
            let problemas = hyurax_nucleo::copia::conferir(&origem.ok_or("falta --origem")?)?;
            if problemas.is_empty() {
                println!("a cópia confere: todos os arquivos com o tamanho e o SHA-512 do manifesto");
                Ok(())
            } else {
                Err(format!("a cópia NÃO confere:\n  {}", problemas.join("\n  ")))
            }
        }
        "restaurar" => {
            let (voltaram, guardados) = hyurax_nucleo::copia::restaurar(&origem.ok_or("falta --origem")?, &o.no.pastas, agora_unix())?;
            println!("{voltaram} arquivo(s) restaurado(s); {guardados} que já existiam ficaram ao lado (.antes-da-restauracao-*)");
            Ok(())
        }
        outro => Err(format!("copia {outro}: use criar, conferir ou restaurar")),
    }
}
