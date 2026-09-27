// ✝ Neemias 4:6 — “Assim edificamos o muro, porque o povo tinha ânimo para trabalhar.”
//! O JOB conferido byte a byte contra `vectors/job.json`, gerado por
//! `reference/hyurax/job.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use hyurax_codec::Writer;
use hyurax_ultrax::job::{
    Dominio, EspecificacaoDeJob, Intervalos, Nivel, PedidoDeJob, Resumo, milicreditos, semente_da_unidade, unidade,
};
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho};
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

fn u64_de(v: &Value) -> u64 {
    v.as_u64().unwrap()
}

#[test]
fn job_codificacao_id_e_sementes_iguais_ao_gabarito() {
    let doc = carregar("job.json");
    let jobs = doc["jobs"].as_array().unwrap();
    assert!(jobs.len() >= 3);
    for caso in jobs {
        let tipo = TipoDeTrabalho::de_codigo(u8::try_from(u64_de(&caso["tipo"])).unwrap()).unwrap();
        let modelo = Especificacao::nova(
            tipo,
            u32::try_from(u64_de(&caso["tamanho"])).unwrap(),
            u32::try_from(u64_de(&caso["passos"])).unwrap(),
        )
        .unwrap();
        let job = EspecificacaoDeJob::nova(PedidoDeJob {
            dominio: Dominio::de_codigo(u8::try_from(u64_de(&caso["dominio"])).unwrap()).unwrap(),
            modelo,
            unidades: u64_de(&caso["unidades"]),
            nivel: Nivel::de_codigo(u8::try_from(u64_de(&caso["nivel"])).unwrap()).unwrap(),
            redundancia: u8::try_from(u64_de(&caso["redundancia"])).unwrap(),
            prazo_s: u64_de(&caso["prazo_s"]),
            orcamento_milicreditos: u64_de(&caso["orcamento_milicreditos"]),
            descricao: caso["descricao"].as_str().unwrap().to_string(),
        })
        .unwrap();
        let codificado = job.codificar().unwrap();
        assert_eq!(codificado, bytes(&caso["codificacao"]), "{}", job.descricao());
        assert_eq!(EspecificacaoDeJob::decodificar(&codificado).unwrap(), job);
        let id = job.id().unwrap();
        assert_eq!(id.to_vec(), bytes(&caso["id"]));
        for s in caso["sementes"].as_array().unwrap() {
            let i = u64_de(&s["indice"]);
            assert_eq!(semente_da_unidade(&id, i).to_vec(), bytes(&s["semente"]), "unidade {i}");
            assert_eq!(unidade(&job, &id, i).unwrap().semente.to_vec(), bytes(&s["semente"]));
        }
    }
}

#[test]
fn resumo_intervalos_e_creditos_iguais_ao_gabarito() {
    let doc = carregar("job.json");

    let mut resumo = Resumo::default();
    for par in doc["resumo"]["pares"].as_array().unwrap() {
        let h: [u8; 64] = bytes(&par["hash"]).try_into().unwrap();
        resumo.somar(u64_de(&par["indice"]), &h);
    }
    assert_eq!(resumo.bytes().to_vec(), bytes(&doc["resumo"]["soma"]));

    let iv = &doc["intervalos"];
    let mut v = Intervalos::new();
    for par in iv["insercoes"].as_array().unwrap() {
        v.inserir(u64_de(&par[0]), u64_de(&par[1]));
    }
    let faixas: Vec<(u64, u64)> = iv["faixas"].as_array().unwrap().iter().map(|f| (u64_de(&f[0]), u64_de(&f[1]))).collect();
    assert_eq!(v.faixas(), faixas.as_slice());
    let mut w = Writer::new();
    v.codificar(&mut w).unwrap();
    assert_eq!(w.into_bytes(), bytes(&iv["codificacao"]));
    assert_eq!(v.concluidas(), u64_de(&iv["concluidas"]));
    for caso in iv["primeira_faltante"].as_array().unwrap() {
        let esperado = caso[2].as_u64();
        assert_eq!(v.primeira_faltante(u64_de(&caso[0]), u64_de(&caso[1])), esperado, "{caso}");
    }

    for caso in doc["creditos"].as_array().unwrap() {
        assert_eq!(milicreditos(u64_de(&caso[0]), u64_de(&caso[1])), u64_de(&caso[2]));
    }
}
