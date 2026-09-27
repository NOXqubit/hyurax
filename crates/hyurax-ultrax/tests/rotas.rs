// ✝ Provérbios 16:9 — “O coração do homem planeja o seu caminho.”
//! Rotas conferidas byte a byte contra `vectors/rotas.json`, gerado por
//! `reference/hyurax/rotas.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use hyurax_ultrax::rotas::{Gerador, certificar, certificar_otimo_local, coordenadas, partida};
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho, executar, verificar};
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

fn n32(v: &Value) -> u32 {
    u32::try_from(v.as_u64().unwrap()).unwrap()
}

fn esp(tamanho: u32, passos: u32, instancia: u32) -> Especificacao {
    Especificacao::nova_com(TipoDeTrabalho::Rotas, tamanho, passos, &[instancia]).unwrap()
}

#[test]
fn gerador_instancia_e_partida_iguais_ao_gabarito() {
    let doc = carregar("rotas.json");
    let mut g = Gerador::do_estado([1, 2, 3, 4]);
    let esperados: Vec<u64> = doc["xoshiro256ss_estado_1234"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    let saidas: Vec<u64> = (0..esperados.len()).map(|_| g.proximo()).collect();
    assert_eq!(saidas, esperados);
    let cidades: Vec<(u64, u64)> =
        doc["coordenadas_7_12"].as_array().unwrap().iter().map(|c| (c[0].as_u64().unwrap(), c[1].as_u64().unwrap())).collect();
    let feitas: Vec<(u64, u64)> = coordenadas(7, 12).iter().map(|c| (u64::from(c.x), u64::from(c.y))).collect();
    assert_eq!(feitas, cidades);
    let p = &doc["partida"];
    let rota: Vec<u32> = p["rota"].as_array().unwrap().iter().map(n32).collect();
    assert_eq!(partida(&bytes(&p["semente"]), n32(&p["n"])), rota);
}

#[test]
fn dois_opt_certificados_e_conferencia_iguais_ao_gabarito() {
    let doc = carregar("rotas.json");
    for caso in doc["casos"].as_array().unwrap() {
        let e = esp(n32(&caso["tamanho"]), n32(&caso["passos"]), n32(&caso["instancia"]));
        let semente = bytes(&caso["semente"]);
        let esperado = bytes(&caso["resultado"]);
        let exec = executar(&e, &semente, &mut |_| true).unwrap();
        assert_eq!(exec.resultado, esperado, "n = {}", e.tamanho());
        assert_eq!(exec.operacoes, caso["operacoes"].as_u64().unwrap(), "operações, n = {}", e.tamanho());
        assert_eq!(certificar(&e, &esperado).unwrap(), caso["comprimento"].as_u64().unwrap());
        let local = certificar_otimo_local(&e, &esperado, &mut |_| true).unwrap();
        assert_eq!(local.is_ok(), caso["otimo_local"].as_bool().unwrap(), "ótimo local, n = {}", e.tamanho());
        assert!(verificar(&e, &semente, &esperado).is_ok());
    }
    let a = &doc["adulterados"];
    let e = esp(n32(&a["tamanho"]), n32(&a["passos"]), n32(&a["instancia"]));
    for caso in a["casos"].as_array().unwrap() {
        let recusado = certificar(&e, &bytes(&caso["resultado"])).is_err();
        assert_eq!(recusado, caso["recusado"].as_bool().unwrap(), "{}", caso["nome"]);
        assert!(recusado, "{} passou", caso["nome"]);
    }
}
