// Hyurax: o programa com janela.
//
// Abre com dois cliques, sem terminal. Liga o nó na rede de teste, a
// mineração e o painel, tudo guardado em %APPDATA%\Hyurax, e mostra o painel
// numa janela própria (WebView2, que já vem no Windows 10 e 11). Fechar a
// janela para a mineração e grava a cadeia antes de sair.
//
// Se o programa já estiver aberto, uma segunda cópia não liga outro motor na
// mesma pasta (isso estragaria a cadeia gravada): só abre outra janela para o
// que já está rodando.

#![windows_subsystem = "windows"]

use std::collections::HashMap;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopWindowTarget};
use tao::window::{Icon, Theme, Window, WindowBuilder, WindowId};
use wry::{WebContext, WebView, WebViewBuilder};

use hyurax_no::painel::{self, JaAberto};

const TITULO: &str = "Hyurax";

fn main() {
    let (url, motor) = match painel::ja_aberto() {
        JaAberto::Sim(porta) => (Some(format!("http://127.0.0.1:{porta}/")), None),
        JaAberto::Nao => match painel::app() {
            Ok(pronto) => (Some(pronto.url()), Some(pronto)),
            Err(erro) => {
                abrir_janela(None, Some(&erro), None);
                return;
            }
        },
    };
    abrir_janela(url.as_deref(), None, motor);
}

/// Pedido que vem de dentro da página para o programa.
enum Pedido {
    /// A página pediu uma janela nova (botão "destacar" de um painel).
    Janela(String),
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

fn abrir_janela(url: Option<&str>, erro: Option<&str>, motor: Option<painel::Pronto>) {
    let laco = EventLoopBuilder::<Pedido>::with_user_event().build();
    let proxy = laco.create_proxy();
    let origem = url.map(|u| u.trim_end_matches('/').to_string()).unwrap_or_default();
    // O motor do navegador guarda o cache na pasta de dados do usuário, e não ao
    // lado do .exe: assim o programa roda de qualquer pasta, mesmo sem permissão
    // de escrita (Arquivos de Programas, pendrive protegido).
    let mut contexto = WebContext::new(painel::pasta_de_dados().ok().map(|d| d.join("navegador")));
    let conteudo = match (url, erro) {
        (Some(u), _) => Ok(u),
        (None, e) => Err(e.unwrap_or("erro desconhecido")),
    };
    let Some((principal, visao)) = montar_janela(&laco, &mut contexto, &proxy, &origem, conteudo, true) else {
        return;
    };
    let id_principal = principal.id();
    // As janelas destacadas ficam guardadas: fechar uma não fecha o programa.
    let mut janelas: HashMap<WindowId, (Window, WebView)> = HashMap::new();

    laco.run(move |evento, alvo, controle| {
        *controle = ControlFlow::Wait;
        match evento {
            Event::UserEvent(Pedido::Janela(endereco)) => {
                if let Some((j, v)) = montar_janela(alvo, &mut contexto, &proxy, &origem, Ok(&endereco), false) {
                    janelas.insert(j.id(), (j, v));
                }
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, window_id, .. } => {
                if window_id == id_principal {
                    // Fechar a janela principal encerra o programa, e antes disso
                    // a mineração para e a cadeia é gravada.
                    if let Some(m) = &motor {
                        m.encerrar();
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

/// Página mostrada quando o motor não consegue ligar.
fn pagina_de_erro(erro: &str) -> String {
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
         <h1 style=\"font:700 28px Bahnschrift,sans-serif\">O Hyurax não conseguiu ligar</h1>\
         <p>{seguro}</p><p style=\"opacity:.6\">Feche esta janela e abra o programa de novo. \
         Se continuar, a pasta de dados fica em %APPDATA%\\Hyurax.</p></div>"
    )
}

/// O "H" de nós do Hyurax, desenhado em 64×64 sem arquivo de imagem.
fn icone() -> Option<Icon> {
    const T: usize = 64;
    let mut rgba = vec![0u8; T * T * 4];
    let linhas: [((f32, f32), (f32, f32)); 3] = [((18.0, 14.0), (18.0, 50.0)), ((46.0, 14.0), (46.0, 50.0)), ((18.0, 32.0), (46.0, 32.0))];
    let nos: [(f32, f32, f32); 6] = [(18.0, 14.0, 5.0), (18.0, 50.0, 5.0), (46.0, 14.0, 5.0), (46.0, 50.0, 5.0), (18.0, 32.0, 5.6), (46.0, 32.0, 5.6)];
    // Cobertura suave de 1 px: borda sem serrilhado.
    let cobre = |d: f32| (0.5 - d).clamp(0.0, 1.0);
    for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = ((i % T) as f32 + 0.5, (i / T) as f32 + 0.5);
        // Fundo: quadrado preto de cantos arredondados (raio 14).
        let (cx, cy) = (x.clamp(14.0, 50.0), y.clamp(14.0, 50.0));
        let fundo = cobre(((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - 14.0);
        let mut branco = 0.0f32;
        for ((x1, y1), (x2, y2)) in linhas {
            let (dx, dy) = (x2 - x1, y2 - y1);
            let t = (((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            let d = ((x - x1 - t * dx).powi(2) + (y - y1 - t * dy).powi(2)).sqrt();
            branco = branco.max(cobre(d - 1.8) * 0.6);
        }
        for (nx, ny, r) in nos {
            branco = branco.max(cobre(((x - nx).powi(2) + (y - ny).powi(2)).sqrt() - r));
        }
        let v = (branco * 243.0) as u8;
        px.copy_from_slice(&[v, v, v, (fundo * 255.0) as u8]);
    }
    Icon::from_rgba(rgba, T as u32, T as u32).ok()
}
