// ✝ Deuteronômio 25:15 — “Peso inteiro e justo terás; medida inteira e justa terás.”
//! Validação cruzada da prova de trabalho contra o gabarito em Python.
//!
//! - `argon2.json`: vetores oficiais da RFC 9106 (seção 5) e casos do regtest.
//! - `genesis.json`: `pow_hash` das três gêneses, com 32 MiB na mainnet.
//! - `chain.json`: cada bloco minerado pelo Python; o minerador em Rust, com uma
//!   linha a partir do nonce 0, precisa achar exatamente o mesmo nonce.
//! - `targets.json`: `bits` desempacotado igual ao alvo canônico do Python.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::Duration;

use argon2::{Algorithm, Argon2, AssociatedData, ParamsBuilder, Version};
use auron_pow::{
    Calculadora, ConfigMineracao, ParametrosPow, alvo_de_bits, bate_alvo, bits_do_cabecalho,
    minerar,
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

fn hash32(v: &Value) -> [u8; 32] {
    bytes(v).try_into().unwrap()
}

#[test]
fn argon2_bate_com_rfc_9106_e_regtest() {
    let casos = carregar("argon2.json");
    let casos = casos.as_array().unwrap();
    for caso in casos {
        let algoritmo = match caso["variant"].as_str().unwrap() {
            "argon2d" => Algorithm::Argon2d,
            "argon2i" => Algorithm::Argon2i,
            "argon2id" => Algorithm::Argon2id,
            outro => panic!("variante desconhecida: {outro}"),
        };
        let mut construtor = ParamsBuilder::new();
        construtor
            .m_cost(caso["m_kib"].as_u64().unwrap() as u32)
            .t_cost(caso["t"].as_u64().unwrap() as u32)
            .p_cost(caso["p"].as_u64().unwrap() as u32)
            .output_len(caso["tag_len"].as_u64().unwrap() as usize);
        if !caso["associated_data"].is_null() {
            construtor.data(AssociatedData::new(&bytes(&caso["associated_data"])).unwrap());
        }
        let params = construtor.build().unwrap();
        let segredo = if caso["secret"].is_null() { vec![] } else { bytes(&caso["secret"]) };
        let argon = if segredo.is_empty() {
            Argon2::new(algoritmo, Version::V0x13, params)
        } else {
            Argon2::new_with_secret(&segredo, algoritmo, Version::V0x13, params).unwrap()
        };
        let mut saida = vec![0u8; caso["tag_len"].as_u64().unwrap() as usize];
        argon
            .hash_password_into_with_memory(
                &bytes(&caso["password"]),
                &bytes(&caso["salt"]),
                &mut saida,
                vec![argon2::Block::new(); argon.params().block_count()],
            )
            .unwrap();
        assert_eq!(saida, bytes(&caso["tag"]), "caso {} diverge", caso["source"]);
    }
    assert_eq!(casos.len(), 5);
}

#[test]
fn pow_hash_das_geneses_com_os_parametros_de_cada_rede() {
    let geneses = carregar("genesis.json");
    for g in geneses.as_array().unwrap() {
        let rede = g["network"].as_str().unwrap();
        let p = ParametrosPow::da_rede(rede).unwrap();
        assert_eq!(u64::from(p.memoria_kib), g["pow_memory_kib"].as_u64().unwrap(), "{rede}");
        let cabecalho = bytes(&g["header_bytes"]);
        let mut calc = Calculadora::nova(p).unwrap();
        assert_eq!(calc.pow_hash(&cabecalho).unwrap(), hash32(&g["pow_hash"]), "{rede}");
        // reaproveitar a memória não pode mudar o resultado
        assert_eq!(calc.pow_hash(&cabecalho).unwrap(), hash32(&g["pow_hash"]), "{rede} 2ª vez");
    }
}

#[test]
fn minerador_acha_o_mesmo_nonce_que_o_gabarito() {
    let cadeia = carregar("chain.json");
    let p = ParametrosPow::da_rede(cadeia["network"].as_str().unwrap()).unwrap();
    let blocos = cadeia["blocks"].as_array().unwrap();
    assert!(blocos.len() >= 4);
    for bloco in blocos {
        let cabecalho = bytes(&bloco["header_bytes"]);
        let bits = bits_do_cabecalho(&cabecalho).unwrap();
        assert_eq!(format!("{bits:#010x}"), bloco["bits"].as_str().unwrap());

        let mut calc = Calculadora::nova(p).unwrap();
        let hash = calc.pow_hash(&cabecalho).unwrap();
        assert_eq!(hash, hash32(&bloco["pow_hash"]));
        assert!(bate_alvo(&hash, &alvo_de_bits(bits).unwrap()));

        let config = ConfigMineracao {
            linhas: 1,
            nonce_inicial: 0,
            limite: Some(1 << 20),
            pausa: Duration::ZERO,
        };
        let r = minerar(&cabecalho, p, config, &AtomicBool::new(false), &AtomicU64::new(0)).unwrap();
        let achado = r.achado.expect("não achou nonce");
        assert_eq!(achado.nonce, bloco["nonce"].as_u64().unwrap(), "altura {}", bloco["height"]);
        assert_eq!(achado.hash, hash);
        assert_eq!(r.tentativas, achado.nonce + 1);
    }
}

#[test]
fn varias_linhas_acham_um_nonce_valido() {
    let cadeia = carregar("chain.json");
    let p = ParametrosPow::REGTEST;
    let cabecalho = bytes(&cadeia["blocks"][0]["header_bytes"]);
    let alvo = alvo_de_bits(bits_do_cabecalho(&cabecalho).unwrap()).unwrap();
    let config = ConfigMineracao { linhas: 4, nonce_inicial: 0, limite: Some(1 << 20), pausa: Duration::ZERO };
    let r = minerar(&cabecalho, p, config, &AtomicBool::new(false), &AtomicU64::new(0)).unwrap();
    let achado = r.achado.unwrap();
    let mut c = cabecalho.clone();
    auron_pow::com_nonce(&mut c, achado.nonce).unwrap();
    let mut calc = Calculadora::nova(p).unwrap();
    assert_eq!(calc.pow_hash(&c).unwrap(), achado.hash);
    assert!(bate_alvo(&achado.hash, &alvo));
}

#[test]
fn alvo_compacto_bate_com_o_gabarito() {
    let alvos = carregar("targets.json");
    let casos = alvos["compact"].as_array().unwrap();
    for caso in casos {
        let bits = u32::from_str_radix(caso["bits"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
        let canonico = caso["canonical"].as_str().unwrap().trim_start_matches("0x");
        let esperado = format!("{:0>64}", canonico);
        assert_eq!(hex::encode(alvo_de_bits(bits).unwrap()), esperado, "bits {bits:#010x}");
    }
    assert!(casos.len() >= 5);
}
