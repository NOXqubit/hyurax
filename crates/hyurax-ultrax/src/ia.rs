// ✝ Provérbios 1:5 — “Ouça o sábio e cresça em conhecimento.”
//! Trabalho de IA: treinar uma rede neural pequena para prever a solubilidade
//! de moléculas reais na água.
//!
//! Tradução de `reference/hyurax/ia.py`, conferida contra `vectors/ia.json`.
//! A base (`dados/moleculas.tsv`, 2048 moléculas da AqSolDB, CC0) vem
//! embutida no programa. Prever solubilidade é uma etapa de triagem na
//! pesquisa de remédios: não é descoberta de remédio.
//!
//! Tudo em inteiros de ponto fixo Q12 (1,0 = 4096). Deslocar para a direita e
//! dividir arredondam para baixo, como no Python. As contas do meio usam
//! `i128`, que cobre com folga os produtos que o Python faz sem limite: é o
//! que garante o mesmo modelo bit a bit nos dois lados.

// Os produtos do treino vão até 2^65 e as somas de um lote até 2^74; `i128`
// guarda 2^127. Os limites vêm da faixa dos dados (|x| <= 2^15) e do prender
// dos pesos (|w| <= 2^31 - 1), então nenhuma conta deste módulo estoura.
#![allow(clippy::arithmetic_side_effects)]

use std::sync::OnceLock;

use hyurax_crypto::xof;

/// Base embutida, gerada por `reference/tools/preparar_moleculas.py`.
pub const BASE_TSV: &str = include_str!("../dados/moleculas.tsv");

/// Bits do ponto fixo.
pub const ESCALA_BITS: u32 = 12;
/// 1,0 em Q12.
pub const ESCALA: i64 = 1 << ESCALA_BITS;
/// Descritores de entrada.
pub const ENTRADAS: usize = 10;
/// Neurônios da camada oculta.
pub const OCULTOS: usize = 16;
/// Maior lote aceito.
pub const LOTE_MAX: u32 = 256;
/// Mais passos que isso é recusado.
pub const PASSOS_MAX: u32 = 4096;
/// Taxa de aprendizado inicial: 1/64. Cai para 1/128 no segundo terço do treino
/// e para 1/256 no último: com taxa fixa, o treino longo pula sem assentar.
pub const TAXA_BITS: u32 = 6;
const LIMITE: i128 = (1 << 31) - 1;

/// Domínio do sorteio dos pesos iniciais.
pub const DOMINIO_PESOS: &[u8] = dominio!("IA-PESOS-v1");
/// Domínio do sorteio dos lotes.
pub const DOMINIO_LOTE: &[u8] = dominio!("IA-LOTE-v1");

/// Nomes dos descritores, na ordem das colunas.
pub const DESCRITORES: [&str; ENTRADAS] = [
    "massa molar",
    "logP",
    "refratividade molar",
    "átomos pesados",
    "aceptores de H",
    "doadores de H",
    "ligações giratórias",
    "anéis aromáticos",
    "anéis",
    "área polar",
];

/// Uma molécula da base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Molecula {
    /// Identificador na AqSolDB.
    pub id: String,
    /// Nome.
    pub nome: String,
    /// Fórmula, tirada do InChI.
    pub formula: String,
    /// Estrutura em SMILES.
    pub smiles: String,
    /// Massa molar em miligramas por mol (g/mol × 1000).
    pub massa_mili: i64,
    /// log S medido, × 1000.
    pub logs_mili: i64,
    /// Alvo normalizado, Q12.
    pub alvo: i64,
    /// Descritores normalizados, Q12.
    pub x: [i64; ENTRADAS],
}

/// A base inteira, mais a normalização do alvo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    /// Moléculas, na ordem do arquivo.
    pub moleculas: Vec<Molecula>,
    /// Média do log S, × 1000.
    pub media_mili: i64,
    /// Desvio do log S, × 1000.
    pub desvio_mili: i64,
}

