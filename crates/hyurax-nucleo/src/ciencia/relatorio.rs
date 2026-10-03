// ✝ Habacuque 2:2 — “Escreve a visão, e torna-a bem legível sobre tábuas.”
//! Relatório científico de um JOB: JSON, CSV e PDF.
//!
//! Tudo o que entra aqui já existe em disco ou na memória do agendador: a
//! especificação, o checkpoint, o registro por unidade (`unidades.jsonl`) e
//! os eventos. Nada é estimado para "completar" o relatório; o que não foi
//! medido aparece como não medido.
//!
//! - **JSON**: o relatório inteiro, com método, verificação, reprodutibilidade,
//!   hashes, incertezas e erros.
//! - **CSV**: uma linha por unidade conferida, direto do registro.
//! - **PDF**: o resumo em texto, sem gráficos, gerado aqui mesmo (sem
//!   biblioteca), com a fonte Helvetica e acentos em WinAnsi.

use std::fmt::Write as _;

use hyurax_codec::merkle_root;
use hyurax_crypto::HASH_LEN;
use hyurax_ultrax::job::{DOMINIO_UNIDADE, Nivel};
use hyurax_ultrax::trabalho::TipoDeTrabalho;

use crate::ciencia::{Ciencia, EstadoDoJob, Job};
use crate::util::texto_json;
use crate::util::{de_hex, hex};

/// Grava `relatorio.json`, `relatorio.csv` e `relatorio.pdf` na pasta do JOB.
pub fn gravar(c: &Ciencia, id: &[u8; HASH_LEN]) -> Result<(), String> {
    let pasta = c.pasta_do_job(id);
    for formato in ["json", "csv", "pdf"] {
        let (bytes, _) = gerar(c, id, formato).ok_or("JOB não encontrado")?;
        std::fs::write(pasta.join(format!("relatorio.{formato}")), bytes).map_err(|e| format!("não consegui gravar o relatório: {e}"))?;
    }
    Ok(())
}

/// O relatório num formato (`json`, `csv` ou `pdf`), com o tipo de conteúdo.
pub fn gerar(c: &Ciencia, id: &[u8; HASH_LEN], formato: &str) -> Option<(Vec<u8>, &'static str)> {
    // o registro por unidade pode estar no buffer: vai para o disco antes de ler
    c.esvaziar(false);
    let jobs = c.jobs.lock().ok()?;
    let job = jobs.iter().find(|j| &j.id == id)?;
    let unidades = std::fs::read_to_string(c.pasta_do_job(id).join("unidades.jsonl")).unwrap_or_default();
    match formato {
        "json" => Some((json(c, job, &unidades).into_bytes(), "application/json; charset=utf-8")),
        "csv" => Some((csv(&unidades).into_bytes(), "text/csv; charset=utf-8")),
        "pdf" => Some((pdf(&linhas_de_texto(c, job, &unidades)), "application/pdf")),
        _ => None,
    }
}

/// O valor cru de uma chave numa linha JSON gravada pelo agendador (formato
/// controlado: números, `null` e textos sem aspas escapadas nos campos lidos
/// aqui).
fn campo<'a>(linha: &'a str, chave: &str) -> Option<&'a str> {
    let marca = format!("\"{chave}\":");
    let resto = linha.get(linha.find(&marca)?.saturating_add(marca.len())..)?;
    if let Some(texto) = resto.strip_prefix('"') {
        return texto.split('"').next();
    }
    resto.split([',', '}']).next().map(str::trim)
}

/// Raiz de Merkle dos resultados registrados, na ordem do índice: folha =
/// `u64 índice || RESULT_HASH`.
fn raiz_das_unidades(unidades: &str) -> ([u8; HASH_LEN], u64) {
    let mut folhas: Vec<(u64, [u8; HASH_LEN])> = unidades
        .lines()
        .filter_map(|l| Some((campo(l, "indice")?.parse::<u64>().ok()?, de_hex(campo(l, "resultado")?)?)))
        .collect();
    folhas.sort_by_key(|&(i, _)| i);
    folhas.dedup_by_key(|&mut (i, _)| i);
    let bytes: Vec<Vec<u8>> = folhas
        .iter()
        .map(|(i, h)| {
            let mut f = i.to_be_bytes().to_vec();
            f.extend_from_slice(h);
            f
        })
        .collect();
    (merkle_root(&bytes), u64::try_from(folhas.len()).unwrap_or(u64::MAX))
}

