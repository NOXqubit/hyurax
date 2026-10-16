// ✝ Provérbios 4:23 — “Sobre tudo o que se deve guardar, guarda o teu coração.”
//! Os testes básicos de segurança da definição de pronto (seção 63) e do
//! modelo de confiança zero (seção 39): ninguém é confiável por padrão.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod comum;

use comum::{Agente, Navegador, pedir, servidor};
use serde_json::json;

#[test]
fn cliente_nao_ve_tarefa_nem_computador_de_outro() {
    let s = servidor("idor", true);
    let mut a = Navegador::novo(&s);
    a.cadastrar("a@exemplo.com", true, true);
    a.post("/api/v1/pagamentos", json!({ "valor_centavos": 1000 }));
    let t = a.post("/api/v1/tarefas", json!({ "tipo": "matrix", "tamanho": 8, "unidades": 1 })).json()["id"].as_u64().unwrap();
    let mut ag = Agente::novo(&s, 1);
    ag.registrar(a.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap(), "pc");
    let mut b = Navegador::novo(&s);
    b.cadastrar("b@exemplo.com", true, true);
    for caminho in [format!("/api/v1/tarefas/{t}"), format!("/api/v1/tarefas/{t}/resultado"), format!("/api/v1/tarefas/{t}/relatorio"), format!("/api/v1/nos/{}", ag.no)] {
        assert_eq!(b.get(&caminho).status, 404, "{caminho} vazou para outra conta");
    }
    assert_eq!(b.post(&format!("/api/v1/tarefas/{t}/cancelar"), json!({})).status, 404);
    assert_eq!(b.post(&format!("/api/v1/nos/{}/pausar", ag.no), json!({})).status, 404);
    assert!(b.get("/api/v1/tarefas").json()["tarefas"].as_array().unwrap().is_empty());
}

#[test]
fn papeis_e_administracao() {
    let s = servidor("papeis", true);
    let mut c = Navegador::novo(&s);
    c.cadastrar("c@exemplo.com", true, false);
    assert_eq!(c.get("/api/v1/admin/painel").status, 403);
    assert_eq!(c.post("/api/v1/admin/emergencia", json!({ "ligado": true })).status, 403);
    assert_eq!(c.post("/api/v1/nos/vinculo", json!({})).status, 403, "cliente sem o papel de fornecedor");
    assert_eq!(c.get("/api/v1/ganhos").status, 403);
    // ativa o papel e passa
    assert_eq!(c.post("/api/v1/conta/papel", json!({ "papel": "fornecedor" })).status, 200);
    assert_eq!(c.post("/api/v1/nos/vinculo", json!({})).status, 201);
    // sem sessão
    let anon = Navegador::novo(&s);
    assert_eq!(anon.get("/api/v1/saldo").status, 401);
    assert_eq!(anon.get("/api/v1/marketplace").status, 200, "o mercado é público");
}

#[test]
fn csrf_exige_cabecalho_e_mesma_origem() {
    let s = servidor("csrf", true);
    let mut c = Navegador::novo(&s);
    c.cadastrar("c@exemplo.com", true, false);
    let cookie = c.cookie.clone().unwrap();
    let corpo = json!({ "valor_centavos": 1000 }).to_string();
    // sem o cabeçalho próprio (um formulário de outro site)
    let r = pedir(&s.endereco, "POST", "/api/v1/pagamentos", &[("Cookie", cookie.clone()), ("Content-Type", "application/json".into())], corpo.as_bytes());
    assert_eq!(r.status, 403, "{}", r.texto());
    // com o cabeçalho, mas de outra origem
    let r = pedir(
        &s.endereco,
        "POST",
        "/api/v1/pagamentos",
        &[("Cookie", cookie.clone()), ("X-Hyx-Pedido", "1".into()), ("Origin", "https://site-malicioso.example".into())],
        corpo.as_bytes(),
    );
    assert_eq!(r.status, 403, "{}", r.texto());
    // o próprio site passa
    let r = pedir(&s.endereco, "POST", "/api/v1/pagamentos", &[("Cookie", cookie), ("X-Hyx-Pedido", "1".into()), ("Origin", format!("http://{}", s.endereco))], corpo.as_bytes());
    assert_eq!(r.status, 201, "{}", r.texto());
}

