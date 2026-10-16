//! A API `/api/v1/` (seção 29 do documento da V1) e as páginas.
//!
//! Três jeitos de se identificar, cada um no seu lugar:
//!
//! - **navegador:** cookie `hyx_sessao` (HttpOnly, SameSite=Strict, Secure
//!   no HTTPS). Todo pedido que muda alguma coisa precisa também do cabeçalho
//!   `X-Hyx-Pedido: 1` e, se vier `Origin`, que ela seja a do próprio site:
//!   formulário de outro site não consegue nenhuma das duas coisas;
//! - **cliente avançado:** `Authorization: Bearer hyx_...` (a HYX_API_KEY),
//!   com escopo e limite por minuto;
//! - **agente:** pedido assinado pela chave do worker (`X-Hyx-No`,
//!   `X-Hyx-Tempo`, `X-Hyx-Assinatura`), só em `/api/v1/agente/`.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::http::{CORPO_PADRAO, Pedido, Resposta};
use crate::modelo::{Config, Limites, PedidoDeTarefa};
use crate::servico::contas::{Cadastro, Quem, SESSAO_MAX_MS};
use crate::servico::tarefas::{ENTREGA_MAX, catalogo};
use crate::servico::{Erro, Plataforma, R};

/// Quanto corpo cada caminho aceita.
pub fn corpo_max(metodo: &str, caminho: &str) -> usize {
    if metodo == "POST" && caminho == "/api/v1/agente/resultado" { ENTREGA_MAX } else { CORPO_PADRAO }
}

fn ok(v: Value) -> R<Resposta> {
    Ok(Resposta::json(200, &v))
}

fn cookie(p: &Plataforma, token: &str, max_age_s: u64) -> String {
    let seguro = if p.opcoes.https { "; Secure" } else { "" };
    format!("hyx_sessao={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age_s}{seguro}")
}

/// Atende um pedido.
pub fn atender(p: &Plataforma, pedido: &Pedido) -> Resposta {
    let ritmo_ip = if pedido.caminho.starts_with("/api/v1/auth/") { 30 } else { 900 };
    if !p.ritmo(&format!("ip:{}:{}", pedido.ip, if ritmo_ip == 30 { "auth" } else { "geral" }), ritmo_ip) {
        return Resposta::erro(&Erro::limite("pedidos demais deste endereço; espere um minuto"));
    }
    match rotear(p, pedido) {
        Ok(r) => r,
        Err(e) => Resposta::erro(&e),
    }
}

fn id(seg: Option<&&str>) -> R<u64> {
    seg.and_then(|s| s.parse().ok()).ok_or_else(|| Erro::nao_achado("número inválido no caminho"))
}

