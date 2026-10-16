// ✝ Lucas 16:10 — “Quem é fiel no mínimo, também é fiel no muito.”
//! As regras da plataforma conferidas byte a byte contra
//! `vectors/plataforma.json`, gerado por `reference/hyurax/plataforma.py`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use hyurax_plataforma::regras::{self, Pagador, Tabela};
use serde_json::Value;

fn carregar() -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "plataforma.json"].iter().collect();
    let bruto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("não consegui ler {}: {e}", caminho.display()));
    let doc: Value = serde_json::from_str(&bruto).unwrap();
    doc["data"].clone()
}

fn bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap()).unwrap()
}

fn tabela(v: &Value) -> Tabela {
    serde_json::from_value(v.clone()).unwrap()
}

#[test]
fn dominios_e_janela() {
    let d = carregar();
    assert_eq!(d["dominio_pedido"].as_str().unwrap().as_bytes(), regras::DOMINIO_PEDIDO);
    assert_eq!(d["dominio_job"].as_str().unwrap().as_bytes(), regras::DOMINIO_JOB);
    assert_eq!(d["janela_ms"].as_u64().unwrap(), regras::JANELA_MS);
}

#[test]
fn pedidos_do_agente() {
    let d = carregar();
    let mut validos = 0;
    for c in d["pedidos"].as_array().unwrap() {
        let r = regras::conferir_pedido(
            &bytes(&c["chave"]),
            c["metodo"].as_str().unwrap(),
            c["caminho"].as_str().unwrap(),
            c["tempo_ms"].as_u64().unwrap(),
            &bytes(&c["corpo"]),
            &bytes(&c["assinatura"]),
            c["agora_ms"].as_u64().unwrap(),
        );
        match c["motivo"].as_str() {
            None => {
                assert_eq!(r, Ok(()), "{c}");
                validos += 1;
            }
            Some(m) => assert_eq!(r, Err(m), "{c}"),
        }
    }
    assert_eq!(validos, 3);
    // e o Rust assina igual ao gabarito
    let c = &d["pedidos"][0];
    let segredo: [u8; 32] = core::array::from_fn(|i| i as u8);
    let a = regras::assinar_pedido(&segredo, "POST", c["caminho"].as_str().unwrap(), c["tempo_ms"].as_u64().unwrap(), &bytes(&c["corpo"]));
    assert_eq!(a.to_vec(), bytes(&c["assinatura"]));
}

#[test]
fn jobs_e_sementes() {
    let d = carregar();
    for c in d["jobs"].as_array().unwrap() {
        let job_id: [u8; 64] = bytes(&c["job_id"]).try_into().unwrap();
        let nonce: [u8; 32] = bytes(&c["nonce"]).try_into().unwrap();
        let job = regras::job_da_tarefa(&job_id, c["tarefa"].as_u64().unwrap(), &nonce);
        assert_eq!(job.to_vec(), bytes(&c["job"]));
        for s in c["sementes"].as_array().unwrap() {
            assert_eq!(regras::semente(&job, s["indice"].as_u64().unwrap()).to_vec(), bytes(&s["semente"]));
        }
    }
}

#[test]
fn custos_e_estimativas() {
    let d = carregar();
    for c in d["custos"].as_array().unwrap() {
        let r = regras::custo_da_execucao(&tabela(&c["tabela"]), c["operacoes"].as_u64().unwrap(), c["memoria"].as_u64().unwrap(), c["gpu"].as_bool().unwrap());
        match c["erro"].as_str() {
            None => assert_eq!(r.unwrap().to_string(), c["micro"].as_str().unwrap(), "{c}"),
            Some(m) => assert_eq!(r, Err(m), "{c}"),
        }
    }
    for c in d["estimativas"].as_array().unwrap() {
        let r = regras::estimativa(
            &tabela(&c["tabela"]),
            c["operacoes_maximas"].as_u64().unwrap(),
            c["memoria"].as_u64().unwrap(),
            c["unidades"].as_u64().unwrap(),
            c["copias"].as_u64().unwrap(),
            c["gpu"].as_bool().unwrap(),
        );
        match c["erro"].as_str() {
            None => {
                let micro = r.unwrap();
                assert_eq!(micro.to_string(), c["micro"].as_str().unwrap());
                assert_eq!(regras::centavos(micro).to_string(), c["centavos"].as_str().unwrap());
            }
            Some(m) => assert_eq!(r, Err(m), "{c}"),
        }
    }
}

#[test]
fn liquidacoes() {
    let d = carregar();
    for c in d["liquidacoes"].as_array().unwrap() {
        let ex: Vec<(u64, u128, Pagador)> = c["execucoes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e["no"].as_u64().unwrap(), u128::from(e["micro"].as_u64().unwrap()), if e["pagador"] == "cliente" { Pagador::Cliente } else { Pagador::Plataforma }))
            .collect();
        let r = regras::liquidar(&ex, u32::try_from(c["fornecedor_bp"].as_u64().unwrap()).unwrap());
        match c["erro"].as_str() {
            Some(m) => assert_eq!(r, Err(m), "{c}"),
            None => {
                let l = r.unwrap();
                assert_eq!(l.cobranca, u128::from(c["cobranca"].as_u64().unwrap()), "{c}");
                assert_eq!(l.conferencia, u128::from(c["conferencia"].as_u64().unwrap()), "{c}");
                assert_eq!(l.plataforma, i128::from(c["plataforma"].as_i64().unwrap()), "{c}");
                let esperado: BTreeMap<u64, u128> = c["fornecedores"].as_array().unwrap().iter().map(|f| (f["no"].as_u64().unwrap(), u128::from(f["centavos"].as_u64().unwrap()))).collect();
                assert_eq!(l.fornecedores, esperado, "{c}");
                // fecha ao centavo
                let soma: u128 = l.fornecedores.values().sum();
                assert_eq!(i128::try_from(soma).unwrap() + l.plataforma, i128::try_from(l.cobranca).unwrap());
            }
        }
    }
}

#[test]
fn reputacao() {
    let d = carregar();
    for c in d["reputacao"].as_array().unwrap() {
        let (v, f, i) = (c["verificadas"].as_u64().unwrap(), c["falhas"].as_u64().unwrap(), c["incorretas"].as_u64().unwrap());
        let p = regras::pontuacao(v, f, i);
        assert_eq!(u64::from(p), c["pontuacao"].as_u64().unwrap(), "{c}");
        assert_eq!(regras::faixa(p), c["faixa"].as_str().unwrap());
        assert_eq!(u64::from(regras::amostragem_bp(p, v)), c["amostragem_bp"].as_u64().unwrap(), "{c}");
        assert_eq!(regras::quarentena(p, i), c["quarentena"].as_bool().unwrap());
    }
}