impl Base {
    /// Lê o TSV. Linha que não se lê é ignorada.
    pub fn ler(texto: &str) -> Self {
        let mut base = Self { moleculas: Vec::new(), media_mili: 0, desvio_mili: 1000 };
        for linha in texto.lines() {
            if let Some(resto) = linha.strip_prefix("#normalizacao\t") {
                let mut partes = resto.split('\t').filter_map(|v| v.trim().parse::<i64>().ok());
                if let (Some(m), Some(d)) = (partes.next(), partes.next()) {
                    base.media_mili = m;
                    base.desvio_mili = d.max(1);
                }
                continue;
            }
            if linha.starts_with('#') || linha.trim().is_empty() {
                continue;
            }
            let c: Vec<&str> = linha.split('\t').collect();
            let numero = |k: usize| c.get(k).and_then(|v| v.trim().parse::<i64>().ok());
            let texto = |k: usize| c.get(k).map(|v| (*v).to_string()).unwrap_or_default();
            let mut x = [0i64; ENTRADAS];
            let mut completo = true;
            for (i, xi) in x.iter_mut().enumerate() {
                match numero(i.saturating_add(7)) {
                    Some(v) => *xi = v,
                    None => completo = false,
                }
            }
            let (Some(massa_mili), Some(logs_mili), Some(alvo)) = (numero(4), numero(5), numero(6)) else { continue };
            if completo {
                base.moleculas.push(Molecula {
                    id: texto(0),
                    nome: texto(1),
                    formula: texto(2),
                    smiles: texto(3),
                    massa_mili,
                    logs_mili,
                    alvo,
                    x,
                });
            }
        }
        base
    }

    /// A base que vem no programa, lida uma vez.
    pub fn embutida() -> &'static Self {
        static BASE: OnceLock<Base> = OnceLock::new();
        BASE.get_or_init(|| Self::ler(BASE_TSV))
    }

    /// Índices de treino (i % 5 != 0) e de validação (i % 5 == 0).
    pub fn divisao(&self) -> (Vec<usize>, Vec<usize>) {
        (0..self.moleculas.len()).partition(|i| i % 5 != 0)
    }

    /// Alvo normalizado (Q12) de volta para log S × 1000.
    pub fn logs_de(&self, alvo_q12: i64) -> i64 {
        (i128::from(alvo_q12) * i128::from(self.desvio_mili) / i128::from(ESCALA))
            .try_into()
            .unwrap_or(0i64)
            .saturating_add(self.media_mili)
    }
}

/// Os pesos da rede.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pesos {
    /// Camada oculta, `OCULTOS × ENTRADAS`.
    pub w1: [[i64; ENTRADAS]; OCULTOS],
    /// Viés da camada oculta.
    pub b1: [i64; OCULTOS],
    /// Saída.
    pub w2: [i64; OCULTOS],
    /// Viés da saída.
    pub b2: i64,
}

fn prender(v: i128) -> i64 {
    i64::try_from(v.clamp(-LIMITE, LIMITE)).unwrap_or(0)
}

fn inteiros(semente: &[u8], quantos: usize, dominio: &[u8]) -> Vec<u32> {
    xof(semente, quantos.saturating_mul(4), dominio).as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect()
}

/// Pesos iniciais: `W1` e `W2` em `[-1024, 1023]`, vieses zero.
pub fn pesos_iniciais(semente: &[u8]) -> Pesos {
    let v = inteiros(semente, OCULTOS * ENTRADAS + OCULTOS, DOMINIO_PESOS);
    let valor = |k: usize| i64::from(v.get(k).copied().unwrap_or(0) % 2048) - 1024;
    let mut p = Pesos { w1: [[0; ENTRADAS]; OCULTOS], b1: [0; OCULTOS], w2: [0; OCULTOS], b2: 0 };
    for (j, linha) in p.w1.iter_mut().enumerate() {
        for (i, w) in linha.iter_mut().enumerate() {
            *w = valor(j * ENTRADAS + i);
        }
    }
    for (j, w) in p.w2.iter_mut().enumerate() {
        *w = valor(OCULTOS * ENTRADAS + j);
    }
    p
}

/// Saída (Q12) e pré-ativações da camada oculta.
pub fn prever(p: &Pesos, x: &[i64; ENTRADAS]) -> (i64, [i64; OCULTOS]) {
    let mut pre = [0i64; OCULTOS];
    let mut soma_saida: i128 = 0;
    for (j, pj) in pre.iter_mut().enumerate() {
        let linha = p.w1.get(j).copied().unwrap_or([0; ENTRADAS]);
        let soma: i128 = linha.iter().zip(x).map(|(&w, &xi)| i128::from(w) * i128::from(xi)).sum();
        *pj = prender((soma >> ESCALA_BITS) + i128::from(p.b1.get(j).copied().unwrap_or(0)));
        let h = (*pj).max(0);
        soma_saida += i128::from(h) * i128::from(p.w2.get(j).copied().unwrap_or(0));
    }
    (prender((soma_saida >> ESCALA_BITS) + i128::from(p.b2)), pre)
}

/// Erro quadrático médio de validação, em Q12.
pub fn erro_de_validacao(p: &Pesos, base: &Base, validacao: &[usize]) -> u64 {
    let mut soma: i128 = 0;
    for &k in validacao {
        let Some(m) = base.moleculas.get(k) else { continue };
        let (y, _) = prever(p, &m.x);
        let e = i128::from(y) - i128::from(m.alvo);
        soma += (e * e) >> ESCALA_BITS;
    }
    let n = i128::try_from(validacao.len().max(1)).unwrap_or(1);
    u64::try_from(soma.div_euclid(n)).unwrap_or(u64::MAX)
}

