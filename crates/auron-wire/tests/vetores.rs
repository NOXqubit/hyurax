// ✝ Provérbios 25:25 — “Como água fresca para a alma sedenta, tais são as boas novas de terra distante.”
//! Formato das mensagens da rede conferido contra o gabarito em Python.
//!
//! Cada quadro válido decodifica e recodifica byte a byte; cada quadro
//! inválido é recusado com a mesma mensagem.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_wire::{Message, WireError, decode_frame, encode_frame};
use serde_json::Value;

fn carregar() -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "wire.json"]
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
fn quadros_validos_decodificam_e_recodificam() {
    let doc = carregar();
    let magic: [u8; 4] = bytes(&doc["magic"]).try_into().unwrap();
    let casos = doc["valid"].as_array().unwrap();
    for caso in casos {
        let nome = caso["name"].as_str().unwrap();
        let quadro = bytes(&caso["frame"]);
        let lido = decode_frame(&magic, &quadro).unwrap_or_else(|e| panic!("{nome}: {e}"));
        assert!(lido.resto.is_empty(), "{nome}: sobrou byte");

        // corpo bate com o que o vetor gravou
        assert_eq!(lido.message.corpo().unwrap(), bytes(&caso["body"]), "{nome}: corpo");

        // tipo desconhecido é guardado para ignorar, não recusado
        if caso["type"].as_u64().unwrap() > 11 {
            assert!(matches!(lido.message, Message::Desconhecida { .. }), "{nome}");
        }

        // recodificar dá exatamente o mesmo quadro
        assert_eq!(encode_frame(&magic, &lido.message).unwrap(), quadro, "{nome}: recodifica");
    }
    assert!(casos.len() >= 12);
}

#[test]
fn quadros_invalidos_recusados_pelo_mesmo_motivo() {
    let doc = carregar();
    let magic: [u8; 4] = bytes(&doc["magic"]).try_into().unwrap();
    let casos = doc["invalid"].as_array().unwrap();
    for caso in casos {
        let nome = caso["name"].as_str().unwrap();
        let erro = decode_frame(&magic, &bytes(&caso["frame"])).unwrap_err();
        assert_eq!(erro.to_string(), caso["error"].as_str().unwrap(), "{nome}");
    }
    assert!(casos.len() >= 5);
}

#[test]
fn dois_quadros_grudados_saem_um_a_um() {
    let doc = carregar();
    let magic: [u8; 4] = bytes(&doc["magic"]).try_into().unwrap();
    let valid = doc["valid"].as_array().unwrap();
    let mut fluxo = bytes(&valid[0]["frame"]);
    fluxo.extend(bytes(&valid[9]["frame"])); // hello + ping

    let primeiro = decode_frame(&magic, &fluxo).unwrap();
    assert!(matches!(primeiro.message, Message::Hello(_)));
    let segundo = decode_frame(&magic, primeiro.resto).unwrap();
    assert!(matches!(segundo.message, Message::Ping(_)));
    assert!(segundo.resto.is_empty());
}

#[test]
fn quadro_pela_metade_pede_mais_bytes() {
    let doc = carregar();
    let magic: [u8; 4] = bytes(&doc["magic"]).try_into().unwrap();
    let quadro = bytes(&doc["valid"][3]["frame"]); // headers, corpo grande
    for corte in [0, 5, 12, quadro.len() - 1] {
        assert_eq!(decode_frame(&magic, &quadro[..corte]).unwrap_err(), WireError::QuadroIncompleto, "corte {corte}");
    }
    // inteiro, funciona
    assert!(decode_frame(&magic, &quadro).is_ok());
}
