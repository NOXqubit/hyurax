//! Validação cruzada contra a implementação de referência em Python.
//!
//! Lê `vectors/codec.json` (caminho feliz da escrita e raízes de Merkle) e
//! `vectors/codec_edge.json` (leitura e provas nos casos de borda), gerados
//! por `reference/tools/gen_vectors.py`. Confere os dois lados: o que a
//! referência aceita e o que ela recusa, e em cada recusa a mesma mensagem.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::path::PathBuf;

use auron_codec::{
    CodecError, HASH_LEN, Reader, Writer, merkle_path, merkle_root, merkle_verify_path,
};
use serde_json::{Value, json};

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
    hex_de(&caso[campo])
}

fn hex_de(valor: &Value) -> Vec<u8> {
    let texto = valor
        .as_str()
        .unwrap_or_else(|| panic!("esperava string hexadecimal, veio {valor}"));
    hex::decode(texto).unwrap_or_else(|e| panic!("{texto:?} não é hex: {e}"))
}

fn hash64(valor: &Value) -> [u8; HASH_LEN] {
    hex_de(valor)
        .try_into()
        .unwrap_or_else(|v: Vec<u8>| panic!("hash com {} bytes, esperava {HASH_LEN}", v.len()))
}

fn lista_hex(valor: &Value) -> Vec<Vec<u8>> {
    valor.as_array().expect("esperava lista").iter().map(hex_de).collect()
}

fn folhas(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| format!("folha-{i}").into_bytes()).collect()
}

#[test]
fn escrita_bate_com_o_python() {
    let doc = carregar("codec.json");
    let casos = doc["data"]["encoding"]
        .as_array()
        .expect("codec.json precisa ter data.encoding");
    assert!(casos.len() >= 5, "poucos casos de escrita: {}", casos.len());

    for caso in casos {
        let tipo = caso["kind"].as_str().expect("kind precisa ser string");
        let inteiro = || caso["value"].as_u64().expect("value precisa ser inteiro");
        let mut w = Writer::new();
        match tipo {
            "u8" => w.u8(u8::try_from(inteiro()).expect("cabe em u8")),
            "u16" => w.u16(u16::try_from(inteiro()).expect("cabe em u16")),
            "u32" => w.u32(u32::try_from(inteiro()).expect("cabe em u32")),
            "u64" => w.u64(inteiro()),
            "bytes" => w.var_bytes(&bytes(caso, "value")).expect("tamanho cabe em u32"),
            "str" => w
                .string(caso["value"].as_str().expect("value precisa ser string"))
                .expect("tamanho cabe em u32"),
            outro => panic!("tipo desconhecido no vetor: {outro}"),
        }
        assert_eq!(w.into_bytes(), bytes(caso, "bytes"), "escrita de {tipo}");
    }
    println!("escrita: {} casos batem com o Python", casos.len());
}

#[test]
fn raiz_de_merkle_bate_com_o_python() {
    let doc = carregar("codec.json");
    let casos = doc["data"]["merkle"]
        .as_array()
        .expect("codec.json precisa ter data.merkle");
    assert!(casos.len() >= 6, "poucos casos de Merkle: {}", casos.len());

    for caso in casos {
        let lista = lista_hex(&caso["leaves"]);
        assert_eq!(
            merkle_root(&lista),
            hash64(&caso["root"]),
            "raiz de {} folhas",
            lista.len()
        );
    }
    println!("merkle: {} raízes batem com o Python", casos.len());
}

/// Executa uma operação de leitura do vetor e devolve o valor no mesmo
/// formato que o gerador Python grava.
fn executar(r: &mut Reader<'_>, operacao: &str) -> Result<Value, CodecError> {
    Ok(match operacao {
        "u8" => json!(r.u8()?),
        "u16" => json!(r.u16()?),
        "u32" => json!(r.u32()?),
        "u64" => json!(r.u64()?),
        "var_bytes" => json!(hex::encode(r.var_bytes()?)),
        "string" => json!(hex::encode(r.string()?.as_bytes())),
        "fixed:0" => json!(hex::encode(r.fixed::<0>()?)),
        "fixed:20" => json!(hex::encode(r.fixed::<20>()?)),
        "list_u32" => json!(r.read_list(|rd| rd.u32())?),
        "list_var_bytes" => json!(r.read_list(|rd| rd.var_bytes().map(hex::encode))?),
        "finish" => {
            r.clone().finish()?;
            Value::Null
        }
        outra => panic!("operação desconhecida no vetor: {outra}"),
    })
}

