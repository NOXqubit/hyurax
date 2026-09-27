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
