// ✝ Neemias 4:6 — “O povo tinha ânimo para trabalhar.”
//! A definição de pronto da V1 (seção 63 do documento), de ponta a ponta,
//! por HTTP de verdade, com dois fornecedores simulados que executam os
//! motores do ULTRAX de verdade e assinam como o programa:
//!
//! conta criada → computador registrado → nó online → saldo adicionado →
//! tarefa criada → agendador acha nó → unidade executa isolada → resultado
//! volta → resultado conferido → cliente cobrado → fornecedor creditado →
//! comissão registrada → tudo no diário → administrador para tarefa e nó.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod comum;

use std::sync::atomic::Ordering;

use comum::{Agente, Navegador, servidor};
use hyurax_plataforma::dinheiro::{Conta, Moeda};
use serde_json::{Value, json};

fn rodar_ate(s: &comum::Servidor, agentes: &[&Agente], tarefa: u64, cliente: &Navegador, final_: &str) -> Value {
    for _ in 0..200 {
        for a in agentes {
            a.ciclo();
        }
        s.p.manutencao().unwrap();
        let t = cliente.get(&format!("/api/v1/tarefas/{tarefa}")).json();
        if t["estado"] == final_ {
            return t;
        }
    }
    panic!("a tarefa {tarefa} não chegou em {final_}: {}", cliente.get(&format!("/api/v1/tarefas/{tarefa}")).texto());
}

