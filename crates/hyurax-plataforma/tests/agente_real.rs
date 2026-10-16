// ✝ Eclesiastes 9:10 — “Tudo quanto te vier à mão para fazer, faze-o conforme as tuas forças.”
//! O agente de verdade contra o servidor de verdade: um núcleo completo do
//! programa (o mesmo da janela e do `hyurax-no agente`), com o worker do
//! ULTRAX, vinculado pelo código do painel. Nada é simulado do lado do
//! fornecedor: pulso, telemetria da máquina, execução, prova assinada e
//! entrega saem do `hyurax-nucleo`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod comum;

use std::time::{Duration, Instant};

use comum::{Navegador, servidor};
use hyurax_nucleo::config::{ConfigDoNo, rede_do_nome};
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::servico::{Modo, Nucleo, Partida};
use serde_json::json;

#[test]
#[ignore = "sobe um núcleo completo do programa e leva dezenas de segundos; rode com --ignored"]
fn nucleo_do_programa_calcula_tarefa_da_plataforma() {
    let s = servidor("agente-real", true);
    let mut forn = Navegador::novo(&s);
    forn.cadastrar("fornecedor@exemplo.com", false, true);
    let codigo = forn.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap().to_string();
    let mut cliente = Navegador::novo(&s);
    cliente.cadastrar("cliente@exemplo.com", true, false);
    cliente.post("/api/v1/pagamentos", json!({ "valor_centavos": 2000 }));

    // o núcleo do programa, numa pasta própria, na rede de teste local
    let pasta = comum::pasta("agente-real-nucleo");
    std::fs::create_dir_all(&pasta).unwrap();
    let config = ConfigDoNo::nova(rede_do_nome("regtest").unwrap(), Pastas::unica(&pasta), 0, Vec::new(), true);
    let n = Nucleo::ligar(Partida { modo: Modo::Terminal, config, arquivo_carteira: None, endereco_fixo: None, linhas_padrao: 1, pausa_fixa_ms: 0, painel_na_rede: false }).unwrap();
    let v = n.plataforma.vincular(&format!("http://{}", s.endereco), &codigo, "núcleo de teste", 1, 512, false).unwrap();
    let no = v["no"].as_u64().unwrap();
    assert!(n.plataforma.config().ligado, "vincular já liga o fornecimento");

    let t = cliente.post("/api/v1/tarefas", json!({ "tipo": "matrix", "tamanho": 24, "unidades": 3, "paralelo": 1, "prazo_s": 600 })).json()["id"].as_u64().unwrap();
    let ate = Instant::now() + Duration::from_secs(240);
    loop {
        s.p.manutencao().unwrap();
        let tj = cliente.get(&format!("/api/v1/tarefas/{t}")).json();
        if tj["estado"] == "COMPLETED" {
            assert_eq!(tj["verificacao"]["freivalds"], 3, "{tj}");
            break;
        }
        assert!(Instant::now() < ate, "a tarefa não terminou a tempo: {tj} · agente: {}", n.plataforma.json());
        std::thread::sleep(Duration::from_millis(500));
    }
    // o painel do fornecedor vê o computador de verdade. O núcleo lê o
    // hardware do sistema em segundo plano (no Windows leva alguns segundos)
    // e manda em todo pulso: o modelo da CPU chega por NODE_UPDATED
    let detalhe = forn.get(&format!("/api/v1/nos/{no}")).json();
    assert_eq!(detalhe["reputacao"]["aceitas"], 3, "{detalhe}");
    assert!(detalhe["hardware"]["nucleos"].as_u64().unwrap() >= 1);
    let ate = Instant::now() + Duration::from_secs(120);
    loop {
        let d = forn.get(&format!("/api/v1/nos/{no}")).json();
        if !d["hardware"]["cpu_modelo"].as_str().unwrap_or("").is_empty() {
            break;
        }
        assert!(Instant::now() < ate, "o modelo da CPU não chegou: {d}");
        std::thread::sleep(Duration::from_secs(2));
    }
    // pausar cancela e para de receber
    n.plataforma.ligar_fornecimento(false);
    let t2 = cliente.post("/api/v1/tarefas", json!({ "tipo": "matrix", "tamanho": 24, "unidades": 1, "prazo_s": 600 })).json()["id"].as_u64().unwrap();
    std::thread::sleep(Duration::from_secs(12));
    let t2j = cliente.get(&format!("/api/v1/tarefas/{t2}")).json();
    assert_eq!(t2j["progresso"]["despachadas"], 0, "pausado não recebe: {t2j}");
    let _ = std::fs::remove_dir_all(&pasta);
}
