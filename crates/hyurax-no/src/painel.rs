//! Painel do minerador: o nó, a mineração e o dashboard, num programa só.
//!
//! Dois jeitos de usar o mesmo motor:
//!
//! ```text
//! hyurax-no painel --arquivo carteira.txt [--painel-porta 8800] [--painel-rede]   (terminal + navegador)
//! Hyurax Minerador.exe                                                          (programa com janela)
//! ```
//!
//! No programa com janela ([`app`]), tudo mora na pasta de dados do usuário
//! (`%APPDATA%\Hyurax` no Windows): a cadeia, a identidade do nó, a carteira e
//! os ajustes. Na primeira abertura não há carteira: a própria janela cria uma,
//! com senha, ou importa um `carteira.txt` que o usuário já tenha.
//!
//! Segurança do servidor local:
//! - Comando (POST) só vem do próprio computador, com `Host` e `Origin` locais:
//!   uma página maliciosa aberta no navegador não consegue mandar o minerador
//!   fazer nada, nem por DNS apontado para 127.0.0.1.
//! - Pedido limitado a 16 KiB de cabeçalho e 8 KiB de corpo, tempo de leitura
//!   curto, nenhum arquivo do disco servido: a interface vem embutida no programa.
//! - A senha da carteira só passa pela memória na hora de cifrar e não é
//!   guardada nem registrada em lugar nenhum.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyurax_block::Block;
use hyurax_consensus::block_reward;
use hyurax_crypto::ADDRESS_LEN;
use hyurax_net::Rede;
use hyurax_pow::ConfigMineracao;
use hyurax_tx::{HYUR, Tx};

use crate::{Opcoes, carteira, hex, hyur, salvar, subir_rede};

const INDEX: &str = include_str!("../painel/index.html");
const CSS: &str = include_str!("../painel/painel.css");
const JS: &str = include_str!("../painel/painel.js");
const ESTACAO: &str = include_str!("../painel/estacao.js");
const THREE: &str = include_str!("../../../site/vendor/three.module.min.js");

/// Versão mostrada no painel.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Quantos eventos e amostras o painel guarda na memória.
const EVENTOS_MAX: usize = 80;
const AMOSTRAS_MAX: usize = 120;
const BLOCOS_NO_LIVRO: usize = 24;
const SEMENTES_MAX: usize = 16;
/// Porta padrão do painel e do nó no programa com janela.
const PORTA_PAINEL: u16 = 8800;
const PORTA_P2P: u16 = 8790;

/// Onde o motor guarda as coisas e como ele se apresenta.
struct Ambiente {
    /// Pasta de dados (cadeia, identidade, carteira, ajustes).
    dados: PathBuf,
    /// Arquivo da carteira que recebe a recompensa.
    arquivo_carteira: Option<PathBuf>,
    /// Rodando dentro do programa com janela: carteira e ajustes pela interface.
    app: bool,
    porta_p2p: u16,
}

/// Tudo o que o minerador e o nó contam ao painel.
struct Painel {
    ambiente: Ambiente,
    endereco: Mutex<Option<[u8; ADDRESS_LEN]>>,
    sementes: Mutex<Vec<String>>,
    nucleos: u32,
    minerando: AtomicBool,
    linhas: AtomicU32,
    /// Liga para a rodada atual abandonar a busca (parou, ou chegou bloco novo).
    interromper: AtomicBool,
    /// Tentativas de todas as rodadas, somadas.
    tentativas: AtomicU64,
    meus: AtomicU64,
    perdidos: AtomicU64,
    inicio: Instant,
    eventos: Mutex<VecDeque<Evento>>,
    /// (segundos desde o início, tentativas acumuladas), uma a cada segundo.
    amostras: Mutex<VecDeque<(f64, u64)>>,
    /// Quando a rodada atual começou e em que altura.
    rodada: Mutex<Option<(Instant, u64)>>,
}

struct Evento {
    quando: u64,
    tipo: &'static str,
    texto: String,
}

