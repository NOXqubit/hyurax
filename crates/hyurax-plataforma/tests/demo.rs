//! Dados de demonstração para ver os painéis com cara de uso real, e agentes
//! simulados que pulsam contra um servidor rodando. Fora da suíte normal:
//!
//! ```text
//! HYX_DEMO_PASTA=... cargo test -p hyurax-plataforma --test demo semear -- --ignored
//! HYX_DEMO_URL=127.0.0.1:8790 HYX_DEMO_MINUTOS=10 cargo test -p hyurax-plataforma --test demo agentes -- --ignored
//! ```
//!
//! Tudo aqui é de teste (domínio `.test`, ambiente de teste, dinheiro de teste).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod comum;

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use comum::Agente;
use hyurax_plataforma::servico::contas::Cadastro;
use hyurax_plataforma::{Opcoes, Plataforma};
use serde_json::json;

fn cadastro(email: &str, nome: &str, cliente: bool, fornecedor: bool) -> Cadastro {
    Cadastro { email: email.into(), nome: nome.into(), senha: "ponte-azul-77-rio".into(), moeda: "BRL".into(), pais: "BR".into(), cliente, fornecedor, aceite: true }
}

fn agente_falso(endereco: &str, semente: u8) -> Agente {
    let segredo: [u8; 32] = hyurax_crypto::sha512(&[semente; 8])[..32].try_into().unwrap();
    Agente { endereco: endereco.to_string(), chave: hyurax_crypto::ed25519_public_key(&segredo), segredo, relogio: Arc::new(AtomicU64::new(0)), no: 0, estraga: false }
}

