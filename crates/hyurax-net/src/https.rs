//! Um cliente HTTP/1.1 mínimo, com TLS, para o agente do programa falar com a
//! plataforma HYURAX Compute (`https://...`).
//!
//! O TLS é o mesmo do transporte por WebSocket (rustls sobre RustCrypto, com
//! as raízes do webpki). Aqui ele protege de verdade o conteúdo (as unidades
//! e os resultados), e por cima dele cada pedido do agente ainda vai assinado
//! pela chave do worker: quem não tem a chave não fala em nome do nó, mesmo
//! que consiga ler o tráfego.
//!
//! Só o necessário: um pedido por conexão (`Connection: close`), corpo por
//! `Content-Length` na ida, `Content-Length` ou `chunked` na volta, teto de
//! tamanho e prazo em tudo.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// A resposta.
#[derive(Clone, Debug)]
pub struct Resposta {
    /// Status HTTP.
    pub status: u16,
    /// Corpo, já sem o `chunked`.
    pub corpo: Vec<u8>,
}

/// O endereço já separado: `(https?, host, porta, caminho)`.
pub fn ler_url(texto: &str) -> Option<(bool, String, u16, String)> {
    let (seguro, resto) = if let Some(r) = texto.strip_prefix("https://") {
        (true, r)
    } else {
        (false, texto.strip_prefix("http://")?)
    };
    let (autoridade, caminho) = match resto.find('/') {
        Some(i) => (resto.get(..i)?, resto.get(i..)?),
        None => (resto, "/"),
    };
    let (host, porta) = match autoridade.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(']') || h.starts_with('[') => (h, p.parse().ok()?),
        _ => (autoridade, if seguro { 443 } else { 80 }),
    };
    let valido = !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'))
        && caminho.chars().all(|c| c.is_ascii_graphic());
    valido.then(|| (seguro, host.to_string(), porta, caminho.to_string()))
}

enum Cano {
    Simples(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Read for Cano {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Cano::Simples(t) => t.read(buf),
            Cano::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Cano {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Cano::Simples(t) => t.write(buf),
            Cano::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Cano::Simples(t) => t.flush(),
            Cano::Tls(s) => s.flush(),
        }
    }
}

fn erro(m: impl Into<String>) -> std::io::Error {
    std::io::Error::new(ErrorKind::InvalidData, m.into())
}

/// Faz um pedido. `base` é o endereço da plataforma (`https://compute.exemplo`),
/// `caminho` começa por `/`. A resposta passa de `max_resposta` bytes = erro.
pub fn pedir(metodo: &str, base: &str, caminho: &str, cabecalhos: &[(&str, String)], corpo: &[u8], prazo: Duration, max_resposta: usize) -> std::io::Result<Resposta> {
    let (seguro, host, porta, prefixo) = ler_url(base.trim_end_matches('/')).ok_or_else(|| erro("endereço da plataforma inválido (use https://...)"))?;
    let caminho_inteiro = format!("{}{caminho}", prefixo.trim_end_matches('/'));
    let alvo = (host.trim_matches(|c| c == '[' || c == ']'), porta).to_socket_addrs()?.next().ok_or_else(|| erro("o nome da plataforma não resolveu"))?;
    let inicio = Instant::now();
    let tcp = TcpStream::connect_timeout(&alvo, prazo.min(Duration::from_secs(20)))?;
    tcp.set_read_timeout(Some(prazo))?;
    tcp.set_write_timeout(Some(prazo))?;
    let mut cano = if seguro {
        let cfg = crate::servidor::websocket::configuracao_tls().ok_or_else(|| erro("TLS indisponível"))?;
        let nome = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| erro("nome inválido para TLS"))?;
        let con = rustls::ClientConnection::new(cfg, nome).map_err(|e| erro(format!("TLS: {e}")))?;
        Cano::Tls(Box::new(rustls::StreamOwned::new(con, tcp)))
    } else {
        Cano::Simples(tcp)
    };
    let hospedeiro = if (seguro && porta == 443) || (!seguro && porta == 80) { host.clone() } else { format!("{host}:{porta}") };
    let mut cab = format!(
        "{metodo} {caminho_inteiro} HTTP/1.1\r\nHost: {hospedeiro}\r\nContent-Length: {}\r\nConnection: close\r\nUser-Agent: hyurax-agente/{}\r\n",
        corpo.len(),
        env!("CARGO_PKG_VERSION")
    );
    for (k, v) in cabecalhos {
        if k.contains(['\r', '\n']) || v.contains(['\r', '\n']) {
            return Err(erro("cabeçalho com quebra de linha"));
        }
        cab.push_str(&format!("{k}: {v}\r\n"));
    }
    cab.push_str("\r\n");
    cano.write_all(cab.as_bytes())?;
    cano.write_all(corpo)?;
    cano.flush()?;