fn agora_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Painel {
    fn registrar(&self, tipo: &'static str, texto: String) {
        // No programa com janela o registro vai para arquivo, nunca para a saída
        // do terminal: sem console, ou com um console que ninguém lê, escrever
        // pode travar a linha de execução que está respondendo a um pedido.
        if self.ambiente.app {
            self.anotar(tipo, &texto);
        } else {
            println!("  [{tipo}] {texto}");
        }
        if let Ok(mut fila) = self.eventos.lock() {
            fila.push_back(Evento { quando: agora_unix(), tipo, texto });
            while fila.len() > EVENTOS_MAX {
                fila.pop_front();
            }
        }
    }

    /// Anota o evento em `PASTA/hyurax.log`. Se o arquivo passar de 1 MiB,
    /// recomeça: registro serve para entender um problema recente, não é história.
    fn anotar(&self, tipo: &str, texto: &str) {
        use std::io::Write as _;
        let arquivo = self.ambiente.dados.join("hyurax.log");
        let grande = std::fs::metadata(&arquivo).is_ok_and(|m| m.len() > 1024 * 1024);
        let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(!grande).write(true).truncate(grande).open(&arquivo) else {
            return;
        };
        let _ = writeln!(f, "{} [{tipo}] {texto}", agora_unix());
    }

    fn endereco(&self) -> Option<[u8; ADDRESS_LEN]> {
        self.endereco.lock().ok().and_then(|e| *e)
    }

    /// Tentativas por segundo nos últimos ~10 s.
    fn ritmo(&self) -> f64 {
        let Ok(a) = self.amostras.lock() else { return 0.0 };
        let (Some(fim), Some(comeco)) = (a.back(), a.iter().rev().nth(10).or(a.front())) else {
            return 0.0;
        };
        let dt = fim.0 - comeco.0;
        if dt <= 0.0 { 0.0 } else { fim.1.saturating_sub(comeco.1) as f64 / dt }
    }

    /// Grava os ajustes do programa com janela (sementes, núcleos, se minerava).
    fn gravar_ajustes(&self) {
        if !self.ambiente.app {
            return;
        }
        let mut texto = String::from(
            "# Ajustes do Hyurax Minerador. Pode apagar: volta tudo ao padrão.\n",
        );
        let _ = writeln!(texto, "minerar={}", u8::from(self.minerando.load(Ordering::Relaxed)));
        let _ = writeln!(texto, "linhas={}", self.linhas.load(Ordering::Relaxed));
        if let Ok(s) = self.sementes.lock() {
            for semente in s.iter() {
                let _ = writeln!(texto, "semente={semente}");
            }
        }
        let arquivo = self.ambiente.dados.join("ajustes.txt");
        let temporario = arquivo.with_extension("tmp");
        if std::fs::write(&temporario, texto).is_ok() {
            let _ = std::fs::rename(&temporario, &arquivo);
        }
    }
}

/// Ajustes salvos pelo programa com janela.
#[derive(Default)]
struct Ajustes {
    minerar: bool,
    linhas: Option<u32>,
    sementes: Vec<String>,
}

fn ler_ajustes(dados: &Path) -> Ajustes {
    let mut a = Ajustes::default();
    let Ok(texto) = std::fs::read_to_string(dados.join("ajustes.txt")) else { return a };
    for linha in texto.lines() {
        match linha.trim().split_once('=') {
            Some(("minerar", v)) => a.minerar = v.trim() == "1",
            Some(("linhas", v)) => a.linhas = v.trim().parse().ok().filter(|n| *n >= 1),
            Some(("semente", v)) if semente_valida(v.trim()) && a.sementes.len() < SEMENTES_MAX => {
                a.sementes.push(v.trim().to_string());
            }
            _ => {}
        }
    }
    a
}

/// `host:porta`, com porta de 1 a 65535 e host sem espaço nem caractere estranho.
fn semente_valida(s: &str) -> bool {
    let Some((host, porta)) = s.rsplit_once(':') else { return false };
    !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'))
        && porta.parse::<u16>().is_ok_and(|p| p > 0)
}

/// A pasta de dados do programa com janela: `%APPDATA%\Hyurax` no Windows,
/// `~/.hyurax` nos outros sistemas.
pub fn pasta_de_dados() -> Result<PathBuf, String> {
    let base = std::env::var_os("APPDATA")
        .map(|a| PathBuf::from(a).join("Hyurax"))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".hyurax")))
        .ok_or("não achei a pasta do usuário (APPDATA ou HOME)")?;
    std::fs::create_dir_all(&base).map_err(|e| format!("não consegui criar {}: {e}", base.display()))?;
    Ok(base)
}

/// O motor ligado: o painel responde em [`Pronto::url`].
pub struct Pronto {
    /// Porta local do painel.
    pub porta: u16,
    painel: Arc<Painel>,
    rede: Arc<Rede>,
    o: Arc<Opcoes>,
}

impl Pronto {
    /// Endereço do painel para abrir na janela.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.porta)
    }

    /// Para de minerar e grava a cadeia: chamar antes de fechar o programa.
    pub fn encerrar(&self) {
        self.painel.minerando.store(false, Ordering::Relaxed);
        self.painel.interromper.store(true, Ordering::Relaxed);
        let _ = salvar(&self.rede, &self.o);
    }
}

/// Resultado de procurar o painel numa porta.
pub enum JaAberto {
    /// Porta livre: pode ligar o motor.
    Nao,
    /// Já há um Hyurax Minerador respondendo nesta porta.
    Sim(u16),
}

/// Outro Hyurax já está aberto neste computador? Dois motores na mesma pasta
/// estragariam a cadeia gravada; o segundo só abre uma janela para o primeiro.
pub fn ja_aberto() -> JaAberto {
    let Ok(mut s) = TcpStream::connect_timeout(
        &SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORTA_PAINEL),
        Duration::from_millis(400),
    ) else {
        return JaAberto::Nao;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let pedido = format!("GET /api/estado HTTP/1.1\r\nHost: 127.0.0.1:{PORTA_PAINEL}\r\nConnection: close\r\n\r\n");
    let mut resposta = Vec::new();
    if s.write_all(pedido.as_bytes()).is_ok() {
        let _ = s.read_to_end(&mut resposta);
    }
    if String::from_utf8_lossy(&resposta).contains("\"rede\":\"hyurax-") {
        JaAberto::Sim(PORTA_PAINEL)
    } else {
        JaAberto::Nao
    }
}

