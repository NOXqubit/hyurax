//! HTTP/1.1 mínimo e defensivo para a plataforma.
//!
//! - cabeçalho até 16 KiB, corpo com teto por rota (o resultado do agente
//!   pode ter 9 MiB; o resto, 256 KiB), prazo de 15 s para o pedido chegar;
//! - corpo só com `Content-Length` (sem `chunked` na entrada);
//! - uma conexão por pedido (`Connection: close`) e no máximo
//!   [`CONEXOES_MAX`] ao mesmo tempo;
//! - cabeçalhos de segurança em toda resposta; CSP estrita nas páginas;
//! - atrás de proxy (Render, Cloudflare), o IP do cliente vem do
//!   `X-Forwarded-For`, contado da direita pelo número de saltos confiáveis.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;

use crate::Plataforma;
use crate::servico::Erro;

/// Conexões atendidas ao mesmo tempo.
pub const CONEXOES_MAX: usize = 128;
const CABECALHO_MAX: usize = 16 * 1024;
/// Corpo padrão.
pub const CORPO_PADRAO: usize = 256 * 1024;
const PRAZO_DO_PEDIDO: Duration = Duration::from_secs(15);

/// Um pedido.
#[derive(Clone, Debug, Default)]
pub struct Pedido {
    /// Método.
    pub metodo: String,
    /// Caminho, sem a busca.
    pub caminho: String,
    /// A busca (depois do `?`), sem decodificar.
    pub busca: String,
    /// Cabeçalhos, com o nome em minúsculas.
    pub cabecalhos: Vec<(String, String)>,
    /// Corpo.
    pub corpo: Vec<u8>,
    /// IP do cliente.
    pub ip: String,
}

impl Pedido {
    /// Um cabeçalho.
    pub fn cabecalho(&self, nome: &str) -> Option<&str> {
        self.cabecalhos.iter().find(|(k, _)| k == nome).map(|(_, v)| v.as_str())
    }

    /// Um cookie.
    pub fn cookie(&self, nome: &str) -> Option<&str> {
        self.cabecalho("cookie")?.split(';').find_map(|par| {
            let (k, v) = par.trim().split_once('=')?;
            (k == nome).then_some(v)
        })
    }

    /// Um parâmetro da busca, decodificado.
    pub fn parametro(&self, nome: &str) -> Option<String> {
        self.busca.split('&').find_map(|par| {
            let (k, v) = par.split_once('=').unwrap_or((par, ""));
            (k == nome).then(|| decodificar(v))
        })
    }

    /// O corpo como JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, Erro> {
        serde_json::from_slice(&self.corpo).map_err(|e| Erro::pedido(format!("JSON inválido: {e}")))
    }
}

/// Decodifica `%XX` e `+`.
pub fn decodificar(texto: &str) -> String {
    let b = texto.as_bytes();
    let mut saida = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while let Some(&c) = b.get(i) {
        match c {
            b'+' => saida.push(b' '),
            b'%' => {
                let h = b.get(i.saturating_add(1)..i.saturating_add(3)).and_then(|x| std::str::from_utf8(x).ok()).and_then(|x| u8::from_str_radix(x, 16).ok());
                if let Some(v) = h {
                    saida.push(v);
                    i = i.saturating_add(2);
                } else {
                    saida.push(c);
                }
            }
            _ => saida.push(c),
        }
        i = i.saturating_add(1);
    }
    String::from_utf8_lossy(&saida).into_owned()
}

/// Uma resposta.
#[derive(Clone, Debug)]
pub struct Resposta {
    /// Status.
    pub status: u16,
    /// `Content-Type`.
    pub tipo: String,
    /// Corpo.
    pub corpo: Vec<u8>,
    /// Cabeçalhos a mais.
    pub extras: Vec<(String, String)>,
}

impl Resposta {
    /// JSON.
    pub fn json(status: u16, v: &serde_json::Value) -> Self {
        Self { status, tipo: "application/json; charset=utf-8".into(), corpo: v.to_string().into_bytes(), extras: Vec::new() }
    }

    /// Erro em JSON: `{"erro": "..."}`.
    pub fn erro(e: &Erro) -> Self {
        Self::json(e.status, &serde_json::json!({ "erro": e.mensagem }))
    }

    /// Bytes.
    pub fn bytes(tipo: &str, corpo: Vec<u8>) -> Self {
        Self { status: 200, tipo: tipo.into(), corpo, extras: Vec::new() }
    }

