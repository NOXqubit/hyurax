//! O servidor local: a interface e a API v1.
//!
//! Segurança:
//! - quem vem de outro aparelho e "ver no celular" está desligado é recusado
//!   antes de qualquer byte ser lido; com ele ligado, outro aparelho só lê;
//! - todo pedido precisa de um `Host` que seja `localhost` ou um IP escrito
//!   por extenso: um domínio que aponta para 127.0.0.1 (DNS rebinding) não lê
//!   nada;
//! - comando (`POST`) e leitura completa (`/estado`, `/fluxo`, `/ciencia`)
//!   deste computador exigem a **chave de sessão do painel**: 32 bytes
//!   aleatórios a cada abertura, gravados só na pasta de configuração do
//!   usuário e entregues à janela pelo endereço (`#chave=`). Outra conta do
//!   Windows, ou um programa que não leia essa pasta, não manda nada;
//! - comando também exige `Origin` local: uma página aberta no navegador não
//!   manda o programa fazer nada;
//! - trancado (segundo fator), só passa o que destranca, e o fluxo de eventos
//!   só leva o estado mínimo;
//! - limites: conexões ao mesmo tempo (no total e por aparelho de fora),
//!   prazo para o pedido inteiro chegar, 16 KiB de cabeçalho, 8 KiB de corpo
//!   (exceto o resultado da GPU, só daqui), fluxos com vagas reservadas para
//!   este computador. Nenhum arquivo do disco é servido: a interface vem
//!   embutida no programa.
//!
//! Rotas de leitura: `/api/v1/estado`, `/api/v1/resumo`, `/api/v1/fluxo`
//! (eventos, SSE), `/api/v1/termos` e `/api/v1/ciencia/…`. Os comandos estão
//! em [`comandos`].

mod comandos;
pub mod estado;
pub mod externa;
pub mod http;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use crate::servico::{Modo, Nucleo};
use crate::ultrax;
use http::{Pedido, responder, responder_json};

/// Os arquivos da interface: (caminho, tipo MIME, bytes).
pub type Arquivos = &'static [(&'static str, &'static str, &'static [u8])];

/// O arquivo da chave de sessão do painel, na pasta de configuração.
pub const ARQUIVO_DA_CHAVE: &str = "painel.chave";

/// Conexões abertas ao mesmo tempo, no total.
const CONEXOES_MAX: usize = 64;
/// Conexões ao mesmo tempo de um mesmo aparelho de fora.
const CONEXOES_POR_APARELHO: usize = 6;
/// Fluxos de eventos ao mesmo tempo: os deste computador (com a chave) e os
/// de outros aparelhos têm vagas separadas, e um celular não tira a da janela.
const FLUXOS_DAQUI: usize = 6;
const FLUXOS_DE_FORA: usize = 4;

static CONEXOES: AtomicUsize = AtomicUsize::new(0);
static DE_FORA: Mutex<Vec<(IpAddr, usize)>> = Mutex::new(Vec::new());
static FLUXOS_LOCAIS: AtomicUsize = AtomicUsize::new(0);
static FLUXOS_REMOTOS: AtomicUsize = AtomicUsize::new(0);

/// Uma vaga de conexão; solta sozinha quando a conexão acaba.
struct Vaga {
    de_fora: Option<IpAddr>,
}

