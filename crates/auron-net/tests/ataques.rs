// ✝ Isaías 54:17 — “Toda arma forjada contra ti não prosperará.”
//! O chapéu de atacante, contra o próprio Auron, em loopback.
//!
//! Cada teste é um ataque que um nó hostil tentaria, e prova que a defesa
//! segura: a rede recusa o abuso, derruba o par malicioso e continua de pé.
//! Não há aqui nenhuma ferramenta contra sistema de terceiros — só o Auron
//! atacando a si mesmo, que é o que fortalece a rede.

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

/// Um atacante que já passou pelo aperto de mão e agora é um par do alvo.
fn conectar_e_apertar_mao(porta: u16, magic: [u8; 4]) -> Conexao {
    let stream = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    let mut c = Conexao::nova(stream, magic);
    let meu = Ponta { protocolo: PROTOCOL_VERSION, magic, altura: 0, trabalho: [0u8; 32], nonce: 0xA1 };
    c.enviar(&Message::Hello(meu)).unwrap();
    match c.receber().unwrap() {
        Message::HelloAck { eco, .. } => assert_eq!(eco, 0xA1, "alvo não ecoou o nonce"),
        outra => panic!("esperava HELLO_ACK, veio {outra:?}"),
    }
    c
}

/// Prova que o alvo continua vivo: um par honesto conecta e sincroniza.
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
#[ignore = "integração de rede: sobe sockets; rode isolado com: cargo test -p auron-net -- --ignored --test-threads=1"]
fn quadro_gigante_nao_estoura_a_memoria() {
    let alvo = Rede::nova(No::novo(regtest_com(2, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;

    let conexao = conectar_e_apertar_mao(porta, magic);
    // Cabeçalho de quadro anunciando um corpo maior que o teto. Se o nó
    // alocasse o que foi anunciado, cairia; ele confere ANTES e desconecta.
    let mut cru = conexao.clonar_stream().unwrap();
    let mut quadro = Vec::new();
    quadro.extend_from_slice(&magic);
    quadro.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    quadro.extend_from_slice(&1u16.to_be_bytes()); // tipo HELLO
    quadro.extend_from_slice(&(MAX_FRAME_BODY + 1).to_be_bytes());
    let _ = cru.write_all(&quadro);
    let _ = cru.flush();

    // O alvo derruba o atacante e segue vivo.
    assert!(esperar(Duration::from_secs(60), || alvo.pares_conectados() == 0), "o atacante não foi desconectado");
    assert_eq!(altura(&alvo), 2);
    alvo_continua_vivo(porta, 2);
    alvo.desligar();
}

#[test]
#[ignore = "integração de rede: sobe sockets; rode isolado com: cargo test -p auron-net -- --ignored --test-threads=1"]
fn bloco_forjado_sem_prova_e_recusado() {
    let alvo = Rede::nova(No::novo(regtest_com(3, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;

    // O atacante monta um bloco que ESTENDE a ponta do alvo, mas com nonce 0:
    // o trabalho útil confere, o Argon2id não bate o alvo. Bloco forjado.
    let espelho = regtest_com(3, MINERADOR);
    let forjado = espelho.build_candidate(MINERADOR, vec![], Some(espelho.tip().unwrap().header.timestamp + 120), Vec::new()).unwrap();

    let mut conexao = conectar_e_apertar_mao(porta, magic);
    conexao.enviar(&Message::Block(Box::new(forjado))).unwrap();

    // O alvo recusa (PoW não bate), derruba o par, e a cadeia fica intacta.
    assert!(esperar(Duration::from_secs(60), || alvo.pares_conectados() == 0), "o forjador não foi desconectado");
    assert_eq!(altura(&alvo), 3, "a cadeia mudou com um bloco forjado");
    alvo_continua_vivo(porta, 3);
    alvo.desligar();
}

#[test]
#[ignore = "integração de rede: sobe sockets; rode isolado com: cargo test -p auron-net -- --ignored --test-threads=1"]
fn lixo_puro_nao_vira_par() {
    let alvo = Rede::nova(No::novo(regtest_com(1, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();

    // Bytes aleatórios, sem magic, sem aperto de mão.
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
    // Uma conta com chave, com saldo maduro, tenta gastar o mesmo nonce duas
    // vezes. O mempool guarda só a primeira.
    let p = ParametrosRede::REGTEST;
    let segredo = [0x21u8; 32];
    let dono = auron_crypto::address_from_ed25519_pubkey(&auron_crypto::ed25519_public_key(&segredo));
    let chain = regtest_com(p.coinbase_maturity + 1, dono);
    let mut no = No::novo(chain);
    assert!(no.chain.state.balance(&dono, &AUR) > 0, "a conta precisa de saldo maduro");

    let destino1 = [0x31u8; 20];
    let destino2 = [0x32u8; 20];
    let uma = |dest: [u8; 20]| {
        sign_transfer_outputs(&segredo, &p.magic, dono, vec![Output { recipient: dest, asset_id: AUR, amount: 1_000 }], 0, 0).unwrap()
    };

    assert_eq!(no.adicionar_tx(uma(destino1)), Ok(true), "a primeira devia entrar");
    // Mesma conta, mesmo nonce, outro destino: gasto duplo. Não entra e não
    // desaloja a primeira.
    assert_eq!(no.adicionar_tx(uma(destino2)), Ok(false), "o gasto duplo entrou no mempool");
    assert_eq!(no.mempool_len(), 1, "o mempool tem mais de uma versão do mesmo gasto");
}
