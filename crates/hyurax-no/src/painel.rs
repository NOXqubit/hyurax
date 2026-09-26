//! Painel do minerador: o nó, a mineração e o dashboard, num programa só.
//!
//! Dois jeitos de usar o mesmo motor:
//!
//! ```text
//! hyurax-no painel --arquivo carteira.txt [--painel-porta 8800] [--painel-rede]   (terminal + navegador)
//! Hyurax.exe                                                                    (programa com janela)
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
use hyurax_tx::{HYX, Tx};

use crate::seguranca::Seguranca;
use crate::ultrax::{self, Ultrax};
use crate::{Opcoes, carteira, envio, hex, hyx, maquinas, salvar, subir_rede, totp};

const INDEX: &str = include_str!("../painel/index.html");
const CSS: &str = include_str!("../painel/painel.css");
const JS: &str = include_str!("../painel/painel.js");
const MOLECULAS: &str = include_str!("../painel/moleculas.js");
const GPU: &str = include_str!("../painel/gpu.js");
const GPU_TRABALHADOR: &str = include_str!("../painel/gpu-trabalhador.js");
/// Gerador de QR Code, o mesmo do site: serve para abrir o painel no celular.
const QRCODE: &str = include_str!("../../../site/vendor/qrcode.min.js");

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
/// Quanto um núcleo minerando gasta, em watts, e o preço do kWh em centavos.
/// São estimativas honestas, e o usuário ajusta as duas nos Ajustes.
/// Painéis que existem, na ordem de fábrica. Um "-" na frente quer dizer fechado.
const PAINEIS_PADRAO: &str =
    "ultrax,ia,verificacao,carteira,mineracao,historico,telemetria,livro,fluxo,-rede,-ritmo,-mercado,-maquinas";
