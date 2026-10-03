//! `hyurax-no identidade ver|girar`: a identidade do nó na rede e a troca
//! dela (rotação de credenciais, Documento Mestre §12).

use hyurax_nucleo::util::{agora_unix, hex};

use crate::opcoes;

/// `hyurax-no identidade ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no identidade ver|girar [--pasta P]")?;
    let o = opcoes::ler(resto)?;
    let pastas = &o.no.pastas;
    match sub.as_str() {
        "ver" => {
            let id = hyurax_nucleo::identidade::na_pasta(&pastas.config)?;
            println!("identidade de rede: {}", hex(&id.publica()));
            println!("WORKER_ID (ULTRAX): {}", hex(&hyurax_nucleo::identidade::worker(&id)));
            Ok(())
        }
        "girar" => {
            let r = hyurax_nucleo::identidade::girar(pastas, agora_unix())?;
            println!("identidade de rede: {} -> {}", hex(&r.antiga), hex(&r.nova));
            println!("WORKER_ID (ULTRAX): {} -> {}", hex(&r.worker_antigo), hex(&r.worker_novo));
            println!("a antiga ficou em {} (só para você; não apague se precisar provar algo que ela assinou)", r.guardada_em.display());
            println!("os outros nós verão uma identidade nova; a reputação da antiga não passa para ela");
            Ok(())
        }
        outro => Err(format!("identidade {outro}: use ver ou girar")),
    }
}
