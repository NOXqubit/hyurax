// ✝ Provérbios 25:2 — “A glória dos reis é investigar a coisa.”
//! Triagem de moléculas reais de um catálogo (tipo 8).
//!
//! Tradução de `reference/hyurax/triagem.py`, conferida por
//! `vectors/triagem.json`; o gabarito tem a especificação completa.
//!
//! **O que é:** para cada molécula de uma faixa do catálogo, conferir filtros
//! de regra pelos descritores (massa, logP, doadores e aceptores de
//! hidrogênio, ligações giratórias, área polar: a "regra dos 5" de Lipinski e
//! a de Veber, com os limites de quem pediu) e prever a solubilidade na água
//! (log S) com o modelo de referência.
//!
//! **De onde vêm os números:**
//! - o catálogo (`dados/catalogo.tsv`) são as moléculas reais da AqSolDB
//!   (CC0) que passam o filtro estrutural, com os descritores e o log S
//!   **medido** que a AqSolDB publica;
//! - o log S **previsto** sai da rede 10 → 16 → 1 de [`crate::ia`], com os
//!   pesos de `dados/modelo-referencia.txt`. É um modelo estatístico pequeno,
//!   com o erro contra o log S medido gravado no próprio arquivo: ordena
//!   candidatos, não substitui a medida.
//!
//! **O que não é:** não calcula afinidade com alvo, estabilidade, toxicidade
//! nem atividade biológica, porque não há modelo para isso no programa.
//! Nenhum resultado é "cura" ou "descoberta".
//!
//! Tudo em inteiros; a semente não entra (a triagem é função só da
//! especificação).

use std::sync::OnceLock;

use crate::ia::{self, ENTRADAS, OCULTOS, Pesos};
use crate::trabalho::{ErroDeTrabalho, Especificacao, Execucao, Recusa, TipoDeTrabalho};

/// O catálogo embutido no programa.
pub const CATALOGO_TSV: &str = include_str!("../dados/catalogo.tsv");
/// O modelo de referência embutido no programa.
pub const MODELO_TXT: &str = include_str!("../dados/modelo-referencia.txt");

/// Quantos parâmetros extras a especificação leva.
pub const PARAMETROS: usize = 9;
/// Faixa aceita de `tamanho`: moléculas da faixa.
pub const TAMANHO: (u32, u32) = (1, 4_096);
/// `passos` não é usado: sempre 0.
pub const PASSOS: (u32, u32) = (0, 0);

/// O único catálogo que existe: a AqSolDB embutida.
pub const CATALOGO_AQSOLDB: u32 = 0;
/// Deslocamento dos limites que podem ser negativos (logP e log S).
pub const DESLOCAMENTO: i64 = 20_000;
/// Maior massa máxima aceita, em milésimos de g/mol.
pub const MASSA_MAX: u32 = 2_000_000;
/// Maior logP máximo (deslocado), em milésimos.
pub const LOGP_MAX: u32 = 40_000;
/// Maior contagem máxima (doadores, aceptores, ligações giratórias).
pub const CONTAGEM_MAX: u32 = 100;
/// Maior área polar máxima, em milésimos de Å².
pub const TPSA_MAX: u32 = 1_000_000;
/// Maior log S mínimo (deslocado), em milésimos.
pub const LOGS_MAX: u32 = 40_000;

/// Os filtros, na ordem dos bits da máscara.
pub const FILTROS: [&str; 7] = ["massa", "logP", "doadores de H", "aceptores de H", "ligações giratórias", "área polar", "log S previsto"];
/// Máscara de quem passa em todos.
pub const TODOS_OS_FILTROS: u8 = 0b111_1111;
/// Multiplicações com soma da inferência (10·16 + 16).
pub const OPERACOES_INFERENCIA: u64 = 176;
/// Operações por molécula: a inferência e os sete filtros.
pub const OPERACOES_POR_MOLECULA: u64 = OPERACOES_INFERENCIA + 7;
/// Peso de cada filtro passado na nota.
pub const NOTA_POR_FILTRO: u32 = 100_000;
/// Deslocamento do log S previsto dentro da nota.
pub const NOTA_LOGS_DESLOCAMENTO: i64 = 50_000;
/// Moléculas entre duas chamadas de `continuar`.
pub const BLOCO: usize = 64;
/// Bytes do cabeçalho do resultado.
pub const BYTES_CABECALHO: usize = 12;
/// Bytes por molécula no resultado.
pub const BYTES_POR_MOLECULA: usize = 9;

