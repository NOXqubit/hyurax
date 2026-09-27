// ✝ Gênesis 1:24 — “Produza a terra alma vivente conforme a sua espécie.”
//! Genética de populações: Wright-Fisher com deriva, seleção, dominância e
//! mutação, em loci independentes.
//!
//! Tradução de `reference/hyurax/genetica.py`, conferida byte a byte contra
//! `vectors/genetica.json`. O desenho está em `docs/COMPUTACAO-CIENTIFICA.md`,
//! seção 2.3.
//!
//! **O que é:** o modelo estatístico clássico da genética de populações, com
//! os parâmetros de quem pediu. Cada locus é uma réplica independente do mesmo
//! processo, e o resultado é a trajetória inteira das contagens de alelos.
//!
//! **O que não é:** previsão do comportamento de uma população real. O modelo
//! supõe população de tamanho fixo, gerações que não se sobrepõem, cruzamento
//! ao acaso (Hardy-Weinberg), loci sem ligação e um só par de alelos por
//! locus. Os números valem para o modelo, não para um organismo. Nada aqui é
//! descoberta: é resultado computacional, que precisa de validação científica.
//!
//! # Especificação (tipo 5)
//!
//! `tamanho` = N indivíduos diploides (M = 2N alelos por locus), `passos` = G
//! gerações, e cinco parâmetros:
//!
//! 0. loci L, `1..=64`;
//! 1. mutação µ em partes por milhão, `0..=10_000`: por alelo e por geração,
//!    simétrica (A vira a e a vira A com a mesma taxa);
//! 2. seleção codificada como `s·10⁴ + 10_000`, `0..=20_000` (s em `[-1, +1]`);
//! 3. dominância h em %, `0..=100`;
//! 4. frequência inicial p0 por 10⁴, `0..=10_000`.
//!
//! Aptidões pelo número de alelos A, na escala 10⁶, com `sel = s·10⁴ + 10⁴`:
//! `w2 = 100·sel` (AA), `w1 = 10⁴·(100 - h) + h·sel` (Aa), `w0 = 10⁶` (aa).
//! Inteiras e nunca negativas. Contagem inicial, igual em todos os loci:
//! `k0 = ⌊(M·p0 + 5000) / 10⁴⌋`.
//!
//! Uma geração, em cada locus, com k alelos A e `j = M - k`:
//!
//! 1. seleção diploide sobre Hardy-Weinberg, em fração exata:
//!    `num = k²·w2 + k·j·w1`, `den = k²·w2 + 2·k·j·w1 + j²·w0`. `den = 0` só
//!    com a população toda AA e AA letal; aí a seleção não muda p
//!    (`num/den = k/M`);
//! 2. mutação, ainda exata: `a = num·(10⁶ - µ) + (den - num)·µ`,
//!    `b = den·10⁶`;
//! 3. um único arredondamento para Q32, com a metade para cima:
//!    `P = ⌊(a·2³² + ⌊b/2⌋) / b⌋`, sempre em `[0, 2³²]`;
//! 4. amostragem binomial: M sorteios u32, e cada sorteio `u < P` é um alelo
//!    A. `P = 0` nunca dá A e `P = 2³²` sempre dá: perder e fixar (sem
//!    mutação) são absorventes pela própria regra.
//!
//! Sorteios: [`splitmix64`] por contador, com o estado inicial tirado da
//! semente pelo XOF ([`DOMINIO_SORTEIO`]). Cada saída dá dois sorteios u32,
//! primeiro os 32 bits altos. A geração g (0 a G-1) no locus l usa as N saídas
//! a partir de `n = (g·L + l)·N`. Como a saída n só depende de n, um locus
//! fixado pula os sorteios sem mudar os outros.
//!
//! # Resultado
//!
//! Inteiros big-endian: `u8 versão (1) || u32 N || u32 G || u8 L || u32 k0 ||
//! G·L × u32`, as contagens de A depois de cada geração, na ordem geração e
//! depois locus. Conferência por recomputação. Operações declaradas: 2N·L·G
//! sorteios, mesmo os que um locus fixado pula.

use hyurax_crypto::xof;

use crate::trabalho::{ErroDeTrabalho, Especificacao, Execucao, Recusa, TipoDeTrabalho};

