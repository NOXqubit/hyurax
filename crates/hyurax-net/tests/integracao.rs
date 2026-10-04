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
//! cargo test -p hyurax-net -- --ignored --test-threads=1
//! ```
//!
//! O gasto duplo no mempool é lógica pura (sem socket), então fica sempre ativo.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects, clippy::indexing_slicing, clippy::collapsible_if)]

use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use hyurax_chain::Chain;
use hyurax_consensus::ParametrosRede;
use hyurax_net::{Conexao, Identidade, No, Papel, Rede};
use hyurax_tx::{HYX, Output, sign_transfer_outputs};
use hyurax_wire::{MAX_FRAME_BODY, Message, PROTOCOL_VERSION, Ponta};

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

fn rede(no: No) -> Arc<Rede> {
    Rede::nova(no).unwrap()
}

fn zerado() -> Arc<Rede> {
    rede(No::novo(regtest_com(0, MINERADOR)))
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
    let a = rede(No::novo(cadeia_com(4)));
    let b = rede(No::novo(cadeia_com(0)));
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
    let a = rede(No::novo(cadeia_com(2)));
    let b = rede(No::novo(cadeia_com(2)));
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
    let a = rede(No::novo(cadeia_com(1)));
    let porta = a.escutar("127.0.0.1:0").unwrap();
    let b = rede(No::novo(Chain::nova(ParametrosRede::TESTNET).unwrap()));
    b.conectar(("127.0.0.1", porta)).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(a.pares_conectados(), 0, "A aceitou um par de outra rede");
    assert_eq!(b.pares_conectados(), 0, "B ficou conectado a outra rede");
    a.desligar();
    b.desligar();
}

