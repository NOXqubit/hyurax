// ✝ Provérbios 22:28 — “Não removas os limites antigos que fizeram teus pais.”
//! O endereço Bech32m conferido contra `vectors/enderecos.json`, gerado por
//! `reference/hyurax/endereco.py`: textos nas três redes, entradas aceitas e
//! recusas com a mensagem exata. O app do celular confere o mesmo arquivo.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use hyurax_nucleo::carteira::endereco;
use serde_json::Value;

fn carregar() -> Value {
    let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "enderecos.json"].iter().collect();
    let bruto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("não consegui ler {}: {e}", caminho.display()));
    serde_json::from_str::<Value>(&bruto).unwrap()["data"].clone()
}

fn bytes20(t: &str) -> [u8; 20] {
    let v: Vec<u8> = (0..t.len()).step_by(2).map(|i| u8::from_str_radix(&t[i..i + 2], 16).unwrap()).collect();
    v.try_into().unwrap()
}

#[test]
fn enderecos_batem_com_o_gabarito() {
    let d = carregar();
    let validos = d["validos"].as_array().unwrap();
    assert!(validos.len() >= 7);
    for v in validos {
        let e = bytes20(v["endereco"].as_str().unwrap());
        for rede in ["mainnet", "testnet", "regtest"] {
            let nome = format!("hyurax-{rede}");
            let texto = v[rede].as_str().unwrap();
            assert_eq!(endereco::mostrar(&e, &nome), texto);
            assert_eq!(endereco::ler(texto, &nome), Ok(e));
        }
    }
    for a in d["aceitos"].as_array().unwrap() {
        assert_eq!(endereco::ler(a["texto"].as_str().unwrap(), a["rede"].as_str().unwrap()), Ok(bytes20(a["endereco"].as_str().unwrap())), "{a}");
    }
    let recusas = d["recusas"].as_array().unwrap();
    assert!(recusas.len() >= 10);
    for r in recusas {
        let erro = endereco::ler(r["texto"].as_str().unwrap(), r["rede"].as_str().unwrap()).unwrap_err();
        assert_eq!(erro, r["erro"].as_str().unwrap(), "{r}");
    }
}
