//! HTTP/1.1 mínimo para o servidor local: ler o pedido com limites, decodificar
//! formulário e responder com os cabeçalhos de segurança.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Teto do cabeçalho.
const CABECALHO_MAX: usize = 16 * 1024;
/// Teto do corpo de formulário.
pub const CORPO_MAX: usize = 8 * 1024;
/// Prazo para o pedido inteiro chegar: quem goteja um byte a cada poucos
/// segundos não segura a linha para sempre.
const PRAZO_DO_PEDIDO: Duration = Duration::from_secs(10);
/// Prazo de cada escrita da resposta.
const PRAZO_DE_ESCRITA: Duration = Duration::from_secs(10);

/// Um pedido.
pub struct Pedido {
    /// `GET` ou `POST`.
    pub metodo: String,
    /// Caminho com a busca (`/a?b=1`).
    pub caminho: String,
    /// `Host`, em minúsculas.
    pub host: String,
    /// `Origin`, em minúsculas, quando veio.
    pub origem: Option<String>,
    /// A chave de sessão do painel (`X-Hyurax-Chave`), quando veio.
    pub chave: Option<String>,
    /// O corpo, em bytes.
    pub corpo: Vec<u8>,
}

impl Pedido {
    /// O caminho sem a busca.
    pub fn rota(&self) -> &str {
        self.caminho.split('?').next().unwrap_or("")
    }

    /// Um parâmetro da busca (`?a=1&b=2`), sem decodificar: só números e
    /// hexadecimal passam por aqui.
    pub fn parametro(&self, nome: &str) -> Option<&str> {
        self.caminho.split_once('?')?.1.split('&').find_map(|par| par.split_once('=').filter(|(k, _)| *k == nome).map(|(_, v)| v))
    }

    /// A chave de sessão: o cabeçalho, ou `?chave=` (o fluxo de eventos e os
    /// links de relatório não conseguem mandar cabeçalho).
    pub fn chave_enviada(&self) -> Option<&str> {
        self.chave.as_deref().or_else(|| self.parametro("chave"))
    }

    /// Os campos do formulário do corpo.
    pub fn campos(&self) -> Vec<(String, String)> {
        campos_do_formulario(&String::from_utf8_lossy(&self.corpo))
    }
}

/// Lê um pedido. `corpo_max` diz quanto corpo este caminho aceita.
pub fn ler(s: &mut TcpStream, corpo_max: &dyn Fn(&str) -> usize) -> Option<Pedido> {
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let inicio = Instant::now();
    let mut buf = Vec::new();
    let mut pedaco = [0u8; 2048];
    let fim_cab = loop {
        if inicio.elapsed() > PRAZO_DO_PEDIDO {
            return None;
        }
        let n = s.read(&mut pedaco).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(pedaco.get(..n)?);
        if let Some(p) = buf.windows(4).position(|j| j == b"\r\n\r\n") {
            break p;
        }
        if buf.len() > CABECALHO_MAX {
            return None;
        }
    };
    let cabecalho = String::from_utf8_lossy(buf.get(..fim_cab)?).into_owned();
    let mut linhas = cabecalho.split("\r\n");
    let mut primeira = linhas.next()?.split(' ');
    let metodo = primeira.next()?.to_string();
    let caminho = primeira.next()?.to_string();
    let (mut host, mut origem, mut chave, mut tamanho) = (String::new(), None, None, 0usize);
    let maximo = corpo_max(caminho.split('?').next().unwrap_or(""));
    for l in linhas {
        let Some((nome, valor)) = l.split_once(':') else { continue };
        let valor = valor.trim();
        match nome.trim().to_ascii_lowercase().as_str() {
            "host" => host = valor.to_ascii_lowercase(),
            "origin" => origem = Some(valor.to_ascii_lowercase()),
            "x-hyurax-chave" => chave = Some(valor.to_string()),
            "content-length" => tamanho = valor.parse().ok().filter(|t| *t <= maximo)?,
            _ => {}
        }
    }
    let mut corpo = buf.get(fim_cab.saturating_add(4)..)?.to_vec();
    while corpo.len() < tamanho {
        if inicio.elapsed() > PRAZO_DO_PEDIDO {
            return None;
        }
        let n = s.read(&mut pedaco).ok()?;
        if n == 0 {
            break;
        }
        corpo.extend_from_slice(pedaco.get(..n)?);
    }
    corpo.truncate(tamanho);
    Some(Pedido { metodo, caminho, host, origem, chave, corpo })
}

