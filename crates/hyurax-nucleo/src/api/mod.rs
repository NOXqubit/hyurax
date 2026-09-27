//! O servidor local: a interface e a API v1.
//!
//! Segurança:
//! - comando (`POST`) só vem do próprio computador, com `Host` e `Origin`
//!   locais: uma página maliciosa aberta no navegador não manda o programa
//!   fazer nada, nem apontando um DNS para 127.0.0.1;
//! - outro aparelho da rede local só lê, e só com "ver no celular" ligado;
//! - trancado (segundo fator), só passa o que destranca;
//! - pedido limitado (16 KiB de cabeçalho, 8 KiB de corpo, exceto o
//!   resultado da GPU), e nenhum arquivo do disco é servido: a interface vem
//!   embutida no programa.
//!
//! Rotas de leitura: `/api/v1/estado`, `/api/v1/resumo`, `/api/v1/fluxo`
//! (eventos, SSE), `/api/v1/termos` e `/api/v1/ciencia/…`. Os comandos estão
//! em [`comandos`].

mod comandos;
pub mod estado;
pub mod http;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;

use crate::servico::{Modo, Nucleo};
use crate::ultrax;
use http::{Pedido, responder, responder_json};

/// Os arquivos da interface: (caminho, tipo MIME, bytes).
pub type Arquivos = &'static [(&'static str, &'static str, &'static [u8])];

/// Quantos fluxos de eventos abertos ao mesmo tempo.
const FLUXOS_MAX: usize = 8;
static FLUXOS: AtomicUsize = AtomicUsize::new(0);

/// Abre o servidor na porta e atende numa linha própria. Devolve a porta.
///
/// # Errors
/// Porta ocupada.
pub fn abrir(n: &Arc<Nucleo>, porta: u16, arquivos: Arquivos) -> Result<u16, String> {
    // no programa com janela a porta sempre abre para a rede, e quem decide se
    // responde a outro aparelho é o ajuste "ver no celular" (403 sem ele)
    let ip = if n.modo == Modo::Janela || n.na_rede.load(Ordering::Relaxed) {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };
    let ouvinte = TcpListener::bind(SocketAddr::new(ip, porta)).map_err(|e| format!("não consegui abrir o painel na porta {porta}: {e}"))?;
    let porta = ouvinte.local_addr().map_or(porta, |a| a.port());
    let n = Arc::clone(n);
    std::thread::spawn(move || {
        for conexao in ouvinte.incoming() {
            let Ok(s) = conexao else { continue };
            let n = Arc::clone(&n);
            std::thread::spawn(move || {
                let _ = atender(s, porta, &n, arquivos);
            });
        }
    });
    Ok(porta)
}

/// Já há um Hyurax respondendo nesta porta? Dois núcleos na mesma pasta
/// estragariam a cadeia gravada: o segundo só abre uma janela para o primeiro.
pub fn ja_aberto(porta: u16) -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), porta), Duration::from_millis(400)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let pedido = format!("GET /api/v1/resumo HTTP/1.1\r\nHost: 127.0.0.1:{porta}\r\nConnection: close\r\n\r\n");
    let mut resposta = Vec::new();
    if s.write_all(pedido.as_bytes()).is_ok() {
        let _ = s.read_to_end(&mut resposta);
    }
    String::from_utf8_lossy(&resposta).contains("\"produto\":\"Hyurax")
}

/// O endereço deste computador na rede local, para abrir o painel no celular.
/// Não manda pacote: só pergunta ao sistema por qual placa sairia.
fn ip_local() -> Option<Ipv4Addr> {
    let s = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("192.0.2.1", 9)).ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

fn atender(mut s: TcpStream, porta: u16, n: &Arc<Nucleo>, arquivos: Arquivos) -> std::io::Result<()> {
    let corpo_max = |rota: &str| if rota.starts_with("/api/v1/gpu/resultado/") { ultrax::GPU_RESULTADO_MAX } else { http::CORPO_MAX };
    let Some(p) = http::ler(&mut s, &corpo_max) else {
        return responder(&mut s, "400 Bad Request", "text/plain", b"pedido invalido");
    };
    let local = s.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
    let hosts_locais = [format!("127.0.0.1:{porta}"), format!("localhost:{porta}")];
    let host_local = hosts_locais.contains(&p.host);
    if !local && !n.na_rede.load(Ordering::Relaxed) {
        return responder(&mut s, "403 Forbidden", "text/plain", b"ligue 'ver no celular' nos ajustes");
    }
    let pode_mandar = local && host_local;
    if p.metodo == "GET" {
        return ler(s, &p, n, arquivos, pode_mandar, porta);
    }
    if p.metodo != "POST" {
        return responder(&mut s, "405 Method Not Allowed", "text/plain", b"metodo");
    }
    let origem_ok = p.origem.as_deref().is_none_or(|og| hosts_locais.iter().any(|h| og == format!("http://{h}")));
    if !(pode_mandar && origem_ok) {
        return responder(&mut s, "403 Forbidden", "text/plain", b"comando so deste computador");
    }
    if n.carteira.trancado() && p.rota() != "/api/v1/destravar" {
        return responder(&mut s, "403 Forbidden", "text/plain", b"programa trancado");
    }
    comandos::atender(&mut s, &p, n)
}

