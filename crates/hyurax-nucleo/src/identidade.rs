//! A identidade do nó na rede (chave da cifra Noise entre nós) e o
//! `WORKER_ID` do ULTRAX, que deriva dela.

use std::path::Path;

use hyurax_crypto::PUBKEY_LEN;
use hyurax_net::Identidade;
use hyurax_net::entropia::entropia_do_sistema;

use crate::arquivos;
use crate::util::{de_hex, hex};

/// Nome do arquivo, na pasta de configuração.
pub const ARQUIVO: &str = "no.chave";

/// A identidade guardada em `pasta/no.chave`, criada na primeira vez. Apagar
/// o arquivo dá uma identidade nova (não mexe em saldo: não é carteira).
///
/// # Errors
/// Arquivo corrompido, ou sem entropia do sistema.
pub fn na_pasta(pasta: &Path) -> Result<Identidade, String> {
    let arquivo = pasta.join(ARQUIVO);
    if arquivo.exists() {
        let texto = arquivos::ler(&arquivo)?;
        let segredo: [u8; 32] = texto
            .lines()
            .find_map(|l| l.trim().strip_prefix("segredo="))
            .and_then(de_hex)
            .ok_or_else(|| format!("{} inválido; apague para gerar outro", arquivo.display()))?;
        return Identidade::de_segredo(segredo).map_err(|e| e.to_string());
    }
    let identidade = Identidade::de_segredo(entropia_do_sistema()?).map_err(|e| e.to_string())?;
    let conteudo = format!(
        "# Identidade deste nó na rede Hyurax (chave da cifra entre nós).\n\
         # Não é carteira e não guarda saldo. Apagar gera uma identidade nova.\n\
         segredo={}\npublica={}\n",
        hex(identidade.segredo()),
        hex(&identidade.publica())
    );
    std::fs::create_dir_all(pasta).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
    arquivos::gravar_privado(&arquivo, &conteudo)?;
    Ok(identidade)
}

/// O `WORKER_ID` do ULTRAX: a chave pública da chave de assinatura derivada do
/// segredo do nó.
pub fn worker(identidade: &Identidade) -> [u8; PUBKEY_LEN] {
    hyurax_crypto::ed25519_public_key(&hyurax_ultrax::prova::chave_do_worker(identidade.segredo()))
}

/// O que a rotação fez.
#[derive(Clone, Debug)]
pub struct Rotacao {
    /// Identidade de rede antiga (chave pública).
    pub antiga: [u8; PUBKEY_LEN],
    /// Identidade nova.
    pub nova: [u8; PUBKEY_LEN],
    /// `WORKER_ID` antigo e novo.
    pub worker_antigo: [u8; PUBKEY_LEN],
    /// `WORKER_ID` novo.
    pub worker_novo: [u8; PUBKEY_LEN],
    /// Onde a identidade antiga ficou guardada.
    pub guardada_em: std::path::PathBuf,
}

/// Troca a identidade do nó (Documento Mestre §12, rotação de credenciais).
///
/// Só com o programa fechado (pega a trava da pasta de dados). A antiga não
/// é apagada: vira `no.chave.revogada-<unix>`, só para o dono, para provar
/// depois o que aquela identidade assinou. A nova nasce na hora.
///
/// O que muda para o resto da rede: os outros nós passam a ver uma
/// identidade nova, e a reputação que tinham da antiga não passa para ela.
/// Quem tiver copiado a chave antiga continua podendo usá-la: não existe
/// lista de revogação anunciada na rede (PENDENTE).
///
/// # Errors
/// Programa aberto (pasta travada), disco, ou sem entropia.
pub fn girar(pastas: &crate::pastas::Pastas, agora_unix: u64) -> Result<Rotacao, String> {
    let _trava = pastas.travar()?;
    let antiga = na_pasta(&pastas.config)?;
    let arquivo = pastas.config.join(ARQUIVO);
    let guardada_em = pastas.config.join(format!("{ARQUIVO}.revogada-{agora_unix}"));
    if guardada_em.exists() {
        return Err(format!("{} já existe; espere um segundo e tente de novo", guardada_em.display()));
    }
    std::fs::rename(&arquivo, &guardada_em).map_err(|e| format!("não consegui guardar a identidade antiga: {e}"))?;
    let nova = match na_pasta(&pastas.config) {
        Ok(n) => n,
        Err(e) => {
            // sem identidade nova, a antiga volta para o lugar
            let _ = std::fs::rename(&guardada_em, &arquivo);
            return Err(e);
        }
    };
    Ok(Rotacao {
        antiga: antiga.publica(),
        nova: nova.publica(),
        worker_antigo: worker(&antiga),
        worker_novo: worker(&nova),
        guardada_em,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes_rotacao {
    use super::*;

    #[test]
    fn girar_troca_a_identidade_e_guarda_a_antiga() {
        let pasta = std::env::temp_dir().join(format!("hyurax-girar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let pastas = crate::pastas::Pastas::unica(&pasta);
        let antes = na_pasta(&pasta).unwrap().publica();
        let r = girar(&pastas, 1_000).unwrap();
        assert_eq!(r.antiga, antes);
        assert_ne!(r.nova, antes);
        assert_ne!(r.worker_novo, r.worker_antigo);
        assert_eq!(na_pasta(&pasta).unwrap().publica(), r.nova, "a nova fica no lugar");
        assert!(r.guardada_em.exists());
        // com o programa aberto (pasta travada), recusa
        let _aberto = pastas.travar().unwrap();
        assert!(girar(&pastas, 2_000).is_err());
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