#[allow(clippy::too_many_lines)]
fn rotear(p: &Plataforma, pd: &Pedido) -> R<Resposta> {
    let m = pd.metodo.as_str();
    let partes: Vec<&str> = pd.caminho.trim_end_matches('/').split('/').skip(1).collect();
    let seg = |i: usize| partes.get(i).copied().unwrap_or("");
    if m == "GET" && !pd.caminho.starts_with("/api/") {
        return crate::web::pagina(p, &pd.caminho);
    }
    if m == "GET" && pd.caminho == "/saude" {
        return saude(p);
    }
    if seg(0) != "api" || seg(1) != "v1" {
        return Err(Erro::nao_achado("caminho desconhecido"));
    }
    let resto: Vec<&str> = partes.iter().skip(2).copied().collect();
    let r = |i: usize| resto.get(i).copied().unwrap_or("");
    match (m, r(0)) {
        // ------------------------------------------------ público
        ("GET", "marketplace") => return ok(p.marketplace()?),
        ("GET", "catalogo") => return ok(catalogo()),
        ("GET", "saude") => return saude(p),
        ("POST", "agente") => return agente(p, pd, r(1)),
        ("GET", "agente") => return agente(p, pd, r(1)),
        ("POST", "auth") if r(1) == "cadastro" => {
            conferir_origem(p, pd)?;
            #[derive(Deserialize)]
            struct C {
                email: String,
                nome: String,
                senha: String,
                #[serde(default)]
                moeda: String,
                #[serde(default)]
                pais: String,
                #[serde(default)]
                cliente: bool,
                #[serde(default)]
                fornecedor: bool,
                #[serde(default)]
                aceite: bool,
            }
            let c: C = pd.json()?;
            let e = p.cadastrar(
                Cadastro {
                    email: c.email,
                    nome: c.nome,
                    senha: c.senha,
                    moeda: if c.moeda.is_empty() { "BRL".into() } else { c.moeda },
                    pais: if c.pais.is_empty() { "BR".into() } else { c.pais },
                    cliente: c.cliente,
                    fornecedor: c.fornecedor,
                    aceite: c.aceite,
                },
                &pd.ip,
                pd.cabecalho("user-agent").unwrap_or(""),
            )?;
            return Ok(Resposta::json(201, &json!({ "usuario": e.usuario, "codigos_de_recuperacao": e.codigos, "aviso": "guarde os códigos de recuperação: são a única forma de recuperar a conta sem a senha" }))
                .com("Set-Cookie", cookie(p, &e.token, SESSAO_MAX_MS / 1000)));
        }
        ("POST", "auth") if r(1) == "entrar" => {
            conferir_origem(p, pd)?;
            #[derive(Deserialize)]
            struct E {
                email: String,
                senha: String,
            }
            let c: E = pd.json()?;
            let e = p.entrar(&c.email, &c.senha, &pd.ip, pd.cabecalho("user-agent").unwrap_or(""))?;
            return Ok(Resposta::json(200, &json!({ "usuario": e.usuario })).com("Set-Cookie", cookie(p, &e.token, SESSAO_MAX_MS / 1000)));
        }
        ("POST", "auth") if r(1) == "recuperar" => {
            conferir_origem(p, pd)?;
            #[derive(Deserialize)]
            struct Rc {
                email: String,
                codigo: String,
                nova: String,
            }
            let c: Rc = pd.json()?;
            p.recuperar(&c.email, &c.codigo, &c.nova)?;
            return ok(json!({ "ok": true, "texto": "senha trocada; entre com a senha nova" }));
        }
        _ => {}
    }

    // ------------------------------------------------ autenticado
    let q = quem(p, pd)?;
    if m == "POST" && q.sessao.is_some() {
        conferir_origem(p, pd)?;
    }
    match (m, r(0), r(1), r(2)) {
        ("POST", "auth", "sair", _) => {
            if let Some(h) = &q.sessao {
                p.sair(h)?;
            }
            Ok(Resposta::json(200, &json!({ "ok": true })).com("Set-Cookie", cookie(p, "", 0)))
        }
        ("GET", "auth", "eu", _) => ok(p.eu(&q)?),
        // ------------------------------------------- conta
        ("POST", "conta", "senha", _) => {
            #[derive(Deserialize)]
            struct S {
                atual: String,
                nova: String,
            }
            let c: S = pd.json()?;
            let token = p.trocar_senha(&q, &c.atual, &c.nova, &pd.ip, pd.cabecalho("user-agent").unwrap_or(""))?;
            Ok(Resposta::json(200, &json!({ "ok": true, "texto": "senha trocada; as outras sessões foram encerradas" })).com("Set-Cookie", cookie(p, &token, SESSAO_MAX_MS / 1000)))
        }
        ("POST", "conta", "papel", _) => {
            #[derive(Deserialize)]
            struct Pa {
                papel: String,
            }
            let c: Pa = pd.json()?;
            ok(p.ativar_papel(&q, &c.papel)?)
        }
        ("POST", "conta", "recuperacao", _) => {
            #[derive(Deserialize)]
            struct Se {
                senha: String,
            }
            let c: Se = pd.json()?;
            ok(json!({ "codigos_de_recuperacao": p.novos_codigos(&q, &c.senha)? }))
        }
        ("GET", "conta", "sessoes", _) => ok(p.sessoes(&q)?),
        ("POST", "conta", "sessoes", "encerrar") => ok(json!({ "encerradas": p.encerrar_outras_sessoes(&q)? })),
        // ------------------------------------------- chaves de API
        ("GET", "chaves", "", _) => ok(p.chaves(&q)?),
        ("POST", "chaves", "", _) => {
            #[derive(Deserialize)]
            struct Ch {
                nome: String,
                escopos: Vec<String>,
                #[serde(default)]
                limite_por_min: u32,
            }
            let c: Ch = pd.json()?;
            Ok(Resposta::json(201, &p.criar_chave(&q, &c.nome, &c.escopos, if c.limite_por_min == 0 { 60 } else { c.limite_por_min })?))
        }
        ("POST", "chaves", n, "revogar") => {
            p.revogar_chave(&q, id(Some(&n))?)?;
            ok(json!({ "ok": true }))
        }
        ("POST", "chaves", n, "rotacionar") => ok(p.rotacionar_chave(&q, id(Some(&n))?)?),
        // ------------------------------------------- cliente
        ("GET", "saldo", _, _) => ok(p.saldo(&q)?),
        ("GET", "extrato", _, _) => ok(p.extrato(&q, pd.parametro("mes").as_deref())?),
        ("GET", "faturas", mes, _) => {
            let alvo = if mes.is_empty() { pd.parametro("mes").unwrap_or_else(|| crate::util::mes(p.agora())) } else { mes.to_string() };
            ok(p.fatura(&q, &alvo)?)
        }
        ("GET", "pagamentos", "", _) => ok(p.meus_pagamentos(&q)?),
        ("POST", "pagamentos", "", _) => {
            #[derive(Deserialize)]
            struct Pg {
                valor_centavos: i64,
            }
            let c: Pg = pd.json()?;
            Ok(Resposta::json(201, &p.criar_pagamento(&q, c.valor_centavos)?))
        }
        ("POST", "pagamentos", n, "cancelar") => {
            p.cancelar_pagamento(&q, id(Some(&n))?)?;
            ok(json!({ "ok": true }))
        }
        ("POST", "tarefas", "estimativa", _) => {
            let c: PedidoDeTarefa = pd.json()?;
            ok(p.estimar(&q, c)?)
        }
        ("POST", "tarefas", "", _) => {
            let c: PedidoDeTarefa = pd.json()?;
            Ok(Resposta::json(201, &p.criar_tarefa(&q, c, q.escopos.is_some())?))
        }
        ("GET", "tarefas", "", _) => ok(p.minhas_tarefas(&q)?),
        ("GET", "tarefas", n, "") => ok(p.tarefa(&q, id(Some(&n))?)?),
        ("POST", "tarefas", n, "cancelar") => ok(p.cancelar_tarefa(&q, id(Some(&n))?)?),
        ("GET", "tarefas", n, "resultado") => ok(p.resultado_da_tarefa(&q, id(Some(&n))?)?),
        ("GET", "tarefas", n, "relatorio") => ok(p.relatorio(&q, id(Some(&n))?)?),
        ("GET", "tarefas", n, "unidades") => {
            let tarefa = id(Some(&n))?;
            let unidade = id(resto.get(3))?;
            let b = p.bytes_da_unidade(&q, tarefa, unidade)?;
            Ok(Resposta::bytes("application/octet-stream", b).com("Content-Disposition", format!("attachment; filename=\"tarefa-{tarefa}-unidade-{unidade}.bin\"")))
        }
        // ------------------------------------------- fornecedor
        ("POST", "nos", "vinculo", _) => Ok(Resposta::json(201, &p.criar_vinculo(&q)?)),
        ("GET", "nos", "", _) => ok(p.meus_nos(&q)?),
        ("GET", "nos", n, "") => ok(p.detalhe_do_no(&q, id(Some(&n))?)?),
        ("POST", "nos", n, acao) => {
            let no = id(Some(&n))?;
            match acao {
                "pausar" => p.pausar_no(&q, no, true)?,
                "retomar" => p.pausar_no(&q, no, false)?,
                "remover" => p.remover_no(&q, no)?,
                "ajustes" => {
                    #[derive(Deserialize)]
                    struct Aj {
                        nome: Option<String>,
                        regiao: Option<String>,
                        limites: Option<Limites>,
                    }
                    let c: Aj = pd.json()?;
                    return ok(p.ajustar_no(&q, no, c.nome, c.regiao, c.limites)?);
                }
                _ => return Err(Erro::nao_achado("ação desconhecida")),
            }
            ok(json!({ "ok": true }))
        }
        ("GET", "ganhos", _, _) => ok(p.ganhos(&q)?),
        ("POST", "saques", "", _) => {
            #[derive(Deserialize)]
            struct Sq {
                valor_centavos: i64,
                destino: String,
            }
            let c: Sq = pd.json()?;
            Ok(Resposta::json(201, &p.pedir_saque(&q, c.valor_centavos, &c.destino)?))
        }
        // ------------------------------------------- administração
        (_, "admin", _, _) => admin(p, pd, &q, &resto),
        _ => Err(Erro::nao_achado("caminho desconhecido")),
    }
}

