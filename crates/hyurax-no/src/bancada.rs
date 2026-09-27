// ✝ Provérbios 11:1 — “Balança enganosa é abominação para o Senhor, mas o peso justo é o seu prazer.”
//! ULTRA BENCHMARK: medidas de verdade, com base, atual, alvo e ganho real.
//!
//! "10×" não é enfeite. Cada linha é uma medida feita agora, nesta máquina:
//!
//! - **BASE**: a primeira medida registrada desta métrica nesta máquina
//!   (`PASTA/ciencia/benchmarks.jsonl`). Na primeira vez, a base é a própria
//!   medida, e o ganho é 1,00×;
//! - **ATUAL**: a medida de agora;
//! - **ALVO**: dez vezes a base, a meta do ULTRA;
//! - **GANHO REAL**: atual ÷ base. Se der 2×, aparece 2×.
//!
//! O que não se mede aqui aparece como tal: a GPU só se mede pela janela do
//! programa (WebGL2), e os nós simultâneos se medem com vários nós de
//! verdade (`hyurax-no ciencia no`), não numa bancada de um processo só.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyurax_crypto::sha512;
use hyurax_ultrax::job::{Dominio, EspecificacaoDeJob, Intervalos, Nivel, PedidoDeJob, Resumo, unidade};
use hyurax_ultrax::trabalho::{self, Especificacao, TipoDeTrabalho};

use crate::ciencia::{Ciencia, EstadoDoJob};
use crate::painel::texto_json;
use crate::ultrax::{self, Ultrax};

/// Uma medida.
#[derive(Clone, Debug)]
pub struct Medida {
    /// Nome estável (vira a chave da base).
    pub nome: String,
    /// Unidade do número.
    pub unidade: &'static str,
    /// O valor medido agora.
    pub valor: f64,
    /// Maior é melhor.
    pub maior_melhor: bool,
    /// O que foi medido, em uma linha.
    pub nota: String,
}

fn por_segundo(quantos: u64, inicio: Instant) -> f64 {
    quantos as f64 / inicio.elapsed().as_secs_f64().max(1e-9)
}