    /// Mais um cabeçalho.
    pub fn com(mut self, nome: &str, valor: String) -> Self {
        self.extras.push((nome.into(), valor));
        self
    }
}

fn razao(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    }
}

/// Escreve a resposta com os cabeçalhos de segurança.
pub fn escrever(s: &mut TcpStream, r: &Resposta, https: bool) -> std::io::Result<()> {
    let mut cab = format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n", r.status, razao(r.status), r.tipo, r.corpo.len());
    cab.push_str("X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\n");
    cab.push_str("Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=()\r\nCross-Origin-Opener-Policy: same-origin\r\n");
    if r.tipo.starts_with("text/html") {
        cab.push_str("Content-Security-Policy: default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self'; font-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'\r\n");
    }
    if https {
        cab.push_str("Strict-Transport-Security: max-age=31536000\r\n");
    }
    if !r.extras.iter().any(|(k, _)| k.eq_ignore_ascii_case("cache-control")) {
        cab.push_str("Cache-Control: no-store\r\n");
    }
    for (k, v) in &r.extras {
        // nada de quebra de linha vinda de dado: não há como injetar cabeçalho
        if !k.contains(['\r', '\n']) && !v.contains(['\r', '\n']) {
            cab.push_str(&format!("{k}: {v}\r\n"));
        }
    }
    cab.push_str("\r\n");
    s.write_all(cab.as_bytes())?;
    s.write_all(&r.corpo)?;
    s.flush()
}

/// Lê um pedido. `corpo_max` diz quanto corpo cada caminho aceita.
pub fn ler(s: &mut TcpStream, corpo_max: &dyn Fn(&str, &str) -> usize) -> Result<Pedido, u16> {
    s.set_read_timeout(Some(Duration::from_secs(5))).map_err(|_| 400u16)?;
    let inicio = Instant::now();
    let mut buf = Vec::new();
    let mut pedaco = [0u8; 4096];
    let fim = loop {
        if inicio.elapsed() > PRAZO_DO_PEDIDO {
            return Err(408);
        }
        let n = s.read(&mut pedaco).map_err(|_| 408u16)?;
        if n == 0 {
            return Err(400);
        }
        buf.extend_from_slice(pedaco.get(..n).unwrap_or_default());
        if let Some(p) = buf.windows(4).position(|j| j == b"\r\n\r\n") {
            break p;
        }
        if buf.len() > CABECALHO_MAX {
            return Err(431);
        }
    };
    let cabecalho = String::from_utf8_lossy(buf.get(..fim).unwrap_or_default()).into_owned();
    let mut linhas = cabecalho.split("\r\n");
    let primeira = linhas.next().unwrap_or("");
    let mut partes = primeira.split(' ');
    let (metodo, alvo, versao) = (partes.next().unwrap_or(""), partes.next().unwrap_or(""), partes.next().unwrap_or(""));
    if !versao.starts_with("HTTP/1.") || !alvo.starts_with('/') || metodo.is_empty() || metodo.len() > 10 {
        return Err(400);
    }
    let mut cabecalhos = Vec::new();
    for l in linhas {
        let Some((k, v)) = l.split_once(':') else { return Err(400) };
        cabecalhos.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        if cabecalhos.len() > 64 {
            return Err(431);
        }
    }
    let (caminho, busca) = alvo.split_once('?').unwrap_or((alvo, ""));
    let caminho = decodificar(caminho);
    if caminho.contains("..") || caminho.contains('\0') {
        return Err(400);
    }
    if cabecalhos.iter().any(|(k, v)| k == "transfer-encoding" && !v.eq_ignore_ascii_case("identity")) {
        return Err(411);
    }
    let tamanho: usize = match cabecalhos.iter().find(|(k, _)| k == "content-length") {
        Some((_, v)) => v.parse().map_err(|_| 400u16)?,
        None => 0,
    };
    if tamanho > corpo_max(metodo, &caminho) {
        return Err(413);
    }
    let mut corpo = buf.get(fim.saturating_add(4)..).unwrap_or_default().to_vec();
    if corpo.len() > tamanho {
        corpo.truncate(tamanho);
    }
    while corpo.len() < tamanho {
        if inicio.elapsed() > PRAZO_DO_PEDIDO.saturating_mul(4) {
            return Err(408);
        }
        let falta = tamanho.saturating_sub(corpo.len()).min(pedaco.len());
        let n = s.read(pedaco.get_mut(..falta).unwrap_or_default()).map_err(|_| 408u16)?;
        if n == 0 {
            return Err(400);
        }
        corpo.extend_from_slice(pedaco.get(..n).unwrap_or_default());
    }
    Ok(Pedido { metodo: metodo.to_string(), caminho, busca: busca.to_string(), cabecalhos, corpo, ip: String::new() })
}

