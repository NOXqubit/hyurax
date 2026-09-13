// ✝ Neemias 4:6 — “Assim edificamos o muro, e todo o muro se fechou até a sua metade; porque o povo tinha ânimo para trabalhar.”
//! Blocos conferidos contra o gabarito em Python.
//!
//! Para cada gênese e cada bloco minerado em `chain.json`: ler e reescrever os
//! mesmos bytes, `block_hash`, raiz de Merkle, compromisso da prova útil, a
//! própria prova útil conferida, e o Argon2id batendo o alvo. É o bloco inteiro
//! conferido em Rust, só com o que já foi migrado.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_block::{BLOCK_VERSION, Block, BlockHeader, HEADER_LEN};
use auron_pow::{Calculadora, ParametrosPow, bate_alvo};
use auron_tx::Tx;
use auron_usefulpow::{ParametrosUteis, verify};
use serde_json::Value;

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

/// Tudo o que dá para conferir de um bloco sem estado nem cadeia.
fn conferir_bloco(rede: &str, cabecalho_hex: &Value, bloco_hex: &Value, hash_hex: &Value) -> Block {
    let bruto = bytes(bloco_hex);
    let bloco = Block::decode(&bruto).unwrap();
    assert_eq!(bloco.encode().unwrap(), bruto, "{rede}: reescrita diverge");

    let cab = bytes(cabecalho_hex);
    assert_eq!(cab.len(), HEADER_LEN);
    assert_eq!(BlockHeader::decode(&cab).unwrap(), bloco.header);
    assert_eq!(bloco.header.encode().to_vec(), cab);
    assert_eq!(bloco.header.version, BLOCK_VERSION);
    assert_eq!(bloco.block_hash().to_vec(), bytes(hash_hex), "{rede}: block_hash");

    assert_eq!(bloco.computed_merkle_root().unwrap(), bloco.header.merkle_root, "{rede}: merkle");
    let prova = bloco.useful_proof.as_ref().expect("bloco sem prova útil");
    assert_eq!(prova.commitment().unwrap(), bloco.header.useful_root, "{rede}: useful_root");

    let coinbase = bloco.coinbase().unwrap();
    let alvo = bloco.header.target().unwrap();
    let p = ParametrosUteis::da_rede(rede).unwrap();
    assert_eq!(
        verify(prova, &p, bloco.header.height, &bloco.header.prev_hash, &coinbase.recipient, &alvo),
        Ok(()),
        "{rede}: prova útil"
    );
    bloco
}

#[test]
fn geneses_das_tres_redes() {
    let geneses = carregar("genesis.json");
    for g in geneses.as_array().unwrap() {
        let rede = g["network"].as_str().unwrap();
        let bloco = conferir_bloco(rede, &g["header_bytes"], &g["block_bytes"], &g["block_hash"]);
        assert_eq!(bloco.header.height, 0);
        assert_eq!(bloco.header.prev_hash, [0u8; 64]);
        assert_eq!(bloco.header.merkle_root.to_vec(), bytes(&g["merkle_root"]));
        assert_eq!(bloco.transactions.len(), 1);
        // o Argon2id da gênese não precisa bater o alvo (nonce 0, congelado),
        // mas o pow_hash precisa ser o mesmo do gabarito
        let mut calc = Calculadora::nova(ParametrosPow::da_rede(rede).unwrap()).unwrap();
        assert_eq!(calc.pow_hash(&bloco.header.encode()).unwrap().to_vec(), bytes(&g["pow_hash"]));
    }
}

#[test]
fn blocos_minerados_pelo_gabarito() {
    let cadeia = carregar("chain.json");
    let rede = cadeia["network"].as_str().unwrap();
    let mut calc = Calculadora::nova(ParametrosPow::da_rede(rede).unwrap()).unwrap();
    let blocos = cadeia["blocks"].as_array().unwrap();
    let mut anterior: Option<[u8; 64]> = None;
    for b in blocos {
        let bloco = conferir_bloco(rede, &b["header_bytes"], &b["block_bytes"], &b["block_hash"]);
        assert_eq!(bloco.header.height, b["height"].as_u64().unwrap());
        assert_eq!(bloco.header.nonce, b["nonce"].as_u64().unwrap());
        if let Some(h) = anterior {
            assert_eq!(bloco.header.prev_hash, h, "altura {}: não liga no anterior", bloco.header.height);
        }
        anterior = Some(bloco.block_hash());

        let hash = calc.pow_hash(&bloco.header.encode()).unwrap();
        assert_eq!(hash.to_vec(), bytes(&b["pow_hash"]));
        assert!(bate_alvo(&hash, &bloco.header.target().unwrap()));
        assert!(matches!(bloco.transactions.first(), Some(Tx::Coinbase(_))));
    }
    assert!(blocos.len() >= 4);
}

#[test]
fn bloco_malformado_nao_decodifica() {
    let cadeia = carregar("chain.json");
    let bruto = bytes(&cadeia["blocks"][0]["block_bytes"]);

    let mut sobra = bruto.clone();
    sobra.push(0);
    assert_eq!(Block::decode(&sobra).unwrap_err().to_string(), "1 bytes não consumidos no fim");

    let truncado = &bruto[..bruto.len() - 1];
    assert!(Block::decode(truncado).unwrap_err().to_string().starts_with("fim inesperado"));

    // sem prova: decodifica, e a validação é que recusa
    let bloco = Block::decode(&bruto).unwrap();
    let sem_prova = Block { useful_proof: None, ..bloco };
    let relido = Block::decode(&sem_prova.encode().unwrap()).unwrap();
    assert_eq!(relido.useful_proof, None);
}