/// Mede tudo. `aviso` recebe o que está sendo medido, para quem espera.
pub fn medir(pasta: &Path, nucleos: u32, aviso: &mut dyn FnMut(&str)) -> Result<Vec<Medida>, String> {
    let mut m = Vec::new();

    aviso("agendador: intervalos com 5 milhões de unidades fora de ordem");
    let total = 5_000_000u64;
    let mut v = Intervalos::new();
    let (mut i, mut pico) = (0u64, 0usize);
    let inicio = Instant::now();
    for k in 0..total {
        v.inserir_um(i);
        i = i.saturating_add(7919) % total;
        if k % 4096 == 0 {
            pico = pico.max(v.faixas().len());
        }
    }
    m.push(Medida {
        nome: "agendador_unidades_marcadas_por_s".into(),
        unidade: "unid/s",
        valor: por_segundo(total, inicio),
        maior_melhor: true,
        nota: "unidades concluídas marcadas no conjunto de intervalos, em ordem espalhada".into(),
    });
    m.push(Medida {
        nome: "agendador_memoria_pico_por_milhao".into(),
        unidade: "bytes",
        valor: (pico.saturating_mul(16)) as f64 / (total as f64 / 1e6),
        maior_melhor: false,
        nota: format!("pico de {pico} faixas de 16 bytes durante a marcação; termina em {} faixa(s)", v.faixas().len()),
    });

    aviso("derivação de unidades de um JOB de 2^40 unidades");
    let job = EspecificacaoDeJob::nova(PedidoDeJob {
        dominio: Dominio::Matematica,
        modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 64, 0).map_err(|e| e.to_string())?,
        unidades: 1 << 40,
        nivel: Nivel::Reexecucao,
        redundancia: 1,
        prazo_s: 0,
        orcamento_milicreditos: 0,
        descricao: "benchmark".into(),
    })
    .map_err(|e| e.to_string())?;
    let id = job.id().map_err(|e| e.to_string())?;
    let inicio = Instant::now();
    let mut soma = 0u8;
    for k in 0..200_000u64 {
        let u = unidade(&job, &id, k.saturating_mul(5_497_558)).map_err(|e| e.to_string())?;
        soma ^= u.semente.first().copied().unwrap_or(0);
    }
    m.push(Medida {
        nome: "agendador_unidades_derivadas_por_s".into(),
        unidade: "unid/s",
        valor: por_segundo(200_000, inicio),
        maior_melhor: true,
        nota: format!("especificação e semente de unidades espalhadas por 2^40 (verificador {soma})"),
    });

    aviso("resumo aditivo dos resultados");
    let mut r = Resumo::default();
    let h = sha512(b"benchmark");
    let inicio = Instant::now();
    for k in 0..200_000u64 {
        r.somar(k, &h);
    }
    m.push(Medida {
        nome: "consolidacao_resumo_por_s".into(),
        unidade: "unid/s",
        valor: por_segundo(200_000, inicio),
        maior_melhor: true,
        nota: "resultados somados ao resumo aditivo (um SHA-512 cada)".into(),
    });

    for (tipo, tamanho, passos, rotulo) in [
        (TipoDeTrabalho::Matriz, 256u32, 0u32, "matriz 256×256"),
        (TipoDeTrabalho::Mochila, 128, 0, "mochila de 128 itens"),
        (TipoDeTrabalho::Difusao, 128, 100, "difusão 128×128, 100 passos"),
        (TipoDeTrabalho::Ia, 16, 50, "treino de IA, lote 16, 50 passos"),
    ] {
        aviso(&format!("motor: {rotulo}"));
        let esp = Especificacao::nova(tipo, tamanho, passos).map_err(|e| e.to_string())?;
        let semente = sha512(rotulo.as_bytes());
        let inicio = Instant::now();
        let exec = trabalho::executar(&esp, &semente, &mut |_| true).map_err(|e| e.to_string())?;
        let segundos = inicio.elapsed().as_secs_f64().max(1e-9);
        m.push(Medida {
            nome: format!("cpu_{}_ops_por_s", tipo.nome()),
            unidade: "ops/s",
            valor: exec.operacoes as f64 / segundos,
            maior_melhor: true,
            nota: format!("{rotulo}, uma linha, {} operações em {segundos:.2} s", exec.operacoes),
        });
        let inicio = Instant::now();
        let conferido = trabalho::verificar(&esp, &semente, &exec.resultado).is_ok();
        let segundos = inicio.elapsed().as_secs_f64().max(1e-9);
        m.push(Medida {
            nome: format!("verificacao_{}_ops_por_s", tipo.nome()),
            unidade: "ops/s",
            valor: trabalho::operacoes_de_verificacao(&esp) as f64 / segundos,
            maior_melhor: true,
            nota: format!("{} de {rotulo}, {segundos:.2} s ({})", tipo.metodo().nome(), if conferido { "aceito" } else { "RECUSADO" }),
        });
    }

    aviso(&format!("escala: matriz 256 em 1 e em {nucleos} linha(s) ao mesmo tempo"));
    let uma = vazao_paralela(1)?;
    let todas = vazao_paralela(nucleos.max(1))?;
    m.push(Medida {
        nome: "cpu_matriz_ops_por_s_todas_as_linhas".into(),
        unidade: "ops/s",
        valor: todas,
        maior_melhor: true,
        nota: format!("{nucleos} linha(s) ao mesmo tempo: {:.2}× uma linha só ({uma:.0} ops/s)", todas / uma.max(1.0)),
    });

    aviso("ponta a ponta: um JOB de 120 unidades (matriz 48) pelo worker de verdade");
    let (vazao, segundos) = ponta_a_ponta(pasta, nucleos)?;
    m.push(Medida {
        nome: "unidades_verificadas_por_s".into(),
        unidade: "unid/s",
        valor: vazao,
        maior_melhor: true,
        nota: format!(
            "JOB inteiro: despacho, execução, registro assinado, conferência, histórico, consolidação e checkpoint; {nucleos} linha(s), {segundos:.1} s"
        ),
    });
    Ok(m)
}

fn vazao_paralela(linhas: u32) -> Result<f64, String> {
    let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 256, 0).map_err(|e| e.to_string())?;
    let inicio = Instant::now();
    let fios: Vec<_> = (0..linhas)
        .map(|k| {
            std::thread::spawn(move || {
                let semente = sha512(&k.to_be_bytes());
                trabalho::executar(&esp, &semente, &mut |_| true).map(|e| e.operacoes).unwrap_or(0)
            })
        })
        .collect();
    let operacoes: u64 = fios.into_iter().map(|f| f.join().unwrap_or(0)).sum();
    Ok(operacoes as f64 / inicio.elapsed().as_secs_f64().max(1e-9))
}