/// Liga o motor do programa com janela: nó na rede de teste, painel local e
/// mineração, com tudo guardado na pasta de dados do usuário.
pub fn app() -> Result<Pronto, String> {
    let dados = pasta_de_dados()?;
    let ajustes = ler_ajustes(&dados);
    let pasta = dados.to_string_lossy().into_owned();
    let mut args: Vec<String> = ["--rede", "testnet", "--pasta", pasta.as_str()].map(String::from).to_vec();
    for s in &ajustes.sementes {
        args.extend(["--semente".to_string(), s.clone()]);
    }
    if let Some(l) = ajustes.linhas {
        args.extend(["--linhas".to_string(), l.to_string()]);
    }
    // Escuta na porta do nó para os outros acharem este; se ela estiver ocupada,
    // o nó ainda funciona, só sem receber conexões.
    let mut com_porta = args.clone();
    com_porta.extend(["--porta".to_string(), PORTA_P2P.to_string()]);
    let (o, rede, porta_p2p) = match crate::ler_opcoes(&com_porta).and_then(|o| subir_rede(&o).map(|r| (o, r))) {
        Ok((o, r)) => (o, r, PORTA_P2P),
        Err(_) => {
            let o = crate::ler_opcoes(&args)?;
            let r = subir_rede(&o)?;
            (o, r, 0)
        }
    };
    let arquivo_carteira = dados.join("carteira.txt");
    let endereco = std::fs::read_to_string(&arquivo_carteira).ok().and_then(|t| carteira::endereco(&t).ok());
    let ambiente = Ambiente { dados, arquivo_carteira: Some(arquivo_carteira), app: true, porta_p2p };
    let pronto = ligar(o, rede, ambiente, endereco, ajustes.sementes, PORTA_PAINEL, false)?;
    if ajustes.minerar && endereco.is_some() {
        pronto.painel.minerando.store(true, Ordering::Relaxed);
    }
    {
        let (painel, rede, o) = (Arc::clone(&pronto.painel), Arc::clone(&pronto.rede), Arc::clone(&pronto.o));
        std::thread::spawn(move || {
            if let Err(e) = vigiar(&painel, &rede, &o) {
                painel.registrar("erro", e);
            }
        });
    }
    Ok(pronto)
}

/// `hyurax-no painel`: nó + mineração + dashboard, pelo terminal.
pub fn painel(args: &[String]) -> Result<(), String> {
    // Opções próprias do painel saem antes; o resto é igual ao do minerador.
    let mut porta_painel: u16 = PORTA_PAINEL;
    let mut na_rede = false;
    let mut resto = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--painel-porta" => {
                porta_painel = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--painel-porta precisa de um número")?;
            }
            "--painel-rede" => na_rede = true,
            _ => resto.push(a.clone()),
        }
    }
    let o = crate::ler_opcoes(&resto)?;
    let endereco = match (o.endereco, &o.arquivo) {
        (Some(e), _) => e,
        (None, Some(arquivo)) => carteira::endereco(&crate::ler_arquivo(arquivo)?)?,
        (None, None) => return Err("falta --arquivo carteira.txt (ou --endereco HEX) para receber a recompensa".into()),
    };
    let rede = subir_rede(&o)?;
    let ambiente = Ambiente { dados: o.pasta.clone(), arquivo_carteira: o.arquivo.clone(), app: false, porta_p2p: o.porta };
    let sementes = o.sementes.clone();
    let pronto = ligar(o, rede, ambiente, Some(endereco), sementes, porta_painel, na_rede)?;
    println!("Painel: {}", pronto.url());
    if na_rede {
        println!("  (visível na rede local; comandos só deste computador)");
    }
    vigiar(&pronto.painel, &pronto.rede, &pronto.o)
}

/// Monta o painel, abre a porta local e põe o servidor e o minerador para rodar.
fn ligar(
    o: Opcoes,
    rede: Arc<Rede>,
    ambiente: Ambiente,
    endereco: Option<[u8; ADDRESS_LEN]>,
    sementes: Vec<String>,
    porta_painel: u16,
    na_rede: bool,
) -> Result<Pronto, String> {
    let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
    let app = ambiente.app;
    let painel = Arc::new(Painel {
        ambiente,
        endereco: Mutex::new(endereco),
        sementes: Mutex::new(sementes),
        nucleos,
        minerando: AtomicBool::new(false),
        linhas: AtomicU32::new(o.linhas.clamp(1, nucleos.max(1))),
        interromper: AtomicBool::new(false),
        tentativas: AtomicU64::new(0),
        meus: AtomicU64::new(0),
        perdidos: AtomicU64::new(0),
        inicio: Instant::now(),
        eventos: Mutex::new(VecDeque::new()),
        amostras: Mutex::new(VecDeque::new()),
        rodada: Mutex::new(None),
    });
    painel.registrar("no", format!("nó no ar na rede {}", o.rede.nome));
    if app && endereco.is_none() {
        painel.registrar("carteira", "nenhuma carteira ainda: crie ou importe uma para minerar".into());
    }

    let ip = if na_rede { IpAddr::V4(Ipv4Addr::UNSPECIFIED) } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
    let ouvinte = TcpListener::bind(SocketAddr::new(ip, porta_painel))
        .map_err(|e| format!("não consegui abrir o painel na porta {porta_painel}: {e}"))?;
    let o = Arc::new(o);
    {
        let (painel, rede, o) = (Arc::clone(&painel), Arc::clone(&rede), Arc::clone(&o));
        std::thread::spawn(move || servir(ouvinte, porta_painel, &painel, &rede, &o));
    }
    {
        let (painel, rede, o) = (Arc::clone(&painel), Arc::clone(&rede), Arc::clone(&o));
        std::thread::spawn(move || minerador(&painel, &rede, &o));
    }
    Ok(Pronto { porta: porta_painel, painel, rede, o })
}