#[test]
#[allow(clippy::too_many_lines)]
fn definicao_de_pronto() {
    let s = servidor("pronto", true);

    // 0. o administrador sobe o preço de teste: unidades de segundos custariam
    // frações de centavo pela tabela real, e o teste quer ver dinheiro andar
    let senha = s.p.criar_admin("admin@exemplo.com", "Equipe").unwrap();
    let mut adm = Navegador::novo(&s);
    let r = adm.post("/api/v1/auth/entrar", json!({ "email": "admin@exemplo.com", "senha": senha }));
    assert_eq!(r.status, 200, "{}", r.texto());
    let mut config = adm.get("/api/v1/admin/config").json();
    config["tabelas"]["BRL"] = json!({ "cpu_nucleo_hora": 100_000_000u64, "ram_gib_hora": 0, "gpu_hora": 3_000_000, "ops_por_s_cpu": 1000, "ops_por_s_gpu": 20_000_000_000u64 });
    let r = adm.post("/api/v1/admin/config", json!({ "config": config, "resumo": "preço de teste" }));
    assert_eq!(r.status, 200, "{}", r.texto());

    // 1. contas: um cliente e dois fornecedores (donos diferentes, para as conferências)
    let mut cliente = Navegador::novo(&s);
    let c = cliente.cadastrar("cliente@exemplo.com", true, false);
    assert_eq!(c["codigos_de_recuperacao"].as_array().unwrap().len(), 8);
    let mut forn_a = Navegador::novo(&s);
    forn_a.cadastrar("fornecedor.a@exemplo.com", false, true);
    let mut forn_b = Navegador::novo(&s);
    forn_b.cadastrar("fornecedor.b@exemplo.com", false, true);
    assert_eq!(cliente.get("/api/v1/auth/eu").json()["papeis"], json!(["CUSTOMER"]));

    // 2. cada fornecedor registra um computador com o código de vínculo
    let codigo_a = forn_a.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap().to_string();
    let codigo_b = forn_b.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap().to_string();
    let mut ag_a = Agente::novo(&s, 1);
    let mut ag_b = Agente::novo(&s, 2);
    ag_a.registrar(&codigo_a, "PC da sala");
    ag_b.registrar(&codigo_b, "Servidor do porão");
    // código usado não serve de novo
    let mut intruso = Agente::novo(&s, 9);
    let r = intruso.assinado("POST", "/api/v1/agente/registrar", json!({ "codigo": codigo_a }).to_string().as_bytes());
    assert_eq!(r.status, 403, "código de vínculo usado duas vezes: {}", r.texto());
    intruso.no = 0;

    // 3. o nó aparece online
    ag_a.pulso(json!([]));
    ag_b.pulso(json!([]));
    let nos_a = forn_a.get("/api/v1/nos").json();
    assert_eq!(nos_a["nos"][0]["estado"], "IDLE", "{nos_a}");
    assert_eq!(nos_a["nos"][0]["agora"]["temperatura"], 51.0);
    let mercado = cliente.get("/api/v1/marketplace").json();
    assert_eq!(mercado["capacidade"]["nos_online"], 2);

    // 4. o cliente adiciona saldo (ambiente de teste: confirma na hora)
    let pg = cliente.post("/api/v1/pagamentos", json!({ "valor_centavos": 5000 }));
    assert_eq!(pg.status, 201, "{}", pg.texto());
    assert_eq!(cliente.get("/api/v1/saldo").json()["disponivel"]["centavos"], 5000);

    // 5. estimativa e tarefa: rotas, 4 unidades
    let pedido = json!({ "tipo": "routing", "tamanho": 24, "passos": 60, "parametros": [11], "unidades": 4, "paralelo": 2, "gpu": false, "prazo_s": 3600, "copias": 1, "descricao": "rotas de entrega do bairro" });
    let est = cliente.post("/api/v1/tarefas/estimativa", pedido.clone()).json();
    assert!(est["reserva_centavos"].as_i64().unwrap() > 100, "{est}");
    assert_eq!(est["politica"]["network"], "nenhuma: o motor não tem rede nem acesso a arquivo");
    let criada = cliente.post("/api/v1/tarefas", pedido);
    assert_eq!(criada.status, 201, "{}", criada.texto());
    let tarefa = criada.json()["id"].as_u64().unwrap();
    let reserva = criada.json()["reserva_centavos"].as_i64().unwrap();
    assert_eq!(cliente.get("/api/v1/saldo").json()["reservado"]["centavos"], reserva);

    // 6-10. agendador, execução, resultado, verificação (nós novos: tudo é
    // conferido por um nó de outro dono no período de prova)
    let t = rodar_ate(&s, &[&ag_a, &ag_b], tarefa, &cliente, "COMPLETED");
    assert_eq!(t["progresso"]["concluidas"], 4, "{t}");
    assert_eq!(t["verificacao"]["concordancia"], 4, "período de prova: tudo conferido por concordância: {t}");
    assert_eq!(t["verificacao"]["divergencias"], 0);
    let res = cliente.get(&format!("/api/v1/tarefas/{tarefa}/resultado")).json();
    assert_eq!(res["unidades_conferidas"], 4, "{res}");
    assert!(res["texto"].as_str().unwrap().contains("rota"), "{res}");
    // os bytes de uma unidade conferem com o hash do diário
    let b = cliente.get(&format!("/api/v1/tarefas/{tarefa}/unidades/0"));
    assert_eq!(b.status, 200);
    assert_eq!(hyurax_plataforma::util::hex(&hyurax_ultrax::trabalho::hash_do_resultado(&b.corpo)), res["unidades"][0]["hash"].as_str().unwrap());
    let rel = cliente.get(&format!("/api/v1/tarefas/{tarefa}/relatorio")).json();
    assert_eq!(rel["relatorio"]["unidades"].as_array().unwrap().len(), 4);

    // 11-13. cobrança, crédito ao fornecedor, comissão: o livro fecha
    let cobrado = t["custo"]["cobrado_centavos"].as_i64().unwrap();
    assert!(cobrado >= 1 && cobrado <= reserva, "cobrado {cobrado}, reserva {reserva}");
    let saldo = cliente.get("/api/v1/saldo").json();
    assert_eq!(saldo["disponivel"]["centavos"].as_i64().unwrap(), 5000 - cobrado);
    assert_eq!(saldo["reservado"]["centavos"], 0);
    let (pend_a, pend_b, receita, soma) = s
        .p
        .ler(|e| {
            let a = e.nos.get(&ag_a.no).unwrap().dono;
            let b = e.nos.get(&ag_b.no).unwrap().dono;
            (
                e.livro.saldo(Conta::FornecedorPendente(a), Moeda::Brl),
                e.livro.saldo(Conta::FornecedorPendente(b), Moeda::Brl),
                e.livro.saldo(Conta::PlataformaReceita, Moeda::Brl),
                e.livro.soma(Moeda::Brl),
            )
        })
        .unwrap();
    assert_eq!(soma, 0, "o livro fecha em zero");
    assert!(pend_a + pend_b > 0, "fornecedores creditados");
    // a comissão e as conferências: a plataforma paga a dúvida dela
    assert_eq!(receita + pend_a + pend_b, cobrado, "o que o cliente pagou = fornecedores + plataforma");
    let ganhos_a = forn_a.get("/api/v1/ganhos").json();
    assert!(!ganhos_a["retidos"].as_array().unwrap().is_empty(), "{ganhos_a}");

    // retenção: 8 dias depois o ganho libera e o fornecedor pede saque
    s.adianto.fetch_add(8 * 86_400_000, Ordering::Relaxed);
    s.p.manutencao().unwrap();
    // oito dias parado: as sessões caíram (24 h sem uso); entra de novo
    assert_eq!(forn_a.get("/api/v1/ganhos").status, 401, "sessão ociosa expira");
    assert_eq!(forn_a.post("/api/v1/auth/entrar", json!({ "email": "fornecedor.a@exemplo.com", "senha": "ponte-azul-77-rio" })).status, 200);
    assert_eq!(adm.post("/api/v1/auth/entrar", json!({ "email": "admin@exemplo.com", "senha": senha })).status, 200);
    assert_eq!(cliente.post("/api/v1/auth/entrar", json!({ "email": "cliente@exemplo.com", "senha": "ponte-azul-77-rio" })).status, 200);
    ag_a.pulso(json!([]));
    ag_b.pulso(json!([]));
    let disp_a = forn_a.get("/api/v1/ganhos").json()["saldo"]["ganhos_disponiveis"]["centavos"].as_i64().unwrap();
    assert_eq!(disp_a, pend_a, "ganho saiu da retenção");

    // 14. o administrador
    let painel = adm.get("/api/v1/admin/painel").json();
    assert_eq!(painel["negocio"][0]["livro_fecha"], true, "{painel}");
    assert_eq!(painel["produto"]["tarefas"]["COMPLETED"], 1);
    // kill task e kill node
    let pedido2 = json!({ "tipo": "knapsack", "tamanho": 40, "passos": 0, "parametros": [], "unidades": 5, "paralelo": 1, "prazo_s": 3600, "copias": 1 });
    let r2 = cliente.post("/api/v1/tarefas", pedido2);
    assert_eq!(r2.status, 201, "{}", r2.texto());
    let t2 = r2.json()["id"].as_u64().unwrap();
    let pulso = ag_a.pulso(json!([]));
    assert!(!pulso["atribuicoes"].as_array().unwrap().is_empty());
    let r = adm.post(&format!("/api/v1/admin/tarefas/{t2}/parar"), json!({ "motivo": "teste do kill switch" }));
    assert_eq!(r.status, 200, "{}", r.texto());
    let t2j = cliente.get(&format!("/api/v1/tarefas/{t2}")).json();
    assert_eq!(t2j["estado"], "CANCELLED");
    // o agente recebe a ordem de parar o que estava fazendo
    let ids: Vec<Value> = pulso["atribuicoes"].as_array().unwrap().iter().map(|a| json!({ "atribuicao": a["id"], "executando": true })).collect();
    let depois = ag_a.pulso(Value::Array(ids.clone()));
    assert_eq!(depois["cancelar"].as_array().unwrap().len(), ids.len(), "{depois}");
    let r = adm.post(&format!("/api/v1/admin/nos/{}/estado", ag_b.no), json!({ "estado": "QUARANTINED", "motivo": "teste" }));
    assert_eq!(r.status, 200, "{}", r.texto());
    assert_eq!(ag_b.pulso(json!([]))["estado"], "QUARANTINED");
    // parada de emergência
    adm.post("/api/v1/admin/emergencia", json!({ "ligado": true }));
    let r = cliente.post("/api/v1/tarefas", json!({ "tipo": "matrix", "tamanho": 8, "passos": 0, "parametros": [], "unidades": 1 }));
    assert_eq!(r.status, 503, "{}", r.texto());
    adm.post("/api/v1/admin/emergencia", json!({ "ligado": false }));

    // 15. o diário tem tudo, e nenhum segredo
    let registro = adm.get("/api/v1/admin/registro?tipo=TASK_COMPLETED").json();
    assert_eq!(registro["eventos"].as_array().unwrap().len(), 1, "{registro}");
    let diario = std::fs::read_to_string(s.pasta.join("diario.hyx")).unwrap();
    assert!(!diario.contains("ponte-azul-77-rio"), "senha em texto no diário");
    assert!(!diario.contains(&senha), "senha do admin em texto no diário");
    assert!(!diario.contains(&codigo_a), "código de vínculo em texto no diário");
    let cookie = cliente.cookie.clone().unwrap();
    assert!(!diario.contains(cookie.trim_start_matches("hyx_sessao=")), "token de sessão no diário");
}