/// O que cada motor não sabe, para o relatório dizer.
fn incertezas(tipo: TipoDeTrabalho) -> &'static [&'static str] {
    match tipo {
        TipoDeTrabalho::Matriz => &["Resultado exato em inteiros de 64 bits: não há erro numérico.", "As matrizes são sorteadas pela semente: é carga de verificação, não dado de alguém."],
        TipoDeTrabalho::Difusao => &["Difusão em ponto fixo com passo explícito: é um modelo numérico simples, sem validação contra medida física."],
        TipoDeTrabalho::Mochila => &["Ótimo exato de cada instância, por programação dinâmica.", "As instâncias são sorteadas pela semente: não são dados de alocação real."],
        TipoDeTrabalho::Ia => &[
            "A rede é pequena (10 → 16 → 1). O erro dela nas moléculas de validação é o medido neste JOB (no resultado consolidado), não um número fixo; e essas moléculas são as usadas para escolher o melhor treino, então o erro em moléculas novas tende a ser maior.",
            "Prever solubilidade é uma etapa de triagem; não descobre remédio.",
        ],
        TipoDeTrabalho::Genetica => &["Modelo Wright-Fisher com os parâmetros de quem pediu: é um modelo estatístico, não uma previsão sobre uma população real."],
        TipoDeTrabalho::Melhoramento => &["Modelo didático de melhoramento com QTL aditivos e ambiente simplificado: não prevê safra real."],
        TipoDeTrabalho::Rotas => &["2-opt chega a um ótimo local, que pode não ser o ótimo global da instância."],
        TipoDeTrabalho::Triagem => &[
            "Descritores e log S medido vêm da AqSolDB; o log S previsto vem de um modelo com erro típico conhecido (ver o motor).",
            "Afinidade com alvo, estabilidade e toxicidade não são calculadas: não há modelo para isso no programa.",
        ],
    }
}

/// O nível de verificação que o JOB atingiu de fato, lido do registro por
/// unidade: 3 só se toda unidade registrada passou por consenso entre nós.
fn nivel_atingido(job: &Job, unidades: &str) -> (u8, &'static str) {
    if job.feitas.concluidas() == 0 {
        return (0, "nenhuma unidade conferida ainda");
    }
    let linhas: Vec<&str> = unidades.lines().filter(|l| !l.trim().is_empty()).collect();
    if !linhas.is_empty() && linhas.iter().all(|l| campo(l, "verificacao").is_some_and(|v| v.starts_with("CONSENSUS"))) {
        return (3, "concordância por maioria entre workers de nós diferentes, com o resultado da maioria refeito aqui antes de entrar");
    }
    if job.workers.len() >= 2 {
        return (2, "reexecução por quem pediu; workers de mais de um nó, mas nem toda unidade passou por consenso entre nós");
    }
    match job.esp.modelo().tipo() {
        TipoDeTrabalho::Matriz => (2, "reexecução nesta máquina, por algoritmo diferente (Freivalds); workers de um nó só"),
        _ => (2, "reexecução nesta máquina, pelo mesmo código; workers de um nó só"),
    }
}

