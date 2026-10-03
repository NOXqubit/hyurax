//! `hyurax-no pacote exportar|importar`: o pacote do Éter pelo terminal
//! (ver `hyurax_net::malha::Pacote` e `docs/HYURAX-MALHA.md`).

use std::path::PathBuf;
use std::sync::Arc;

use hyurax_net::{Identidade, No, Rede};
use hyurax_nucleo::cadeia;

use crate::opcoes;

/// `hyurax-no pacote ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no pacote exportar --saida ARQ [--blocos N] | importar --arquivo ARQ")?;
    let (mut saida, mut entrada, mut blocos) = (None::<PathBuf>, None::<PathBuf>, 100usize);
    let mut comuns = Vec::new();
    let mut it = resto.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--saida" => saida = it.next().map(PathBuf::from),
            // --arquivo é a opção comum (a carteira); aqui é o pacote
            "--arquivo" => entrada = it.next().map(PathBuf::from),
            "--blocos" => blocos = it.next().and_then(|v| v.parse().ok()).ok_or("--blocos precisa ser número")?,
            _ => comuns.push(arg.clone()),
        }
    }
    let o = opcoes::ler(&comuns)?;
    // o nó do disco, sem rede: nada de socket nem porta
    let _trava = o.no.pastas.travar()?;
    let cadeia_do_disco = cadeia::abrir(o.no.rede, &cadeia::arquivo(&o.no))?;
    let rede: Arc<Rede> = Rede::com_identidade(No::novo(cadeia_do_disco), Identidade::nova().map_err(|e| e.to_string())?);
    match sub.as_str() {
        "exportar" => {
            let destino = saida.ok_or("falta --saida")?;
            let pacote = rede.exportar_pacote(blocos)?;
            std::fs::write(&destino, &pacote).map_err(|e| format!("não consegui gravar {}: {e}", destino.display()))?;
            println!("pacote com os últimos {blocos} blocos em {} ({} bytes)", destino.display(), pacote.len());
            println!("leve por qualquer meio: eter enviar {} --bluetooth COM5 (ou pendrive, som)", destino.display());
            Ok(())
        }
        "importar" => {
            let origem = entrada.ok_or("falta --arquivo (o pacote)")?;
            let dados = std::fs::read(&origem).map_err(|e| format!("não consegui ler {}: {e}", origem.display()))?;
            let r = rede.importar_pacote(&dados)?;
            cadeia::salvar(&rede, &o.no)?;
            println!(
                "{} bloco(s) e {} transação(ões) entraram; {} já conhecidos; {} recusados",
                r.blocos, r.transacoes, r.repetidos, r.recusados
            );
            if r.transacoes > 0 {
                println!("as transações ficam no mempool deste nó: abra o programa conectado para elas seguirem para a rede");
            }
            Ok(())
        }
        outro => Err(format!("pacote {outro}: use exportar ou importar")),
    }
}