// colunas dos descritores brutos (a ordem de ia::DESCRITORES)
const MASSA: usize = 0;
const LOGP: usize = 1;
const ACEPTORES: usize = 4;
const DOADORES: usize = 5;
const ROTAVEIS: usize = 6;
const TPSA: usize = 9;

/// Uma molécula do catálogo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Molecula {
    /// Identificador na AqSolDB.
    pub id: String,
    /// Nome.
    pub nome: String,
    /// Fórmula.
    pub formula: String,
    /// SMILES.
    pub smiles: String,
    /// log S medido (log10 mol/L), em milésimos.
    pub logs_mili: i64,
    /// Os 10 descritores brutos, na ordem de [`ia::DESCRITORES`] (massa,
    /// logP, refratividade e área polar em milésimos; contagens inteiras).
    pub bruto: [i64; ENTRADAS],
    /// Os mesmos descritores normalizados em Q12, os do modelo.
    pub x: [i64; ENTRADAS],
}

/// O catálogo e a normalização do alvo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalogo {
    /// As moléculas, na ordem do InChIKey.
    pub moleculas: Vec<Molecula>,
    /// Média do log S das 2048 do modelo, em milésimos.
    pub media_mili: i64,
    /// Desvio do log S das 2048 do modelo, em milésimos.
    pub desvio_mili: i64,
}

impl Catalogo {
    /// Lê o formato de `catalogo.tsv`. Linha com número errado de colunas é
    /// pulada; o teste de vetores confere que nenhuma foi.
    pub fn ler(texto: &str) -> Self {
        let mut c = Self { moleculas: Vec::new(), media_mili: 0, desvio_mili: 1000 };
        for linha in texto.lines() {
            if let Some(resto) = linha.strip_prefix("#normalizacao\t") {
                let mut partes = resto.split('\t').map(|v| v.trim().parse::<i64>().ok());
                if let (Some(Some(m)), Some(Some(d))) = (partes.next(), partes.next()) {
                    c.media_mili = m;
                    c.desvio_mili = d.max(1);
                }
                continue;
            }
            if linha.starts_with('#') || linha.trim().is_empty() {
                continue;
            }
            let colunas: Vec<&str> = linha.split('\t').collect();
            if colunas.len() != 25 {
                continue;
            }
            let numero = |k: usize| colunas.get(k).and_then(|v| v.trim().parse::<i64>().ok());
            let mut bruto = [0i64; ENTRADAS];
            let mut x = [0i64; ENTRADAS];
            let mut ok = true;
            for k in 0..ENTRADAS {
                match (numero(k.saturating_add(5)), numero(k.saturating_add(15))) {
                    (Some(b), Some(v)) => {
                        if let (Some(db), Some(dx)) = (bruto.get_mut(k), x.get_mut(k)) {
                            *db = b;
                            *dx = v;
                        }
                    }
                    _ => ok = false,
                }
            }
            let (Some(logs), true) = (numero(4), ok) else { continue };
            let texto_de = |k: usize| colunas.get(k).map(|s| (*s).to_string()).unwrap_or_default();
            c.moleculas.push(Molecula { id: texto_de(0), nome: texto_de(1), formula: texto_de(2), smiles: texto_de(3), logs_mili: logs, bruto, x });
        }
        c
    }
}

/// O catálogo embutido, lido uma vez.
pub fn catalogo() -> &'static Catalogo {
    static CATALOGO: OnceLock<Catalogo> = OnceLock::new();
    CATALOGO.get_or_init(|| Catalogo::ler(CATALOGO_TSV))
}

/// O modelo de referência: os pesos e como foram treinados.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modelo {
    /// Os pesos, Q12.
    pub pesos: Pesos,
    /// Semente do treino.
    pub semente: Vec<u8>,
    /// Lote do treino.
    pub lote: u32,
    /// Passos do treino.
    pub passos: u32,
    /// Erro de validação do treino, Q12.
    pub erro_validacao: u64,
}

