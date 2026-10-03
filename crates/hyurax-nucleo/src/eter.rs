//! A pasta do Éter (`docs/HYURAX-MALHA.md`, etapa M5): o caminho dos pacotes
//! que atravessam pendrive, Bluetooth, som ou rádio.
//!
//! - `DADOS/eter/entrada/`: todo `.hxp` que aparecer aqui é conferido e
//!   importado (como se viesse de um par) e vai para `importados/` (ou
//!   `recusados/`). O que entrou é repassado aos pares conectados.
//! - `DADOS/eter/saida/hyurax-pacote.hxp`: o pacote atual deste nó (os
//!   últimos blocos e o mempool), refeito quando a cadeia ou o mempool mudam,
//!   pronto para `eter enviar`.

use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::time::Duration;

use crate::servico::Nucleo;

/// Blocos no pacote de saída.
pub const BLOCOS_NO_PACOTE: usize = 100;
/// Maior pacote lido da entrada.
const PACOTE_MAX: u64 = 256 * 1024 * 1024;

/// As pastas do Éter dentro dos dados.
pub fn pastas(dados: &Path) -> (PathBuf, PathBuf) {
    let base = dados.join("eter");
    (base.join("entrada"), base.join("saida"))
}

/// Vigia a pasta enquanto o núcleo viver.
pub fn vigiar(fraco: &Weak<Nucleo>) {
    let mut ultimo = (u64::MAX, usize::MAX);
    loop {
        std::thread::sleep(Duration::from_secs(15));
        let Some(n) = fraco.upgrade() else { return };
        let (entrada, saida) = pastas(&n.config.pastas.dados);
        let _ = std::fs::create_dir_all(&entrada);
        let _ = std::fs::create_dir_all(&saida);
        importar_o_que_chegou(&n, &entrada);
        let agora = n.rede.no.lock().map_or(ultimo, |no| (no.chain.height(), no.mempool_len()));
        if agora != ultimo {
            match n.rede.exportar_pacote(BLOCOS_NO_PACOTE) {
                Ok(p) => {
                    if crate::arquivos::gravar_atomico(&saida.join("hyurax-pacote.hxp"), &p).is_ok() {
                        ultimo = agora;
                    }
                }
                Err(e) => n.barramento.registrar("erro", format!("pacote do Éter: {e}")),
            }
        }
    }
}

fn importar_o_que_chegou(n: &Nucleo, entrada: &Path) {
    let Ok(lista) = std::fs::read_dir(entrada) else { return };
    for e in lista.flatten() {
        let caminho = e.path();
        if caminho.extension().is_none_or(|x| x != "hxp") || e.metadata().map_or(true, |m| m.len() > PACOTE_MAX) {
            continue;
        }
        let nome = e.file_name();
        let destino_ok = entrada.with_file_name("importados");
        let destino_ruim = entrada.with_file_name("recusados");
        let resultado = std::fs::read(&caminho).map_err(|x| x.to_string()).and_then(|d| n.rede.importar_pacote(&d));
        let destino = match resultado {
            Ok(r) => {
                n.barramento.registrar(
                    "rede",
                    format!(
                        "pacote do Éter {}: {} bloco(s) e {} transação(ões) entraram, {} já conhecidos, {} recusados",
                        nome.to_string_lossy(),
                        r.blocos,
                        r.transacoes,
                        r.repetidos,
                        r.recusados
                    ),
                );
                destino_ok
            }
            Err(x) => {
                n.barramento.registrar("erro", format!("pacote do Éter {} recusado: {x}", nome.to_string_lossy()));
                destino_ruim
            }
        };
        let _ = std::fs::create_dir_all(&destino);
        if std::fs::rename(&caminho, destino.join(&nome)).is_err() {
            // não dá para mover (outro programa segurando): não reimporta em laço
            let _ = std::fs::remove_file(&caminho);
        }
    }
}