/// Compara duas chaves sem vazar, pelo tempo, quantos caracteres bateram.
pub fn mesma_chave(a: &str, b: &str) -> bool {
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Decodifica `application/x-www-form-urlencoded`: `+` é espaço, `%XX` é byte.
pub fn decodificar(texto: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(texto.len());
    let mut it = texto.bytes();
    while let Some(b) = it.next() {
        match b {
            b'+' => bytes.push(b' '),
            b'%' => {
                let alto = char::from(it.next()?).to_digit(16)?;
                let baixo = char::from(it.next()?).to_digit(16)?;
                bytes.push(u8::try_from(alto.checked_mul(16)?.checked_add(baixo)?).ok()?);
            }
            outro => bytes.push(outro),
        }
    }
    String::from_utf8(bytes).ok()
}

/// Os pares `nome=valor` de um formulário.
pub fn campos_do_formulario(corpo: &str) -> Vec<(String, String)> {
    corpo
        .split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|par| {
            let (k, v) = par.split_once('=').unwrap_or((par, ""));
            Some((decodificar(k)?, decodificar(v)?))
        })
        .collect()
}

/// O valor de um campo.
pub fn campo(campos: &[(String, String)], nome: &str) -> Option<String> {
    campos.iter().find(|(k, _)| k == nome).map(|(_, v)| v.clone())
}

/// A política de segurança de conteúdo da interface: só o que vem deste
/// próprio servidor.
const CSP: &str = "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; worker-src 'self' blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

/// Responde e fecha.
///
/// # Errors
/// Conexão caiu.
pub fn responder(s: &mut TcpStream, status: &str, tipo: &str, corpo: &[u8]) -> std::io::Result<()> {
    responder_com(s, status, tipo, &[], corpo)
}

/// Responde e fecha, com cabeçalhos a mais (nome, valor). Os valores vêm do
/// próprio programa (hex), nunca do pedido.
///
/// # Errors
/// Conexão caiu.
pub fn responder_com(s: &mut TcpStream, status: &str, tipo: &str, extras: &[(&str, String)], corpo: &[u8]) -> std::io::Result<()> {
    let extras: String = extras.iter().map(|(nome, valor)| format!("{nome}: {valor}\r\n")).collect();
    let cab = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\n\
         Content-Security-Policy: {CSP}\r\n{extras}Connection: close\r\n\r\n",
        corpo.len()
    );
    // uma escrita só, e sem Nagle: em duas, o Windows segurava o corpo
    // esperando o ACK atrasado do cabeçalho (200 a 400 ms por resposta)
    let _ = s.set_nodelay(true);
    let _ = s.set_write_timeout(Some(PRAZO_DE_ESCRITA));
    let mut tudo = Vec::with_capacity(cab.len().saturating_add(corpo.len()));
    tudo.extend_from_slice(cab.as_bytes());
    tudo.extend_from_slice(corpo);
    s.write_all(&tudo)?;
    s.flush()
}

/// Responde JSON: `Ok` com 200, `Err` com 400 e `{"erro": "…"}`.
///
/// # Errors
/// Conexão caiu.
pub fn responder_json(s: &mut TcpStream, r: Result<serde_json::Value, String>) -> std::io::Result<()> {
    match r {
        Ok(v) => responder(s, "200 OK", "application/json; charset=utf-8", v.to_string().as_bytes()),
        Err(erro) => responder(
            s,
            "400 Bad Request",
            "application/json; charset=utf-8",
            serde_json::json!({ "erro": erro }).to_string().as_bytes(),
        ),
    }
}

/// Começa uma resposta de fluxo de eventos (Server-Sent Events).
///
/// # Errors
/// Conexão caiu.
pub fn comecar_fluxo(s: &mut TcpStream) -> std::io::Result<()> {
    let _ = s.set_nodelay(true);
    let _ = s.set_write_timeout(Some(PRAZO_DE_ESCRITA));
    s.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\n\
          X-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\nretry: 2000\n\n",
    )?;
    s.flush()
}

/// Um evento do fluxo.
///
/// # Errors
/// Conexão caiu (a tela fechou).
pub fn evento(s: &mut TcpStream, id: u64, tipo: &str, dados: &str) -> std::io::Result<()> {
    let mut t = String::with_capacity(dados.len().saturating_add(48));
    t.push_str("id: ");
    t.push_str(&id.to_string());
    t.push_str("\nevent: ");
    t.push_str(tipo);
    for linha in dados.lines() {
        t.push_str("\ndata: ");
        t.push_str(linha);
    }
    t.push_str("\n\n");
    s.write_all(t.as_bytes())?;
    s.flush()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn formulario_decodifica_espaco_e_acento() {
        let c = campos_do_formulario("senha=minha+senha%20longa%C3%A7&x=1&vazio=");
        assert_eq!(campo(&c, "senha").as_deref(), Some("minha senha longaç"));
        assert_eq!(campo(&c, "x").as_deref(), Some("1"));
        assert_eq!(campo(&c, "vazio").as_deref(), Some(""));
        // %XX quebrado ou UTF-8 inválido: o par some, não vira lixo
        assert!(campo(&campos_do_formulario("a=%ZZ"), "a").is_none());
        assert!(campo(&campos_do_formulario("a=%FF"), "a").is_none());
    }
}