impl Modelo {
    /// Lê o formato de `modelo-referencia.txt`.
    pub fn ler(texto: &str) -> Option<Self> {
        let mut w1 = [[0i64; ENTRADAS]; OCULTOS];
        let (mut linhas_w1, mut b1, mut w2, mut b2) = (0usize, None, None, None);
        let (mut semente, mut lote, mut passos, mut erro) = (None, None, None, None);
        for linha in texto.lines() {
            if linha.starts_with("# ") || linha == "#" || linha.trim().is_empty() {
                continue;
            }
            let colunas: Vec<&str> = linha.split('\t').collect();
            let nome = colunas.first().copied().unwrap_or("");
            let valor = colunas.get(1).copied().unwrap_or("");
            match nome {
                "#semente" => semente = Some(decodificar_hex(valor)?),
                "#lote" => lote = valor.parse::<u32>().ok(),
                "#passos" => passos = valor.parse::<u32>().ok(),
                "#erro_validacao_q12" => erro = valor.parse::<u64>().ok(),
                _ => {
                    let numeros: Vec<i64> = colunas.iter().skip(1).map(|v| v.parse::<i64>().ok()).collect::<Option<_>>()?;
                    match nome {
                        "w1" => {
                            let destino = w1.get_mut(linhas_w1)?;
                            *destino = <[i64; ENTRADAS]>::try_from(numeros.as_slice()).ok()?;
                            linhas_w1 = linhas_w1.saturating_add(1);
                        }
                        "b1" if b1.is_none() => b1 = Some(<[i64; OCULTOS]>::try_from(numeros.as_slice()).ok()?),
                        "w2" if w2.is_none() => w2 = Some(<[i64; OCULTOS]>::try_from(numeros.as_slice()).ok()?),
                        "b2" if b2.is_none() && numeros.len() == 1 => b2 = numeros.first().copied(),
                        _ => return None,
                    }
                }
            }
        }
        if linhas_w1 != OCULTOS {
            return None;
        }
        Some(Self { pesos: Pesos { w1, b1: b1?, w2: w2?, b2: b2? }, semente: semente?, lote: lote?, passos: passos?, erro_validacao: erro? })
    }
}

fn decodificar_hex(texto: &str) -> Option<Vec<u8>> {
    let bytes = texto.trim().as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    bytes
        .chunks(2)
        .map(|par| std::str::from_utf8(par).ok().and_then(|s| u8::from_str_radix(s, 16).ok()))
        .collect()
}

/// O modelo embutido, lido uma vez. `None` só se o arquivo embutido estiver
/// estragado, e aí toda triagem é recusada.
pub fn modelo() -> Option<&'static Modelo> {
    static MODELO: OnceLock<Option<Modelo>> = OnceLock::new();
    MODELO.get_or_init(|| Modelo::ler(MODELO_TXT)).as_ref()
}

/// Saída da rede (log S normalizado, Q12) para log S × 1000: divisão que
/// trunca para o zero, somada à média, presa à faixa de `i32`.
pub fn logs_de(y_q12: i64, media_mili: i64, desvio_mili: i64) -> i64 {
    let v = i128::from(y_q12).saturating_mul(i128::from(desvio_mili)).checked_div(i128::from(ia::ESCALA)).unwrap_or(0);
    let v = v.saturating_add(i128::from(media_mili));
    i64::try_from(v.clamp(i128::from(i32::MIN), i128::from(i32::MAX))).unwrap_or(0)
}

/// Os limites de filtro de uma especificação, já com o deslocamento tirado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Limites {
    inicio: usize,
    massa: i64,
    logp: i64,
    doadores: i64,
    aceptores: i64,
    rotaveis: i64,
    tpsa: i64,
    logs_min: i64,
}

fn limites(p: &[u32]) -> Option<Limites> {
    let [_, inicio, massa, logp, doadores, aceptores, rotaveis, tpsa, logs] = <[u32; PARAMETROS]>::try_from(p).ok()?;
    Some(Limites {
        inicio: usize::try_from(inicio).ok()?,
        massa: i64::from(massa),
        logp: i64::from(logp).saturating_sub(DESLOCAMENTO),
        doadores: i64::from(doadores),
        aceptores: i64::from(aceptores),
        rotaveis: i64::from(rotaveis),
        tpsa: i64::from(tpsa),
        logs_min: i64::from(logs).saturating_sub(DESLOCAMENTO),
    })
}