fn json(c: &Ciencia, job: &Job, unidades: &str) -> String {
    let esp = &job.esp;
    let modelo = esp.modelo();
    let (raiz, registradas) = raiz_das_unidades(unidades);
    let (atingido, como) = nivel_atingido(job, unidades);
    // as versões do programa que calcularam as unidades (não a de quem gera o relatório)
    let mut versoes: Vec<&str> = unidades.lines().filter_map(|l| campo(l, "programa")).collect();
    versoes.sort_unstable();
    versoes.dedup();
    let versoes: Vec<String> = versoes.iter().map(|v| texto_json(v)).collect();
    let falhas: Vec<String> = job.abandonadas.faixas().iter().take(100).map(|(a, b)| format!("[{a},{b}]")).collect();
    let parametros: Vec<String> = modelo.parametros().iter().map(u32::to_string).collect();
    let workers: Vec<String> = job.workers.iter().map(|w| format!("\"{}\"", hex(w))).collect();
    let duracao = if job.fim_ms > job.inicio_ms && job.inicio_ms > 0 { job.fim_ms.saturating_sub(job.inicio_ms) } else { 0 };
    let incerteza: Vec<String> = incertezas(modelo.tipo()).iter().map(|t| texto_json(t)).collect();
    let mut j = String::new();
    let _ = write!(
        j,
        "{{\"relatorio\":\"hyurax-ciencia-v1\",\"programa\":\"{}\",\"programas_que_calcularam\":[{}],\"job_id\":\"{}\",\"estado\":\"{}\",\"motivo\":{},\
         \"aviso\":{},\
         \"entrada\":{{\"dominio\":\"{}\",\"descricao\":{},\"motor\":\"{}\",\"motor_codigo\":{},\"descricao_motor\":{},\
         \"tamanho\":{},\"passos\":{},\"parametros\":[{}],\"unidades\":{},\"prazo_s\":{},\"orcamento_milicreditos\":{}}},\
         \"metodo\":{{\"verificacao\":{},\"nivel_pedido\":{},\"nivel_pedido_nome\":{},\"nivel_atingido\":{atingido},\"como\":{},\"redundancia\":{}}},\
         \"unidades\":{{\"total\":{},\"conferidas\":{},\"falhas\":{},\"faixas_com_falha\":[{}],\"recusas\":{},\"repetidas\":{}}},\
         \"nos\":[{}],\
         \"tempo\":{{\"criado_ms\":{},\"inicio_ms\":{},\"fim_ms\":{},\"duracao_ms\":{duracao},\"cpu_ms\":{},\"gpu_ms\":{}}},\
         \"consumo\":{{\"operacoes\":{},\"operacoes_verificacao\":{},\"memoria_mib_s\":{},\"bytes_rede\":{},\"milicreditos\":{},\"formula\":\"creditos v{}: 1 credito = 10^9 operacoes executadas e conferidas\",\"liquidacao\":\"nenhuma: credito nao vira HYX\"}},\
         \"resultado\":{{\"consolidado\":{}}},\
         \"incertezas\":[{}],\
         \"reprodutibilidade\":{{\"motor\":\"{}\",\"semente_da_unidade\":\"H(DOMINIO_UNIDADE || JOB_ID || u64 indice)\",\"dominio_unidade\":{},\
         \"comando\":\"hyurax-no ciencia refazer --pasta PASTA --job {} --unidade INDICE\",\"regra\":\"inteiros; o mesmo resultado bit a bit em qualquer maquina\"}},\
         \"hashes\":{{\"resumo_aditivo\":\"{}\",\"raiz_merkle_das_unidades_registradas\":\"{}\",\"unidades_registradas\":{registradas},\"registro_cortado\":{}}}}}",
        c.versao(),
        versoes.join(","),
        hex(&job.id),
        job.estado.nome(),
        texto_json(&job.motivo),
        texto_json("Resultado computacional. Consenso entre nós quer dizer que chegaram ao mesmo número, não que o modelo esteja certo sobre o mundo: precisa de validação científica."),
        esp.dominio().nome(),
        texto_json(esp.descricao()),
        modelo.tipo().nome(),
        modelo.tipo().codigo(),
        texto_json(modelo.tipo().descricao()),
        modelo.tamanho(),
        modelo.passos(),
        parametros.join(","),
        esp.unidades(),
        esp.prazo_s(),
        esp.orcamento_milicreditos(),
        texto_json(modelo.tipo().metodo().nome()),
        esp.nivel().codigo(),
        texto_json(esp.nivel().nome()),
        texto_json(como),
        esp.redundancia(),
        esp.unidades(),
        job.feitas.concluidas(),
        job.abandonadas.concluidas(),
        falhas.join(","),
        job.recusas,
        job.repetidas,
        workers.join(","),
        job.criado_ms,
        job.inicio_ms,
        job.fim_ms,
        job.consumo.cpu_ms,
        job.consumo.gpu_ms,
        job.consumo.operacoes,
        job.consumo.operacoes_verificacao,
        job.consumo.memoria_mib_s,
        job.consumo.bytes_rede,
        job.consumo.milicreditos(),
        hyurax_ultrax::job::CREDITOS_VERSAO,
        texto_json(&job.agregador.texto()),
        incerteza.join(","),
        modelo.tipo().nome(),
        texto_json(&String::from_utf8_lossy(DOMINIO_UNIDADE)),
        hex(&job.id),
        hex(&job.resumo.bytes()),
        hex(&raiz),
        job.registro_cortado,
    );
    j
}

