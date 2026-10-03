//! Computação científica entre nós de verdade (Documento Mestre, fase 6):
//! três núcleos completos, cada um com a sua pasta, a sua identidade e a sua
//! cadeia, ligados pela rede P2P real (Noise sobre TCP) em 127.0.0.1.
//!
//! O nó A pede um JOB de nível 3 (redundância 3): cada unidade é calculada
//! aqui e nos nós B e C, com compromisso assinado, revelação, maioria e
//! conferência local. Nada é injetado: as mensagens passam pelos sockets.
//!
//! Depois, o nó C cai no meio de um JOB: as unidades fecham com A e B (2 de
//! 3), sem esperar o prazo do nó que sumiu e sem pesar na reputação dele.
//!
//! O que isto NÃO prova: máquinas diferentes, rede de verdade (latência,
//! perda, NAT). Isso continua PENDENTE para o ensaio entre máquinas.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use hyurax_nucleo::ciencia::EstadoDoJob;
use hyurax_nucleo::config::{ConfigDoNo, rede_do_nome};
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::servico::{Modo, Nucleo, Partida};
use hyurax_ultrax::job::{Dominio, Nivel, PedidoDeJob};
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho};

fn ligar(nome: &str) -> (Arc<Nucleo>, u16, std::path::PathBuf) {
    let pasta = std::env::temp_dir().join(format!("hyurax-tres-nos-{nome}-{}", std::process::id()));
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
    let porta = n.rede.escutar(("127.0.0.1", 0)).unwrap();
    // o ULTRAX ligado com uma linha, aceitando trabalho de outros nós
    n.ultrax.ligar(true);
    n.ciencia.aceitar_da_rede(true);
    (n, porta, pasta)
}

