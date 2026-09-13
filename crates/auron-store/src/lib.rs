// ✝ Apocalipse 21:4 — “E Deus limpará de seus olhos toda lágrima.”
//! Auron — persistência da cadeia.
//!
//! Tradução de `reference/auron/store.py`, conferida contra `vectors/chain.json`:
//! o arquivo gravado pelo Python é lido pelo Rust, o Rust grava os mesmos
//! bytes, e cada adulteração é recusada com a mesma mensagem.
//!
//! ```text
//! "AURONDB1" || string(nome da rede) || u64 quantidade || var_bytes(bloco)...
//! ```
//!
//! A gênese não é gravada: é derivada dos parâmetros da rede. Ao carregar, a
//! cadeia é **reconstruída** passando pela validação de cada bloco. Arquivo
//! adulterado no disco não vira estado válido.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::{Path, PathBuf};

use auron_block::Block;
use auron_chain::Chain;
use auron_codec::{Reader, Writer};
use auron_consensus::ParametrosRede;

/// Início de todo arquivo de cadeia.
pub const STORE_MAGIC: &[u8; 8] = b"AURONDB1";

/// Folga de horário ao recarregar: a regra de "timestamp no futuro" não faz
/// sentido para bloco antigo, e todas as outras continuam valendo.
const FOLGA_DE_RECARGA: u64 = 1_000_000_000;

/// Arquivo de cadeia corrompido ou incompatível. Texto igual ao do gabarito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(pub String);

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

fn erro(m: impl Into<String>) -> StoreError {
    StoreError(m.into())
}

/// Os bytes do arquivo de uma cadeia.
pub fn encode_chain(chain: &Chain) -> Result<Vec<u8>, StoreError> {
    let mut w = Writer::new();
    w.fixed(STORE_MAGIC);
    w.string(chain.params.nome).map_err(|e| erro(e.to_string()))?;
    let blocos = chain.entries.get(1..).unwrap_or(&[]);
    w.u64(blocos.len() as u64);
    for entrada in blocos {
        let bruto = entrada.block.encode().map_err(|e| erro(e.to_string()))?;
        w.var_bytes(&bruto).map_err(|e| erro(e.to_string()))?;
    }
    Ok(w.into_bytes())
}

/// Grava a cadeia com troca atômica: nunca deixa arquivo meio escrito.
/// Devolve quantos blocos foram escritos.
pub fn save_chain(chain: &Chain, caminho: &Path) -> Result<u64, StoreError> {
    let dados = encode_chain(chain)?;
    let mut temporario = PathBuf::from(caminho);
    temporario.as_mut_os_string().push(".tmp");
    std::fs::write(&temporario, &dados).map_err(|e| erro(format!("não consegui gravar: {e}")))?;
    std::fs::rename(&temporario, caminho).map_err(|e| erro(format!("não consegui gravar: {e}")))?;
    Ok(chain.entries.len().saturating_sub(1) as u64)
}

/// Reconstrói a cadeia a partir dos bytes, revalidando cada bloco.
///
/// `confiar_no_argon2` pula só o Argon2id, para recarga rápida do próprio disco.
pub fn decode_chain(
    dados: &[u8],
    confiar_no_argon2: bool,
    params: Option<ParametrosRede>,
) -> Result<Chain, StoreError> {
    let Some(resto) = dados.strip_prefix(STORE_MAGIC.as_slice()) else {
        return Err(erro("arquivo não é uma cadeia Auron"));
    };
    let mut r = Reader::new(resto);
    let (rede, quantidade) = r
        .string()
        .and_then(|rede| Ok((rede, r.u64()?)))
        .map_err(|e| erro(format!("cabeçalho corrompido: {e}")))?;

    let params = match params {
        None => ParametrosRede::da_rede(rede)
            .filter(|p| p.nome == rede)
            .ok_or_else(|| erro(format!("rede desconhecida no arquivo: {rede}")))?,
        Some(p) if p.nome != rede => {
            return Err(erro(format!("arquivo é da rede {rede}, mas foi pedida {}", p.nome)));
        }
        Some(p) => p,
    };

    let mut cadeia = Chain::nova(params).map_err(|e| erro(e.to_string()))?;
    for indice in 1..=quantidade {
        let bloco = r
            .var_bytes()
            .map_err(|e| e.to_string())
            .and_then(|bruto| Block::decode(bruto).map_err(|e| e.to_string()))
            .map_err(|e| erro(format!("bloco {indice} corrompido: {e}")))?;
        let agora = Some(bloco.header.timestamp.saturating_add(FOLGA_DE_RECARGA));
        let aceito = if confiar_no_argon2 {
            cadeia.accept_block_do_proprio_disco(bloco, agora)
        } else {
            cadeia.accept_block(bloco, agora)
        };
        aceito.map_err(|e| erro(format!("bloco {indice} rejeitado ao recarregar: {e}")))?;
    }
    r.finish().map_err(|e| erro(format!("sobra de dados no fim do arquivo: {e}")))?;
    Ok(cadeia)
}

/// Lê e reconstrói a cadeia de um arquivo.
pub fn load_chain(
    caminho: &Path,
    confiar_no_argon2: bool,
    params: Option<ParametrosRede>,
) -> Result<Chain, StoreError> {
    if !caminho.exists() {
        return Err(erro(format!("arquivo não encontrado: {}", caminho.display())));
    }
    let dados = std::fs::read(caminho).map_err(|e| erro(format!("não consegui ler: {e}")))?;
    decode_chain(&dados, confiar_no_argon2, params)
}
