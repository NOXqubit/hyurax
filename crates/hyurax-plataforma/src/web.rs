//! Os painéis (seção 5 do documento da V1), embutidos no programa: a página
//! pública com os preços, a entrada e o cadastro, o painel do cliente, o do
//! fornecedor e o do administrador. Uma página só; o JavaScript troca as
//! telas pelo endereço depois do `#`.
//!
//! O tema e as fontes são os do programa (Cloud Design 2.0), copiados de
//! `crates/hyurax-interface/ui` na compilação.

use crate::Plataforma;
use crate::http::Resposta;
use crate::servico::{Erro, R};

macro_rules! arquivo {
    ($caminho:literal, $tipo:literal) => {
        ($caminho, $tipo, include_bytes!(concat!("../web/", $caminho)) as &[u8])
    };
}

const ARQUIVOS: &[(&str, &str, &[u8])] = &[
    arquivo!("index.html", "text/html; charset=utf-8"),
    arquivo!("app.css", "text/css; charset=utf-8"),
    arquivo!("app.js", "text/javascript; charset=utf-8"),
    arquivo!("favicon.svg", "image/svg+xml"),
];

const FONTES: &[(&str, &[u8])] = &[
    ("archivo.woff2", include_bytes!("../../hyurax-interface/ui/fontes/archivo.woff2")),
    ("plex-mono-400.woff2", include_bytes!("../../hyurax-interface/ui/fontes/plex-mono-400.woff2")),
    ("plex-mono-500.woff2", include_bytes!("../../hyurax-interface/ui/fontes/plex-mono-500.woff2")),
    ("plex-mono-600.woff2", include_bytes!("../../hyurax-interface/ui/fontes/plex-mono-600.woff2")),
];

const TOKENS: &[u8] = include_bytes!("../../hyurax-interface/ui/tokens.css");

/// Serve um arquivo da interface. Qualquer caminho sem extensão cai na
/// página (a navegação é pelo `#`).
pub fn pagina(_p: &Plataforma, caminho: &str) -> R<Resposta> {
    let nome = caminho.trim_start_matches('/');
    if nome == "tokens.css" {
        return Ok(Resposta::bytes("text/css; charset=utf-8", TOKENS.to_vec()).com("Cache-Control", "public, max-age=3600".into()));
    }
    if let Some(f) = nome.strip_prefix("fontes/") {
        let (_, b) = FONTES.iter().find(|(n, _)| *n == f).ok_or_else(|| Erro::nao_achado("fonte não encontrada"))?;
        return Ok(Resposta::bytes("font/woff2", b.to_vec()).com("Cache-Control", "public, max-age=604800".into()));
    }
    let nome = if nome.is_empty() || !nome.contains('.') { "index.html" } else { nome };
    let (_, tipo, b) = ARQUIVOS.iter().find(|(n, _, _)| *n == nome).ok_or_else(|| Erro::nao_achado("página não encontrada"))?;
    let cache = if nome == "index.html" { "no-cache" } else { "public, max-age=300" };
    Ok(Resposta::bytes(tipo, b.to_vec()).com("Cache-Control", cache.into()))
}
