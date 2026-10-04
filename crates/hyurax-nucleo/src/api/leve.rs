//! A API das carteiras leves: o aplicativo do celular (`mobile/`) não guarda
//! a cadeia, então pergunta a um nó o saldo, o nonce e o histórico de um
//! endereço, e entrega a ele a transação **já assinada no celular**.
//!
//! O segredo nunca passa por aqui. O nó só lê a cadeia e repassa uma
//! transação que a própria validação dele confere (assinatura, nonce, saldo,
//! magic da rede) antes de aceitar. Por isso não há chave de acesso: é o
//! mesmo que qualquer par da rede já pode pedir, e o pior que um curioso
//! consegue é ler o que a cadeia já mostra a todos.
//!
//! Desligada por padrão; o dono liga em Ajustes ("Servir carteiras de
//! celular") ou com `hyurax-no painel --carteiras`. Com ela ligada, o nó
//! aceita conexões de fora só nestas rotas.
//!
//! | Método | Rota | O que faz |
//! |---|---|---|
//! | GET | `/api/v1/leve/info` | rede, prefixo dos endereços, magic, altura, maturidade, taxa sugerida |
//! | GET | `/api/v1/leve/conta/ENDERECO` | saldo gastável, saldo imaturo e o próximo nonce |
//! | GET | `/api/v1/leve/historico/ENDERECO` | os últimos movimentos (esperando e em bloco) |
//! | POST | `/api/v1/leve/transacao` | corpo: a transação codificada, em hexadecimal; devolve o txid |
//!
//! Valores sempre em unidades inteiras (`_unidades`, 1 HYX = 10^8), e também
//! em texto para exibição. Limite: 60 pedidos por minuto por IP.

use std::collections::HashMap;
use std::net::{IpAddr, TcpStream};
use std::sync::Mutex;

use hyurax_tx::{HYX, Tx, decode_tx};
use serde_json::{Value, json};

use super::http::{Pedido, responder_com};
use crate::carteira::{endereco, envio};
use crate::servico::Nucleo;
use crate::util::{agora_unix, hex, hyx};
use crate::{PRODUTO, VERSAO};

/// Prefixo das rotas.
pub const PREFIXO: &str = "/api/v1/leve/";
/// Maior transação aceita no corpo, em bytes (o texto hexadecimal tem o dobro).
const TX_MAX: usize = 16 * 1024;
/// Pedidos por minuto por IP.
const RITMO: u32 = 60;
/// Movimentos devolvidos no histórico.
const MOVIMENTOS: usize = 50;

/// Corpo aceito em cada rota (zero: nenhum).
pub fn corpo_max(rota: &str) -> usize {
    if rota == "/api/v1/leve/transacao" { TX_MAX * 2 + 2 } else { 0 }
}

/// Janela de um minuto por IP. Os IPs velhos saem quando o mapa cresce.
fn dentro_do_ritmo(ip: IpAddr, agora: u64) -> bool {
    static JANELAS: Mutex<Option<HashMap<IpAddr, (u64, u32)>>> = Mutex::new(None);
    let Ok(mut guarda) = JANELAS.lock() else { return false };
    let mapa = guarda.get_or_insert_with(HashMap::new);
    let minuto = agora / 60;
    if mapa.len() > 4096 {
        mapa.retain(|_, (m, _)| *m == minuto);
    }
    let janela = mapa.entry(ip).or_insert((minuto, 0));
    if janela.0 != minuto {
        *janela = (minuto, 0);
    }
    janela.1 = janela.1.saturating_add(1);
    janela.1 <= RITMO
}

/// Respostas com `Access-Control-Allow-Origin: *`: a API é pública, sem
/// cookie nem chave, e assim uma carteira web também consegue ler. Só pedidos
/// simples (GET, e POST com `text/plain`), então não há pré-voo.
fn json(s: &mut TcpStream, status: &str, v: &Value) -> std::io::Result<()> {
    responder_com(s, status, "application/json; charset=utf-8", &[("Access-Control-Allow-Origin", "*".to_string())], v.to_string().as_bytes())
}

fn erro(s: &mut TcpStream, status: &str, mensagem: &str) -> std::io::Result<()> {
    json(s, status, &json!({ "erro": mensagem }))
}

fn resultado(s: &mut TcpStream, r: Result<Value, String>) -> std::io::Result<()> {
    match r {
        Ok(v) => json(s, "200 OK", &v),
        Err(m) => erro(s, "400 Bad Request", &m),
    }
}

