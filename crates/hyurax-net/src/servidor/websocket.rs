//! Transporte por WebSocket: o nó atravessa lugares que só deixam passar
//! HTTPS (o plano grátis do Render, por exemplo) sem mudar nada do protocolo.
//!
//! O WebSocket é só o cano. Por dentro dele passam os mesmos bytes da conexão
//! TCP de sempre: o aperto de mão Noise XX (que prova a identidade do par e
//! cifra tudo), as mensagens e a validação de cada bloco. O TLS do `wss://`
//! existe porque o Render só aceita HTTPS; a segurança entre os nós continua
//! sendo a do Noise, então uma falha no TLS não deixa ninguém ler nem alterar
//! o tráfego entre nós.
//!
//! Como entra no resto do código sem mexer nele: cada conexão WebSocket ganha
//! um par de sockets locais (127.0.0.1) e duas linhas que copiam bytes de um
//! lado para o outro. O código de rede recebe um `TcpStream` comum, marcado
//! como "via ponte" (o IP de loopback não é o do par: não vai para o livro de
//! endereços nem para banimento por IP; a identidade Noise continua valendo).
//!
//! - **Discar:** a semente `wss://nome.onrender.com/p2p` vira TLS + WebSocket.
//! - **Escutar:** `escutar_ws(porta)` atende `GET /p2p` com o upgrade e
//!   `GET /` ou `/saude` com um texto curto (o Render confere a saúde por aí).
//!
//! RFC 6455, só o necessário: quadros binários, ping, pong e fechamento.

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::Rede;
use crate::cifra::Papel;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// Maior quadro aceito. As mensagens da rede já têm teto próprio bem menor.
const MAX_QUADRO: usize = 4 * 1024 * 1024;
/// Cabeçalho HTTP do aperto de mão, no máximo.
const MAX_CABECALHO: usize = 8 * 1024;
/// Discar: TCP e TLS.
const PRAZO_CONEXAO: Duration = Duration::from_secs(15);
/// A resposta ao upgrade. Folgado: no plano grátis do Render, um serviço que
/// dormiu leva perto de um minuto para acordar.
const PRAZO_UPGRADE: Duration = Duration::from_secs(90);
/// Leitura curta no cano: a linha que lê solta a trava para a que escreve.
const FATIA: Duration = Duration::from_millis(50);
/// Ping para o proxy no meio do caminho não fechar a conexão parada.
const PING_A_CADA: Duration = Duration::from_secs(25);

// ---------------------------------------------------------------------------
// SHA-1 e base64: só para o `Sec-WebSocket-Accept` (exigência do protocolo,
// não segurança; a segurança é o Noise).

fn sha1(dados: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let bits = u64::try_from(dados.len()).unwrap_or(0).wrapping_mul(8);
    let mut msg = dados.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for bloco in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (destino, palavra) in w.iter_mut().zip(bloco.as_chunks::<4>().0) {
            *destino = u32::from_be_bytes(*palavra);
        }
        for i in 16usize..80 {
            let v = [3usize, 8, 14, 16].iter().fold(0u32, |a, &k| a ^ w.get(i.wrapping_sub(k)).copied().unwrap_or(0));
            if let Some(destino) = w.get_mut(i) {
                *destino = v.rotate_left(1);
            }
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6u32),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut saida = [0u8; 20];
    for (pedaco, x) in saida.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        pedaco.copy_from_slice(&x.to_be_bytes());
    }
    saida
}