/// Quantos parâmetros extras a especificação leva.
pub const PARAMETROS: usize = 5;
/// Faixa aceita de `tamanho` (indivíduos diploides).
pub const TAMANHO: (u32, u32) = (2, 100_000);
/// Faixa aceita de `passos` (gerações).
pub const PASSOS: (u32, u32) = (1, 10_000);
/// Teto de loci por unidade. `L` vai num `u8` do resultado, e 64 réplicas
/// por unidade bastam: mais réplicas são mais unidades.
pub const LOCI_MAX: u32 = 64;
/// Maior taxa de mutação, em partes por milhão (1%).
pub const MUTACAO_MAX_PPM: u32 = 10_000;
/// Código da seleção neutra (s = 0).
pub const SELECAO_ZERO: u32 = 10_000;
/// Maior código de seleção (s = +1).
pub const SELECAO_MAX: u32 = 20_000;
/// Maior dominância, em %.
pub const DOMINANCIA_MAX: u32 = 100;
/// Escala da frequência inicial: 10⁴ é 1,0.
pub const P0_ESCALA: u32 = 10_000;
/// Teto de custo: 2³³ sorteios por unidade. Medido em release num Atom
/// x5-Z8350, com a máquina ocupada: cerca de 100 milhões de sorteios por
/// segundo, então a maior unidade leva perto de 85 s. Um estudo maior vira
/// várias unidades.
pub const SORTEIOS_MAX: u64 = 1 << 33;
/// Escala das aptidões.
pub const APTIDAO_ESCALA: u64 = 1_000_000;
/// Escala da mutação (partes por milhão).
pub const MUTACAO_ESCALA: u64 = 1_000_000;
/// 1,0 em Q32, o ponto fixo do limiar.
pub const Q32: u64 = 1 << 32;
/// Versão do formato do resultado.
pub const VERSAO: u8 = 1;
/// Bytes do cabeçalho do resultado.
pub const CABECALHO: usize = 1 + 4 + 4 + 1 + 4;
/// Sorteios entre duas chamadas de `continuar` (passa no máximo por uma
/// geração de um locus, 2N <= 2·10⁵). No Atom medido, mediana de 5 ms em
/// release (maior 13 ms) e de 20 ms em debug (maior 41 ms).
pub const LOTE_SORTEIOS: u64 = 1 << 19;

/// Domínio do estado inicial dos sorteios.
pub const DOMINIO_SORTEIO: &[u8] = dominio!("GENETICA-SORTEIO-v1");

const GAMA: u64 = 0x9E37_79B9_7F4A_7C15;
const MISTURA_1: u64 = 0xBF58_476D_1CE4_E5B9;
const MISTURA_2: u64 = 0x94D0_49BB_1331_11EB;

/// Os cinco parâmetros, com nome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parametros {
    /// Loci independentes.
    pub loci: u32,
    /// Mutação por alelo por geração, em partes por milhão.
    pub mutacao_ppm: u32,
    /// `s·10⁴ + 10_000`.
    pub selecao: u32,
    /// Dominância h, em %.
    pub dominancia: u32,
    /// Frequência inicial de A, por 10⁴.
    pub p0: u32,
}

impl Parametros {
    /// Lê os parâmetros na ordem da especificação. Não confere faixas: isso é
    /// o [`validar`].
    pub fn de(parametros: &[u32]) -> Result<Self, ErroDeTrabalho> {
        let [loci, mutacao_ppm, selecao, dominancia, p0] = <[u32; PARAMETROS]>::try_from(parametros).map_err(|_| {
            ErroDeTrabalho::Parametros(format!("a genética leva {PARAMETROS} parâmetros, vieram {}", parametros.len()))
        })?;
        Ok(Self { loci, mutacao_ppm, selecao, dominancia, p0 })
    }
}

/// Aptidões pelo número de alelos A no genótipo, escala 10⁶.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aptidoes {
    /// aa.
    pub w0: u64,
    /// Aa.
    pub w1: u64,
    /// AA.
    pub w2: u64,
}

/// Sorteios de uma unidade: 2N·L·G.
fn sorteios(tamanho: u32, passos: u32, loci: u32) -> u64 {
    u64::from(tamanho).saturating_mul(2).saturating_mul(u64::from(loci)).saturating_mul(u64::from(passos))
}

/// Bytes do resultado: cabeçalho e `G·L` contagens de 4 bytes.
pub fn tamanho_do_resultado(passos: u32, loci: u32) -> usize {
    (passos as usize).saturating_mul(loci as usize).saturating_mul(4).saturating_add(CABECALHO)
}

/// Confere as faixas dos parâmetros e o custo.
///
/// Recusa custo acima de [`SORTEIOS_MAX`] (2³³ sorteios). A memória fica
/// limitada pelas faixas: no máximo 10 000 gerações × 64 loci × 4 bytes,
/// 2,56 MB de resultado.
pub fn validar(tamanho: u32, passos: u32, parametros: &[u32]) -> Result<(), ErroDeTrabalho> {
    if !(TAMANHO.0..=TAMANHO.1).contains(&tamanho) {
        return Err(ErroDeTrabalho::Tamanho {
            tipo: TipoDeTrabalho::Genetica,
            valor: tamanho,
            minimo: TAMANHO.0,
            maximo: TAMANHO.1,
        });
    }
    if !(PASSOS.0..=PASSOS.1).contains(&passos) {
        return Err(ErroDeTrabalho::Passos(passos));
    }
    let p = Parametros::de(parametros)?;
    for (nome, valor, minimo, maximo) in [
        ("loci", p.loci, 1, LOCI_MAX),
        ("mutação (ppm)", p.mutacao_ppm, 0, MUTACAO_MAX_PPM),
        ("seleção (s·10^4 + 10^4)", p.selecao, 0, SELECAO_MAX),
        ("dominância (%)", p.dominancia, 0, DOMINANCIA_MAX),
        ("frequência inicial (por 10^4)", p.p0, 0, P0_ESCALA),
    ] {
        if !(minimo..=maximo).contains(&valor) {
            return Err(ErroDeTrabalho::Parametros(format!("{nome} = {valor} fora da faixa {minimo}..={maximo}")));
        }
    }
    let custo = sorteios(tamanho, passos, p.loci);
    if custo > SORTEIOS_MAX {
        return Err(ErroDeTrabalho::Parametros(format!(
            "custo de {custo} sorteios passa do teto de {SORTEIOS_MAX} (2^33)"
        )));
    }
    Ok(())
}