#[test]
#[ignore = "gera dados de demonstração numa pasta"]
fn semear() {
    let pasta = std::path::PathBuf::from(std::env::var("HYX_DEMO_PASTA").expect("HYX_DEMO_PASTA"));
    let _ = std::fs::remove_dir_all(&pasta);
    let p = Plataforma::abrir(&pasta, Opcoes { modo_teste: true, ..Opcoes::default() }).unwrap().plataforma;
    let senha = p.criar_admin("admin@hyurax.test", "Equipe HYURAX").unwrap();
    std::fs::write(pasta.join("admin-senha-de-teste.txt"), &senha).unwrap();

    let c = p.cadastrar(cadastro("laboratorio@hyurax.test", "Laboratório de Genética Aplicada", true, false), "10.0.0.10", "demo").unwrap();
    let qc = p.por_sessao(&c.token).unwrap();
    p.criar_pagamento(&qc, 20_000).unwrap();

    let mut agentes = Vec::new();
    for (i, (email, nome, pcs)) in [("ana@hyurax.test", "Ana Ferreira", vec!["Estação de trabalho", "PC gamer da sala"]), ("bruno@hyurax.test", "Bruno Lima", vec!["Servidor do escritório"])].iter().enumerate() {
        let f = p.cadastrar(cadastro(email, nome, false, true), &format!("10.0.1.{i}"), "demo").unwrap();
        let qf = p.por_sessao(&f.token).unwrap();
        for (k, pc) in pcs.iter().enumerate() {
            let semente = u8::try_from(40 + i * 10 + k).unwrap();
            let ag = agente_falso("127.0.0.1:1", semente);
            let codigo = p.criar_vinculo(&qf).unwrap()["codigo"].as_str().unwrap().to_string();
            let cpu = ["AMD Ryzen 7 5800X", "Intel Core i5-12400F", "Intel Xeon E-2236"][(i * 2 + k) % 3];
            let r = p
                .registrar_no(
                    ag.chave,
                    serde_json::from_value(json!({ "codigo": codigo, "nome": pc, "versao": "1.5.0",
                        "hardware": { "cpu_modelo": cpu, "nucleos": 8 + 4 * k, "ram_mib": 16384 * (k + 1), "gpu_modelo": if k == 1 { "NVIDIA GeForce RTX 3060" } else { "" }, "gpu_vram_mib": if k == 1 { 12288 } else { 0 }, "disco_livre_mib": 200000, "so": "Windows 11" },
                        "limites": { "linhas": 4, "ram_mib": 8192, "gpu": k == 1 } }))
                    .unwrap(),
                )
                .unwrap();
            agentes.push((r["no"].as_u64().unwrap(), ag));
        }
    }

    // tarefas: genética (vai concluir), rotas e triagem
    let tarefas = [
        json!({ "tipo": "population-genetics", "tamanho": 500, "passos": 200, "parametros": [8, 0, 10100, 50, 1000], "unidades": 12, "paralelo": 4, "prazo_s": 86400, "descricao": "deriva e seleção fraca em 8 loci, N = 500" }),
        json!({ "tipo": "routing", "tamanho": 120, "passos": 400, "parametros": [11], "unidades": 8, "paralelo": 4, "prazo_s": 86400, "descricao": "rotas de coleta, instância 11" }),
        json!({ "tipo": "molecular-screening", "tamanho": 400, "passos": 0, "parametros": [0, 0, 500000, 25000, 5, 10, 10, 140000, 16000], "unidades": 6, "paralelo": 3, "prazo_s": 86400, "descricao": "regra dos cinco e solubilidade sobre o AqSolDB" }),
    ];
    let mut ids = Vec::new();
    for t in tarefas {
        ids.push(p.criar_tarefa(&qc, serde_json::from_value(t).unwrap(), false).unwrap()["id"].as_u64().unwrap());
    }
    // os agentes trabalham até a primeira tarefa fechar (o resto fica para os agentes ao vivo)
    for _ in 0..80 {
        for (_, ag) in &agentes {
            let pulso = p.pulso(ag.chave, serde_json::from_value(json!({ "versao": "1.5.0", "livres": 4, "ram_livre_mib": 8192, "tipos": [1,2,3,4,5,6,7,8], "telemetria": { "cpu": 38.0, "ram_usada_mib": 9000, "temperatura": 62.0, "potencia_w": 85.0 } })).unwrap()).unwrap();
            for a in pulso["atribuicoes"].as_array().unwrap() {
                let _ = p.entregar(ag.chave, &ag.envelope(a));
            }
        }
        p.manutencao().unwrap();
        if p.tarefa(&qc, ids[0]).unwrap()["estado"] == "COMPLETED" {
            break;
        }
    }
    let mut chaves = String::new();
    for (no, ag) in &agentes {
        chaves.push_str(&format!("{no} {}\n", hyurax_plataforma::util::hex(&ag.segredo)));
    }
    std::fs::write(pasta.join("agentes-de-teste.txt"), chaves).unwrap();
    println!("demo pronta em {} (admin: admin@hyurax.test)", pasta.display());
}

#[test]
#[ignore = "agentes simulados contra um servidor rodando"]
fn agentes() {
    let endereco = std::env::var("HYX_DEMO_URL").expect("HYX_DEMO_URL");
    let minutos: u64 = std::env::var("HYX_DEMO_MINUTOS").ok().and_then(|x| x.parse().ok()).unwrap_or(5);
    let fim = std::time::Instant::now() + std::time::Duration::from_secs(minutos * 60);
    let sementes: Vec<u8> = vec![40, 41, 50];
    let agentes: Vec<Agente> = sementes.iter().map(|&s| agente_falso(&endereco, s)).collect();
    let mut volta = 0u64;
    while std::time::Instant::now() < fim {
        for (i, ag) in agentes.iter().enumerate() {
            let carga = 20.0 + ((volta * 7 + i as u64 * 13) % 50) as f64;
            let corpo = json!({ "versao": "1.5.0", "livres": 4, "ram_livre_mib": 8192, "tipos": [1,2,3,4,5,6,7,8], "gpu": i == 1,
                "telemetria": { "cpu": carga, "ram_usada_mib": 8000 + (volta % 9) * 100, "temperatura": 55.0 + carga / 4.0, "potencia_w": 60.0 + carga, "rede_bytes": 4000 } });
            let r = ag.assinado("POST", "/api/v1/agente/pulso", corpo.to_string().as_bytes());
            if r.status == 200 {
                for a in r.json()["atribuicoes"].as_array().cloned().unwrap_or_default() {
                    ag.executar_e_entregar(&a);
                }
            }
        }
        volta += 1;
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