fn csv(unidades: &str) -> String {
    let colunas = [
        "indice", "tarefa", "worker", "linha", "despachada", "inicio", "fim", "verificada", "entrada", "resultado", "operacoes",
        "operacoes_verificacao", "ms_calculo", "ms_verificacao", "memoria", "gpu", "verificacao", "metodo",
    ];
    let mut saida = colunas.join(",");
    saida.push('\n');
    // uma linha por unidade: depois de uma queda entre o registro e o
    // checkpoint, a unidade é refeita e aparece duas vezes no registro; vale
    // a última
    let mut por_indice: std::collections::BTreeMap<u64, &str> = std::collections::BTreeMap::new();
    let mut sem_indice: Vec<&str> = Vec::new();
    for linha in unidades.lines().filter(|l| !l.trim().is_empty()) {
        match campo(linha, "indice").and_then(|v| v.parse::<u64>().ok()) {
            Some(i) => {
                por_indice.insert(i, linha);
            }
            None => sem_indice.push(linha),
        }
    }
    for linha in por_indice.into_values().chain(sem_indice) {
        let valores: Vec<String> = colunas
            .iter()
            .map(|k| {
                let v = campo(linha, k).unwrap_or("");
                let v = if v == "null" { "" } else { v };
                if v.contains([',', '"', '\n']) { format!("\"{}\"", v.replace('"', "\"\"")) } else { v.to_string() }
            })
            .collect();
        saida.push_str(&valores.join(","));
        saida.push('\n');
    }
    saida
}

// Contas de calendário (Howard Hinnant, "days_from_civil" ao contrário) com
// dias desde 1970 cabendo em i64 com folga enorme: não há como estourar.
#[allow(clippy::arithmetic_side_effects)]
fn data(ms: u64) -> String {
    if ms == 0 {
        return "-".into();
    }
    // dias desde 1970 para ano-mês-dia (UTC), sem biblioteca
    let segundos = ms / 1000;
    let dias = i64::try_from(segundos / 86_400).unwrap_or(0);
    let resto = segundos % 86_400;
    let z = dias.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + era * 400 + i64::from(m <= 2);
    format!("{a:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC", resto / 3600, (resto / 60) % 60, resto % 60)
}

