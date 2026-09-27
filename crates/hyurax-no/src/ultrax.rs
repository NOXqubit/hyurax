//! `hyurax-no ultrax lab|auditar`: o ULTRAX pelo terminal.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::ultrax::{Lembranca, MEMORIA_PADRAO_MIB, MODO, Partida, SELO, Ultrax, auditar};
use hyurax_nucleo::util::hex;

/// `hyurax-no ultrax ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (sub, resto) = args.split_first().ok_or("use: hyurax-no ultrax lab|auditar --pasta dados")?;
    let mut pasta: Option<PathBuf> = None;
    let mut tarefas: u64 = 0;
    let mut amostra: usize = 5;
    let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
    let mut partida = Partida { ligado: true, linhas: 1, uso_cpu: 100, memoria_mib: MEMORIA_PADRAO_MIB, debug: false, gpu: false, gpu_uso: 50 };
    let mut it = resto.iter();
    while let Some(nome) = it.next() {
        if nome == "--debug" {
            partida.debug = true;
            continue;
        }
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        let numero = || valor.parse::<u64>().map_err(|_| format!("{nome} precisa ser número"));
        match nome.as_str() {
            "--pasta" => pasta = Some(PathBuf::from(valor)),
            "--tarefas" => tarefas = numero()?,
            "--amostra" => amostra = usize::try_from(numero()?).unwrap_or(usize::MAX),
            "--linhas" => partida.linhas = u32::try_from(numero()?).unwrap_or(1),
            "--uso-cpu" => partida.uso_cpu = u32::try_from(numero()?).unwrap_or(100),
            "--memoria-mib" => partida.memoria_mib = u32::try_from(numero()?).unwrap_or(MEMORIA_PADRAO_MIB),
            _ => return Err(format!("opção desconhecida: {nome}")),
        }
    }
    let pastas = match pasta {
        Some(p) => Pastas::unica(p),
        None => Pastas::do_sistema()?,
    };
    let pasta = pastas.dados.clone();
    match sub.as_str() {
        "auditar" => {
            let a = auditar(&pasta, &pastas.config, amostra)?;
            println!("Auditoria do ULTRAX em {}", pasta.join("ultrax").display());
            println!("  registros:            {} ({} ilegíveis)", a.registros, a.ilegiveis);
            println!("  TASK_ID conferido:    {}", a.ids_certos);
            println!("  assinaturas válidas:  {} de {} com prova", a.assinaturas_certas, a.com_prova);
            println!("  vereditos assinados:  {} de {} com prova", a.vereditos_certos, a.com_prova);
            println!("  refeitas do zero:     {} iguais de {} sorteadas", a.refeitas_iguais, a.refeitas);
            println!("  placar conferido:     {}", if a.placar_conferido { "sim, contra o histórico inteiro" } else { "não" });
            if a.so_refazendo > 0 {
                println!(
                    "  aviso: {} registro(s) de rotas: as operações dependem do caminho do 2-opt; aqui só o teto é conferido, e o número exato nas refeitas",
                    a.so_refazendo
                );
            }
            for aviso in &a.avisos {
                println!("  aviso: {aviso}");
            }
            for p in &a.problemas {
                println!("  PROBLEMA: {p}");
            }
            if a.problemas.is_empty() { Ok(()) } else { Err(format!("{} problema(s) na auditoria", a.problemas.len())) }
        }
        "lab" => {
            let _trava = pastas.travar()?;
            let identidade = hyurax_nucleo::identidade::na_pasta(&pastas.config)?;
            let u = Ultrax::abrir(&pasta, identidade.segredo(), nucleos, &partida, Box::new(|tipo, texto| println!("  [{tipo}] {texto}")));
            println!(
                "ULTRAX, modo {MODO} ({SELO}): tarefas geradas e conferidas nesta máquina.\n  worker {}\n  {} linha(s), CPU até {}%, memória até {} MiB, GPU não usada",
                hex(&u.worker()),
                u.linhas.load(Ordering::Relaxed),
                u.uso_cpu.load(Ordering::Relaxed),
                u.memoria_mib.load(Ordering::Relaxed)
            );
            let antes = u.placar();
            u.iniciar();
            let mut impressos = 0u64;
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let encerrados = u.encerradas();
                if encerrados > impressos {
                    let novos = usize::try_from(encerrados.saturating_sub(impressos)).unwrap_or(usize::MAX);
                    let linhas: Vec<Lembranca> =
                        u.historico_recente(novos);
                    for x in linhas.iter().rev() {
                        println!(
                            "  #{:08} {:<10} {:<30} {:<9} {:>13} operações {:>7.2} s{}",
                            x.numero,
                            x.tipo.nome(),
                            x.resumo,
                            x.estado.nome(),
                            x.operacoes,
                            x.ms_calculo as f64 / 1000.0,
                            if x.nota.is_empty() { String::new() } else { format!("  · {}", x.nota) }
                        );
                    }
                    impressos = encerrados;
                }
                if tarefas > 0 && u.finalizadas() >= tarefas {
                    // para de gerar, cancela a fila e espera as outras linhas fecharem o que têm
                    u.ligar(false);
                    let limite = Instant::now();
                    while u.tem_ativas() && limite.elapsed() < Duration::from_secs(10) {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    u.gravar_placar();
                    let p = u.placar();
                    println!(
                        "Pronto: {} liquidada(s), {} recusada(s), {} cancelada(s). Work Score acumulado {} (medida local; não é HYX), reputação local {}/1000",
                        p.liquidadas.saturating_sub(antes.liquidadas),
                        p.recusadas.saturating_sub(antes.recusadas),
                        p.canceladas.saturating_sub(antes.canceladas),
                        p.score.texto(),
                        p.reputacao.nota()
                    );
                    return Ok(());
                }
            }
        }
        outro => Err(format!("subcomando desconhecido: {outro} (use lab ou auditar)")),
    }
}