fn saude(p: &Plataforma) -> R<Resposta> {
    let eventos = p.ler(|e| e.aplicados)?;
    let estado = if p.degradada() { "leitura" } else { "ok" };
    ok(json!({ "estado": estado, "versao": crate::VERSAO, "eventos": eventos, "modo_teste": p.opcoes.modo_teste }))
}

/// Pedido que muda alguma coisa, vindo do navegador: exige o cabeçalho
/// próprio e, quando há `Origin`, a do próprio site.
fn conferir_origem(p: &Plataforma, pd: &Pedido) -> R<()> {
    if pd.cabecalho("x-hyx-pedido") != Some("1") {
        return Err(Erro::proibido("pedido sem o cabeçalho X-Hyx-Pedido"));
    }
    if let Some(origem) = pd.cabecalho("origin") {
        let host = pd.cabecalho("host").unwrap_or("");
        let publico = p.opcoes.endereco_publico.trim_end_matches('/');
        let mesma = origem == publico || origem.split_once("://").is_some_and(|(_, resto)| resto == host);
        if !mesma {
            return Err(Erro::proibido("origem não permitida"));
        }
    }
    Ok(())
}

fn quem(p: &Plataforma, pd: &Pedido) -> R<Quem> {
    if let Some(a) = pd.cabecalho("authorization") {
        let chave = a.strip_prefix("Bearer ").or_else(|| a.strip_prefix("bearer ")).ok_or_else(|| Erro { status: 401, mensagem: "use Authorization: Bearer <HYX_API_KEY>".into() })?;
        return p.por_chave(chave);
    }
    let token = pd.cookie("hyx_sessao").filter(|t| !t.is_empty()).ok_or_else(Erro::sem_sessao)?;
    p.por_sessao(token)
}