/// Operações da execução: um sorteio por alelo, por locus, por geração.
pub fn operacoes(esp: &Especificacao) -> u64 {
    sorteios(esp.tamanho(), esp.passos(), esp.parametros().first().copied().unwrap_or(0))
}

/// Operações da verificação: a recomputação inteira.
pub fn operacoes_de_verificacao(esp: &Especificacao) -> u64 {
    operacoes(esp)
}

/// Memória no pico, execução e verificação somadas, em bytes.
///
/// Três resultados (o da execução, guardado até a verificação; o que chegou
/// para conferir; e o refeito) e 64 KiB de folga para as contagens por locus
/// e o resto. O gerador por contador não guarda sorteios.
pub fn memoria_bytes(esp: &Especificacao) -> u64 {
    let loci = esp.parametros().first().copied().unwrap_or(0);
    (tamanho_do_resultado(esp.passos(), loci) as u64).saturating_mul(3).saturating_add(64 * 1024)
}

/// Texto curto para o painel.
pub fn resumo(esp: &Especificacao) -> String {
    match Parametros::de(esp.parametros()) {
        Ok(p) => {
            let sinal = if p.selecao < SELECAO_ZERO { '-' } else { '+' };
            let s = p.selecao.abs_diff(SELECAO_ZERO);
            format!(
                "N {} · {} gerações · {} loci · s {sinal}{},{:04} · h {}% · µ {} ppm · p0 {},{:04}",
                esp.tamanho(),
                esp.passos(),
                p.loci,
                s.wrapping_div(10_000),
                s.wrapping_rem(10_000),
                p.dominancia,
                p.mutacao_ppm,
                p.p0.wrapping_div(10_000),
                p.p0.wrapping_rem(10_000),
            )
        }
        Err(_) => format!("genética de populações, tamanho {}", esp.tamanho()),
    }
}

// ---------------------------------------------------------------------------
// As contas
// ---------------------------------------------------------------------------

/// Aptidões de aa, Aa e AA a partir dos códigos de seleção e dominância.
pub fn aptidoes(selecao: u32, dominancia: u32) -> Aptidoes {
    let sel = u64::from(selecao);
    let h = u64::from(dominancia);
    Aptidoes {
        w0: APTIDAO_ESCALA,
        w1: 10_000u64.saturating_mul(100u64.saturating_sub(h)).saturating_add(h.saturating_mul(sel)),
        w2: sel.saturating_mul(100),
    }
}

/// `k0 = ⌊(2N·p0 + 5000) / 10⁴⌋`.
pub fn contagem_inicial(tamanho: u32, p0: u32) -> u32 {
    let k = u64::from(tamanho)
        .saturating_mul(2)
        .saturating_mul(u64::from(p0))
        .saturating_add(u64::from(P0_ESCALA).wrapping_div(2))
        .checked_div(u64::from(P0_ESCALA))
        .unwrap_or(0);
    u32::try_from(k).unwrap_or(u32::MAX)
}

/// O limiar P em Q32: a frequência de A depois de seleção e mutação, em
/// fração exata, com um único arredondamento (metade para cima).
///
/// Em `u128`, e nenhuma conta chega perto do teto: com M <= 2·10⁵ e aptidões
/// <= 2·10⁶, `den <= M²·2·10⁶ = 8·10¹⁶`, `b <= 8·10²²` e `a·2³² < 2¹¹⁶`.
pub fn limiar(k: u32, m: u32, w: Aptidoes, mutacao_ppm: u32) -> u64 {
    let k = u128::from(k);
    let j = u128::from(m).saturating_sub(k);
    let (w0, w1, w2) = (u128::from(w.w0), u128::from(w.w1), u128::from(w.w2));
    let hetero = k.saturating_mul(j).saturating_mul(w1);
    let mut num = k.saturating_mul(k).saturating_mul(w2).saturating_add(hetero);
    let mut den = num.saturating_add(hetero).saturating_add(j.saturating_mul(j).saturating_mul(w0));
    if den == 0 {
        // a população toda AA, e AA letal: sem outro alelo, a seleção não mexe
        num = k;
        den = u128::from(m);
    }
    let mu = u128::from(mutacao_ppm);
    let escala = u128::from(MUTACAO_ESCALA);
    let a = num
        .saturating_mul(escala.saturating_sub(mu))
        .saturating_add(den.saturating_sub(num).saturating_mul(mu));
    let b = den.saturating_mul(escala);
    let p = a.saturating_mul(u128::from(Q32)).saturating_add(b.wrapping_shr(1)).checked_div(b).unwrap_or(0);
    u64::try_from(p).unwrap_or(Q32)
}

