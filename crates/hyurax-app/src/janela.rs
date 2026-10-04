// ✝ Salmos 127:1 — “Se o Senhor não edificar a casa, em vão trabalham os que a edificam.”
//! Hyurax / Ultrax 1.0: o programa com janela.
//!
//! Abre com dois cliques, sem terminal. Liga o núcleo (`hyurax-nucleo`) na
//! rede de teste, abre a API local com a interface embutida
//! (`hyurax-interface`) e mostra tudo numa janela própria (WebView2, que já
//! vem no Windows 10 e 11). Fechar a janela para a mineração e o ULTRAX e
//! grava a cadeia antes de sair.
//!
//! Configuração e carteira ficam em `%APPDATA%\Hyurax`; cadeia, ciência e
//! registros, em `%LOCALAPPDATA%\Hyurax` (ver `hyurax_nucleo::pastas`).
//!
//! Se o programa já estiver aberto, uma segunda cópia não liga outro núcleo
//! (a pasta de dados é travada): só abre outra janela para o que já roda.

use std::collections::HashMap;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopWindowTarget};
use tao::window::{Icon, Theme, Window, WindowBuilder, WindowId};
use wry::{WebContext, WebView, WebViewBuilder};

use std::sync::Arc;

use hyurax_nucleo::ajustes::Ajustes;
use hyurax_nucleo::config::{self, ConfigDoNo};
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::servico::{Modo, Nucleo, Partida};

const TITULO: &str = "Hyurax / Ultrax · rede de teste";

pub fn main() {
    if std::env::args().any(|a| a == "--desinstalar") {
        let mensagem = desinstalar();
        if !std::env::args().any(|a| a == "--silencioso") {
            abrir_janela(None, Some(&mensagem), None);
        }
        return;
    }
    if hyurax_nucleo::api::ja_aberto(config::PORTA_PAINEL) {
        // outra janela para o programa que já roda, com a chave de sessão dele
        // (gravada na pasta de configuração deste usuário)
        let chave = Pastas::do_sistema().ok().and_then(|p| hyurax_nucleo::api::ler_chave(&p.config)).unwrap_or_default();
        abrir_janela(Some(&hyurax_nucleo::api::endereco_da_janela(config::PORTA_PAINEL, &chave)), None, None);
        return;
    }
    match ligar() {
        Ok((url, n)) => abrir_janela(Some(&url), None, Some(n)),
        Err(erro) => abrir_janela(None, Some(&erro), None),
    }
}

/// Liga o núcleo e a API. Escuta os outros nós na porta padrão; se ela
/// estiver ocupada, o nó funciona mesmo assim, só sem receber conexões.
fn ligar() -> Result<(String, Arc<Nucleo>), String> {
    let pastas = Pastas::do_sistema()?;
    pastas.criar()?;
    let ajustes = Ajustes::ler(&pastas.config);
    let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
    let partida = |porta: u16| Partida {
        modo: Modo::Janela,
        config: ConfigDoNo::nova(config::REDE_PADRAO, pastas.clone(), porta, ajustes.sementes.clone(), false),
        arquivo_carteira: Some(pastas.carteira()),
        endereco_fixo: None,
        linhas_padrao: (nucleos / 2).max(1),
        pausa_fixa_ms: 0,
        painel_na_rede: false,
    };
    let n = match Nucleo::ligar(partida(config::PORTA_P2P)) {
        Ok(n) => n,
        Err(e) if e.contains("porta") => Nucleo::ligar(partida(0))?,
        Err(e) => return Err(e),
    };
    let porta = hyurax_nucleo::api::abrir(&n, config::PORTA_PAINEL, hyurax_interface::ARQUIVOS)?;
    Ok((hyurax_nucleo::api::endereco_da_janela(porta, &n.chave_painel), n))
}

/// Pedido que vem de dentro da página para o programa.
enum Pedido {
    /// A página pediu uma janela nova (botão "destacar" de um painel).
    Janela(String),
    /// O instalador da versão nova abriu: fecha para ele trocar os arquivos.
    Sair,
}

