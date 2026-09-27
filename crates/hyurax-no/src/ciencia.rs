// ✝ Eclesiastes 3:1 — “Tudo tem o seu tempo determinado, e há tempo para todo o propósito debaixo do céu.”
//! `hyurax-no ciencia ...`: a computação científica pelo terminal.
//!
//! - `rodar`: submete um JOB e calcula até o fim, sem janela, imprimindo o
//!   progresso. Retoma de onde parou se o mesmo JOB já estiver na pasta;
//! - `listar`: os JOBs da pasta;
//! - `relatorio`: grava e mostra o relatório de um JOB;
//! - `refazer`: refaz uma unidade do zero e compara com o registro. É o
//!   comando que o relatório cita para a reprodutibilidade;
//! - `benchmark`: o ULTRA BENCHMARK.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use hyurax_ultrax::job::{EspecificacaoDeJob, unidade};
use hyurax_ultrax::trabalho::{self, hash_da_entrada, hash_do_resultado};

use hyurax_nucleo::VERSAO;
use hyurax_nucleo::cadeia;
use hyurax_nucleo::ciencia::{Ciencia, EstadoDoJob, bancada, pedido_do_formulario, relatorio};
use hyurax_nucleo::config::{self, ConfigDoNo};
use hyurax_nucleo::identidade;
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::ultrax::{self, Ultrax};
use hyurax_nucleo::util::hex;

const USO: &str = "use: hyurax-no ciencia rodar|listar|relatorio|refazer|benchmark|no [--pasta PASTA] [opções]\n\
  rodar:     --dominio D --tipo T --tamanho N [--passos P] [--parametros a,b,c] --unidades U\n\
             [--nivel 2] [--redundancia 1] [--prazo-s 0] [--orcamento 0] [--descricao TEXTO]\n\
             [--linhas L] [--uso-cpu 100] [--memoria-mib 256]\n\
  relatorio: --job JOB_ID [--formato json|csv|pdf]\n\
  refazer:   --job JOB_ID --unidade I\n\
  no:        nó com rede: --porta P [--semente host:porta]... [--rede regtest|testnet]\n\
             [--aceitar-rede] [--segundos S] [e os campos de rodar, para pedir um JOB]";

