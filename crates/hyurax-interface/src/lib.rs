//! A interface do Hyurax / Ultrax: HTML, CSS, JavaScript (módulos) e as
//! fontes, embutidos no programa. Nada é lido do disco em tempo de execução:
//! o que o servidor local entrega é exatamente o que foi compilado.
//!
//! A tela só consome a API do núcleo (`hyurax-nucleo::api`): o estado e os
//! eventos pelo fluxo `/api/v1/fluxo`, e os comandos por `POST /api/v1/…`.

/// Um arquivo: (caminho no servidor, tipo MIME, bytes).
pub type Arquivo = (&'static str, &'static str, &'static [u8]);

macro_rules! arquivo {
    ($caminho:literal, $tipo:expr) => {
        (concat!("/", $caminho), $tipo, include_bytes!(concat!("../ui/", $caminho)) as &'static [u8])
    };
}

const JS: &str = "text/javascript; charset=utf-8";
const FONTE: &str = "font/woff2";

/// Todos os arquivos da interface.
pub const ARQUIVOS: &[Arquivo] = &[
    arquivo!("index.html", "text/html; charset=utf-8"),
    arquivo!("estilo.css", "text/css; charset=utf-8"),
    arquivo!("js/app.js", JS),
    arquivo!("js/api.js", JS),
    arquivo!("js/estado.js", JS),
    arquivo!("js/util.js", JS),
    arquivo!("js/moleculas.js", JS),
    arquivo!("js/cena/motor.js", JS),
    arquivo!("js/cena/visualizadores.js", JS),
    arquivo!("js/gpu/computacao.js", JS),
    arquivo!("js/gpu/trabalhador.js", JS),
    arquivo!("js/telas/comum.js", JS),
    arquivo!("js/telas/visao.js", JS),
    arquivo!("js/telas/ultrax.js", JS),
    arquivo!("js/telas/ciencia.js", JS),
    arquivo!("js/telas/historico.js", JS),
    arquivo!("js/telas/carteira.js", JS),
    arquivo!("js/telas/cadeia.js", JS),
    arquivo!("js/telas/rede.js", JS),
    arquivo!("js/telas/registro.js", JS),
    arquivo!("js/telas/ajustes.js", JS),
    arquivo!("vendor/qrcode.min.js", JS),
    arquivo!("fontes/archivo.woff2", FONTE),
    arquivo!("fontes/plex-mono-400.woff2", FONTE),
    arquivo!("fontes/plex-mono-500.woff2", FONTE),
    arquivo!("fontes/plex-mono-600.woff2", FONTE),
];

#[cfg(test)]
mod testes {
    use super::*;

    fn existe(caminho: &str) -> bool {
        ARQUIVOS.iter().any(|(c, _, _)| *c == caminho)
    }

    /// Resolve `./x.js` e `../x.js` a partir da pasta de quem importa.
    fn resolver(de: &str, alvo: &str) -> String {
        if alvo.starts_with('/') {
            return alvo.to_string();
        }
        let mut partes: Vec<&str> = de.split('/').filter(|p| !p.is_empty()).collect();
        partes.pop();
        for p in alvo.split('/') {
            match p {
                "." => {}
                ".." => {
                    partes.pop();
                }
                _ => partes.push(p),
            }
        }
        format!("/{}", partes.join("/"))
    }

    #[test]
    fn caminhos_unicos_e_com_barra() {
        for (i, (c, _, bytes)) in ARQUIVOS.iter().enumerate() {
            assert!(c.starts_with('/'), "{c}");
            assert!(!bytes.is_empty(), "{c} vazio");
            assert!(ARQUIVOS.iter().skip(i + 1).all(|(o, _, _)| o != c), "{c} repetido");
        }
    }

    /// Todo módulo importado (e todo arquivo que o HTML, o CSS e o worker
    /// pedem) está embutido: uma tela nunca quebra por arquivo faltando.
    #[test]
    fn tudo_que_a_tela_pede_esta_embutido() {
        let mut pedidos = Vec::new();
        for (c, _, bytes) in ARQUIVOS {
            let Ok(texto) = std::str::from_utf8(bytes) else { continue };
            for marca in ["from \"", "import(\"", "src=\"/", "href=\"/", "url(\"/", "new Worker(\""] {
                for (k, _) in texto.match_indices(marca) {
                    let resto = texto.get(k + marca.len()..).unwrap_or_default();
                    let fim = resto.find('"').unwrap_or(0);
                    let alvo = resto.get(..fim).unwrap_or_default();
                    let alvo = if marca.ends_with('/') { format!("/{alvo}") } else { alvo.to_string() };
                    if alvo.ends_with(".js") || alvo.ends_with(".css") || alvo.ends_with(".woff2") {
                        pedidos.push((c.to_string(), resolver(c, &alvo)));
                    }
                }
            }
        }
        assert!(pedidos.len() > 20, "o teste não achou os pedidos ({})", pedidos.len());
        for (de, alvo) in pedidos {
            assert!(existe(&alvo), "{de} pede {alvo}, que não está embutido");
        }
    }
}
