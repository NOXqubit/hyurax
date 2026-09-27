// ✝ Mateus 7:17 — “Assim, toda árvore boa produz bons frutos, e toda árvore má produz frutos maus.”
//! O motor de melhoramento de culturas conferido byte a byte contra
//! `vectors/melhoramento.json`, gerado por `reference/hyurax/melhoramento.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use hyurax_ultrax::melhoramento::{self, Gerador, Resultado};
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho, executar, verificar};
use serde_json::Value;

fn carregar() -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "melhoramento.json"].iter().collect();
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

fn lista(v: &Value) -> Vec<u32> {
    v.as_array().unwrap().iter().map(numero).collect()
}

fn especificacao(caso: &Value) -> Result<Especificacao, hyurax_ultrax::trabalho::ErroDeTrabalho> {
    Especificacao::nova_com(TipoDeTrabalho::Melhoramento, numero(&caso["tamanho"]), numero(&caso["passos"]), &lista(&caso["parametros"]))
}

#[test]
fn gerador_arquitetura_e_liebig_iguais_ao_gabarito() {
    let doc = carregar();
    let semente = bytes(&doc["semente"]);
    let mut g = Gerador::da_semente(&semente);
    let esperado: Vec<u64> = doc["gerador"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    let feito: Vec<u64> = esperado.iter().map(|_| g.proximo()).collect();
    assert_eq!(feito, esperado, "xoshiro256** semeado pelo XOF");

    let arq = &doc["arquitetura"];
    let efeitos = melhoramento::arquitetura(numero(&arq["codigo"]), numero(&arq["qtl"]));
    let esperado: Vec<(i64, i64)> = arq["efeitos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|par| (par[0].as_i64().unwrap(), par[1].as_i64().unwrap()))
        .collect();
    assert_eq!(efeitos.iter().map(|q| (q.a, q.d)).collect::<Vec<_>>(), esperado);

    let fatores = doc["fatores"].as_array().unwrap();
    assert!(!fatores.is_empty());
    for f in fatores {
        let v = lista(f);
        let limitante = u8::try_from(v[4]).unwrap();
        assert_eq!(melhoramento::fator_ambiental(v[0], v[1], v[2]), (v[3], limitante), "água {}, N {}, solo {}", v[0], v[1], v[2]);
    }
}

#[test]
fn unidades_iguais_ao_gabarito() {
    let doc = carregar();
    let semente = bytes(&doc["semente"]);
    let casos = doc["casos"].as_array().unwrap();
    assert!(!casos.is_empty());
    for caso in casos {
        let esp = especificacao(caso).unwrap();
        let esperado = bytes(&caso["resultado"]);
        let mut chamadas = Vec::new();
        let exec = executar(&esp, &semente, &mut |ops| {
            chamadas.push(ops);
            true
        })
        .unwrap();
        assert_eq!(exec.resultado, esperado, "{}", esp.resumo());
        let pedacos: Vec<u64> = caso["chamadas"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(chamadas, pedacos, "os pedaços de `continuar`: {}", esp.resumo());
        assert_eq!(exec.operacoes, caso["operacoes"].as_u64().unwrap());
        assert_eq!(esp.operacoes_fixas(), caso["operacoes"].as_u64());

        let r = Resultado::decodificar(&esperado, esp.passos(), esp.parametros()[0]).unwrap();
        assert_eq!(r.fator, numero(&caso["fator"]));
        assert_eq!(u32::from(r.limitante), numero(&caso["limitante"]));
        assert_eq!(r.sigma, caso["sigma"].as_u64().unwrap());
        assert_eq!(r.selecionados, numero(&caso["selecionados"]));
        assert_eq!(verificar(&esp, &semente, &esperado), Ok(()));
        let mut errado = esperado.clone();
        *errado.last_mut().unwrap() ^= 1;
        assert!(verificar(&esp, &semente, &errado).is_err());
    }
}

#[test]
fn faixas_e_teto_iguais_ao_gabarito() {
    let doc = carregar();
    assert_eq!(doc["teto_operacoes"].as_u64(), Some(melhoramento::TETO_OPERACOES));
    for caso in doc["aceitas"].as_array().unwrap() {
        assert!(especificacao(caso).is_ok(), "o gabarito aceita {caso}");
    }
    for caso in doc["recusadas"].as_array().unwrap() {
        assert!(especificacao(caso).is_err(), "o gabarito recusa {caso}");
    }
}
