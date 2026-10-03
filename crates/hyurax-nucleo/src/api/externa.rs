//! A API externa: JOBs mandados de fora deste computador, por contas com
//! chave de acesso (ver `crate::contas`). Desligada por padrão; o dono liga
//! em Ajustes ou com `hyurax-no painel --api-externa`.
//!
//! Autenticação: `Authorization: Bearer <chave>`. Sem a chave da sessão do
//! painel, sem `Origin` e sem cookie: uma página aberta no navegador não
//! consegue mandar esse cabeçalho para outro endereço sem a permissão de
//! CORS, que esta API nunca dá.
//!
//! | Método | Rota | O que faz |
//! |---|---|---|
//! | GET | `/api/v1/externa/conta` | nome, limite, consumo e saldo de créditos |
//! | POST | `/api/v1/externa/estimar` | custo e tempo previstos de um JOB |
//! | POST | `/api/v1/externa/jobs` | submete um JOB (orçamento = saldo, ou menos) |
//! | GET | `/api/v1/externa/jobs` | os JOBs da conta, com estado |
//! | GET | `/api/v1/externa/jobs/ID` | um JOB, inteiro |
//! | GET | `/api/v1/externa/jobs/ID/relatorio.json\|csv\|pdf` | o relatório |
//! | GET | `/api/v1/externa/jobs/ID/unidades` | o registro de cada unidade (dados brutos, JSONL) |
//! | POST | `/api/v1/externa/jobs/ID/cancelar` | cancela |
//!
//! Os campos de um JOB são os do formulário da tela (`dominio`, `tipo`,
//! `tamanho`, `passos`, `parametros`, `unidades`, `nivel`, `redundancia`,
//! `prazo_s`, `orcamento_milicreditos`, `descricao`), em
//! `application/x-www-form-urlencoded`.

use std::net::TcpStream;

use serde_json::{Value, json};

use super::http::{self, Pedido, responder, responder_json};
use crate::contas::{self, Conta};
use crate::servico::Nucleo;
use crate::util::{agora_unix, hex};

/// Prefixo das rotas.
pub const PREFIXO: &str = "/api/v1/externa/";

fn saldo(n: &Nucleo, conta: &Conta) -> (u64, u64) {
    let consumido: u64 = n
        .contas
        .jobs_da_conta(conta.id)
        .iter()
        .filter_map(|j| n.ciencia.situacao(j))
        .map(|s| s.milicreditos)
        .fold(0u64, u64::saturating_add);
    (consumido, conta.limite_milicreditos.saturating_sub(consumido))
}

fn json_situacao(s: &crate::ciencia::Situacao) -> Value {
    json!({
        "id": hex(&s.id),
        "estado": s.estado.nome(),
        "motivo": s.motivo,
        "feitas": s.feitas,
        "falhas": s.falhas,
        "total": s.total,
        "milicreditos": s.milicreditos,
        "orcamento_milicreditos": s.orcamento_milicreditos,
    })
}