/// `(máscara, log S previsto em milésimos, nota)` de uma molécula.
fn avaliar(m: &Molecula, l: &Limites, pesos: &Pesos, c: &Catalogo) -> (u8, i64, u32) {
    let (y, _) = ia::prever(pesos, &m.x);
    let previsto = logs_de(y, c.media_mili, c.desvio_mili);
    let b = |k: usize| m.bruto.get(k).copied().unwrap_or(0);
    let testes = [
        b(MASSA) <= l.massa,
        b(LOGP) <= l.logp,
        b(DOADORES) <= l.doadores,
        b(ACEPTORES) <= l.aceptores,
        b(ROTAVEIS) <= l.rotaveis,
        b(TPSA) <= l.tpsa,
        previsto >= l.logs_min,
    ];
    let mut mascara = 0u8;
    let mut passados = 0u32;
    for (k, ok) in testes.iter().enumerate() {
        if *ok {
            mascara |= 1u8.wrapping_shl(u32::try_from(k).unwrap_or(0));
            passados = passados.saturating_add(1);
        }
    }
    let parte_logs = previsto.saturating_add(NOTA_LOGS_DESLOCAMENTO).clamp(0, i64::from(NOTA_POR_FILTRO).saturating_sub(1));
    let nota = passados.saturating_mul(NOTA_POR_FILTRO).saturating_add(u32::try_from(parte_logs).unwrap_or(0));
    (mascara, previsto, nota)
}

/// Confere as faixas dos parâmetros e que a faixa cabe no catálogo.
pub fn validar(tamanho: u32, passos: u32, parametros: &[u32]) -> Result<(), ErroDeTrabalho> {
    let erro = |m: String| Err(ErroDeTrabalho::Parametros(m));
    let Ok([cat, inicio, massa, logp, doadores, aceptores, rotaveis, tpsa, logs]) = <[u32; PARAMETROS]>::try_from(parametros) else {
        return erro(format!("a triagem leva {PARAMETROS} parâmetros"));
    };
    if !(TAMANHO.0..=TAMANHO.1).contains(&tamanho) || passos != 0 {
        return erro("tamanho ou passos fora da faixa".into());
    }
    if cat != CATALOGO_AQSOLDB {
        return erro(format!("catálogo {cat} desconhecido: só existe o 0, AqSolDB embutido"));
    }
    let n = catalogo().moleculas.len();
    let fim = u64::from(inicio).saturating_add(u64::from(tamanho));
    if fim > u64::try_from(n).unwrap_or(0) {
        return erro(format!("faixa [{inicio}, {fim}) fora do catálogo de {n} moléculas"));
    }
    for (nome, valor, maximo) in [
        ("massa máxima", massa, MASSA_MAX),
        ("logP máximo", logp, LOGP_MAX),
        ("doadores de H máximos", doadores, CONTAGEM_MAX),
        ("aceptores de H máximos", aceptores, CONTAGEM_MAX),
        ("ligações giratórias máximas", rotaveis, CONTAGEM_MAX),
        ("área polar máxima", tpsa, TPSA_MAX),
        ("log S previsto mínimo", logs, LOGS_MAX),
    ] {
        if valor > maximo {
            return erro(format!("{nome} fora da faixa: {valor} (máximo {maximo})"));
        }
    }
    if modelo().is_none() {
        return Err(ErroDeTrabalho::SemMotor("triagem molecular (modelo de referência ilegível)"));
    }
    Ok(())
}

/// A especificação da unidade `indice` de um JOB: a faixa anda
/// `indice × tamanho`, e a última é cortada no fim do catálogo.
pub fn derivar_unidade(modelo_do_job: &Especificacao, indice: u64) -> Result<Especificacao, ErroDeTrabalho> {
    let mut p = modelo_do_job.parametros().to_vec();
    let base = u64::from(p.get(1).copied().unwrap_or(0));
    let tamanho = u64::from(modelo_do_job.tamanho());
    let n = u64::try_from(catalogo().moleculas.len()).unwrap_or(0);
    let inicio = indice.checked_mul(tamanho).and_then(|d| d.checked_add(base)).filter(|&i| i < n).ok_or_else(|| {
        ErroDeTrabalho::Parametros(format!("a unidade {indice} começaria depois do fim do catálogo ({n} moléculas)"))
    })?;
    if let Some(destino) = p.get_mut(1) {
        *destino = u32::try_from(inicio).map_err(|_| ErroDeTrabalho::Parametros("início grande demais".into()))?;
    }
    let cabe = u32::try_from(tamanho.min(n.saturating_sub(inicio))).unwrap_or(0);
    Especificacao::nova_com(TipoDeTrabalho::Triagem, cabe, 0, &p)
}

/// Operações da execução: 183 por molécula.
pub fn operacoes(esp: &Especificacao) -> u64 {
    u64::from(esp.tamanho()).saturating_mul(OPERACOES_POR_MOLECULA)
}