fn ler(mut s: TcpStream, p: &Pedido, n: &Arc<Nucleo>, arquivos: Arquivos, pode_mandar: bool, porta: u16) -> std::io::Result<()> {
    let rota = p.rota();
    if !rota.starts_with("/api/") {
        let alvo = if rota == "/" { "/index.html" } else { rota };
        return match arquivos.iter().find(|(c, _, _)| *c == alvo) {
            Some((_, tipo, bytes)) => responder(&mut s, "200 OK", tipo, bytes),
            None => responder(&mut s, "404 Not Found", "text/plain", b"nao existe"),
        };
    }
    let json_ok = |s: &mut TcpStream, v: serde_json::Value| responder(s, "200 OK", "application/json; charset=utf-8", v.to_string().as_bytes());
    match rota {
        "/api/v1/estado" => {
            let url = if n.na_rede.load(Ordering::Relaxed) { ip_local().map(|ip| format!("http://{ip}:{porta}/")).unwrap_or_default() } else { String::new() };
            json_ok(&mut s, estado::estado(n, pode_mandar, &url))
        }
        "/api/v1/resumo" => json_ok(&mut s, estado::resumo(n)),
        "/api/v1/termos" => responder(&mut s, "200 OK", "text/html; charset=utf-8", crate::termos::html().as_bytes()),
        "/api/v1/fluxo" => fluxo(s, p, n, pode_mandar, porta),
        r if r.starts_with("/api/v1/ciencia") && n.carteira.trancado() => responder(&mut s, "403 Forbidden", "text/plain", b"programa trancado"),
        "/api/v1/ciencia" => responder(&mut s, "200 OK", "application/json; charset=utf-8", n.ciencia.json().as_bytes()),
        "/api/v1/ciencia/benchmarks" => {
            let (rodando, ultimo) = n.ciencia.benchmark.lock().map(|b| b.clone()).unwrap_or_default();
            let ultimo: serde_json::Value = serde_json::from_str(&ultimo).unwrap_or(serde_json::Value::Null);
            let registradas: serde_json::Value =
                serde_json::from_str(&crate::ciencia::bancada::registradas(&n.ciencia)).unwrap_or(serde_json::Value::Null);
            json_ok(&mut s, json!({ "rodando": rodando, "ultimo": ultimo, "registradas": registradas }))
        }
        "/api/v1/ciencia/eventos" => {
            let desde = p.parametro("desde").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            responder(&mut s, "200 OK", "application/json; charset=utf-8", n.ciencia.eventos_desde(desde).as_bytes())
        }
        r if r.starts_with("/api/v1/ciencia/molecula/") => {
            match r.trim_start_matches("/api/v1/ciencia/molecula/").parse::<usize>().ok().and_then(json_da_molecula) {
                Some(j) => json_ok(&mut s, j),
                None => responder(&mut s, "404 Not Found", "text/plain", b"molecula fora do catalogo"),
            }
        }
        r if r.starts_with("/api/v1/ciencia/rotas/") => {
            let mut partes = r.trim_start_matches("/api/v1/ciencia/rotas/").split('/').map(|v| v.parse::<u32>().ok());
            match (partes.next().flatten(), partes.next().flatten()) {
                (Some(instancia), Some(k)) if (hyurax_ultrax::rotas::TAMANHO.0..=hyurax_ultrax::rotas::TAMANHO.1).contains(&k) => {
                    let cidades: Vec<[u32; 2]> = hyurax_ultrax::rotas::coordenadas(instancia, k).iter().map(|c| [c.x, c.y]).collect();
                    json_ok(&mut s, json!({ "instancia": instancia, "lado": hyurax_ultrax::rotas::LADO, "cidades": cidades }))
                }
                _ => responder(&mut s, "404 Not Found", "text/plain", b"instancia ou numero de cidades invalido"),
            }
        }
        r if r.starts_with("/api/v1/ciencia/job/") => {
            let resto = r.trim_start_matches("/api/v1/ciencia/job/");
            let (id_texto, sub) = resto.split_once('/').unwrap_or((resto, ""));
            match (crate::ciencia::id_de_hex(id_texto), sub) {
                (Some(id), "") => match n.ciencia.json_job(&id) {
                    Some(j) => responder(&mut s, "200 OK", "application/json; charset=utf-8", j.as_bytes()),
                    None => responder(&mut s, "404 Not Found", "text/plain", b"JOB desconhecido"),
                },
                (Some(id), "historico") => match n.ciencia.historico_do_job(&id) {
                    Some(j) => responder(&mut s, "200 OK", "application/json; charset=utf-8", j.as_bytes()),
                    None => responder(&mut s, "404 Not Found", "text/plain", b"sem historico"),
                },
                (Some(id), arquivo) if arquivo.starts_with("relatorio.") => {
                    match crate::ciencia::relatorio::gerar(&n.ciencia, &id, arquivo.trim_start_matches("relatorio.")) {
                        Some((bytes, tipo)) => responder(&mut s, "200 OK", tipo, &bytes),
                        None => responder(&mut s, "404 Not Found", "text/plain", b"relatorio desconhecido"),
                    }
                }
                _ => responder(&mut s, "404 Not Found", "text/plain", b"nao existe"),
            }
        }
        // as matrizes de uma tarefa da GPU: só para a janela deste computador
        r if r.starts_with("/api/v1/gpu/entrada/") => {
            let numero = r.trim_start_matches("/api/v1/gpu/entrada/").parse::<u32>().ok();
            match numero.filter(|_| pode_mandar).and_then(|k| n.ultrax.gpu_entrada(k)) {
                Some(bytes) => responder(&mut s, "200 OK", "application/octet-stream", &bytes),
                None => responder(&mut s, "404 Not Found", "text/plain", b"tarefa de GPU desconhecida"),
            }
        }
        _ => responder_json(&mut s, Err("rota desconhecida".into())),
    }
}

