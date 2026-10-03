// Hyurax / Ultrax 1.0: o instalador para Windows.
//
// Um .exe só, que leva o programa dentro. Mostra os termos de uso e só
// instala depois do "li e aceito". Instala para o usuário atual, sem pedir
// administrador, em %LOCALAPPDATA%\Programs\Hyurax; cria os atalhos no Menu
// Iniciar (e, se a pessoa quiser, na Área de Trabalho); registra-se em
// "Aplicativos" para desinstalar pelo Windows; e grava o manifesto
// `instalacao.txt` com tudo o que pôs no disco (hyurax_nucleo::instalacao).
//
// Configuração e carteira ficam em %APPDATA%\Hyurax, e os dados em
// %LOCALAPPDATA%\Hyurax, fora da pasta do programa: instalar por cima
// (atualizar) e desinstalar não tocam neles.
//
// Sem janela: `--silencioso --aceito-os-termos [--area-de-trabalho] [--abrir]`
// (código de saída 0 = instalado, 1 = falhou, 2 = faltou aceitar os termos).
// `--chave-de-teste NOME` registra em "Aplicativos" sob outro nome, para o
// teste de instalação limpa não mexer na instalação de verdade.
//
// Montagem (o roteiro scripts/empacotar-windows.ps1 faz tudo isto):
//   cargo build --release --locked -p hyurax-app --bin hyurax
//   HYURAX_CARGA_EXE=... HYURAX_CARGA_DLL=... cargo build --release --locked -p hyurax-instalador

use std::path::{Path, PathBuf};
use std::process::Command;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Theme, WindowBuilder};
use wry::{WebContext, WebViewBuilder};

use hyurax_nucleo::instalacao::{self, Manifesto};
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::termos;

const PROGRAMA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/Hyurax.exe"));
const DLL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/WebView2Loader.dll"));
const LEIA_ME: &str = include_str!("../LEIA-ME.txt");
const VERSAO: &str = env!("CARGO_PKG_VERSION");
/// Não mostra janela preta de console para os comandos que o instalador roda.
const SEM_JANELA: u32 = 0x0800_0000;

enum Pedido {
    Instalar { area_de_trabalho: bool, abrir: bool },
    Fechar,
    Resposta(String),
}

