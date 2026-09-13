// ✝ Eclesiastes 9:10 — “Tudo quanto te vier à mão para fazer, faze-o conforme as tuas forças.”
//! Minerador Argon2id do Auron. Feito para rodar no Termux do celular.
//!
//! ```text
//! auron-minerar medir     [--rede mainnet] [--linhas N] [--segundos S]
//! auron-minerar cabecalho HEX [--rede mainnet] [--linhas N] [--pausa-ms P] [--nonce-inicial N]
//! ```
//!
//! `medir` diz quantas tentativas por segundo o aparelho faz e quanta memória
//! gasta. `cabecalho` procura o nonce de um cabeçalho de 222 bytes montado pelo
//! nó (hoje, o gabarito em Python).

#![allow(clippy::arithmetic_side_effects)]

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use auron_pow::{
    BITS_OFFSET, ConfigMineracao, HEADER_LEN, NONCE_OFFSET, ParametrosPow, Resultado, alvo_de_bits,
    bits_do_cabecalho, com_nonce, minerar, tentativas_esperadas,
};

const AJUDA: &str = "\
auron-minerar — minerador Argon2id do Auron

  auron-minerar medir [opções]
      Mede tentativas por segundo e memória usada. Não precisa de rede.

  auron-minerar cabecalho HEX [opções]
      Procura o nonce de um cabeçalho de 222 bytes (hexadecimal).

Opções:
  --rede mainnet|testnet|regtest   parâmetros do Argon2id (padrão: mainnet)
  --linhas N        linhas de execução; cada uma usa 32 MiB na mainnet
                    (padrão: metade dos núcleos, para o aparelho continuar usável)
  --segundos S      duração da medição (padrão: 20)
  --pausa-ms P      pausa depois de cada tentativa, para não esquentar (padrão: 0)
  --nonce-inicial N primeiro nonce (padrão: 0)
";

struct Opcoes {
    rede: String,
    linhas: u32,
    segundos: u64,
    pausa_ms: u64,
    nonce_inicial: u64,
}

fn ler_opcoes(args: &[String]) -> Result<Opcoes, String> {
    let nucleos = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut o = Opcoes {
        rede: "mainnet".into(),
        linhas: u32::try_from((nucleos / 2).max(1)).unwrap_or(1),
        segundos: 20,
        pausa_ms: 0,
        nonce_inicial: 0,
    };
    let mut it = args.iter();
    while let Some(nome) = it.next() {
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        let numero = || valor.parse::<u64>().map_err(|_| format!("{nome}: \"{valor}\" não é número"));
        match nome.as_str() {
            "--rede" => o.rede = valor.clone(),
            "--linhas" => {
                o.linhas = u32::try_from(numero()?)
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or("--linhas precisa ser pelo menos 1")?;
            }
            "--segundos" => o.segundos = numero()?.max(1),
            "--pausa-ms" => o.pausa_ms = numero()?,
            "--nonce-inicial" => o.nonce_inicial = numero()?,
            _ => return Err(format!("opção desconhecida: {nome}")),
        }
    }
    Ok(o)
}

fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn duracao_legivel(segundos: f64) -> String {
    if !segundos.is_finite() {
        return "sem estimativa".into();
    }
    match segundos {
        s if s < 120.0 => format!("{s:.0} s"),
        s if s < 7200.0 => format!("{:.0} min", s / 60.0),
        s if s < 172_800.0 => format!("{:.1} h", s / 3600.0),
        s => format!("{:.1} dias", s / 86_400.0),
    }
}

/// Roda a busca numa thread e mostra o andamento a cada 5 segundos.
fn rodar(
    cabecalho: &[u8],
    p: ParametrosPow,
    config: ConfigMineracao,
    prazo: Option<Duration>,
) -> Result<(Resultado, f64), String> {
    let parar = AtomicBool::new(false);
    let progresso = AtomicU64::new(0);
    let inicio = Instant::now();
    std::thread::scope(|escopo| {
        let busca = escopo.spawn(|| minerar(cabecalho, p, config, &parar, &progresso));
        let mut ultimo_aviso = Instant::now();
        while !busca.is_finished() {
            std::thread::sleep(Duration::from_millis(200));
            let passado = inicio.elapsed();
            if prazo.is_some_and(|limite| passado >= limite) {
                parar.store(true, Ordering::Relaxed);
            }
            if ultimo_aviso.elapsed() >= Duration::from_secs(5) {
                let feitas = progresso.load(Ordering::Relaxed);
                eprintln!(
                    "  {:>5.0} s · {feitas} tentativas · {:.2} por segundo",
                    passado.as_secs_f64(),
                    feitas as f64 / passado.as_secs_f64()
                );
                ultimo_aviso = Instant::now();
            }
        }
        let resultado = busca
            .join()
            .map_err(|_| "a linha de mineração parou com erro interno".to_string())?
            .map_err(|e| e.to_string())?;
        Ok((resultado, inicio.elapsed().as_secs_f64()))
    })
}