/// O fluxo de eventos: tudo o que o barramento publica depois de `desde`, e
/// o estado inteiro a cada segundo. Fica aberto até a tela fechar.
fn fluxo(mut s: TcpStream, p: &Pedido, n: &Arc<Nucleo>, pode_mandar: bool, porta: u16) -> std::io::Result<()> {
    if FLUXOS.fetch_add(1, Ordering::SeqCst) >= FLUXOS_MAX {
        FLUXOS.fetch_sub(1, Ordering::SeqCst);
        return responder(&mut s, "503 Service Unavailable", "text/plain", b"fluxos demais abertos");
    }
    let r = (|| {
        http::comecar_fluxo(&mut s)?;
        let mut seq = p.parametro("desde").and_then(|v| v.parse::<u64>().ok()).unwrap_or_else(|| n.barramento.ultimo_seq());
        let _ = s.set_write_timeout(Some(Duration::from_secs(10)));
        let mut ultimo_estado: Option<std::time::Instant> = None;
        loop {
            let eventos = n.barramento.esperar(seq, 256, Duration::from_millis(1000));
            for e in &eventos {
                http::evento(&mut s, e.seq, e.tipo, &e.json)?;
                seq = e.seq;
            }
            // o estado inteiro, no máximo uma vez por segundo: é também a
            // batida que diz à tela que a conexão vive
            if ultimo_estado.is_some_and(|t| t.elapsed() < Duration::from_millis(1000)) {
                continue;
            }
            ultimo_estado = Some(std::time::Instant::now());
            let url = if n.na_rede.load(Ordering::Relaxed) { ip_local().map(|ip| format!("http://{ip}:{porta}/")).unwrap_or_default() } else { String::new() };
            http::evento(&mut s, seq, "estado", &estado::estado(n, pode_mandar, &url).to_string())?;
        }
    })();
    FLUXOS.fetch_sub(1, Ordering::SeqCst);
    r
}

/// Uma molécula do catálogo da triagem, com os descritores, para a tela
/// desenhar a que está sendo calculada.
fn json_da_molecula(i: usize) -> Option<serde_json::Value> {
    let m = hyurax_ultrax::triagem::catalogo().moleculas.get(i)?;
    Some(json!({
        "indice": i,
        "id": m.id,
        "nome": m.nome,
        "formula": m.formula,
        "smiles": m.smiles,
        "logs_medido_mili": m.logs_mili,
        "descritores": m.bruto,
    }))
}