#[test]
fn cookie_e_cabecalhos_de_seguranca() {
    let s = servidor("cabecalhos", false);
    let mut c = Navegador::novo(&s);
    let r = c.post(
        "/api/v1/auth/cadastro",
        json!({ "email": "x@exemplo.com", "nome": "Xis", "senha": "ponte-azul-77-rio", "cliente": true, "aceite": true }),
    );
    let cookie = r.cabecalhos.iter().find(|(k, _)| k.eq_ignore_ascii_case("set-cookie")).unwrap().1.clone();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"), "{cookie}");
    let pagina = c.get("/");
    let cab = |n: &str| pagina.cabecalhos.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|x| x.1.clone()).unwrap_or_default();
    assert!(cab("content-security-policy").contains("default-src 'self'"));
    assert_eq!(cab("x-frame-options"), "DENY");
    assert_eq!(cab("x-content-type-options"), "nosniff");
    // fora do modo de teste não existe pagamento que confirma sozinho
    let r = c.post("/api/v1/pagamentos", json!({ "valor_centavos": 1000 }));
    assert_eq!(r.status, 503, "sem Pix configurado, o pagamento não abre: {}", r.texto());
}

#[test]
fn agente_assinatura_janela_e_repeticao() {
    let s = servidor("agente", true);
    let mut f = Navegador::novo(&s);
    f.cadastrar("f@exemplo.com", false, true);
    let mut ag = Agente::novo(&s, 7);
    ag.registrar(f.post("/api/v1/nos/vinculo", json!({})).json()["codigo"].as_str().unwrap(), "pc");
    let corpo = json!({ "versao": "1.5.0", "livres": 1, "ram_livre_mib": 512, "tipos": [1] }).to_string();
    let cab = ag.cabecalhos("POST", "/api/v1/agente/pulso", corpo.as_bytes());
    assert_eq!(pedir(&s.endereco, "POST", "/api/v1/agente/pulso", &cab, corpo.as_bytes()).status, 200);
    // o mesmo pedido de novo: repetição
    let r = pedir(&s.endereco, "POST", "/api/v1/agente/pulso", &cab, corpo.as_bytes());
    assert_eq!(r.status, 401, "{}", r.texto());
    assert!(r.texto().contains("repetido"));
    // corpo trocado com a assinatura velha
    let cab = ag.cabecalhos("POST", "/api/v1/agente/pulso", corpo.as_bytes());
    let r = pedir(&s.endereco, "POST", "/api/v1/agente/pulso", &cab, b"{\"livres\":64}");
    assert_eq!(r.status, 401);
    // relógio adiantado demais
    let mut cab = ag.cabecalhos("POST", "/api/v1/agente/pulso", corpo.as_bytes());
    cab[1].1 = (ag.agora() + 10 * 60_000).to_string();
    assert_eq!(pedir(&s.endereco, "POST", "/api/v1/agente/pulso", &cab, corpo.as_bytes()).status, 401);
    // chave que não está registrada
    let estranho = Agente::novo(&s, 8);
    assert_eq!(estranho.assinado("POST", "/api/v1/agente/pulso", corpo.as_bytes()).status, 410);
    // a assinatura do agente não abre a API do cliente
    let cab = ag.cabecalhos("GET", "/api/v1/saldo", b"");
    assert_eq!(pedir(&s.endereco, "GET", "/api/v1/saldo", &cab, b"").status, 401);
}

#[test]
fn forca_bruta_trava_a_entrada() {
    let s = servidor("bruta", true);
    let mut c = Navegador::novo(&s);
    c.cadastrar("vitima@exemplo.com", true, false);
    let mut atacante = Navegador::novo(&s);
    for _ in 0..5 {
        let r = atacante.post("/api/v1/auth/entrar", json!({ "email": "vitima@exemplo.com", "senha": "chute-errado-123" }));
        assert_eq!(r.status, 401);
        assert!(r.texto().contains("e-mail ou senha incorretos"));
    }
    // até a senha certa fica travada por 15 minutos
    let r = atacante.post("/api/v1/auth/entrar", json!({ "email": "vitima@exemplo.com", "senha": "ponte-azul-77-rio" }));
    assert_eq!(r.status, 429, "{}", r.texto());
    // e-mail que não existe responde igual (não diz quem tem conta)
    let r = atacante.post("/api/v1/auth/entrar", json!({ "email": "ninguem@exemplo.com", "senha": "qualquer-coisa-1" }));
    assert_eq!(r.status, 401);
    assert!(r.texto().contains("e-mail ou senha incorretos"));
    assert!(s.p.ler(|e| e.alertas.values().any(|a| a.tipo == "seguranca:forca-bruta")).unwrap());
}

