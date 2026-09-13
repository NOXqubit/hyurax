// ✝ João 17:21 — “Para que todos sejam um.”
//! Integração da rede entre nós, com nós de verdade em loopback.
//!
//! Tudo num binário só, de propósito: com `--test-threads=1` os cenários rodam
//! um de cada vez, e nunca dois nós-de-teste disputam os poucos núcleos desta
//! máquina. Não é vetor byte a byte (isto é I/O, não consenso): é integração.
//!
//! Os testes que sobem sockets são `#[ignore]` e rodam sob demanda:
//!
//! ```text
//! cargo test -p auron-net -- --ignored --test-threads=1
//! ```
//!
//! O gasto duplo no mempool é lógica pura (sem socket), então fica sempre ativo.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects)]

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use auron_chain::Chain;
use auron_consensus::ParametrosRede;
use auron_net::{Conexao, No, Rede};
use auron_tx::{AUR, Output, sign_transfer_outputs};
use auron_wire::{MAX_FRAME_BODY, Message, PROTOCOL_VERSION, Ponta};

const MINERADOR: [u8; 20] = [7u8; 20];

fn regtest_com(n: u64, minerador: [u8; 20]) -> Chain {
    let p = ParametrosRede::REGTEST;
    let mut chain = Chain::nova(p).unwrap();
    for _ in 0..n {
        let ts = chain.tip().unwrap().header.timestamp + p.target_spacing;
        let bloco = chain.mine(minerador, vec![], Some(ts), 2).unwrap();
        chain.accept_block(bloco, Some(ts + 10)).unwrap();
    }
    chain
}

fn cadeia_com(n: u64) -> Chain {
    regtest_com(n, MINERADOR)
}

