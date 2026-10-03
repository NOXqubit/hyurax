// ✝ Provérbios 11:1 — “A balança enganosa é abominação para o SENHOR, mas o peso justo é o seu prazer.”
//! O recibo do Argon2id ([`conferir_pow`]) deixa o nó conferir a prova de
//! trabalho fora da trava, sem abrir uma porta: ele só vale para o cabeçalho
//! que foi conferido.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use hyurax_chain::{Chain, conferir_pow};
use hyurax_consensus::ParametrosRede;

#[test]
fn recibo_vale_so_para_o_cabecalho_conferido() {
    let p = ParametrosRede::REGTEST;
    let minerador = [7u8; 20];
    let cadeia = Chain::nova(p).unwrap();
    let ts = cadeia.tip().unwrap().header.timestamp + p.target_spacing;
    let bloco = cadeia.mine(minerador, vec![], Some(ts), 1).unwrap();
    let recibo = conferir_pow(&p, &bloco.header).unwrap();
    assert_eq!(*recibo.block_hash(), bloco.block_hash());

    // Com o recibo do próprio bloco, entra.
    let mut a = cadeia.clone();
    a.accept_block_com_pow(bloco.clone(), &recibo, Some(ts + 10)).unwrap();
    assert_eq!(a.tip_hash(), bloco.block_hash());

    // Outro cabeçalho com o recibo do primeiro: é conferido do zero, e o
    // resultado é exatamente o da conferência completa.
    let mut outro = bloco.clone();
    outro.header.nonce = outro.header.nonce.wrapping_add(1);
    let mut b = cadeia.clone();
    let mut c = cadeia.clone();
    let com_recibo_alheio = b.accept_block_com_pow(outro.clone(), &recibo, Some(ts + 10));
    let completo = c.accept_block(outro, Some(ts + 10));
    assert_eq!(com_recibo_alheio, completo);
    assert_eq!(b.tip_hash(), c.tip_hash());
}

#[test]
fn recibo_de_outra_rede_nao_vale() {
    // Os parâmetros do Argon2id da rede de teste diferem dos do regtest, e o
    // recibo carrega os parâmetros com que foi conferido.
    let regtest = ParametrosRede::REGTEST;
    let cadeia = Chain::nova(regtest).unwrap();
    let ts = cadeia.tip().unwrap().header.timestamp + regtest.target_spacing;
    let bloco = cadeia.mine([3u8; 20], vec![], Some(ts), 1).unwrap();
    assert_ne!(regtest.pow, ParametrosRede::TESTNET.pow);
    // Pelas regras da rede de teste, o bloco do regtest não passa.
    assert!(conferir_pow(&ParametrosRede::TESTNET, &bloco.header).is_err());
}

#[test]
fn versao_e_alvo_sao_conferidos_antes_do_argon2() {
    let p = ParametrosRede::REGTEST;
    let cadeia = Chain::nova(p).unwrap();
    let ts = cadeia.tip().unwrap().header.timestamp + p.target_spacing;
    let bloco = cadeia.mine([5u8; 20], vec![], Some(ts), 1).unwrap();
    let mut versao = bloco.header;
    versao.version = versao.version.wrapping_add(1);
    assert!(conferir_pow(&p, &versao).unwrap_err().0.contains("versão"));
    let mut facil = bloco.header;
    facil.bits = 0x2100_ffff; // muito acima do limite da rede
    assert!(conferir_pow(&p, &facil).is_err());
}