#[test]
fn chave_de_api_com_escopo_limite_e_revogacao() {
    let s = servidor("chave", true);
    let mut c = Navegador::novo(&s);
    c.cadastrar("dev@exemplo.com", true, false);
    c.post("/api/v1/pagamentos", json!({ "valor_centavos": 1000 }));
    let k = c.post("/api/v1/chaves", json!({ "nome": "script de leitura", "escopos": ["tarefas:ler"], "limite_por_min": 3 })).json();
    let chave = k["chave"].as_str().unwrap().to_string();
    let auth = [("Authorization", format!("Bearer {chave}")), ("Content-Type", "application/json".into())];
    assert_eq!(pedir(&s.endereco, "GET", "/api/v1/tarefas", &auth, b"").status, 200);
    let corpo = json!({ "tipo": "matrix", "tamanho": 8, "unidades": 1 }).to_string();
    let r = pedir(&s.endereco, "POST", "/api/v1/tarefas", &auth, corpo.as_bytes());
    assert_eq!(r.status, 403, "escopo só de leitura: {}", r.texto());
    assert_eq!(pedir(&s.endereco, "GET", "/api/v1/saldo", &auth, b"").status, 403);
    // limite por minuto
    assert_eq!(pedir(&s.endereco, "GET", "/api/v1/tarefas", &auth, b"").status, 429);
    // a chave não aparece de novo nem no diário
    assert!(!c.get("/api/v1/chaves").texto().contains(&chave));
    assert!(!std::fs::read_to_string(s.pasta.join("diario.hyx")).unwrap().contains(&chave));
    // revogada, para de valer
    let id = k["id"].as_u64().unwrap();
    assert_eq!(c.post(&format!("/api/v1/chaves/{id}/revogar"), json!({})).status, 200);
    assert_eq!(pedir(&s.endereco, "GET", "/api/v1/tarefas", &auth, b"").status, 401);
}

#[test]
fn corpo_grande_e_caminho_estranho() {
    let s = servidor("corpo", true);
    let grande = vec![b'a'; 300 * 1024];
    let r = pedir(&s.endereco, "POST", "/api/v1/auth/entrar", &[("X-Hyx-Pedido", "1".into())], &grande);
    assert_eq!(r.status, 413);
    let r = pedir(&s.endereco, "GET", "/../../etc/passwd", &[], b"");
    assert_eq!(r.status, 400);
    let r = pedir(&s.endereco, "GET", "/api/v1/nada", &[], b"");
    assert_eq!(r.status, 401, "o que não é público pede sessão antes de dizer se existe");
}

/// O motor que roda no computador do fornecedor não tem como abrir arquivo,
/// rede ou processo: o código do ULTRAX não usa nenhuma dessas APIs.
#[test]
fn motores_nao_tocam_arquivo_rede_nem_processo() {
    let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("hyurax-ultrax").join("src");
    let proibidos = ["std::fs", "std::net", "std::process", "std::env", "File::", "TcpStream", "UdpSocket", "Command::"];
    let mut arquivos = 0;
    for entrada in std::fs::read_dir(&pasta).unwrap() {
        let caminho = entrada.unwrap().path();
        if caminho.extension().is_some_and(|x| x == "rs") {
            arquivos += 1;
            let texto = std::fs::read_to_string(&caminho).unwrap();
            // a parte de testes de cada arquivo pode ler vetores do disco
            let codigo = texto.split("#[cfg(test)]").next().unwrap_or("");
            for p in proibidos {
                assert!(!codigo.contains(p), "{} usa {p}", caminho.display());
            }
        }
    }
    assert!(arquivos >= 15, "achei {arquivos} arquivos do ULTRAX");
}