const PAINEIS_CONHECIDOS: [&str; 13] = [
    "ultrax",
    "ia",
    "verificacao",
    "carteira",
    "mineracao",
    "historico",
    "telemetria",
    "livro",
    "fluxo",
    "rede",
    "ritmo",
    "mercado",
    "maquinas",
];
/// Quantos movimentos da carteira o painel mostra.
const HISTORICO_MAX: usize = 30;
/// De quanto em quanto tempo as outras máquinas são perguntadas.
const MAQUINAS_INTERVALO: Duration = Duration::from_secs(6);
/// De quanto em quanto tempo o mercado é consultado, e as moedas seguidas.
const MERCADO_INTERVALO: Duration = Duration::from_secs(120);
const MERCADO_MOEDAS: [(&str, &str); 5] = [
    ("bitcoin", "Bitcoin"),
    ("ethereum", "Ethereum"),
    ("solana", "Solana"),
    ("monero", "Monero"),
    ("dogecoin", "Dogecoin"),
];
const WATTS_NUCLEO_PADRAO: u32 = 12;
const CENTAVOS_KWH_PADRAO: u32 = 90;

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
    /// Quanto da CPU a mineração pode ocupar, de 10 a 100 por cento.
    uso_cpu: AtomicU32,
    /// Tempo medido de uma tentativa, em milissegundos: base do limitador.
    ms_tentativa: Mutex<f64>,
    /// Watts que um núcleo minerando gasta, e o preço do kWh em centavos.
    watts_nucleo: AtomicU32,
    centavos_kwh: AtomicU32,
    /// O painel aceita ser visto por outros aparelhos da rede local.
    na_rede: AtomicBool,
    /// Acompanhar o mercado das criptomoedas grandes (liga a internet).
    mercado_ligado: AtomicBool,
    /// Última leitura do mercado, e quando veio.
    mercado: Mutex<Option<(u64, String)>>,
    /// Quais painéis o dono deixou abertos, e em que ordem.
    paineis: Mutex<String>,
    /// Avisar quando um bloco meu entrar: na tela, e com som se o dono quiser.
    avisar_bloco: AtomicBool,
    som_bloco: AtomicBool,
    /// Segundo fator: segredo do código de 6 dígitos e o que ele protege.
    seguranca: Mutex<Seguranca>,
    /// Segredo novo, ainda esperando o primeiro código certo para valer.
    totp_pendente: Mutex<Option<Vec<u8>>>,
    /// Já destrancou nesta abertura do programa.
    destravado: AtomicBool,
    /// As outras máquinas do dono, por `IP:PORTA`, e a última olhada em cada uma.
    maquinas: Mutex<Vec<String>>,
    maquinas_vistas: Mutex<Vec<maquinas::Vista>>,
    /// O motor de trabalho útil, no modo LAB. Separado da mineração: ela
    /// protege a cadeia e rende HYX; ele executa trabalho verificável e rende
    /// Work Score, sem conversão entre os dois.
    ultrax: Arc<Ultrax>,
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

    /// O programa está trancado agora? (cadeado ligado e ainda sem o código).
    fn trancado(&self) -> bool {
        if self.destravado.load(Ordering::Relaxed) {
            return false;
        }
        self.seguranca.lock().is_ok_and(|s| s.ligado() && s.trava)
    }

    /// O texto da carteira gravada, para assinar um envio.
    fn texto_da_carteira(&self) -> Result<String, String> {
        let arquivo = self.ambiente.arquivo_carteira.as_ref().ok_or("este painel não guarda carteira")?;
        crate::ler_arquivo(arquivo)
    }

    /// Pausa entre tentativas para a mineração ocupar só a fatia escolhida da
    /// CPU. Com 50%, a linha trabalha um tempo e descansa o mesmo tempo. A base
    /// é o tempo medido de uma tentativa nesta máquina, então o limite vale
    /// igual num Atom e num PC de jogo.
    fn pausa_do_limite(&self) -> Duration {
        let uso = f64::from(self.uso_cpu.load(Ordering::Relaxed).clamp(10, 100));
        if uso >= 100.0 {
            return Duration::ZERO;
        }
        let ms = self.ms_tentativa.lock().map(|m| *m).unwrap_or(0.0);
        if ms <= 0.0 {
            // Ainda não medi nada: começa com uma pausa modesta e corrige depois.
            return Duration::from_millis(u64::from(100 - uso.min(99.0) as u32));
        }
        let descanso = ms * (100.0 / uso - 1.0);
        Duration::from_millis(descanso.clamp(0.0, 5_000.0) as u64)
    }

    /// Guarda quanto durou, em média, uma tentativa nesta rodada.
    fn medir_tentativa(&self, tentativas: u64, duracao: Duration, linhas: u32) {
        if tentativas == 0 {
            return;
        }
        let ms = duracao.as_secs_f64() * 1000.0 * f64::from(linhas.max(1)) / tentativas as f64;
        // Desconta a pausa que eu mesmo impus: quero o tempo puro de cálculo.
        let ms = (ms - self.pausa_do_limite().as_secs_f64() * 1000.0).max(1.0);
        if let Ok(mut m) = self.ms_tentativa.lock() {
            // Média que esquece devagar: um pico não estraga o limite.
            *m = if *m <= 0.0 { ms } else { *m * 0.7 + ms * 0.3 };
        }
    }

    /// Watts estimados agora e o custo por mês, em centavos.
    fn energia(&self) -> (f64, f64) {
        if !self.minerando.load(Ordering::Relaxed) {
            return (0.0, 0.0);
        }
        let linhas = f64::from(self.linhas.load(Ordering::Relaxed));
        let uso = f64::from(self.uso_cpu.load(Ordering::Relaxed).clamp(10, 100)) / 100.0;
        let watts = linhas * f64::from(self.watts_nucleo.load(Ordering::Relaxed)) * uso;
        let kwh_mes = watts * 24.0 * 30.0 / 1000.0;
        (watts, kwh_mes * f64::from(self.centavos_kwh.load(Ordering::Relaxed)))
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
            "# Ajustes do Hyurax. Pode apagar: volta tudo ao padrão.\n",
        );
        let _ = writeln!(texto, "minerar={}", u8::from(self.minerando.load(Ordering::Relaxed)));
        let _ = writeln!(texto, "linhas={}", self.linhas.load(Ordering::Relaxed));
        let _ = writeln!(texto, "uso_cpu={}", self.uso_cpu.load(Ordering::Relaxed));
        let _ = writeln!(texto, "watts_nucleo={}", self.watts_nucleo.load(Ordering::Relaxed));
        let _ = writeln!(texto, "centavos_kwh={}", self.centavos_kwh.load(Ordering::Relaxed));
        let _ = writeln!(texto, "na_rede={}", u8::from(self.na_rede.load(Ordering::Relaxed)));
        let _ = writeln!(texto, "mercado={}", u8::from(self.mercado_ligado.load(Ordering::Relaxed)));
        if let Ok(p) = self.paineis.lock() {
            let _ = writeln!(texto, "paineis={p}");
        }
        let _ = writeln!(texto, "avisar_bloco={}", u8::from(self.avisar_bloco.load(Ordering::Relaxed)));
        let _ = writeln!(texto, "som_bloco={}", u8::from(self.som_bloco.load(Ordering::Relaxed)));
        let u = &self.ultrax;
        let _ = writeln!(texto, "ultrax={}", u8::from(u.ligado.load(Ordering::Relaxed)));
        let _ = writeln!(texto, "ultrax_linhas={}", u.linhas.load(Ordering::Relaxed));
        let _ = writeln!(texto, "ultrax_uso_cpu={}", u.uso_cpu.load(Ordering::Relaxed));
        let _ = writeln!(texto, "ultrax_memoria_mib={}", u.memoria_mib.load(Ordering::Relaxed));
        let _ = writeln!(texto, "ultrax_debug={}", u8::from(u.debug.load(Ordering::Relaxed)));
        let _ = writeln!(texto, "ultrax_gpu={}", u8::from(u.gpu_ligada.load(Ordering::Relaxed)));
        let _ = writeln!(texto, "ultrax_gpu_uso={}", u.gpu_uso.load(Ordering::Relaxed));
        if let Ok(s) = self.sementes.lock() {
            for semente in s.iter() {
                let _ = writeln!(texto, "semente={semente}");
            }
        }
        if let Ok(m) = self.maquinas.lock() {
            for maquina in m.iter() {
                let _ = writeln!(texto, "maquina={maquina}");
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
    maquinas: Vec<String>,
    uso_cpu: Option<u32>,
    mercado: Option<bool>,
    paineis: Option<String>,
    watts_nucleo: Option<u32>,
    centavos_kwh: Option<u32>,
    na_rede: Option<bool>,
    avisar_bloco: Option<bool>,
    som_bloco: Option<bool>,
    ultrax: bool,
    ultrax_linhas: Option<u32>,
    ultrax_uso_cpu: Option<u32>,
    ultrax_memoria_mib: Option<u32>,
    ultrax_debug: bool,
    ultrax_gpu: bool,
    ultrax_gpu_uso: Option<u32>,
}

fn ler_ajustes(dados: &Path) -> Ajustes {
    let mut a = Ajustes::default();
    let Ok(texto) = std::fs::read_to_string(dados.join("ajustes.txt")) else { return a };
    for linha in texto.lines() {
        match linha.trim().split_once('=') {
            Some(("minerar", v)) => a.minerar = v.trim() == "1",
            Some(("linhas", v)) => a.linhas = v.trim().parse().ok().filter(|n| *n >= 1),
            Some(("uso_cpu", v)) => a.uso_cpu = v.trim().parse().ok().filter(|n| (10..=100).contains(n)),
            Some(("watts_nucleo", v)) => a.watts_nucleo = v.trim().parse().ok().filter(|n| (1..=200).contains(n)),
            Some(("centavos_kwh", v)) => a.centavos_kwh = v.trim().parse().ok().filter(|n| (1..=99999).contains(n)),
            Some(("na_rede", v)) => a.na_rede = Some(v.trim() == "1"),
            Some(("mercado", v)) => a.mercado = Some(v.trim() == "1"),
            Some(("paineis", v)) => {
                let lista = migrar_paineis(v.trim());
                if paineis_validos(&lista) {
                    a.paineis = Some(completar_paineis(&lista));
                }
            }
            Some(("avisar_bloco", v)) => a.avisar_bloco = Some(v.trim() == "1"),
            Some(("som_bloco", v)) => a.som_bloco = Some(v.trim() == "1"),
            Some(("ultrax", v)) => a.ultrax = v.trim() == "1",
            Some(("ultrax_linhas", v)) => a.ultrax_linhas = v.trim().parse().ok().filter(|n| *n >= 1),
            Some(("ultrax_uso_cpu", v)) => a.ultrax_uso_cpu = v.trim().parse().ok().filter(|n| (10..=100).contains(n)),
            Some(("ultrax_memoria_mib", v)) => {
                a.ultrax_memoria_mib =
                    v.trim().parse().ok().filter(|n| (ultrax::MEMORIA_MIN_MIB..=ultrax::MEMORIA_MAX_MIB).contains(n));
            }
            Some(("ultrax_debug", v)) => a.ultrax_debug = v.trim() == "1",
            Some(("ultrax_gpu", v)) => a.ultrax_gpu = v.trim() == "1",
            Some(("ultrax_gpu_uso", v)) => a.ultrax_gpu_uso = v.trim().parse().ok().filter(|n| (10..=100).contains(n)),
            Some(("semente", v)) if semente_valida(v.trim()) && a.sementes.len() < SEMENTES_MAX => {
                a.sementes.push(v.trim().to_string());
            }
            Some(("maquina", v)) if semente_valida(v.trim()) && a.maquinas.len() < maquinas::MAXIMO => {
                a.maquinas.push(v.trim().to_string());
            }
            _ => {}
        }
    }
    a
}

/// Lista de nomes separados por vírgula, com "-" na frente do que está
/// desligado. É o jeito que o programa guarda os painéis abertos: a ordem é a
/// que o dono escolheu.
///
/// Só aceita nomes conhecidos, sem repetição.
fn lista_valida(lista: &str, conhecidos: &[&str]) -> bool {
    let mut vistos: Vec<&str> = Vec::new();
    for item in lista.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let nome = item.strip_prefix('-').unwrap_or(item);
        if !conhecidos.contains(&nome) || vistos.contains(&nome) {
            return false;
        }
        vistos.push(nome);
    }
    !vistos.is_empty()
}

