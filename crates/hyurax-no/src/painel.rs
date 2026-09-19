//! Painel do minerador: o nó, a mineração e um dashboard no navegador, num
//! programa só.
//!
//! ```text
//! hyurax-no painel --arquivo carteira.txt [--painel-porta 8800] [--painel-rede]
//! ```
//!
//! O dashboard é servido pelo próprio programa, sem internet e sem instalar
//! nada: abre em `http://127.0.0.1:8800`. Com `--painel-rede`, outros aparelhos
//! da rede local (o celular no mesmo Wi-Fi) também **veem** o painel, mas só o
//! próprio computador pode ligar, parar ou mudar a mineração.
//!
//! Segurança do servidor local:
//! - Comando (POST) só vem do próprio computador, com `Host` e `Origin` locais:
//!   uma página maliciosa aberta no navegador não consegue mandar o minerador
//!   fazer nada, nem por DNS apontado para 127.0.0.1.
//! - Pedido limitado a 16 KiB, tempo de leitura curto, nenhum arquivo do disco
//!   é servido: tudo o que o painel mostra está embutido no programa.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyurax_block::Block;
use hyurax_consensus::block_reward;
use hyurax_crypto::ADDRESS_LEN;
use hyurax_net::Rede;
use hyurax_pow::ConfigMineracao;
use hyurax_tx::{HYUR, Tx};

use crate::{Opcoes, hex, hyur, salvar, subir_rede};

const INDEX: &str = include_str!("../painel/index.html");
const CSS: &str = include_str!("../painel/painel.css");
const JS: &str = include_str!("../painel/painel.js");
const ESTACAO: &str = include_str!("../painel/estacao.js");
const THREE: &str = include_str!("../../../site/vendor/three.module.min.js");

/// Quantos eventos e amostras o painel guarda na memória.
const EVENTOS_MAX: usize = 80;
const AMOSTRAS_MAX: usize = 120;
const BLOCOS_NO_LIVRO: usize = 24;

/// Tudo o que o minerador e o nó contam ao painel.
struct Painel {
    endereco: [u8; ADDRESS_LEN],
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
        println!("  [{tipo}] {texto}");
        if let Ok(mut fila) = self.eventos.lock() {
            fila.push_back(Evento { quando: agora_unix(), tipo, texto });
            while fila.len() > EVENTOS_MAX {
                fila.pop_front();
            }
        }
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
}