/// `hyurax-no ciencia ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or(USO)?;
    let mut pasta: Option<PathBuf> = None;
    let mut campos: Vec<(String, String)> = Vec::new();
    let mut job_texto = String::new();
    let mut formato = "json".to_string();
    let mut indice: Option<u64> = None;
    let mut rede_args: Vec<String> = Vec::new();
    let mut aceitar_rede = false;
    let mut segundos: Option<u64> = None;
    let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
    let mut partida = ultrax::Partida {
        ligado: true,
        linhas: nucleos,
        uso_cpu: 100,
        memoria_mib: ultrax::MEMORIA_PADRAO_MIB,
        debug: false,
        gpu: false,
        gpu_uso: 50,
    };
    let mut it = resto.iter();
    while let Some(nome) = it.next() {
        if nome == "--aceitar-rede" {
            aceitar_rede = true;
            continue;
        }
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        let numero = || valor.parse::<u64>().map_err(|_| format!("{nome} precisa ser número"));
        match nome.as_str() {
            "--pasta" => pasta = Some(PathBuf::from(valor)),
            "--job" => job_texto = valor.to_ascii_lowercase(),
            "--formato" => formato = valor.clone(),
            "--unidade" => indice = Some(numero()?),
            "--linhas" => partida.linhas = u32::try_from(numero()?).unwrap_or(1).clamp(1, nucleos),
            "--uso-cpu" => partida.uso_cpu = u32::try_from(numero()?).unwrap_or(100),
            "--memoria-mib" => partida.memoria_mib = u32::try_from(numero()?).unwrap_or(ultrax::MEMORIA_PADRAO_MIB),
            "--dominio" | "--tipo" | "--tamanho" | "--passos" | "--parametros" | "--unidades" | "--nivel" | "--redundancia"
            | "--descricao" => campos.push((nome.trim_start_matches("--").to_string(), valor.clone())),
            "--prazo-s" => campos.push(("prazo_s".into(), valor.clone())),
            "--porta" | "--semente" | "--rede" => rede_args.extend([nome.clone(), valor.clone()]),
            "--segundos" => segundos = Some(numero()?),
            "--orcamento" => campos.push(("orcamento_milicreditos".into(), valor.clone())),
            _ => return Err(format!("opção desconhecida: {nome}\n{USO}")),
        }
    }
    let pastas = match pasta {
        Some(p) => Pastas::unica(p),
        None => Pastas::do_sistema()?,
    };
    pastas.criar()?;
    let identidade = identidade::na_pasta(&pastas.config)?;
    let pasta = pastas.dados.clone();
    // o que calcula trava a pasta; listar e ler relatório não precisam
    let _trava = if matches!(sub.as_str(), "rodar" | "no" | "benchmark") { Some(pastas.travar()?) } else { None };
    match sub.as_str() {
        "rodar" => {
            let pedido = pedido_do_formulario(&campos)?;
            let u = Ultrax::abrir(&pasta, identidade.segredo(), nucleos, &partida, Box::new(|tipo, texto| println!("  [{tipo}] {texto}")));
            u.lab.store(false, Ordering::Relaxed);
            let c = Ciencia::abrir(&pasta, u.worker(), VERSAO)?;
            c.ligar(&u);
            let esp = EspecificacaoDeJob::nova(pedido.clone()).map_err(|e| e.to_string())?;
            let id = esp.id().map_err(|e| e.to_string())?;
            let ja_existe = c.situacao(&id).is_some();
            if ja_existe {
                println!("Este JOB já está na pasta: continua de onde parou.");
                let _ = c.mudar(&id, "retomar");
            } else {
                c.submeter(pedido)?;
            }
            println!("JOB {}", hex(&id));
            println!("  estimativa: {}", c.json_estimativa(&esp_de(&c, &id).ok_or("JOB sumiu")?));
            u.iniciar();
            let inicio = Instant::now();
            let mut ultimo = u64::MAX;
            loop {
                std::thread::sleep(Duration::from_millis(1000));
                let Some(s) = c.situacao(&id) else {
                    return Err("JOB sumiu".into());
                };
                let (estado, feitas, falhas, total, agregado, motivo) = (s.estado, s.feitas, s.falhas, s.total, s.agregado, s.motivo);
                if feitas != ultimo {
                    println!(
                        "  {:>7.1} s  {feitas}/{total} conferidas  {falhas} falha(s)  {:.2} unid/s  {agregado}",
                        inicio.elapsed().as_secs_f64(),
                        feitas as f64 / inicio.elapsed().as_secs_f64().max(1e-9)
                    );
                    ultimo = feitas;
                }
                if estado != EstadoDoJob::Rodando {
                    u.ligar(false);
                    let limite = Instant::now();
                    while u.ocupado() && limite.elapsed() < Duration::from_secs(10) {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    println!("Estado: {}{}", estado.nome(), if motivo.is_empty() { String::new() } else { format!(" ({motivo})") });
                    let _ = relatorio::gravar(&c, &id);
                    println!("Relatório: {}", c.pasta_do_job(&id).join("relatorio.json").display());
                    return if estado == EstadoDoJob::Concluido { Ok(()) } else { Err(format!("o JOB parou em {}", estado.nome())) };
                }
            }
        }
        "no" => no_com_rede(&pastas, &campos, &rede_args, aceitar_rede, segundos, nucleos, partida, &identidade),
        "listar" => {
            let c = Ciencia::abrir(&pasta, identidade::worker(&identidade), VERSAO)?;
            let jobs = c.situacoes();
            if jobs.is_empty() {
                println!("Nenhum JOB em {}", c.pasta().display());
            }
            for j in jobs.iter() {
                println!(
                    "{}  {:<18} {:<20} {}/{} conferidas, {} falha(s)  {}",
                    hex(j.id.get(..16).unwrap_or_default()),
                    j.estado.nome(),
                    j.tipo.nome(),
                    j.feitas,
                    j.total,
                    j.falhas,
                    j.agregado
                );
            }
            Ok(())
        }
        "relatorio" => {
            let c = Ciencia::abrir(&pasta, identidade::worker(&identidade), VERSAO)?;
            let id = c.achar(&job_texto).ok_or("JOB não encontrado (use 16 ou mais dígitos do JOB_ID)")?;
            relatorio::gravar(&c, &id)?;
            let (bytes, _) = relatorio::gerar(&c, &id, &formato).ok_or("formato desconhecido")?;
            if formato == "pdf" {
                println!("{}", c.pasta_do_job(&id).join("relatorio.pdf").display());
            } else {
                println!("{}", String::from_utf8_lossy(&bytes));
            }
            Ok(())
        }
        "refazer" => {
            let c = Ciencia::abrir(&pasta, identidade::worker(&identidade), VERSAO)?;
            let id = c.achar(&job_texto).ok_or("JOB não encontrado (use 16 ou mais dígitos do JOB_ID)")?;
            let i = indice.ok_or("falta --unidade")?;
            let esp = esp_de(&c, &id).ok_or("JOB sumiu")?;
            let u = unidade(&esp, &id, i).map_err(|e| e.to_string())?;
            println!("JOB {}\nunidade {i}: {} {}", hex(&id), u.especificacao.tipo().descricao(), u.especificacao.resumo());
            println!("  semente    {}", hex(&u.semente));
            let entrada = hash_da_entrada(&u.especificacao, &u.semente).map_err(|e| e.to_string())?;
            println!("  INPUT_HASH {}", hex(&entrada));
            let inicio = Instant::now();
            let exec = trabalho::executar(&u.especificacao, &u.semente, &mut |_| true).map_err(|e| e.to_string())?;
            let segundos = inicio.elapsed().as_secs_f64();
            let resultado = hash_do_resultado(&exec.resultado);
            println!("  RESULT_HASH {}", hex(&resultado));
            println!("  {} operações em {segundos:.2} s", exec.operacoes);
            let conferido = trabalho::verificar(&u.especificacao, &u.semente, &exec.resultado);
            println!("  conferência: {}", conferido.as_ref().map_or_else(|r| format!("RECUSADO ({r})"), |()| "aceito".into()));
            let registro = std::fs::read_to_string(c.pasta_do_job(&id).join("unidades.jsonl")).unwrap_or_default();
            let marca = format!("{{\"indice\":{i},");
            match registro.lines().find(|l| l.starts_with(&marca)) {
                Some(l) if l.contains(&hex(&resultado)) => {
                    println!("  IGUAL ao registrado no JOB: o resultado se reproduz bit a bit.");
                    Ok(())
                }
                Some(_) => Err("DIFERENTE do registrado no JOB".into()),
                None => {
                    println!("  (esta unidade não está no registro do JOB para comparar)");
                    Ok(())
                }
            }
        }
        "benchmark" => {
            let c = Ciencia::abrir(&pasta, identidade::worker(&identidade), VERSAO)?;
            println!("ULTRA BENCHMARK em {nucleos} núcleo(s). Medidas de agora; a base é a primeira medida registrada nesta máquina.");
            let json = bancada::rodar(&c, nucleos, &mut |t| println!("  medindo {t}…"))?;
            imprimir_benchmark(&json);
            Ok(())
        }
        outro => Err(format!("subcomando desconhecido: {outro}\n{USO}")),
    }
}