/// Laço de fundo: amostra o ritmo, grava a cadeia, percebe blocos de fora.
fn vigiar(painel: &Arc<Painel>, rede: &Arc<Rede>, o: &Opcoes) -> Result<(), String> {
    let mut ultima_altura = u64::MAX;
    let mut ultimos_pares = usize::MAX;
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let t = painel.inicio.elapsed().as_secs_f64();
        if let Ok(mut a) = painel.amostras.lock() {
            a.push_back((t, painel.tentativas.load(Ordering::Relaxed)));
            while a.len() > AMOSTRAS_MAX {
                a.pop_front();
            }
        }
        let (altura, pares) = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            (no.chain.height(), rede.pares_conectados())
        };
        if altura != ultima_altura {
            // Chegou bloco de outro nó: a rodada em curso minera em cima de ponta
            // velha. (Se o bloco foi meu, a rodada seguinte já nasceu na ponta nova.)
            let rodada_velha = painel.rodada.lock().ok().and_then(|r| *r).is_some_and(|(_, a)| a <= altura);
            if rodada_velha {
                painel.interromper.store(true, Ordering::Relaxed);
            }
            salvar(rede, o)?;
            ultima_altura = altura;
        }
        if pares != ultimos_pares {
            if ultimos_pares != usize::MAX {
                painel.registrar("rede", format!("{pares} par(es) conectado(s)"));
            }
            ultimos_pares = pares;
        }
    }
}