fn base64(dados: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for trio in dados.chunks(3) {
        let (b0, b1, b2) = (trio.first().copied().unwrap_or(0), trio.get(1).copied().unwrap_or(0), trio.get(2).copied().unwrap_or(0));
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        for (i, desloca) in [18u32, 12, 6, 0].iter().enumerate() {
            if i <= trio.len() {
                s.push(char::from(A.get(usize::try_from((n >> desloca) & 63).unwrap_or(0)).copied().unwrap_or(b'A')));
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn aceite(chave: &str) -> String {
    base64(&sha1(format!("{chave}{GUID}").as_bytes()))
}

// ---------------------------------------------------------------------------
// Quadros

const OP_CONTINUACAO: u8 = 0;
const OP_BINARIO: u8 = 2;
const OP_FECHAR: u8 = 8;
const OP_PING: u8 = 9;
const OP_PONG: u8 = 10;

fn quadro(opcode: u8, dados: &[u8], mascara: Option<[u8; 4]>) -> Vec<u8> {
    let mut q = vec![0x80 | (opcode & 0x0F)];
    let bit = if mascara.is_some() { 0x80 } else { 0 };
    match dados.len() {
        n if n < 126 => q.push(bit | u8::try_from(n).unwrap_or(0)),
        n if n <= 0xFFFF => {
            q.push(bit | 126);
            q.extend_from_slice(&u16::try_from(n).unwrap_or(0).to_be_bytes());
        }
        n => {
            q.push(bit | 127);
            q.extend_from_slice(&u64::try_from(n).unwrap_or(0).to_be_bytes());
        }
    }
    match mascara {
        Some(m) => {
            q.extend_from_slice(&m);
            q.extend(dados.iter().zip(m.iter().cycle()).map(|(d, k)| d ^ k));
        }
        None => q.extend_from_slice(dados),
    }
    q
}

/// Tira um quadro completo do começo de `buf`, se já chegou inteiro.
/// `Err` para quadro grande demais ou fragmentado de um jeito que não usamos.
fn tirar_quadro(buf: &mut Vec<u8>) -> Result<Option<(u8, Vec<u8>)>, &'static str> {
    let (Some(&b0), Some(&b1)) = (buf.first(), buf.get(1)) else { return Ok(None) };
    let opcode = b0 & 0x0F;
    let mascarado = b1 & 0x80 != 0;
    let (tamanho, mut pos) = match b1 & 0x7F {
        126 => match buf.get(2..4) {
            Some(b) => (usize::from(u16::from_be_bytes(<[u8; 2]>::try_from(b).unwrap_or([0, 0]))), 4usize),
            None => return Ok(None),
        },
        127 => match buf.get(2..10).and_then(|b| <[u8; 8]>::try_from(b).ok()) {
            Some(b) => (usize::try_from(u64::from_be_bytes(b)).unwrap_or(usize::MAX), 10usize),
            None => return Ok(None),
        },
        n => (usize::from(n), 2usize),
    };
    if tamanho > MAX_QUADRO {
        return Err("quadro grande demais");
    }
    let mascara = if mascarado {
        let Some(m) = buf.get(pos..pos.saturating_add(4)).and_then(|b| <[u8; 4]>::try_from(b).ok()) else { return Ok(None) };
        pos = pos.saturating_add(4);
        Some(m)
    } else {
        None
    };
    let fim = pos.saturating_add(tamanho);
    let Some(corpo) = buf.get(pos..fim) else { return Ok(None) };
    let dados: Vec<u8> = match mascara {
        Some(m) => corpo.iter().zip(m.iter().cycle()).map(|(d, k)| d ^ k).collect(),
        None => corpo.to_vec(),
    };
    buf.drain(..fim);
    Ok(Some((opcode, dados)))
}

// ---------------------------------------------------------------------------
// O cano: TCP simples (atrás do proxy do Render) ou TLS (discando `wss://`)

enum Cano {
    Simples(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Cano {
    fn tcp(&self) -> &TcpStream {
        match self {
            Cano::Simples(t) => t,
            Cano::Tls(s) => &s.sock,
        }
    }
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

fn espera(e: &std::io::Error) -> bool {
    matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted)
}

/// Dois sockets locais ligados um no outro.
fn par_local() -> std::io::Result<(TcpStream, TcpStream)> {
    let l = TcpListener::bind("127.0.0.1:0")?;
    let a = TcpStream::connect(l.local_addr()?)?;
    let (b, _) = l.accept()?;
    Ok((a, b))
}

fn mascara_nova() -> Option<[u8; 4]> {
    let mut m = [0u8; 4];
    crate::entropia::preencher(&mut m).ok()?;
    Some(m)
}

/// Copia nos dois sentidos entre o WebSocket e o socket local, até um lado
/// fechar. `mascarar`: quem discou mascara o que manda (exigência da RFC).
fn bombear(cano: Cano, local: TcpStream, mascarar: bool, sobra: Vec<u8>) {
    let _ = cano.tcp().set_read_timeout(Some(FATIA));
    let _ = cano.tcp().set_nodelay(true);
    let _ = local.set_nodelay(true);
    let Ok(local_leitura) = local.try_clone() else { return };
    let cano = Arc::new(Mutex::new(cano));
    let fim = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mascara = move || if mascarar { mascara_nova() } else { None };

    // local → WebSocket
    {
        let cano = Arc::clone(&cano);
        let fim = Arc::clone(&fim);
        let mut local_leitura = local_leitura;
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                match local_leitura.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let q = quadro(OP_BINARIO, buf.get(..n).unwrap_or_default(), mascara());
                        let Ok(mut c) = cano.lock() else { break };
                        if c.write_all(&q).and_then(|()| c.flush()).is_err() {
                            break;
                        }
                    }
                }
            }
            fim.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Ok(mut c) = cano.lock() {
                let _ = c.write_all(&quadro(OP_FECHAR, &[], mascara()));
                let _ = c.tcp().shutdown(Shutdown::Both);
            }
        });
    }

    // WebSocket → local
    let mut local_escrita = local;
    let mut buf = sobra;
    let mut pedaco = vec![0u8; 16 * 1024];
    let mut ultimo_ping = Instant::now();
    'laco: while !fim.load(std::sync::atomic::Ordering::Relaxed) {
        let lido = {
            let Ok(mut c) = cano.lock() else { break };
            if ultimo_ping.elapsed() >= PING_A_CADA {
                ultimo_ping = Instant::now();
                if c.write_all(&quadro(OP_PING, b"hyx", mascara())).and_then(|()| c.flush()).is_err() {
                    break;
                }
            }
            c.read(&mut pedaco)
        };
        match lido {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(pedaco.get(..n).unwrap_or_default()),
            Err(e) if espera(&e) => {
                // deixa a outra linha escrever antes de tentar ler de novo
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(_) => break,
        }
        loop {
            match tirar_quadro(&mut buf) {
                Ok(None) => break,
                Err(_) => break 'laco,
                Ok(Some((op, dados))) => match op {
                    OP_BINARIO | OP_CONTINUACAO => {
                        if local_escrita.write_all(&dados).is_err() {
                            break 'laco;
                        }
                    }
                    OP_PING => {
                        let Ok(mut c) = cano.lock() else { break 'laco };
                        let _ = c.write_all(&quadro(OP_PONG, &dados, mascara()));
                    }
                    OP_FECHAR => break 'laco,
                    _ => {} // pong e texto: ignorados
                },
            }
        }
    }
    fim.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = local_escrita.shutdown(Shutdown::Both);
    if let Ok(c) = cano.lock() {
        let _ = c.tcp().shutdown(Shutdown::Both);
    }
}

/// Lê o cabeçalho HTTP até a linha vazia. Devolve o cabeçalho e o que veio
/// depois dele (já é WebSocket).
fn ler_cabecalho(c: &mut Cano, prazo: Duration) -> std::io::Result<(String, Vec<u8>)> {
    let limite = Instant::now().checked_add(prazo);
    let _ = c.tcp().set_read_timeout(Some(Duration::from_millis(500)));
    let mut buf = Vec::new();
    let mut pedaco = [0u8; 1024];
    loop {
        if let Some(p) = buf.windows(4).position(|j| j == b"\r\n\r\n") {
            let resto = buf.split_off(p.saturating_add(4));
            return Ok((String::from_utf8_lossy(&buf).into_owned(), resto));
        }
        if buf.len() > MAX_CABECALHO {
            return Err(std::io::Error::new(ErrorKind::InvalidData, "cabeçalho HTTP grande demais"));
        }
        if limite.is_some_and(|l| Instant::now() > l) {
            return Err(std::io::Error::new(ErrorKind::TimedOut, "sem resposta ao upgrade"));
        }
        match c.read(&mut pedaco) {
            Ok(0) => return Err(std::io::Error::new(ErrorKind::UnexpectedEof, "fechou no meio do cabeçalho")),
            Ok(n) => buf.extend_from_slice(pedaco.get(..n).unwrap_or_default()),
            Err(e) if espera(&e) => {}
            Err(e) => return Err(e),
        }
    }
}

fn campo<'a>(cabecalho: &'a str, nome: &str) -> Option<&'a str> {
    cabecalho.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(nome).then(|| v.trim())
    })
}

