// ✝ Provérbios 13:11 — “A riqueza de procedência vã diminuirá, mas quem a ajunta com o próprio trabalho a aumentará.”
//! Nó local do Auron: carteira, mineração e saldo, numa cadeia gravada no disco.
//!
//! ```text
//! auron-no carteira nova    --arquivo carteira.txt
//! auron-no carteira ver     --arquivo carteira.txt
//! auron-no minerar          --rede testnet --pasta dados --endereco HEX [--blocos N] [--linhas L]
//! auron-no estado           --rede testnet --pasta dados [--endereco HEX]
//! ```
//!
//! Ainda não conversa com outros nós: a rede entre nós é a próxima fase. Cada
//! bloco minerado passa pela mesma validação completa de um bloco vindo de
//! fora, e a cadeia é gravada no disco depois de cada bloco.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, Instant};

use auron_block::Block;
use auron_chain::Chain;
use auron_consensus::{ParametrosRede, block_reward};
use auron_crypto::{ADDRESS_LEN, SECRET_LEN, address_from_ed25519_pubkey, ed25519_public_key};
use auron_net::{No, Rede};
use auron_pow::ConfigMineracao;
use auron_store::{load_chain, save_chain};
use auron_tx::AUR;

const AJUDA: &str = "\
auron-no — nó local do Auron (sem rede entre nós, ainda)

  auron-no carteira nova --arquivo carteira.txt
      Cria uma chave nova e mostra o endereço. Recusa sobrescrever arquivo.

  auron-no carteira ver --arquivo carteira.txt
      Mostra o endereço de uma carteira existente.

  auron-no minerar --rede testnet --pasta dados --endereco HEX [--blocos N] [--linhas L]
                   [--porta P] [--semente IP:PORTA,...] [--pausa-ms X]
      Minera N blocos (padrão 1; 0 = sem parar) e grava a cadeia depois de cada um.
      Com --porta e/ou --semente, entra na rede: sincroniza e propaga o que minerar.

  auron-no no --rede testnet --pasta dados [--porta P] [--semente IP:PORTA,...]
      Só roda o nó: escuta, sincroniza, serve e propaga. Sem minerar.

  auron-no estado --rede testnet --pasta dados [--endereco HEX]
      Mostra a altura da cadeia e, com --endereco, o saldo.

Dois aparelhos na mesma rede local, por exemplo:
  no PC:      auron-no no --porta 8790 --pasta dados
  no celular: auron-no minerar --porta 8790 --semente IP_DO_PC:8790 --endereco SEU_ENDERECO --blocos 0

Redes: mainnet (difícil: 16 bits de trabalho por bloco), testnet (8 bits), regtest (quase nada).
Lembrete: a rede pública não existe e o AUR não tem valor. Isto é teste.
";

