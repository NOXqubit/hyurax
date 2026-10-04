// ✝ Atos 2:44 — “Todos os que criam estavam juntos e tinham tudo em comum.”
//! A nuvem conferida byte a byte contra `vectors/nuvem.json`, gerado por
//! `reference/hyurax/nuvem.py`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use hyurax_nuvem::codigo::{cauchy, codificar, prova, reconstruir};
use hyurax_nuvem::livro::{Lancamento, repartir};
use hyurax_nuvem::mensagem::MensagemNuvem;
use hyurax_nuvem::{TIPO_NUVEM, mensagem};
use serde_json::Value;

fn carregar() -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "nuvem.json"].iter().collect();
    let bruto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("não consegui ler {}: {e}", caminho.display()));
    let doc: Value = serde_json::from_str(&bruto).unwrap();
    doc["data"].clone()
}

fn bytes(v: &Value) -> Vec<u8> {
    hex::decode(v.as_str().unwrap()).unwrap()
}

fn fixo<const N: usize>(v: &Value) -> [u8; N] {
    bytes(v).try_into().unwrap()
}

#[test]
fn reed_solomon_igual_ao_gabarito() {
    let doc = carregar();
    assert_eq!(u64::from(TIPO_NUVEM), doc["tipo_nuvem"].as_u64().unwrap());
    let dados = bytes(&doc["dados"]);
    for caso in doc["codigos"].as_array().unwrap() {
        let k = caso["k"].as_u64().unwrap() as usize;
        let m = caso["m"].as_u64().unwrap() as usize;
        let esperados: Vec<Vec<u8>> = caso["fragmentos"].as_array().unwrap().iter().map(bytes).collect();
        let f = codificar(&dados, k, m).unwrap();
        assert_eq!(f, esperados, "k={k} m={m}");
        let perdidos: Vec<usize> = caso["reconstroi_sem"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
        let sobra: BTreeMap<usize, Vec<u8>> = f.into_iter().enumerate().filter(|(i, _)| !perdidos.contains(i)).collect();
        assert_eq!(reconstruir(&sobra, k, m, dados.len()).unwrap(), dados, "k={k} m={m}");
    }
    let vazio: Vec<Vec<u8>> = doc["vazio"].as_array().unwrap().iter().map(bytes).collect();
    assert_eq!(codificar(b"", 2, 1).unwrap(), vazio);
    let c: Vec<Vec<u8>> = serde_json::from_value(doc["cauchy_3_2"].clone()).unwrap();
    assert_eq!(cauchy(3, 2).unwrap(), c);
    let p = &doc["prova"];
    assert_eq!(prova(&fixo(&p["nonce"]), &bytes(&p["fragmento"])).to_vec(), bytes(&p["h"]));
}

#[test]
fn mensagens_vao_e_voltam_iguais_ao_gabarito() {
    let doc = carregar();
    for caso in doc["mensagens"].as_array().unwrap() {
        let corpo = bytes(&caso["corpo"]);
        let m = MensagemNuvem::decodificar(&corpo).unwrap_or_else(|e| panic!("{}: {e}", caso["nome"]));
        assert_eq!(m.subtipo(), caso["subtipo"].as_str().unwrap(), "{}", caso["nome"]);
        assert_eq!(m.codificar().unwrap(), corpo, "{}", caso["nome"]);
        match &m {
            MensagemNuvem::Anuncio(a) => {
                assert!(a.confere(), "{}: a assinatura do gabarito confere", caso["nome"]);
                assert_eq!(a.worker.to_vec(), bytes(&doc["anuncio_worker"]));
            }
            MensagemNuvem::Recibo(r) => {
                assert!(r.confere());
                assert_eq!(r.cliente.to_vec(), bytes(&doc["recibo_cliente"]));
            }
            _ => {}
        }
    }
    for caso in doc["recusados"].as_array().unwrap() {
        assert!(MensagemNuvem::decodificar(&bytes(&caso["corpo"])).is_err(), "{} foi aceito", caso["nome"]);
    }
    // assinatura adulterada: decodifica, mas não confere
    match MensagemNuvem::decodificar(&bytes(&doc["anuncio_assinatura_adulterada"])).unwrap() {
        MensagemNuvem::Anuncio(a) => assert!(!a.confere()),
        outra => panic!("esperava anúncio, veio {outra:?}"),
    }
    assert_eq!(mensagem::MOTIVO_MAX, 200);
}

#[test]
fn livro_e_partilha_iguais_ao_gabarito() {
    let doc = carregar();
    let mut anterior = [0u8; 64];
    for l in doc["livro"].as_array().unwrap() {
        let lanc = Lancamento {
            seq: l["seq"].as_u64().unwrap(),
            instante: l["instante"].as_u64().unwrap(),
            tipo: l["tipo"].as_str().unwrap().into(),
            conta: u32::try_from(l["conta"].as_u64().unwrap()).unwrap(),
            job: fixo(&l["job"]),
            contraparte: fixo(&l["contraparte"]),
            valor_mili: l["valor_mili"].as_u64().unwrap(),
            nota: l["nota"].as_str().unwrap().into(),
        };
        let h = lanc.hash(&anterior).unwrap();
        assert_eq!(h.to_vec(), bytes(&l["hash"]), "lançamento {}", lanc.seq);
        anterior = h;
    }
    for c in doc["repartir"].as_array().unwrap() {
        let partes = repartir(c["total"].as_u64().unwrap(), 80, 15).unwrap();
        let esperado: Vec<u64> = c["partes"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(vec![partes.0, partes.1, partes.2], esperado);
    }
}