#[test]
fn matriz_conferida_por_freivalds_e_fornecedor_desonesto_vai_para_quarentena() {
    let s = servidor("freivalds", true);
    let mut cliente = Navegador::novo(&s);
    cliente.cadastrar("c2@exemplo.com", true, false);
    cliente.post("/api/v1/pagamentos", json!({ "valor_centavos": 2000 }));
    let mut f = Navegador::novo(&s);
    f.cadastrar("f2@exemplo.com", false, true);
    let mut ag = Agente::novo(&s, 3);
    ag.registrar(f.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap(), "honesto?");
    ag.estraga = true;
    let t = cliente.post("/api/v1/tarefas", json!({ "tipo": "matrix", "tamanho": 16, "passos": 0, "parametros": [], "unidades": 3, "paralelo": 2, "prazo_s": 600 })).json()["id"].as_u64().unwrap();
    // o nó devolve matrizes erradas, bem assinadas: Freivalds recusa no servidor
    for _ in 0..3 {
        ag.ciclo();
        s.p.manutencao().unwrap();
    }
    let no = f.get("/api/v1/nos").json();
    assert_eq!(no["nos"][0]["estado"], "QUARANTINED", "{no}");
    assert!(no["nos"][0]["reputacao"]["incorretas"].as_u64().unwrap() >= 1);
    // ninguém pagou por resultado errado
    let tj = cliente.get(&format!("/api/v1/tarefas/{t}")).json();
    assert_eq!(tj["progresso"]["concluidas"], 0, "{tj}");
    let alertas = s.p.ler(|e| e.alertas.values().filter(|a| a.tipo == "fraude:resultado-falso").count()).unwrap();
    assert_eq!(alertas, 1);
    // um fornecedor honesto chega e termina a tarefa; a matriz fecha só com
    // Freivalds (prova matemática no servidor, sem segundo nó)
    let mut f3 = Navegador::novo(&s);
    f3.cadastrar("f3@exemplo.com", false, true);
    let mut bom = Agente::novo(&s, 4);
    bom.registrar(f3.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap(), "honesto");
    let tj = rodar_ate(&s, &[&bom], t, &cliente, "COMPLETED");
    assert_eq!(tj["verificacao"]["freivalds"], 3, "{tj}");
    assert_eq!(s.p.ler(|e| e.livro.soma(Moeda::Brl)).unwrap(), 0);
}