    let mut tudo = Vec::new();
    let mut pedaco = [0u8; 8192];
    loop {
        if inicio.elapsed() > prazo.saturating_mul(2) {
            return Err(std::io::Error::new(ErrorKind::TimedOut, "a plataforma demorou demais para responder"));
        }
        match cano.read(&mut pedaco) {
            Ok(0) => break,
            Ok(n) => {
                tudo.extend_from_slice(pedaco.get(..n).unwrap_or_default());
                if tudo.len() > max_resposta.saturating_add(16 * 1024) {
                    return Err(erro("resposta grande demais"));
                }
            }
            // o servidor fechou sem o aviso de fim do TLS: vale o que chegou
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(e) if matches!(e.kind(), ErrorKind::Interrupted) => {}
            // fechou com "reset" (no Windows, quando sobra byte não lido do
            // lado do servidor): se a resposta já chegou inteira, vale
            Err(e) if matches!(e.kind(), ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted) && completa(&tudo) => break,
            Err(e) => return Err(e),
        }
    }
    let fim = tudo.windows(4).position(|j| j == b"\r\n\r\n").ok_or_else(|| erro("resposta sem cabeçalho"))?;
    let cabecalho = String::from_utf8_lossy(tudo.get(..fim).unwrap_or_default()).into_owned();
    let status: u16 = cabecalho.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|s| s.parse().ok()).ok_or_else(|| erro("status inválido"))?;
    let campo = |nome: &str| {
        cabecalho.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case(nome).then(|| v.trim().to_string())
        })
    };
    let resto = tudo.get(fim.saturating_add(4)..).unwrap_or_default();
    let corpo = if campo("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        sem_chunked(resto)?
    } else if let Some(n) = campo("content-length").and_then(|v| v.parse::<usize>().ok()) {
        resto.get(..n).ok_or_else(|| erro("resposta cortada"))?.to_vec()
    } else {
        resto.to_vec()
    };
    if corpo.len() > max_resposta {
        return Err(erro("resposta grande demais"));
    }
    Ok(Resposta { status, corpo })
}

/// A resposta já chegou inteira (cabeçalho e o corpo do `Content-Length`)?
fn completa(tudo: &[u8]) -> bool {
    let Some(fim) = tudo.windows(4).position(|j| j == b"\r\n\r\n") else { return false };
    let cabecalho = String::from_utf8_lossy(tudo.get(..fim).unwrap_or_default()).to_ascii_lowercase();
    let tamanho = cabecalho.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()));
    tamanho.is_some_and(|n| tudo.len() >= fim.saturating_add(4).saturating_add(n))
}

/// Junta os pedaços de um corpo `chunked`.
fn sem_chunked(mut dados: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut saida = Vec::new();
    loop {
        let fim = dados.windows(2).position(|j| j == b"\r\n").ok_or_else(|| erro("chunked sem tamanho"))?;
        let linha = String::from_utf8_lossy(dados.get(..fim).unwrap_or_default()).into_owned();
        let tamanho = usize::from_str_radix(linha.split(';').next().unwrap_or("").trim(), 16).map_err(|_| erro("chunked com tamanho inválido"))?;
        dados = dados.get(fim.saturating_add(2)..).unwrap_or_default();
        if tamanho == 0 {
            return Ok(saida);
        }
        saida.extend_from_slice(dados.get(..tamanho).ok_or_else(|| erro("chunked cortado"))?);
        dados = dados.get(tamanho.saturating_add(2)..).unwrap_or_default();
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn urls() {
        assert_eq!(ler_url("https://compute.exemplo.com"), Some((true, "compute.exemplo.com".into(), 443, "/".into())));
        assert_eq!(ler_url("http://127.0.0.1:8790/base"), Some((false, "127.0.0.1".into(), 8790, "/base".into())));
        assert_eq!(ler_url("ftp://x"), None);
        assert_eq!(ler_url("https://ex ample"), None);
    }

    #[test]
    fn chunked() {
        assert_eq!(sem_chunked(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n").unwrap(), b"Wikipedia");
        assert!(sem_chunked(b"zz\r\n").is_err());
        assert!(completa(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok"));
        assert!(!completa(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nok"));
    }

    #[test]
    fn pedido_de_verdade_num_servidor_local() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let end = l.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            // lê o pedido inteiro (cabeçalho e as 5 letras do corpo)
            let mut tudo = Vec::new();
            let mut buf = [0u8; 1024];
            while !tudo.ends_with(b"corpo") {
                let n = s.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                tudo.extend_from_slice(&buf[..n]);
            }
            s.write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok").unwrap();
        });
        let r = pedir("POST", &format!("http://{end}"), "/api/v1/x", &[("X-A", "1".into())], b"corpo", Duration::from_secs(5), 1024).unwrap();
        assert_eq!((r.status, r.corpo.as_slice()), (201, b"ok".as_slice()));
    }
}
