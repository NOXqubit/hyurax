// ✝ Eclesiastes 4:12 — “E, se alguém quiser prevalecer contra um, os dois lhe resistirão; o cordão de três dobras não se quebra tão depressa.”
//! Auron — transporte da rede entre nós.
//!
//! Junta o formato das mensagens (`auron-wire`, seção 21) a sockets TCP de
//! verdade: aperto de mão, sincronização por cabeçalhos e depois blocos,
//! propagação de blocos e transações, e um mempool.
//!
//! O que é consenso mora nos crates de baixo (`auron-chain`, `auron-wire`).
//! Aqui é só transporte: conexões, threads, difusão. Trocar este crate por
//! outro (async, outra topologia) não muda o que a rede considera válido.
//!
//! Sem async e sem dependência de rede: `std::net` e threads.

#![forbid(unsafe_code)]

mod conexao;
mod no;
mod servidor;

pub use conexao::{Conexao, NetError};
pub use no::{Malicia, No, Reacao};
pub use servidor::Rede;
