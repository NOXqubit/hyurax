// ✝ Jó 38:5 — “Quem lhe pôs as medidas, se é que o sabes? Ou quem estendeu sobre ela o cordel?”
//! Regras numéricas conferidas contra o gabarito em Python.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_consensus::{
    Alvo, ParametrosRede, bits_de_alvo, block_reward, cumulative_emission, median_time_past,
    next_target, normalizar_alvo, target_to_work,
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

fn alvo(v: &Value) -> Alvo {
    let texto = v.as_str().unwrap().trim_start_matches("0x");
    let mut saida = [0u8; 32];
    let cheio = format!("{texto:0>64}");
    for (i, byte) in saida.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&cheio[2 * i..2 * i + 2], 16).unwrap();
    }
    saida
}

fn numero(v: &Value) -> u64 {
    v.as_str().map_or_else(|| v.as_u64().unwrap(), |s| s.parse().unwrap())
}

#[test]
fn forma_compacta_normalizacao_e_trabalho() {
    let doc = carregar("targets.json");
    let casos = doc["compact"].as_array().unwrap();
    for caso in casos {
        let entrada = alvo(&caso["target_in"]);
        let bits = u32::from_str_radix(caso["bits"].as_str().unwrap().trim_start_matches("0x"), 16).unwrap();
        assert_eq!(bits_de_alvo(&entrada), bits, "{caso}");
        let canonico = normalizar_alvo(&entrada).unwrap();
        assert_eq!(canonico, alvo(&caso["canonical"]), "{caso}");
        assert_eq!(
            target_to_work(&canonico).unwrap().to_decimal(),
            caso["work"].as_str().unwrap(),
            "{caso}"
        );
    }
    assert_eq!(casos.len(), 8);
}

#[test]
fn lwma_igual_nos_seis_cenarios() {
    let doc = carregar("targets.json");
    let casos = doc["lwma"].as_array().unwrap();
    for caso in casos {
        let p = ParametrosRede::da_rede(caso["network"].as_str().unwrap()).unwrap();
        let ts: Vec<u64> = caso["timestamps"].as_array().unwrap().iter().map(numero).collect();
        let alvos: Vec<Alvo> = caso["targets_in"].as_array().unwrap().iter().map(alvo).collect();
        assert_eq!(
            next_target(&ts, &alvos, &p).unwrap(),
            alvo(&caso["next_target"]),
            "cenário {}",
            caso["name"]
        );
    }
    assert_eq!(casos.len(), 6);
}

#[test]
fn emissao_igual_ao_gabarito() {
    let doc = carregar("emission.json");
    for caso in doc.as_array().unwrap() {
        let p = ParametrosRede::da_rede(caso["network"].as_str().unwrap()).unwrap();
        if let Some(total) = caso.get("total_emission") {
            assert_eq!(p.total_emission(), numero(total));
            continue;
        }
        let altura = numero(&caso["height"]);
        assert_eq!(block_reward(altura, &p), numero(&caso["reward"]), "altura {altura}");
        assert_eq!(cumulative_emission(altura, &p), numero(&caso["cumulative"]), "altura {altura}");
    }
}

#[test]
fn casos_pequenos_do_retarget_e_da_mediana() {
    let p = ParametrosRede::REGTEST;
    // sem histórico: alvo máximo; um bloco só: o alvo dele, limitado ao máximo
    assert_eq!(next_target(&[], &[], &p).unwrap(), p.max_target);
    assert_eq!(next_target(&[1], &[[0xff; 32]], &p).unwrap(), p.max_target);
    assert!(next_target(&[1, 2], &[p.max_target], &p).is_err());

    assert_eq!(median_time_past(&[], &p), 0);
    assert_eq!(median_time_past(&[5, 1, 3], &p), 3);
    // só os últimos 11 contam
    let muitos: Vec<u64> = (0..30).collect();
    assert_eq!(median_time_past(&muitos, &p), 24);

    assert!(target_to_work(&[0u8; 32]).is_err());
    assert_eq!(target_to_work(&[0xff; 32]).unwrap().to_decimal(), "1");
}

#[test]
fn geneses_usam_o_alvo_maximo_da_rede() {
    let geneses = carregar("genesis.json");
    for g in geneses.as_array().unwrap() {
        let p = ParametrosRede::da_rede(g["network"].as_str().unwrap()).unwrap();
        let cabecalho = hex_bytes(g["header_bytes"].as_str().unwrap());
        let bits = u32::from_be_bytes(cabecalho[210..214].try_into().unwrap());
        assert_eq!(bits, bits_de_alvo(&p.max_target), "{}", p.nome);
        assert_eq!(normalizar_alvo(&p.max_target).unwrap(), p.max_target, "{}", p.nome);
    }
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