struct Opcoes {
    rede: ParametrosRede,
    pasta: PathBuf,
    arquivo: Option<PathBuf>,
    endereco: Option<[u8; ADDRESS_LEN]>,
    blocos: u64,
    linhas: u32,
    porta: u16,
    sementes: Vec<String>,
    pausa_ms: u64,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn de_hex<const N: usize>(texto: &str) -> Option<[u8; N]> {
    let texto = texto.trim();
    if texto.len() != N.checked_mul(2)? {
        return None;
    }
    let mut saida = [0u8; N];
    for (i, byte) in saida.iter_mut().enumerate() {
        *byte = u8::from_str_radix(texto.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(saida)
}

fn ler_opcoes(args: &[String]) -> Result<Opcoes, String> {
    let nucleos = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut o = Opcoes {
        rede: ParametrosRede::TESTNET,
        pasta: PathBuf::from("dados-auron"),
        arquivo: None,
        endereco: None,
        blocos: 1,
        linhas: u32::try_from((nucleos / 2).max(1)).unwrap_or(1),
        porta: 0,
        sementes: Vec::new(),
        pausa_ms: 0,
    };
    let mut it = args.iter();
    while let Some(nome) = it.next() {
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        match nome.as_str() {
            "--rede" => {
                o.rede = ParametrosRede::da_rede(valor).ok_or(format!("rede desconhecida: {valor}"))?;
            }
            "--pasta" => o.pasta = PathBuf::from(valor),
            "--arquivo" => o.arquivo = Some(PathBuf::from(valor)),
            "--endereco" => {
                o.endereco = Some(de_hex(valor).ok_or("--endereco precisa de 40 dígitos hexadecimais")?);
            }
            "--blocos" => o.blocos = valor.parse().map_err(|_| "--blocos precisa ser número")?,
            "--porta" => o.porta = valor.parse().map_err(|_| "--porta precisa ser número")?,
            "--semente" => {
                o.sementes.extend(valor.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
            }
            "--pausa-ms" => o.pausa_ms = valor.parse().map_err(|_| "--pausa-ms precisa ser número")?,
            "--linhas" => {
                o.linhas = valor
                    .parse::<u32>()
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or("--linhas precisa ser pelo menos 1")?;
            }
            _ => return Err(format!("opção desconhecida: {nome}")),
        }
    }
    Ok(o)
}

/// 1 AUR = 100 000 000 unidades (seção 1 da especificação).
const AUR_UNIDADE: u128 = 100_000_000;

fn aur(unidades: u128) -> String {
    format!("{}.{:08}", unidades / AUR_UNIDADE, unidades % AUR_UNIDADE)
}

// -- carteira --

/// 32 bytes do gerador criptográfico do sistema operacional.
///
/// Linux e Android (Termux): `/dev/urandom`, lido direto, sem biblioteca.
#[cfg(unix)]
fn entropia_do_sistema() -> Result<[u8; SECRET_LEN], String> {
    use std::io::Read;
    let mut segredo = [0u8; SECRET_LEN];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut segredo))
        .map_err(|e| format!("não consegui ler /dev/urandom: {e}"))?;
    Ok(segredo)
}

/// Windows: `RandomNumberGenerator` do .NET, que é o gerador criptográfico do
/// sistema, chamado pelo PowerShell que vem em todo Windows.
#[cfg(windows)]
fn entropia_do_sistema() -> Result<[u8; SECRET_LEN], String> {
    let saida = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$b = [byte[]]::new(32); [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($b); ($b | ForEach-Object { $_.ToString('x2') }) -join ''",
        ])
        .output()
        .map_err(|e| format!("não consegui chamar o PowerShell: {e}"))?;
    let texto = String::from_utf8_lossy(&saida.stdout);
    let segredo: [u8; SECRET_LEN] = de_hex(texto.trim()).ok_or("o PowerShell não devolveu 32 bytes")?;
    if segredo == [0u8; SECRET_LEN] {
        return Err("entropia zerada; recusando criar carteira".into());
    }
    Ok(segredo)
}

fn endereco_da_carteira(arquivo: &Path) -> Result<[u8; ADDRESS_LEN], String> {
    let texto = std::fs::read_to_string(arquivo)
        .map_err(|e| format!("não consegui ler {}: {e}", arquivo.display()))?;
    let linha = texto
        .lines()
        .find_map(|l| l.strip_prefix("segredo="))
        .ok_or("arquivo de carteira sem a linha segredo=")?;
    let segredo: [u8; SECRET_LEN] = de_hex(linha).ok_or("segredo inválido no arquivo")?;
    Ok(address_from_ed25519_pubkey(&ed25519_public_key(&segredo)))
}

fn carteira(args: &[String]) -> Result<(), String> {
    let (acao, resto) = args.split_first().ok_or("use: carteira nova|ver --arquivo ARQUIVO")?;
    let o = ler_opcoes(resto)?;
    let arquivo = o.arquivo.ok_or("falta --arquivo")?;
    match acao.as_str() {
        "nova" => {
            if arquivo.exists() {
                return Err(format!("{} já existe; não sobrescrevo carteira", arquivo.display()));
            }
            let segredo = entropia_do_sistema()?;
            let endereco = address_from_ed25519_pubkey(&ed25519_public_key(&segredo));
            let conteudo = format!(
                "# Carteira Auron de TESTE. Quem tiver este arquivo gasta o saldo.\n\
                 # A rede pública não existe e o AUR não tem valor.\n\
                 segredo={}\nendereco={}\n",
                hex(&segredo),
                hex(&endereco)
            );
            std::fs::write(&arquivo, conteudo).map_err(|e| format!("não consegui gravar: {e}"))?;
            println!("Carteira criada em {}", arquivo.display());
            println!("Endereço: {}", hex(&endereco));
            println!("Guarde o arquivo: sem ele, o saldo não pode ser gasto.");
            Ok(())
        }
        "ver" => {
            println!("Endereço: {}", hex(&endereco_da_carteira(&arquivo)?));
            Ok(())
        }
        outro => Err(format!("ação desconhecida: {outro}")),
    }
}

// -- cadeia --

fn arquivo_da_cadeia(o: &Opcoes) -> PathBuf {
    o.pasta.join(format!("{}.cadeia", o.rede.nome))
}

fn abrir_cadeia(o: &Opcoes) -> Result<Chain, String> {
    let arquivo = arquivo_da_cadeia(o);
    if arquivo.exists() {
        // o próprio disco: pula só o Argon2id, e revalida todo o resto
        load_chain(&arquivo, true, Some(o.rede)).map_err(|e| e.to_string())
    } else {
        Chain::nova(o.rede).map_err(|e| e.to_string())
    }
}

/// Sobe a rede: abre a cadeia do disco, escuta (se `--porta`) e conecta às
/// sementes (`--semente`, separadas por vírgula).
fn subir_rede(o: &Opcoes) -> Result<Arc<Rede>, String> {
    std::fs::create_dir_all(&o.pasta).map_err(|e| format!("não consegui criar a pasta: {e}"))?;
    let cadeia = abrir_cadeia(o)?;
    let rede = Rede::nova(No::novo(cadeia));

    if o.porta > 0 {
        let porta = rede
            .escutar(("0.0.0.0", o.porta))
            .map_err(|e| format!("não consegui escutar na porta {}: {e}", o.porta))?;
        println!("Escutando na porta {porta}");
    }
    for semente in o.sementes.iter().filter(|s| !s.is_empty()) {
        rede.semear(semente);
        match rede.conectar(semente.as_str()) {
            Ok(()) => println!("Conectando em {semente}"),
            Err(e) => println!("  aviso: não consegui conectar em {semente}: {e}"),
        }
    }
    Ok(rede)
}

/// Grava a cadeia no disco.
fn salvar(rede: &Arc<Rede>, o: &Opcoes) -> Result<(), String> {
    let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
    save_chain(&no.chain, &arquivo_da_cadeia(o)).map_err(|e| e.to_string())?;
    Ok(())
}

fn minerar(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let endereco = o.endereco.ok_or("falta --endereco (crie com: auron-no carteira nova --arquivo carteira.txt)")?;
    let rede = subir_rede(&o)?;
    {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        println!(
            "Rede {} · altura {} · {} linha(s), {:.0} MiB",
            o.rede.nome,
            no.chain.height(),
            o.linhas,
            (u64::from(o.rede.pow.memoria_kib) * u64::from(o.linhas)) as f64 / 1024.0
        );
    }

    let mut feitos = 0u64;
    let mut perdidos = 0u64;
    while o.blocos == 0 || feitos < o.blocos {
        let inicio = Instant::now();

        // Monta o candidato com o cadeado, e minera SEM ele: a busca do nonce
        // demora, e travar o nó nesse tempo pararia a rede.
        let (candidato, pow) = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            let transfers = no.mempool_ordenado();
            let candidato = no
                .chain
                .build_candidate(endereco, transfers, None, Vec::new())
                .map_err(|e| e.to_string())?;
            (candidato, o.rede.pow)
        };

        let config = ConfigMineracao {
            linhas: o.linhas,
            nonce_inicial: 0,
            limite: Some(1u64 << 32),
            pausa: Duration::from_millis(o.pausa_ms),
        };
        let achado = auron_pow::minerar(
            &candidato.header.encode(),
            pow,
            config,
            &AtomicBool::new(false),
            &AtomicU64::new(0),
        )
        .map_err(|e| e.to_string())?
        .achado
        .ok_or("não achei nonce no limite de tentativas")?;

        let altura = candidato.header.height;
        let n = candidato.useful_proof.as_ref().map_or(0, |p| p.n);
        let bits = candidato.header.bits;
        let bloco = Block { header: candidato.header.with_nonce(achado.nonce), ..candidato };

        // Enquanto eu minerava, a rede pode ter achado outro bloco na mesma
        // altura. Aí este vira órfão e eu recomeço: é a corrida normal.
        match rede.submeter_bloco(bloco) {
            Ok(true) => {
                salvar(&rede, &o)?;
                feitos = feitos.saturating_add(1);
                println!(
                    "  bloco {altura} em {:.1} s · trabalho útil {n}×{n} · bits {bits:#010x} · recompensa {} AUR (libera em {} blocos) · {} par(es)",
                    inicio.elapsed().as_secs_f64(),
                    aur(u128::from(block_reward(altura, &o.rede))),
                    o.rede.coinbase_maturity,
                    rede.pares_conectados()
                );
            }
            Ok(false) | Err(_) => {
                perdidos = perdidos.saturating_add(1);
                println!("  bloco {altura} perdido na corrida (outro nó chegou primeiro); recomeçando");
            }
        }
    }
    if perdidos > 0 {
        println!("{feitos} bloco(s) meus, {perdidos} perdido(s) na corrida.");
    }
    salvar(&rede, &o)?;
    Ok(())
}