/// Estado inicial do fluxo: 8 bytes big-endian do XOF da semente.
pub fn estado_inicial(semente: &[u8]) -> u64 {
    xof(semente, 8, DOMINIO_SORTEIO).first_chunk::<8>().map_or(0, |c| u64::from_be_bytes(*c))
}

fn misturar(z: u64) -> u64 {
    let z = (z ^ z.wrapping_shr(30)).wrapping_mul(MISTURA_1);
    let z = (z ^ z.wrapping_shr(27)).wrapping_mul(MISTURA_2);
    z ^ z.wrapping_shr(31)
}

/// A saída número `n` do splitmix64 de estado inicial `s0`:
/// `misturar(s0 + (n + 1)·γ)`, tudo módulo 2⁶⁴.
pub fn splitmix64(s0: u64, n: u64) -> u64 {
    misturar(s0.wrapping_add(n.wrapping_add(1).wrapping_mul(GAMA)))
}

/// Alelos A em `2n` sorteios a partir da saída `inicio`: cada saída dá o
/// sorteio dos 32 bits altos e o dos 32 baixos, e `u < limiar` é A.
///
/// `limiar = 0` e `limiar = 2³²` não sorteiam: o resultado seria 0 e `2n`
/// de qualquer jeito, e o contador deixa pular sem mexer no resto do fluxo.
pub fn contar(s0: u64, inicio: u64, n: u32, limiar: u64) -> u32 {
    if limiar == 0 {
        return 0;
    }
    if limiar >= Q32 {
        return n.saturating_mul(2);
    }
    let mut z = s0.wrapping_add(inicio.wrapping_add(1).wrapping_mul(GAMA));
    let mut c = 0u32;
    for _ in 0..n {
        let x = misturar(z);
        // c <= 2n <= 2·10⁵: não dá a volta
        c = c
            .wrapping_add(u32::from(x.wrapping_shr(32) < limiar))
            .wrapping_add(u32::from((x & 0xFFFF_FFFF) < limiar));
        z = z.wrapping_add(GAMA);
    }
    c
}

/// Roda o modelo e devolve os bytes do resultado. `continuar` recebe os
/// sorteios feitos a cada [`LOTE_SORTEIOS`] (e o resto no fim).
fn simular(
    tamanho: u32,
    passos: u32,
    p: Parametros,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Vec<u8>, ErroDeTrabalho> {
    let m = tamanho.saturating_mul(2);
    let w = aptidoes(p.selecao, p.dominancia);
    let s0 = estado_inicial(semente);
    let k0 = contagem_inicial(tamanho, p.p0);
    let loci = u64::from(p.loci);
    let n = u64::from(tamanho);

    let mut saida = Vec::with_capacity(tamanho_do_resultado(passos, p.loci));
    saida.push(VERSAO);
    saida.extend_from_slice(&tamanho.to_be_bytes());
    saida.extend_from_slice(&passos.to_be_bytes());
    saida.push(u8::try_from(p.loci).unwrap_or(u8::MAX));
    saida.extend_from_slice(&k0.to_be_bytes());

    let mut ks = vec![k0; p.loci as usize];
    let total = u64::from(passos).saturating_mul(loci);
    let mut feitos = 0u64;
    let mut pendente = 0u64;
    for g in 0..u64::from(passos) {
        for (l, k) in (0u64..).zip(ks.iter_mut()) {
            let inicio = g.saturating_mul(loci).saturating_add(l).saturating_mul(n);
            *k = contar(s0, inicio, tamanho, limiar(*k, m, w, p.mutacao_ppm));
            saida.extend_from_slice(&k.to_be_bytes());
            feitos = feitos.saturating_add(1);
            pendente = pendente.saturating_add(u64::from(m));
            if pendente >= LOTE_SORTEIOS || feitos == total {
                let seguir = continuar(pendente);
                pendente = 0;
                // o último pedaço conta as operações, mas não cancela: o trabalho já acabou
                if !seguir && feitos < total {
                    return Err(ErroDeTrabalho::Cancelado);
                }
            }
        }
    }
    Ok(saida)
}

/// Executa a unidade.
pub fn executar(esp: &Especificacao, semente: &[u8], continuar: &mut dyn FnMut(u64) -> bool) -> Result<Execucao, ErroDeTrabalho> {
    validar(esp.tamanho(), esp.passos(), esp.parametros())?;
    let p = Parametros::de(esp.parametros())?;
    let resultado = simular(esp.tamanho(), esp.passos(), p, semente, continuar)?;
    Ok(Execucao { resultado, operacoes: operacoes(esp), curva: Vec::new() })
}

/// Confere um resultado por recomputação. Mesma convenção de
/// [`crate::trabalho::verificar_controlado`]: `Err(Cancelado)` é "ninguém
/// julgou"; `Ok(Err(recusa))` é "julgado e errado", com o motivo.
pub fn verificar(
    esp: &Especificacao,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    let p = Parametros::de(esp.parametros())?;
    // o tamanho primeiro: resultado que nem fecha G × L não merece a recomputação
    if resultado.len() != tamanho_do_resultado(esp.passos(), p.loci) {
        return Ok(Err(Recusa("resultado com tamanho que não fecha G × L contagens".into())));
    }
    let refeito = executar(esp, semente, continuar)?.resultado;
    if refeito == resultado {
        return Ok(Ok(()));
    }
    if refeito.get(..CABECALHO) != resultado.get(..CABECALHO) {
        return Ok(Err(Recusa("o cabeçalho não confere com a especificação".into())));
    }
    // compara no lugar, sem copiar: a memória declarada conta três resultados, não cinco
    fn contagens(r: &[u8]) -> &[[u8; 4]] {
        r.get(CABECALHO..).unwrap_or(&[]).as_chunks::<4>().0
    }
    let loci = p.loci.max(1) as usize;
    Ok(Err(Recusa(
        match contagens(&refeito).iter().zip(contagens(resultado)).position(|(a, b)| a != b) {
            Some(i) => format!(
                "a contagem da geração {}, locus {} difere da recomputação",
                i.checked_div(loci).unwrap_or(0).saturating_add(1),
                i.checked_rem(loci).unwrap_or(0)
            ),
            None => "difere da recomputação".into(),
        },
    )))
}

// ---------------------------------------------------------------------------
// Leitura do resultado, para o relatório
// ---------------------------------------------------------------------------

/// Um resultado lido.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trajetoria {
    /// N, indivíduos diploides.
    pub tamanho: u32,
    /// G, gerações.
    pub geracoes: u32,
    /// L, loci.
    pub loci: u32,
    /// k0, a contagem inicial de A em cada locus.
    pub inicial: u32,
    /// Contagens de A depois de cada geração (1..=G), na ordem geração e
    /// depois locus.
    pub contagens: Vec<u32>,
}