pub fn main() {
    // Instalação sem janela, para quem instala por script: só com o aceite
    // escrito na linha de comando, que tem o mesmo peso do "li e aceito".
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let chave = chave_do_registro(&argumentos);
    if argumentos.iter().any(|a| a == "--silencioso") {
        let tem = |nome: &str| argumentos.iter().any(|a| a == nome);
        let codigo = if !tem("--aceito-os-termos") {
            2
        } else {
            match instalar(&chave, tem("--area-de-trabalho"), tem("--abrir")) {
                Ok(_) => 0,
                Err(e) => {
                    let _ = std::fs::write(std::env::temp_dir().join("hyurax-instalador-erro.txt"), &e);
                    1
                }
            }
        };
        std::process::exit(codigo);
    }

    let laco = EventLoopBuilder::<Pedido>::with_user_event().build();
    let proxy = laco.create_proxy();
    let Ok(janela) = WindowBuilder::new()
        .with_title("Instalar o Hyurax / Ultrax")
        .with_inner_size(LogicalSize::new(760.0, 680.0))
        .with_min_inner_size(LogicalSize::new(480.0, 520.0))
        .with_theme(Some(Theme::Dark))
        .build(&laco)
    else {
        return;
    };
    let mut contexto = WebContext::new(std::env::var_os("TEMP").map(|t| PathBuf::from(t).join("hyurax-instalador")));
    let pedir = proxy.clone();
    let Ok(visao) = WebViewBuilder::with_web_context(&mut contexto)
        .with_background_color((6, 6, 7, 255))
        .with_devtools(false)
        .with_html(pagina())
        .with_ipc_handler(move |mensagem| {
            let corpo = mensagem.body();
            let pedido = match corpo.as_str() {
                "fechar" => Pedido::Fechar,
                c if c.starts_with("instalar:") => {
                    let partes: Vec<&str> = c.trim_start_matches("instalar:").split(',').collect();
                    Pedido::Instalar {
                        area_de_trabalho: partes.first() == Some(&"1"),
                        abrir: partes.get(1) == Some(&"1"),
                    }
                }
                _ => return,
            };
            let _ = pedir.send_event(pedido);
        })
        .build(&janela)
    else {
        return;
    };
    let responder = proxy.clone();
    laco.run(move |evento, _, controle| {
        *controle = ControlFlow::Wait;
        match evento {
            Event::UserEvent(Pedido::Instalar { area_de_trabalho, abrir }) => {
                let r = instalar(&chave, area_de_trabalho, abrir);
                let js = match r {
                    Ok(destino) => format!("pronto({})", texto_js(&destino.display().to_string())),
                    Err(e) => format!("falhou({})", texto_js(&e)),
                };
                let _ = responder.send_event(Pedido::Resposta(js));
            }
            Event::UserEvent(Pedido::Resposta(js)) => {
                let _ = visao.evaluate_script(&js);
            }
            Event::UserEvent(Pedido::Fechar) | Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *controle = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn texto_js(t: &str) -> String {
    let mut s = String::from("\"");
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '<' => s.push_str("\\u003c"),
            c if (c as u32) < 0x20 => {}
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// A chave de "Aplicativos": a de verdade, ou uma de teste
/// (`--chave-de-teste NOME`, só letras e números) que não colide com a
/// instalação do dono.
fn chave_do_registro(argumentos: &[String]) -> String {
    let teste = argumentos.iter().position(|a| a == "--chave-de-teste").and_then(|i| argumentos.get(i + 1));
    match teste {
        Some(nome) => {
            let limpo: String = nome.chars().filter(char::is_ascii_alphanumeric).take(32).collect();
            format!("{}-teste-{limpo}", instalacao::CHAVE_PADRAO)
        }
        None => instalacao::CHAVE_PADRAO.to_string(),
    }
}

/// Texto para o Bloco de Notas: marca UTF-8 e fim de linha do Windows.
fn para_o_bloco_de_notas(t: &str) -> Vec<u8> {
    let mut v = vec![0xEF, 0xBB, 0xBF];
    v.extend_from_slice(t.replace("\r\n", "\n").replace('\n', "\r\n").as_bytes());
    v
}

fn rodar(programa: &str, args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt as _;
    let saida = Command::new(programa)
        .args(args)
        .creation_flags(SEM_JANELA)
        .output()
        .map_err(|e| format!("não consegui rodar {programa}: {e}"))?;
    if saida.status.success() {
        Ok(())
    } else {
        Err(format!("{programa} falhou: {}", String::from_utf8_lossy(&saida.stderr).trim()))
    }
}

/// Atalho do Windows (.lnk), pelo próprio Windows (WScript.Shell).
fn atalho(arquivo: &Path, alvo: &Path, pasta: &Path) -> Result<(), String> {
    let aspas = |p: &Path| p.display().to_string().replace('\'', "''");
    let script = format!(
        "$a=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$a.TargetPath='{}';$a.WorkingDirectory='{}';$a.Description='Hyurax / Ultrax: nó, carteira, mineração e trabalho útil (rede de teste)';$a.Save()",
        aspas(arquivo),
        aspas(alvo),
        aspas(pasta)
    );
    rodar("powershell.exe", &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
}

fn instalar(chave: &str, area_de_trabalho: bool, abrir: bool) -> Result<PathBuf, String> {
    if PROGRAMA.is_empty() || DLL.is_empty() {
        return Err("este instalador foi montado sem o programa dentro (faltou HYURAX_CARGA_EXE/HYURAX_CARGA_DLL no empacotamento)".into());
    }
    let destino = instalacao::pasta_do_programa()?;
    std::fs::create_dir_all(&destino).map_err(|e| format!("não consegui criar {}: {e}", destino.display()))?;
    let exe = destino.join("Hyurax.exe");
    std::fs::write(&exe, PROGRAMA).map_err(|e| {
        format!("não consegui gravar o programa ({e}). Se o Hyurax estiver aberto, feche e tente de novo.")
    })?;
    std::fs::write(destino.join("WebView2Loader.dll"), DLL).map_err(|e| format!("não consegui gravar a WebView2Loader.dll: {e}"))?;
    std::fs::write(destino.join("TERMOS-DE-USO.txt"), para_o_bloco_de_notas(termos::TEXTO))
        .map_err(|e| format!("não consegui gravar os termos: {e}"))?;
    std::fs::write(destino.join("LEIA-ME.txt"), para_o_bloco_de_notas(LEIA_ME)).map_err(|e| format!("não consegui gravar o LEIA-ME: {e}"))?;

    // Menu Iniciar, sempre; Área de Trabalho, se a pessoa quis.
    let menu = std::env::var_os("APPDATA")
        .map(|a| PathBuf::from(a).join(r"Microsoft\Windows\Start Menu\Programs"))
        .ok_or("não achei a pasta do Menu Iniciar")?;
    std::fs::create_dir_all(&menu).map_err(|e| format!("não consegui criar {}: {e}", menu.display()))?;
    let mut atalhos = vec![menu.join("Hyurax.lnk")];
    if area_de_trabalho && let Some(perfil) = std::env::var_os("USERPROFILE") {
        let desktop = PathBuf::from(perfil).join("Desktop");
        if desktop.is_dir() {
            atalhos.push(desktop.join("Hyurax.lnk"));
        }
    }
    for a in &atalhos {
        atalho(a, &exe, &destino)?;
    }

    // o manifesto: a desinstalação apaga só o que está nele
    let manifesto = Manifesto {
        versao: VERSAO.into(),
        rede: "testnet".into(),
        chave: chave.into(),
        atalhos,
        arquivos: instalacao::ARQUIVOS.iter().map(|s| (*s).to_string()).collect(),
    };
    std::fs::write(destino.join(instalacao::MANIFESTO), manifesto.texto()).map_err(|e| format!("não consegui gravar o manifesto: {e}"))?;

    // "Aplicativos", só para este usuário.
    let exe_texto = exe.display().to_string();
    let desinstalar = format!("\"{exe_texto}\" --desinstalar");
    let tamanho_kb = ((PROGRAMA.len() + DLL.len()) / 1024).to_string();
    let local = destino.display().to_string();
    for (nome, tipo, valor) in [
        ("DisplayName", "REG_SZ", instalacao::NOME),
        ("DisplayVersion", "REG_SZ", VERSAO),
        ("Publisher", "REG_SZ", "Projeto Hyurax (independente)"),
        ("Comments", "REG_SZ", "Programa 1.0; a rede é de teste e o HYX não tem valor."),
        ("DisplayIcon", "REG_SZ", exe_texto.as_str()),
        ("InstallLocation", "REG_SZ", local.as_str()),
        ("UninstallString", "REG_SZ", desinstalar.as_str()),
        ("URLInfoAbout", "REG_SZ", "https://github.com/NOXqubit/hyurax"),
        ("NoModify", "REG_DWORD", "1"),
        ("NoRepair", "REG_DWORD", "1"),
        ("EstimatedSize", "REG_DWORD", tamanho_kb.as_str()),
    ] {
        rodar("reg.exe", &["add", chave, "/v", nome, "/t", tipo, "/d", valor, "/f"])?;
    }

    // o aceite fica na pasta de configuração, e o programa não pergunta de novo
    let pastas = Pastas::do_sistema()?;
    pastas.criar()?;
    termos::aceitar(&pastas.config, "instalador")?;
    if abrir {
        use std::os::windows::process::CommandExt as _;
        let _ = Command::new(&exe).current_dir(&destino).creation_flags(SEM_JANELA).spawn();
    }
    Ok(destino)
}

fn pagina() -> String {
    // instalar por cima: diz de qual versão para qual
    let anterior = instalacao::pasta_do_programa().ok().and_then(|p| instalacao::versao_instalada(&p));
    let (acao, verbo, andamento, situacao) = match anterior.as_deref() {
        Some(v) if v == VERSAO => ("Reinstalar", "reinstalar", "Reinstalando", format!("Versão {VERSAO}, já instalada: os arquivos do programa são regravados")),
        Some(v) => ("Atualizar", "atualizar", "Atualizando", format!("Da versão {v} para a {VERSAO}")),
        None => ("Instalar", "instalar", "Instalando", format!("Versão {VERSAO}")),
    };
    format!(
        r#"<!doctype html><html lang="pt-BR"><head><meta charset="utf-8">
<meta name="color-scheme" content="dark">
<title>Instalar o Hyurax / Ultrax</title>
<style>
:root {{ --fundo:#060607; --painel:#0e0e10; --tinta:#f4f4f2; --tinta-2:rgba(244,244,242,.68); --tinta-3:rgba(244,244,242,.42); --fio:rgba(244,244,242,.22); }}
* {{ box-sizing:border-box; }}
body {{ margin:0; background:var(--fundo); color:var(--tinta); font:14px/1.55 "Segoe UI", system-ui, sans-serif; display:grid; grid-template-rows:auto 1fr auto; height:100vh; }}
header {{ padding:22px 28px 14px; border-bottom:1px solid rgba(244,244,242,.09); }}
header p {{ margin:4px 0 0; color:var(--tinta-2); }}
h1 {{ margin:0; font:700 26px/1.1 "Bahnschrift", "Segoe UI", sans-serif; letter-spacing:.02em; }}
.selo {{ display:inline-block; margin-left:10px; border:1px dashed var(--tinta-2); border-radius:999px; padding:1px 9px; font:600 10px/1.6 Consolas, monospace; letter-spacing:.14em; vertical-align:middle; }}
main {{ overflow:auto; padding:16px 28px; }}
#termos {{ border:1px solid var(--fio); border-radius:8px; padding:6px 18px 16px; background:var(--painel); color:var(--tinta-2); max-height:100%; overflow:auto; }}
#termos h2 {{ font-size:19px; color:var(--tinta); margin:12px 0 4px; }}
#termos h3 {{ font-size:14.5px; color:var(--tinta); margin:16px 0 4px; }}
#termos strong {{ color:var(--tinta); }}
#termos code {{ font-family:Consolas, monospace; font-size:12.5px; }}
footer {{ padding:14px 28px 20px; border-top:1px solid rgba(244,244,242,.09); display:grid; gap:10px; }}
label {{ display:flex; gap:10px; align-items:center; cursor:pointer; color:var(--tinta-2); }}
input {{ accent-color:var(--tinta); width:16px; height:16px; }}
.acoes {{ display:flex; gap:10px; justify-content:flex-end; margin-top:4px; }}
button {{ font:600 13px/1 "Segoe UI", sans-serif; letter-spacing:.06em; padding:11px 20px; border-radius:7px; cursor:pointer; }}
.principal {{ background:var(--tinta); color:var(--fundo); border:1px solid var(--tinta); }}
.principal:disabled {{ background:transparent; color:var(--tinta-3); border:1px dashed var(--fio); cursor:not-allowed; }}
.secundario {{ background:transparent; color:var(--tinta); border:1px solid var(--fio); }}
#estado {{ margin:0; min-height:1.4em; color:var(--tinta); }}
#estado.erro {{ border-left:3px solid var(--tinta); padding-left:10px; }}
</style></head><body>
<header><h1>{acao} o Hyurax / Ultrax <span class="selo">REDE DE TESTE</span></h1>
<p>{situacao} · para este usuário, sem administrador · a carteira fica em %APPDATA%\Hyurax e nunca é apagada pelo instalador.</p></header>
<main><div id="termos" tabindex="0">{termos}</div></main>
<footer>
<label for="li"><input type="checkbox" id="li" disabled><span id="li-rotulo">Role os termos até o fim para poder aceitar</span></label>
<label for="desk"><input type="checkbox" id="desk" checked><span>Criar atalho na Área de Trabalho</span></label>
<label for="abrir"><input type="checkbox" id="abrir" checked><span>Abrir o Hyurax quando terminar</span></label>
<p id="estado" role="status"></p>
<div class="acoes"><button type="button" class="secundario" id="cancelar">Cancelar</button><button type="button" class="principal" id="instalar" disabled>Aceito os termos e quero {verbo}</button></div>
</footer>
<script>
const t=document.getElementById('termos'), li=document.getElementById('li'), b=document.getElementById('instalar'), st=document.getElementById('estado');
function conferir(){{ if(t.scrollTop+t.clientHeight>=t.scrollHeight-24){{ li.disabled=false; document.getElementById('li-rotulo').textContent='Li e aceito os termos de uso'; }} }}
t.addEventListener('scroll',conferir); conferir();
li.addEventListener('change',()=>{{ b.disabled=!li.checked; }});
document.getElementById('cancelar').addEventListener('click',()=>window.ipc.postMessage('fechar'));
b.addEventListener('click',()=>{{ b.disabled=true; st.className=''; st.textContent='{andamento}…'; window.ipc.postMessage('instalar:'+(document.getElementById('desk').checked?1:0)+','+(document.getElementById('abrir').checked?1:0)); }});
function pronto(pasta){{ st.className=''; st.textContent='Pronto. O Hyurax / Ultrax {VERSAO} está em '+pasta+' e no Menu Iniciar.'; b.textContent='Fechar'; b.disabled=false; b.onclick=()=>window.ipc.postMessage('fechar'); document.getElementById('cancelar').hidden=true; }}
function falhou(erro){{ st.className='erro'; st.textContent='Não deu: '+erro; b.disabled=false; }}
</script></body></html>"#,
        termos = termos::html()
    )
}