/// O que um treino devolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Treino {
    /// Pesos finais.
    pub pesos: Pesos,
    /// Erro de validação no fim, Q12.
    pub erro: u64,
    /// Erro médio do lote em cada passo, Q12.
    pub curva: Vec<u64>,
}

/// Treina `passos` passos com lotes de `lote` moléculas. `continuar` recebe
/// as operações de cada passo e pode interromper (devolve `None`).
pub fn treinar(semente: &[u8], lote: u32, passos: u32, base: &Base, continuar: &mut dyn FnMut(u64) -> bool) -> Option<Treino> {
    treinar_observado(semente, lote, passos, base, continuar, &mut crate::observador::Nenhum)
}

/// Como [`treinar`], entregando a perda e os pesos de saída de cada passo a
/// `obs`. O treino é o mesmo, bit a bit.
pub fn treinar_observado(
    semente: &[u8],
    lote: u32,
    passos: u32,
    base: &Base,
    continuar: &mut dyn FnMut(u64) -> bool,
    obs: &mut dyn crate::observador::Observador,
) -> Option<Treino> {
    let (treino, validacao) = base.divisao();
    if treino.is_empty() {
        return None;
    }
    let n_lote = lote as usize;
    let mut p = pesos_iniciais(semente);
    let sorteio = inteiros(semente, n_lote.saturating_mul(passos as usize), DOMINIO_LOTE);
    let por_passo = u64::from(lote).saturating_mul(176 + 192).saturating_add(193);
    let mut curva = Vec::with_capacity(passos as usize);
    for passo in 0..passos as usize {
        let mut g1 = [[0i128; ENTRADAS]; OCULTOS];
        let mut gb1 = [0i128; OCULTOS];
        let mut g2 = [0i128; OCULTOS];
        let mut gb2: i128 = 0;
        let mut perda: i128 = 0;
        for k in 0..n_lote {
            let indice = sorteio.get(passo.saturating_mul(n_lote).saturating_add(k)).copied().unwrap_or(0) as usize;
            let Some(m) = treino.get(indice % treino.len()).and_then(|&i| base.moleculas.get(i)) else { continue };
            let (y, pre) = prever(&p, &m.x);
            let e = i128::from(y) - i128::from(m.alvo);
            perda += (e * e) >> ESCALA_BITS;
            gb2 += e;
            for j in 0..OCULTOS {
                let pj = pre.get(j).copied().unwrap_or(0);
                if pj > 0 {
                    if let Some(g) = g2.get_mut(j) {
                        *g += (e * i128::from(pj)) >> ESCALA_BITS;
                    }
                    let dh = (e * i128::from(p.w2.get(j).copied().unwrap_or(0))) >> ESCALA_BITS;
                    if let Some(g) = gb1.get_mut(j) {
                        *g += dh;
                    }
                    if let Some(linha) = g1.get_mut(j) {
                        for (g, &xi) in linha.iter_mut().zip(&m.x) {
                            *g += (dh * i128::from(xi)) >> ESCALA_BITS;
                        }
                    }
                }
            }
        }
        let degrau = u32::try_from(passo.saturating_mul(3) / (passos as usize).max(1)).unwrap_or(2);
        let div = i128::from(lote) << (TAXA_BITS + degrau);
        for j in 0..OCULTOS {
            if let (Some(linha), Some(gl)) = (p.w1.get_mut(j), g1.get(j)) {
                for (w, g) in linha.iter_mut().zip(gl) {
                    *w = prender(i128::from(*w) - g.div_euclid(div));
                }
            }
            if let (Some(b), Some(g)) = (p.b1.get_mut(j), gb1.get(j)) {
                *b = prender(i128::from(*b) - g.div_euclid(div));
            }
            if let (Some(w), Some(g)) = (p.w2.get_mut(j), g2.get(j)) {
                *w = prender(i128::from(*w) - g.div_euclid(div));
            }
        }
        p.b2 = prender(i128::from(p.b2) - gb2.div_euclid(div));
        let perda_do_lote = u64::try_from(perda.div_euclid(i128::from(lote.max(1)))).unwrap_or(u64::MAX);
        curva.push(perda_do_lote);
        if obs.quer() {
            obs.amostra(crate::observador::Amostra::Ia {
                passo: u32::try_from(passo.saturating_add(1)).unwrap_or(u32::MAX),
                passos,
                perda: perda_do_lote,
                pesos_saida: p.w2.to_vec(),
            });
        }
        // o último passo conta as operações, mas não cancela: o treino já acabou
        if !continuar(por_passo) && passo.saturating_add(1) < passos as usize {
            return None;
        }
    }
    let erro = erro_de_validacao(&p, base, &validacao);
    let _ = continuar((validacao.len() as u64).saturating_mul(176));
    Some(Treino { pesos: p, erro, curva })
}