fn linhas_de_texto(c: &Ciencia, job: &Job, unidades: &str) -> Vec<String> {
    let esp = &job.esp;
    let modelo = esp.modelo();
    let (raiz, registradas) = raiz_das_unidades(unidades);
    let (atingido, como) = nivel_atingido(job, unidades);
    let mut l = vec![
        "HYURAX - RELATORIO DE COMPUTACAO CIENTIFICA".to_string(),
        format!("Programa {} - gerado a partir dos arquivos do JOB", c.versao()),
        String::new(),
        "Resultado computacional. Precisa de validação científica.".into(),
        "Consenso entre nós quer dizer que chegaram ao mesmo número, não que o modelo esteja certo.".into(),
        String::new(),
        format!("JOB {}", hex(&job.id).get(..64).unwrap_or_default()),
        format!("    {}", hex(&job.id).get(64..).unwrap_or_default()),
        format!("Estado: {}{}", job.estado.nome(), if job.motivo.is_empty() { String::new() } else { format!(" - {}", job.motivo) }),
        String::new(),
        "ENTRADA".into(),
        format!("Domínio: {}", esp.dominio().nome()),
        format!("Motor: {} ({})", modelo.tipo().descricao(), modelo.tipo().nome()),
        format!("Especificação: {}; parâmetros {:?}", modelo.resumo(), modelo.parametros()),
        format!("Unidades: {}", esp.unidades()),
    ];
    for trecho in esp.descricao().lines() {
        l.push(format!("Pedido: {trecho}"));
    }
    l.extend([
        String::new(),
        "MÉTODO E VERIFICAÇÃO".into(),
        format!("Conferência: {}", modelo.tipo().metodo().nome()),
        format!("Nível pedido: {} ({}); redundância {}", esp.nivel().codigo(), esp.nivel().nome(), esp.redundancia()),
        format!("Nível atingido: {atingido} - {como}"),
        String::new(),
        "UNIDADES".into(),
        format!("Conferidas: {} de {}; falhas: {}; recusas: {}; repetidas: {}", job.feitas.concluidas(), esp.unidades(), job.abandonadas.concluidas(), job.recusas, job.repetidas),
        format!("Nós que calcularam: {}", job.workers.len()),
        String::new(),
        "TEMPO E CONSUMO".into(),
        format!("Criado: {}  Início: {}  Fim: {}", data(job.criado_ms), data(job.inicio_ms), data(job.fim_ms)),
        format!("CPU: {:.1} s  GPU: {:.1} s", job.consumo.cpu_ms as f64 / 1000.0, job.consumo.gpu_ms as f64 / 1000.0),
        format!("Operações: {} executadas, {} na conferência", job.consumo.operacoes, job.consumo.operacoes_verificacao),
        format!("Créditos (v1): {:.3}; liquidação: nenhuma (crédito não vira HYX)", job.consumo.milicreditos() as f64 / 1000.0),
        String::new(),
        "RESULTADO CONSOLIDADO".into(),
        job.agregador.texto(),
        String::new(),
        "INCERTEZAS".into(),
    ]);
    l.extend(incertezas(modelo.tipo()).iter().map(|t| format!("- {t}")));
    l.extend([
        String::new(),
        "REPRODUTIBILIDADE".into(),
        "Semente da unidade i: H(DOMINIO_UNIDADE || JOB_ID || u64 i); tudo em inteiros.".into(),
        "Refazer uma unidade: hyurax-no ciencia refazer --pasta PASTA --job <JOB_ID> --unidade <i>".into(),
        String::new(),
        "HASHES".into(),
        format!("Resumo aditivo: {}", hex(&job.resumo.bytes()).get(..64).unwrap_or_default()),
        format!("Raiz de Merkle de {registradas} unidade(s) registradas: {}", hex(&raiz).get(..64).unwrap_or_default()),
    ]);
    if job.registro_cortado {
        l.push("O registro por unidade parou no teto de disco; o resumo aditivo cobre todas as unidades.".into());
    }
    if matches!(job.estado, EstadoDoJob::AguardandoNos) || esp.nivel() >= Nivel::Concordancia {
        l.push(String::new());
        l.push("Este JOB pede concordância entre nós: sem outros nós, não é mostrado como conferido por eles.".into());
    }
    l
}

/// Um caractere em WinAnsi (cp1252), ou `?`.
fn winansi(c: char) -> u8 {
    let u = u32::from(c);
    match u {
        0x20..=0x7E | 0xA0..=0xFF => u8::try_from(u).unwrap_or(b'?'),
        0x2013 => 0x96,
        0x2014 => 0x97,
        0x2018 => 0x91,
        0x2019 => 0x92,
        0x201C => 0x93,
        0x201D => 0x94,
        0x2022 => 0x95,
        0x2192 => b'>',
        _ => b'?',
    }
}

