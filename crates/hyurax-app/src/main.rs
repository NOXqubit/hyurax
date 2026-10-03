//! Hyurax / Ultrax 1.0: o programa com janela (Windows).
//!
//! O código mora em `janela.rs`, que só compila no Windows (WebView2, registro,
//! atalhos). Fora do Windows, use `hyurax-no painel`: o mesmo núcleo, com a interface no navegador.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod janela;

#[cfg(windows)]
fn main() {
    janela::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Fora do Windows, use hyurax-no painel: o mesmo núcleo, com a interface no navegador.");
    std::process::exit(1);
}