impl Vaga {
    fn reservar(ip: IpAddr, local: bool) -> Option<Self> {
        if CONEXOES.fetch_add(1, Ordering::SeqCst) >= CONEXOES_MAX {
            CONEXOES.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        if local {
            return Some(Self { de_fora: None });
        }
        let Ok(mut lista) = DE_FORA.lock() else {
            CONEXOES.fetch_sub(1, Ordering::SeqCst);
            return None;
        };
        let cheia = match lista.iter_mut().find(|(i, _)| *i == ip) {
            Some((_, n)) if *n >= CONEXOES_POR_APARELHO => true,
            Some((_, n)) => {
                *n = n.saturating_add(1);
                false
            }
            None => {
                lista.push((ip, 1));
                false
            }
        };
        drop(lista);
        if cheia {
            CONEXOES.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self { de_fora: Some(ip) })
    }
}

impl Drop for Vaga {
    fn drop(&mut self) {
        CONEXOES.fetch_sub(1, Ordering::SeqCst);
        if let Some(ip) = self.de_fora
            && let Ok(mut lista) = DE_FORA.lock()
        {
            if let Some((_, n)) = lista.iter_mut().find(|(i, _)| *i == ip) {
                *n = n.saturating_sub(1);
            }
            lista.retain(|(_, n)| *n > 0);
        }
    }
}

/// Uma vaga de fluxo de eventos; solta sozinha.
struct VagaDeFluxo(&'static AtomicUsize);

impl VagaDeFluxo {
    fn reservar(daqui: bool) -> Option<Self> {
        let (contador, maximo) = if daqui { (&FLUXOS_LOCAIS, FLUXOS_DAQUI) } else { (&FLUXOS_REMOTOS, FLUXOS_DE_FORA) };
        if contador.fetch_add(1, Ordering::SeqCst) >= maximo {
            contador.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self(contador))
    }
}

impl Drop for VagaDeFluxo {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Sorteia a chave de sessão desta abertura e grava na pasta de configuração
/// (só o dono lê). Devolve a chave.
///
/// # Errors
/// Gerador ou disco indisponível.
pub fn nova_chave(pasta_config: &Path) -> Result<String, String> {
    let chave = crate::util::hex(&hyurax_net::entropia::entropia_do_sistema()?);
    crate::arquivos::gravar_privado(&pasta_config.join(ARQUIVO_DA_CHAVE), &chave)?;
    Ok(chave)
}

/// A chave de sessão do programa que já está aberto (para uma segunda cópia
/// abrir outra janela para ele).
pub fn ler_chave(pasta_config: &Path) -> Option<String> {
    let t = std::fs::read_to_string(pasta_config.join(ARQUIVO_DA_CHAVE)).ok()?;
    let t = t.trim();
    (t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit())).then(|| t.to_string())
}

/// O endereço da janela: a chave vai depois do `#`, que o navegador nunca
/// manda ao servidor nem guarda no histórico de pedidos.
pub fn endereco_da_janela(porta: u16, chave: &str) -> String {
    format!("http://127.0.0.1:{porta}/#chave={chave}")
}

/// Abre o servidor na porta e atende numa linha própria. Devolve a porta.
///
/// # Errors
/// Porta ocupada.
pub fn abrir(n: &Arc<Nucleo>, porta: u16, arquivos: Arquivos) -> Result<u16, String> {
    // no programa com janela a porta sempre abre para a rede, e quem decide se
    // responde a outro aparelho é o ajuste "ver no celular", conferido antes
    // de ler qualquer byte
    let ip = if n.modo == Modo::Janela || n.na_rede.load(Ordering::Relaxed) || n.api_externa.load(Ordering::Relaxed) {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };
    let ouvinte = TcpListener::bind(SocketAddr::new(ip, porta)).map_err(|e| format!("não consegui abrir o painel na porta {porta}: {e}"))?;
    let porta = ouvinte.local_addr().map_or(porta, |a| a.port());
    let n = Arc::clone(n);
    std::thread::spawn(move || {
        for conexao in ouvinte.incoming() {
            let Ok(mut s) = conexao else { continue };
            let Ok(par) = s.peer_addr() else { continue };
            let local = par.ip().is_loopback();
            if !local && !n.na_rede.load(Ordering::Relaxed) && !n.api_externa.load(Ordering::Relaxed) {
                // recusa sem ler nada e sem abrir linha: a resposta cabe no
                // buffer do sistema, então a escrita não bloqueia
                let _ = s.set_nonblocking(true);
                let _ = s.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                continue;
            }
            let Some(vaga) = Vaga::reservar(par.ip(), local) else { continue };
            let n = Arc::clone(&n);
            std::thread::spawn(move || {
                let _vaga = vaga;
                let _ = atender(s, porta, local, &n, arquivos);
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

/// `Host` aceito: `localhost`, `[::1]` ou um IPv4 por extenso, na porta
/// certa. Domínio nunca: é o que fecha o DNS rebinding.
fn host_valido(host: &str, porta: u16) -> bool {
    let Some((nome, p)) = host.rsplit_once(':') else { return false };
    p.parse::<u16>().ok() == Some(porta) && (nome == "localhost" || nome == "[::1]" || nome.parse::<Ipv4Addr>().is_ok())
}

/// `Host` deste próprio computador.
fn host_local(host: &str, porta: u16) -> bool {
    [format!("127.0.0.1:{porta}"), format!("localhost:{porta}"), format!("[::1]:{porta}")].iter().any(|h| h == host)
}

fn atender(mut s: TcpStream, porta: u16, local: bool, n: &Arc<Nucleo>, arquivos: Arquivos) -> std::io::Result<()> {
    // de fora, só leitura: nenhum corpo é aceito (fora os formulários da API externa)
    let corpo_max = |rota: &str| {
        if !local {
            externa::corpo_max(rota)
        } else if rota.starts_with("/api/v1/gpu/resultado/") {
            ultrax::GPU_RESULTADO_MAX
        } else {
            http::CORPO_MAX
        }
    };
    let Some(p) = http::ler(&mut s, &corpo_max) else {
        return responder(&mut s, "400 Bad Request", "text/plain", b"pedido invalido");
    };
    if !host_valido(&p.host, porta) {
        return responder(&mut s, "403 Forbidden", "text/plain", b"host desconhecido");
    }
    // a API externa: só com ela ligada, e só com a chave de uma conta
    if p.rota().starts_with(externa::PREFIXO) {
        if !n.api_externa.load(Ordering::Relaxed) {
            return responder(&mut s, "403 Forbidden", "application/json; charset=utf-8", r#"{"erro":"a API externa deste nó está desligada"}"#.as_bytes());
        }
        return externa::atender(&mut s, &p, n);
    }
    // com só a API externa ligada, o resto continua só deste computador
    if !local && !n.na_rede.load(Ordering::Relaxed) {
        return responder(&mut s, "403 Forbidden", "text/plain", b"painel so neste computador");
    }
    let com_chave = p.chave_enviada().is_some_and(|c| http::mesma_chave(c, &n.chave_painel));
    // a janela deste computador: daqui, Host local e a chave desta abertura
    let daqui = local && host_local(&p.host, porta) && com_chave;
    if p.metodo == "GET" {
        return ler(s, &p, n, arquivos, daqui, porta);
    }
    if p.metodo != "POST" {
        return responder(&mut s, "405 Method Not Allowed", "text/plain", b"metodo");
    }
    let origem_ok = p.origem.as_deref().is_none_or(|og| {
        [format!("http://127.0.0.1:{porta}"), format!("http://localhost:{porta}")].iter().any(|h| og == h)
    });
    if !(daqui && origem_ok) {
        return responder(&mut s, "403 Forbidden", "text/plain", b"comando so pela janela deste computador");
    }
    if n.carteira.trancado() && p.rota() != "/api/v1/destravar" {
        return responder(&mut s, "403 Forbidden", "text/plain", b"programa trancado");
    }
    comandos::atender(&mut s, &p, n)
}

fn ler(mut s: TcpStream, p: &Pedido, n: &Arc<Nucleo>, arquivos: Arquivos, daqui: bool, porta: u16) -> std::io::Result<()> {
    let rota = p.rota();
    if !rota.starts_with("/api/") {
        let alvo = if rota == "/" { "/index.html" } else { rota };
        return match arquivos.iter().find(|(c, _, _)| *c == alvo) {
            Some((_, tipo, bytes)) => responder(&mut s, "200 OK", tipo, bytes),
            None => responder(&mut s, "404 Not Found", "text/plain", b"nao existe"),
        };
    }
    // públicas: o resumo (para as outras máquinas do dono) e os termos
    match rota {
        "/api/v1/resumo" => {
            // Com `?desafio=` (32 bytes em hex), o resumo vai assinado pela
            // chave do worker deste nó: quem pergunta sabe que foi esta
            // máquina, e agora, que respondeu (ver `maquinas`).
            let corpo = estado::resumo(n).to_string();
            let extras = match p.parametro("desafio").and_then(crate::util::de_hex::<32>) {
                Some(desafio) => {
                    let assinatura = hyurax_crypto::ed25519_sign(&n.chave_do_resumo, &crate::maquinas::mensagem(&desafio, corpo.as_bytes()));
                    vec![
                        (crate::maquinas::CABECALHO_CHAVE, crate::util::hex(&hyurax_crypto::ed25519_public_key(&n.chave_do_resumo))),
                        (crate::maquinas::CABECALHO_ASSINATURA, crate::util::hex(&assinatura)),
                    ]
                }
                None => Vec::new(),
            };
            return http::responder_com(&mut s, "200 OK", "application/json; charset=utf-8", &extras, corpo.as_bytes());
        }
        "/api/v1/termos" => return responder(&mut s, "200 OK", "text/html; charset=utf-8", crate::termos::html().as_bytes()),
        _ => {}
    }
    // o resto: a janela daqui com a chave, ou só leitura com "ver no celular"
    if !daqui && !n.na_rede.load(Ordering::Relaxed) {
        return responder(&mut s, "401 Unauthorized", "text/plain", b"sem a chave desta abertura: abra pela janela do programa");
    }
    let json_ok = |s: &mut TcpStream, v: serde_json::Value| responder(s, "200 OK", "application/json; charset=utf-8", v.to_string().as_bytes());
    match rota {
        "/api/v1/estado" => {
            let url = if n.na_rede.load(Ordering::Relaxed) { ip_local().map(|ip| format!("http://{ip}:{porta}/")).unwrap_or_default() } else { String::new() };
            json_ok(&mut s, estado::estado(n, daqui, &url))
        }
        "/api/v1/fluxo" => fluxo(s, p, n, daqui, porta),
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
            match numero.filter(|_| daqui).and_then(|k| n.ultrax.gpu_entrada(k)) {
                Some(bytes) => responder(&mut s, "200 OK", "application/octet-stream", &bytes),
                None => responder(&mut s, "404 Not Found", "text/plain", b"tarefa de GPU desconhecida"),
            }
        }
        _ => responder_json(&mut s, Err("rota desconhecida".into())),
    }
}

/// O fluxo de eventos: tudo o que o barramento publica depois de `desde`, e
/// o estado inteiro a cada segundo. Fica aberto até a tela fechar. Com o
/// programa trancado, só o estado mínimo passa (o que o cadeado esconde não
/// sai pelo fluxo).
fn fluxo(mut s: TcpStream, p: &Pedido, n: &Arc<Nucleo>, daqui: bool, porta: u16) -> std::io::Result<()> {
    let Some(_vaga) = VagaDeFluxo::reservar(daqui) else {
        return responder(&mut s, "503 Service Unavailable", "text/plain", b"fluxos demais abertos");
    };
    http::comecar_fluxo(&mut s)?;
    let mut seq = p.parametro("desde").and_then(|v| v.parse::<u64>().ok()).unwrap_or_else(|| n.barramento.ultimo_seq());
    let mut ultimo_estado: Option<std::time::Instant> = None;
    loop {
        let eventos = n.barramento.esperar(seq, 256, Duration::from_millis(1000));
        let trancado = n.carteira.trancado();
        for e in &eventos {
            if !trancado {
                http::evento(&mut s, e.seq, e.tipo, &e.json)?;
            }
            seq = e.seq;
        }
        // o estado inteiro, no máximo uma vez por segundo: é também a
        // batida que diz à tela que a conexão vive
        if ultimo_estado.is_some_and(|t| t.elapsed() < Duration::from_millis(1000)) {
            continue;
        }
        ultimo_estado = Some(std::time::Instant::now());
        let url = if n.na_rede.load(Ordering::Relaxed) { ip_local().map(|ip| format!("http://{ip}:{porta}/")).unwrap_or_default() } else { String::new() };
        http::evento(&mut s, seq, "estado", &estado::estado(n, daqui, &url).to_string())?;
    }
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