/// Duas cadeias que saem do mesmo tronco e divergem: `base` blocos em comum,
/// depois `extra` blocos minerados por `minerador` (minerador diferente = ramo
/// diferente).
fn ramo(base: &Chain, extra: u64, minerador: [u8; 20]) -> Chain {
    let p = ParametrosRede::REGTEST;
    let mut chain = base.clone();
    for _ in 0..extra {
        let ts = chain.tip().unwrap().header.timestamp + p.target_spacing;
        let bloco = chain.mine(minerador, vec![], Some(ts), 2).unwrap();
        chain.accept_block(bloco, Some(ts + 10)).unwrap();
    }
    chain
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn cadeia_com_mais_trabalho_vence_mesmo_bifurcando_fundo() {
    // Tronco comum de 2 blocos. A segue com 3 (altura 5); B segue com 6
    // (altura 8), num ramo diferente. A precisa abandonar os seus 3 e adotar
    // os 6 de B: reorganização profunda.
    let tronco = cadeia_com(2);
    let cadeia_a = ramo(&tronco, 3, [0xA1; 20]);
    let cadeia_b = ramo(&tronco, 6, [0xB2; 20]);
    assert_eq!(cadeia_a.height(), 5);
    assert_eq!(cadeia_b.height(), 8);
    assert_ne!(cadeia_a.tip_hash(), cadeia_b.tip_hash(), "os ramos precisam divergir");
    let ponta_b = cadeia_b.tip_hash();

    let a = rede(No::novo(cadeia_a));
    let b = rede(No::novo(cadeia_b));
    let porta = b.escutar("127.0.0.1:0").unwrap();
    a.conectar(("127.0.0.1", porta)).unwrap();

    assert!(esperar(Duration::from_secs(60), || altura(&a) == 8),
        "A não reorganizou: altura {}", altura(&a));
    assert_eq!(ponta(&a), ponta_b, "A adotou uma cadeia diferente da de B");
    // B não muda: tem mais trabalho.
    assert_eq!(altura(&b), 8);
    a.desligar();
    b.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn ramo_mais_curto_nao_desvia_a_cadeia() {
    // O contrário: B tem MENOS trabalho. A não pode trocar.
    let tronco = cadeia_com(2);
    let cadeia_a = ramo(&tronco, 5, [0xA1; 20]);
    let cadeia_b = ramo(&tronco, 2, [0xB2; 20]);
    let ponta_a = cadeia_a.tip_hash();

    let a = rede(No::novo(cadeia_a));
    let b = rede(No::novo(cadeia_b));
    let porta = b.escutar("127.0.0.1:0").unwrap();
    a.conectar(("127.0.0.1", porta)).unwrap();

    // Dá tempo de trocarem tudo o que quiserem; B é que deve alcançar A.
    assert!(esperar(Duration::from_secs(60), || altura(&b) == 7), "B não alcançou A: {}", altura(&b));
    assert_eq!(altura(&a), 7, "A trocou por um ramo mais fraco");
    assert_eq!(ponta(&a), ponta_a, "A mudou de ponta sem precisar");
    a.desligar();
    b.desligar();
}

// ==================================================================== ATAQUES

fn conectar_e_apertar_mao(porta: u16, magic: [u8; 4]) -> Conexao {
    let stream = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    // O atacante tem a própria identidade: passa pela cifra e ataca por dentro.
    let (mut c, _) = Conexao::com_cifra(stream, magic, &Identidade::nova().unwrap(), Papel::Discou, Duration::from_secs(10)).unwrap();
    let meu = Ponta { protocolo: PROTOCOL_VERSION, magic, altura: 0, trabalho: [0u8; 32], nonce: 0xA1, porta_escuta: 0 };
    c.enviar(&Message::Hello(meu)).unwrap();
    match c.receber().unwrap() {
        Message::HelloAck { eco, .. } => assert_eq!(eco, 0xA1, "alvo não ecoou o nonce"),
        outra => panic!("esperava HELLO_ACK, veio {outra:?}"),
    }
    c
}

fn alvo_continua_vivo(porta: u16, altura_esperada: u64) {
    let honesto = rede(No::novo(regtest_com(0, MINERADOR)));
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
    let alvo = rede(No::novo(regtest_com(2, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;

    let mut conexao = conectar_e_apertar_mao(porta, magic);
    let mut quadro = Vec::new();
    quadro.extend_from_slice(&magic);
    quadro.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    quadro.extend_from_slice(&1u16.to_be_bytes());
    quadro.extend_from_slice(&(MAX_FRAME_BODY + 1).to_be_bytes());
    let _ = conexao.enviar_cru(&quadro);

    assert!(esperar(Duration::from_secs(60), || alvo.pares_conectados() == 0), "o atacante não foi desconectado");
    assert_eq!(altura(&alvo), 2);
    alvo_continua_vivo(porta, 2);
    alvo.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn bloco_forjado_sem_prova_e_recusado() {
    let alvo = rede(No::novo(regtest_com(3, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;

    // A cópia vem da própria cadeia do alvo. Minerar outra cópia à parte não
    // serve: com duas linhas e dificuldade de regtest, o nonce achado varia, e
    // o bloco forjado deixaria de encaixar na ponta do alvo (falhou assim no
    // GitHub Actions).
    let espelho = alvo.no.lock().unwrap().chain.clone();
    let mut forjado = espelho
        .build_candidate(MINERADOR, vec![], Some(espelho.tip().unwrap().header.timestamp + 120), Vec::new())
        .unwrap();
    // O alvo do regtest é fácil: um nonce qualquer às vezes bate a prova por
    // sorte, e aí o bloco nem é forjado. Escolhe um que comprovadamente não bate.
    while hyurax_chain::conferir_pow(&ParametrosRede::REGTEST, &forjado.header).is_ok() {
        forjado.header.nonce += 1;
    }

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
    let alvo = rede(No::novo(regtest_com(1, MINERADOR)));
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
    let dono = hyurax_crypto::address_from_ed25519_pubkey(&hyurax_crypto::ed25519_public_key(&segredo));
    let chain = regtest_com(p.coinbase_maturity + 1, dono);
    let mut no = No::novo(chain);
    assert!(no.chain.state.balance(&dono, &HYX) > 0, "a conta precisa de saldo maduro");

    let uma = |dest: [u8; 20]| {
        sign_transfer_outputs(&segredo, &p.magic, dono, vec![Output { recipient: dest, asset_id: HYX, amount: 1_000 }], 0, 0).unwrap()
    };
    assert_eq!(no.adicionar_tx(uma([0x31u8; 20])), Ok(true), "a primeira devia entrar");
    assert_eq!(no.adicionar_tx(uma([0x32u8; 20])), Ok(false), "o gasto duplo entrou no mempool");
    assert_eq!(no.mempool_len(), 1, "o mempool tem mais de uma versão do mesmo gasto");
}

/// Uma conta com saldo maduro, e uma fábrica de transferências assinadas por ela.
fn conta_com_saldo() -> (No, [u8; 20], u64, impl Fn(u64, u64) -> hyurax_tx::Transfer) {
    let p = ParametrosRede::REGTEST;
    let segredo = [0x21u8; 32];
    let dono = hyurax_crypto::address_from_ed25519_pubkey(&hyurax_crypto::ed25519_public_key(&segredo));
    let no = No::novo(regtest_com(p.coinbase_maturity + 1, dono));
    let saldo = no.chain.state.balance(&dono, &HYX);
    assert!(saldo > 0, "a conta precisa de saldo maduro");
    let assinar = move |nonce: u64, valor: u64| {
        let saida = Output { recipient: [0x31u8; 20], asset_id: HYX, amount: valor };
        sign_transfer_outputs(&segredo, &p.magic, dono, vec![saida], 0, nonce).unwrap()
    };
    (no, dono, saldo, assinar)
}

#[test]
fn nonce_com_buraco_nao_entra_no_mempool() {
    // Lógica pura, sem socket. O mempool é o que o minerador põe no bloco sem
    // conferir de novo: um nonce fora de sequência viraria bloco recusado, e a
    // rodada inteira de mineração iria fora.
    let (mut no, _dono, _saldo, assinar) = conta_com_saldo();
    assert_eq!(no.adicionar_tx(assinar(0, 1_000)), Ok(true), "o nonce 0 devia entrar");
    assert_eq!(no.adicionar_tx(assinar(2, 1_000)), Ok(false), "o nonce 2 pulou o 1 e entrou");
    assert_eq!(no.adicionar_tx(assinar(9, 1_000)), Ok(false), "um nonce distante entrou");
    assert_eq!(no.mempool_len(), 1, "o mempool guardou transferência que não dá para minerar");
    // Fechado o buraco, a fila anda.
    assert_eq!(no.adicionar_tx(assinar(1, 1_000)), Ok(true), "o nonce 1 devia entrar");
    assert_eq!(no.adicionar_tx(assinar(2, 1_000)), Ok(true), "agora o 2 é o seguinte");
    assert_eq!(no.mempool_len(), 3);
}

#[test]
fn saldo_e_contado_somando_a_fila_inteira() {
    let (mut no, _dono, saldo, assinar) = conta_com_saldo();
    // Duas que, sozinhas, cabem; juntas, não.
    let metade_e_pouco = saldo / 2 + 10;
    assert_eq!(no.adicionar_tx(assinar(0, metade_e_pouco)), Ok(true), "a primeira devia entrar");
    assert_eq!(
        no.adicionar_tx(assinar(1, metade_e_pouco)),
        Ok(false),
        "duas que somadas passam do saldo entraram juntas no mempool"
    );
    assert_eq!(no.mempool_len(), 1);
    // O que cabe no que sobrou entra.
    let sobra = saldo - metade_e_pouco;
    assert_eq!(no.adicionar_tx(assinar(1, sobra)), Ok(true), "o que cabia na sobra foi recusado");
    assert_eq!(no.mempool_len(), 2);
    // E agora não cabe mais nada, nem um.
    assert_eq!(no.adicionar_tx(assinar(2, 1)), Ok(false), "entrou gasto sem saldo nenhum sobrando");
}

#[test]
fn a_fila_do_mempool_sempre_vira_um_bloco_valido() {
    // A promessa que o mempool faz: dá para minerar tudo o que está nele, na
    // ordem em que está. Este teste cobra a promessa de ponta a ponta.
    let (mut no, _dono, saldo, assinar) = conta_com_saldo();
    let pedaco = saldo / 4;
    for nonce in 0..3 {
        assert_eq!(no.adicionar_tx(assinar(nonce, pedaco)), Ok(true), "nonce {nonce}");
    }
    // Lixo que não pode encostar na fila: buraco e gasto acima do que sobrou.
    assert_eq!(no.adicionar_tx(assinar(7, 1)), Ok(false));
    assert_eq!(no.adicionar_tx(assinar(3, saldo)), Ok(false));

    let p = ParametrosRede::REGTEST;
    let ts = no.chain.tip().unwrap().header.timestamp + p.target_spacing;
    let bloco = no.chain.mine(MINERADOR, no.mempool_ordenado(), Some(ts), 2).unwrap();
    assert_eq!(bloco.transactions.len(), 4, "coinbase mais as três da fila");
    assert_eq!(no.aceitar_bloco(bloco), Ok(true), "o bloco montado com o mempool foi recusado");
    assert_eq!(no.mempool_len(), 0, "o que entrou no bloco continua no mempool");
}

#[test]
fn bloco_que_derruba_o_comeco_da_fila_leva_o_rabo_junto() {
    let p = ParametrosRede::REGTEST;
    let segredo = [0x21u8; 32];
    let dono = hyurax_crypto::address_from_ed25519_pubkey(&hyurax_crypto::ed25519_public_key(&segredo));
    // Só o primeiro bloco paga o dono: assim o saldo dele é exatamente uma
    // recompensa, e nenhuma outra amadurece no meio do teste para confundir a
    // conta.
    let mut chain = Chain::nova(p).unwrap();
    for i in 0..=p.coinbase_maturity {
        let ts = chain.tip().unwrap().header.timestamp + p.target_spacing;
        let quem = if i == 0 { dono } else { MINERADOR };
        let bloco = chain.mine(quem, vec![], Some(ts), 2).unwrap();
        chain.accept_block(bloco, Some(ts + 10)).unwrap();
    }
    let mut no = No::novo(chain);
    let saldo = no.chain.state.balance(&dono, &HYX);
    assert!(saldo > 0, "a conta precisa de saldo maduro");
    let assinar = |nonce: u64, valor: u64, destino: [u8; 20]| {
        let saida = Output { recipient: destino, asset_id: HYX, amount: valor };
        sign_transfer_outputs(&segredo, &p.magic, dono, vec![saida], 0, nonce).unwrap()
    };
    assert_eq!(no.adicionar_tx(assinar(0, saldo / 4, [0x31u8; 20])), Ok(true));
    assert_eq!(no.adicionar_tx(assinar(1, saldo / 4, [0x31u8; 20])), Ok(true));
    assert_eq!(no.mempool_len(), 2);

    // Outro nó minerou uma transferência diferente com o mesmo nonce 0, e ela
    // gasta o saldo inteiro: a do nonce 0 daqui fica obsoleta, e a do nonce 1
    // deixa de ter com que pagar.
    let espelho = no.chain.clone();
    let ts = espelho.tip().unwrap().header.timestamp + p.target_spacing;
    let bloco = espelho.mine(MINERADOR, vec![assinar(0, saldo, [0x99u8; 20])], Some(ts), 2).unwrap();
    assert_eq!(no.aceitar_bloco(bloco), Ok(true));

    assert_eq!(no.chain.state.balance(&dono, &HYX), 0, "o bloco devia ter gasto o saldo todo");
    assert_eq!(no.mempool_len(), 0, "sobrou no mempool transferência que já não dá para pagar");
    // E o nó continua sabendo dizer qual é o próximo nonce livre.
    assert_eq!(no.proximo_nonce(&dono), 1);
}

// =============================================================== REANIMAÇÃO

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn um_no_sobrevivente_ressemeia_a_rede() {
    let sobrevivente = rede(No::novo(regtest_com(5, MINERADOR)));
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

// ================================================================== CIFRA

/// Um intermediário no caminho: aceita a conexão de um lado, abre outra para o
/// alvo e repassa os bytes nos dois sentidos, guardando tudo o que passou.
/// Com `trocar_em = Some(n)`, inverte um bit do n-ésimo byte que vem do alvo.
fn intermediario(alvo: u16, trocar_em: Option<usize>) -> (u16, Arc<std::sync::Mutex<Vec<u8>>>) {
    use std::io::Read;
    let escuta = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = escuta.local_addr().unwrap().port();
    let gravado = Arc::new(std::sync::Mutex::new(Vec::new()));
    let g = Arc::clone(&gravado);
    std::thread::spawn(move || {
        let Ok((cliente, _)) = escuta.accept() else { return };
        let Ok(servidor) = std::net::TcpStream::connect(("127.0.0.1", alvo)) else { return };
        let repassa = move |mut de: std::net::TcpStream, mut para: std::net::TcpStream, g: Arc<std::sync::Mutex<Vec<u8>>>, trocar: Option<usize>| {
            let mut total = 0usize;
            let mut buf = [0u8; 4096];
            loop {
                let n = match de.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => n };
                if let Some(pos) = trocar {
                    if pos >= total && pos < total + n {
                        buf[pos - total] ^= 0x01;
                    }
                }
                total += n;
                g.lock().unwrap().extend_from_slice(&buf[..n]);
                if para.write_all(&buf[..n]).is_err() { break; }
            }
            let _ = para.shutdown(std::net::Shutdown::Both);
        };
        let (c2, s2) = (cliente.try_clone().unwrap(), servidor.try_clone().unwrap());
        let g2 = Arc::clone(&g);
        std::thread::spawn(move || repassa(c2, s2, g2, None));
        repassa(servidor, cliente, g, trocar_em);
    });
    (porta, gravado)
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn intermediario_so_ve_bytes_cifrados() {
    let a = rede(No::novo(cadeia_com(4)));
    let porta_a = a.escutar("127.0.0.1:0").unwrap();
    let (porta_meio, gravado) = intermediario(porta_a, None);
    let b = zerado();
    b.conectar(("127.0.0.1", porta_meio)).unwrap();

    assert!(esperar(Duration::from_secs(60), || altura(&b) == 4), "B não sincronizou pelo intermediário: {}", altura(&b));
    let visto = gravado.lock().unwrap().clone();
    assert!(visto.len() > 1000, "o intermediário quase não viu tráfego: {} bytes", visto.len());
    // O que o intermediário guardou não tem nada legível: nem o hash da ponta,
    // nem a magic dos quadros do hyurax-wire.
    let ponta_a = ponta(&a);
    assert!(!visto.windows(64).any(|w| w == ponta_a), "o hash da ponta passou em claro");
    let magic = ParametrosRede::REGTEST.magic;
    assert!(!visto.windows(4).any(|w| w == magic), "a magic dos quadros passou em claro");
    a.desligar();
    b.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn intermediario_que_altera_derruba_a_conexao() {
    let a = rede(No::novo(cadeia_com(4)));
    let porta_a = a.escutar("127.0.0.1:0").unwrap();
    // A resposta do XX (e, ee, s, es) tem 96 bytes mais 2 de tamanho. O byte
    // 110 cai no primeiro pedaço cifrado depois do aperto de mão.
    let (porta_meio, _) = intermediario(porta_a, Some(110));
    let b = zerado();
    b.conectar(("127.0.0.1", porta_meio)).unwrap();

    std::thread::sleep(Duration::from_secs(5));
    assert_eq!(altura(&b), 0, "B aceitou dados alterados no caminho");
    assert_eq!(b.pares_conectados(), 0, "B manteve uma conexão adulterada");
    a.desligar();
    b.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn identidade_do_no_e_provada_na_cifra() {
    let segredo = [0x42u8; 32];
    let identidade = Identidade::de_segredo(segredo).unwrap();
    let a = Rede::com_identidade(No::novo(cadeia_com(1)), identidade.clone());
    assert_eq!(a.identidade_publica(), identidade.publica());
    let porta = a.escutar("127.0.0.1:0").unwrap();
    let stream = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    let (_, chave_do_par) = Conexao::com_cifra(stream, ParametrosRede::REGTEST.magic, &Identidade::nova().unwrap(), Papel::Discou, Duration::from_secs(10)).unwrap();
    assert_eq!(chave_do_par, identidade.publica(), "a chave provada no aperto não é a do nó");
    a.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn uma_conexao_por_no_mesmo_discando_varias_vezes() {
    let a = zerado();
    let b = zerado();
    let porta_a = a.escutar("127.0.0.1:0").unwrap();
    let porta_b = b.escutar("127.0.0.1:0").unwrap();
    for _ in 0..3 {
        b.conectar(("127.0.0.1", porta_a)).unwrap();
        a.conectar(("127.0.0.1", porta_b)).unwrap();
    }
    // e um nó discando para si mesmo
    a.conectar(("127.0.0.1", porta_a)).unwrap();
    std::thread::sleep(Duration::from_secs(6));
    assert!(esperar(Duration::from_secs(30), || a.pares_conectados() == 1 && b.pares_conectados() == 1),
        "conexões duplicadas: A={} B={}", a.pares_conectados(), b.pares_conectados());
    a.desligar();
    b.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn orfao_forjado_derruba_quem_mandou() {
    let alvo = rede(No::novo(regtest_com(2, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;

    // Um bloco que não encaixa na ponta (pai inventado) e sem prova de trabalho
    // de verdade: alvo o mais difícil possível, nonce qualquer.
    let espelho = alvo.no.lock().unwrap().chain.clone();
    let mut forjado = espelho
        .build_candidate(MINERADOR, vec![], Some(espelho.tip().unwrap().header.timestamp + 120), Vec::new())
        .unwrap();
    forjado.header.prev_hash = [0x66; 64];
    forjado.header.bits = 0x0300_0001;

    let mut conexao = conectar_e_apertar_mao(porta, magic);
    conexao.enviar(&Message::Block(Box::new(forjado))).unwrap();

    assert!(esperar(Duration::from_secs(60), || alvo.pares_conectados() == 0), "quem mandou órfão forjado continua conectado");
    assert_eq!(altura(&alvo), 2);
    alvo_continua_vivo(porta, 2);
    alvo.desligar();
}

#[test]
fn transferencia_impagavel_nao_derruba_o_par() {
    // Saldo é opinião da minha cadeia: um par que relaia uma transferência
    // gastando dinheiro que chegou numa outra que eu ainda não vi está sendo
    // honesto. Recusar, sim; derrubar a conexão, não — senão a rede se parte
    // por desencontro de vista. (Qualquer Malicia aqui derruba o par: ver
    // servidor.rs, "par malicioso".)
    let (mut no, _dono, saldo, assinar) = conta_com_saldo();
    let impagavel = assinar(0, saldo.saturating_add(1_000_000));
    assert_eq!(no.adicionar_tx(impagavel), Ok(false), "gastar mais que o saldo derrubou o par");
    assert_eq!(no.mempool_len(), 0, "o impagável ficou guardado");

    // O que continua sendo malícia: coinbase solta na rede, que nenhuma vista
    // de cadeia pode tornar válida.
    let coinbase = hyurax_tx::Tx::Coinbase(hyurax_tx::Coinbase {
        height: 1,
        recipient: MINERADOR,
        amount: 1,
        extra_nonce: Vec::new(),
    });
    assert!(no.adicionar_qualquer_tx(coinbase).is_err(), "coinbase solta devia derrubar o par");
}

// ============================================================ PRAZOS E BANIMENTO

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn aperto_gotejado_cai_no_prazo() {
    let alvo = rede(No::novo(regtest_com(1, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let mut s = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    s.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    // anuncia uma mensagem de aperto de 200 bytes e goteja um byte a cada 300 ms:
    // sem nunca dar timeout de leitura, como faria um atacante paciente
    s.write_all(&200u16.to_be_bytes()).unwrap();
    let inicio = Instant::now();
    let mut caiu = false;
    while inicio.elapsed() < Duration::from_secs(40) {
        if s.write_all(&[0u8]).is_err() {
            caiu = true;
            break;
        }
        let mut b = [0u8; 1];
        match s.read(&mut b) {
            Ok(0) => {
                caiu = true;
                break;
            }
            Err(e) if !matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                caiu = true;
                break;
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    assert!(caiu, "o aperto gotejado não caiu");
    assert!(inicio.elapsed() < Duration::from_secs(25), "caiu tarde demais: {:?}", inicio.elapsed());
    alvo_continua_vivo(porta, 1);
    alvo.desligar();
}

fn apertar_com(porta: u16, magic: [u8; 4], identidade: &Identidade) -> Result<Conexao, String> {
    let stream = std::net::TcpStream::connect(("127.0.0.1", porta)).map_err(|e| e.to_string())?;
    let (mut c, _) = Conexao::com_cifra(stream, magic, identidade, Papel::Discou, Duration::from_secs(10)).map_err(|e| e.to_string())?;
    let meu = Ponta { protocolo: PROTOCOL_VERSION, magic, altura: 0, trabalho: [0u8; 32], nonce: 0xB2, porta_escuta: 0 };
    c.enviar(&Message::Hello(meu)).map_err(|e| e.to_string())?;
    match c.receber() {
        Ok(Message::HelloAck { .. }) => Ok(c),
        Ok(outra) => Err(format!("esperava HELLO_ACK, veio {outra:?}")),
        Err(e) => Err(e.to_string()),
    }
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn identidade_que_mostrou_malicia_fica_banida() {
    let alvo = rede(No::novo(regtest_com(2, MINERADOR)));
    let porta = alvo.escutar("127.0.0.1:0").unwrap();
    let magic = ParametrosRede::REGTEST.magic;
    let atacante = Identidade::nova().unwrap();

    let espelho = alvo.no.lock().unwrap().chain.clone();
    let mut forjado = espelho
        .build_candidate(MINERADOR, vec![], Some(espelho.tip().unwrap().header.timestamp + 120), Vec::new())
        .unwrap();
    forjado.header.prev_hash = [0x66; 64];
    forjado.header.bits = 0x0300_0001;
    let mut c = apertar_com(porta, magic, &atacante).unwrap();
    c.enviar(&Message::Block(Box::new(forjado))).unwrap();
    assert!(esperar(Duration::from_secs(60), || alvo.pares_conectados() == 0), "o forjador não caiu");

    // a mesma identidade volta: a cifra fecha, mas o alvo não a aceita como par
    std::thread::sleep(Duration::from_millis(500));
    assert!(apertar_com(porta, magic, &atacante).is_err(), "identidade banida voltou a ser par");
    assert_eq!(alvo.pares_conectados(), 0);
    // outra identidade, do mesmo computador, continua bem-vinda
    alvo_continua_vivo(porta, 2);
    alvo.desligar();
}

// ================================================================ MALHA
// docs/HYURAX-MALHA.md: os nós se acham e se alcançam sem servidor central.

fn porta_udp_livre() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn dois_nos_sem_porta_aberta_se_falam_por_uma_ponte() {
    // R escuta (é alcançável); A e B não escutam (como atrás de NAT)
    let r = rede(No::novo(cadeia_com(2)));
    let a = zerado();
    let b = zerado();
    let porta = r.escutar("127.0.0.1:0").unwrap();
    b.conectar(("127.0.0.1", porta)).unwrap();
    a.conectar(("127.0.0.1", porta)).unwrap();
    let id_a = a.identidade_publica();
    let id_b = b.identidade_publica();
    let conectado = |x: &Arc<Rede>, id: [u8; 32]| x.pares_com_identidade().iter().any(|(_, i)| *i == id);
    // B guarda uma vaga na ponte, A aprende a rota e disca por ela
    assert!(
        esperar(Duration::from_secs(60), || conectado(&a, id_b) && conectado(&b, id_a)),
        "A e B não se conectaram pela ponte: A {:?}, B {:?}, R {:?}",
        a.estado_da_malha(),
        b.estado_da_malha(),
        r.estado_da_malha()
    );
    assert!(r.estado_da_malha().circuitos >= 1, "a conexão A-B passa pela ponte R");
    // e a cadeia anda pelos dois caminhos
    assert!(esperar(Duration::from_secs(60), || altura(&a) == 2 && altura(&b) == 2));
    for x in [&a, &b, &r] {
        x.desligar();
    }
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn ponte_sem_reserva_recusa_o_circuito() {
    let r = zerado();
    let porta = r.escutar("127.0.0.1:0").unwrap();
    // circuito até uma identidade sem reserva: a ponte responde 0 e fecha
    let mut s = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(&hyurax_net::malha::Prefixo::Circuito([9; 32]).bytes()).unwrap();
    let mut resposta = [7u8; 1];
    s.read_exact(&mut resposta).unwrap();
    assert_eq!(resposta, [0]);
    // reserva com token que ninguém pediu (fora da cifra) não guarda nada
    let mut falsa = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    falsa.write_all(&hyurax_net::malha::Prefixo::Reserva([5; 16]).bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let mut s2 = std::net::TcpStream::connect(("127.0.0.1", porta)).unwrap();
    s2.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s2.write_all(&hyurax_net::malha::Prefixo::Circuito([9; 32]).bytes()).unwrap();
    s2.read_exact(&mut resposta).unwrap();
    assert_eq!(resposta, [0], "reserva sem token pedido pela cifra não vale");
    // e o nó continua atendendo o aperto de mão normal
    alvo_continua_vivo(porta, 0);
    r.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn no_confere_o_proprio_alcance_com_um_par() {
    let r = zerado();
    let a = zerado();
    let porta_r = r.escutar("127.0.0.1:0").unwrap();
    a.escutar("127.0.0.1:0").unwrap();
    a.conectar(("127.0.0.1", porta_r)).unwrap();
    // R tenta voltar até A com o token; como R é da rede local (loopback), o
    // resultado vale só para a rede local
    assert!(
        esperar(Duration::from_secs(40), || a.estado_da_malha().alcance == "só na rede local"),
        "alcance de A: {:?}",
        a.estado_da_malha()
    );
    a.desligar();
    r.desligar();
}

#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn vizinhos_na_rede_local_se_acham_sem_semente() {
    let a = rede(No::novo(cadeia_com(3)));
    let b = zerado();
    a.escutar("127.0.0.1:0").unwrap();
    b.escutar("127.0.0.1:0").unwrap();
    let (ua, ub) = (porta_udp_livre(), porta_udp_livre());
    // nenhuma semente, nenhum conectar(): só o anúncio na rede local
    a.ligar_vizinhos(ua, vec![format!("127.0.0.1:{ub}").parse().unwrap()]).unwrap();
    b.ligar_vizinhos(ub, vec![format!("127.0.0.1:{ua}").parse().unwrap()]).unwrap();
    assert!(esperar(Duration::from_secs(60), || b.pares_conectados() >= 1), "B não achou A: {:?}", b.estado_da_malha());
    assert!(b.estado_da_malha().vizinhos >= 1);
    assert!(esperar(Duration::from_secs(60), || altura(&b) == 3), "B não sincronizou pelo vizinho");
    a.desligar();
    b.desligar();
}

#[test]
fn pacote_do_eter_leva_blocos_e_transacoes_sem_rede() {
    // Lógica sem socket: o pacote é o que atravessa pendrive, Bluetooth ou som.
    let (mut no, _dono, _saldo, assinar) = conta_com_saldo();
    assert_eq!(no.adicionar_tx(assinar(0, 1_000)), Ok(true));
    let altura_a = no.chain.height();
    let a = rede(no);
    let pacote = a.exportar_pacote(1000).unwrap();

    // um nó que nunca falou com A recebe o arquivo e fica com a cadeia e a tx
    let b = zerado();
    let r = b.importar_pacote(&pacote).unwrap();
    assert_eq!((r.blocos as u64, r.transacoes, r.recusados), (altura_a, 1, 0), "{r:?}");
    assert_eq!(altura(&b), altura_a);
    assert_eq!(b.no.lock().unwrap().mempool_len(), 1);
    // importar de novo não muda nada
    let de_novo = b.importar_pacote(&pacote).unwrap();
    assert_eq!((de_novo.blocos, de_novo.transacoes), (0, 0), "{de_novo:?}");

    // um byte trocado dentro de um quadro: só aquele quadro é recusado
    let mut ruim = pacote.clone();
    let meio = ruim.len() / 2;
    ruim[meio] ^= 0xff;
    let c = zerado();
    match c.importar_pacote(&ruim) {
        Ok(r) => assert!(r.recusados >= 1, "{r:?}"),
        Err(e) => assert!(e.contains("ilegível"), "{e}"),
    }
    // pacote de outra rede
    let mut outra = pacote;
    outra[4] ^= 1;
    assert!(zerado().importar_pacote(&outra).unwrap_err().contains("outra rede"));
}

/// Os bytes que passam pelo socket entram na contagem do fio, nos dois
/// sentidos (a contagem é do processo inteiro: só cresce).
#[test]
fn bytes_no_fio_contam_o_que_passa_pelo_socket() {
    let ouvinte = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endereco = ouvinte.local_addr().unwrap();
    let magic = *b"HYXt";
    let (env0, rec0) = hyurax_net::bytes_no_fio();
    let lado = std::thread::spawn(move || {
        let (s, _) = ouvinte.accept().unwrap();
        let mut c = Conexao::nova(s, magic);
        c.definir_timeout(Some(Duration::from_secs(5))).unwrap();
        let m = c.receber().unwrap();
        c.enviar(&m).unwrap();
    });
    let mut c = Conexao::nova(std::net::TcpStream::connect(endereco).unwrap(), magic);
    c.definir_timeout(Some(Duration::from_secs(5))).unwrap();
    c.enviar(&Message::Ping(7)).unwrap();
    assert_eq!(c.receber().unwrap(), Message::Ping(7));
    lado.join().unwrap();
    let (env1, rec1) = hyurax_net::bytes_no_fio();
    // dois quadros de ida (um de cada lado) e dois de volta
    assert!(env1 - env0 >= 2 * 9, "enviados {}", env1 - env0);
    assert!(rec1 - rec0 >= 2 * 9, "recebidos {}", rec1 - rec0);
}

// ============================================================ WEBSOCKET

/// A semente atrás de HTTPS (o Render): o nó escuta WebSocket, o outro disca
/// `ws://…/p2p`, e por dentro é a mesma conexão de sempre: aperto Noise,
/// sincronização e bloco novo atravessando. A página de saúde responde.
#[test]
#[ignore = "sobe sockets — ver cabeçalho do arquivo"]
fn no_por_websocket_sincroniza_e_recebe_bloco() {
    let semente = rede(No::novo(cadeia_com(4)));
    let porta_ws = semente.escutar_ws(0).unwrap();
    let b = zerado();
    b.conectar_texto(&format!("ws://127.0.0.1:{porta_ws}/p2p")).unwrap();
    assert!(esperar(Duration::from_secs(60), || altura(&b) == 4), "não sincronizou pelo WebSocket: {}", altura(&b));
    assert_eq!(ponta(&semente), ponta(&b));
    assert_eq!((semente.pares_conectados(), b.pares_conectados()), (1, 1));

    // bloco novo do lado da semente chega pelo cano
    let bloco = {
        let no = semente.no.lock().unwrap();
        let ts = no.chain.tip().unwrap().header.timestamp + ParametrosRede::REGTEST.target_spacing;
        no.chain.mine(MINERADOR, vec![], Some(ts), 2).unwrap()
    };
    assert!(semente.submeter_bloco(bloco).unwrap());
    assert!(esperar(Duration::from_secs(60), || altura(&b) == 5), "bloco não atravessou: {}", altura(&b));

    // a página de saúde (o Render confere por ela) e o 404
    let get = |caminho: &str| {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", porta_ws)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s.write_all(format!("GET {caminho} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
        let mut r = String::new();
        let _ = s.read_to_string(&mut r);
        r
    };
    let saude = get("/saude");
    assert!(saude.starts_with("HTTP/1.1 200") && saude.contains("altura=5"), "{saude}");
    assert!(get("/nada").starts_with("HTTP/1.1 404"));
    // upgrade sem a chave não vira conexão
    let mut s = std::net::TcpStream::connect(("127.0.0.1", porta_ws)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"GET /p2p HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\n\r\n").unwrap();
    let mut r = String::new();
    let _ = s.read_to_string(&mut r);
    assert!(!r.contains("101"), "{r}");
    semente.desligar();
    b.desligar();
}
