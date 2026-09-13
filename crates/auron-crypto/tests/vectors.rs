// ✝ 2 Pedro 3:10 — “Mas o Dia do Senhor virá como o ladrão de noite.”
//! Validação cruzada contra a implementação de referência em Python.
//!
//! Lê `vectors/hash.json` e `vectors/crypto_ed25519.json`, gerados por
//! `reference/tools/gen_vectors.py`, e confere byte a byte. A regra do projeto
//! é que um módulo só está migrado quando cada vetor bate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_crypto::{
    SECRET_LEN, address_from_ed25519_pubkey, ed25519_public_key, ed25519_sign, ed25519_verify,
    sha512, xof,
};
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
    let doc: Value =
        serde_json::from_str(&bruto).unwrap_or_else(|e| panic!("{nome} não é JSON válido: {e}"));
    assert_eq!(
        doc["spec"].as_str(),
        Some("AURON-SPEC-01"),
        "{nome}: vetor de outra versão da especificação"
    );
    doc
}

/// Campo hexadecimal de um caso, já decodificado.
fn bytes(caso: &Value, campo: &str) -> Vec<u8> {
    let texto = caso[campo]
        .as_str()
        .unwrap_or_else(|| panic!("campo {campo:?} precisa ser string hexadecimal"));
    hex::decode(texto).unwrap_or_else(|e| panic!("campo {campo:?} não é hex: {e}"))
}

#[test]
fn sha512_bate_com_o_python() {
    let doc = carregar("hash.json");
    let casos: Vec<&Value> = doc["data"]
        .as_array()
        .expect("hash.json precisa ter data")
        .iter()
        .filter(|caso| caso.get("sha512").is_some())
        .collect();
    assert!(casos.len() >= 4, "poucos casos de sha512 no vetor: {}", casos.len());

    for caso in &casos {
        let entrada = bytes(caso, "input");
        assert_eq!(
            sha512(&entrada).as_slice(),
            bytes(caso, "sha512").as_slice(),
            "sha512 de {} bytes",
            entrada.len()
        );
    }
    println!("sha512: {} casos batem com o Python", casos.len());
}

#[test]
fn xof_bate_com_o_python() {
    let doc = carregar("hash.json");
    let casos: Vec<&Value> = doc["data"]
        .as_array()
        .expect("hash.json precisa ter data")
        .iter()
        .filter(|caso| caso.get("xof_seed").is_some())
        .collect();
    assert!(casos.len() >= 3, "poucos casos de xof no vetor: {}", casos.len());

    for caso in &casos {
        let tamanho = caso["xof_len"].as_u64().expect("xof_len precisa ser inteiro") as usize;
        let obtido = xof(&bytes(caso, "xof_seed"), tamanho, &bytes(caso, "xof_domain"));
        assert_eq!(obtido, bytes(caso, "output"), "xof de {tamanho} bytes");
    }
    println!("xof: {} casos batem com o Python", casos.len());
}

#[test]
fn chaves_enderecos_e_assinaturas_batem_com_o_python() {
    let doc = carregar("crypto_ed25519.json");
    let chaves = doc["data"]
        .as_array()
        .expect("crypto_ed25519.json precisa ter data");
    assert!(chaves.len() >= 2, "poucas chaves no vetor: {}", chaves.len());

    let mut assinaturas = 0_usize;
    for chave in chaves {
        let rotulo = chave["label"].as_str().unwrap_or("?");
        let segredo: [u8; SECRET_LEN] = bytes(chave, "secret")
            .try_into()
            .unwrap_or_else(|v: Vec<u8>| panic!("{rotulo}: segredo com {} bytes", v.len()));

        let publica = ed25519_public_key(&segredo);
        assert_eq!(
            publica.as_slice(),
            bytes(chave, "public_key").as_slice(),
            "chave pública de {rotulo}"
        );
        assert_eq!(
            address_from_ed25519_pubkey(&publica).as_slice(),
            bytes(chave, "address").as_slice(),
            "endereço de {rotulo}"
        );

        for caso in chave["signatures"].as_array().expect("signatures") {
            let mensagem = bytes(caso, "message");
            let assinatura = bytes(caso, "signature");
            assert_eq!(
                ed25519_sign(&segredo, &mensagem).as_slice(),
                assinatura.as_slice(),
                "assinatura de {rotulo} sobre {} bytes",
                mensagem.len()
            );
            let valida = caso["valid"].as_bool().expect("valid precisa ser bool");
            assert_eq!(
                ed25519_verify(&publica, &mensagem, &assinatura),
                valida,
                "verificação de {rotulo} sobre {} bytes: o Python diz {valida}",
                mensagem.len()
            );
            assinaturas += 1;
        }
    }
    assert!(assinaturas >= 8, "poucas assinaturas no vetor: {assinaturas}");
    println!(
        "ed25519: {} chaves, {assinaturas} assinaturas e os endereços batem com o Python",
        chaves.len()
    );
}

