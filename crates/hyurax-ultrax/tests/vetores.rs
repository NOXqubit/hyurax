// ✝ Provérbios 11:1 — “Balança enganosa é abominação para o Senhor, mas o peso justo é o seu prazer.”
//! Os trabalhos do ULTRAX conferidos byte a byte contra `vectors/utrax.json`,
//! gerado pelo `reference/hyurax/utrax.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho, executar, instancia_mochila, verificar};
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

fn numero(v: &Value) -> u32 {
    u32::try_from(v.as_u64().unwrap()).unwrap()
}

fn rodar(esp: &Especificacao, semente: &[u8]) -> Vec<u8> {
    executar(esp, semente, &mut |_| true).unwrap().resultado
}

#[test]
fn matriz_igual_ao_gabarito() {
    let doc = carregar("utrax.json");
    let casos = doc["matrix"].as_array().unwrap();
    assert!(!casos.is_empty());
    for caso in casos {
        let semente = bytes(&caso["seed"]);
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, numero(&caso["size"]), 0).unwrap();
        let esperado = bytes(&caso["result"]);
        assert_eq!(rodar(&esp, &semente), esperado, "n = {}", esp.tamanho());
        assert_eq!(verificar(&esp, &semente, &esperado).is_ok(), caso["verifies"].as_bool().unwrap());
    }
}

#[test]
fn mochila_igual_ao_gabarito() {
    let doc = carregar("utrax.json");
    let casos = doc["knapsack"].as_array().unwrap();
    assert!(!casos.is_empty());
    for caso in casos {
        let semente = bytes(&caso["seed"]);
        let itens = numero(&caso["n_items"]);
        let inst = instancia_mochila(itens, &semente).unwrap();
        let lista = |v: &Value| v.as_array().unwrap().iter().map(numero).collect::<Vec<_>>();
        assert_eq!(inst.pesos, lista(&caso["weights"]), "pesos, n = {itens}");
        assert_eq!(inst.valores, lista(&caso["values"]), "valores, n = {itens}");
        assert_eq!(inst.capacidade, numero(&caso["capacity"]));

        let esp = Especificacao::nova(TipoDeTrabalho::Mochila, itens, 0).unwrap();
        let esperado = bytes(&caso["result"]);
        assert_eq!(rodar(&esp, &semente), esperado, "a mesma máscara ótima, n = {itens}");
        assert_eq!(verificar(&esp, &semente, &esperado).is_ok(), caso["verifies"].as_bool().unwrap());
        assert_eq!(verificar(&esp, &semente, &[0; 40]).is_err(), caso["all_zeros_rejected"].as_bool().unwrap());
    }
}

#[test]
fn difusao_igual_ao_gabarito() {
    let doc = carregar("utrax.json");
    let casos = doc["diffusion"].as_array().unwrap();
    assert!(!casos.is_empty());
    for caso in casos {
        let semente = bytes(&caso["seed"]);
        let esp = Especificacao::nova(TipoDeTrabalho::Difusao, numero(&caso["grid_size"]), numero(&caso["steps"])).unwrap();
        let esperado = bytes(&caso["result"]);
        assert_eq!(rodar(&esp, &semente), esperado, "{}", esp.resumo());
        assert!(verificar(&esp, &semente, &esperado).is_ok());
    }
}