/// O que o relatório e o agregador do JOB precisam, em inteiros exatos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estatisticas {
    /// Loci com A fixado (k = 2N) no fim.
    pub fixados: u32,
    /// Loci que perderam A (k = 0) no fim.
    pub perdidos: u32,
    /// Por geração t = 0..=G, a soma das contagens nos loci (S1).
    pub soma_por_geracao: Vec<u64>,
    /// Por geração t = 0..=G, a soma dos quadrados das contagens (S2).
    pub soma_quadrados_por_geracao: Vec<u64>,
}

impl Trajetoria {
    /// Lê um resultado bem formado; `None` se a estrutura não fecha (versão,
    /// faixas, tamanho, contagem acima de 2N).
    pub fn decodificar(resultado: &[u8]) -> Option<Self> {
        let (cabeca, corpo) = resultado.split_first_chunk::<CABECALHO>()?;
        let [versao, t0, t1, t2, t3, g0, g1, g2, g3, loci, k0, k1, k2, k3] = *cabeca;
        let tamanho = u32::from_be_bytes([t0, t1, t2, t3]);
        let geracoes = u32::from_be_bytes([g0, g1, g2, g3]);
        let loci = u32::from(loci);
        let inicial = u32::from_be_bytes([k0, k1, k2, k3]);
        let m = tamanho.saturating_mul(2);
        let faixas = versao == VERSAO
            && (TAMANHO.0..=TAMANHO.1).contains(&tamanho)
            && (PASSOS.0..=PASSOS.1).contains(&geracoes)
            && (1..=LOCI_MAX).contains(&loci)
            && inicial <= m;
        if !faixas || resultado.len() != tamanho_do_resultado(geracoes, loci) {
            return None;
        }
        let contagens: Vec<u32> = corpo.as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect();
        if contagens.iter().any(|&c| c > m) {
            return None;
        }
        Some(Self { tamanho, geracoes, loci, inicial, contagens })
    }

    /// Contagem de A na geração `geracao` (0 = início) e no locus `locus`.
    pub fn contagem(&self, geracao: u32, locus: u32) -> Option<u32> {
        if locus >= self.loci || geracao > self.geracoes {
            return None;
        }
        if geracao == 0 {
            return Some(self.inicial);
        }
        let i = (geracao as usize).saturating_sub(1).saturating_mul(self.loci as usize).saturating_add(locus as usize);
        self.contagens.get(i).copied()
    }

    /// As contagens de cada locus numa geração (0 = início).
    fn geracao(&self, geracao: u32) -> Vec<u32> {
        (0..self.loci).filter_map(|l| self.contagem(geracao, l)).collect()
    }