fn ponta_a_ponta(pasta: &Path, nucleos: u32) -> Result<(f64, f64), String> {
    let dir = pasta.join("benchmark-temporario");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let identidade = crate::identidade_na_pasta(&dir)?;
    let partida = ultrax::Partida {
        ligado: true,
        linhas: nucleos.max(1),
        uso_cpu: 100,
        memoria_mib: 256,
        debug: false,
        gpu: false,
        gpu_uso: 50,
    };
    let u = Ultrax::abrir(&dir, identidade.segredo(), nucleos.max(1), &partida, Box::new(|_, _| {}));
    u.lab.store(false, Ordering::Relaxed);
    let c = Ciencia::abrir(&dir, u.worker(), crate::painel::VERSAO)?;
    c.ligar(&u);
    let unidades = 120u64;
    let id = c.submeter(PedidoDeJob {
        dominio: Dominio::Matematica,
        modelo: Especificacao::nova(TipoDeTrabalho::Matriz, 48, 0).map_err(|e| e.to_string())?,
        unidades,
        nivel: Nivel::Reexecucao,
        redundancia: 1,
        prazo_s: 0,
        orcamento_milicreditos: 0,
        descricao: "ULTRA BENCHMARK, ponta a ponta".into(),
    })?;
    let inicio = Instant::now();
    u.iniciar();
    let limite = Duration::from_secs(600);
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let estado = c.jobs.lock().ok().and_then(|j| j.iter().find(|x| x.id == id).map(|x| x.estado));
        if estado == Some(EstadoDoJob::Concluido) {
            break;
        }
        if inicio.elapsed() > limite {
            u.ligar(false);
            return Err("o JOB do benchmark não terminou em 10 minutos".into());
        }
    }
    let segundos = inicio.elapsed().as_secs_f64();
    u.ligar(false);
    let _ = std::fs::remove_dir_all(&dir);
    Ok((unidades as f64 / segundos.max(1e-9), segundos))
}

/// A primeira medida registrada de cada métrica, a base.
fn bases(arquivo: &Path) -> Vec<(String, f64)> {
    let texto = std::fs::read_to_string(arquivo).unwrap_or_default();
    let mut vistas: Vec<(String, f64)> = Vec::new();
    for linha in texto.lines() {
        // cada linha: {"quando":..,"versao":"..","medidas":[{"nome":"x","valor":1.0,...},...]}
        for trecho in linha.split("{\"nome\":\"").skip(1) {
            let Some((nome, resto)) = trecho.split_once('"') else { continue };
            let Some(valor) = resto.split("\"valor\":").nth(1).and_then(|v| v.split([',', '}']).next()).and_then(|v| v.parse::<f64>().ok())
            else {
                continue;
            };
            if !vistas.iter().any(|(n, _)| n == nome) {
                vistas.push((nome.to_string(), valor));
            }
        }
    }
    vistas
}

/// Mede, registra em `benchmarks.jsonl` e devolve o JSON com base, atual,
/// alvo e ganho de cada métrica.
pub fn rodar(ciencia: &Arc<Ciencia>, nucleos: u32, aviso: &mut dyn FnMut(&str)) -> Result<String, String> {
    let arquivo = ciencia.pasta().join("benchmarks.jsonl");
    let base = bases(&arquivo);
    let medidas = medir(ciencia.pasta(), nucleos, aviso)?;
    let quando = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut linha = format!("{{\"quando\":{quando},\"versao\":\"{}\",\"nucleos\":{nucleos},\"medidas\":[", ciencia.versao());
    let mut saida = format!("{{\"quando\":{quando},\"versao\":\"{}\",\"nucleos\":{nucleos},\"linhas\":[", ciencia.versao());
    for (k, x) in medidas.iter().enumerate() {
        if k > 0 {
            linha.push(',');
            saida.push(',');
        }
        let _ = write!(linha, "{{\"nome\":\"{}\",\"valor\":{:.3},\"unidade\":\"{}\"}}", x.nome, x.valor, x.unidade);
        let b = base.iter().find(|(n, _)| *n == x.nome).map_or(x.valor, |(_, v)| *v);
        let ganho = if x.maior_melhor { x.valor / b.max(1e-12) } else { b / x.valor.max(1e-12) };
        let alvo = if x.maior_melhor { b * 10.0 } else { b / 10.0 };
        let _ = write!(
            saida,
            "{{\"nome\":\"{}\",\"unidade\":\"{}\",\"base\":{b:.3},\"atual\":{:.3},\"alvo\":{alvo:.3},\"ganho\":{ganho:.3},\
             \"maior_melhor\":{},\"primeira\":{},\"nota\":{}}}",
            x.nome,
            x.unidade,
            x.valor,
            x.maior_melhor,
            !base.iter().any(|(n, _)| *n == x.nome),
            texto_json(&x.nota)
        );
    }
    linha.push_str("]}");
    saida.push_str("],\"gpu\":\"medida pela janela do programa (WebGL2); não entra no benchmark de terminal\",\"nos\":\"medidos com vários nós (hyurax-no ciencia no), não nesta bancada\"}");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&arquivo) {
        let _ = writeln!(f, "{linha}");
    }
    Ok(saida)
}

/// As medidas já registradas, na ordem.
pub fn registradas(ciencia: &Ciencia) -> String {
    let texto = std::fs::read_to_string(ciencia.pasta().join("benchmarks.jsonl")).unwrap_or_default();
    format!("[{}]", texto.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join(","))
}
