// ✝ João 17:21 — “Para que todos sejam um.”
//! Dois nós de verdade, conversando por TCP em loopback.
//!
//! Não é vetor byte a byte (isto é I/O, não consenso): é integração. Sobe nós,
//! deixa eles se acharem, e confere que a cadeia de um alcança a do outro.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use auron_chain::Chain;
use auron_consensus::ParametrosRede;
use auron_net::{No, Rede};

const MINERADOR: [u8; 20] = [7u8; 20];

/// Uma cadeia regtest com `n` blocos minerados por cima da gênese.
fn cadeia_com(n: u64) -> Chain {
    let p = ParametrosRede::REGTEST;
    let mut chain = Chain::nova(p).unwrap();
    for _ in 0..n {
        let ts = chain.tip().unwrap().header.timestamp + p.target_spacing;
        let bloco = chain.mine(MINERADOR, vec![], Some(ts), 2).unwrap();
        chain.accept_block(bloco, Some(ts + 10)).unwrap();
    }
    chain
}

/// Espera até `cond` virar verdade, ou estoura. Devolve se conseguiu.
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

#[test]
#[ignore = "integração de rede: sobe sockets; rode isolado com: cargo test -p auron-net -- --ignored --test-threads=1"]
fn no_atrasado_alcanca_o_no_a_frente() {
    let a = Rede::nova(No::novo(cadeia_com(4)));
    let b = Rede::nova(No::novo(cadeia_com(0)));

    let porta = a.escutar("127.0.0.1:0").unwrap();
    b.conectar(("127.0.0.1", porta)).unwrap();

    assert!(esperar(Duration::from_secs(60), || altura(&b) == 4), "B não sincronizou: {}", altura(&b));
    assert_eq!(ponta(&a), ponta(&b), "pontas diferentes depois de sincronizar");

    a.desligar();
    b.desligar();
}

#[test]
#[ignore = "integração de rede: sobe sockets; rode isolado com: cargo test -p auron-net -- --ignored --test-threads=1"]
fn bloco_novo_se_espalha_para_os_pares() {
    let a = Rede::nova(No::novo(cadeia_com(2)));
    let b = Rede::nova(No::novo(cadeia_com(2)));

    let porta = a.escutar("127.0.0.1:0").unwrap();
    b.conectar(("127.0.0.1", porta)).unwrap();
    assert!(esperar(Duration::from_secs(60), || a.pares_conectados() == 1 && b.pares_conectados() == 1));

    // A minera um bloco e o injeta. Deve chegar em B pela difusão.
    let bloco = {
        let no = a.no.lock().unwrap();
        let ts = no.chain.tip().unwrap().header.timestamp + ParametrosRede::REGTEST.target_spacing;
        no.chain.mine(MINERADOR, vec![], Some(ts), 2).unwrap()
    };
    assert!(a.submeter_bloco(bloco).unwrap());
    assert_eq!(altura(&a), 3);

    // Prazos folgados de propósito: nesta máquina fraca os testes rodam em
    // paralelo e o Argon2id de um satura os núcleos do outro.
    assert!(esperar(Duration::from_secs(60), || altura(&b) == 3), "B não recebeu o bloco: {}", altura(&b));
    assert_eq!(ponta(&a), ponta(&b));

    a.desligar();
    b.desligar();
}

#[test]
#[ignore = "integração de rede: sobe sockets; rode isolado com: cargo test -p auron-net -- --ignored --test-threads=1"]
fn rede_errada_nao_conecta() {
    // A é regtest; um cliente testnet tenta conectar. O magic não bate e o
    // aperto de mão fecha antes de virar par.
    let a = Rede::nova(No::novo(cadeia_com(1)));
    let porta = a.escutar("127.0.0.1:0").unwrap();

    let b = Rede::nova(No::novo(Chain::nova(ParametrosRede::TESTNET).unwrap()));
    b.conectar(("127.0.0.1", porta)).unwrap();

    // Dá tempo de tentar; nenhum vira par.
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(a.pares_conectados(), 0, "A aceitou um par de outra rede");
    assert_eq!(b.pares_conectados(), 0, "B ficou conectado a outra rede");

    a.desligar();
    b.desligar();
}