/// Operações da verificação: a recomputação inteira.
pub fn operacoes_de_verificacao(esp: &Especificacao) -> u64 {
    operacoes(esp)
}

/// Bytes do resultado.
pub fn tamanho_do_resultado(tamanho: u32) -> usize {
    usize::try_from(tamanho).unwrap_or(0).saturating_mul(BYTES_POR_MOLECULA).saturating_add(BYTES_CABECALHO)
}

/// Memória no pico: o resultado, a recomputação e folga. O catálogo e o
/// modelo são carregados uma vez para o programa inteiro, não por tarefa.
pub fn memoria_bytes(esp: &Especificacao) -> u64 {
    u64::try_from(tamanho_do_resultado(esp.tamanho())).unwrap_or(u64::MAX).saturating_mul(3).saturating_add(64 * 1024)
}

/// Texto curto para o painel.
pub fn resumo(esp: &Especificacao) -> String {
    let inicio = esp.parametros().get(1).copied().unwrap_or(0);
    format!("moléculas {} a {} do catálogo AqSolDB", inicio, u64::from(inicio).saturating_add(u64::from(esp.tamanho())).saturating_sub(1))
}

/// Executa a unidade. A semente não entra.
pub fn executar(esp: &Especificacao, _semente: &[u8], continuar: &mut dyn FnMut(u64) -> bool) -> Result<Execucao, ErroDeTrabalho> {
    let l = limites(esp.parametros()).ok_or_else(|| ErroDeTrabalho::Parametros("a especificação não é de triagem".into()))?;
    let m = modelo().ok_or(ErroDeTrabalho::SemMotor("triagem molecular (modelo de referência ilegível)"))?;
    let c = catalogo();
    let quantidade = usize::try_from(esp.tamanho()).unwrap_or(0);
    let faixa = c
        .moleculas
        .get(l.inicio..l.inicio.saturating_add(quantidade))
        .ok_or_else(|| ErroDeTrabalho::Parametros("faixa fora do catálogo".into()))?;
    let mut corpo = Vec::with_capacity(quantidade.saturating_mul(BYTES_POR_MOLECULA));
    let mut aprovadas = 0u32;
    for pedaco in faixa.chunks(BLOCO) {
        for molecula in pedaco {
            let (mascara, previsto, nota) = avaliar(molecula, &l, &m.pesos, c);
            if mascara == TODOS_OS_FILTROS {
                aprovadas = aprovadas.saturating_add(1);
            }
            corpo.push(mascara);
            corpo.extend_from_slice(&i32::try_from(previsto).unwrap_or(0).to_be_bytes());
            corpo.extend_from_slice(&nota.to_be_bytes());
        }
        let ops = u64::try_from(pedaco.len()).unwrap_or(0).saturating_mul(OPERACOES_POR_MOLECULA);
        if !continuar(ops) {
            return Err(ErroDeTrabalho::Cancelado);
        }
    }
    let mut resultado = Vec::with_capacity(tamanho_do_resultado(esp.tamanho()));
    resultado.extend_from_slice(&u32::try_from(l.inicio).unwrap_or(0).to_be_bytes());
    resultado.extend_from_slice(&esp.tamanho().to_be_bytes());
    resultado.extend_from_slice(&aprovadas.to_be_bytes());
    resultado.extend_from_slice(&corpo);
    Ok(Execucao { resultado, operacoes: operacoes(esp), curva: Vec::new() })
}

/// Uma linha do resultado: `(máscara, log S previsto em milésimos, nota)`.
pub type Linha = (u8, i32, u32);

/// O resultado lido de volta: `(início, aprovadas, linhas)`, ou `None` se o
/// tamanho não fecha.
pub fn decodificar(resultado: &[u8]) -> Option<(u32, u32, Vec<Linha>)> {
    let cabecalho = resultado.get(..BYTES_CABECALHO)?;
    let palavra = |k: usize| cabecalho.get(k..k.saturating_add(4)).and_then(|b| <[u8; 4]>::try_from(b).ok()).map(u32::from_be_bytes);
    let (inicio, quantidade, aprovadas) = (palavra(0)?, palavra(4)?, palavra(8)?);
    let corpo = resultado.get(BYTES_CABECALHO..)?;
    if corpo.len() != usize::try_from(quantidade).ok()?.checked_mul(BYTES_POR_MOLECULA)? {
        return None;
    }
    let linhas = corpo
        .as_chunks::<BYTES_POR_MOLECULA>()
        .0
        .iter()
        .map(|&[mascara, p0, p1, p2, p3, n0, n1, n2, n3]| (mascara, i32::from_be_bytes([p0, p1, p2, p3]), u32::from_be_bytes([n0, n1, n2, n3])))
        .collect();
    Some((inicio, aprovadas, linhas))
}