/// `ws://host[:porta]/caminho` ou `wss://…`.
pub(crate) fn ler_url(texto: &str) -> Option<(bool, String, u16, String)> {
    let (seguro, resto) = if let Some(r) = texto.strip_prefix("wss://") {
        (true, r)
    } else {
        (false, texto.strip_prefix("ws://")?)
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

fn configuracao_tls() -> Option<Arc<rustls::ClientConfig>> {
    static CFG: std::sync::OnceLock<Option<Arc<rustls::ClientConfig>>> = std::sync::OnceLock::new();
    CFG.get_or_init(|| {
        let raizes = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        rustls::ClientConfig::builder_with_provider(Arc::new(rustls_rustcrypto::provider()))
            .with_safe_default_protocol_versions()
            .ok()
            .map(|b| Arc::new(b.with_root_certificates(raizes).with_no_client_auth()))
    })
    .clone()
}

/// Disca `ws://` ou `wss://`, faz o upgrade e devolve a ponta local do par:
/// para o resto do código é um `TcpStream` como outro qualquer.
pub(crate) fn discar(url: &str) -> std::io::Result<TcpStream> {
    let erro = |m: &str| std::io::Error::new(ErrorKind::InvalidInput, m.to_string());
    let (seguro, host, porta, caminho) = ler_url(url).ok_or_else(|| erro("endereço WebSocket inválido"))?;
    let alvo = (host.trim_matches(|c| c == '[' || c == ']'), porta)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| erro("o nome não resolveu"))?;
    let tcp = TcpStream::connect_timeout(&alvo, PRAZO_CONEXAO)?;
    tcp.set_write_timeout(Some(PRAZO_CONEXAO))?;
    let mut cano = if seguro {
        let cfg = configuracao_tls().ok_or_else(|| erro("TLS indisponível"))?;
        let nome = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| erro("nome inválido para TLS"))?;
        let con = rustls::ClientConnection::new(cfg, nome).map_err(|e| erro(&format!("TLS: {e}")))?;
        Cano::Tls(Box::new(rustls::StreamOwned::new(con, tcp)))
    } else {
        Cano::Simples(tcp)
    };
    let mut chave = [0u8; 16];
    crate::entropia::preencher(&mut chave).map_err(|e| erro(&e))?;
    let chave = base64(&chave);
    let hospedeiro = if (seguro && porta == 443) || (!seguro && porta == 80) { host.clone() } else { format!("{host}:{porta}") };
    let pedido = format!(
        "GET {caminho} HTTP/1.1\r\nHost: {hospedeiro}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: {chave}\r\nSec-WebSocket-Version: 13\r\nUser-Agent: hyurax-no\r\n\r\n"
    );
    cano.write_all(pedido.as_bytes())?;
    cano.flush()?;
    let (cabecalho, sobra) = ler_cabecalho(&mut cano, PRAZO_UPGRADE)?;
    let status_ok = cabecalho.lines().next().is_some_and(|l| l.split_whitespace().nth(1) == Some("101"));
    if !status_ok || campo(&cabecalho, "Sec-WebSocket-Accept") != Some(aceite(&chave).as_str()) {
        return Err(std::io::Error::new(
            ErrorKind::ConnectionRefused,
            format!("o servidor não aceitou o WebSocket: {}", cabecalho.lines().next().unwrap_or("")),
        ));
    }
    let (minha, outra) = par_local()?;
    std::thread::spawn(move || bombear(cano, outra, true, sobra));
    Ok(minha)
}