/// Atende um pedido em `/api/v1/externa/`.
///
/// # Errors
/// Conexão caiu.
pub fn atender(s: &mut TcpStream, p: &Pedido, n: &Nucleo) -> std::io::Result<()> {
    let Some(conta) = p.autorizacao.as_deref().and_then(|a| a.strip_prefix("Bearer ")).and_then(|chave| n.contas.autenticar(chave)) else {
        return responder(s, "401 Unauthorized", "application/json; charset=utf-8", br#"{"erro":"chave de acesso ausente, errada ou revogada"}"#);
    };
    if !n.contas.dentro_do_ritmo(conta.id, agora_unix()) {
        return responder(s, "429 Too Many Requests", "application/json; charset=utf-8", br#"{"erro":"pedidos demais neste minuto"}"#);
    }
    let rota = p.rota().trim_start_matches(PREFIXO);
    let jobs_da_conta = || n.contas.jobs_da_conta(conta.id);
    match (p.metodo.as_str(), rota) {
        ("GET", "conta") => {
            let (consumido, livre) = saldo(n, &conta);
            responder_json(s, Ok(json!({
                "conta": conta.id,
                "nome": conta.nome,
                "limite_milicreditos": conta.limite_milicreditos,
                "consumido_milicreditos": consumido,
                "saldo_milicreditos": livre,
                "jobs": jobs_da_conta().len(),
                "aviso": "créditos de computação: contabilidade, não dinheiro nem HYX",
            })))
        }
        ("POST", "estimar") => {
            let r = crate::ciencia::pedido_do_formulario(&p.campos())
                .and_then(|pedido| n.ciencia.estimar(pedido))
                .and_then(|j| serde_json::from_str(&j).map_err(|e| e.to_string()));
            responder_json(s, r)
        }
        ("POST", "jobs") => {
            let r = submeter(n, &conta, &p.campos());
            responder_json(s, r)
        }
        ("GET", "jobs") => {
            let lista: Vec<Value> = jobs_da_conta().iter().filter_map(|j| n.ciencia.situacao(j)).map(|x| json_situacao(&x)).collect();
            responder_json(s, Ok(json!({ "jobs": lista })))
        }
        (metodo, r) if r.starts_with("jobs/") => {
            let resto = r.trim_start_matches("jobs/");
            let (id_texto, sub) = resto.split_once('/').unwrap_or((resto, ""));
            // JOB de outra conta (ou de ninguém) é "não existe": não confirma que existe
            let Some(id) = crate::ciencia::id_de_hex(id_texto).filter(|id| n.contas.e_da_conta(conta.id, id)) else {
                return responder(s, "404 Not Found", "application/json; charset=utf-8", br#"{"erro":"JOB desconhecido"}"#);
            };
            match (metodo, sub) {
                ("GET", "") => match n.ciencia.json_job(&id) {
                    Some(j) => responder(s, "200 OK", "application/json; charset=utf-8", j.as_bytes()),
                    None => responder(s, "404 Not Found", "application/json; charset=utf-8", br#"{"erro":"JOB desconhecido"}"#),
                },
                ("GET", "unidades") => {
                    let bytes = std::fs::read(n.ciencia.pasta_do_job(&id).join("unidades.jsonl")).unwrap_or_default();
                    responder(s, "200 OK", "application/x-ndjson; charset=utf-8", &bytes)
                }
                ("GET", arquivo) if arquivo.starts_with("relatorio.") => {
                    match crate::ciencia::relatorio::gerar(&n.ciencia, &id, arquivo.trim_start_matches("relatorio.")) {
                        Some((bytes, tipo)) => responder(s, "200 OK", tipo, &bytes),
                        None => responder(s, "404 Not Found", "application/json; charset=utf-8", r#"{"erro":"relatório desconhecido"}"#.as_bytes()),
                    }
                }
                ("POST", "cancelar") => responder_json(s, n.ciencia.mudar(&id, "cancelar").map(|()| json!({ "ok": true }))),
                _ => responder(s, "404 Not Found", "application/json; charset=utf-8", br#"{"erro":"rota desconhecida"}"#),
            }
        }
        _ => responder(s, "404 Not Found", "application/json; charset=utf-8", br#"{"erro":"rota desconhecida"}"#),
    }
}

/// Submete um JOB para a conta, dentro do saldo e do limite de JOBs abertos.
fn submeter(n: &Nucleo, conta: &Conta, campos: &[(String, String)]) -> Result<Value, String> {
    let mut pedido = crate::ciencia::pedido_do_formulario(campos)?;
    let abertos = n
        .contas
        .jobs_da_conta(conta.id)
        .iter()
        .filter_map(|j| n.ciencia.situacao(j))
        .filter(|x| !x.estado.e_final())
        .count();
    if abertos >= contas::JOBS_ABERTOS {
        return Err(format!("já há {abertos} JOBs abertos nesta conta (máximo {})", contas::JOBS_ABERTOS));
    }
    let (_, livre) = saldo(n, conta);
    if livre == 0 {
        return Err("sem saldo de créditos nesta conta".into());
    }
    // o orçamento nunca passa do saldo: o JOB para sozinho quando chegar nele
    pedido.orcamento_milicreditos = match pedido.orcamento_milicreditos {
        0 => livre,
        pedido_pela_conta => pedido_pela_conta.min(livre),
    };
    pedido.descricao = format!("[conta {} · {}] {}", conta.id, conta.nome, pedido.descricao);
    let orcamento = pedido.orcamento_milicreditos;
    let id = n.ciencia.submeter(pedido)?;
    n.contas.registrar_job(conta.id, &id)?;
    n.barramento.registrar("ciencia", format!("JOB {} submetido pela API externa (conta {} · {})", hex(&id), conta.id, conta.nome));
    Ok(json!({ "id": hex(&id), "orcamento_milicreditos": orcamento }))
}

/// O corpo aceito de fora numa rota da API externa (só os formulários).
pub fn corpo_max(rota: &str) -> usize {
    if rota.starts_with(PREFIXO) { http::CORPO_MAX } else { 0 }
}
