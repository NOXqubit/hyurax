// ✝ Provérbios 11:1 — “Balança enganosa é abominação para o Senhor, mas o peso justo é o seu prazer.”
//! Os trabalhos do ULTRAX conferidos byte a byte contra `vectors/ultrax.json`,
//! gerado pelo `reference/hyurax/ultrax.py`.

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
    let doc = carregar("ultrax.json");
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
    let doc = carregar("ultrax.json");
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
    let doc = carregar("ultrax.json");
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

#[test]
fn ia_igual_ao_gabarito() {
    use hyurax_ultrax::ia::{self, Base};
    let doc = carregar("ia.json");
    assert_eq!(
        hex::encode(hyurax_crypto::sha512(ia::BASE_TSV.as_bytes())),
        doc["base_sha512"].as_str().unwrap(),
        "a base embutida é a mesma que o gabarito leu"
    );
    let base = Base::embutida();
    assert_eq!(base.moleculas.len() as u64, doc["moleculas"].as_u64().unwrap());
    let semente = bytes(&doc["semente"]);
    let iniciais = ia::pesos_iniciais(&semente);
    assert_eq!(ia::codificar(&iniciais, 0), bytes(&doc["pesos_iniciais"]));
    let (_, validacao) = base.divisao();
    assert_eq!(ia::erro_de_validacao(&iniciais, base, &validacao), doc["erro_inicial"].as_u64().unwrap());
    for caso in doc["treinos"].as_array().unwrap() {
        let lote = numero(&caso["lote"]);
        let passos = numero(&caso["passos"]);
        let t = ia::treinar(&semente, lote, passos, base, &mut |_| true).unwrap();
        assert_eq!(ia::codificar(&t.pesos, t.erro), bytes(&caso["resultado"]), "lote {lote}, {passos} passos");
        let curva: Vec<u64> = caso["curva"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(t.curva, curva);
        let esp = Especificacao::nova(TipoDeTrabalho::Ia, lote, passos).unwrap();
        assert_eq!(esp.operacoes_fixas(), caso["operacoes"].as_u64());
        // e pelo caminho do worker, com a mesma semente
        assert_eq!(rodar(&esp, &semente), bytes(&caso["resultado"]));
    }
}