/// Pesos (int32 big-endian, na ordem `W1`, `b1`, `W2`, `b2`) e o erro (u64 big-endian).
pub fn codificar(p: &Pesos, erro: u64) -> Vec<u8> {
    let mut saida = Vec::with_capacity((OCULTOS * ENTRADAS + OCULTOS * 2 + 1) * 4 + 8);
    let mut um = |v: i64| saida.extend_from_slice(&i32::try_from(v).unwrap_or(0).to_be_bytes());
    for linha in &p.w1 {
        for &w in linha {
            um(w);
        }
    }
    for &b in &p.b1 {
        um(b);
    }
    for &w in &p.w2 {
        um(w);
    }
    um(p.b2);
    saida.extend_from_slice(&erro.to_be_bytes());
    saida
}

/// O inverso de [`codificar`].
pub fn decodificar(bytes: &[u8]) -> Option<(Pesos, u64)> {
    let (corpo, erro) = bytes.split_last_chunk::<8>()?;
    let valores: Vec<i64> = corpo.as_chunks::<4>().0.iter().map(|c| i64::from(i32::from_be_bytes(*c))).collect();
    if valores.len() != OCULTOS * ENTRADAS + OCULTOS * 2 + 1 || corpo.len() % 4 != 0 {
        return None;
    }
    let mut it = valores.into_iter();
    let mut p = Pesos { w1: [[0; ENTRADAS]; OCULTOS], b1: [0; OCULTOS], w2: [0; OCULTOS], b2: 0 };
    for linha in &mut p.w1 {
        for w in linha.iter_mut() {
            *w = it.next()?;
        }
    }
    for b in &mut p.b1 {
        *b = it.next()?;
    }
    for w in &mut p.w2 {
        *w = it.next()?;
    }
    p.b2 = it.next()?;
    Some((p, u64::from_be_bytes(*erro)))
}

/// Modelo de custo: por amostra de treino, 176 na ida e 192 na volta; por
/// passo, 193 na atualização; por amostra de validação, 176.
pub fn operacoes(lote: u32, passos: u32, validacao: usize) -> u64 {
    u64::from(passos)
        .saturating_mul(u64::from(lote).saturating_mul(176 + 192).saturating_add(193))
        .saturating_add((validacao as u64).saturating_mul(176))
}

/// Raiz do erro quadrático médio, em log S × 1000 (a unidade de verdade).
pub fn rmse_em_logs(erro_q12: u64, base: &Base) -> u64 {
    // erro_q12 é o erro quadrático médio normalizado em Q12; a raiz fica na
    // escala do alvo, e o desvio devolve para log S
    let mse_normalizado = erro_q12 as f64 / ESCALA as f64;
    (mse_normalizado.sqrt() * base.desvio_mili as f64).round() as u64
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn base_embutida_tem_as_2048() {
        let b = Base::embutida();
        assert_eq!(b.moleculas.len(), 2048);
        assert_eq!((b.media_mili, b.desvio_mili), (-2954, 2255));
        let (t, v) = b.divisao();
        assert_eq!((t.len(), v.len()), (1638, 410));
        assert!(b.moleculas.iter().all(|m| !m.smiles.is_empty() && !m.formula.is_empty()));
    }

    #[test]
    fn treino_aprende() {
        let b = Base::embutida();
        let (_, v) = b.divisao();
        let semente = hyurax_crypto::sha512(b"a");
        let antes = erro_de_validacao(&pesos_iniciais(&semente), b, &v);
        let t = treinar(&semente, 32, 200, b, &mut |_| true).unwrap();
        assert!(t.erro * 2 < antes, "o erro de validação cai: {antes} -> {}", t.erro);
        assert_eq!(t.curva.len(), 200);
    }

    #[test]
    fn codificar_ida_e_volta() {
        let b = Base::embutida();
        let t = treinar(b"x", 8, 5, b, &mut |_| true).unwrap();
        let bytes = codificar(&t.pesos, t.erro);
        assert_eq!(decodificar(&bytes), Some((t.pesos.clone(), t.erro)));
        assert!(decodificar(&bytes[1..]).is_none());
    }

    #[test]
    fn interromper_para() {
        let b = Base::embutida();
        let mut n = 0;
        assert!(treinar(b"x", 8, 50, b, &mut |_| {
            n += 1;
            n < 10
        })
        .is_none());
    }
}
