// ✝ Provérbios 16:11 — “O peso e a balança justos são do Senhor; obra sua são todos os pesos da bolsa.”
//! Transações conferidas contra o gabarito em Python.
//!
//! - `transactions.json`: assinar, codificar e calcular o txid, byte a byte.
//! - `transactions_edge.json`: o que a leitura recusa e o que a conferência de
//!   assinatura recusa, com a mensagem exata.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_tx::{Coinbase, Output, Tx, decode_tx, sign_transfer_outputs};
use serde_json::Value;

const SEED_A: [u8; 32] = [0x11; 32];
const MAGIC_REGTEST: [u8; 4] = *b"AURR";

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

fn fixo<const N: usize>(v: &Value) -> [u8; N] {
    bytes(v).try_into().unwrap()
}

fn numero(v: &Value) -> u64 {
    v.as_str().map_or_else(|| v.as_u64().unwrap(), |s| s.parse().unwrap())
}

#[test]
fn transferencias_iguais_byte_a_byte() {
    let doc = carregar("transactions.json");
    let casos = doc["transfers"].as_array().unwrap();
    for caso in casos {
        assert_eq!(caso["network"].as_str(), Some("auron-regtest"));
        let saidas: Vec<Output> = caso["outputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| Output {
                recipient: fixo(&o["recipient"]),
                asset_id: fixo(&o["asset_id"]),
                amount: numero(&o["amount"]),
            })
            .collect();
        let t = sign_transfer_outputs(
            &SEED_A,
            &MAGIC_REGTEST,
            fixo(&caso["sender"]),
            saidas,
            numero(&caso["fee"]),
            numero(&caso["nonce"]),
        )
        .unwrap();
        assert_eq!(t.signing_payload(&MAGIC_REGTEST).unwrap(), bytes(&caso["signing_payload"]));
        assert_eq!(t.signature, bytes(&caso["signature"]));
        assert_eq!(t.encode().unwrap(), bytes(&caso["encoded"]));
        assert_eq!(t.txid().unwrap().to_vec(), bytes(&caso["txid"]));
        assert_eq!(t.check_signature(&MAGIC_REGTEST), Ok(()));
        assert_eq!(decode_tx(&bytes(&caso["encoded"])).unwrap(), Tx::Transfer(t));
    }
    assert!(casos.len() >= 4);
}

#[test]
fn coinbases_iguais_byte_a_byte() {
    let doc = carregar("transactions.json");
    for caso in doc["coinbases"].as_array().unwrap() {
        let c = Coinbase {
            height: numero(&caso["height"]),
            recipient: fixo(&caso["recipient"]),
            amount: numero(&caso["amount"]),
            extra_nonce: bytes(&caso["extra_nonce"]),
        };
        assert_eq!(c.encode().unwrap(), bytes(&caso["encoded"]));
        assert_eq!(c.txid().unwrap().to_vec(), bytes(&caso["txid"]));
        assert_eq!(decode_tx(&bytes(&caso["encoded"])).unwrap(), Tx::Coinbase(c));
    }
}

#[test]
fn bordas_com_o_mesmo_veredito_e_motivo() {
    let casos = carregar("transactions_edge.json");
    let casos = casos.as_array().unwrap();
    let (mut recusas_leitura, mut recusas_conferencia) = (0, 0);
    for caso in casos {
        let rotulo = caso["label"].as_str().unwrap();
        let magic: [u8; 4] = fixo(&caso["magic"]);
        let dados = bytes(&caso["encoded"]);
        match decode_tx(&dados) {
            Err(e) => {
                assert_eq!(Some(e.to_string().as_str()), caso["decode_error"].as_str(), "{rotulo}");
                recusas_leitura += 1;
            }
            Ok(tx) => {
                assert!(caso["decode_error"].is_null(), "{rotulo}: o gabarito recusou a leitura");
                assert_eq!(tx.txid().unwrap().to_vec(), bytes(&caso["txid"]), "{rotulo}");
                // relê e recodifica: a leitura aceita só a forma canônica
                assert_eq!(tx.encode().unwrap(), dados, "{rotulo}");
                match tx {
                    Tx::Coinbase(_) => assert_eq!(caso["kind"].as_str(), Some("coinbase"), "{rotulo}"),
                    Tx::Transfer(t) => {
                        assert_eq!(caso["kind"].as_str(), Some("transfer"), "{rotulo}");
                        let esperado = caso["check"].as_str().unwrap();
                        match t.check_signature(&magic) {
                            Ok(()) => assert_eq!(esperado, "ok", "{rotulo}"),
                            Err(motivo) => {
                                assert_eq!(motivo, esperado, "{rotulo}");
                                recusas_conferencia += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(recusas_leitura >= 10, "poucas recusas de leitura: {recusas_leitura}");
    assert!(recusas_conferencia >= 10, "poucas recusas de conferência: {recusas_conferencia}");
}

#[test]
fn escrita_recusa_o_que_a_leitura_recusa() {
    let grande = Coinbase { height: 1, recipient: [1; 20], amount: 1, extra_nonce: vec![0; 65] };
    assert_eq!(grande.encode().unwrap_err().to_string(), "extra_nonce longo demais");

    let conta = auron_crypto::address_from_ed25519_pubkey(&auron_crypto::ed25519_public_key(&SEED_A));
    let vazia = sign_transfer_outputs(&SEED_A, &MAGIC_REGTEST, conta, vec![], 0, 0);
    assert_eq!(vazia.unwrap_err().to_string(), "transferência precisa de 1 a 16 saídas, tem 0");

    let outro = sign_transfer_outputs(&[0x22; 32], &MAGIC_REGTEST, conta, vec![], 0, 0);
    assert_eq!(outro.unwrap_err().to_string(), "segredo não corresponde ao endereço de origem");
}
