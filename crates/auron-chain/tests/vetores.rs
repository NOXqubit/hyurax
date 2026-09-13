// ✝ Neemias 6:15 — “Acabou-se, pois, o muro aos vinte e cinco do mês de elul, em cinquenta e dois dias.”
//! A cadeia inteira conferida contra o gabarito em Python.
//!
//! - `genesis.json`: as três gêneses montadas em Rust, byte a byte.
//! - `chain.json`: minerar a mesma sequência de blocos dá os mesmos bytes, o
//!   mesmo trabalho acumulado e o mesmo estado final.
//! - `chain_edge.json`: cada bloco recusado pelo mesmo motivo, e a ponta igual
//!   depois de cada passo, rollback inclusive.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_block::Block;
use auron_chain::{Chain, compare_chains, make_genesis};
use auron_consensus::ParametrosRede;
use serde_json::Value;

const SEED_A: [u8; 32] = [0x11; 32];

fn carregar(nome: &str) -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", nome]
        .iter()
        .collect();
    let bruto = std::fs::read_to_string(&caminho)
        .unwrap_or_else(|e| panic!("não consegui ler {}: {e}", caminho.display()));
    let doc: Value = serde_json::from_str(&bruto).unwrap();
    assert_eq!(doc["spec"].as_str(), Some("AURON-SPEC-01"));
    doc["data"].clone()
}

fn bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap()).unwrap()
}

#[test]
fn geneses_montadas_em_rust() {
    let geneses = carregar("genesis.json");
    for g in geneses.as_array().unwrap() {
        let p = ParametrosRede::da_rede(g["network"].as_str().unwrap()).unwrap();
        let bloco = make_genesis(&p).unwrap();
        assert_eq!(bloco.encode().unwrap(), bytes(&g["block_bytes"]), "{}", p.nome);
        assert_eq!(bloco.block_hash().to_vec(), bytes(&g["block_hash"]), "{}", p.nome);
    }
}

#[test]
fn minerar_a_mesma_cadeia_do_gabarito() {
    let doc = carregar("chain.json");
    let p = ParametrosRede::da_rede(doc["network"].as_str().unwrap()).unwrap();
    let minerador = auron_crypto::address_from_ed25519_pubkey(&auron_crypto::ed25519_public_key(&SEED_A));
    let mut cadeia = Chain::nova(p).unwrap();
    for esperado in doc["blocks"].as_array().unwrap() {
        let ts = cadeia.tip().unwrap().header.timestamp + p.target_spacing;
        let bloco = cadeia.mine(minerador, vec![], Some(ts), 1).unwrap();
        assert_eq!(bloco.encode().unwrap(), bytes(&esperado["block_bytes"]), "altura {}", esperado["height"]);
        cadeia.accept_block(bloco, Some(ts + 10)).unwrap();
        assert_eq!(cadeia.total_work().to_decimal(), esperado["total_work_after"].as_str().unwrap());
    }
    let fim = &doc["final_state"];
    assert_eq!(cadeia.height(), fim["height"].as_u64().unwrap());
    assert_eq!(cadeia.tip_hash().to_vec(), bytes(&fim["tip_hash"]));
    assert_eq!(cadeia.state.total_emitted.to_string(), fim["total_emitted"].as_str().unwrap());
    let saldos: Vec<(String, String)> = cadeia
        .state
        .balances
        .iter()
        .map(|((a, ativo), v)| (format!("{}:{}", hex::encode(a), hex::encode(ativo)), v.to_string()))
        .collect();
    let esperados: Vec<(String, String)> = fim["balances"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect();
    assert_eq!(saldos, esperados);
}

#[test]
fn recusas_e_rollback_iguais_ao_gabarito() {
    let doc = carregar("chain_edge.json");
    let p = ParametrosRede::da_rede(doc["network"].as_str().unwrap()).unwrap();
    let mut cadeia = Chain::nova(p).unwrap();
    let (mut aceitos, mut recusados) = (0, 0);
    for passo in doc["steps"].as_array().unwrap() {
        let nome = passo["name"].as_str().unwrap();
        match passo["op"].as_str().unwrap() {
            "accept" => {
                let bloco = Block::decode(&bytes(&passo["block"])).unwrap();
                match cadeia.accept_block(bloco, Some(passo["now"].as_u64().unwrap())) {
                    Ok(()) => {
                        assert_eq!(passo["result"].as_str(), Some("ok"), "{nome}");
                        aceitos += 1;
                    }
                    Err(e) => {
                        assert_eq!(Some(e.to_string().as_str()), passo["result"].as_str(), "{nome}");
                        recusados += 1;
                    }
                }
            }
            "rollback" => {
                cadeia.rollback(passo["count"].as_u64().unwrap() as usize).unwrap();
            }
            outro => panic!("operação desconhecida: {outro}"),
        }
        let ponta = &passo["tip"];
        assert_eq!(cadeia.height(), ponta["height"].as_u64().unwrap(), "{nome}");
        assert_eq!(cadeia.tip_hash().to_vec(), bytes(&ponta["tip_hash"]), "{nome}");
        assert_eq!(cadeia.total_work().to_decimal(), ponta["total_work"].as_str().unwrap(), "{nome}");
    }
    assert!(aceitos >= 4 && recusados >= 15, "{aceitos} aceitos, {recusados} recusados");
    assert!(cadeia.rollback(cadeia.entries.len()).is_err(), "a gênese não pode sair");
}

#[test]
fn mais_trabalho_vence() {
    let p = ParametrosRede::REGTEST;
    let minerador = [9u8; 20];
    let curta = Chain::nova(p).unwrap();
    let mut longa = curta.clone();
    let ts = longa.tip().unwrap().header.timestamp + p.target_spacing;
    let bloco = longa.mine(minerador, vec![], Some(ts), 2).unwrap();
    longa.accept_block(bloco, Some(ts + 10)).unwrap();
    assert_eq!(compare_chains(&longa, &curta), 1);
    assert_eq!(compare_chains(&curta, &longa), -1);
    assert_eq!(compare_chains(&curta, &curta), 0);
}