/// Monta uma janela com o painel dentro. Serve para a principal e para as
/// destacadas: a diferença é só o tamanho e o endereço aberto.
fn montar_janela(
    alvo: &EventLoopWindowTarget<Pedido>,
    contexto: &mut WebContext,
    proxy: &tao::event_loop::EventLoopProxy<Pedido>,
    origem: &str,
    conteudo: Result<&str, &str>,
    principal: bool,
) -> Option<(Window, WebView)> {
    let tamanho = if principal { (1320.0, 820.0) } else { (760.0, 560.0) };
    let janela = WindowBuilder::new()
        .with_title(TITULO)
        .with_inner_size(LogicalSize::new(tamanho.0, tamanho.1))
        .with_min_inner_size(LogicalSize::new(420.0, 360.0))
        .with_window_icon(icone())
        .with_theme(Some(Theme::Dark))
        .build(alvo)
        .ok()?;

    let dentro = origem.to_string();
    let pedir = proxy.clone();
    let construtor = WebViewBuilder::with_web_context(contexto)
        .with_background_color((3, 3, 3, 255))
        .with_devtools(false)
        // Links para fora (GitHub, site) abrem no navegador do sistema; o que é
        // do próprio painel continua dentro do programa.
        .with_navigation_handler({
            let dentro = dentro.clone();
            move |destino: String| {
                if destino.starts_with(&dentro) || destino.starts_with("about:") || destino.starts_with("data:") {
                    true
                } else {
                    abrir_no_navegador(&destino);
                    false
                }
            }
        })
        // Janela nova pedida pela página: se for do painel, o programa abre outra
        // janela dele mesmo. Qualquer outro endereço vai para o navegador.
        .with_new_window_req_handler(move |destino: String| {
            if destino.starts_with(&dentro) {
                let _ = pedir.send_event(Pedido::Janela(destino));
            } else {
                abrir_no_navegador(&destino);
            }
            false
        });
    let construtor = match conteudo {
        Ok(endereco) => construtor.with_url(endereco),
        Err(erro) => construtor.with_html(pagina_de_erro(erro)),
    };
    let visao = construtor.build(&janela).ok()?;
    Some((janela, visao))
}

fn abrir_janela(url: Option<&str>, erro: Option<&str>, motor: Option<Arc<Nucleo>>) {
    let laco = EventLoopBuilder::<Pedido>::with_user_event().build();
    let proxy = laco.create_proxy();
    // a origem do painel, sem o "#chave=…": é com ela que a navegação interna
    // é reconhecida
    let origem = url.map(|u| u.split('#').next().unwrap_or(u).trim_end_matches('/').to_string()).unwrap_or_default();
    // O navegador da janela guarda o cache na pasta de dados do usuário, e não
    // ao lado do .exe: o programa nunca grava na pasta de instalação.
    let mut contexto = WebContext::new(Pastas::do_sistema().ok().map(|p| p.navegador()));
    let conteudo = match (url, erro) {
        (Some(u), _) => Ok(u),
        (None, e) => Err(e.unwrap_or("erro desconhecido")),
    };
    let Some((principal, visao)) = montar_janela(&laco, &mut contexto, &proxy, &origem, conteudo, true) else {
        return;
    };
    let id_principal = principal.id();
    // o instalador da versão nova (conferido) pede para o programa fechar
    if let Some(n) = &motor
        && let Ok(mut s) = n.ao_sair.lock()
    {
        let p = proxy.clone();
        *s = Some(Box::new(move || {
            let _ = p.send_event(Pedido::Sair);
        }));
    }
    // As janelas destacadas ficam guardadas: fechar uma não fecha o programa.
    let mut janelas: HashMap<WindowId, (Window, WebView)> = HashMap::new();

    laco.run(move |evento, alvo, controle| {
        *controle = ControlFlow::Wait;
        match evento {
            Event::UserEvent(Pedido::Sair) => {
                if let Some(n) = &motor {
                    n.encerrar();
                }
                *controle = ControlFlow::Exit;
            }
            Event::UserEvent(Pedido::Janela(endereco)) => {
                if let Some((j, v)) = montar_janela(alvo, &mut contexto, &proxy, &origem, Ok(&endereco), false) {
                    janelas.insert(j.id(), (j, v));
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, window_id, .. } => {
                if window_id == id_principal {
                    // fechar a janela principal encerra o programa: antes, a
                    // mineração e o ULTRAX param e a cadeia é gravada
                    if let Some(n) = &motor {
                        n.encerrar();
                    }
                    *controle = ControlFlow::Exit;
                } else {
                    janelas.remove(&window_id);
                }
            }
            _ => {}
        }
        let _ = &visao;
    });
}

/// Abre um endereço http(s) no navegador do sistema. Nada além de http(s).
fn abrir_no_navegador(destino: &str) {
    if !(destino.starts_with("https://") || destino.starts_with("http://")) {
        return;
    }
    let _ = std::process::Command::new("explorer").arg(destino).spawn();
}

/// O "Aplicativos" do Windows chama `Hyurax.exe --desinstalar`
/// (`--silencioso` junto: sem janela, para script e teste).
///
/// Só age se ao lado do programa houver o manifesto do instalador
/// (`instalacao.txt`, ver `hyurax_nucleo::instalacao`), e apaga só o que está
/// nele: os arquivos do programa, os atalhos e a chave de "Aplicativos". O
/// próprio `Hyurax.exe` e a pasta (se ficar vazia) saem depois que este
/// processo termina, porque o Windows não apaga um .exe em uso.
/// Configuração, carteira e dados ficam: são do dono.
///
/// Devolve a mensagem para mostrar (com o prefixo `desinstalar:`).
fn desinstalar() -> String {
    use hyurax_nucleo::instalacao::{self, Manifesto};
    use std::os::windows::process::CommandExt as _;
    use std::path::PathBuf;
    const SEM_JANELA: u32 = 0x0800_0000;
    const DADOS: &str = r"A configuração e a CARTEIRA continuam em %APPDATA%\Hyurax, e a cadeia e os dados em %LOCALAPPDATA%\Hyurax. Se for apagar essas pastas, guarde antes uma cópia do carteira.txt.";
    let Some(aqui) = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)) else {
        return "desinstalar:Não achei a pasta deste programa.".into();
    };
    let manifesto = match Manifesto::da_pasta(&aqui) {
        Ok(m) if m.arquivos.iter().any(|a| a.eq_ignore_ascii_case("Hyurax.exe")) => m,
        _ => {
            return format!(
                "desinstalar:Esta cópia do Hyurax não foi instalada pelo instalador 1.0 (falta o {} ao lado dela). Para remover, apague a pasta dela. {DADOS}",
                instalacao::MANIFESTO
            );
        }
    };
    let _ = std::process::Command::new("reg.exe").args(["delete", &manifesto.chave, "/f"]).creation_flags(SEM_JANELA).output();
    // Os atalhos e o que não estiver em uso saem agora. O que este processo
    // segura (o .exe e, na montagem GNU, a WebView2Loader.dll) sai depois.
    let pasta_texto = aqui.display().to_string();
    let falhas: Vec<String> = instalacao::remover_arquivos(&aqui, &manifesto, &["Hyurax.exe"])
        .into_iter()
        .filter(|f| !f.starts_with(&pasta_texto))
        .collect();
    // depois que este processo sair: o resto do manifesto (nomes soltos,
    // conferidos por Manifesto::ler), e a pasta só se ficar vazia. Os
    // caminhos vão por variável de ambiente, nunca dentro do texto do script.
    let arquivos = manifesto.arquivos.join("|");
    let script = format!(
        "Wait-Process -Id {} -ErrorAction SilentlyContinue; Start-Sleep -Seconds 1; \
         foreach ($n in $env:HYX_ARQUIVOS.Split('|')) {{ Remove-Item -LiteralPath (Join-Path $env:HYX_PASTA $n) -Force -ErrorAction SilentlyContinue }}; \
         if (-not (Get-ChildItem -LiteralPath $env:HYX_PASTA -Force -ErrorAction SilentlyContinue)) {{ Remove-Item -LiteralPath $env:HYX_PASTA -Force -ErrorAction SilentlyContinue }}",
        std::process::id()
    );
    let _ = std::process::Command::new("powershell.exe")
        .env("HYX_PASTA", &aqui)
        .env("HYX_ARQUIVOS", &arquivos)
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .creation_flags(SEM_JANELA)
        .spawn();
    if falhas.is_empty() {
        format!("desinstalar:O Hyurax / Ultrax {} foi desinstalado. {DADOS}", manifesto.versao)
    } else {
        format!(
            "desinstalar:O Hyurax foi desinstalado, mas não consegui apagar: {}. Feche o programa e apague à mão. {DADOS}",
            falhas.join("; ")
        )
    }
}