fn zerado() -> Arc<Rede> {
    Rede::nova(No::novo(regtest_com(0, MINERADOR)))
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

// ============================================================ SINCRONIZAÇÃO

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
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
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn bloco_novo_se_espalha_para_os_pares() {
    let a = Rede::nova(No::novo(cadeia_com(2)));
    let b = Rede::nova(No::novo(cadeia_com(2)));
    let porta = a.escutar("127.0.0.1:0").unwrap();
    b.conectar(("127.0.0.1", porta)).unwrap();
    assert!(esperar(Duration::from_secs(60), || a.pares_conectados() == 1 && b.pares_conectados() == 1));

    let bloco = {
        let no = a.no.lock().unwrap();
        let ts = no.chain.tip().unwrap().header.timestamp + ParametrosRede::REGTEST.target_spacing;
        no.chain.mine(MINERADOR, vec![], Some(ts), 2).unwrap()
    };
    assert!(a.submeter_bloco(bloco).unwrap());
    assert_eq!(altura(&a), 3);
    assert!(esperar(Duration::from_secs(60), || altura(&b) == 3), "B não recebeu o bloco: {}", altura(&b));
    assert_eq!(ponta(&a), ponta(&b));
    a.desligar();
    b.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn rede_errada_nao_conecta() {
    let a = Rede::nova(No::novo(cadeia_com(1)));
    let porta = a.escutar("127.0.0.1:0").unwrap();
    let b = Rede::nova(No::novo(Chain::nova(ParametrosRede::TESTNET).unwrap()));
    b.conectar(("127.0.0.1", porta)).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(a.pares_conectados(), 0, "A aceitou um par de outra rede");
    assert_eq!(b.pares_conectados(), 0, "B ficou conectado a outra rede");
    a.desligar();
    b.desligar();
}

// ==================================================================== ATAQUES

fn conectar_e_apertar_mao(porta: u16, magic: [u8; 4]) -> Conexao {
    let stream = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    let mut c = Conexao::nova(stream, magic);
    let meu = Ponta { protocolo: PROTOCOL_VERSION, magic, altura: 0, trabalho: [0u8; 32], nonce: 0xA1, porta_escuta: 0 };
    c.enviar(&Message::Hello(meu)).unwrap();
    match c.receber().unwrap() {
        Message::HelloAck { eco, .. } => assert_eq!(eco, 0xA1, "alvo não ecoou o nonce"),
        outra => panic!("esperava HELLO_ACK, veio {outra:?}"),
    }
    c
}

fn alvo_continua_vivo(porta: u16, altura_esperada: u64) {
    let honesto = Rede::nova(No::novo(regtest_com(0, MINERADOR)));
    honesto.conectar(("127.0.0.1", porta)).unwrap();
    assert!(
        esperar(Duration::from_secs(60), || altura(&honesto) == altura_esperada),
        "o alvo não respondeu a um par honesto depois do ataque: {}",
        altura(&honesto)
    );
    honesto.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn quadro_gigante_nao_estoura_a_memoria() {
    let alvo = Rede::nova(No::novo(regtest_com(2, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;

    let conexao = conectar_e_apertar_mao(porta, magic);
    let mut cru = conexao.clonar_stream().unwrap();
    let mut quadro = Vec::new();
    quadro.extend_from_slice(&magic);
    quadro.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    quadro.extend_from_slice(&1u16.to_be_bytes());
    quadro.extend_from_slice(&(MAX_FRAME_BODY + 1).to_be_bytes());
    let _ = cru.write_all(&quadro);
    let _ = cru.flush();

    assert!(esperar(Duration::from_secs(60), || alvo.pares_conectados() == 0), "o atacante não foi desconectado");
    assert_eq!(altura(&alvo), 2);
    alvo_continua_vivo(porta, 2);
    alvo.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn bloco_forjado_sem_prova_e_recusado() {
    let alvo = Rede::nova(No::novo(regtest_com(3, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;

    let espelho = regtest_com(3, MINERADOR);
    let forjado = espelho
        .build_candidate(MINERADOR, vec![], Some(espelho.tip().unwrap().header.timestamp + 120), Vec::new())
        .unwrap();

    let mut conexao = conectar_e_apertar_mao(porta, magic);
    conexao.enviar(&Message::Block(Box::new(forjado))).unwrap();

    assert!(esperar(Duration::from_secs(60), || alvo.pares_conectados() == 0), "o forjador não foi desconectado");
    assert_eq!(altura(&alvo), 3, "a cadeia mudou com um bloco forjado");
    alvo_continua_vivo(porta, 3);
    alvo.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn lixo_puro_nao_vira_par() {
    let alvo = Rede::nova(No::novo(regtest_com(1, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let mut cru = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    let _ = cru.write_all(&[0xABu8; 4096]);
    let _ = cru.flush();
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(alvo.pares_conectados(), 0, "lixo virou par");
    assert_eq!(altura(&alvo), 1);
    alvo_continua_vivo(porta, 1);
    alvo.desligar();
}

#[test]
fn gasto_duplo_no_mempool_nao_passa() {
    // Lógica pura, sem socket: fica sempre ativo (sem #[ignore]).
    let p = ParametrosRede::REGTEST;
    let segredo = [0x21u8; 32];
    let dono = auron_crypto::address_from_ed25519_pubkey(&auron_crypto::ed25519_public_key(&segredo));
    let chain = regtest_com(p.coinbase_maturity + 1, dono);
    let mut no = No::novo(chain);
    assert!(no.chain.state.balance(&dono, &AUR) > 0, "a conta precisa de saldo maduro");

    let uma = |dest: [u8; 20]| {
        sign_transfer_outputs(&segredo, &p.magic, dono, vec![Output { recipient: dest, asset_id: AUR, amount: 1_000 }], 0, 0).unwrap()
    };
    assert_eq!(no.adicionar_tx(uma([0x31u8; 20])), Ok(true), "a primeira devia entrar");
    assert_eq!(no.adicionar_tx(uma([0x32u8; 20])), Ok(false), "o gasto duplo entrou no mempool");
    assert_eq!(no.mempool_len(), 1, "o mempool tem mais de uma versão do mesmo gasto");
}

// =============================================================== REANIMAÇÃO

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn um_no_sobrevivente_ressemeia_a_rede() {
    let sobrevivente = Rede::nova(No::novo(regtest_com(5, MINERADOR)));
    let porta_s = sobrevivente.escutar("127.0.0.1:0").unwrap();
    let alvo = ponta(&sobrevivente);

    let n1 = zerado();
    let n2 = zerado();
    n1.conectar(("127.0.0.1", porta_s)).unwrap();
    n2.conectar(("127.0.0.1", porta_s)).unwrap();
    assert!(esperar(Duration::from_secs(60), || altura(&n1) == 5 && altura(&n2) == 5),
        "os novos não reconstruíram: n1={} n2={}", altura(&n1), altura(&n2));
    assert_eq!(ponta(&n1), alvo);
    assert_eq!(ponta(&n2), alvo);

    let porta_1 = n1.escutar("127.0.0.1:0").unwrap();
    sobrevivente.desligar();
    std::thread::sleep(Duration::from_secs(1));

    let tarde = zerado();
    tarde.conectar(("127.0.0.1", porta_1)).unwrap();
    assert!(esperar(Duration::from_secs(60), || altura(&tarde) == 5),
        "o nó tardio não reconstruiu a partir de um sobrevivente: {}", altura(&tarde));
    assert_eq!(ponta(&tarde), alvo, "reconstruiu uma cadeia diferente");
    n1.desligar();
    n2.desligar();
    tarde.desligar();
}

// =============================================================== DESCOBERTA

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn no_novo_descobre_a_rede_a_partir_de_uma_semente() {
    let a = zerado();
    let b = zerado();
    let c = zerado();
    let porta_a = a.escutar("127.0.0.1:0").unwrap();
    b.escutar("127.0.0.1:0").unwrap();
    c.escutar("127.0.0.1:0").unwrap();
    b.conectar(("127.0.0.1", porta_a)).unwrap();
    c.conectar(("127.0.0.1", porta_a)).unwrap();

    assert!(esperar(Duration::from_secs(60), || a.enderecos_conhecidos() >= 2),
        "A não aprendeu os vizinhos: {}", a.enderecos_conhecidos());

    let novo = zerado();
    novo.escutar("127.0.0.1:0").unwrap();
    novo.semear(&format!("127.0.0.1:{porta_a}"));
    assert!(esperar(Duration::from_secs(120), || novo.pares_conectados() >= 3),
        "o nó novo não descobriu a rede: {} pares", novo.pares_conectados());

    a.desligar();
    b.desligar();
    c.desligar();
    novo.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn par_que_cai_libera_a_vaga() {
    let a = zerado();
    let b = zerado();
    let porta_a = a.escutar("127.0.0.1:0").unwrap();
    b.escutar("127.0.0.1:0").unwrap();
    b.conectar(("127.0.0.1", porta_a)).unwrap();
    assert!(esperar(Duration::from_secs(60), || a.pares_conectados() >= 1));
    b.desligar();
    assert!(esperar(Duration::from_secs(60), || a.pares_conectados() == 0),
        "A não soltou o par que caiu: {}", a.pares_conectados());
    a.desligar();
}
