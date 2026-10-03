//! Hyurax / Ultrax 1.0: o instalador (Windows).
//!
//! O código mora em `janela.rs`, que só compila no Windows (WebView2, registro,
//! atalhos). O instalador é só para Windows. No Linux e no macOS, use os pacotes do `hyurax-no`.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod janela;

#[cfg(windows)]
fn main() {
    janela::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("O instalador é só para Windows. No Linux e no macOS, use os pacotes do hyurax-no.");
    std::process::exit(1);
}