/// Um PDF de texto, Helvetica 9 pt em A4, quantas páginas precisar.
fn pdf(linhas: &[String]) -> Vec<u8> {
    const POR_PAGINA: usize = 70;
    const LARGURA_MAX: usize = 110;
    // quebra linhas longas
    let mut quebradas: Vec<Vec<u8>> = Vec::new();
    for linha in linhas {
        let bytes: Vec<u8> = linha.chars().map(winansi).collect();
        if bytes.is_empty() {
            quebradas.push(Vec::new());
        }
        for pedaco in bytes.chunks(LARGURA_MAX) {
            quebradas.push(pedaco.to_vec());
        }
    }
    let paginas: Vec<&[Vec<u8>]> = quebradas.chunks(POR_PAGINA).collect();
    let n = paginas.len().max(1);
    let mut objetos: Vec<Vec<u8>> = Vec::new();
    // 1: catálogo, 2: páginas, 3: fonte; depois (página, conteúdo) por página
    objetos.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let filhos: Vec<String> = (0..n).map(|k| format!("{} 0 R", 4usize.saturating_add(k.saturating_mul(2)))).collect();
    objetos.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", filhos.join(" ")).into_bytes());
    objetos.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
    for (k, pagina) in paginas.iter().enumerate() {
        let conteudo_obj = 5usize.saturating_add(k.saturating_mul(2));
        objetos.push(
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R >> >> /Contents {conteudo_obj} 0 R >>")
                .into_bytes(),
        );
        let mut fluxo = b"BT /F1 9 Tf 40 800 Td 11 TL\n".to_vec();
        for linha in *pagina {
            fluxo.push(b'(');
            for &b in linha {
                if matches!(b, b'(' | b')' | b'\\') {
                    fluxo.push(b'\\');
                }
                fluxo.push(b);
            }
            fluxo.extend_from_slice(b") Tj T*\n");
        }
        fluxo.extend_from_slice(format!("ET\nBT /F1 7 Tf 40 30 Td (Hyurax - pagina {} de {n}) Tj ET\n", k.saturating_add(1)).as_bytes());
        let mut obj = format!("<< /Length {} >>\nstream\n", fluxo.len()).into_bytes();
        obj.extend_from_slice(&fluxo);
        obj.extend_from_slice(b"\nendstream");
        objetos.push(obj);
    }
    let mut saida = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut posicoes = Vec::with_capacity(objetos.len());
    for (k, obj) in objetos.iter().enumerate() {
        posicoes.push(saida.len());
        saida.extend_from_slice(format!("{} 0 obj\n", k.saturating_add(1)).as_bytes());
        saida.extend_from_slice(obj);
        saida.extend_from_slice(b"\nendobj\n");
    }
    let inicio_xref = saida.len();
    saida.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len().saturating_add(1)).as_bytes());
    for p in posicoes {
        saida.extend_from_slice(format!("{p:010} 00000 n \n").as_bytes());
    }
    saida.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio_xref}\n%%EOF\n", objetos.len().saturating_add(1)).as_bytes());
    saida
}

/// Confere que um PDF gerado aqui tem o esqueleto certo.
#[cfg(test)]
pub fn pdf_parece_valido(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-1.4") && bytes.ends_with(b"%%EOF\n") && bytes.windows(4).any(|w| w == b"xref")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    #[test]
    fn campo_le_numeros_textos_e_null() {
        let l = "{\"indice\":7,\"tarefa\":\"ab\",\"gpu\":null,\"metodo\":\"Freivalds\"}";
        assert_eq!(campo(l, "indice"), Some("7"));
        assert_eq!(campo(l, "tarefa"), Some("ab"));
        assert_eq!(campo(l, "gpu"), Some("null"));
        assert_eq!(campo(l, "metodo"), Some("Freivalds"));
        assert_eq!(campo(l, "falta"), None);
    }

    #[test]
    fn pdf_tem_xref_e_acentos_em_winansi() {
        let bytes = pdf(&vec!["Relatório (teste) \\ ação".to_string(); 150]);
        assert!(pdf_parece_valido(&bytes));
        assert!(bytes.windows(8).any(|w| w == b"/Count 3"), "150 linhas em 3 páginas");
        assert!(bytes.contains(&0xF3), "ó em WinAnsi");
        assert!(bytes.windows(2).any(|w| w == b"\\("), "parêntese escapado");
    }

    #[test]
    fn data_em_utc() {
        assert_eq!(data(0), "-");
        assert_eq!(data(1_790_467_366_000), "2026-09-27 00:02:46 UTC", "conferido com o datetime do Python");
    }

    #[test]
    fn raiz_nao_depende_da_ordem_das_linhas() {
        let a = "{\"indice\":1,\"resultado\":\"".to_string() + &"11".repeat(64) + "\"}\n{\"indice\":0,\"resultado\":\"" + &"22".repeat(64) + "\"}";
        let b = "{\"indice\":0,\"resultado\":\"".to_string() + &"22".repeat(64) + "\"}\n{\"indice\":1,\"resultado\":\"" + &"11".repeat(64) + "\"}";
        assert_eq!(raiz_das_unidades(&a), raiz_das_unidades(&b));
        assert_eq!(raiz_das_unidades(&a).1, 2);
    }
}