fn de_hex(texto: &str) -> Option<Vec<u8>> {
    let texto = texto.trim();
    if !texto.len().is_multiple_of(2) || texto.len() > TX_MAX * 2 {
        return None;
    }
    (0..texto.len()).step_by(2).map(|i| texto.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect()
}

fn info(n: &Nucleo) -> Value {
    let rede = &n.config.rede;
    let (altura, pares) = (n.rede.no.lock().map(|no| no.chain.height()).unwrap_or(0), n.rede.pares_conectados());
    json!({
        "produto": PRODUTO,
        "versao": VERSAO,
        "rede": rede.nome,
        "prefixo": endereco::prefixo(rede.nome),
        "magic": hex(&rede.magic),
        "altura": altura,
        "pares": pares,
        "maturidade": rede.coinbase_maturity,
        "taxa_sugerida_unidades": envio::TAXA_PADRAO,
        "unidade": 100_000_000u64,
    })
}

fn conta(n: &Nucleo, e: &[u8; 20]) -> Result<Value, String> {
    let no = n.rede.no.lock().map_err(|_| "nó travado".to_string())?;
    let c = &no.chain;
    let saldo = c.state.balance(e, &HYX);
    let imaturo = c.state.immature_balance(e);
    Ok(json!({
        "endereco": endereco::mostrar(e, n.config.rede.nome),
        "saldo_unidades": saldo,
        "saldo": hyx(u128::from(saldo)),
        "imaturo_unidades": imaturo,
        "imaturo": hyx(imaturo),
        "proximo_nonce": no.proximo_nonce(e),
        "altura": c.height(),
    }))
}

fn historico(n: &Nucleo, e: &[u8; 20]) -> Result<Value, String> {
    let no = n.rede.no.lock().map_err(|_| "nó travado".to_string())?;
    let movimentos: Vec<Value> = envio::historico(&no.chain, &no.mempool_ordenado(), e, MOVIMENTOS)
        .iter()
        .map(|m| {
            json!({
                "quando": m.quando, "altura": m.altura, "pendente": m.pendente, "entrada": m.entrada,
                "outro": m.outro, "valor_unidades": m.valor, "taxa_unidades": m.taxa,
                "valor": hyx(u128::from(m.valor)), "txid": m.txid, "tipo": m.tipo,
            })
        })
        .collect();
    Ok(json!({ "movimentos": movimentos, "altura": no.chain.height() }))
}

fn transacao(n: &Nucleo, corpo: &[u8]) -> Result<Value, String> {
    let texto = std::str::from_utf8(corpo).map_err(|_| "o corpo precisa ser texto hexadecimal".to_string())?;
    let bytes = de_hex(texto).ok_or("o corpo precisa ser a transação em hexadecimal")?;
    let Tx::Transfer(t) = decode_tx(&bytes).map_err(|e| format!("transação malformada: {e}"))? else {
        return Err("só transferências entram por aqui".into());
    };
    let txid = Tx::Transfer(t.clone()).txid().map_err(|e| e.to_string())?;
    // o mempool só diz "não entrou"; o motivo, para o app mostrar, sai daqui
    let (proximo, saldo) = {
        let no = n.rede.no.lock().map_err(|_| "nó travado".to_string())?;
        (no.proximo_nonce(&t.sender), no.chain.state.balance(&t.sender, &HYX))
    };
    let custo = t.costs().ok().and_then(|c| c.get(&HYX).copied()).unwrap_or(0);
    match n.rede.submeter_tx(t.clone()) {
        Ok(true) => Ok(json!({ "txid": hex(&txid), "pares": n.rede.pares_conectados() })),
        Ok(false) if t.nonce < proximo => Err(format!("nonce {} já usado; o próximo desta conta é {proximo}", t.nonce)),
        Ok(false) if t.nonce > proximo => Err(format!("nonce {} adiantado; o próximo desta conta é {proximo}", t.nonce)),
        Ok(false) => Err(format!(
            "saldo gastável insuficiente: a conta tem {} HYX livres e a transação custa {} HYX",
            hyx(u128::from(saldo)),
            hyx(u128::from(custo))
        )),
        Err(m) => Err(format!("a validação do nó recusou: {}", m.0)),
    }
}

/// Atende um pedido em `/api/v1/leve/`.
///
/// # Errors
/// Conexão caiu.
pub fn atender(s: &mut TcpStream, p: &Pedido, n: &Nucleo, ip: IpAddr) -> std::io::Result<()> {
    if !dentro_do_ritmo(ip, agora_unix()) {
        return erro(s, "429 Too Many Requests", "pedidos demais neste minuto");
    }
    let rota = p.rota().trim_start_matches(PREFIXO);
    let rede = n.config.rede.nome;
    let ler_endereco = |t: &str| endereco::ler(t, rede);
    match (p.metodo.as_str(), rota) {
        ("GET", "info") => resultado(s, Ok(info(n))),
        ("GET", r) if r.starts_with("conta/") => match ler_endereco(r.trim_start_matches("conta/")) {
            Ok(e) => resultado(s, conta(n, &e)),
            Err(m) => erro(s, "400 Bad Request", &m),
        },
        ("GET", r) if r.starts_with("historico/") => match ler_endereco(r.trim_start_matches("historico/")) {
            Ok(e) => resultado(s, historico(n, &e)),
            Err(m) => erro(s, "400 Bad Request", &m),
        },
        ("POST", "transacao") => {
            let r = transacao(n, &p.corpo);
            if let Ok(v) = &r {
                n.barramento.registrar("carteira", format!("transação de carteira de celular repassada à rede ({})", v["txid"].as_str().unwrap_or("")));
            }
            resultado(s, r)
        }
        _ => erro(s, "404 Not Found", "rota desconhecida"),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ritmo_por_ip_e_por_minuto() {
        let ip: IpAddr = "203.0.113.9".parse().unwrap_or(IpAddr::from([0, 0, 0, 0]));
        let agora = 6_000_000;
        assert!((0..RITMO).all(|_| dentro_do_ritmo(ip, agora)));
        assert!(!dentro_do_ritmo(ip, agora));
        assert!(dentro_do_ritmo(ip, agora + 60), "minuto novo zera");
    }

    #[test]
    fn hexadecimal_e_corpo() {
        assert_eq!(de_hex("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(de_hex("0"), None);
        assert_eq!(de_hex("zz"), None);
        assert_eq!(corpo_max("/api/v1/leve/transacao"), TX_MAX * 2 + 2);
        assert_eq!(corpo_max("/api/v1/leve/info"), 0);
    }
}