/// O IP do cliente: o do socket, ou o do `X-Forwarded-For` contado da
/// direita quando há proxy confiável na frente.
pub fn ip_do_cliente(par: &str, xff: Option<&str>, saltos: usize) -> String {
    if saltos == 0 {
        return par.to_string();
    }
    let lista: Vec<&str> = xff.unwrap_or("").split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
    lista.len().checked_sub(saltos).and_then(|i| lista.get(i)).map_or_else(|| par.to_string(), |ip| ip.chars().filter(|c| c.is_ascii_hexdigit() || ".:".contains(*c)).take(45).collect())
}

/// Como o servidor escuta.
#[derive(Clone, Debug)]
pub struct Escuta {
    /// Endereço (`0.0.0.0:8790`).
    pub endereco: String,
    /// Saltos de proxy confiáveis na frente (0 = nenhum).
    pub saltos_de_proxy: usize,
}

/// Serve até o processo acabar.
pub fn servir(p: Arc<Plataforma>, escuta: &Escuta) -> std::io::Result<()> {
    let ouvinte = TcpListener::bind(&escuta.endereco)?;
    servir_em(p, ouvinte, escuta.saltos_de_proxy)
}

/// Serve num ouvinte já aberto (os testes abrem na porta 0).
pub fn servir_em(p: Arc<Plataforma>, ouvinte: TcpListener, saltos: usize) -> std::io::Result<()> {
    let abertas = Arc::new(AtomicUsize::new(0));
    for conexao in ouvinte.incoming() {
        let Ok(mut s) = conexao else { continue };
        let _ = s.set_write_timeout(Some(Duration::from_secs(30)));
        if abertas.fetch_add(1, Ordering::AcqRel) >= CONEXOES_MAX {
            abertas.fetch_sub(1, Ordering::AcqRel);
            let _ = escrever(&mut s, &Resposta::erro(&Erro::indisponivel("servidor cheio; tente em instantes")), p.opcoes.https);
            continue;
        }
        let (p, abertas) = (Arc::clone(&p), Arc::clone(&abertas));
        std::thread::spawn(move || {
            atender_conexao(&p, &mut s, saltos);
            abertas.fetch_sub(1, Ordering::AcqRel);
        });
    }
    Ok(())
}

fn atender_conexao(p: &Plataforma, s: &mut TcpStream, saltos: usize) {
    let inicio = Instant::now();
    let par = s.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
    let resposta = match ler(s, &crate::rotas::corpo_max) {
        Ok(mut pedido) => {
            pedido.ip = ip_do_cliente(&par, pedido.cabecalho("x-forwarded-for"), saltos);
            crate::rotas::atender(p, &pedido)
        }
        Err(status) => Resposta::erro(&Erro { status, mensagem: "pedido inválido".into() }),
    };
    let _ = escrever(s, &resposta, p.opcoes.https);
    p.anotar_pedido(resposta.status, u32::try_from(inicio.elapsed().as_millis()).unwrap_or(u32::MAX));
    let _ = s.shutdown(std::net::Shutdown::Both);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ip_atras_do_proxy() {
        assert_eq!(ip_do_cliente("10.0.0.1", Some("1.2.3.4, 5.6.7.8"), 1), "5.6.7.8");
        assert_eq!(ip_do_cliente("10.0.0.1", Some("1.2.3.4, 5.6.7.8"), 2), "1.2.3.4");
        assert_eq!(ip_do_cliente("10.0.0.1", Some("1.2.3.4"), 0), "10.0.0.1");
        assert_eq!(ip_do_cliente("10.0.0.1", None, 1), "10.0.0.1");
        assert_eq!(ip_do_cliente("10.0.0.1", Some("<script>"), 1), "c");
    }

    #[test]
    fn decodifica_busca() {
        assert_eq!(decodificar("a%20b+c%2"), "a b c%2");
        let p = Pedido { busca: "mes=2026-10&q=jo%C3%A3o".into(), cabecalhos: vec![("cookie".into(), "a=1; hyx_sessao=abc".into())], ..Pedido::default() };
        assert_eq!(p.parametro("q").as_deref(), Some("joão"));
        assert_eq!(p.cookie("hyx_sessao"), Some("abc"));
        assert_eq!(p.parametro("x"), None);
    }
}