/// Casos de borda da verificação, com o veredito calculado pelo oráculo.
///
/// Travam a regra da AURON-SPEC-01, seção 2, nos dois sentidos: chave em
/// encoding não-canônico é recusada, e ponto de ordem pequena é aceito. Trocar
/// `verify` por `verify_strict`, ou tirar a checagem de canonicidade, quebra
/// este teste.
#[test]
fn verificacao_nos_casos_de_borda_bate_com_o_python() {
    let doc = carregar("crypto_ed25519_verify.json");
    let casos = doc["data"]
        .as_array()
        .expect("crypto_ed25519_verify.json precisa ter data");

    let mut aceitos = 0_usize;
    let mut recusados = 0_usize;
    let mut divergencias: Vec<String> = Vec::new();
    for caso in casos {
        let rotulo = caso["label"].as_str().unwrap_or("?");
        let python = caso["valid"].as_bool().expect("valid precisa ser bool");
        let rust = ed25519_verify(
            &bytes(caso, "public_key"),
            &bytes(caso, "message"),
            &bytes(caso, "signature"),
        );
        let nome = |v: bool| if v { "aceita" } else { "recusa" };
        if rust != python {
            divergencias.push(format!("{rotulo}: Python {}, Rust {}", nome(python), nome(rust)));
        } else if python {
            aceitos += 1;
        } else {
            recusados += 1;
        }
    }

    assert!(
        divergencias.is_empty(),
        "{} divergência(s) entre Python e Rust:\n  {}",
        divergencias.len(),
        divergencias.join("\n  ")
    );
    assert!(aceitos >= 4, "poucos casos aceitos no vetor: {aceitos}");
    assert!(recusados >= 10, "poucos casos recusados no vetor: {recusados}");
    println!("verificação: {aceitos} aceitos e {recusados} recusados batem com o Python");
}

/// O `MANIFEST.json` declara tamanho e hash de cada vetor. Sem este teste
/// ninguém confere: o manifesto já ficou desatualizado por um commit inteiro
/// sem nada acusar.
#[test]
fn manifesto_bate_com_os_arquivos() {
    let manifesto = carregar("MANIFEST.json");
    let arquivos = manifesto["files"]
        .as_object()
        .expect("MANIFEST.json precisa ter files");
    assert!(arquivos.len() >= 17, "poucos arquivos no manifesto: {}", arquivos.len());

    let pasta: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors"]
        .iter()
        .collect();
    for (nome, esperado) in arquivos {
        // Bytes crus: ler como texto esconderia uma conversão de fim de linha.
        let bruto = std::fs::read(pasta.join(nome)).unwrap_or_else(|e| panic!("{nome}: {e}"));
        let tamanho = esperado["bytes"].as_u64().expect("bytes precisa ser inteiro");
        assert_eq!(bruto.len() as u64, tamanho, "{nome}: tamanho diverge do manifesto");
        let hash = hex::encode(sha512(&bruto));
        assert_eq!(
            &hash[..32],
            esperado["sha512_16"].as_str().expect("sha512_16 precisa ser string"),
            "{nome}: hash diverge do manifesto"
        );
    }
    println!("manifesto: {} arquivos batem com o conteúdo", arquivos.len());
}
