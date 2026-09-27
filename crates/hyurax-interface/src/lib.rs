//! A interface do Hyurax / Ultrax: HTML, CSS, JavaScript (módulos) e as
//! fontes, embutidos no programa. Nada é lido do disco em tempo de execução:
//! o que o servidor local entrega é exatamente o que foi compilado.
//!
//! A tela só consome a API do núcleo (`hyurax-nucleo::api`): o estado e os
//! eventos pelo fluxo `/api/v1/fluxo`, e os comandos por `POST /api/v1/…`.

/// Um arquivo: (caminho no servidor, tipo MIME, bytes).
pub type Arquivo = (&'static str, &'static str, &'static [u8]);

macro_rules! arquivo {
    ($caminho:literal, $tipo:literal) => {
        (concat!("/", $caminho), $tipo, include_bytes!(concat!("../ui/", $caminho)) as &'static [u8])
    };
}

const JS: &str = "text/javascript; charset=utf-8";

/// Todos os arquivos da interface.
pub const ARQUIVOS: &[Arquivo] = &[
    arquivo!("index.html", "text/html; charset=utf-8"),
    arquivo!("painel.css", "text/css; charset=utf-8"),
    ("/painel.js", JS, include_bytes!("../ui/painel.js")),
    ("/ciencia.js", JS, include_bytes!("../ui/ciencia.js")),
    ("/moleculas.js", JS, include_bytes!("../ui/moleculas.js")),
    ("/gpu.js", JS, include_bytes!("../ui/gpu.js")),
    ("/gpu-trabalhador.js", JS, include_bytes!("../ui/gpu-trabalhador.js")),
    arquivo!("fontes/archivo.woff2", "font/woff2"),
    arquivo!("fontes/plex-mono-400.woff2", "font/woff2"),
    arquivo!("fontes/plex-mono-500.woff2", "font/woff2"),
    arquivo!("fontes/plex-mono-600.woff2", "font/woff2"),
];

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn caminhos_unicos_e_com_barra() {
        for (i, (c, _, bytes)) in ARQUIVOS.iter().enumerate() {
            assert!(c.starts_with('/'), "{c}");
            assert!(!bytes.is_empty(), "{c} vazio");
            assert!(ARQUIVOS.iter().skip(i + 1).all(|(o, _, _)| o != c), "{c} repetido");
        }
    }
}