impl Rede {
    /// Disca uma semente `ws(s)://` numa linha própria (o upgrade pode levar
    /// quase um minuto quando o servidor está acordando) e serve a conexão.
    pub(super) fn discar_ws(self: &Arc<Self>, url: String) {
        self.garantir_manutencao();
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            let ok = match discar(&url) {
                Ok(stream) => {
                    let r = Arc::clone(&rede);
                    let rotulo = url.clone();
                    std::thread::spawn(move || {
                        let _ = r.servir(stream, Papel::Discou, Some(rotulo), true, None);
                    });
                    true
                }
                Err(_) => false,
            };
            rede.anotar_discagem(&url, ok);
        });
    }

    /// Escuta WebSocket em `porta` (todas as interfaces): `GET /p2p` com
    /// upgrade vira uma conexão de nó; `GET /` e `/saude` respondem um texto
    /// curto, que serve de verificação de saúde. Devolve a porta real.
    ///
    /// # Errors
    /// Porta ocupada.
    pub fn escutar_ws(self: &Arc<Self>, porta: u16) -> std::io::Result<u16> {
        let ouvinte = TcpListener::bind(("0.0.0.0", porta))?;
        let porta = ouvinte.local_addr()?.port();
        self.garantir_manutencao();
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            for conexao in ouvinte.incoming() {
                if rede.parando() {
                    break;
                }
                let Ok(s) = conexao else { continue };
                let r = Arc::clone(&rede);
                std::thread::spawn(move || r.atender_ws(s));
            }
        });
        Ok(porta)
    }

    fn atender_ws(self: &Arc<Self>, tcp: TcpStream) {
        let _ = tcp.set_write_timeout(Some(PRAZO_CONEXAO));
        let mut cano = Cano::Simples(tcp);
        let Ok((cabecalho, sobra)) = ler_cabecalho(&mut cano, Duration::from_secs(10)) else { return };
        let mut linha = cabecalho.lines().next().unwrap_or("").split_whitespace();
        let (metodo, caminho) = (linha.next().unwrap_or(""), linha.next().unwrap_or(""));
        let rota = caminho.split('?').next().unwrap_or("");
        let upgrade = campo(&cabecalho, "Upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
        if metodo == "GET" && rota == "/p2p" && upgrade {
            let Some(chave) = campo(&cabecalho, "Sec-WebSocket-Key").map(str::to_string) else { return };
            // a vaga (teto de entradas) antes de responder
            let Ok((minha, outra)) = par_local() else { return };
            let Some(vaga) = self.vaga_de_entrada(&minha) else {
                let _ = cano.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                return;
            };
            let resposta = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                aceite(&chave)
            );
            if cano.write_all(resposta.as_bytes()).and_then(|()| cano.flush()).is_err() {
                return;
            }
            let r = Arc::clone(self);
            std::thread::spawn(move || {
                let _vaga = vaga;
                let _ = r.servir(minha, Papel::Recebeu, None, true, None);
            });
            bombear(cano, outra, false, sobra);
            return;
        }
        let (status, corpo) = if metodo == "GET" && (rota == "/" || rota == "/saude") {
            let altura = self.no.lock().map(|n| n.chain.height()).unwrap_or(0);
            (
                "200 OK",
                format!(
                    "Hyurax: nó semente da rede de TESTE (o HYX não tem valor).\naltura={altura}\npares={}\nConecte o programa em wss://ESTE-ENDERECO/p2p\n",
                    self.pares_conectados()
                ),
            )
        } else {
            ("404 Not Found", "nada aqui\n".to_string())
        };
        let resposta = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{corpo}",
            corpo.len()
        );
        let _ = cano.write_all(resposta.as_bytes());
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn sha1_e_base64_conhecidos() {
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(&[b'a'; 1000])), "291e9a6c66994949b57ba5e650361e98fc36b1ba");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // o exemplo da própria RFC 6455
        assert_eq!(aceite("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn quadros_ida_e_volta() {
        for tamanho in [0usize, 5, 125, 126, 1000, 70_000] {
            let dados: Vec<u8> = (0..tamanho).map(|i| (i % 251) as u8).collect();
            for mascara in [None, Some([1u8, 2, 3, 4])] {
                let mut buf = quadro(OP_BINARIO, &dados, mascara);
                buf.extend_from_slice(&[0x82]); // começo de outro quadro
                assert_eq!(tirar_quadro(&mut buf).unwrap(), Some((OP_BINARIO, dados.clone())));
                assert_eq!(buf, vec![0x82]);
                assert_eq!(tirar_quadro(&mut buf).unwrap(), None, "incompleto espera");
            }
        }
        let mut gigante = vec![0x82, 127];
        gigante.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(tirar_quadro(&mut gigante).is_err());
    }

    #[test]
    fn urls() {
        assert_eq!(ler_url("wss://semente.onrender.com/p2p"), Some((true, "semente.onrender.com".into(), 443, "/p2p".into())));
        assert_eq!(ler_url("ws://127.0.0.1:9000/p2p"), Some((false, "127.0.0.1".into(), 9000, "/p2p".into())));
        assert_eq!(ler_url("ws://exemplo.org"), Some((false, "exemplo.org".into(), 80, "/".into())));
        assert_eq!(ler_url("http://exemplo.org/p2p"), None);
        assert_eq!(ler_url("wss:///p2p"), None);
        assert_eq!(ler_url("wss://a b/p2p"), None);
    }
}
