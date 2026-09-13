// ✝ Provérbios 11:1 — “Balança enganosa é abominação para o Senhor, mas o peso justo é o seu prazer.”
//! Trabalho útil conferido contra o gabarito em Python.
//!
//! - `usefulpow.json`: regra do tamanho nas três redes, semente, prova,
//!   compromisso, e as recusas com o motivo exato.
//! - `utrax.json`: o gerador de matrizes é o mesmo do UTRAX; o produto
//!   calculado aqui precisa bater com o `matrix_work` do Python.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_usefulpow::{
    ParametrosUteis, UsefulWorkProof, generate_matrices, solve, task_seed, useful_work_size, verify,
};
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

fn alvo(v: &Value) -> [u8; 32] {
    let texto = v.as_str().unwrap().trim_start_matches("0x");
    hex::decode(format!("{texto:0>64}")).unwrap().try_into().unwrap()
}

fn rede(v: &Value) -> ParametrosUteis {
    ParametrosUteis::da_rede(v.as_str().unwrap()).unwrap()
}

#[test]
fn tamanho_exigido_igual_nas_tres_redes() {
    let doc = carregar("usefulpow.json");
    let casos = doc["sizes"].as_array().unwrap();
    for caso in casos {
        let n = useful_work_size(&alvo(&caso["target"]), &rede(&caso["network"]));
        assert_eq!(u64::from(n), caso["n"].as_u64().unwrap(), "{caso}");
    }
    assert_eq!(casos.len(), 39);
}

#[test]
fn prova_honesta_igual_byte_a_byte() {
    let doc = carregar("usefulpow.json");
    for caso in doc["proofs"].as_array().unwrap() {
        let p = rede(&caso["network"]);
        let altura = caso["height"].as_u64().unwrap();
        let prev = bytes(&caso["prev_hash"]);
        let minerador = bytes(&caso["miner"]);
        let t = alvo(&caso["target"]);

        assert_eq!(task_seed(&p, altura, &prev, &minerador).unwrap().to_vec(), bytes(&caso["seed"]));

        let feita = solve(&p, altura, &prev, &minerador, &t).unwrap();
        assert_eq!(u64::from(feita.n), caso["n"].as_u64().unwrap());
        assert_eq!(feita.encode().unwrap(), bytes(&caso["encoded"]));
        assert_eq!(feita.commitment().unwrap().to_vec(), bytes(&caso["commitment"]));

        let lida = UsefulWorkProof::decode(&bytes(&caso["encoded"])).unwrap();
        assert_eq!(lida, feita);
        assert_eq!(verify(&lida, &p, altura, &prev, &minerador, &t), Ok(()));
    }
}

#[test]
fn recusas_pelo_mesmo_motivo_do_gabarito() {
    let doc = carregar("usefulpow.json");
    let casos = doc["rejections"].as_array().unwrap();
    for caso in casos {
        let prova = UsefulWorkProof::decode(&bytes(&caso["encoded"])).unwrap();
        let motivo = verify(
            &prova,
            &ParametrosUteis::REGTEST,
            caso["height"].as_u64().unwrap(),
            &bytes(&caso["prev_hash"]),
            &bytes(&caso["miner"]),
            &alvo(&caso["target"]),
        );
        assert_eq!(motivo, Err(caso["reason"].as_str().unwrap().to_string()), "{}", caso["case"]);
    }
    assert_eq!(casos.len(), 6);
}

#[test]
fn gerador_de_matrizes_igual_ao_utrax() {
    let doc = carregar("utrax.json");
    for caso in doc["matrix"].as_array().unwrap() {
        let n = caso["size"].as_u64().unwrap() as u32;
        let (a, b) = generate_matrices(&bytes(&caso["seed"]), n).unwrap();
        let n = n as usize;
        // o Python grava C como int64 little-endian (numpy)
        let mut esperado = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let soma: i64 = (0..n).map(|k| i64::from(a[i * n + k]) * i64::from(b[k * n + j])).sum();
                esperado.extend(soma.to_le_bytes());
            }
        }
        assert_eq!(esperado, bytes(&caso["result"]), "n = {n}");
    }
}

#[test]
fn entradas_de_tamanho_errado_sao_recusadas() {
    let p = ParametrosUteis::REGTEST;
    assert!(task_seed(&p, 1, &[0u8; 63], &[0u8; 20]).is_err());
    assert!(task_seed(&p, 1, &[0u8; 64], &[0u8; 19]).is_err());
    assert!(generate_matrices(&[0u8; 64], 0).is_err());
    assert!(generate_matrices(&[0u8; 64], 1025).is_err());
    // prova com byte sobrando no fim não decodifica
    let mut sobra = solve(&p, 1, &[0u8; 64], &[0u8; 20], &p.max_target).unwrap().encode().unwrap();
    sobra.push(0);
    assert!(UsefulWorkProof::decode(&sobra).is_err());
}
