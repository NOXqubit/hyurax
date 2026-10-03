//! A API externa de ponta a ponta (Documento Mestre, fase 5): um núcleo de
//! verdade, contas de cliente e pedidos HTTP pelo socket, como um programa
//! de outro computador faria.
//!
//! Confere: chave de acesso (e revogação), créditos (o orçamento do JOB é o
//! saldo), JOB de uma conta invisível para outra, relatório e dados brutos,
//! e que nada responde com a API desligada.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use hyurax_nucleo::config::{ConfigDoNo, rede_do_nome};
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::servico::{Modo, Nucleo, Partida};
use serde_json::Value;

const INTERFACE: hyurax_nucleo::api::Arquivos = &[("/index.html", "text/html", b"<p>HYURAX</p>")];

fn ligar(nome: &str) -> (Arc<Nucleo>, u16, std::path::PathBuf) {
    let pasta = std::env::temp_dir().join(format!("hyurax-api-externa-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    let config = ConfigDoNo::nova(rede_do_nome("regtest").unwrap(), Pastas::unica(&pasta), 0, Vec::new(), true);
    let n = Nucleo::ligar(Partida {
        modo: Modo::Terminal,
        config,
        arquivo_carteira: None,
        endereco_fixo: None,
        linhas_padrao: 1,
        pausa_fixa_ms: 0,
        painel_na_rede: false,
    })
    .unwrap();
    let porta = hyurax_nucleo::api::abrir(&n, 0, INTERFACE).unwrap();
    (n, porta, pasta)
}

fn pedido(porta: u16, metodo: &str, caminho: &str, chave: Option<&str>, corpo: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
    let mut texto = format!("{metodo} {caminho} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{porta}\r\nContent-Length: {}\r\n", corpo.len());
    if let Some(c) = chave {
        texto.push_str(&format!("Authorization: Bearer {c}\r\n"));
    }
    texto.push_str("Content-Type: application/x-www-form-urlencoded\r\n\r\n");
    texto.push_str(corpo);
    s.write_all(texto.as_bytes()).unwrap();
    let mut resposta = String::new();
    let _ = s.read_to_string(&mut resposta);
    let status = resposta.split(' ').nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
    let corpo = resposta.split_once("\r\n\r\n").map(|(_, c)| c.to_string()).unwrap_or_default();
    (status, corpo)
}

fn json(t: &str) -> Value {
    serde_json::from_str(t).expect(t)
}

#[test]
fn contas_creditos_e_isolamento_pela_api_externa() {
    let (n, porta, pasta) = ligar("ponta");
    let (lab, chave) = n.contas.criar("Laboratório", 50, 1).unwrap();
    let (_, chave_outra) = n.contas.criar("Outra", 50, 1).unwrap();

    // desligada: nem com chave
    assert_eq!(pedido(porta, "GET", "/api/v1/externa/conta", Some(&chave), "").0, 403);
    n.api_externa.store(true, Ordering::Relaxed);
    n.ultrax.ligar(true);

    // sem chave, chave errada
    assert_eq!(pedido(porta, "GET", "/api/v1/externa/conta", None, "").0, 401);
    assert_eq!(pedido(porta, "GET", "/api/v1/externa/conta", Some(&"0".repeat(64)), "").0, 401);
    // a chave de conta não abre o resto do painel
    assert_eq!(pedido(porta, "GET", "/api/v1/estado", Some(&chave), "").0, 401);

    let (s, c) = pedido(porta, "GET", "/api/v1/externa/conta", Some(&chave), "");
    assert_eq!(s, 200, "{c}");
    let c = json(&c);
    assert_eq!((c["conta"].as_u64(), c["saldo_milicreditos"].as_u64()), (Some(u64::from(lab.id)), Some(50)));

    let form = "dominio=7&tipo=1&tamanho=64&unidades=2&nivel=2&descricao=teste+externo";
    let (s, e) = pedido(porta, "POST", "/api/v1/externa/estimar", Some(&chave), form);
    assert_eq!(s, 200, "{e}");

    // o orçamento do JOB é o saldo da conta
    let (s, j) = pedido(porta, "POST", "/api/v1/externa/jobs", Some(&chave), form);
    assert_eq!(s, 200, "{j}");
    let j = json(&j);
    let id = j["id"].as_str().unwrap().to_string();
    assert_eq!(j["orcamento_milicreditos"].as_u64(), Some(50));

    let lista = json(&pedido(porta, "GET", "/api/v1/externa/jobs", Some(&chave), "").1);
    assert_eq!(lista["jobs"][0]["id"].as_str(), Some(id.as_str()));
    // a outra conta não vê o JOB, nem sabe que ele existe
    assert_eq!(json(&pedido(porta, "GET", "/api/v1/externa/jobs", Some(&chave_outra), "").1)["jobs"].as_array().map(Vec::len), Some(0));
    assert_eq!(pedido(porta, "GET", &format!("/api/v1/externa/jobs/{id}"), Some(&chave_outra), "").0, 404);
    assert_eq!(pedido(porta, "POST", &format!("/api/v1/externa/jobs/{id}/cancelar"), Some(&chave_outra), "").0, 404);

    // espera o JOB terminar e baixa o relatório e os dados brutos
    let ate = Instant::now() + Duration::from_secs(240);
    loop {
        let x = json(&pedido(porta, "GET", "/api/v1/externa/jobs", Some(&chave), "").1);
        let estado = x["jobs"][0]["estado"].as_str().unwrap_or("").to_string();
        if ["COMPLETED", "OUT OF BUDGET", "CANCELLED", "EXPIRED"].iter().any(|e| estado.eq_ignore_ascii_case(e)) {
            break;
        }
        assert!(Instant::now() < ate, "o JOB não terminou: {x}");
        std::thread::sleep(Duration::from_millis(500));
    }
    let (s, r) = pedido(porta, "GET", &format!("/api/v1/externa/jobs/{id}/relatorio.json"), Some(&chave), "");
    assert_eq!(s, 200);
    assert!(json(&r).is_object());
    let (s, u) = pedido(porta, "GET", &format!("/api/v1/externa/jobs/{id}/unidades"), Some(&chave), "");
    assert_eq!(s, 200);
    assert!(u.lines().count() >= 1, "uma linha por unidade feita");

    // o consumo sai do saldo
    let c = json(&pedido(porta, "GET", "/api/v1/externa/conta", Some(&chave), "").1);
    let consumido = c["consumido_milicreditos"].as_u64().unwrap();
    assert_eq!(c["saldo_milicreditos"].as_u64(), Some(50u64.saturating_sub(consumido)));

    // revogada: a chave para de valer na hora
    n.contas.revogar(lab.id).unwrap();
    assert_eq!(pedido(porta, "GET", "/api/v1/externa/conta", Some(&chave), "").0, 401);

    n.encerrar();
    let _ = std::fs::remove_dir_all(pasta);
}
