// ✝ Provérbios 16:11 — “O peso e a balança justos pertencem ao Senhor.”
//! A genética de populações conferida byte a byte contra `vectors/genetica.json`,
//! gerado pelo `reference/hyurax/genetica.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use hyurax_ultrax::genetica::{self, Trajetoria};
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho, executar, verificar};
use serde_json::Value;

fn carregar() -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "genetica.json"].iter().collect();
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

fn u64_hex(v: &Value) -> u64 {
    u64::from_str_radix(v.as_str().unwrap(), 16).unwrap()
}

fn lista_u32(v: &Value) -> Vec<u32> {
    v.as_array().unwrap().iter().map(numero).collect()
}

fn lista_u64(v: &Value) -> Vec<u64> {
    v.as_array().unwrap().iter().map(|x| x.as_u64().unwrap()).collect()
}

#[test]
fn fluxo_de_sorteios_igual_ao_gabarito() {
    let doc = carregar();
    let semente = bytes(&doc["semente"]);
    let s0 = genetica::estado_inicial(&semente);
    assert_eq!(s0, u64_hex(&doc["estado_inicial"]));
    for caso in doc["fluxo"].as_array().unwrap() {
        let n = caso["n"].as_u64().unwrap();
        assert_eq!(genetica::splitmix64(s0, n), u64_hex(&caso["saida"]), "saída {n}");
    }
    let padrao: Vec<u64> = doc["splitmix64_estado_zero"].as_array().unwrap().iter().map(u64_hex).collect();
    assert_eq!(padrao, (0..4).map(|n| genetica::splitmix64(0, n)).collect::<Vec<_>>());
}

#[test]
fn limiar_igual_ao_gabarito() {
    let doc = carregar();
    let casos = doc["limiares"].as_array().unwrap();
    assert!(!casos.is_empty());
    for caso in casos {
        let w = genetica::aptidoes(numero(&caso["selecao"]), numero(&caso["dominancia"]));
        let obtido = genetica::limiar(numero(&caso["k"]), numero(&caso["m"]), w, numero(&caso["mutacao"]));
        assert_eq!(obtido, caso["limiar"].as_u64().unwrap(), "{caso}");
    }
}

#[test]
fn trajetorias_iguais_ao_gabarito() {
    let doc = carregar();
    let semente = bytes(&doc["semente"]);
    let casos = doc["casos"].as_array().unwrap();
    assert!(casos.len() >= 8);
    for caso in casos {
        let (tamanho, passos) = (numero(&caso["tamanho"]), numero(&caso["passos"]));
        let parametros = lista_u32(&caso["parametros"]);
        let esp = Especificacao::nova_com(TipoDeTrabalho::Genetica, tamanho, passos, &parametros).unwrap();
        let esperado = bytes(&caso["resultado"]);
        let exec = executar(&esp, &semente, &mut |_| true).unwrap();
        assert_eq!(exec.resultado, esperado, "{}", esp.resumo());
        assert_eq!(exec.operacoes, caso["operacoes"].as_u64().unwrap());
        assert_eq!(esp.operacoes_fixas(), caso["operacoes"].as_u64());
        assert_eq!(verificar(&esp, &semente, &esperado), Ok(()));

        let est = Trajetoria::decodificar(&esperado).unwrap().estatisticas();
        let gabarito = &caso["estatisticas"];
        assert_eq!(est.fixados, numero(&gabarito["fixados"]));
        assert_eq!(est.perdidos, numero(&gabarito["perdidos"]));
        assert_eq!(est.soma_por_geracao, lista_u64(&gabarito["soma_por_geracao"]));
        assert_eq!(est.soma_quadrados_por_geracao, lista_u64(&gabarito["soma_quadrados_por_geracao"]));
    }
}

#[test]
fn recusas_iguais_ao_gabarito() {
    let doc = carregar();
    for caso in doc["recusados"].as_array().unwrap() {
        let parametros = lista_u32(&caso["parametros"]);
        let r = Especificacao::nova_com(TipoDeTrabalho::Genetica, numero(&caso["tamanho"]), numero(&caso["passos"]), &parametros);
        assert!(r.is_err(), "aceitou {caso}");
    }
    let limite = &doc["no_limite"];
    let parametros = lista_u32(&limite["parametros"]);
    let esp = Especificacao::nova_com(TipoDeTrabalho::Genetica, numero(&limite["tamanho"]), numero(&limite["passos"]), &parametros)
        .unwrap();
    assert!(esp.operacoes_fixas().unwrap() <= genetica::SORTEIOS_MAX);
}
