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
use std::time::Instant;

use auron_chain::Chain;
use auron_consensus::{ParametrosRede, block_reward};
use auron_crypto::{ADDRESS_LEN, SECRET_LEN, address_from_ed25519_pubkey, ed25519_public_key};
use auron_store::{load_chain, save_chain};
use auron_tx::AUR;

const AJUDA: &str = "\
auron-no — nó local do Auron (sem rede entre nós, ainda)

  auron-no carteira nova --arquivo carteira.txt
      Cria uma chave nova e mostra o endereço. Recusa sobrescrever arquivo.

  auron-no carteira ver --arquivo carteira.txt
      Mostra o endereço de uma carteira existente.

  auron-no minerar --rede testnet --pasta dados --endereco HEX [--blocos N] [--linhas L]
      Minera N blocos (padrão 1; 0 = sem parar) e grava a cadeia depois de cada um.

  auron-no estado --rede testnet --pasta dados [--endereco HEX]
      Mostra a altura da cadeia e, com --endereco, o saldo.

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

fn minerar(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let endereco = o.endereco.ok_or("falta --endereco (crie com: auron-no carteira nova --arquivo carteira.txt)")?;
    std::fs::create_dir_all(&o.pasta).map_err(|e| format!("não consegui criar a pasta: {e}"))?;
    let mut cadeia = abrir_cadeia(&o)?;
    println!(
        "Rede {} · altura {} · {} linha(s), {:.0} MiB",
        o.rede.nome,
        cadeia.height(),
        o.linhas,
        (u64::from(o.rede.pow.memoria_kib) * u64::from(o.linhas)) as f64 / 1024.0
    );
    let mut feitos = 0u64;
    while o.blocos == 0 || feitos < o.blocos {
        let inicio = Instant::now();
        let bloco = cadeia.mine(endereco, vec![], None, o.linhas).map_err(|e| e.to_string())?;
        let altura = bloco.header.height;
        let recompensa = block_reward(altura, &o.rede);
        let n = bloco.useful_proof.as_ref().map_or(0, |p| p.n);
        let bits = bloco.header.bits;
        cadeia.accept_block(bloco, None).map_err(|e| format!("bloco recusado: {e}"))?;
        save_chain(&cadeia, &arquivo_da_cadeia(&o)).map_err(|e| e.to_string())?;
        feitos = feitos.saturating_add(1);
        println!(
            "  bloco {altura} em {:.1} s · trabalho útil {n}×{n} · bits {bits:#010x} · recompensa {} AUR (libera em {} blocos)",
            inicio.elapsed().as_secs_f64(),
            aur(u128::from(recompensa)),
            o.rede.coinbase_maturity
        );
    }
    Ok(())
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