fn cabecalho_vazio_com_bits(bits: u32) -> Vec<u8> {
    let mut c = vec![0u8; HEADER_LEN];
    if let Some(campo) = c.get_mut(BITS_OFFSET..NONCE_OFFSET) {
        campo.copy_from_slice(&bits.to_be_bytes());
    }
    c
}

fn principal(args: &[String]) -> Result<(), String> {
    let (comando, resto) = args.split_first().ok_or(AJUDA)?;
    match comando.as_str() {
        "medir" => {
            let o = ler_opcoes(resto)?;
            let p = ParametrosPow::da_rede(&o.rede).ok_or(format!("rede desconhecida: {}", o.rede))?;
            println!(
                "Medindo {} linha(s), {:.1} MiB cada, {:.1} MiB no total, por {} s...",
                o.linhas,
                mib(p.memoria_bytes()),
                mib(p.memoria_bytes() * u64::from(o.linhas)),
                o.segundos
            );
            // Alvo 1: não bate nunca, então mede só velocidade.
            let cabecalho = cabecalho_vazio_com_bits(0x0101_0000);
            let config = ConfigMineracao {
                linhas: o.linhas,
                nonce_inicial: 0,
                limite: None,
                pausa: Duration::from_millis(o.pausa_ms),
            };
            let (r, segundos) = rodar(&cabecalho, p, config, Some(Duration::from_secs(o.segundos)))?;
            let taxa = r.tentativas as f64 / segundos;
            println!("\nResultado ({})", o.rede);
            println!("  tentativas:            {}", r.tentativas);
            println!("  tentativas por segundo: {taxa:.2}");
            println!("  por linha:              {:.2}", taxa / f64::from(o.linhas));
            println!("  memória reservada:      {:.1} MiB", mib(p.memoria_bytes() * u64::from(o.linhas)));
            Ok(())
        }
        "cabecalho" => {
            let (hex, resto) = resto.split_first().ok_or("falta o cabeçalho em hexadecimal")?;
            let o = ler_opcoes(resto)?;
            let p = ParametrosPow::da_rede(&o.rede).ok_or(format!("rede desconhecida: {}", o.rede))?;
            let mut cabecalho = decodificar_hex(hex)?;
            let alvo = alvo_de_bits(bits_do_cabecalho(&cabecalho).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            println!(
                "Minerando com {} linha(s) ({:.1} MiB). Em média são {:.0} tentativas.",
                o.linhas,
                mib(p.memoria_bytes() * u64::from(o.linhas)),
                tentativas_esperadas(&alvo)
            );
            let config = ConfigMineracao {
                linhas: o.linhas,
                nonce_inicial: o.nonce_inicial,
                limite: None,
                pausa: Duration::from_millis(o.pausa_ms),
            };
            let (r, segundos) = rodar(&cabecalho, p, config, None)?;
            let achado = r.achado.ok_or("a busca parou sem achar nonce")?;
            com_nonce(&mut cabecalho, achado.nonce).map_err(|e| e.to_string())?;
            println!("\nAchou em {} ({} tentativas)", duracao_legivel(segundos), r.tentativas);
            println!("  nonce:     {}", achado.nonce);
            println!("  pow_hash:  {}", codificar_hex(&achado.hash));
            println!("  cabeçalho: {}", codificar_hex(&cabecalho));
            let taxa = r.tentativas as f64 / segundos;
            println!(
                "  tempo médio neste alvo, neste aparelho: {}",
                duracao_legivel(tentativas_esperadas(&alvo) / taxa)
            );
            Ok(())
        }
        "--ajuda" | "-h" | "ajuda" => {
            print!("{AJUDA}");
            Ok(())
        }
        outro => Err(format!("comando desconhecido: {outro}\n\n{AJUDA}")),
    }
}

fn decodificar_hex(texto: &str) -> Result<Vec<u8>, String> {
    let texto = texto.trim();
    if !texto.len().is_multiple_of(2) {
        return Err("hexadecimal com número ímpar de dígitos".into());
    }
    (0..texto.len())
        .step_by(2)
        .map(|i| {
            texto
                .get(i..i + 2)
                .and_then(|par| u8::from_str_radix(par, 16).ok())
                .ok_or_else(|| format!("hexadecimal inválido na posição {i}"))
        })
        .collect()
}

fn codificar_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