/// Confere um resultado por recomputação. Mesma convenção de
/// [`crate::trabalho::verificar_controlado`].
pub fn verificar(
    esp: &Especificacao,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    if resultado.len() != tamanho_do_resultado(esp.tamanho()) {
        return Ok(Err(Recusa("resultado com tamanho que não fecha a faixa de moléculas".into())));
    }
    let refeito = executar(esp, semente, continuar)?.resultado;
    if refeito.get(..BYTES_CABECALHO) != resultado.get(..BYTES_CABECALHO) {
        return Ok(Err(Recusa("o cabeçalho (faixa ou aprovadas) difere da recomputação".into())));
    }
    let inicio = esp.parametros().get(1).copied().unwrap_or(0);
    let corpo = |r: &[u8]| r.get(BYTES_CABECALHO..).unwrap_or(&[]).as_chunks::<BYTES_POR_MOLECULA>().0.to_vec();
    Ok(match corpo(&refeito).iter().zip(corpo(resultado)).position(|(a, b)| *a != b) {
        Some(k) => Err(Recusa(format!(
            "a molécula {} do catálogo difere da recomputação",
            u64::from(inicio).saturating_add(u64::try_from(k).unwrap_or(0))
        ))),
        None => Ok(()),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    fn esp(tamanho: u32, inicio: u32) -> Especificacao {
        // Lipinski (massa 500, logP 5, doadores 5, aceptores 10), Veber (10 giratórias, TPSA 140), log S >= -4
        Especificacao::nova_com(TipoDeTrabalho::Triagem, tamanho, 0, &[0, inicio, 500_000, 25_000, 5, 10, 10, 140_000, 16_000]).unwrap()
    }

    #[test]
    fn catalogo_e_modelo_embutidos_se_leem() {
        let c = catalogo();
        assert!(c.moleculas.len() > 8000);
        assert!(modelo().is_some());
        // as 2048 primeiras são as da base da IA, na mesma ordem
        let base = ia::Base::embutida();
        for (a, b) in c.moleculas.iter().zip(&base.moleculas) {
            assert_eq!((&a.id, a.x), (&b.id, b.x));
        }
    }

    #[test]
    fn faixa_fora_do_catalogo_e_parametros_errados_sao_recusados() {
        let n = u32::try_from(catalogo().moleculas.len()).unwrap();
        assert!(Especificacao::nova_com(TipoDeTrabalho::Triagem, 10, 0, &[0, n - 5, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Triagem, 10, 0, &[1, 0, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Triagem, 10, 0, &[0, 0, 0, 40_001, 0, 0, 0, 0, 0]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Triagem, 10, 1, &[0; 9]).is_err());
    }

    #[test]
    fn adulteracao_e_recusada_e_cancelamento_nao_julga() {
        let e = esp(100, 0);
        let mut r = executar(&e, b"", &mut |_| true).unwrap().resultado;
        assert!(verificar(&e, b"", &r, &mut |_| true).unwrap().is_ok());
        r[BYTES_CABECALHO + 5 * BYTES_POR_MOLECULA] ^= 1;
        let recusa = verificar(&e, b"", &r, &mut |_| true).unwrap().unwrap_err();
        assert!(recusa.0.contains("molécula 5"), "{recusa}");
        assert!(matches!(executar(&e, b"", &mut |_| false), Err(ErroDeTrabalho::Cancelado)));
    }

    #[test]
    fn unidades_andam_pela_faixa_e_a_ultima_e_cortada() {
        let n = u64::try_from(catalogo().moleculas.len()).unwrap();
        let modelo_do_job = esp(1000, 0);
        let ultima = (n - 1) / 1000;
        let u = derivar_unidade(&modelo_do_job, ultima).unwrap();
        assert_eq!(u64::from(u.parametros()[1]), ultima * 1000);
        assert_eq!(u64::from(u.tamanho()), n - ultima * 1000);
        assert!(derivar_unidade(&modelo_do_job, ultima + 1).is_err());
    }
}
