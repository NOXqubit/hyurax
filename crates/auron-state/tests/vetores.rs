// ✝ Mateus 25:21 — “Bem está, servo bom e fiel. Sobre o pouco foste fiel, sobre muito te colocarei.”
//! Estado de contas conferido passo a passo contra o gabarito em Python.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_consensus::ParametrosRede;
use auron_state::{State, Undo};
use auron_tx::decode_tx;
use serde_json::{Value, json};

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

/// A mesma fotografia que o gerador grava.
fn foto(e: &State) -> Value {
    json!({
        "balances": e.balances.iter()
            .map(|((a, ativo), v)| json!([hex::encode(a), hex::encode(ativo), v.to_string()]))
            .collect::<Vec<_>>(),
        "nonces": e.nonces.iter().map(|(a, v)| json!([hex::encode(a), v])).collect::<Vec<_>>(),
        "pending_coinbase": e.pending_coinbase.iter()
            .map(|(h, entradas)| json!([h, entradas.iter()
                .map(|(a, v)| json!([hex::encode(a), v.to_string()]))
                .collect::<Vec<_>>()]))
            .collect::<Vec<_>>(),
        "total_emitted": e.total_emitted.to_string(),
    })
}

#[test]
fn sequencia_de_blocos_igual_ao_gabarito() {
    let doc = carregar("state.json");
    let p = ParametrosRede::da_rede(doc["network"].as_str().unwrap()).unwrap();
    let mut estado = State::novo(p);
    let mut desfazer: Vec<Undo> = Vec::new();
    let (mut aceitos, mut recusados, mut desfeitos) = (0, 0, 0);

    for passo in doc["steps"].as_array().unwrap() {
        let nome = passo["name"].as_str().unwrap();
        match passo["op"].as_str().unwrap() {
            "apply" => {
                let txs: Vec<_> = passo["transactions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| decode_tx(&hex::decode(t.as_str().unwrap()).unwrap()).unwrap())
                    .collect();
                let antes = estado.clone();
                let resultado = estado.apply_block(passo["height"].as_u64().unwrap(), &txs, &p.magic);
                match resultado {
                    Ok(undo) => {
                        assert_eq!(passo["result"].as_str(), Some("ok"), "{nome}");
                        estado.check_invariants().unwrap();
                        desfazer.push(undo);
                        aceitos += 1;
                    }
                    Err(e) => {
                        assert_eq!(Some(e.to_string().as_str()), passo["result"].as_str(), "{nome}");
                        assert_eq!(estado, antes, "{nome}: bloco recusado mexeu no estado");
                        recusados += 1;
                    }
                }
            }
            "revert" => {
                estado.revert_block(&desfazer.pop().unwrap());
                desfeitos += 1;
            }
            outro => panic!("operação desconhecida: {outro}"),
        }
        assert_eq!(foto(&estado), passo["state"], "{nome}: fotografia do estado diverge");
    }
    assert!(aceitos >= 6 && recusados >= 9 && desfeitos >= 2, "{aceitos} {recusados} {desfeitos}");
}
