//! A nuvem entre nós de verdade: núcleos completos, cada um com a sua pasta e
//! a sua identidade, ligados pela rede P2P real (Noise sobre TCP) em
//! 127.0.0.1. Nada é injetado: as mensagens passam pelos sockets.
//!
//! 1. Armazenamento: A guarda um arquivo em 3 nós (2 de dados + 1 de
//!    paridade), cifrado; os guardiões não veem o conteúdo; os desafios de
//!    guarda conferem; um guardião sai da rede, o fragmento dele é dado como
//!    perdido, reconstruído e mandado para um quarto nó; o arquivo volta
//!    inteiro e igual.
//! 2. Mercado e aluguel: B anuncia a máquina; A vê o anúncio, aluga, e o JOB
//!    só manda unidades para B; cada unidade é refeita em A; A assina
//!    recibos; o livro de A tem o consumo e a partilha, o de B o recibo.
//!
//! O que isto NÃO prova: máquinas diferentes, rede de verdade (latência,
//! perda, NAT). Isso continua PENDENTE para o ensaio entre máquinas.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use hyurax_nucleo::ciencia::EstadoDoJob;
use hyurax_nucleo::config::{ConfigDoNo, rede_do_nome};
use hyurax_nucleo::nuvem::AjustesDaNuvem;
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::servico::{Modo, Nucleo, Partida};
use hyurax_ultrax::job::{Dominio, Nivel, PedidoDeJob};
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho};

fn ligar(nome: &str) -> (Arc<Nucleo>, u16, std::path::PathBuf) {
    let pasta = std::env::temp_dir().join(format!("hyurax-nuvem-{nome}-{}", std::process::id()));
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
    (n, porta, pasta)
}