#[test]
fn cliente_que_tambem_fornece_nao_recebe_a_propria_tarefa() {
    let s = servidor("proprio", true);
    let mut ambos = Navegador::novo(&s);
    ambos.cadastrar("ambos@exemplo.com", true, true);
    ambos.post("/api/v1/pagamentos", json!({ "valor_centavos": 2000 }));
    let mut ag = Agente::novo(&s, 5);
    ag.registrar(ambos.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap(), "meu PC");
    let r = ambos.post("/api/v1/tarefas", json!({ "tipo": "diffusion", "tamanho": 8, "passos": 4, "parametros": [], "unidades": 2, "prazo_s": 600 }));
    assert_eq!(r.status, 201, "{}", r.texto());
    for _ in 0..3 {
        assert_eq!(ag.ciclo(), 0, "o nó do próprio cliente não recebe a tarefa dele");
    }
}

#[test]
fn copias_pagas_pelo_cliente_e_concordancia() {
    let s = servidor("copias", true);
    let mut cliente = Navegador::novo(&s);
    cliente.cadastrar("c4@exemplo.com", true, false);
    cliente.post("/api/v1/pagamentos", json!({ "valor_centavos": 3000 }));
    let mut ags = Vec::new();
    for (i, email) in ["p1@exemplo.com", "p2@exemplo.com"].iter().enumerate() {
        let mut f = Navegador::novo(&s);
        f.cadastrar(email, false, true);
        let mut a = Agente::novo(&s, 10 + u8::try_from(i).unwrap());
        a.registrar(f.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap(), email);
        ags.push(a);
    }
    let t = cliente
        .post("/api/v1/tarefas", json!({ "tipo": "knapsack", "tamanho": 30, "passos": 0, "parametros": [], "unidades": 3, "paralelo": 3, "prazo_s": 600, "copias": 2 }))
        .json()["id"]
        .as_u64()
        .unwrap();
    let refs: Vec<&Agente> = ags.iter().collect();
    let tj = rodar_ate(&s, &refs, t, &cliente, "COMPLETED");
    assert_eq!(tj["verificacao"]["concordancia"], 3, "{tj}");
    // cada unidade foi feita duas vezes, por donos diferentes, e o cliente pagou as duas
    let rel = cliente.get(&format!("/api/v1/tarefas/{t}/relatorio")).json();
    for u in rel["relatorio"]["unidades"].as_array().unwrap() {
        let aceitas: Vec<&Value> = u["entregas"].as_array().unwrap().iter().filter(|e| e["desfecho"] == "aceita").collect();
        assert_eq!(aceitas.len(), 2, "{u}");
        assert_ne!(aceitas[0]["no"], aceitas[1]["no"]);
    }
}