    /// Somas por geração e fixação no fim. Com `M = 2N`, a frequência média é
    /// `S1/(L·M)` e a heterozigosidade esperada média, `2p(1-p)` nos loci,
    /// é `(2·M·S1 - 2·S2)/(L·M²)`.
    pub fn estatisticas(&self) -> Estatisticas {
        let m = self.tamanho.saturating_mul(2);
        let mut soma_por_geracao = Vec::with_capacity((self.geracoes as usize).saturating_add(1));
        let mut soma_quadrados_por_geracao = Vec::with_capacity((self.geracoes as usize).saturating_add(1));
        for g in 0..=self.geracoes {
            let linha = self.geracao(g);
            soma_por_geracao.push(linha.iter().fold(0u64, |s, &k| s.saturating_add(u64::from(k))));
            soma_quadrados_por_geracao
                .push(linha.iter().fold(0u64, |s, &k| s.saturating_add(u64::from(k).saturating_mul(u64::from(k)))));
        }
        let final_ = self.geracao(self.geracoes);
        let quantos = |alvo: u32| u32::try_from(final_.iter().filter(|&&k| k == alvo).count()).unwrap_or(u32::MAX);
        Estatisticas {
            fixados: quantos(m),
            perdidos: quantos(0),
            soma_por_geracao,
            soma_quadrados_por_geracao,
        }
    }

    /// Frequência média de A em cada geração, **só para mostrar** (ponto
    /// flutuante nunca entra no que é conferido).
    pub fn frequencias_medias(&self) -> Vec<f64> {
        let denominador = f64::from(self.loci) * f64::from(self.tamanho) * 2.0;
        self.estatisticas().soma_por_geracao.iter().map(|&s| s as f64 / denominador).collect()
    }

