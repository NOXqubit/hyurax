// ✝ Provérbios 25:2 — “A glória dos reis é investigar a coisa.”
//! Triagem conferida byte a byte contra `vectors/triagem.json`, gerado por
//! `reference/hyurax/triagem.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use hyurax_crypto::sha512;
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho, derivar_unidade, executar, verificar};
use hyurax_ultrax::triagem::{CATALOGO_TSV, MODELO_TXT, catalogo};
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

fn lista(v: &Value) -> Vec<u32> {
    v.as_array().unwrap().iter().map(|x| u32::try_from(x.as_u64().unwrap()).unwrap()).collect()
}

fn esp(tamanho: &Value, parametros: &Value) -> Especificacao {
    Especificacao::nova_com(TipoDeTrabalho::Triagem, u32::try_from(tamanho.as_u64().unwrap()).unwrap(), 0, &lista(parametros)).unwrap()
}

#[test]
fn catalogo_e_modelo_sao_os_do_gabarito() {
    let doc = carregar("triagem.json");
    assert_eq!(hex::encode(sha512(CATALOGO_TSV.as_bytes())), doc["catalogo_sha512"].as_str().unwrap());
    assert_eq!(hex::encode(sha512(MODELO_TXT.as_bytes())), doc["modelo_sha512"].as_str().unwrap());
    let c = catalogo();
    assert_eq!(c.moleculas.len() as u64, doc["moleculas"].as_u64().unwrap(), "nenhuma linha do catálogo foi pulada");
    assert_eq!([c.media_mili, c.desvio_mili], [doc["normalizacao"][0].as_i64().unwrap(), doc["normalizacao"][1].as_i64().unwrap()]);
}

#[test]
fn triagem_igual_ao_gabarito_e_adulteracao_recusada() {
    let doc = carregar("triagem.json");
    for caso in doc["casos"].as_array().unwrap() {
        let e = esp(&caso["tamanho"], &caso["parametros"]);
        let exec = executar(&e, b"", &mut |_| true).unwrap();
        assert_eq!(hex::encode(&exec.resultado), caso["resultado"].as_str().unwrap(), "{}", e.resumo());
        assert_eq!(exec.operacoes, caso["operacoes"].as_u64().unwrap());
        assert!(verificar(&e, b"", &exec.resultado).is_ok());
    }
    let a = &doc["adulterado"];
    let e = esp(&a["tamanho"], &a["parametros"]);
    let recusa = verificar(&e, b"", &bytes(&a["resultado"])).unwrap_err();
    assert_eq!(recusa.0, a["motivo"].as_str().unwrap(), "o mesmo motivo do gabarito");
}

#[test]
fn unidades_do_job_iguais_ao_gabarito() {
    let doc = carregar("triagem.json");
    let u = &doc["unidades_do_job"];
    let modelo = esp(&u["tamanho"], &u["parametros"]);
    for unidade in u["unidades"].as_array().unwrap() {
        let d = derivar_unidade(&modelo, unidade["indice"].as_u64().unwrap()).unwrap();
        assert_eq!(u64::from(d.tamanho()), unidade["tamanho"].as_u64().unwrap());
        assert_eq!(d.parametros(), lista(&unidade["parametros"]).as_slice());
    }
    assert!(derivar_unidade(&modelo, u["depois_do_fim"].as_u64().unwrap()).is_err());
}
