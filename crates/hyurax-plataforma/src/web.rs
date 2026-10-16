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
    arquivo!("termos.html", "text/html; charset=utf-8"),
    arquivo!("privacidade.html", "text/html; charset=utf-8"),
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
        return Ok(Resposta::bytes("text/css; charset=utf-8", TOKENS.to_vec()).com("Cache-Control", "public, max-age=86400".into()));
    }
    if let Some(f) = nome.strip_prefix("fontes/") {
        let (_, b) = FONTES.iter().find(|(n, _)| *n == f).ok_or_else(|| Erro::nao_achado("fonte não encontrada"))?;
        return Ok(Resposta::bytes("font/woff2", b.to_vec()).com("Cache-Control", "public, max-age=604800".into()));
    }
    let nome = if nome.is_empty() || !nome.contains('.') { "index.html" } else { nome };
    let (_, tipo, b) = ARQUIVOS.iter().find(|(n, _, _)| *n == nome).ok_or_else(|| Erro::nao_achado("página não encontrada"))?;
    // desenvolvimento: HYX_WEB_DIR serve os mesmos arquivos do disco, sem
    // recompilar (só os nomes da lista acima; nada de caminho de fora)
    if let Ok(pasta) = std::env::var("HYX_WEB_DIR")
        && let Ok(do_disco) = std::fs::read(std::path::Path::new(&pasta).join(nome))
    {
        let corpo = if nome == "index.html" { versionar(&do_disco, &crate::util::agora_ms().to_string()) } else { do_disco };
        return Ok(Resposta::bytes(tipo, corpo).com("Cache-Control", "no-store".into()));
    }
    if nome == "index.html" {
        // a página leva a versão nos endereços do CSS e do JS: depois de uma
        // atualização, ninguém fica com o arquivo velho no cache
        return Ok(Resposta::bytes(tipo, versionar(b, crate::VERSAO)).com("Cache-Control", "no-cache".into()));
    }
    Ok(Resposta::bytes(tipo, b.to_vec()).com("Cache-Control", "public, max-age=86400".into()))
}

fn versionar(html: &[u8], versao: &str) -> Vec<u8> {
    String::from_utf8_lossy(html).replace("__VERSAO__", versao).into_bytes()
}