    /// Heterozigosidade esperada média, `2p(1-p)` nos loci, em cada geração,
    /// **só para mostrar**.
    pub fn heterozigosidades_medias(&self) -> Vec<f64> {
        let m = f64::from(self.tamanho) * 2.0;
        let l = f64::from(self.loci);
        let e = self.estatisticas();
        e.soma_por_geracao
            .iter()
            .zip(&e.soma_quadrados_por_geracao)
            .map(|(&s1, &s2)| (2.0 * m * s1 as f64 - 2.0 * s2 as f64) / (l * m * m))
            .collect()
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

    use super::*;
    use crate::trabalho;

    const SEMENTE: &[u8] = b"semente de teste";

    fn esp(tamanho: u32, passos: u32, parametros: [u32; PARAMETROS]) -> Especificacao {
        Especificacao::nova_com(TipoDeTrabalho::Genetica, tamanho, passos, &parametros).unwrap()
    }

    #[test]
    fn splitmix64_e_o_de_sempre() {
        let primeiros: Vec<u64> = (0..4).map(|n| splitmix64(0, n)).collect();
        assert_eq!(primeiros, [0xE220_A839_7B1D_CDAF, 0x6E78_9E6A_A1B9_65F4, 0x06C4_5D18_8009_454F, 0xF88B_B8A8_724C_81EC]);
    }

    #[test]
    fn recusa_fora_da_faixa_e_acima_do_teto() {
        let bom = [4, 0, SELECAO_ZERO, 50, 5000];
        let recusados: [(u32, u32, [u32; PARAMETROS]); 11] = [
            (1, 10, bom),
            (100_001, 10, bom),
            (10, 0, bom),
            (10, 10_001, bom),
            (10, 10, [0, 0, SELECAO_ZERO, 50, 5000]),
            (10, 10, [65, 0, SELECAO_ZERO, 50, 5000]),
            (10, 10, [4, 10_001, SELECAO_ZERO, 50, 5000]),
            (10, 10, [4, 0, 20_001, 50, 5000]),
            (10, 10, [4, 0, SELECAO_ZERO, 101, 5000]),
            (10, 10, [4, 0, SELECAO_ZERO, 50, 10_001]),
            // 2·100000·64·672 = 8 601 600 000 > 2^33
            (100_000, 672, [64, 0, SELECAO_ZERO, 50, 5000]),
        ];
        for (t, p, par) in recusados {
            assert!(Especificacao::nova_com(TipoDeTrabalho::Genetica, t, p, &par).is_err(), "{t} {p} {par:?}");
            assert!(validar(t, p, &par).is_err(), "{t} {p} {par:?}");
        }
        let custo = validar(100_000, 672, &[64, 0, SELECAO_ZERO, 50, 5000]).unwrap_err();
        assert!(custo.to_string().contains("teto"), "{custo}");
        assert!(validar(10, 10, &bom[..4]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Genetica, 10, 10, &bom[..4]).is_err());
        // no limite, passa
        assert!(validar(100_000, 671, &[64, 10_000, 20_000, 100, 10_000]).is_ok());
        assert!(validar(2, 1, &[1, 0, 0, 0, 0]).is_ok());
    }

    #[test]
    fn limiar_nas_bordas() {
        let neutro = aptidoes(SELECAO_ZERO, 50);
        assert_eq!(limiar(0, 16, neutro, 0), 0);
        assert_eq!(limiar(16, 16, neutro, 0), Q32);
        assert_eq!(limiar(4, 16, neutro, 0), Q32 / 4);
        assert_eq!(limiar(1, 3, neutro, 0), (Q32 + 1) / 3);
        assert_eq!(limiar(0, 10, neutro, 10_000), (Q32 * 10_000 + 500_000) / 1_000_000);
        let letal = aptidoes(0, 50);
        assert_eq!(limiar(10, 10, letal, 0), Q32);
        assert_eq!(limiar(10, 10, letal, 100), (Q32 * (1_000_000 - 100) + 500_000) / 1_000_000);
        // o maior caso: nada satura
        let forte = aptidoes(SELECAO_MAX, 100);
        assert_eq!(limiar(200_000, 200_000, forte, 0), Q32);
        assert!(limiar(1, 200_000, forte, 0) > 0);
        assert_eq!(contagem_inicial(10, 2250), 5);
        assert_eq!(contagem_inicial(10, 2249), 4);
        assert_eq!(contagem_inicial(100_000, 10_000), 200_000);
    }

    #[test]
    fn contar_bate_com_o_fluxo() {
        let s0 = estado_inicial(SEMENTE);
        for limiar in [1u64, 1 << 31, Q32 - 1] {
            let mut esperado = 0u32;
            for n in 100..150u64 {
                let x = splitmix64(s0, n);
                esperado += u32::from((x >> 32) < limiar) + u32::from((x & 0xFFFF_FFFF) < limiar);
            }
            assert_eq!(contar(s0, 100, 50, limiar), esperado);
        }
        assert_eq!(contar(s0, 7, 50, 0), 0);
        assert_eq!(contar(s0, 7, 50, Q32), 100);
    }

    #[test]
    fn executa_verifica_e_conta_as_operacoes() {
        let e = esp(40, 30, [8, 100, 10_500, 50, 5000]);
        let mut feitas = 0u64;
        let exec = executar(&e, SEMENTE, &mut |ops| {
            feitas += ops;
            true
        })
        .unwrap();
        assert_eq!(exec.operacoes, 2 * 40 * 8 * 30);
        assert_eq!(feitas, exec.operacoes);
        assert_eq!(Some(exec.operacoes), e.operacoes_fixas());
        assert_eq!(trabalho::operacoes_de_verificacao(&e), exec.operacoes);
        assert_eq!(exec.resultado.len(), CABECALHO + 4 * 30 * 8);
        assert!(e.memoria_bytes() >= 2 * exec.resultado.len() as u64);
        assert_eq!(trabalho::verificar(&e, SEMENTE, &exec.resultado), Ok(()));
        // determinístico, e a semente importa
        assert_eq!(executar(&e, SEMENTE, &mut |_| true).unwrap(), exec);
        assert_ne!(executar(&e, b"outra", &mut |_| true).unwrap().resultado, exec.resultado);
        assert!(e.resumo().contains("s +0,0500"), "{}", e.resumo());
        assert!(esp(40, 30, [8, 100, 2_500, 50, 5000]).resumo().contains("s -0,7500"));
    }

    #[test]
    fn resultado_adulterado_e_recusado() {
        let e = esp(30, 12, [3, 50, 11_000, 25, 4000]);
        let bom = executar(&e, SEMENTE, &mut |_| true).unwrap().resultado;

        let mut errado = bom.clone();
        errado[CABECALHO + 4 * (7 * 3 + 2) + 3] ^= 1;
        let recusa = trabalho::verificar(&e, SEMENTE, &errado).unwrap_err();
        assert!(recusa.0.contains("geração 8, locus 2"), "{recusa}");

        assert!(trabalho::verificar(&e, SEMENTE, &bom[..bom.len() - 4]).unwrap_err().0.contains("tamanho"));
        let mut cabeca = bom.clone();
        cabeca[13] ^= 1;
        assert!(trabalho::verificar(&e, SEMENTE, &cabeca).unwrap_err().0.contains("cabeçalho"));
        assert!(trabalho::verificar(&e, b"outra", &bom).is_err());
        assert!(trabalho::verificar(&esp(30, 12, [3, 50, 19_000, 25, 4000]), SEMENTE, &bom).is_err());
        assert!(trabalho::verificar(&e, SEMENTE, &[]).is_err());
    }

    #[test]
    fn cancelar_para_no_meio_e_o_fim_nao_cancela() {
        // 2·1000·8·200 = 3,2 milhões de sorteios: sete pedaços de LOTE_SORTEIOS
        let e = esp(1000, 200, [8, 0, SELECAO_ZERO, 50, 5000]);
        let mut chamadas = 0u32;
        let r = executar(&e, SEMENTE, &mut |ops| {
            chamadas += 1;
            assert!(ops <= LOTE_SORTEIOS + 2 * u64::from(TAMANHO.1), "pedaço grande demais: {ops}");
            chamadas < 2
        });
        assert_eq!(r, Err(ErroDeTrabalho::Cancelado));
        assert_eq!(chamadas, 2);

        // dizer "pare" só no último pedaço não desfaz o trabalho
        let mut pedacos = 0u32;
        let _ = executar(&e, SEMENTE, &mut |_| {
            pedacos += 1;
            true
        })
        .unwrap();
        let mut n = 0u32;
        let ultimo = executar(&e, SEMENTE, &mut |_| {
            n += 1;
            n < pedacos
        })
        .unwrap();
        assert_eq!(trabalho::verificar(&e, SEMENTE, &ultimo.resultado), Ok(()));
    }

    #[test]
    fn verificacao_interrompida_nao_e_recusa() {
        let e = esp(1000, 200, [8, 0, SELECAO_ZERO, 50, 5000]);
        let exec = executar(&e, SEMENTE, &mut |_| true).unwrap();
        let mut chamadas = 0u32;
        let r = trabalho::verificar_controlado(&e, SEMENTE, &exec.resultado, &mut |_| {
            chamadas += 1;
            false
        });
        assert_eq!(r, Err(ErroDeTrabalho::Cancelado));
        assert_eq!(chamadas, 1);
        // resultado errado, mas interrompido: ninguém julgou
        let mut errado = exec.resultado.clone();
        errado[CABECALHO + 3] ^= 1;
        assert_eq!(trabalho::verificar_controlado(&e, SEMENTE, &errado, &mut |_| false), Err(ErroDeTrabalho::Cancelado));
        assert_eq!(trabalho::verificar_controlado(&e, SEMENTE, &exec.resultado, &mut |_| true), Ok(Ok(())));
    }

    #[test]
    fn estatisticas_batem_com_a_trajetoria() {
        let e = esp(12, 20, [5, 200, 10_300, 60, 3000]);
        let r = executar(&e, SEMENTE, &mut |_| true).unwrap().resultado;
        let t = Trajetoria::decodificar(&r).unwrap();
        assert_eq!((t.tamanho, t.geracoes, t.loci), (12, 20, 5));
        let est = t.estatisticas();
        assert_eq!(est.soma_por_geracao.len(), 21);
        assert_eq!(est.soma_por_geracao[0], 5 * u64::from(t.inicial));
        let finais = &t.contagens[t.contagens.len() - 5..];
        assert_eq!(est.soma_por_geracao[20], finais.iter().map(|&k| u64::from(k)).sum::<u64>());
        assert_eq!(est.fixados, u32::try_from(finais.iter().filter(|&&k| k == 24).count()).unwrap());
        assert_eq!(t.contagem(20, 4), Some(finais[4]));
        assert_eq!(t.contagem(21, 0), None);
        assert_eq!(t.frequencias_medias().len(), 21);
        assert!(t.heterozigosidades_medias().iter().all(|h| (0.0..=0.5).contains(h)));
        assert!(Trajetoria::decodificar(&r[..r.len() - 1]).is_none());
        let mut acima = r.clone();
        acima[CABECALHO..CABECALHO + 4].copy_from_slice(&25u32.to_be_bytes());
        assert!(Trajetoria::decodificar(&acima).is_none());
    }

    /// Vazão do motor. Rodar à mão:
    /// `cargo test -p hyurax-ultrax --release --lib -- --ignored --nocapture vazao`.
    #[test]
    #[ignore = "medida de tempo, não teste"]
    fn vazao() {
        let s0 = estado_inicial(SEMENTE);
        let n = 100_000u32;
        let voltas = 500u64;
        let inicio = std::time::Instant::now();
        let mut soma = 0u64;
        for v in 0..voltas {
            soma += u64::from(contar(s0, v * u64::from(n), n, 1 << 31));
        }
        let dt = inicio.elapsed().as_secs_f64();
        let sorteios = (voltas * 2 * u64::from(n)) as f64;
        println!("contar: {:.1} milhões de sorteios/s ({soma})", sorteios / dt / 1e6);

        let e = esp(100_000, 50, [8, 100, 10_200, 50, 5000]);
        let inicio = std::time::Instant::now();
        let mut pedacos = Vec::new();
        let mut ultimo = std::time::Instant::now();
        let exec = executar(&e, SEMENTE, &mut |_| {
            pedacos.push(ultimo.elapsed());
            ultimo = std::time::Instant::now();
            true
        })
        .unwrap();
        let dt = inicio.elapsed().as_secs_f64();
        pedacos.sort();
        println!(
            "executar N=100000, L=8, G=50: {:.2} s, {:.1} milhões de sorteios/s; {} pedaços entre chamadas, mediana {:?}, maior {:?}",
            dt,
            exec.operacoes as f64 / dt / 1e6,
            pedacos.len(),
            pedacos[pedacos.len() / 2],
            pedacos[pedacos.len() - 1]
        );
        println!("2^33 sorteios levariam {:.0} s neste ritmo", (1u64 << 33) as f64 / (exec.operacoes as f64 / dt));
    }
}
