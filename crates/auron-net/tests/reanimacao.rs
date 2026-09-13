// ✝ Ezequiel 37:5 — “Eis que farei entrar em vós o espírito, e vivereis.”
//! Auto-reanimação (seção 22): enquanto um único nó guardar a cadeia, a rede
//! inteira volta dele.
//!
//! O roteiro é o pedido do autor: a rede "cai por completo", sobra um nó, e a
//! partir dele tudo se reconstrói — inclusive nós que nasceram depois de a
//! rede ter morrido.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use auron_chain::Chain;
use auron_consensus::ParametrosRede;
use auron_net::{No, Rede};

const MINERADOR: [u8; 20] = [7u8; 20];

fn regtest_com(n: u64) -> Chain {
    let p = ParametrosRede::REGTEST;
    let mut chain = Chain::nova(p).unwrap();
    for _ in 0..n {
        let ts = chain.tip().unwrap().header.timestamp + p.target_spacing;
        let bloco = chain.mine(MINERADOR, vec![], Some(ts), 2).unwrap();
        chain.accept_block(bloco, Some(ts + 10)).unwrap();
    }
    chain
}

fn esperar(prazo: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let ate = Instant::now() + prazo;
    while Instant::now() < ate {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

fn altura(rede: &Arc<Rede>) -> u64 {
    rede.no.lock().unwrap().chain.height()
}
fn ponta(rede: &Arc<Rede>) -> [u8; 64] {
    rede.no.lock().unwrap().chain.tip_hash()
}
fn zerado() -> Arc<Rede> {
    Rede::nova(No::novo(regtest_com(0)))
}

#[test]
#[ignore = "integração de rede: sobe sockets; rode isolado com: cargo test -p auron-net -- --ignored --test-threads=1"]
fn um_no_sobrevivente_ressemeia_a_rede() {
    // Um nó guarda uma cadeia de 5 blocos: o único que sobreviveu à queda.
    let sobrevivente = Rede::nova(No::novo(regtest_com(5)));
    let porta_s = sobrevivente.escutar("127.0.0.1:0").unwrap();
    let alvo = ponta(&sobrevivente);

    // Dois nós recém-nascidos, sem nada além da gênese, conectam a ele.
    let n1 = zerado();
    let n2 = zerado();
    n1.conectar(("127.0.0.1", porta_s)).unwrap();
    n2.conectar(("127.0.0.1", porta_s)).unwrap();

    assert!(esperar(Duration::from_secs(60), || altura(&n1) == 5 && altura(&n2) == 5),
        "os novos não reconstruíram: n1={} n2={}", altura(&n1), altura(&n2));
    assert_eq!(ponta(&n1), alvo);
    assert_eq!(ponta(&n2), alvo);

    // Agora a "rede volta": n1 e n2 já são cópias vivas. Eles passam a escutar,
    // e o sobrevivente original morre.
    let porta_1 = n1.escutar("127.0.0.1:0").unwrap();
    sobrevivente.desligar();
    std::thread::sleep(Duration::from_secs(1)); // deixa o original cair

    // Um nó novo, que nasce DEPOIS de o original ter morrido, reconstrói a
    // cadeia inteira só a partir de n1. É a reanimação: a cadeia vive em
    // qualquer cópia e ressemeia as próximas.
    let tarde = zerado();
    tarde.conectar(("127.0.0.1", porta_1)).unwrap();
    assert!(esperar(Duration::from_secs(60), || altura(&tarde) == 5),
        "o nó tardio não reconstruiu a partir de um sobrevivente: {}", altura(&tarde));
    assert_eq!(ponta(&tarde), alvo, "reconstruiu uma cadeia diferente");

    n1.desligar();
    n2.desligar();
    tarde.desligar();
}
