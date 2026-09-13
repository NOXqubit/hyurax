// ✝ Habacuque 2:2 — “Escreve a visão, torna-a bem legível sobre tábuas.”
//! Persistência conferida contra o arquivo gravado pelo gabarito em Python.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_chain::Chain;
use auron_consensus::ParametrosRede;
use auron_store::{decode_chain, encode_chain, load_chain, save_chain};
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

#[test]
fn le_o_arquivo_do_python_e_grava_os_mesmos_bytes() {
    let doc = carregar("chain.json");
    let arquivo = bytes(&doc["store_file"]);
    for confiar in [false, true] {
        let cadeia = decode_chain(&arquivo, confiar, None).unwrap();
        assert_eq!(cadeia.height(), doc["final_state"]["height"].as_u64().unwrap());
        assert_eq!(cadeia.tip_hash().to_vec(), bytes(&doc["final_state"]["tip_hash"]));
        assert_eq!(encode_chain(&cadeia).unwrap(), arquivo, "confiar = {confiar}");
    }
}

#[test]
fn arquivo_adulterado_recusado_pelo_mesmo_motivo() {
    let doc = carregar("chain.json");
    let casos = doc["store_edge"].as_array().unwrap();
    for caso in casos {
        let rotulo = caso["label"].as_str().unwrap();
        let motivo = decode_chain(&bytes(&caso["file"]), false, None).unwrap_err().to_string();
        assert_eq!(motivo, caso["error"].as_str().unwrap(), "{rotulo}");
    }
    assert!(casos.len() >= 7);
}

#[test]
fn gravar_e_recarregar_do_disco() {
    let p = ParametrosRede::REGTEST;
    let mut cadeia = Chain::nova(p).unwrap();
    for _ in 0..3 {
        let ts = cadeia.tip().unwrap().header.timestamp + p.target_spacing;
        let bloco = cadeia.mine([4u8; 20], vec![], Some(ts), 2).unwrap();
        cadeia.accept_block(bloco, Some(ts + 10)).unwrap();
    }
    let pasta = std::env::temp_dir().join(format!("auron-store-teste-{}", std::process::id()));
    std::fs::create_dir_all(&pasta).unwrap();
    let arquivo = pasta.join("cadeia.bin");
    assert_eq!(save_chain(&cadeia, &arquivo).unwrap(), 3);
    let relida = load_chain(&arquivo, false, Some(p)).unwrap();
    assert_eq!(relida.tip_hash(), cadeia.tip_hash());
    assert_eq!(relida.state, cadeia.state);

    let errada = load_chain(&arquivo, false, Some(ParametrosRede::TESTNET)).unwrap_err();
    assert_eq!(errada.to_string(), "arquivo é da rede auron-regtest, mas foi pedida auron-testnet");
    std::fs::remove_dir_all(&pasta).unwrap();
    assert!(load_chain(&arquivo, false, None).unwrap_err().to_string().starts_with("arquivo não encontrado"));
}