fn esperar(ate: Instant, o_que: &str, mut feito: impl FnMut() -> bool) {
    while !feito() {
        assert!(Instant::now() < ate, "não aconteceu a tempo: {o_que}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn estado(n: &Arc<Nucleo>) -> serde_json::Value {
    let c = Arc::clone(&n.ciencia);
    n.nuvem.json(&move |w| c.reputacao_de(w))
}

fn guardiao(n: &Arc<Nucleo>) {
    n.nuvem
        .mudar_ajustes(AjustesDaNuvem { anunciar: true, disco_mib: 16, preco_gb_mes_mili: 40_000, ..AjustesDaNuvem::default() })
        .unwrap();
}

#[test]
#[ignore = "sobe cinco núcleos completos; rode com --ignored"]
fn arquivo_cifrado_em_tres_nos_sobrevive_a_um_que_sai() {
    let (a, porta_a, pasta_a) = ligar("a");
    let mut outros = Vec::new();
    for nome in ["b", "c", "d", "e"] {
        let (n, _, p) = ligar(nome);
        guardiao(&n);
        n.rede.conectar(("127.0.0.1", porta_a)).unwrap();
        outros.push((n, p));
    }
    // o dono confere rápido e dá o guardião ausente por perdido em 3 s
    a.nuvem
        .mudar_ajustes(AjustesDaNuvem { desafio_s: 1, perda_s: 3, ..AjustesDaNuvem::default() })
        .unwrap();
    let ate = Instant::now() + Duration::from_secs(240);
    esperar(ate, "A ver os quatro anúncios", || estado(&a)["mercado"].as_array().map_or(0, Vec::len) >= 4);

    // um arquivo com um marcador em claro que não pode aparecer nos guardiões
    let marcador = b"CONTEUDO-SECRETO-DO-DONO";
    let mut dados = Vec::new();
    for i in 0..40_000u32 {
        dados.extend_from_slice(marcador);
        dados.extend_from_slice(&i.to_be_bytes());
    }
    let id = a.nuvem.guardar_arquivo("relatorio.bin", &dados, 2, 1).unwrap();
    let id_hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
    let arquivo = |a: &Arc<Nucleo>| estado(a)["arquivos"].as_array().unwrap().iter().find(|x| x["arquivo"] == id_hex.as_str()).cloned().unwrap();
    esperar(ate, "os 3 fragmentos guardados", || arquivo(&a)["saude"] == "integro");

    // os guardiões têm bytes, mas nenhum tem o conteúdo em claro
    let mut com_fragmento = 0;
    for (_, p) in &outros {
        for e in walk(&p.join("nuvem").join("guarda")) {
            if e.extension().is_some_and(|x| x == "frag") {
                com_fragmento += 1;
                let b = std::fs::read(&e).unwrap();
                assert!(!b.windows(marcador.len()).any(|w| w == marcador), "o guardião viu o conteúdo em claro");
            }
        }
    }
    assert_eq!(com_fragmento, 3);

    // os desafios de guarda passam (cada um consome um desafio pré-calculado)
    esperar(ate, "desafios respondidos", || {
        arquivo(&a)["fragmentos"].as_array().unwrap().iter().all(|f| f["desafios_restantes"].as_u64().unwrap() < 8 && f["falhas"] == 0)
    });

    // um guardião sai da rede: o fragmento dele é dado como perdido e reparado
    let guardioes: Vec<String> = arquivo(&a)["fragmentos"].as_array().unwrap().iter().map(|f| f["guardiao"].as_str().unwrap().to_string()).collect();
    let quem_sai = outros.iter().position(|(n, _)| guardioes.contains(&n.nuvem.identidade.iter().map(|b| format!("{b:02x}")).collect::<String>())).unwrap();
    let (saiu, pasta_saiu) = outros.remove(quem_sai);
    let id_saiu: String = saiu.nuvem.identidade.iter().map(|b| format!("{b:02x}")).collect();
    saiu.encerrar();
    drop(saiu);
    esperar(ate, "o fragmento do que saiu ir para outro nó", || {
        let x = arquivo(&a);
        x["saude"] == "integro" && x["fragmentos"].as_array().unwrap().iter().all(|f| f["guardiao"] != id_saiu.as_str())
    });

    // o arquivo volta inteiro e igual
    a.nuvem.recuperar(&id).unwrap();
    esperar(ate, "arquivo recuperado", || a.nuvem.recuperado(&id).is_some());
    let (nome, volta) = a.nuvem.recuperado(&id).unwrap();
    assert_eq!(nome, "relatorio.bin");
    assert_eq!(volta, dados);
    // o livro de A lançou o armazenamento pago pelas provas (ou ainda não deu 1 milicrédito)
    assert!(a.nuvem.livro.quebra().is_none());

    for (n, p) in outros {
        n.encerrar();
        drop(n);
        let _ = std::fs::remove_dir_all(p);
    }
    a.encerrar();
    drop(a);
    let _ = std::fs::remove_dir_all(pasta_a);
    let _ = std::fs::remove_dir_all(pasta_saiu);
}

fn walk(p: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if let Ok(l) = std::fs::read_dir(p) {
        for e in l.flatten() {
            let c = e.path();
            if c.is_dir() {
                v.extend(walk(&c));
            } else {
                v.push(c);
            }
        }
    }
    v
}

#[test]
#[ignore = "sobe dois núcleos completos e calcula; rode com --ignored"]
fn aluguel_de_capacidade_com_recibo_e_livro() {
    let (a, porta_a, pasta_a) = ligar("aluga-a");
    let (b, _, pasta_b) = ligar("aluga-b");
    b.nuvem
        .mudar_ajustes(AjustesDaNuvem { anunciar: true, preco_credito_mili: 1_500, descricao: "teste de aluguel".into(), ..AjustesDaNuvem::default() })
        .unwrap();
    for n in [&a, &b] {
        n.ultrax.ligar(true);
        n.ciencia.aceitar_da_rede(true);
    }
    b.rede.conectar(("127.0.0.1", porta_a)).unwrap();
    let ate = Instant::now() + Duration::from_secs(300);
    let worker_b: [u8; 32] = b.nuvem.worker;
    esperar(ate, "A ver o anúncio de B", || a.nuvem.anuncio_para_alugar(&worker_b).is_ok());
    // e a oferta do ULTRAX de B (sai a cada 10 s)
    esperar(ate, "A receber a oferta de B", || {
        let r: serde_json::Value = serde_json::from_str(&a.ciencia.json_rede()).unwrap();
        r["ofertas"].as_array().map_or(0, Vec::len) >= 1
    });
    let anuncio = a.nuvem.anuncio_para_alugar(&worker_b).unwrap();
    assert_eq!(anuncio.preco_credito_mili, 1_500);
    let id = a
        .ciencia
        .submeter(PedidoDeJob {
            dominio: Dominio::Matematica,
            // 128 × 128: ~4 milhões de operações por unidade (4 milicréditos)
            modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 128, 0).unwrap(),
            unidades: 3,
            nivel: Nivel::Concordancia,
            redundancia: 2,
            prazo_s: 0,
            orcamento_milicreditos: 0,
            descricao: "aluguel".into(),
        })
        .unwrap();
    a.ciencia.restringir_a(id, anuncio.worker);
    a.nuvem.registrar_aluguel(id, &anuncio);
    esperar(ate, "JOB alugado concluir", || a.ciencia.situacao(&id).is_some_and(|s| s.estado == EstadoDoJob::Concluido));
    esperar(ate, "recibo chegar a B", || b.nuvem.livro.totais().get("recibo").copied().unwrap_or(0) > 0);
    let al = &estado(&a)["alugueis"][0];
    assert!(al["creditos_mili"].as_u64().unwrap() > 0, "créditos verificados do fornecedor");
    esperar(ate, "consumo e partilha no livro de A", || {
        let t = a.nuvem.livro.totais();
        t.get("consumo").copied().unwrap_or(0) > 0 && t.get("provedor").copied().unwrap_or(0) > 0
    });
    let t = a.nuvem.livro.totais();
    assert_eq!(t["consumo"], t["provedor"] + t["plataforma"] + t["reserva"], "a partilha soma o consumo");
    assert!(a.nuvem.livro.quebra().is_none() && b.nuvem.livro.quebra().is_none());
    for (n, p) in [(a, pasta_a), (b, pasta_b)] {
        n.encerrar();
        drop(n);
        let _ = std::fs::remove_dir_all(p);
    }
}

#[test]
#[ignore = "sobe dois núcleos completos; rode com --ignored"]
fn anuncio_de_maquina_chega_ao_outro_no() {
    let (a, porta_a, pasta_a) = ligar("anuncio-a");
    let (b, _, pasta_b) = ligar("anuncio-b");
    guardiao(&b);
    b.rede.conectar(("127.0.0.1", porta_a)).unwrap();
    let ate = Instant::now() + Duration::from_secs(240);
    esperar(ate, "conectar", || a.rede.pares_conectados() >= 1);
    esperar(ate, "A ver o anúncio de B", || estado(&a)["mercado"].as_array().map_or(0, Vec::len) == 1);
    let m = &estado(&a)["mercado"][0];
    assert_eq!(m["disco_mib"], 16);
    assert_eq!(m["conectado"], true);
    assert_eq!(m["por_1000_creditos"]["total_mili"], 1_000_000);
    for (n, p) in [(a, pasta_a), (b, pasta_b)] {
        n.encerrar();
        drop(n);
        let _ = std::fs::remove_dir_all(p);
    }
}

