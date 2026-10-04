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

// ------------------------------------------------------------------ planos

fn carregar_plano() -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "plano.json"].iter().collect();
    let bruto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("não consegui ler {}: {e}", caminho.display()));
    serde_json::from_str::<Value>(&bruto).unwrap()["data"].clone()
}

#[test]
fn planos_batem_com_o_gabarito() {
    use hyurax_nuvem::plano::{CATALOGO, Voucher};
    let d = carregar_plano();
    let emissor: [u8; 32] = fixo(&d["emissor_publica"]);
    let segredo: [u8; 32] = core::array::from_fn(|i| i as u8);
    assert_eq!(hyurax_crypto::ed25519_public_key(&segredo), emissor);

    // o catálogo do Rust é o do gabarito
    let cat = d["catalogo"].as_array().unwrap();
    assert_eq!(cat.len(), CATALOGO.len());
    for (c, p) in cat.iter().zip(CATALOGO.iter()) {
        assert_eq!(c["id"].as_u64().unwrap(), u64::from(p.id));
        assert_eq!(c["nome"].as_str().unwrap(), p.nome);
        assert_eq!(c["mensal_centavos"].as_u64().unwrap(), p.mensal_centavos);
        assert_eq!(c["anual_centavos"].as_u64().unwrap(), p.anual_centavos);
        assert_eq!(c["creditos_mes"].as_u64().unwrap(), p.creditos_mes);
        assert_eq!(c["armazenamento_gib"].as_u64().unwrap(), u64::from(p.armazenamento_gib));
        assert_eq!(c["comissao_bp"].as_u64().unwrap(), u64::from(p.comissao_bp));
    }

    for v in d["vouchers"].as_array().unwrap() {
        let id = u8::try_from(v["plano"].as_u64().unwrap()).unwrap();
        let quem: [u8; 32] = fixo(&v["beneficiario"]);
        let serie = u32::try_from(v["serie"].as_u64().unwrap()).unwrap();
        let emitido = Voucher::emitir(&segredo, id, quem, v["inicio_ms"].as_u64().unwrap(), v["fim_ms"].as_u64().unwrap(), serie).unwrap();
        assert_eq!(emitido.bytes().unwrap(), bytes(&v["bytes"]), "emitir igual ao gabarito");
        assert_eq!(emitido.texto().unwrap(), v["texto"].as_str().unwrap());
        let lido = Voucher::ler_texto(v["texto"].as_str().unwrap()).unwrap();
        assert_eq!(lido, emitido);
        for caso in v["vale"].as_array().unwrap() {
            let w: [u8; 32] = fixo(&caso["worker"]);
            assert_eq!(lido.vale(&emissor, &w, caso["agora_ms"].as_u64().unwrap()), caso["vale"].as_bool().unwrap(), "{caso}");
        }
    }
    for c in d["nao_confere"].as_array().unwrap() {
        let v = Voucher::ler(&bytes(&c["bytes"])).unwrap();
        assert!(!v.assinatura_confere(&emissor), "{c}");
    }
    for r in d["recusas"].as_array().unwrap() {
        assert_eq!(Voucher::ler(&bytes(&r["bytes"])).unwrap_err().to_string(), r["erro"].as_str().unwrap(), "{r}");
    }
    for r in d["recusas_texto"].as_array().unwrap() {
        assert_eq!(Voucher::ler_texto(r["texto"].as_str().unwrap()).unwrap_err().to_string(), r["erro"].as_str().unwrap(), "{r}");
    }
}
