// ✝ Provérbios 6:6-8 — “Vai ter com a formiga, ó preguiçoso; olha para os seus caminhos e sê sábio: no verão prepara o seu pão.”
//! Validação cruzada contra a implementação de referência em Python.
//!
//! Lê `vectors/units.json`, gerado por `reference/tools/gen_vectors.py`, e
//! confere caso a caso. A regra do projeto é que um módulo só está migrado
//! quando cada vetor bate. "Compilou e os testes locais passaram" não é
//! migração; é coincidência até prova em contrário.
//!
//! Confere os dois lados: o que a referência aceita, e o que ela recusa. Um
//! parser mais permissivo de um lado que do outro é divergência de consenso.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_types::{Amount, MAX_SUPPLY};
use serde_json::Value;

/// Localiza `vectors/` a partir da raiz deste crate.
fn carregar(nome: &str) -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", nome]
        .iter()
        .collect();
    let bruto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| {
        panic!(
            "não consegui ler {}: {e}\n\
             Os vetores são gerados por: python reference/tools/gen_vectors.py",
            caminho.display()
        )
    });
    serde_json::from_str(&bruto).unwrap_or_else(|e| panic!("{nome} não é JSON válido: {e}"))
}

#[test]
fn vetores_de_unidades_parse() {
    let doc = carregar("units.json");
    assert_eq!(
        doc["spec"].as_str(),
        Some("AURON-SPEC-01"),
        "vetor de outra versão da especificação"
    );

    let casos = doc["data"]["parse"]
        .as_array()
        .expect("units.json precisa ter data.parse");
    assert!(!casos.is_empty(), "nenhum caso de parse no vetor");

    let mut aceitos = 0_usize;
    let mut recusados = 0_usize;
    let mut divergencias: Vec<String> = Vec::new();

    for caso in casos {
        let entrada = caso["input"].as_str().expect("input precisa ser string");
        let deve_aceitar = caso["accepted"].as_bool().expect("accepted precisa ser bool");

        match (Amount::from_aur_str(entrada), deve_aceitar) {
            (Ok(valor), true) => {
                aceitos += 1;
                let esperado_units: u64 = caso["units"]
                    .as_str()
                    .expect("units precisa ser string")
                    .parse()
                    .expect("units precisa ser inteiro");
                if valor.units() != esperado_units {
                    divergencias.push(format!(
                        "{entrada:?}: Python deu {esperado_units}, Rust deu {}",
                        valor.units()
                    ));
                }
                let esperado_texto = caso["formatted"].as_str().expect("formatted");
                if valor.to_aur_string() != esperado_texto {
                    divergencias.push(format!(
                        "{entrada:?}: formatação Python {esperado_texto:?}, Rust {:?}",
                        valor.to_aur_string()
                    ));
                }
            }
            (Err(_), false) => {
                recusados += 1;
            }
            (Ok(valor), false) => {
                divergencias.push(format!(
                    "{entrada:?}: Python RECUSOU, Rust aceitou e deu {}",
                    valor.units()
                ));
            }
            (Err(e), true) => {
                divergencias.push(format!(
                    "{entrada:?}: Python ACEITOU, Rust recusou com {e}"
                ));
            }
        }
    }

    assert!(
        divergencias.is_empty(),
        "{} divergência(s) entre Python e Rust:\n  {}",
        divergencias.len(),
        divergencias.join("\n  ")
    );

    // Um vetor que só testa o caminho feliz não prova quase nada.
    assert!(aceitos >= 10, "poucos casos aceitos no vetor: {aceitos}");
    assert!(recusados >= 10, "poucos casos recusados no vetor: {recusados}");
    println!("parse: {aceitos} aceitos e {recusados} recusados batem com o Python");
}

#[test]
fn vetores_de_unidades_formatacao() {
    let doc = carregar("units.json");
    let casos = doc["data"]["format"]
        .as_array()
        .expect("units.json precisa ter data.format");

    for caso in casos {
        let unidades: u64 = caso["units"]
            .as_str()
            .expect("units")
            .parse()
            .expect("units inteiro");
        let esperado = caso["formatted"].as_str().expect("formatted");
        let obtido = Amount::from_units(unidades).to_aur_string();
        assert_eq!(obtido, esperado, "formatação de {unidades} unidades");
    }
    println!("formatação: {} casos batem com o Python", casos.len());
}

#[test]
fn ida_e_volta_preserva_o_valor() {
    let doc = carregar("units.json");
    let casos = doc["data"]["parse"].as_array().expect("data.parse");

    for caso in casos {
        if !caso["accepted"].as_bool().unwrap_or(false) {
            continue;
        }
        let entrada = caso["input"].as_str().expect("input");
        let valor = Amount::from_aur_str(entrada).expect("caso aceito");
        let voltou = Amount::from_aur_str(&valor.to_aur_string())
            .expect("a própria formatação precisa ser aceita de volta");
        assert_eq!(valor, voltou, "ida e volta mudou o valor de {entrada:?}");
    }
    println!("ida e volta preserva o valor em todos os casos aceitos");
}

#[test]
fn constantes_batem_com_a_especificacao() {
    assert_eq!(MAX_SUPPLY, 21_000_000 * 100_000_000);
    assert_eq!(Amount::MAX_SUPPLY.to_aur_string(), "21000000.00000000");
    assert_eq!(auron_types::AUR_UNIT, 100_000_000);
    assert_eq!(auron_types::AUR_DECIMALS, 8);
}