/// Só roda o nó: escuta, sincroniza, serve e propaga. Sem minerar.
fn servir_no(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let rede = subir_rede(&o)?;
    println!("Nó no ar. Ctrl+C para parar. A cadeia é gravada a cada mudança.");
    let mut ultima_altura = u64::MAX;
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let (altura, pares, mempool) = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            (no.chain.height(), rede.pares_conectados(), no.mempool_len())
        };
        if altura != ultima_altura {
            ultima_altura = altura;
            salvar(&rede, &o)?;
            println!("  altura {altura} · {pares} par(es) · {mempool} no mempool");
        }
    }
}

fn estado(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let cadeia = abrir_cadeia(&o)?;
    println!("Rede:              {}", o.rede.nome);
    println!("Altura:            {}", cadeia.height());
    println!("Ponta:             {}", hex(&cadeia.tip_hash()));
    println!("Trabalho total:    {}", cadeia.total_work().to_decimal());
    println!("Emitido:           {} AUR", aur(u128::from(cadeia.state.total_emitted)));
    if let Some(endereco) = o.endereco {
        println!("Saldo gastável:    {} AUR", aur(u128::from(cadeia.state.balance(&endereco, &AUR))));
        println!("Esperando liberar: {} AUR", aur(cadeia.state.immature_balance(&endereco)));
    }
    Ok(())
}

fn principal(args: &[String]) -> Result<(), String> {
    let (comando, resto) = args.split_first().ok_or(AJUDA)?;
    match comando.as_str() {
        "carteira" => carteira(resto),
        "minerar" => minerar(resto),
        "no" => servir_no(resto),
        "estado" => estado(resto),
        "ajuda" | "--ajuda" | "-h" => {
            print!("{AJUDA}");
            Ok(())
        }
        outro => Err(format!("comando desconhecido: {outro}\n\n{AJUDA}")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match principal(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}
