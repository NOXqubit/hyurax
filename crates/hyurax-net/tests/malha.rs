// ✝ Eclesiastes 4:12 — “O cordão de três dobras não se quebra tão depressa.”
//! As mensagens da malha conferidas byte a byte contra `vectors/malha.json`,
//! gerado por `reference/hyurax/malha.py`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use hyurax_net::malha::{AnuncioVizinho, MensagemMalha, PORTA_VIZINHOS, Pacote, Prefixo, TIPO_MALHA};
use serde_json::Value;

fn carregar(nome: &str) -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", nome].iter().collect();
    let bruto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("não consegui ler {}: {e}", caminho.display()));
    let doc: Value = serde_json::from_str(&bruto).unwrap();
    doc["data"].clone()
}

fn bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap()).unwrap()
}

#[test]
fn mensagens_vao_e_voltam_iguais_ao_gabarito() {
    let doc = carregar("malha.json");
    assert_eq!(u64::from(TIPO_MALHA), doc["tipo_malha"].as_u64().unwrap());
    assert_eq!(u64::from(PORTA_VIZINHOS), doc["porta_vizinhos"].as_u64().unwrap());
    for caso in doc["mensagens"].as_array().unwrap() {
        let corpo = bytes(&caso["corpo"]);
        let m = MensagemMalha::decodificar(&corpo).unwrap_or_else(|e| panic!("{}: {e}", caso["nome"]));
        assert_eq!(m.codificar().unwrap(), corpo, "{}", caso["nome"]);
    }
    for caso in doc["recusados"].as_array().unwrap() {
        assert!(MensagemMalha::decodificar(&bytes(&caso["corpo"])).is_err(), "{} foi aceito", caso["nome"]);
    }
}

#[test]
fn anuncio_e_prefixos_iguais_ao_gabarito() {
    let doc = carregar("malha.json");
    let v = &doc["vizinho"];
    let a = AnuncioVizinho::ler(&bytes(&v["corpo"])).unwrap();
    assert_eq!(a.magic.to_vec(), bytes(&v["magic"]));
    assert_eq!(u64::from(a.porta), v["porta"].as_u64().unwrap());
    assert_eq!(a.identidade.to_vec(), bytes(&v["identidade"]));
    assert_eq!(a.bytes(), bytes(&v["corpo"]));
    for nome in ["verificar", "reserva", "circuito"] {
        let p = bytes(&doc["prefixos"][nome]);
        assert_eq!(Prefixo::ler(&p).unwrap().bytes(), p, "{nome}");
    }
}

#[test]
fn pacote_igual_ao_gabarito() {
    let doc = carregar("malha.json");
    let p = &doc["pacote"];
    let pacote = Pacote::ler(&bytes(&p["corpo"])).unwrap();
    assert_eq!(&pacote.magic, b"HYXT");
    let esperados: Vec<Vec<u8>> = p["quadros"].as_array().unwrap().iter().map(bytes).collect();
    assert_eq!(pacote.quadros, esperados);
    assert_eq!(pacote.bytes().unwrap(), bytes(&p["corpo"]));
    assert!(Pacote::ler(&bytes(&p["recusado_sobra"])).is_err());
}