/// `hyurax-no painel`: nó + mineração + dashboard.
pub fn painel(args: &[String]) -> Result<(), String> {
    // Opções próprias do painel saem antes; o resto é igual ao do minerador.
    let mut porta_painel: u16 = 8800;
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
        (None, Some(arquivo)) => crate::carteira::endereco(&crate::ler_arquivo(arquivo)?)?,
        (None, None) => return Err("falta --arquivo carteira.txt (ou --endereco HEX) para receber a recompensa".into()),
    };
    let rede = subir_rede(&o)?;
    let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
    let painel = Arc::new(Painel {
        endereco,
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

    let ip = if na_rede { IpAddr::V4(Ipv4Addr::UNSPECIFIED) } else { IpAddr::V4(Ipv4Addr::LOCALHOST) };
    let ouvinte = TcpListener::bind(SocketAddr::new(ip, porta_painel))
        .map_err(|e| format!("não consegui abrir o painel na porta {porta_painel}: {e}"))?;
    println!("Painel: http://127.0.0.1:{porta_painel}");
    if na_rede {
        println!("  (visível na rede local; comandos só deste computador)");
    }
    let o = Arc::new(o);
    {
        let (painel, rede, o) = (Arc::clone(&painel), Arc::clone(&rede), Arc::clone(&o));
        std::thread::spawn(move || servir(ouvinte, porta_painel, &painel, &rede, &o));
    }
    {
        let (painel, rede, o) = (Arc::clone(&painel), Arc::clone(&rede), Arc::clone(&o));
        std::thread::spawn(move || minerador(&painel, &rede, &o));
    }
    vigiar(&painel, &rede, &o)
}

/// Laço principal: amostra o ritmo, grava a cadeia, percebe blocos de fora.
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
        if !painel.minerando.load(Ordering::Relaxed) {
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
        if let Err(e) = uma_rodada(painel, rede, o) {
            painel.registrar("erro", e);
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

fn uma_rodada(painel: &Arc<Painel>, rede: &Arc<Rede>, o: &Opcoes) -> Result<(), String> {
    let candidato = {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        no.chain
            .build_candidate(painel.endereco, no.mempool_ordenado(), None, Vec::new())
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
            "content-length" => tamanho = valor.parse().ok().filter(|t| *t <= 4096)?,
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

fn atender(mut s: TcpStream, porta: u16, painel: &Painel, rede: &Rede, o: &Opcoes) -> std::io::Result<()> {
    let Some(p) = ler_pedido(&mut s) else {
        return responder(&mut s, "400 Bad Request", "text/plain", b"pedido invalido");
    };
    let local = s.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
    let hosts_locais = [format!("127.0.0.1:{porta}"), format!("localhost:{porta}")];
    let host_local = hosts_locais.contains(&p.host);
    let rota = p.caminho.split('?').next().unwrap_or("");
    match (p.metodo.as_str(), rota) {
        ("GET", "/") => responder(&mut s, "200 OK", "text/html; charset=utf-8", INDEX.as_bytes()),
        ("GET", "/painel.css") => responder(&mut s, "200 OK", "text/css; charset=utf-8", CSS.as_bytes()),
        ("GET", "/painel.js") => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", JS.as_bytes()),
        ("GET", "/estacao.js") => responder(&mut s, "200 OK", "text/javascript; charset=utf-8", ESTACAO.as_bytes()),
        ("GET", "/three.module.min.js") => {
            responder(&mut s, "200 OK", "text/javascript; charset=utf-8", THREE.as_bytes())
        }
        ("GET", "/api/estado") => {
            let json = estado_json(painel, rede, o, local && host_local);
            responder(&mut s, "200 OK", "application/json; charset=utf-8", json.as_bytes())
        }
        ("POST", "/api/minerar") => {
            // Só o próprio computador manda, e só a partir da página do painel.
            let origem_ok = p
                .origem
                .as_deref()
                .is_none_or(|og| hosts_locais.iter().any(|h| og == format!("http://{h}")));
            if !(local && host_local && origem_ok) {
                return responder(&mut s, "403 Forbidden", "text/plain", b"comando so deste computador");
            }
            for par in p.corpo.split('&') {
                match par.split_once('=') {
                    Some(("ligar", v)) => painel.minerando.store(v == "1", Ordering::Relaxed),
                    Some(("linhas", v)) => {
                        if let Ok(n) = v.parse::<u32>() {
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
            responder(&mut s, "204 No Content", "text/plain", b"")
        }
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
    let Ok(no) = rede.no.lock() else {
        return "{\"erro\":\"no travado\"}".into();
    };
    let c = &no.chain;
    let altura = c.height();
    let saldo = c.state.balance(&painel.endereco, &HYUR);
    let imaturo = c.state.immature_balance(&painel.endereco);
    let rodada = painel.rodada.lock().ok().and_then(|r| *r);
    // Blocos meus na cadeia inteira, não só nesta sessão.
    let meus_na_cadeia = c
        .entries
        .iter()
        .filter(|e| matches!(e.block.transactions.first(), Some(Tx::Coinbase(cb)) if cb.recipient == painel.endereco))
        .count();
    let _ = write!(
        j,
        "{{\"rede\":{},\"altura\":{altura},\"ponta\":\"{}\",\"trabalho\":\"{}\",\"emitido\":\"{}\",\
         \"pares\":{},\"mempool\":{},\"endereco\":\"{}\",\"saldo\":\"{}\",\"imaturo\":\"{}\",\
         \"recompensa\":\"{}\",\"maturidade\":{},\"minerando\":{},\"linhas\":{},\"nucleos\":{},\
         \"memoria_mib\":{},\"tentativas\":{},\"ritmo\":{:.3},\"meus\":{},\"meus_cadeia\":{meus_na_cadeia},\"perdidos\":{},\
         \"ligado_s\":{},\"rodada_s\":{},\"rodada_altura\":{},\"pode_mandar\":{},\"agora\":{},",
        texto_json(o.rede.nome),
        hex(&c.tip_hash()),
        c.total_work().to_decimal(),
        hyur(u128::from(c.state.total_emitted)),
        rede.pares_conectados(),
        no.mempool_len(),
        hex(&painel.endereco),
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
        pode_mandar,
        agora_unix(),
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
        let meu = matches!(e.block.transactions.first(), Some(Tx::Coinbase(cb)) if cb.recipient == painel.endereco);
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
