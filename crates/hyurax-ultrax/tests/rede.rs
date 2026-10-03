// ✝ Eclesiastes 4:9 — “Melhor é serem dois do que um, porque têm melhor paga do seu trabalho.”
//! As mensagens do ULTRAX entre nós conferidas byte a byte contra
//! `vectors/rede_ultrax.json`, gerado por `reference/hyurax/rede_ultrax.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use hyurax_ultrax::rede::{MensagemUltrax, TIPO_ULTRAX};
use hyurax_ultrax::validador::compromisso;
use serde_json::Value;

fn carregar(nome: &str) -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", nome].iter().collect();
    let bruto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("não consegui ler {}: {e}", caminho.display()));
    let doc: Value = serde_json::from_str(&bruto).unwrap();
    assert_eq!(doc["spec"].as_str(), Some(concat!(hyurax_identidade::raiz!(), "-SPEC-01")));
    doc["data"].clone()
}

fn bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap()).unwrap()
}

#[test]
fn cada_mensagem_vai_e_volta_igual_ao_gabarito() {
    let doc = carregar("rede_ultrax.json");
    assert_eq!(u64::from(TIPO_ULTRAX), doc["tipo_ultrax"].as_u64().unwrap());
    for caso in doc["mensagens"].as_array().unwrap() {
        let corpo = bytes(&caso["corpo"]);
        let m = MensagemUltrax::decodificar(&corpo).unwrap_or_else(|e| panic!("{}: {e}", caso["nome"]));
        assert_eq!(m.codificar().unwrap(), corpo, "{}", caso["nome"]);
        if let Some(confere) = caso["assinatura_confere"].as_bool() {
            assert_eq!(m.oferta_confere(), confere);
        }
    }
    let adulterada = MensagemUltrax::decodificar(&bytes(&doc["oferta_adulterada"]["corpo"])).unwrap();
    assert!(!adulterada.oferta_confere());
    assert!(!doc["oferta_adulterada"]["assinatura_confere"].as_bool().unwrap());
}

#[test]
fn compromisso_igual_ao_gabarito_e_recusados_nao_se_leem() {
    let doc = carregar("rede_ultrax.json");
    let worker: [u8; 32] = bytes(&doc["worker"]).try_into().unwrap();
    let r: [u8; 64] = bytes(&doc["resultado_hash"]).try_into().unwrap();
    assert_eq!(compromisso(&r, &worker).to_vec(), bytes(&doc["compromisso"]));
    for caso in doc["recusados"].as_array().unwrap() {
        assert!(MensagemUltrax::decodificar(&bytes(&caso["corpo"])).is_err(), "{} foi aceito", caso["nome"]);
    }
}

#[test]
fn compromisso_assinado_igual_ao_gabarito() {
    let doc = carregar("rede_ultrax.json");
    let caso = doc["mensagens"].as_array().unwrap().iter().find(|c| c["nome"] == "compromisso").unwrap();
    let m = MensagemUltrax::decodificar(&bytes(&caso["corpo"])).unwrap();
    let a = &doc["compromisso_assinado"];
    let job: [u8; 64] = bytes(&a["job"]).try_into().unwrap();
    let indice = a["indice"].as_u64().unwrap();
    assert_eq!(m.compromisso_confere(&job, indice), a["confere"].as_bool().unwrap());
    assert_eq!(m.compromisso_confere(&job, indice + 1), a["outro_indice_confere"].as_bool().unwrap());
    let mut outro = job;
    outro[0] ^= 1;
    assert!(!m.compromisso_confere(&outro, indice));
    assert!(a["confere"].as_bool().unwrap() && !a["outro_job_confere"].as_bool().unwrap());
}