/// Minera enquanto o painel mandar; cada rodada é um candidato em cima da ponta atual.
fn minerador(painel: &Arc<Painel>, rede: &Arc<Rede>, o: &Opcoes) {
    let mut estava = false;
    loop {
        let endereco = painel.endereco();
        if !painel.minerando.load(Ordering::Relaxed) || endereco.is_none() {
            if estava {
                painel.registrar("minerador", "mineração parada".into());
                estava = false;
            }
            if let Ok(mut r) = painel.rodada.lock() {
                *r = None;
            }
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        let Some(endereco) = endereco else { continue };
        if !estava {
            let linhas = painel.linhas.load(Ordering::Relaxed);
            painel.registrar(
                "minerador",
                format!(
                    "mineração ligada: {linhas} núcleo(s), {:.0} MiB de memória",
                    (u64::from(o.rede.pow.memoria_kib) * u64::from(linhas)) as f64 / 1024.0
                ),
            );
            estava = true;
        }
        if let Err(e) = uma_rodada(painel, rede, o, endereco) {
            painel.registrar("erro", e);
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

fn uma_rodada(painel: &Arc<Painel>, rede: &Arc<Rede>, o: &Opcoes, endereco: [u8; ADDRESS_LEN]) -> Result<(), String> {
    let candidato = {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        no.chain
            .build_candidate(endereco, no.mempool_ordenado(), None, Vec::new())
            .map_err(|e| e.to_string())?
    };
    let altura = candidato.header.height;
    let inicio = Instant::now();
    if let Ok(mut r) = painel.rodada.lock() {
        *r = Some((inicio, altura));
    }
    painel.interromper.store(false, Ordering::Relaxed);
    let config = ConfigMineracao {
        linhas: painel.linhas.load(Ordering::Relaxed).max(1),
        nonce_inicial: agora_unix().wrapping_mul(0x9E37_79B9),
        limite: None,
        pausa: Duration::from_millis(o.pausa_ms),
    };
    // Um vigia liga a interrupção quando o painel manda parar.
    let fim = AtomicBool::new(false);
    let resultado = std::thread::scope(|s| {
        s.spawn(|| {
            while !fim.load(Ordering::Relaxed) {
                if !painel.minerando.load(Ordering::Relaxed) {
                    painel.interromper.store(true, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        let r = hyurax_pow::minerar(
            &candidato.header.encode(),
            o.rede.pow,
            config,
            &painel.interromper,
            &painel.tentativas,
        );
        fim.store(true, Ordering::Relaxed);
        r
    })
    .map_err(|e| e.to_string())?;

    let Some(achado) = resultado.achado else {
        return Ok(()); // interrompida: bloco novo chegou ou mandaram parar
    };
    let n = candidato.useful_proof.as_ref().map_or(0, |p| p.n);
    let bloco = Block { header: candidato.header.with_nonce(achado.nonce), ..candidato };
    match rede.submeter_bloco(bloco) {
        Ok(true) => {
            salvar(rede, o)?;
            painel.meus.fetch_add(1, Ordering::Relaxed);
            painel.registrar(
                "meu-bloco",
                format!(
                    "bloco {altura} minerado em {:.1} s · trabalho útil {n}×{n} · +{} HYUR",
                    inicio.elapsed().as_secs_f64(),
                    hyur(u128::from(block_reward(altura, &o.rede)))
                ),
            );
        }
        Ok(false) | Err(_) => {
            painel.perdidos.fetch_add(1, Ordering::Relaxed);
            painel.registrar("perdido", format!("bloco {altura} perdido na corrida: outro nó chegou primeiro"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Carteira pelo painel (programa com janela)
// ---------------------------------------------------------------------------

fn criar_carteira(painel: &Painel, campos: &[(String, String)]) -> Result<String, String> {
    let arquivo = painel.ambiente.arquivo_carteira.as_ref().ok_or("este painel não guarda carteira")?;
    if painel.endereco().is_some() || arquivo.exists() {
        return Err("já existe uma carteira neste computador".into());
    }
    let senha = campo(campos, "senha").unwrap_or_default();
    if campo(campos, "senha2").unwrap_or_default() != senha {
        return Err("as duas senhas não são iguais".into());
    }
    carteira::senha_aceitavel(&senha)?;
    // A chave vem direto do sistema operacional, não do gerador derivado.
    let segredo = hyurax_net::entropia::entropia_do_sistema()?;
    let conteudo = crate::cifrar_segredo(&segredo, &senha)?;
    crate::gravar_carteira(arquivo, &conteudo)?;
    let endereco = carteira::endereco(&conteudo)?;
    if let Ok(mut e) = painel.endereco.lock() {
        *e = Some(endereco);
    }
    painel.registrar("carteira", format!("carteira criada: {}", hex(&endereco)));
    Ok(hex(&endereco))
}

fn importar_carteira(painel: &Painel, campos: &[(String, String)]) -> Result<String, String> {
    let arquivo = painel.ambiente.arquivo_carteira.as_ref().ok_or("este painel não guarda carteira")?;
    if painel.endereco().is_some() || arquivo.exists() {
        return Err("já existe uma carteira neste computador".into());
    }
    let conteudo = campo(campos, "conteudo").unwrap_or_default();
    let conteudo = conteudo.trim();
    if conteudo.is_empty() {
        return Err("cole o conteúdo do arquivo carteira.txt".into());
    }
    let endereco = carteira::endereco(conteudo).map_err(|e| format!("não é uma carteira do Hyurax: {e}"))?;
    let mut texto = conteudo.to_string();
    texto.push('\n');
    crate::gravar_carteira(arquivo, &texto)?;
    if let Ok(mut e) = painel.endereco.lock() {
        *e = Some(endereco);
    }
    painel.registrar("carteira", format!("carteira importada: {}", hex(&endereco)));
    if carteira::e_formato_antigo(conteudo) {
        painel.registrar("carteira", "aviso: esta carteira guarda o segredo sem senha; proteja com hyurax-no carteira cifrar".into());
    }
    Ok(hex(&endereco))
}

fn trocar_sementes(painel: &Painel, rede: &Arc<Rede>, campos: &[(String, String)]) -> Result<Vec<String>, String> {
    let lista = campo(campos, "lista").unwrap_or_default();
    let novas: Vec<String> = lista
        .split([',', '\n', ' ', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if novas.len() > SEMENTES_MAX {
        return Err(format!("no máximo {SEMENTES_MAX} sementes"));
    }
    if let Some(ruim) = novas.iter().find(|s| !semente_valida(s)) {
        return Err(format!("\"{ruim}\" não é IP:PORTA (exemplo: 203.0.113.7:8790)"));
    }
    if let Ok(mut s) = painel.sementes.lock() {
        s.clone_from(&novas);
    }
    painel.gravar_ajustes();
    for s in &novas {
        rede.semear(s);
        match rede.conectar(s.as_str()) {
            Ok(()) => painel.registrar("rede", format!("conectando em {s}")),
            Err(e) => painel.registrar("rede", format!("não consegui conectar em {s}: {e}")),
        }
    }
    Ok(novas)
}

fn abrir_pasta(painel: &Painel) -> Result<(), String> {
    let pasta = &painel.ambiente.dados;
    let programa = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(programa)
        .arg(pasta)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("não consegui abrir {}: {e}", pasta.display()))
}

// ---------------------------------------------------------------------------
// HTTP
// ---------------------------------------------------------------------------

fn servir(ouvinte: TcpListener, porta: u16, painel: &Arc<Painel>, rede: &Arc<Rede>, o: &Arc<Opcoes>) {
    for conexao in ouvinte.incoming() {
        let Ok(s) = conexao else { continue };
        let (painel, rede, o) = (Arc::clone(painel), Arc::clone(rede), Arc::clone(o));
        std::thread::spawn(move || {
            let _ = atender(s, porta, &painel, &rede, &o);
        });
    }
}

struct Pedido {
    metodo: String,
    caminho: String,
    host: String,
    origem: Option<String>,
    corpo: String,
}

fn ler_pedido(s: &mut TcpStream) -> Option<Pedido> {
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut buf = Vec::new();
    let mut pedaco = [0u8; 2048];
    let fim_cab = loop {
        let n = s.read(&mut pedaco).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(pedaco.get(..n)?);
        if let Some(p) = buf.windows(4).position(|j| j == b"\r\n\r\n") {
            break p;
        }
        if buf.len() > 16 * 1024 {
            return None;
        }
    };
    let cabecalho = String::from_utf8_lossy(buf.get(..fim_cab)?).into_owned();
    let mut linhas = cabecalho.split("\r\n");
    let mut primeira = linhas.next()?.split(' ');
    let metodo = primeira.next()?.to_string();
    let caminho = primeira.next()?.to_string();
    let (mut host, mut origem, mut tamanho) = (String::new(), None, 0usize);
    for l in linhas {
        let Some((nome, valor)) = l.split_once(':') else { continue };
        let valor = valor.trim();
        match nome.trim().to_ascii_lowercase().as_str() {
            "host" => host = valor.to_ascii_lowercase(),
            "origin" => origem = Some(valor.to_ascii_lowercase()),
            "content-length" => tamanho = valor.parse().ok().filter(|t| *t <= 8192)?,
            _ => {}
        }
    }
    let mut corpo = buf.get(fim_cab.saturating_add(4)..)?.to_vec();
    while corpo.len() < tamanho {
        let n = s.read(&mut pedaco).ok()?;
        if n == 0 {
            break;
        }
        corpo.extend_from_slice(pedaco.get(..n)?);
    }
    corpo.truncate(tamanho);
    Some(Pedido { metodo, caminho, host, origem, corpo: String::from_utf8_lossy(&corpo).into_owned() })
}

/// Decodifica `application/x-www-form-urlencoded`: `+` é espaço, `%XX` é byte.
fn decodificar(texto: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(texto.len());
    let mut it = texto.bytes();
    while let Some(b) = it.next() {
        match b {
            b'+' => bytes.push(b' '),
            b'%' => {
                let alto = char::from(it.next()?).to_digit(16)?;
                let baixo = char::from(it.next()?).to_digit(16)?;
                bytes.push(u8::try_from(alto * 16 + baixo).ok()?);
            }
            outro => bytes.push(outro),
        }
    }
    String::from_utf8(bytes).ok()
}

fn campos_do_formulario(corpo: &str) -> Vec<(String, String)> {
    corpo
        .split('&')
        .filter_map(|par| {
            let (k, v) = par.split_once('=').unwrap_or((par, ""));
            Some((decodificar(k)?, decodificar(v)?))
        })
        .collect()
}

fn campo(campos: &[(String, String)], nome: &str) -> Option<String> {
    campos.iter().find(|(k, _)| k == nome).map(|(_, v)| v.clone())
}

fn responder(s: &mut TcpStream, status: &str, tipo: &str, corpo: &[u8]) -> std::io::Result<()> {
    let cab = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'\r\n\
         Connection: close\r\n\r\n",
        corpo.len()
    );
    s.write_all(cab.as_bytes())?;
    s.write_all(corpo)?;
    s.flush()
}

fn responder_json(s: &mut TcpStream, resultado: Result<String, String>) -> std::io::Result<()> {
    match resultado {
        Ok(json) => responder(s, "200 OK", "application/json; charset=utf-8", json.as_bytes()),
        Err(erro) => {
            let json = format!("{{\"erro\":{}}}", texto_json(&erro));
            responder(s, "400 Bad Request", "application/json; charset=utf-8", json.as_bytes())
        }
    }
}

fn atender(mut s: TcpStream, porta: u16, painel: &Painel, rede: &Arc<Rede>, o: &Opcoes) -> std::io::Result<()> {
    let Some(p) = ler_pedido(&mut s) else {
        return responder(&mut s, "400 Bad Request", "text/plain", b"pedido invalido");
    };
    let local = s.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
    let hosts_locais = [format!("127.0.0.1:{porta}"), format!("localhost:{porta}")];
    let host_local = hosts_locais.contains(&p.host);
    let rota = p.caminho.split('?').next().unwrap_or("");
    if p.metodo == "GET" {
        return match rota {
            "/" => responder(&mut s, "200 OK", "text/html; charset=utf-8", INDEX.as_bytes()),
            "/painel.css" => responder(&mut s, "200 OK", "text/css; charset=utf-8", CSS.as_bytes()),
            "/painel.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", JS.as_bytes()),
            "/estacao.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", ESTACAO.as_bytes()),
            "/three.module.min.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", THREE.as_bytes()),
            "/api/estado" => {
                let json = estado_json(painel, rede, o, local && host_local);
                responder(&mut s, "200 OK", "application/json; charset=utf-8", json.as_bytes())
            }
            _ => responder(&mut s, "404 Not Found", "text/plain", b"nao existe"),
        };
    }
    if p.metodo != "POST" {
        return responder(&mut s, "405 Method Not Allowed", "text/plain", b"metodo");
    }
    // Só o próprio computador manda, e só a partir da página do painel.
    let origem_ok = p
        .origem
        .as_deref()
        .is_none_or(|og| hosts_locais.iter().any(|h| og == format!("http://{h}")));
    if !(local && host_local && origem_ok) {
        return responder(&mut s, "403 Forbidden", "text/plain", b"comando so deste computador");
    }
    let campos = campos_do_formulario(&p.corpo);
    let app = painel.ambiente.app;
    match rota {
        "/api/minerar" => {
            for (nome, valor) in &campos {
                match nome.as_str() {
                    "ligar" => {
                        let ligar = valor == "1" && painel.endereco().is_some();
                        painel.minerando.store(ligar, Ordering::Relaxed);
                    }
                    "linhas" => {
                        if let Ok(n) = valor.parse::<u32>() {
                            let n = n.clamp(1, painel.nucleos.max(1));
                            if n != painel.linhas.swap(n, Ordering::Relaxed) {
                                // Vale na próxima rodada: interrompe esta para aplicar já.
                                painel.interromper.store(true, Ordering::Relaxed);
                                painel.registrar("minerador", format!("agora com {n} núcleo(s)"));
                            }
                        }
                    }
                    _ => {}
                }
            }
            painel.gravar_ajustes();
            responder(&mut s, "204 No Content", "text/plain", b"")
        }
        "/api/carteira/nova" if app => {
            let r = criar_carteira(painel, &campos).map(|e| format!("{{\"endereco\":\"{e}\"}}"));
            responder_json(&mut s, r)
        }
        "/api/carteira/importar" if app => {
            let r = importar_carteira(painel, &campos).map(|e| format!("{{\"endereco\":\"{e}\"}}"));
            responder_json(&mut s, r)
        }
        "/api/sementes" if app => {
            let r = trocar_sementes(painel, rede, &campos).map(|lista| {
                let itens: Vec<String> = lista.iter().map(|x| texto_json(x)).collect();
                format!("{{\"sementes\":[{}]}}", itens.join(","))
            });
            responder_json(&mut s, r)
        }
        "/api/abrir-pasta" if app => match abrir_pasta(painel) {
            Ok(()) => responder(&mut s, "204 No Content", "text/plain", b""),
            Err(e) => responder_json(&mut s, Err(e)),
        },
        _ => responder(&mut s, "404 Not Found", "text/plain", b"nao existe"),
    }
}

// ---------------------------------------------------------------------------
// Estado em JSON
// ---------------------------------------------------------------------------

fn texto_json(s: &str) -> String {
    let mut j = String::with_capacity(s.len().saturating_add(2));
    j.push('"');
    for c in s.chars() {
        match c {
            '"' => j.push_str("\\\""),
            '\\' => j.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(j, "\\u{:04x}", c as u32);
            }
            c => j.push(c),
        }
    }
    j.push('"');
    j
}

fn estado_json(painel: &Painel, rede: &Rede, o: &Opcoes, pode_mandar: bool) -> String {
    let mut j = String::with_capacity(8 * 1024);
    let endereco = painel.endereco();
    let Ok(no) = rede.no.lock() else {
        return "{\"erro\":\"no travado\"}".into();
    };
    let c = &no.chain;
    let altura = c.height();
    let (saldo, imaturo) = endereco.map_or((0, 0), |e| (c.state.balance(&e, &HYUR), c.state.immature_balance(&e)));
    let rodada = painel.rodada.lock().ok().and_then(|r| *r);
    let e_meu = |b: &Block| {
        endereco.is_some_and(|e| matches!(b.transactions.first(), Some(Tx::Coinbase(cb)) if cb.recipient == e))
    };
    // Blocos meus na cadeia inteira, não só nesta sessão.
    let meus_na_cadeia = c.entries.iter().filter(|e| e_meu(&e.block)).count();
    let sementes: Vec<String> = painel.sementes.lock().map(|s| s.iter().map(|x| texto_json(x)).collect()).unwrap_or_default();
    let _ = write!(
        j,
        "{{\"rede\":{},\"altura\":{altura},\"ponta\":\"{}\",\"trabalho\":\"{}\",\"emitido\":\"{}\",\
         \"pares\":{},\"mempool\":{},\"endereco\":\"{}\",\"saldo\":\"{}\",\"imaturo\":\"{}\",\
         \"recompensa\":\"{}\",\"maturidade\":{},\"minerando\":{},\"linhas\":{},\"nucleos\":{},\
         \"memoria_mib\":{},\"tentativas\":{},\"ritmo\":{:.3},\"meus\":{},\"meus_cadeia\":{meus_na_cadeia},\"perdidos\":{},\
         \"ligado_s\":{},\"rodada_s\":{},\"rodada_altura\":{},\"pode_mandar\":{pode_mandar},\"agora\":{},\
         \"app\":{},\"versao\":\"{VERSAO}\",\"carteira\":{},\"pode_minerar\":{},\"dados\":{},\"sementes\":[{}],\"porta_p2p\":{},",
        texto_json(o.rede.nome),
        hex(&c.tip_hash()),
        c.total_work().to_decimal(),
        hyur(u128::from(c.state.total_emitted)),
        rede.pares_conectados(),
        no.mempool_len(),
        endereco.map(|e| hex(&e)).unwrap_or_default(),
        hyur(u128::from(saldo)),
        hyur(imaturo),
        hyur(u128::from(block_reward(altura.saturating_add(1), &o.rede))),
        o.rede.coinbase_maturity,
        painel.minerando.load(Ordering::Relaxed),
        painel.linhas.load(Ordering::Relaxed),
        painel.nucleos,
        o.rede.pow.memoria_kib / 1024,
        painel.tentativas.load(Ordering::Relaxed),
        painel.ritmo(),
        painel.meus.load(Ordering::Relaxed),
        painel.perdidos.load(Ordering::Relaxed),
        painel.inicio.elapsed().as_secs(),
        rodada.map_or(0.0, |(t, _)| t.elapsed().as_secs_f64()),
        rodada.map_or(0, |(_, a)| a),
        agora_unix(),
        painel.ambiente.app,
        endereco.is_some(),
        endereco.is_some(),
        // A pasta de dados só aparece para quem pode mandar (o próprio PC).
        texto_json(&if pode_mandar { painel.ambiente.dados.display().to_string() } else { String::new() }),
        sementes.join(","),
        painel.ambiente.porta_p2p,
    );
    j.push_str("\"blocos\":[");
    let mut anterior: Option<u64> = None;
    let recentes: Vec<_> = c.entries.iter().rev().take(BLOCOS_NO_LIVRO.saturating_add(1)).collect();
    for (i, e) in recentes.iter().rev().enumerate() {
        let h = &e.block.header;
        let intervalo = anterior.map_or(0, |a| h.timestamp.saturating_sub(a));
        anterior = Some(h.timestamp);
        if i == 0 && recentes.len() > BLOCOS_NO_LIVRO {
            continue; // só serviu para medir o intervalo do seguinte
        }
        let meu = e_meu(&e.block);
        let n = e.block.useful_proof.as_ref().map_or(0, |p| p.n);
        let _ = write!(
            j,
            "{}{{\"altura\":{},\"hash\":\"{}\",\"horario\":{},\"intervalo\":{intervalo},\"txs\":{},\"n\":{n},\"meu\":{meu},\"bits\":\"{:#010x}\"}}",
            if j.ends_with('[') { "" } else { "," },
            h.height,
            hex(&e.block.block_hash()),
            h.timestamp,
            e.block.transactions.len(),
            h.bits,
        );
    }
    drop(no);
    j.push_str("],\"eventos\":[");
    if let Ok(fila) = painel.eventos.lock() {
        for (i, ev) in fila.iter().rev().take(40).enumerate() {
            let _ = write!(
                j,
                "{}{{\"quando\":{},\"tipo\":\"{}\",\"texto\":{}}}",
                if i > 0 { "," } else { "" },
                ev.quando,
                ev.tipo,
                texto_json(&ev.texto)
            );
        }
    }
    j.push_str("],\"amostras\":[");
    if let Ok(a) = painel.amostras.lock() {
        for (i, (t, n)) in a.iter().enumerate() {
            let _ = write!(j, "{}[{t:.1},{n}]", if i > 0 { "," } else { "" });
        }
    }
    j.push_str("]}");
    j
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    #[test]
    fn formulario_decodifica_espaco_e_acento() {
        let c = campos_do_formulario("senha=minha+senha%20longa%C3%A7&x=1&vazio=");
        assert_eq!(campo(&c, "senha").as_deref(), Some("minha senha longaç"));
        assert_eq!(campo(&c, "x").as_deref(), Some("1"));
        assert_eq!(campo(&c, "vazio").as_deref(), Some(""));
        // %XX quebrado ou UTF-8 inválido: o par some, não vira lixo.
        assert!(campo(&campos_do_formulario("a=%ZZ"), "a").is_none());
        assert!(campo(&campos_do_formulario("a=%FF"), "a").is_none());
    }

    #[test]
    fn semente_so_aceita_host_e_porta() {
        assert!(semente_valida("203.0.113.7:8790"));
        assert!(semente_valida("semente.hyurax.com:8790"));
        assert!(!semente_valida("203.0.113.7"));
        assert!(!semente_valida("203.0.113.7:0"));
        assert!(!semente_valida("203.0.113.7:99999"));
        assert!(!semente_valida("host com espaço:8790"));
        assert!(!semente_valida(":8790"));
    }

    #[test]
    fn json_escapa_aspas_barras_e_controle() {
        assert_eq!(texto_json("a\"b\\c\nd"), "\"a\\\"b\\\\c\\u000ad\"");
    }
}