/// Um nó com rede, worker e agendador. Pede um JOB (se vierem os campos) e
/// espera o fim; senão, trabalha para os outros (com `--aceitar-rede`) até
/// `--segundos`.
#[allow(clippy::too_many_arguments)]
fn no_com_rede(
    pastas: &Pastas,
    campos: &[(String, String)],
    rede_args: &[String],
    aceitar: bool,
    segundos: Option<u64>,
    nucleos: u32,
    partida: ultrax::Partida,
    identidade: &hyurax_net::Identidade,
) -> Result<(), String> {
    let mut rede_p = config::rede_do_nome("regtest")?;
    let (mut porta, mut sementes) = (0u16, Vec::new());
    for par in rede_args.chunks(2) {
        match (par.first().map(String::as_str), par.get(1)) {
            (Some("--rede"), Some(v)) => rede_p = config::rede_do_nome(v)?,
            (Some("--porta"), Some(v)) => porta = v.parse().map_err(|_| "--porta precisa ser número")?,
            (Some("--semente"), Some(v)) => sementes.extend(v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
            _ => {}
        }
    }
    let no = ConfigDoNo::nova(rede_p, pastas.clone(), porta, sementes, true);
    let rede = cadeia::subir(&no, &cadeia::saida_do_terminal())?;
    let pasta = &pastas.dados;
    let u = Ultrax::abrir(pasta, identidade.segredo(), nucleos, &partida, Box::new(|tipo, texto| println!("  [{tipo}] {texto}")));
    u.lab.store(false, Ordering::Relaxed);
    let c = Ciencia::abrir(pasta, u.worker(), VERSAO)?;
    c.ligar(&u);
    c.ligar_rede(&rede, hyurax_ultrax::prova::chave_do_worker(identidade.segredo()), aceitar);
    u.iniciar();
    println!("worker {} · aceita trabalho da rede: {}", hex(&u.worker()), if aceitar { "sim" } else { "não" });
    let id = if campos.is_empty() {
        None
    } else {
        let pedido = pedido_do_formulario(campos)?;
        let id = c.submeter(pedido)?;
        println!("JOB {}", hex(&id));
        Some(id)
    };
    let inicio = Instant::now();
    let limite = segundos.map(Duration::from_secs);
    let mut ultimo = String::new();
    loop {
        std::thread::sleep(Duration::from_millis(1000));
        let rede_json = c.json_rede();
        if let Some(id) = id {
            let Some(s) = c.situacao(&id) else {
                return Err("JOB sumiu".into());
            };
            let (estado, feitas, falhas, total, agregado) = (s.estado, s.feitas, s.falhas, s.total, s.agregado);
            let linha = format!("{:>7.1} s  {} {feitas}/{total} conferidas  {falhas} falha(s)  pares {}  {agregado}", inicio.elapsed().as_secs_f64(), estado.nome(), rede.pares_conectados());
            if linha.split("  ").skip(1).collect::<Vec<_>>() != ultimo.split("  ").skip(1).collect::<Vec<_>>() {
                println!("{linha}");
                ultimo = linha;
            }
            if estado.e_final() {
                println!("rede: {rede_json}");
                let _ = relatorio::gravar(&c, &id);
                println!("Relatório: {}", c.pasta_do_job(&id).join("relatorio.json").display());
                u.ligar(false);
                return if estado == EstadoDoJob::Concluido { Ok(()) } else { Err(format!("o JOB parou em {}", estado.nome())) };
            }
        } else if inicio.elapsed().as_secs().is_multiple_of(10) {
            println!("{:>7.1} s  pares {}  {rede_json}", inicio.elapsed().as_secs_f64(), rede.pares_conectados());
        }
        if limite.is_some_and(|l| inicio.elapsed() > l) {
            println!("rede: {rede_json}");
            u.ligar(false);
            return if id.is_some() { Err("tempo esgotado antes do fim do JOB".into()) } else { Ok(()) };
        }
    }
}

fn esp_de(c: &Ciencia, id: &[u8; 64]) -> Option<EspecificacaoDeJob> {
    c.especificacao(id)
}

fn imprimir_benchmark(json: &str) {
    println!("{:<44} {:>16} {:>16} {:>16} {:>9}  UNIDADE", "MÉTRICA", "BASE", "ATUAL", "ALVO (10×)", "GANHO");
    for trecho in json.split("{\"nome\":\"").skip(1) {
        let texto = |chave: &str| trecho.split(&format!("\"{chave}\":")).nth(1).and_then(|v| v.split([',', '}']).next()).unwrap_or("");
        let nome = trecho.split('"').next().unwrap_or("");
        let num = |chave: &str| texto(chave).parse::<f64>().unwrap_or(0.0);
        println!(
            "{nome:<44} {:>16.1} {:>16.1} {:>16.1} {:>8.2}×  {}",
            num("base"),
            num("atual"),
            num("alvo"),
            num("ganho"),
            texto("unidade").trim_matches('"')
        );
    }
}