#[test]
fn leitor_aceita_e_recusa_o_mesmo_que_o_python() {
    let doc = carregar("codec_edge.json");
    let casos = doc["data"]["decode"]
        .as_array()
        .expect("codec_edge.json precisa ter data.decode");

    let mut aceitos = 0_usize;
    let mut recusados = 0_usize;
    let mut divergencias: Vec<String> = Vec::new();

    for caso in casos {
        let rotulo = caso["label"].as_str().unwrap_or("?");
        let entrada = bytes(caso, "input");
        let operacoes: Vec<&str> = caso["ops"]
            .as_array()
            .expect("ops precisa ser lista")
            .iter()
            .map(|op| op.as_str().expect("operação precisa ser string"))
            .collect();

        let mut r = Reader::new(&entrada);
        let mut valores: Vec<Value> = Vec::new();
        let mut falha: Option<(usize, String)> = None;
        for (i, op) in operacoes.iter().enumerate() {
            match executar(&mut r, op) {
                Ok(v) => valores.push(v),
                Err(e) => {
                    falha = Some((i, e.to_string()));
                    break;
                }
            }
        }

        let python_aceita = caso["accepted"].as_bool().expect("accepted precisa ser bool");
        if caso["values"].as_array().expect("values precisa ser lista") != &valores {
            divergencias.push(format!(
                "{rotulo}: valores Python {}, Rust {}",
                caso["values"],
                Value::Array(valores.clone())
            ));
        }
        match (&falha, python_aceita) {
            (None, true) => aceitos += 1,
            (Some((i, msg)), false) => {
                recusados += 1;
                let python_i = caso["failed_at"].as_u64().expect("failed_at");
                let python_msg = caso["error"].as_str().expect("error");
                if u64::try_from(*i).unwrap() != python_i || msg != python_msg {
                    divergencias.push(format!(
                        "{rotulo}: Python recusou na operação {python_i} com {python_msg:?}, \
                         Rust na {i} com {msg:?}"
                    ));
                }
            }
            (None, false) => divergencias.push(format!("{rotulo}: Python RECUSOU, Rust aceitou")),
            (Some((_, msg)), true) => {
                divergencias.push(format!("{rotulo}: Python ACEITOU, Rust recusou com {msg:?}"));
            }
        }
    }

    assert!(
        divergencias.is_empty(),
        "{} divergência(s) entre Python e Rust:\n  {}",
        divergencias.len(),
        divergencias.join("\n  ")
    );
    assert!(aceitos >= 18, "poucas leituras aceitas no vetor: {aceitos}");
    assert!(recusados >= 20, "poucas leituras recusadas no vetor: {recusados}");
    println!("leitura: {aceitos} aceitas e {recusados} recusadas batem com o Python");
}

#[test]
fn provas_de_merkle_batem_com_o_python() {
    let doc = carregar("codec_edge.json");
    let casos = doc["data"]["merkle_proofs"]
        .as_array()
        .expect("codec_edge.json precisa ter data.merkle_proofs");

    let mut caminhos = 0_usize;
    for caso in casos {
        let lista = lista_hex(&caso["leaves"]);
        let raiz = hash64(&caso["root"]);
        assert_eq!(merkle_root(&lista), raiz, "raiz de {} folhas", lista.len());

        let esperados = caso["paths"].as_array().expect("paths precisa ser lista");
        let validos = caso["valid"].as_array().expect("valid precisa ser lista");
        let total = u64::try_from(lista.len()).unwrap();
        for (i, folha) in lista.iter().enumerate() {
            let esperado: Vec<[u8; HASH_LEN]> = esperados[i]
                .as_array()
                .expect("caminho precisa ser lista")
                .iter()
                .map(hash64)
                .collect();
            let obtido = merkle_path(&lista, i).expect("índice dentro da faixa");
            assert_eq!(obtido, esperado, "caminho da folha {i} de {}", lista.len());
            assert_eq!(
                merkle_verify_path(folha, u64::try_from(i).unwrap(), total, &obtido, &raiz),
                validos[i].as_bool().expect("valid precisa ser bool"),
                "verificação da folha {i} de {}",
                lista.len()
            );
            caminhos += 1;
        }
    }
    assert!(caminhos >= 70, "poucos caminhos no vetor: {caminhos}");
    println!("merkle: {caminhos} caminhos de prova batem com o Python");
}

#[test]
fn caminho_fora_da_faixa_como_no_python() {
    let doc = carregar("codec_edge.json");
    let casos = doc["data"]["merkle_path_errors"]
        .as_array()
        .expect("codec_edge.json precisa ter data.merkle_path_errors");
    assert!(!casos.is_empty(), "nenhum caso de caminho fora da faixa");

    for caso in casos {
        let total = usize::try_from(caso["total"].as_u64().expect("total")).unwrap();
        let indice = usize::try_from(caso["index"].as_u64().expect("index")).unwrap();
        let resultado = merkle_path(&folhas(total), indice);
        let python_aceita = caso["accepted"].as_bool().expect("accepted precisa ser bool");
        assert_eq!(
            resultado.is_ok(),
            python_aceita,
            "caminho com total {total} e índice {indice}"
        );
        if let Err(e) = resultado {
            assert_eq!(
                e.to_string(),
                caso["error"].as_str().expect("error"),
                "mensagem com total {total} e índice {indice}"
            );
        }
    }
}

#[test]
fn verificacao_de_prova_nos_casos_de_borda_bate_com_o_python() {
    let doc = carregar("codec_edge.json");
    let casos = doc["data"]["merkle_verify"]
        .as_array()
        .expect("codec_edge.json precisa ter data.merkle_verify");

    let mut aceitos = 0_usize;
    let mut recusados = 0_usize;
    let mut divergencias: Vec<String> = Vec::new();
    for caso in casos {
        let rotulo = caso["label"].as_str().unwrap_or("?");
        let caminho: Vec<[u8; HASH_LEN]> = caso["path"]
            .as_array()
            .expect("path precisa ser lista")
            .iter()
            .map(hash64)
            .collect();
        let rust = merkle_verify_path(
            &bytes(caso, "leaf"),
            caso["index"].as_u64().expect("index precisa caber em u64"),
            caso["total"].as_u64().expect("total precisa caber em u64"),
            &caminho,
            &hash64(&caso["root"]),
        );
        let python = caso["valid"].as_bool().expect("valid precisa ser bool");
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
    assert!(aceitos >= 4, "poucas provas aceitas no vetor: {aceitos}");
    assert!(recusados >= 10, "poucas provas recusadas no vetor: {recusados}");
    println!("verificação de prova: {aceitos} aceitas e {recusados} recusadas batem com o Python");
}