fn agente(p: &Plataforma, pd: &Pedido, acao: &str) -> R<Resposta> {
    let chave = p.autenticar_agente(
        &pd.metodo,
        &pd.caminho,
        pd.cabecalho("x-hyx-no").unwrap_or(""),
        pd.cabecalho("x-hyx-tempo").unwrap_or(""),
        pd.cabecalho("x-hyx-assinatura").unwrap_or(""),
        &pd.corpo,
    )?;
    match (pd.metodo.as_str(), acao) {
        ("POST", "registrar") => ok(p.registrar_no(chave, pd.json()?)?),
        ("POST", "pulso") => ok(p.pulso(chave, pd.json()?)?),
        ("POST", "resultado") => ok(p.entregar(chave, &pd.corpo)?),
        ("POST", "falha") => {
            #[derive(Deserialize)]
            struct F {
                atribuicao: u64,
                #[serde(default)]
                motivo: String,
            }
            let c: F = pd.json()?;
            p.falha_do_agente(chave, c.atribuicao, &c.motivo)?;
            ok(json!({ "ok": true }))
        }
        ("GET", "resumo") => ok(p.resumo_do_agente(chave)?),
        _ => Err(Erro::nao_achado("ação de agente desconhecida")),
    }
}

#[allow(clippy::too_many_lines)]
fn admin(p: &Plataforma, pd: &Pedido, q: &Quem, resto: &[&str]) -> R<Resposta> {
    q.admin()?;
    let r = |i: usize| resto.get(i).copied().unwrap_or("");
    #[derive(Deserialize, Default)]
    struct Nota {
        #[serde(default)]
        motivo: String,
        #[serde(default)]
        nota: String,
        #[serde(default)]
        referencia: String,
        #[serde(default)]
        taxa_centavos: i64,
        #[serde(default)]
        estado: String,
        #[serde(default)]
        ligado: bool,
        #[serde(default)]
        valor_centavos: i64,
        #[serde(default)]
        usuario: u64,
    }
    let nota = || -> R<Nota> { if pd.corpo.is_empty() { Ok(Nota::default()) } else { pd.json() } };
    match (pd.metodo.as_str(), r(1), r(3)) {
        ("GET", "painel", _) => ok(p.painel(q)?),
        ("GET", "usuarios", _) => ok(p.admin_usuarios(q, &pd.parametro("busca").unwrap_or_default())?),
        ("POST", "usuarios", "codigos") => ok(p.admin_codigos(q, id(resto.get(2))?)?),
        ("POST", "usuarios", acao) => {
            let n = nota()?;
            ok(p.admin_usuario(q, id(resto.get(2))?, acao, &n.motivo)?)
        }
        ("GET", "nos", _) => ok(p.admin_nos(q)?),
        ("POST", "nos", "estado") => {
            let n = nota()?;
            p.admin_no(q, id(resto.get(2))?, &n.estado, &n.motivo)?;
            ok(json!({ "ok": true }))
        }
        ("GET", "tarefas", _) => ok(p.admin_tarefas(q)?),
        ("POST", "tarefas", "parar") => {
            let n = nota()?;
            p.admin_parar_tarefa(q, id(resto.get(2))?, &n.motivo)?;
            ok(json!({ "ok": true }))
        }
        ("POST", "tarefas", "soltar-ganhos") => ok(json!({ "liberados": p.admin_soltar_ganhos(q, id(resto.get(2))?)? })),
        ("GET", "pagamentos", _) => ok(p.admin_pagamentos(q)?),
        ("POST", "pagamentos", acao) => {
            let n = nota()?;
            let pid = id(resto.get(2))?;
            match acao {
                "confirmar" => p.admin_confirmar_pagamento(q, pid, &n.referencia, n.taxa_centavos)?,
                "cancelar" => p.admin_cancelar_pagamento(q, pid, &n.motivo)?,
                "estornar" => p.admin_estornar_pagamento(q, pid, &n.motivo)?,
                _ => return Err(Erro::nao_achado("ação: confirmar, cancelar ou estornar")),
            }
            ok(json!({ "ok": true }))
        }
        ("GET", "saques", _) => ok(p.admin_saques(q)?),
        ("POST", "saques", acao) => {
            let n = nota()?;
            p.admin_saque(q, id(resto.get(2))?, acao, if n.nota.is_empty() { &n.referencia } else { &n.nota })?;
            ok(json!({ "ok": true }))
        }
        ("GET", "config", _) => ok(p.admin_config(q)?),
        ("POST", "config", _) => {
            #[derive(Deserialize)]
            struct Cf {
                config: Config,
                #[serde(default)]
                resumo: String,
            }
            let c: Cf = pd.json()?;
            p.admin_trocar_config(q, c.config, &c.resumo)?;
            ok(p.admin_config(q)?)
        }
        ("POST", "emergencia", _) => {
            let n = nota()?;
            p.admin_emergencia(q, n.ligado)?;
            ok(json!({ "parada_de_emergencia": n.ligado }))
        }
        ("POST", "motores", _) => {
            let n = nota()?;
            p.admin_motor(q, r(2), n.ligado)?;
            ok(json!({ "ok": true }))
        }
        ("GET", "alertas", _) => ok(p.admin_alertas(q)?),
        ("POST", "alertas", "resolver") => {
            let n = nota()?;
            p.admin_resolver_alerta(q, id(resto.get(2))?, &n.nota)?;
            ok(json!({ "ok": true }))
        }
        ("GET", "registro", _) => ok(p.admin_registro(q, &pd.parametro("tipo").unwrap_or_default(), &pd.parametro("busca").unwrap_or_default(), pd.parametro("antes").and_then(|x| x.parse().ok()))?),
        ("GET", "livro", _) => ok(p.admin_livro(q)?),
        ("GET", "copia", _) => {
            let b = p.admin_copia(q)?;
            Ok(Resposta::bytes("application/octet-stream", b).com("Content-Disposition", format!("attachment; filename=\"diario-{}.hyx\"", crate::util::data_hora(p.agora()).replace([' ', ':'], "-"))))
        }
        ("POST", "ajuste", _) => {
            let n = nota()?;
            p.admin_ajuste(q, n.usuario, n.valor_centavos, &n.motivo)?;
            ok(json!({ "ok": true }))
        }
        _ => Err(Erro::nao_achado("caminho de administração desconhecido")),
    }
}
