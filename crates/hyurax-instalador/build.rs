//! Prepara a carga do instalador: o `Hyurax.exe` e a `WebView2Loader.dll` que
//! ele leva dentro.
//!
//! Os caminhos vêm de `HYURAX_CARGA_EXE` e `HYURAX_CARGA_DLL` (o roteiro de
//! empacotamento, `scripts/empacotar-windows.ps1`, e o GitHub Actions passam
//! os dois). Sem eles, a carga fica vazia: o workspace inteiro continua
//! compilando e testando, e o instalador assim montado se recusa a instalar.

use std::path::PathBuf;

fn main() {
    let saida = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default());
    for (var, nome) in [("HYURAX_CARGA_EXE", "Hyurax.exe"), ("HYURAX_CARGA_DLL", "WebView2Loader.dll")] {
        println!("cargo:rerun-if-env-changed={var}");
        let destino = saida.join(nome);
        match std::env::var_os(var) {
            Some(origem) => {
                println!("cargo:rerun-if-changed={}", PathBuf::from(&origem).display());
                if let Err(e) = std::fs::copy(&origem, &destino) {
                    println!("cargo:warning=não consegui copiar {var} ({}): {e}", PathBuf::from(&origem).display());
                    let _ = std::fs::write(&destino, b"");
                }
            }
            None => {
                let _ = std::fs::write(&destino, b"");
            }
        }
    }
}