/// Completa a lista com o que o programa aprendeu depois de ela ter sido salva.
/// O item novo nasce desligado, mas precisa **existir** na lista: senão ele não
/// aparece no menu, e o dono não teria como ligar.
fn completar(lista: &str, conhecidos: &[&str]) -> String {
    let mut saida: Vec<String> = lista.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    for nome in conhecidos {
        if !saida.iter().any(|item| item.trim_start_matches('-') == *nome) {
            saida.push(format!("-{nome}"));
        }
    }
    saida.join(",")
}

fn paineis_validos(lista: &str) -> bool {
    lista_valida(lista, &PAINEIS_CONHECIDOS)
}

fn completar_paineis(lista: &str) -> String {
    completar(lista, &PAINEIS_CONHECIDOS)
}

/// A estação 3D saiu em 26/09/2026, e o ULTRAX entrou no lugar dela. Quem
/// tinha a lista salva ganha os painéis novos onde a estação estava, abertos
/// ou fechados como ela.
fn migrar_paineis(lista: &str) -> String {
    lista
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|item| match item {
            "estacao" => "ultrax,ia,verificacao,mineracao,historico,telemetria".to_string(),
            "-estacao" => "-ultrax,-ia,-verificacao,mineracao,-historico,-telemetria".to_string(),
            outro => outro.to_string(),
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// `host:porta`: a mesma regra que o nó usa para a lista de sementes, para o
/// painel nunca aceitar um endereço que o nó recusaria.
fn semente_valida(s: &str) -> bool {
    crate::sementes::valida(s)
}

/// O endereço deste computador na rede local, para abrir o painel no celular.
///
/// Não manda pacote nenhum: só pergunta ao sistema por qual placa ele sairia
/// para fora, e lê o endereço dessa placa.
fn ip_local() -> Option<Ipv4Addr> {
    let s = std::net::UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("8.8.8.8", 53)).ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
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
    /// Já há um Hyurax respondendo nesta porta.
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
    let partida = Partida {
        ambiente,
        endereco,
        sementes: ajustes.sementes.clone(),
        porta_painel: PORTA_PAINEL,
        na_rede: false,
    };
    let pronto = ligar(o, rede, partida, &ajustes)?;
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
    let partida = Partida { ambiente, endereco: Some(endereco), sementes, porta_painel, na_rede };
    let pronto = ligar(o, rede, partida, &Ajustes::default())?;
    println!("Painel: {}", pronto.url());
    if na_rede {
        println!("  (visível na rede local; comandos só deste computador)");
    }
    vigiar(&pronto.painel, &pronto.rede, &pronto.o)
}

/// Monta o painel, abre a porta local e põe o servidor e o minerador para rodar.
/// O que o motor precisa saber para subir, junto num lugar só.
struct Partida {
    ambiente: Ambiente,
    endereco: Option<[u8; ADDRESS_LEN]>,
    sementes: Vec<String>,
    porta_painel: u16,
    na_rede: bool,
}

fn ligar(o: Opcoes, rede: Arc<Rede>, partida: Partida, ajustes: &Ajustes) -> Result<Pronto, String> {
    let Partida { ambiente, endereco, sementes, porta_painel, na_rede } = partida;
    let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
    let app = ambiente.app;
    let seguranca = Seguranca::ler(&ambiente.dados);
    let trava = seguranca.ligado() && seguranca.trava;
    // O ULTRAX assina com uma chave derivada da identidade do nó. Os avisos
    // dele chegam ao fluxo de eventos por um canal, porque o painel ainda não
    // existe na hora de abrir o worker.
    let identidade = crate::identidade_do_no(&o)?;
    let (avisos, avisos_rx) = std::sync::mpsc::channel::<(&'static str, String)>();
    let ultrax = Ultrax::abrir(
        &ambiente.dados,
        identidade.segredo(),
        nucleos,
        &ultrax::Partida {
            ligado: ajustes.ultrax,
            linhas: ajustes.ultrax_linhas.unwrap_or(1),
            uso_cpu: ajustes.ultrax_uso_cpu.unwrap_or(100),
            memoria_mib: ajustes.ultrax_memoria_mib.unwrap_or(ultrax::MEMORIA_PADRAO_MIB),
            debug: ajustes.ultrax_debug,
            gpu: ajustes.ultrax_gpu,
            gpu_uso: ajustes.ultrax_gpu_uso.unwrap_or(50),
        },
        Box::new(move |tipo, texto| {
            let _ = avisos.send((tipo, texto));
        }),
    );
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
        uso_cpu: AtomicU32::new(ajustes.uso_cpu.unwrap_or(100)),
        ms_tentativa: Mutex::new(0.0),
        watts_nucleo: AtomicU32::new(ajustes.watts_nucleo.unwrap_or(WATTS_NUCLEO_PADRAO)),
        centavos_kwh: AtomicU32::new(ajustes.centavos_kwh.unwrap_or(CENTAVOS_KWH_PADRAO)),
        na_rede: AtomicBool::new(ajustes.na_rede.unwrap_or(na_rede)),
        mercado_ligado: AtomicBool::new(ajustes.mercado.unwrap_or(false)),
        mercado: Mutex::new(None),
        paineis: Mutex::new(ajustes.paineis.clone().unwrap_or_else(|| PAINEIS_PADRAO.to_string())),
        avisar_bloco: AtomicBool::new(ajustes.avisar_bloco.unwrap_or(true)),
        som_bloco: AtomicBool::new(ajustes.som_bloco.unwrap_or(false)),
        seguranca: Mutex::new(seguranca),
        totp_pendente: Mutex::new(None),
        // Trancado começa fechado; sem cadeado, já nasce aberto.
        destravado: AtomicBool::new(!trava),
        maquinas: Mutex::new(ajustes.maquinas.clone()),
        maquinas_vistas: Mutex::new(Vec::new()),
        ultrax,
    });
    {
        let painel = Arc::clone(&painel);
        std::thread::spawn(move || {
            for (tipo, texto) in avisos_rx {
                painel.registrar(tipo, texto);
            }
        });
    }
    painel.registrar("no", format!("nó no ar na rede {}", o.rede.nome));
    if app && endereco.is_none() {
        painel.registrar("carteira", "nenhuma carteira ainda: crie ou importe uma para minerar".into());
    }

    // No programa com janela a porta sempre abre para a rede, e quem decide se
    // responde a outro aparelho é o ajuste "ver no celular": assim o usuário
    // liga e desliga na hora, sem reiniciar o programa. Com o ajuste desligado,
    // qualquer pedido de fora leva 403.
    let ip = if na_rede || app { IpAddr::V4(Ipv4Addr::UNSPECIFIED) } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
    let ouvinte = TcpListener::bind(SocketAddr::new(ip, porta_painel))
        .map_err(|e| format!("não consegui abrir o painel na porta {porta_painel}: {e}"))?;
    // O worker só começa depois de o painel ter onde aparecer: sem a porta, ele
    // rodaria escondido, sem botão para desligar.
    painel.ultrax.iniciar();
    let o = Arc::new(o);
    {
        let (painel, rede, o) = (Arc::clone(&painel), Arc::clone(&rede), Arc::clone(&o));
        std::thread::spawn(move || servir(ouvinte, porta_painel, &painel, &rede, &o));
    }
    {
        let (painel, rede, o) = (Arc::clone(&painel), Arc::clone(&rede), Arc::clone(&o));
        std::thread::spawn(move || minerador(&painel, &rede, &o));
    }
    {
        let painel = Arc::clone(&painel);
        std::thread::spawn(move || vigia_do_mercado(&painel));
    }
    {
        let painel = Arc::clone(&painel);
        std::thread::spawn(move || vigia_das_maquinas(&painel));
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
    let linhas = painel.linhas.load(Ordering::Relaxed).max(1);
    // A pausa entre tentativas é o limitador de CPU: sem ela a mineração come a
    // máquina inteira, que foi a reclamação de quem testou.
    let pausa = if o.pausa_ms > 0 { Duration::from_millis(o.pausa_ms) } else { painel.pausa_do_limite() };
    let config = ConfigMineracao {
        linhas,
        nonce_inicial: agora_unix().wrapping_mul(0x9E37_79B9),
        limite: None,
        pausa,
    };
    let tentativas_antes = painel.tentativas.load(Ordering::Relaxed);
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

    painel.medir_tentativa(
        painel.tentativas.load(Ordering::Relaxed).saturating_sub(tentativas_antes),
        inicio.elapsed(),
        linhas,
    );
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
                    "bloco {altura} minerado em {:.1} s · trabalho útil {n}×{n} · +{} HYX",
                    inicio.elapsed().as_secs_f64(),
                    hyx(u128::from(block_reward(altura, &o.rede)))
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

/// Perfis prontos: quanto da máquina a mineração pode tomar.
///
/// "Leve" deixa o computador livre para trabalhar; "equilibrado" usa metade;
/// "turbo" usa tudo e o computador fica lento. Os números saem dos núcleos que
/// a máquina tem, então o mesmo perfil vale num Atom e num PC de jogo.
fn perfil_para(nucleos: u32, perfil: &str) -> Option<(u32, u32)> {
    let n = nucleos.max(1);
    match perfil {
        "leve" => Some(((n / 4).max(1), 35)),
        "equilibrado" => Some(((n / 2).max(1), 70)),
        "turbo" => Some((n, 100)),
        _ => None,
    }
}

fn trocar_ajustes(painel: &Painel, campos: &[(String, String)]) -> Result<(), String> {
    let mut mudou_mineracao = false;
    for (nome, valor) in campos {
        match nome.as_str() {
            "perfil" => {
                let (linhas, uso) = perfil_para(painel.nucleos, valor).ok_or("perfil desconhecido")?;
                painel.linhas.store(linhas, Ordering::Relaxed);
                painel.uso_cpu.store(uso, Ordering::Relaxed);
                mudou_mineracao = true;
                painel.registrar("minerador", format!("perfil {valor}: {linhas} núcleo(s), até {uso}% da CPU"));
            }
            "uso_cpu" => {
                let n: u32 = valor.parse().map_err(|_| "uso da CPU inválido")?;
                painel.uso_cpu.store(n.clamp(10, 100), Ordering::Relaxed);
                mudou_mineracao = true;
            }
            "linhas" => {
                let n: u32 = valor.parse().map_err(|_| "número de núcleos inválido")?;
                painel.linhas.store(n.clamp(1, painel.nucleos.max(1)), Ordering::Relaxed);
                mudou_mineracao = true;
            }
            "watts_nucleo" => {
                let n: u32 = valor.parse().map_err(|_| "watts por núcleo inválido")?;
                painel.watts_nucleo.store(n.clamp(1, 200), Ordering::Relaxed);
            }
            "centavos_kwh" => {
                let n: u32 = valor.parse().map_err(|_| "preço do kWh inválido")?;
                painel.centavos_kwh.store(n.clamp(1, 99_999), Ordering::Relaxed);
            }
            "na_rede" => {
                let ligado = valor == "1";
                painel.na_rede.store(ligado, Ordering::Relaxed);
                painel.registrar(
                    "painel",
                    if ligado { "painel visível na rede local (só leitura)".into() } else { "painel só neste computador".to_string() },
                );
            }
            "mercado" => {
                let ligado = valor == "1";
                painel.mercado_ligado.store(ligado, Ordering::Relaxed);
                painel.registrar(
                    "mercado",
                    if ligado {
                        "acompanhando o mercado (fala com api.coingecko.com a cada 2 min)".into()
                    } else {
                        "mercado desligado: nada sai deste computador".to_string()
                    },
                );
            }
            "paineis" => {
                if !paineis_validos(valor) {
                    return Err("lista de painéis inválida".into());
                }
                if let Ok(mut p) = painel.paineis.lock() {
                    *p = completar_paineis(valor);
                }
            }
            "avisar_bloco" => painel.avisar_bloco.store(valor == "1", Ordering::Relaxed),
            "som_bloco" => painel.som_bloco.store(valor == "1", Ordering::Relaxed),
            _ => {}
        }
    }
    if mudou_mineracao {
        // Vale na próxima rodada: interrompe esta para o limite valer já.
        painel.interromper.store(true, Ordering::Relaxed);
    }
    painel.gravar_ajustes();
    Ok(())
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
// Enviar HYX pela janela
// ---------------------------------------------------------------------------

/// Assina e manda uma transferência pedida pelo painel.
///
/// A ordem das conferências é de propósito: o que é de graça primeiro (formato,
/// segundo fator), o que custa depois (abrir a carteira, falar com a rede).
fn enviar_do_painel(painel: &Painel, rede: &Arc<Rede>, o: &Opcoes, campos: &[(String, String)]) -> Result<String, String> {
    let de = painel.endereco().ok_or("ainda não há carteira neste computador")?;
    let pedido = envio::conferir(
        &de,
        &campo(campos, "para").unwrap_or_default(),
        &campo(campos, "valor").unwrap_or_default(),
        &campo(campos, "taxa").unwrap_or_default(),
        o.rede.nome,
    )?;
    let senha = campo(campos, "senha").unwrap_or_default();
    if senha.is_empty() {
        return Err("digite a senha da carteira para assinar o envio.".into());
    }
    let exige = painel.seguranca.lock().is_ok_and(|s| s.ligado() && s.exige_envio);
    if exige {
        let codigo = campo(campos, "codigo").unwrap_or_default();
        let vale = painel.seguranca.lock().is_ok_and(|s| s.confere(agora_unix(), &codigo));
        if !vale {
            return Err("código de 6 dígitos errado ou vencido. Olhe o aplicativo de novo.".into());
        }
    }
    let texto = painel.texto_da_carteira()?;
    let feita = envio::enviar(rede, &o.rede.magic, o.rede.coinbase_maturity, &texto, &senha, &pedido)?;
    painel.registrar(
        "enviado",
        format!(
            "enviados {} HYX para {} (taxa {}, nonce {}, {} par(es))",
            hyx(u128::from(pedido.valor)),
            crate::endereco::mostrar(&pedido.para, o.rede.nome),
            hyx(u128::from(pedido.taxa)),
            feita.nonce,
            feita.pares
        ),
    );
    Ok(format!(
        "{{\"txid\":\"{}\",\"valor\":\"{}\",\"taxa\":\"{}\",\"para\":\"{}\",\"nonce\":{},\"pares\":{}}}",
        hex(&feita.txid),
        hyx(u128::from(pedido.valor)),
        hyx(u128::from(pedido.taxa)),
        crate::endereco::mostrar(&pedido.para, o.rede.nome),
        feita.nonce,
        feita.pares
    ))
}

// ---------------------------------------------------------------------------
// Segundo fator (código de 6 dígitos)
// ---------------------------------------------------------------------------

/// Começa a ligar o segundo fator: sorteia um segredo e mostra o QR Code.
///
/// Só vale depois que o dono digitar um código certo: enquanto isso o segredo
/// fica na memória, e nada é gravado. Assim ninguém fica trancado para fora por
/// ter fechado a janela no meio.
fn seguranca_comecar(painel: &Painel) -> Result<String, String> {
    if painel.seguranca.lock().is_ok_and(|s| s.ligado()) {
        return Err("o segundo fator já está ligado neste computador".into());
    }
    // Pelo gerador do nó, e não lendo o sistema de novo: no Windows aquela
    // leitura sobe um PowerShell e demora segundos. O gerador já nasceu da
    // entropia do sistema, uma vez, quando o programa abriu.
    let mut segredo = vec![0u8; totp::SEGREDO_LEN];
    hyurax_net::entropia::preencher(&mut segredo)?;
    let conta = painel.endereco().map(|e| hex(&e).chars().take(10).collect::<String>()).unwrap_or_default();
    let resposta = format!(
        "{{\"segredo\":\"{}\",\"uri\":{}}}",
        totp::base32(&segredo),
        texto_json(&totp::uri(&segredo, &conta))
    );
    if let Ok(mut p) = painel.totp_pendente.lock() {
        *p = Some(segredo);
    }
    Ok(resposta)
}

/// Confirma o segredo com o primeiro código certo e grava o arquivo.
fn seguranca_confirmar(painel: &Painel, campos: &[(String, String)]) -> Result<String, String> {
    let segredo = painel
        .totp_pendente
        .lock()
        .ok()
        .and_then(|p| p.clone())
        .ok_or("comece de novo: o segredo desta tela já não vale")?;
    let digitado = campo(campos, "codigo").unwrap_or_default();
    if !totp::confere(&segredo, agora_unix(), &digitado) {
        return Err("código errado. Confira a hora do celular e digite o código que está na tela agora.".into());
    }
    let nova = Seguranca { segredo: Some(segredo), exige_envio: true, trava: campo(campos, "trava").as_deref() == Some("1") };
    nova.gravar(&painel.ambiente.dados)?;
    if let Ok(mut s) = painel.seguranca.lock() {
        *s = nova;
    }
    if let Ok(mut p) = painel.totp_pendente.lock() {
        *p = None;
    }
    painel.destravado.store(true, Ordering::Relaxed);
    painel.registrar("seguranca", "segundo fator ligado: enviar HYX agora pede o código de 6 dígitos".into());
    Ok("{\"ok\":true}".to_string())
}

/// Muda o que o segundo fator protege, ou desliga tudo. Sempre com um código
/// certo na mão: quem não tem o celular não mexe no cadeado.
fn seguranca_mudar(painel: &Painel, campos: &[(String, String)]) -> Result<String, String> {
    let digitado = campo(campos, "codigo").unwrap_or_default();
    let mut guarda = painel.seguranca.lock().map_err(|_| "segurança travada".to_string())?;
    if !guarda.ligado() {
        return Err("o segundo fator não está ligado".into());
    }
    if !guarda.confere(agora_unix(), &digitado) {
        return Err("código de 6 dígitos errado ou vencido.".into());
    }
    let nova = if campo(campos, "desligar").as_deref() == Some("1") {
        Seguranca::default()
    } else {
        Seguranca {
            segredo: guarda.segredo.clone(),
            exige_envio: campo(campos, "exige_envio").as_deref().unwrap_or("1") == "1",
            trava: campo(campos, "trava").as_deref() == Some("1"),
        }
    };
    nova.gravar(&painel.ambiente.dados)?;
    let desligou = !nova.ligado();
    *guarda = nova;
    drop(guarda);
    painel.destravado.store(true, Ordering::Relaxed);
    painel.registrar(
        "seguranca",
        if desligou { "segundo fator desligado".into() } else { "segundo fator ajustado".to_string() },
    );
    Ok("{\"ok\":true}".to_string())
}

/// Destranca o programa nesta abertura.
fn destravar(painel: &Painel, campos: &[(String, String)]) -> Result<String, String> {
    let digitado = campo(campos, "codigo").unwrap_or_default();
    if !painel.seguranca.lock().is_ok_and(|s| s.confere(agora_unix(), &digitado)) {
        return Err("código errado. O código muda a cada 30 segundos.".into());
    }
    painel.destravado.store(true, Ordering::Relaxed);
    painel.registrar("seguranca", "programa destrancado".into());
    Ok("{\"ok\":true}".to_string())
}

// ---------------------------------------------------------------------------
// As outras máquinas do dono
// ---------------------------------------------------------------------------

fn trocar_maquinas(painel: &Painel, campos: &[(String, String)]) -> Result<Vec<String>, String> {
    let lista = campo(campos, "lista").unwrap_or_default();
    let novas: Vec<String> = lista
        .split([',', '\n', ' ', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if novas.len() > maquinas::MAXIMO {
        return Err(format!("no máximo {} máquinas", maquinas::MAXIMO));
    }
    if let Some(ruim) = novas.iter().find(|s| !semente_valida(s)) {
        return Err(format!("\"{ruim}\" não é IP:PORTA (exemplo: 192.168.0.12:8800)"));
    }
    if let Ok(mut m) = painel.maquinas.lock() {
        m.clone_from(&novas);
    }
    // A lista velha some na hora: nada de mostrar máquina que já foi tirada.
    if let Ok(mut v) = painel.maquinas_vistas.lock() {
        v.retain(|vista| novas.contains(&vista.alvo));
    }
    painel.gravar_ajustes();
    Ok(novas)
}

/// Pergunta o estado das outras máquinas de tempo em tempo. Só lê.
fn vigia_das_maquinas(painel: &Arc<Painel>) {
    loop {
        let alvos: Vec<String> = painel.maquinas.lock().map(|m| m.clone()).unwrap_or_default();
        if alvos.is_empty() {
            if let Ok(mut v) = painel.maquinas_vistas.lock() {
                v.clear();
            }
            std::thread::sleep(MAQUINAS_INTERVALO);
            continue;
        }
        let vistas: Vec<maquinas::Vista> = alvos.iter().map(|a| maquinas::olhar(a, agora_unix())).collect();
        if let Ok(mut v) = painel.maquinas_vistas.lock() {
            *v = vistas;
        }
        std::thread::sleep(MAQUINAS_INTERVALO);
    }
}

// ---------------------------------------------------------------------------
// Mercado das criptomoedas
// ---------------------------------------------------------------------------

/// Busca os preços das moedas grandes, de tempos em tempos, quando o dono liga.
///
/// Usa o `curl` que já vem no Windows 10 e 11 (e em quase todo Linux) em vez de
/// puxar uma biblioteca de HTTPS: a regra do projeto é só dependência Rust pura,
/// e uma biblioteca de TLS traria compilador C junto.
///
/// Só sai daqui quando o ajuste está ligado, e o painel avisa que isso fala com
/// um site de fora (api.coingecko.com). Nenhum dado do usuário vai junto: o
/// pedido é a lista fixa de moedas e mais nada.
fn vigia_do_mercado(painel: &Arc<Painel>) {
    loop {
        if painel.mercado_ligado.load(Ordering::Relaxed) {
            let velho = painel
                .mercado
                .lock()
                .ok()
                .and_then(|m| m.as_ref().map(|(q, _)| *q))
                .unwrap_or(0);
            if agora_unix().saturating_sub(velho) >= MERCADO_INTERVALO.as_secs() {
                match buscar_mercado() {
                    Ok(json) => {
                        if let Ok(mut m) = painel.mercado.lock() {
                            *m = Some((agora_unix(), json));
                        }
                    }
                    Err(e) => painel.registrar("mercado", format!("não consegui ler os preços: {e}")),
                }
            }
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn buscar_mercado() -> Result<String, String> {
    let ids: Vec<&str> = MERCADO_MOEDAS.iter().map(|(id, _)| *id).collect();
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=brl,usd&include_24hr_change=true",
        ids.join(",")
    );
    let saida = std::process::Command::new("curl")
        .args(["-s", "-S", "--max-time", "20", "--max-filesize", "65536", &url])
        .output()
        .map_err(|e| format!("curl não abriu: {e}"))?;
    if !saida.status.success() {
        return Err(String::from_utf8_lossy(&saida.stderr).trim().to_string());
    }
    let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
    if !texto.starts_with('{') || texto.len() > 65_536 {
        return Err("resposta estranha".into());
    }
    Ok(monta_mercado(&texto))
}

/// Tira os números da resposta e monta o JSON que o painel entende.
///
/// Leitura na unha, sem biblioteca: o formato é conhecido e minúsculo, e um
/// campo que falte simplesmente não aparece no painel.
fn monta_mercado(bruto: &str) -> String {
    let mut itens: Vec<String> = Vec::new();
    for (id, nome) in MERCADO_MOEDAS {
        let Some(pedaco) = bruto.split(&format!("\"{id}\":{{")).nth(1).and_then(|p| p.split('}').next()) else {
            continue;
        };
        let numero = |campo: &str| -> Option<f64> {
            pedaco
                .split(&format!("\"{campo}\":"))
                .nth(1)?
                .split(',')
                .next()?
                .trim()
                .parse()
                .ok()
        };
        let (Some(brl), Some(usd)) = (numero("brl"), numero("usd")) else { continue };
        let variacao = numero("brl_24h_change").unwrap_or(0.0);
        itens.push(format!(
            "{{\"id\":\"{id}\",\"nome\":\"{nome}\",\"brl\":{brl:.2},\"usd\":{usd:.2},\"variacao\":{variacao:.2}}}"
        ));
    }
    format!("[{}]", itens.join(","))
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
    /// O corpo em bytes, para quem manda binário (o resultado da GPU).
    bruto: Vec<u8>,
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
            // o resultado da GPU é uma matriz inteira; o resto do painel é formulário pequeno
            "content-length" => {
                let maximo = if caminho.starts_with("/api/ultrax/gpu/resultado/") { ultrax::GPU_RESULTADO_MAX } else { 8192 };
                tamanho = valor.parse().ok().filter(|t| *t <= maximo)?;
            }
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
    let texto = if caminho.starts_with("/api/ultrax/gpu/resultado/") { String::new() } else { String::from_utf8_lossy(&corpo).into_owned() };
    Some(Pedido { metodo, caminho, host, origem, corpo: texto, bruto: corpo })
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
    // Uma escrita só, e sem o algoritmo de Nagle: em duas escritas, o Windows
    // segurava o corpo esperando o ACK atrasado do cabeçalho, e cada resposta
    // do painel levava de 200 a 400 ms à toa.
    let _ = s.set_nodelay(true);
    let mut tudo = Vec::with_capacity(cab.len().saturating_add(corpo.len()));
    tudo.extend_from_slice(cab.as_bytes());
    tudo.extend_from_slice(corpo);
    s.write_all(&tudo)?;
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
    // Pedido de outro aparelho só passa com "ver no celular" ligado, e mesmo
    // assim é só leitura: comando continua exigindo ser deste computador.
    if !local && !painel.na_rede.load(Ordering::Relaxed) {
        return responder(&mut s, "403 Forbidden", "text/plain", b"ligue 'ver no celular' nos ajustes");
    }
    if p.metodo == "GET" {
        return match rota {
            "/" => responder(&mut s, "200 OK", "text/html; charset=utf-8", INDEX.as_bytes()),
            "/painel.css" => responder(&mut s, "200 OK", "text/css; charset=utf-8", CSS.as_bytes()),
            "/painel.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", JS.as_bytes()),
            "/moleculas.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", MOLECULAS.as_bytes()),
            "/gpu.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", GPU.as_bytes()),
            "/gpu-trabalhador.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", GPU_TRABALHADOR.as_bytes()),
            "/qrcode.min.js" => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", QRCODE.as_bytes()),
            "/api/estado" => {
                let json = estado_json(painel, rede, o, local && host_local, porta);
                responder(&mut s, "200 OK", "application/json; charset=utf-8", json.as_bytes())
            }
            // As matrizes da tarefa da GPU: só para a janela deste computador.
            r if r.starts_with("/api/ultrax/gpu/entrada/") => {
                let numero = r.trim_start_matches("/api/ultrax/gpu/entrada/").parse::<u32>().ok();
                match numero.filter(|_| local && host_local).and_then(|n| painel.ultrax.gpu_entrada(n)) {
                    Some(bytes) => responder(&mut s, "200 OK", "application/octet-stream", &bytes),
                    None => responder(&mut s, "404 Not Found", "text/plain", b"tarefa de GPU desconhecida"),
                }
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
    // Trancado: o único comando que passa é o que destranca.
    if painel.trancado() && rota != "/api/destravar" {
        return responder(&mut s, "403 Forbidden", "text/plain", b"programa trancado");
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
        "/api/enviar" => responder_json(&mut s, enviar_do_painel(painel, rede, o, &campos)),
        "/api/seguranca/comecar" => responder_json(&mut s, seguranca_comecar(painel)),
        "/api/seguranca/confirmar" => responder_json(&mut s, seguranca_confirmar(painel, &campos)),
        "/api/seguranca/mudar" => responder_json(&mut s, seguranca_mudar(painel, &campos)),
        "/api/destravar" => responder_json(&mut s, destravar(painel, &campos)),
        "/api/maquinas" => {
            let r = trocar_maquinas(painel, &campos).map(|lista| {
                let itens: Vec<String> = lista.iter().map(|x| texto_json(x)).collect();
                format!("{{\"maquinas\":[{}]}}", itens.join(","))
            });
            responder_json(&mut s, r)
        }
        "/api/sementes" if app => {
            let r = trocar_sementes(painel, rede, &campos).map(|lista| {
                let itens: Vec<String> = lista.iter().map(|x| texto_json(x)).collect();
                format!("{{\"sementes\":[{}]}}", itens.join(","))
            });
            responder_json(&mut s, r)
        }
        "/api/ajustes" if app => {
            let r = trocar_ajustes(painel, &campos).map(|()| "{\"ok\":true}".to_string());
            responder_json(&mut s, r)
        }
        "/api/ultrax/gpu/pegar" => {
            let nome = campo(&campos, "nome").unwrap_or_default();
            let r = painel.ultrax.gpu_pegar(&nome).map(|(numero, n)| format!("{{\"numero\":{numero},\"n\":{n}}}"));
            responder_json(&mut s, r)
        }
        r if r.starts_with("/api/ultrax/gpu/progresso/") => {
            if let (Ok(numero), Some(linhas)) = (
                r.trim_start_matches("/api/ultrax/gpu/progresso/").parse::<u32>(),
                campo(&campos, "linhas").and_then(|v| v.parse::<u64>().ok()),
            ) {
                painel.ultrax.gpu_progresso(numero, linhas);
            }
            responder(&mut s, "204 No Content", "text/plain", b"")
        }
        r if r.starts_with("/api/ultrax/gpu/cancelar/") => {
            if let Ok(numero) = r.trim_start_matches("/api/ultrax/gpu/cancelar/").parse::<u32>() {
                let motivo = campo(&campos, "motivo").unwrap_or_else(|| "a janela desistiu da tarefa".into());
                let motivo: String = motivo.chars().filter(|c| !c.is_control()).take(160).collect();
                painel.ultrax.gpu_cancelar(numero, &motivo);
            }
            responder(&mut s, "204 No Content", "text/plain", b"")
        }
        r if r.starts_with("/api/ultrax/gpu/resultado/") => {
            let r = r
                .trim_start_matches("/api/ultrax/gpu/resultado/")
                .parse::<u32>()
                .map_err(|_| "número de tarefa inválido".to_string())
                .and_then(|numero| painel.ultrax.gpu_resultado(numero, &p.bruto))
                .map(|()| "{\"ok\":true}".to_string());
            responder_json(&mut s, r)
        }
        "/api/ultrax" => {
            let numero = |nome: &str| campo(&campos, nome).and_then(|v| v.parse::<u32>().ok());
            let sim = |nome: &str| campo(&campos, nome).map(|v| v == "1");
            painel.ultrax.ajustar_tudo(
                numero("linhas"),
                numero("uso_cpu"),
                numero("memoria_mib"),
                sim("debug"),
                sim("gpu"),
                numero("gpu_uso"),
            );
            if let Some(ligar) = sim("ligar") {
                painel.ultrax.ligar(ligar);
            }
            painel.gravar_ajustes();
            responder(&mut s, "204 No Content", "text/plain", b"")
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

pub(crate) fn texto_json(s: &str) -> String {
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

fn estado_json(painel: &Painel, rede: &Rede, o: &Opcoes, pode_mandar: bool, porta: u16) -> String {
    // Trancado: a página só recebe o suficiente para desenhar o cadeado. Nem
    // saldo, nem endereço, nem o livro de blocos saem daqui antes do código.
    if painel.trancado() {
        return format!(
            "{{\"trancado\":true,\"rede\":{},\"versao\":\"{VERSAO}\",\"app\":{},\"pode_mandar\":{pode_mandar},\"agora\":{}}}",
            texto_json(o.rede.nome),
            painel.ambiente.app,
            agora_unix()
        );
    }
    let mut j = String::with_capacity(8 * 1024);
    let endereco = painel.endereco();
    let Ok(no) = rede.no.lock() else {
        return "{\"erro\":\"no travado\"}".into();
    };
    let c = &no.chain;
    let altura = c.height();
    let (saldo, imaturo) = endereco.map_or((0, 0), |e| (c.state.balance(&e, &HYX), c.state.immature_balance(&e)));
    let rodada = painel.rodada.lock().ok().and_then(|r| *r);
    let e_meu = |b: &Block| {
        endereco.is_some_and(|e| matches!(b.transactions.first(), Some(Tx::Coinbase(cb)) if cb.recipient == e))
    };
    // Blocos meus na cadeia inteira, não só nesta sessão.
    let meus_na_cadeia = c.entries.iter().filter(|e| e_meu(&e.block)).count();
    // O trabalho útil do último bloco: o lado das matrizes que a rede está pedindo.
    let trabalho_n = c
        .tip()
        .and_then(|b| b.useful_proof.as_ref().map(|p| p.n))
        .unwrap_or(o.rede.uteis.useful_size_base);
    let (watts, custo_centavos) = painel.energia();
    let (mercado_quando, mercado_json) = painel
        .mercado
        .lock()
        .ok()
        .and_then(|m| m.clone())
        .unwrap_or_else(|| (0, "[]".to_string()));
    let centavos = custo_centavos as u64;
    let custo_reais = format!("{}.{:02}", centavos / 100, centavos % 100);
    // Endereço para abrir o painel no celular, só quando o dono ligou isso.
    let url_celular = if painel.na_rede.load(Ordering::Relaxed) {
        ip_local().map(|ip| format!("http://{ip}:{porta}/")).unwrap_or_default()
    } else {
        String::new()
    };
    let sementes: Vec<String> = painel.sementes.lock().map(|s| s.iter().map(|x| texto_json(x)).collect()).unwrap_or_default();
    let _ = write!(
        j,
        "{{\"rede\":{},\"altura\":{altura},\"ponta\":\"{}\",\"trabalho\":\"{}\",\"emitido\":\"{}\",\
         \"pares\":{},\"mempool\":{},\"endereco\":\"{}\",\"saldo\":\"{}\",\"imaturo\":\"{}\",\
         \"recompensa\":\"{}\",\"maturidade\":{},\"minerando\":{},\"linhas\":{},\"nucleos\":{},\
         \"memoria_mib\":{},\"tentativas\":{},\"ritmo\":{:.3},\"meus\":{},\"meus_cadeia\":{meus_na_cadeia},\"perdidos\":{},\
         \"ligado_s\":{},\"rodada_s\":{},\"rodada_altura\":{},\"pode_mandar\":{pode_mandar},\"agora\":{},\
         \"app\":{},\"versao\":\"{VERSAO}\",\"carteira\":{},\"pode_minerar\":{},\"dados\":{},\"sementes\":[{}],\"porta_p2p\":{},\
         \"uso_cpu\":{},\"ms_tentativa\":{:.1},\"watts\":{:.1},\"watts_nucleo\":{},\"centavos_kwh\":{},\"custo_mes\":\"{}\",\
         \"memoria_total_mib\":{},\"na_rede\":{},\"url_celular\":{},\
         \"paineis\":{},\"mercado_ligado\":{},\"mercado_quando\":{},\"mercado\":{},         \"trabalho_util\":{{\"familia\":\"matrizes\",\"n\":{},\"rodadas\":{},\"lado_min\":{},\"lado_base\":{},\"lado_max\":{},\"bytes\":{}}},",
        texto_json(o.rede.nome),
        hex(&c.tip_hash()),
        c.total_work().to_decimal(),
        hyx(u128::from(c.state.total_emitted)),
        rede.pares_conectados(),
        no.mempool_len(),
        endereco.map(|e| crate::endereco::mostrar(&e, o.rede.nome)).unwrap_or_default(),
        hyx(u128::from(saldo)),
        hyx(imaturo),
        hyx(u128::from(block_reward(altura.saturating_add(1), &o.rede))),
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
        painel.uso_cpu.load(Ordering::Relaxed),
        painel.ms_tentativa.lock().map(|m| *m).unwrap_or(0.0),
        watts,
        painel.watts_nucleo.load(Ordering::Relaxed),
        painel.centavos_kwh.load(Ordering::Relaxed),
        // Custo do mês em reais, com duas casas: centavos são inteiros até aqui.
        custo_reais,
        u64::from(o.rede.pow.memoria_kib / 1024) * u64::from(painel.linhas.load(Ordering::Relaxed)),
        painel.na_rede.load(Ordering::Relaxed),
        texto_json(&url_celular),
        texto_json(&painel.paineis.lock().map(|p| p.clone()).unwrap_or_default()),
        painel.mercado_ligado.load(Ordering::Relaxed),
        mercado_quando,
        mercado_json,
        trabalho_n,
        o.rede.uteis.useful_rounds,
        o.rede.uteis.useful_size_min,
        o.rede.uteis.useful_size_base,
        o.rede.uteis.useful_size_max,
        u64::from(trabalho_n) * u64::from(trabalho_n) * 4,
    );
    // ULTRAX: o motor de trabalho útil, separado da mineração.
    let _ = write!(j, "\"ultrax\":{},", painel.ultrax.json());
    // Segundo fator, envio e histórico da carteira.
    let (fator_ligado, exige_envio, trava) = painel
        .seguranca
        .lock()
        .map(|s| (s.ligado(), s.exige_envio, s.trava))
        .unwrap_or((false, false, false));
    let _ = write!(
        j,
        "\"trancado\":false,\"seguranca\":{{\"ligado\":{fator_ligado},\"exige_envio\":{exige_envio},\"trava\":{trava}}},\
         \"pode_enviar\":{},\"maximo_envio\":\"{}\",\"taxa_padrao\":\"{}\",",
        endereco.is_some() && pode_mandar && saldo > 0,
        hyx(u128::from(envio::maximo(saldo, envio::TAXA_PADRAO))),
        hyx(u128::from(envio::TAXA_PADRAO)),
    );
    // O aviso de bloco achado.
    let _ = write!(
        j,
        "\"avisar_bloco\":{},\"som_bloco\":{},",
        painel.avisar_bloco.load(Ordering::Relaxed),
        painel.som_bloco.load(Ordering::Relaxed),
    );
    j.push_str("\"historico\":[");
    if let Some(meu) = endereco {
        let movimentos = envio::historico(c, &no.mempool_ordenado(), &meu, HISTORICO_MAX);
        for (i, m) in movimentos.iter().enumerate() {
            let _ = write!(
                j,
                "{}{{\"quando\":{},\"altura\":{},\"pendente\":{},\"entrada\":{},\"outro\":\"{}\",\
                 \"valor\":\"{}\",\"taxa\":\"{}\",\"txid\":\"{}\",\"tipo\":\"{}\"}}",
                if i > 0 { "," } else { "" },
                m.quando,
                m.altura,
                m.pendente,
                m.entrada,
                m.outro,
                hyx(u128::from(m.valor)),
                hyx(u128::from(m.taxa)),
                m.txid,
                m.tipo,
            );
        }
    }
    j.push_str("],\"maquinas\":[");
    if let Ok(vistas) = painel.maquinas_vistas.lock() {
        for (i, v) in vistas.iter().enumerate() {
            let _ = write!(
                j,
                "{}{{\"alvo\":{},\"ok\":{},\"erro\":{},\"quando\":{},\"minerando\":{},\"ritmo\":{:.3},\
                 \"linhas\":{},\"nucleos\":{},\"altura\":{},\"watts\":{:.1},\"tentativas\":{},\
                 \"versao\":{},\"endereco\":{},\"saldo\":{},\"rede\":{}}}",
                if i > 0 { "," } else { "" },
                texto_json(&v.alvo),
                v.ok,
                texto_json(&v.erro),
                v.quando,
                v.minerando,
                v.ritmo,
                v.linhas,
                v.nucleos,
                v.altura,
                v.watts,
                v.tentativas,
                texto_json(&v.versao),
                texto_json(&v.endereco),
                texto_json(&v.saldo),
                texto_json(&v.rede),
            );
        }
    }
    j.push_str("],\"blocos\":[");
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
    fn painel_novo_entra_fechado_na_lista_salva_antes_dele() {
        // Lista salva por uma versão que ainda não tinha "mercado" nem "maquinas".
        let velha = "ultrax,carteira,rede,livro,ritmo,fluxo";
        let nova = completar_paineis(velha);
        assert!(nova.starts_with(velha), "a ordem de quem já estava não muda: {nova}");
        for nome in PAINEIS_CONHECIDOS {
            assert!(nova.split(',').any(|i| i.trim_start_matches('-') == nome), "faltou {nome} em {nova}");
        }
        assert!(nova.contains("-mercado") && nova.contains("-maquinas"), "painel novo nasce fechado: {nova}");
        assert!(paineis_validos(&nova));
        // Completar de novo não duplica nada, e quem já estava aberto continua aberto.
        assert_eq!(completar_paineis(&nova), nova);
        assert!(completar_paineis("mercado,ultrax").starts_with("mercado,ultrax,"));
    }

    #[test]
    fn lista_salva_com_a_estacao_ganha_o_ultrax_no_lugar() {
        let velha = "carteira,estacao,rede,livro,ritmo,fluxo,-mercado,-maquinas";
        let migrada = migrar_paineis(velha);
        assert!(migrada.starts_with("carteira,ultrax,ia,verificacao,mineracao,historico,telemetria,rede"), "{migrada}");
        assert!(paineis_validos(&migrada), "{migrada}");
        let completa = completar_paineis(&migrada);
        assert_eq!(completa.split(',').count(), PAINEIS_CONHECIDOS.len(), "{completa}");
        // estação fechada: o ULTRAX nasce fechado, mas a mineração, que estava dentro dela, fica à vista
        let fechada = migrar_paineis("-estacao,carteira");
        assert!(fechada.starts_with("-ultrax,-ia,-verificacao,mineracao,"), "{fechada}");
        assert!(paineis_validos(&fechada));
        // sem estação, nada muda
        assert_eq!(migrar_paineis("carteira,-rede"), "carteira,-rede");
        assert!(!paineis_validos("estacao"), "a estação não existe mais");
    }

    #[test]
    fn json_escapa_aspas_barras_e_controle() {
        assert_eq!(texto_json("a\"b\\c\nd"), "\"a\\\"b\\\\c\\u000ad\"");
    }
}