fn esperar(ate: Instant, o_que: &str, mut feito: impl FnMut() -> bool) {
    while !feito() {
        assert!(Instant::now() < ate, "não aconteceu a tempo: {o_que}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
#[ignore = "sobe três núcleos completos e leva dezenas de segundos; rode com --ignored"]
fn job_de_nivel_3_entre_tres_nos_pelos_sockets() {
    let (a, porta_a, pasta_a) = ligar("a");
    let (b, _, pasta_b) = ligar("b");
    let (c, _, pasta_c) = ligar("c");
    b.rede.conectar(("127.0.0.1", porta_a)).unwrap();
    c.rede.conectar(("127.0.0.1", porta_a)).unwrap();
    let ate = Instant::now() + Duration::from_secs(240);
    esperar(ate, "A ver os dois pares", || a.rede.pares_conectados() >= 2);
    // as ofertas assinadas saem a cada 10 s
    esperar(ate, "A receber as duas ofertas", || {
        let r: serde_json::Value = serde_json::from_str(&a.ciencia.json_rede()).unwrap();
        r["ofertas"].as_array().map_or(0, Vec::len) >= 2
    });

    let id = a
        .ciencia
        .submeter(PedidoDeJob {
            dominio: Dominio::Matematica,
            modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 16, 0).unwrap(),
            unidades: 3,
            nivel: Nivel::Concordancia,
            redundancia: 3,
            prazo_s: 0,
            orcamento_milicreditos: 0,
            descricao: "três nós de verdade".into(),
        })
        .unwrap();
    esperar(ate, "o JOB concluir", || a.ciencia.situacao(&id).is_some_and(|s| s.estado.e_final()));
    let s = a.ciencia.situacao(&id).unwrap();
    assert_eq!(s.estado, EstadoDoJob::Concluido, "{}", s.motivo);
    assert_eq!(s.feitas, 3);

    // as três unidades por consenso de 3, com a evidência gravada
    let pasta_do_job = a.ciencia.pasta_do_job(&id);
    let unidades = std::fs::read_to_string(pasta_do_job.join("unidades.jsonl")).unwrap();
    assert_eq!(unidades.lines().filter(|l| l.contains("CONSENSUS")).count(), 3, "{unidades}");
    let consenso = std::fs::read_to_string(pasta_do_job.join("consenso.jsonl")).unwrap();
    // um voto aceito por linha (3 nós × 3 unidades), cada um com o registro
    // de prova e, os de fora, com a assinatura do worker
    let votos: Vec<serde_json::Value> = consenso.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(votos.len(), 9, "{consenso}");
    for i in 0..3u64 {
        assert_eq!(votos.iter().filter(|v| v["indice"] == i).count(), 3, "três votos na unidade {i}");
    }
    assert!(votos.iter().filter(|v| v["assinatura"].is_string()).count() >= 6, "os votos de B e C assinados");

    // A conferiu e passou a conhecer B e C: reputação de verdade, gravada
    let r: serde_json::Value = serde_json::from_str(&a.ciencia.json_rede()).unwrap();
    let verificadas: Vec<u64> = r["reputacao"].as_array().unwrap().iter().map(|w| w["verificadas"].as_u64().unwrap()).collect();
    assert!(verificadas.iter().filter(|v| **v >= 3).count() >= 2, "B e C com 3 unidades verificadas cada: {verificadas:?}");
    // e B e C fizeram as contas pedidas por A
    for n in [&b, &c] {
        let r: serde_json::Value = serde_json::from_str(&n.ciencia.json_rede()).unwrap();
        assert!(r["mensagens_recebidas"].as_u64().unwrap() > 0);
    }

    // FALHA: C cai no meio do JOB, depois de receber as unidades. As unidades
    // que estavam com ele falham na hora (sem esperar o prazo de 2 min), sem
    // pesar na reputação dele (cair não é fraude), e fecham com A e B: os dois
    // concordam, que é a maioria exigida (2 de 3).
    let id2 = a
        .ciencia
        .submeter(PedidoDeJob {
            dominio: Dominio::Matematica,
            modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 16, 0).unwrap(),
            unidades: 2,
            nivel: Nivel::Concordancia,
            redundancia: 3,
            prazo_s: 0,
            orcamento_milicreditos: 0,
            descricao: "um nó cai no meio".into(),
        })
        .unwrap();
    esperar(ate + Duration::from_secs(60), "A mandar as duas unidades para fora", || {
        let r: serde_json::Value = serde_json::from_str(&a.ciencia.json_rede()).unwrap();
        r["redundantes"].as_u64() == Some(2)
    });
    c.rede.desligar();
    let inicio = Instant::now();
    esperar(Instant::now() + Duration::from_secs(240), "o JOB fechar sem C", || a.ciencia.situacao(&id2).is_some_and(|s| s.estado.e_final()));
    let s2 = a.ciencia.situacao(&id2).unwrap();
    assert_eq!((s2.estado, s2.feitas), (EstadoDoJob::Concluido, 2), "{}", s2.motivo);
    assert!(inicio.elapsed() < Duration::from_secs(110), "esperou o prazo do nó que caiu: {:?}", inicio.elapsed());
    let unidades2 = std::fs::read_to_string(a.ciencia.pasta_do_job(&id2).join("unidades.jsonl")).unwrap();
    // dois votos entregues (A e B), os dois iguais: "CONSENSUS 2/2"; a falha
    // de C fica no diagnóstico do evento
    assert_eq!(unidades2.lines().filter(|l| l.contains("\"verificacao\":\"CONSENSUS 2/2\"")).count(), 2, "{unidades2}");
    let r: serde_json::Value = serde_json::from_str(&a.ciencia.json_rede()).unwrap();
    assert!(r["reputacao"].as_array().unwrap().iter().all(|w| w["recusadas"].as_u64() == Some(0)), "cair não é fraude: {r}");

    // e sem C, um JOB que exige dois workers de fora espera nós em vez de
    // baixar a exigência
    esperar(Instant::now() + Duration::from_secs(60), "A perceber que C caiu", || a.rede.pares_conectados() == 1);

    for (n, p) in [(a, pasta_a), (b, pasta_b), (c, pasta_c)] {
        n.encerrar();
        drop(n);
        let _ = std::fs::remove_dir_all(p);
    }
}
