// ✝ Joel 2:25 — “Restituir-vos-ei os anos que comeu o gafanhoto.”
//! Recuperação (seção 44): o servidor cai no meio do trabalho e volta
//! exatamente como estava, porque o estado é o diário relido. A unidade que
//! estava com o agente continua valendo depois da volta (o prazo é um
//! horário gravado, não um cronômetro na memória).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod comum;

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use comum::{Agente, relogio};
use hyurax_plataforma::estado::Estado;
use hyurax_plataforma::servico::contas::Cadastro;
use hyurax_plataforma::{Opcoes, Plataforma};
use serde_json::json;

fn abrir(pasta: &std::path::Path, adianto: &Arc<AtomicU64>) -> Arc<Plataforma> {
    Plataforma::abrir_com_relogio(pasta, Opcoes { modo_teste: true, ..Opcoes::default() }, relogio(adianto)).unwrap().plataforma
}

fn cadastro(email: &str, cliente: bool, fornecedor: bool) -> Cadastro {
    Cadastro { email: email.into(), nome: "Teste".into(), senha: "ponte-azul-77-rio".into(), moeda: "BRL".into(), pais: "BR".into(), cliente, fornecedor, aceite: true }
}

#[test]
fn servidor_cai_e_volta_igual() {
    let pasta = comum::pasta("recuperacao");
    let adianto = Arc::new(AtomicU64::new(0));
    let ag = {
        // um agente que só serve para assinar e calcular (a plataforma é chamada direto)
        let s = comum::Servidor { p: abrir(&comum::pasta("recuperacao-aux"), &adianto), endereco: String::new(), adianto: Arc::clone(&adianto), pasta: pasta.clone() };
        Agente::novo(&s, 3)
    };
    let p = abrir(&pasta, &adianto);
    let c = p.cadastrar(cadastro("c@exemplo.com", true, false), "10.0.0.1", "teste").unwrap();
    let qc = p.por_sessao(&c.token).unwrap();
    p.criar_pagamento(&qc, 3000).unwrap();
    let f = p.cadastrar(cadastro("f@exemplo.com", false, true), "10.0.0.2", "teste").unwrap();
    let qf = p.por_sessao(&f.token).unwrap();
    let codigo = p.criar_vinculo(&qf).unwrap()["codigo"].as_str().unwrap().to_string();
    p.registrar_no(ag.chave, serde_json::from_value(json!({ "codigo": codigo, "nome": "pc", "versao": "1.5.0", "limites": { "linhas": 2, "ram_mib": 2048, "gpu": false } })).unwrap()).unwrap();
    let t = p.criar_tarefa(&qc, serde_json::from_value(json!({ "tipo": "matrix", "tamanho": 12, "unidades": 3, "paralelo": 2, "prazo_s": 3600 })).unwrap(), false).unwrap()["id"].as_u64().unwrap();
    let pulso = p.pulso(ag.chave, serde_json::from_value(json!({ "versao": "1.5.0", "livres": 2, "ram_livre_mib": 2048, "tipos": [1] })).unwrap()).unwrap();
    let lista = pulso["atribuicoes"].as_array().unwrap().clone();
    assert_eq!(lista.len(), 2);
    p.entregar(ag.chave, &ag.envelope(&lista[0])).unwrap();
    let antes = p.ler(Estado::impressao).unwrap();
    // queda: a plataforma some sem fechar nada
    drop(p);

    let p = abrir(&pasta, &adianto);
    assert_eq!(p.ler(Estado::impressao).unwrap(), antes, "reler o diário dá o mesmo estado");
    // a sessão do cliente continua (está no diário, só o hash do token)
    let qc = p.por_sessao(&c.token).unwrap();
    assert_eq!(p.tarefa(&qc, t).unwrap()["progresso"]["concluidas"], 1);
    // a unidade que estava com o agente é entregue depois da volta
    let r = p.entregar(ag.chave, &ag.envelope(&lista[1])).unwrap();
    assert_eq!(r["aceito"], true, "{r}");
    assert_eq!(p.tarefa(&qc, t).unwrap()["progresso"]["concluidas"], 2);
    // o diário confere inteiro e o livro fecha
    let l = hyurax_plataforma::diario::ler(&pasta.join("diario.hyx")).unwrap();
    assert!(!l.cauda_cortada);
    assert_eq!(p.ler(|e| e.livro.soma(hyurax_plataforma::dinheiro::Moeda::Brl)).unwrap(), 0);
}

#[test]
fn queda_no_meio_da_escrita_corta_so_a_ultima_linha() {
    let pasta = comum::pasta("queda");
    let adianto = Arc::new(AtomicU64::new(0));
    {
        let p = abrir(&pasta, &adianto);
        p.cadastrar(cadastro("a@exemplo.com", true, false), "10.0.0.1", "teste").unwrap();
    }
    // meia linha no fim, como se a luz tivesse acabado no meio do write
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new().append(true).open(pasta.join("diario.hyx")).unwrap();
    f.write_all(b"{\"n\":3,\"t\":1,\"ev\":{\"evento\":\"LOG").unwrap();
    drop(f);
    let a = Plataforma::abrir(&pasta, Opcoes::default()).unwrap();
    assert!(a.cauda_cortada);
    assert_eq!(a.eventos, 2);
    assert_eq!(a.plataforma.ler(|e| e.usuarios.len()).unwrap(), 1);
}