/// Página mostrada quando o motor não consegue ligar.
fn pagina_de_erro(erro: &str) -> String {
    // a desinstalação usa a mesma página, com outro título e sem "abra de novo"
    let (titulo, erro, rodape) = match erro.strip_prefix("desinstalar:") {
        Some(m) => ("Desinstalação do Hyurax", m, "Pode fechar esta janela."),
        None => (
            "O Hyurax não conseguiu ligar",
            erro,
            r"Feche esta janela e abra o programa de novo. Se continuar, os registros ficam em %LOCALAPPDATA%\Hyurax\registros.",
        ),
    };
    let seguro: String = erro
        .chars()
        .map(|c| match c {
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '&' => "&amp;".to_string(),
            '"' => "&quot;".to_string(),
            c => c.to_string(),
        })
        .collect();
    format!(
        "<!doctype html><meta charset=utf-8><body style=\"margin:0;background:#030303;color:#f3f3f1;\
         font:15px/1.6 Consolas,monospace;display:grid;place-items:center;min-height:100vh\">\
         <div style=\"max-width:560px;padding:24px;border:1px solid rgba(243,243,241,.3)\">\
         <p style=\"letter-spacing:.3em;font-size:11px;opacity:.6\">HYURAX</p>\
         <h1 style=\"font:700 28px Bahnschrift,sans-serif\">{titulo}</h1>\
         <p>{seguro}</p><p style=\"opacity:.6\">{rodape}</p></div>"
    )
}

/// A marca do Hyurax em 64×64 RGBA, feita por `scripts/gerar-marca.py`.
fn icone() -> Option<Icon> {
    const RGBA: &[u8] = include_bytes!("../../../design/marca/icone-64.rgba");
    Icon::from_rgba(RGBA.to_vec(), 64, 64).ok()
}
