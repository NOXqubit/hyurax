// ✝ Gênesis 1:11 — “Produza a terra erva verde, erva que dê semente.”
//! Melhoramento de culturas: QTL aditivos, ambiente (água, nitrogênio, solo)
//! com interação G×E, seleção truncada e cruzamento.
//!
//! Tradução de `reference/hyurax/melhoramento.py`, conferida byte a byte
//! contra `vectors/melhoramento.json`. O desenho está em
//! `docs/COMPUTACAO-CIENTIFICA.md`, seção 2.3. Tipo de trabalho 6: `tamanho`
//! = N plantas, `passos` = G gerações de seleção, verificação por
//! recomputação.
//!
//! **O que é:** um modelo didático de genética quantitativa com os
//! parâmetros de quem pediu. Mostra como a seleção muda uma população ao longo
//! das gerações (resposta à seleção, perda de variância, fixação de alelos)
//! num ambiente com água, nitrogênio e solo definidos.
//!
//! **O que não é:** não prevê safra real, não descreve variedade real e não
//! substitui ensaio de campo. Os efeitos dos QTL são sorteados, não medidos;
//! a resposta do ambiente é uma linha-platô com números escolhidos para o
//! modelo. É resultado computacional, que precisa de validação científica.
//!
//! # Parâmetros (8 `u32`, nesta ordem)
//!
//! | # | nome | faixa | sentido |
//! |---|---|---|---|
//! | 0 | `qtl` | 1 a 200 | L, número de QTL |
//! | 1 | `arquitetura` | qualquer | sorteia os efeitos; é parâmetro, e não semente, para que as unidades de um JOB tenham a mesma arquitetura |
//! | 2 | `selecionados` | 1 a 100 | % selecionada por geração |
//! | 3 | `ruido` | 0 a 1000 | desvio do ruído em % do desvio genético inicial esperado; 100 dá h² perto de 1/2 |
//! | 4 | `agua` | 0 a 200 | % da demanda da cultura |
//! | 5 | `nitrogenio` | 0 a 200 | % da dose de referência |
//! | 6 | `solo` | 0 a 200 | qualidade do solo, % da referência |
//! | 7 | `gxe` | 0 a 100 | intensidade da interação genótipo × ambiente, % |
//!
//! Combinações com custo acima de [`TETO_OPERACOES`] são recusadas.
//!
//! # Modelo (Q16: inteiro com 1,0 = 65536)
//!
//! 1. **Arquitetura.** Por QTL, quatro `u32` big-endian de
//!    `XOF(u32 arquitetura BE, MELHORAMENTO-ARQUITETURA-v1)`:
//!    `a = 1 + (r1 mod 10⁴)·(r2 mod 10⁴) // 10⁴` em `[1, 9999]` (muitos
//!    efeitos pequenos, poucos grandes); se `r3` é ímpar o QTL é sensível ao
//!    ambiente e `d = (r4 mod 19999) − 9999`, senão `d = 0`.
//! 2. **Liebig.** Cada recurso responde em linha-platô,
//!    `f(x) = min(Q16, I + (Q16 − I)·x // c)` com `I = intercepto·Q16 // 100`:
//!    água (0 %, platô em 80 %), nitrogênio (30 %, 100 %), solo (40 %, 100 %).
//!    O fator é o mínimo, `F = min(f_água, f_N, f_solo)`. Acima do ponto
//!    crítico a resposta fica no platô: o modelo não representa excesso.
//! 3. **G×E.** Com estresse `S = Q16 − F`, o efeito no ambiente é
//!    `e = a + (d·S·gxe) // (100·Q16)` (piso).
//! 4. **Valor genético.** `n ∈ {0,1,2}` cópias do alelo 1;
//!    `Gp = max(0, 1.000.000 + Σ e·(n − 1))` em partes por milhão do potencial
//!    de referência, e o valor genético no ambiente é `G = (Gp·F) >> 16`.
//!    QTL aditivos, sem dominância nem epistasia.
//! 5. **Ruído.** `sigma = isqrt(Σe²·F²·ruido² // (2·Q16²·100²))` (desvio
//!    nominal com frequência 1/2). Por planta, três sorteios de 64 bits dão
//!    doze uniformes de 16 bits; `z = Σu − 393210` (Irwin-Hall, desvio ≈
//!    65536); o ruído é `(z·sigma + 32768) >> 16` e o fenótipo
//!    `P = max(0, G + ruído)`.
//! 6. **População inicial** em Hardy-Weinberg com frequência 1/2: por planta,
//!    `W = ceil(L/64)` sorteios para um alelo e `W` para o outro.
//! 7. **Geração** `t = 0..=G`: avalia, registra, seleciona os
//!    `M = ceil(N·selecionados/100)` de maior fenótipo (empate: menor índice)
//!    e, se `t < G`, cruza: pais `sel[x mod M]` e `sel[y mod M]`
//!    (autofecundação permitida), depois `W` sorteios para cada gameta.
//!    Segregação mendeliana independente por locus.
//!
//! O gerador é o xoshiro256** com estado de `XOF(semente, 32,
//! MELHORAMENTO-GERADOR-v1)` (quatro `u64` big-endian; todos zero vira
//! `s0 = 1`), um fluxo só, consumido nessa ordem.
//!
//! # Resultado (big-endian)
//!
//! `u32 F || u8 limitante || u64 sigma || u32 M`, depois `G+1` linhas de 48
//! bytes (`u64` média de G, variância de G, média de P, variância de P, média
//! de P dos selecionados; `u32` QTL fixados no alelo 1 e no alelo 0), depois
//! `L × u32` com as cópias do alelo 1 na população final. Médias com piso;
//! variância populacional `(N·Σx² − (Σx)²) // N²`.

use std::cmp::Reverse;

use hyurax_crypto::xof;

use crate::observador::{Amostra, Nenhum, Observador};
use crate::trabalho::{ErroDeTrabalho, Especificacao, Execucao, Recusa};

/// Quantos parâmetros extras a especificação leva.
pub const PARAMETROS: usize = 8;
/// Faixa aceita de `tamanho`: plantas na população.
pub const TAMANHO: (u32, u32) = (4, 20_000);
/// Faixa aceita de `passos`: gerações de seleção.
pub const PASSOS: (u32, u32) = (1, 1_000);
/// Mais QTL que isso é recusado (e cabe em [`PALAVRAS`] palavras de 64 bits).
pub const QTL_MAX: u32 = 200;
/// Maior ruído aceito, em % do desvio genético inicial.
pub const RUIDO_MAX: u32 = 1_000;
/// Maior oferta de um recurso, em %.
pub const RECURSO_MAX: u32 = 200;
/// Teto de custo: combinações acima disso (pelo modelo de [`operacoes`]) são
/// recusadas. Em release, o Atom de desenvolvimento faz perto de 5·10^8
/// operações do modelo por segundo (medido em 27/09/2026, `medir_velocidade`):
/// o teto é cerca de 8 s por unidade, e o dobro com a verificação.
pub const TETO_OPERACOES: u64 = 4_000_000_000;

/// 1,0 em Q16.
pub const Q16: u32 = 1 << 16;
/// Potencial de referência: 100 % em partes por milhão.
pub const BASE: i64 = 1_000_000;
/// Os efeitos aditivos ficam em `[1, EFEITO_MAX − 1]`.
pub const EFEITO_MAX: u32 = 10_000;
/// A sensibilidade ao ambiente fica em `[−GXE_MAX, GXE_MAX]`.
pub const GXE_MAX: u32 = 9_999;
/// Uniformes de 16 bits por ruído (Irwin-Hall).
pub const UNIFORMES: u64 = 12;
/// Média da soma dos doze uniformes: `12 · 65535 / 2`.
const MEDIA_IRWIN_HALL: i64 = 393_210;
/// `r4 mod FAIXA_GXE` fica em `[0, 2·GXE_MAX]`.
const FAIXA_GXE: u32 = 2 * GXE_MAX + 1;
/// Divisor do termo G×E: `100 · Q16`.
const DIVISOR_GXE: i64 = 100 * Q16 as i64;
/// Divisor da variância do ruído: `2 · Q16² · 100²`.
const DIVISOR_RUIDO: u128 = 2 * (Q16 as u128) * (Q16 as u128) * 100 * 100;
/// Plantas por chamada de `continuar`.
pub const BLOCO: usize = 1024;
/// Palavras de 64 bits de cada máscara de locos.
pub const PALAVRAS: usize = 4;
/// (intercepto %, ponto crítico %) de água, nitrogênio e solo.
pub const RECURSOS: [(u32, u32); 3] = [(0, 80), (30, 100), (40, 100)];
/// Nomes dos recursos pelo código do limitante; 3 é "nenhum limita".
pub const NOMES_DOS_RECURSOS: [&str; 4] = ["água", "nitrogênio", "solo", "nenhum"];
/// Bytes do cabeçalho do resultado.
pub const CABECALHO_BYTES: usize = 17;
/// Bytes de cada linha (geração) do resultado.
pub const LINHA_BYTES: usize = 48;
/// Por planta numa rodada: duas populações, valor genético e fenótipo (`i64`)
/// e a ordem da seleção (`u32`).
const BYTES_POR_PLANTA: u64 = 2 * size_of::<Planta>() as u64 + 8 + 8 + 4;

/// Domínio do sorteio da arquitetura (efeitos dos QTL).
pub const DOMINIO_ARQUITETURA: &[u8] = dominio!("MELHORAMENTO-ARQUITETURA-v1");
/// Domínio do estado inicial do gerador.
pub const DOMINIO_GERADOR: &[u8] = dominio!("MELHORAMENTO-GERADOR-v1");

// ---------------------------------------------------------------------------
// Parâmetros
// ---------------------------------------------------------------------------

/// Os parâmetros, com nome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parametros {
    /// L, número de QTL.
    pub qtl: u32,
    /// Código da arquitetura genética.
    pub arquitetura: u32,
    /// % selecionada por geração.
    pub selecionados: u32,
    /// Ruído ambiental, % do desvio genético inicial.
    pub ruido: u32,
    /// Água, % da demanda.
    pub agua: u32,
    /// Nitrogênio, % da referência.
    pub nitrogenio: u32,
    /// Solo, % da referência.
    pub solo: u32,
    /// Intensidade da interação G×E, %.
    pub gxe: u32,
}

impl Parametros {
    /// Lê a lista da especificação, na ordem do módulo.
    pub fn de(lista: &[u32]) -> Result<Self, ErroDeTrabalho> {
        match *lista {
            [qtl, arquitetura, selecionados, ruido, agua, nitrogenio, solo, gxe] => {
                Ok(Self { qtl, arquitetura, selecionados, ruido, agua, nitrogenio, solo, gxe })
            }
            _ => Err(ErroDeTrabalho::Parametros(format!(
                "melhoramento leva {PARAMETROS} parâmetros, vieram {}",
                lista.len()
            ))),
        }
    }

    /// A lista, na ordem da especificação.
    pub fn lista(&self) -> [u32; PARAMETROS] {
        [self.qtl, self.arquitetura, self.selecionados, self.ruido, self.agua, self.nitrogenio, self.solo, self.gxe]
    }
}

/// `M = ceil(N · porcentagem / 100)`.
pub fn selecionados(tamanho: u32, porcentagem: u32) -> u32 {
    let m = u64::from(tamanho).saturating_mul(u64::from(porcentagem)).saturating_add(99) / 100;
    u32::try_from(m).unwrap_or(u32::MAX)
}

/// `ceil(log2 N)` para `N >= 1`: os bits de `N − 1`.
fn log2_teto(n: u32) -> u64 {
    u64::from(u32::BITS.saturating_sub(n.saturating_sub(1).leading_zeros()))
}

/// Modelo de custo:
/// `(G+1)·N·(L+12) + (G+1)·N·ceil(log2 N) + G·N·(2L+2)`, avaliar, selecionar e
/// cruzar.
pub fn operacoes_de(tamanho: u32, passos: u32, qtl: u32) -> u64 {
    let n = u64::from(tamanho);
    let g = u64::from(passos);
    let l = u64::from(qtl);
    let avaliacoes = g.saturating_add(1).saturating_mul(n);
    avaliacoes
        .saturating_mul(l.saturating_add(UNIFORMES))
        .saturating_add(avaliacoes.saturating_mul(log2_teto(tamanho)))
        .saturating_add(g.saturating_mul(n).saturating_mul(l.saturating_mul(2).saturating_add(2)))
}

/// Confere as faixas e o teto de custo.
pub fn validar(tamanho: u32, passos: u32, parametros: &[u32]) -> Result<(), ErroDeTrabalho> {
    let p = Parametros::de(parametros)?;
    let fora = |nome: &str, valor: u32, minimo: u32, maximo: u32| {
        ErroDeTrabalho::Parametros(format!("{nome} fora da faixa: {valor} ({minimo} a {maximo})"))
    };
    if !(TAMANHO.0..=TAMANHO.1).contains(&tamanho) {
        return Err(fora("indivíduos", tamanho, TAMANHO.0, TAMANHO.1));
    }
    if !(PASSOS.0..=PASSOS.1).contains(&passos) {
        return Err(fora("gerações", passos, PASSOS.0, PASSOS.1));
    }
    let faixas = [
        ("QTL", p.qtl, 1, QTL_MAX),
        ("porcentagem selecionada", p.selecionados, 1, 100),
        ("ruído ambiental", p.ruido, 0, RUIDO_MAX),
        ("água", p.agua, 0, RECURSO_MAX),
        ("nitrogênio", p.nitrogenio, 0, RECURSO_MAX),
        ("solo", p.solo, 0, RECURSO_MAX),
        ("intensidade G×E", p.gxe, 0, 100),
    ];
    for (nome, valor, minimo, maximo) in faixas {
        if !(minimo..=maximo).contains(&valor) {
            return Err(fora(nome, valor, minimo, maximo));
        }
    }
    let custo = operacoes_de(tamanho, passos, p.qtl);
    if custo > TETO_OPERACOES {
        return Err(ErroDeTrabalho::Parametros(format!(
            "custo de {custo} operações passa do teto de {TETO_OPERACOES}"
        )));
    }
    Ok(())
}

/// Operações da execução, pelo modelo de custo do motor.
pub fn operacoes(esp: &Especificacao) -> u64 {
    let qtl = esp.parametros().first().copied().unwrap_or(0);
    operacoes_de(esp.tamanho(), esp.passos(), qtl)
}

/// Operações da verificação: a recomputação inteira.
pub fn operacoes_de_verificacao(esp: &Especificacao) -> u64 {
    operacoes(esp)
}

/// Bytes do resultado: `17 + 48·(G+1) + 4·L`.
pub fn tamanho_do_resultado(passos: u32, qtl: u32) -> usize {
    let linhas = (passos as usize).saturating_add(1).saturating_mul(LINHA_BYTES);
    CABECALHO_BYTES.saturating_add(linhas).saturating_add((qtl as usize).saturating_mul(4))
}

/// Memória no pico, execução e verificação somadas, em bytes.
///
/// Numa rodada: duas populações de [`Planta`] (a atual e a dos filhos, 128
/// bytes por planta), valor genético e fenótipo (`i64` cada) e a ordem da
/// seleção (`u32`): 148 bytes por planta. Mais a tabela de somas (2 KiB por
/// byte de máscara), a arquitetura (56 bytes por QTL, com o XOF), as linhas,
/// as frequências e os bytes do resultado. A verificação refaz tudo com o
/// resultado da execução guardado, e 16 KiB cobrem as alocações pequenas.
pub fn memoria_bytes(esp: &Especificacao) -> u64 {
    let n = u64::from(esp.tamanho());
    let g = u64::from(esp.passos());
    let l = u64::from(esp.parametros().first().copied().unwrap_or(0));
    let resultado = tamanho_do_resultado(esp.passos(), esp.parametros().first().copied().unwrap_or(0)) as u64;
    let rodada = n
        .saturating_mul(BYTES_POR_PLANTA)
        .saturating_add(l.div_ceil(8).saturating_mul(256 * 8))
        .saturating_add(l.saturating_mul(56))
        .saturating_add(g.saturating_add(1).saturating_mul(size_of::<Linha>() as u64))
        .saturating_add(l.saturating_mul(4))
        .saturating_add(resultado);
    rodada.saturating_mul(2).saturating_add(resultado).saturating_add(16 * 1024)
}

/// Texto curto para o painel.
pub fn resumo(esp: &Especificacao) -> String {
    let p = esp.parametros();
    let em = |k: usize| p.get(k).copied().unwrap_or(0);
    format!("{} plantas, {} gerações, {} QTL, {} % selecionadas", esp.tamanho(), esp.passos(), em(0), em(2))
}

// ---------------------------------------------------------------------------
// Gerador
// ---------------------------------------------------------------------------

/// xoshiro256** (Blackman e Vigna): rápido, 256 bits de estado, o mesmo
/// fluxo no Python e no Rust.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gerador {
    s: [u64; 4],
}

impl Gerador {
    /// A partir do estado; os quatro zero viram `[1, 0, 0, 0]`.
    pub fn novo(mut estado: [u64; 4]) -> Self {
        if estado == [0; 4] {
            estado = [1, 0, 0, 0];
        }
        Self { s: estado }
    }

    /// Estado de `XOF(semente, 32, MELHORAMENTO-GERADOR-v1)`, quatro `u64` BE.
    pub fn da_semente(semente: &[u8]) -> Self {
        let bruto = xof(semente, 32, DOMINIO_GERADOR);
        let mut estado = [0u64; 4];
        for (s, bytes) in estado.iter_mut().zip(bruto.as_chunks::<8>().0) {
            *s = u64::from_be_bytes(*bytes);
        }
        Self::novo(estado)
    }

    /// O estado atual.
    pub fn estado(&self) -> [u64; 4] {
        self.s
    }

    /// O próximo número de 64 bits.
    pub fn proximo(&mut self) -> u64 {
        let [mut s0, mut s1, mut s2, mut s3] = self.s;
        let resultado = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s1 << 17;
        s2 ^= s0;
        s3 ^= s1;
        s1 ^= s2;
        s0 ^= s3;
        s2 ^= t;
        s3 = s3.rotate_left(45);
        self.s = [s0, s1, s2, s3];
        resultado
    }

    /// `quantas` palavras sorteadas, na ordem; as outras ficam zero.
    fn palavras(&mut self, quantas: usize) -> [u64; PALAVRAS] {
        let mut v = [0u64; PALAVRAS];
        for w in v.iter_mut().take(quantas) {
            *w = self.proximo();
        }
        v
    }
}

// ---------------------------------------------------------------------------
// Arquitetura e ambiente
// ---------------------------------------------------------------------------

/// Um QTL: efeito aditivo `a` do alelo 1 e sensibilidade `d` ao estresse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Qtl {
    /// Efeito por cópia do alelo 1 sem estresse, em `[1, 9999]`.
    pub a: i64,
    /// Mudança do efeito sob estresse total, em `[−9999, 9999]`; 0 se o QTL
    /// não é sensível.
    pub d: i64,
}

/// A arquitetura: só depende do código e de L, nunca da semente da unidade.
pub fn arquitetura(codigo: u32, qtl: u32) -> Vec<Qtl> {
    let bruto = xof(&codigo.to_be_bytes(), (qtl as usize).saturating_mul(16), DOMINIO_ARQUITETURA);
    let valores: Vec<u32> = bruto.as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect();
    valores
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&[r1, r2, r3, r4]| {
            let produto = u64::from(r1 % EFEITO_MAX).saturating_mul(u64::from(r2 % EFEITO_MAX));
            let a = i64::try_from(produto.checked_div(u64::from(EFEITO_MAX)).unwrap_or(0)).unwrap_or(0).saturating_add(1);
            let d = if r3 % 2 == 1 { i64::from(r4 % FAIXA_GXE).saturating_sub(i64::from(GXE_MAX)) } else { 0 };
            Qtl { a, d }
        })
        .collect()
}

/// Linha-platô de um recurso (0 água, 1 nitrogênio, 2 solo), em Q16.
pub fn resposta(recurso: usize, x: u32) -> u32 {
    let Some(&(intercepto, critico)) = RECURSOS.get(recurso) else { return Q16 };
    let base = u64::from(intercepto).saturating_mul(u64::from(Q16)) / 100;
    let subida = u64::from(Q16).saturating_sub(base).saturating_mul(u64::from(x)).checked_div(u64::from(critico)).unwrap_or(0);
    u32::try_from(base.saturating_add(subida).min(u64::from(Q16))).unwrap_or(Q16)
}

/// `(F em Q16, recurso limitante)` pela lei do mínimo de Liebig. O limitante
/// é o de menor resposta (empate: o de menor índice), e 3 quando nenhum
/// limita.
pub fn fator_ambiental(agua: u32, nitrogenio: u32, solo: u32) -> (u32, u8) {
    let f = [resposta(0, agua), resposta(1, nitrogenio), resposta(2, solo)];
    let mut fator = Q16;
    let mut limitante = 3u8;
    // estritamente menor: no empate fica o primeiro
    for (k, &v) in (0u8..).zip(&f) {
        if v < fator {
            fator = v;
            limitante = k;
        }
    }
    (fator, limitante)
}

/// Efeito de cada QTL no ambiente: `e = a + (d · S · gxe) // (100 · Q16)`,
/// com `S = Q16 − F` e divisão com piso.
pub fn efeitos(arq: &[Qtl], fator: u32, gxe: u32) -> Vec<i64> {
    let estresse = i64::from(Q16.saturating_sub(fator));
    // |d · S · gxe| <= 9999 · 65536 · 100 < 2^37: cabe folgado em i64
    arq.iter()
        .map(|q| q.a.saturating_add(q.d.saturating_mul(estresse).saturating_mul(i64::from(gxe)).div_euclid(DIVISOR_GXE)))
        .collect()
}

/// Desvio nominal do ruído: `isqrt(Σe² · F² · ruido² // (2 · Q16² · 100²))`.
pub fn desvio_do_ruido(efeitos: &[i64], fator: u32, ruido: u32) -> u64 {
    // Σe² <= 200 · 19998² < 2^37; vezes F² (2^32) e ruido² (< 2^20): < 2^89
    let soma = efeitos.iter().fold(0u128, |s, &e| s.saturating_add(u128::from(e.unsigned_abs()).saturating_mul(u128::from(e.unsigned_abs()))));
    let f = u128::from(fator);
    let r = u128::from(ruido);
    let v = soma.saturating_mul(f).saturating_mul(f).saturating_mul(r).saturating_mul(r) / DIVISOR_RUIDO;
    u64::try_from(v.isqrt()).unwrap_or(u64::MAX)
}

/// Por byte da máscara, a soma dos efeitos dos bits ligados.
fn tabela(efeitos: &[i64]) -> Vec<[i64; 256]> {
    let bytes = efeitos.len().div_ceil(8);
    (0..bytes)
        .map(|b| {
            let mut linha = [0i64; 256];
            for v in 1..256usize {
                let anterior = linha.get(v & v.wrapping_sub(1)).copied().unwrap_or(0);
                let i = b.saturating_mul(8).saturating_add(v.trailing_zeros() as usize);
                let e = efeitos.get(i).copied().unwrap_or(0);
                if let Some(x) = linha.get_mut(v) {
                    *x = anterior.saturating_add(e);
                }
            }
            linha
        })
        .collect()
}

/// `Σ e_i` dos locos ligados na máscara: byte `b` (bits `8b..8b+7`) na tabela `b`.
fn soma(tabela: &[[i64; 256]], mascara: &[u64; PALAVRAS]) -> i64 {
    let mut total = 0i64;
    for (linhas, palavra) in tabela.chunks(8).zip(mascara) {
        for (linha, byte) in linhas.iter().zip(palavra.to_le_bytes()) {
            total = total.saturating_add(linha.get(usize::from(byte)).copied().unwrap_or(0));
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Simulação
// ---------------------------------------------------------------------------

/// Uma planta diploide, por máscaras de locos: `hom1` tem as duas cópias do
/// alelo 1; `het`, uma de cada. O resto (dentro de L) é homozigoto no 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Planta {
    /// Homozigota no alelo 1.
    pub hom1: [u64; PALAVRAS],
    /// Heterozigota.
    pub het: [u64; PALAVRAS],
}

/// As estatísticas de uma geração.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Linha {
    /// Média do valor genético no ambiente.
    pub media_g: u64,
    /// Variância do valor genético (divisor N).
    pub var_g: u64,
    /// Média do fenótipo.
    pub media_p: u64,
    /// Variância do fenótipo.
    pub var_p: u64,
    /// Média do fenótipo dos selecionados.
    pub media_sel: u64,
    /// QTL fixados no alelo 1.
    pub fixados_1: u32,
    /// QTL fixados no alelo 0.
    pub fixados_0: u32,
}

/// Nomes dos campos da linha, na ordem dos bytes.
pub const CAMPOS: [&str; 7] = [
    "média do valor genético",
    "variância do valor genético",
    "média do fenótipo",
    "variância do fenótipo",
    "média dos selecionados",
    "QTL fixados no alelo 1",
    "QTL fixados no alelo 0",
];

impl Linha {
    fn campos(&self) -> [u64; 7] {
        [
            self.media_g,
            self.var_g,
            self.media_p,
            self.var_p,
            self.media_sel,
            u64::from(self.fixados_1),
            u64::from(self.fixados_0),
        ]
    }
}

/// O resultado de uma unidade.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resultado {
    /// Fator ambiental de Liebig, Q16.
    pub fator: u32,
    /// Recurso limitante: 0 água, 1 nitrogênio, 2 solo, 3 nenhum.
    pub limitante: u8,
    /// Desvio do ruído ambiental.
    pub sigma: u64,
    /// Plantas selecionadas por geração.
    pub selecionados: u32,
    /// Uma linha por geração, `0..=G`.
    pub linhas: Vec<Linha>,
    /// Cópias do alelo 1 na população final, por QTL (0 a 2N).
    pub frequencias: Vec<u32>,
}

impl Resultado {
    /// Os bytes canônicos.
    pub fn codificar(&self) -> Vec<u8> {
        let mut saida = Vec::with_capacity(
            CABECALHO_BYTES.saturating_add(self.linhas.len().saturating_mul(LINHA_BYTES)).saturating_add(self.frequencias.len().saturating_mul(4)),
        );
        saida.extend_from_slice(&self.fator.to_be_bytes());
        saida.push(self.limitante);
        saida.extend_from_slice(&self.sigma.to_be_bytes());
        saida.extend_from_slice(&self.selecionados.to_be_bytes());
        for l in &self.linhas {
            for v in [l.media_g, l.var_g, l.media_p, l.var_p, l.media_sel] {
                saida.extend_from_slice(&v.to_be_bytes());
            }
            saida.extend_from_slice(&l.fixados_1.to_be_bytes());
            saida.extend_from_slice(&l.fixados_0.to_be_bytes());
        }
        for f in &self.frequencias {
            saida.extend_from_slice(&f.to_be_bytes());
        }
        saida
    }

    /// O inverso de [`Self::codificar`], com G e L conhecidos pela especificação.
    pub fn decodificar(bytes: &[u8], passos: u32, qtl: u32) -> Option<Self> {
        if bytes.len() != tamanho_do_resultado(passos, qtl) {
            return None;
        }
        let (fator, resto) = bytes.split_first_chunk::<4>()?;
        let (&limitante, resto) = resto.split_first()?;
        let (sigma, resto) = resto.split_first_chunk::<8>()?;
        let (selecionados, resto) = resto.split_first_chunk::<4>()?;
        let linhas_bytes = (passos as usize).saturating_add(1).saturating_mul(LINHA_BYTES);
        let (corpo, frequencias) = resto.split_at_checked(linhas_bytes)?;
        let mut linhas = Vec::with_capacity((passos as usize).saturating_add(1));
        for l in corpo.as_chunks::<LINHA_BYTES>().0 {
            let (u64s, u32s) = l.split_at_checked(40)?;
            let v: Vec<u64> = u64s.as_chunks::<8>().0.iter().map(|c| u64::from_be_bytes(*c)).collect();
            let w: Vec<u32> = u32s.as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect();
            let (&[media_g, var_g, media_p, var_p, media_sel], &[fixados_1, fixados_0]) = (v.as_slice(), w.as_slice()) else {
                return None;
            };
            linhas.push(Linha { media_g, var_g, media_p, var_p, media_sel, fixados_1, fixados_0 });
        }
        Some(Self {
            fator: u32::from_be_bytes(*fator),
            limitante,
            sigma: u64::from_be_bytes(*sigma),
            selecionados: u32::from_be_bytes(*selecionados),
            linhas,
            frequencias: frequencias.as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect(),
        })
    }
}

/// Conta as operações e deixa quem chamou parar. A última chamada conta, mas
/// não cancela: o trabalho já acabou.
fn avancar(continuar: &mut dyn FnMut(u64) -> bool, ops: u64, fim: bool) -> Result<(), ErroDeTrabalho> {
    if continuar(ops) || fim { Ok(()) } else { Err(ErroDeTrabalho::Cancelado) }
}

/// Média com piso e variância populacional com piso, pelas somas exatas.
fn media_e_variancia(valores: &[i64]) -> (u64, u64) {
    let n = valores.len().max(1) as u128;
    let (soma, quadrados) = valores.iter().fold((0u128, 0u128), |(s, q), &v| {
        let v = u128::from(v.unsigned_abs());
        (s.saturating_add(v), q.saturating_add(v.saturating_mul(v)))
    });
    // N·Σx² >= (Σx)² sempre (Cauchy-Schwarz); os valores são >= 0
    let variancia = n.saturating_mul(quadrados).saturating_sub(soma.saturating_mul(soma)).checked_div(n.saturating_mul(n)).unwrap_or(0);
    let media = soma.checked_div(n).unwrap_or(0);
    (u64::try_from(media).unwrap_or(u64::MAX), u64::try_from(variancia).unwrap_or(u64::MAX))
}

/// Roda a unidade inteira e devolve o resultado estruturado.
///
/// `continuar` recebe as operações de cada bloco de [`BLOCO`] plantas
/// avaliadas, de cada seleção e de cada bloco de filhos; devolver `false`
/// cancela.
pub fn simular(
    esp: &Especificacao,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Resultado, ErroDeTrabalho> {
    simular_observado(esp, semente, continuar, &mut Nenhum)
}

/// Como [`simular`], entregando a população avaliada de cada geração a `obs`.
pub fn simular_observado(
    esp: &Especificacao,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
    obs: &mut dyn Observador,
) -> Result<Resultado, ErroDeTrabalho> {
    validar(esp.tamanho(), esp.passos(), esp.parametros())?;
    let p = Parametros::de(esp.parametros())?;
    let n = esp.tamanho() as usize;
    let passos = esp.passos();
    let qtl = p.qtl as usize;
    let palavras = qtl.div_ceil(64);
    let mut mascara = [0u64; PALAVRAS];
    for (k, w) in mascara.iter_mut().enumerate() {
        let inicio = k.saturating_mul(64);
        *w = match qtl.saturating_sub(inicio) {
            0 => 0,
            resto if resto >= 64 => u64::MAX,
            resto => (1u64 << resto).wrapping_sub(1),
        };
    }
    let (fator, limitante) = fator_ambiental(p.agua, p.nitrogenio, p.solo);
    let efs = efeitos(&arquitetura(p.arquitetura, p.qtl), fator, p.gxe);
    let sigma = desvio_do_ruido(&efs, fator, p.ruido);
    let sigma_i = i64::try_from(sigma).unwrap_or(i64::MAX);
    let m = selecionados(esp.tamanho(), p.selecionados);
    let m_usize = m as usize;
    let tab = tabela(&efs);
    let mut rng = Gerador::da_semente(semente);
    let log = log2_teto(esp.tamanho());
    let l = u64::from(p.qtl);
    let custo_avaliar = l.saturating_add(UNIFORMES);
    let custo_cruzar = l.saturating_mul(2).saturating_add(2);

    let mut pop: Vec<Planta> = Vec::with_capacity(n);
    for _ in 0..n {
        let a = rng.palavras(palavras);
        let b = rng.palavras(palavras);
        let mut planta = Planta::default();
        for k in 0..PALAVRAS {
            if let (Some(h1), Some(ht), Some(&x), Some(&y), Some(&mk)) =
                (planta.hom1.get_mut(k), planta.het.get_mut(k), a.get(k), b.get(k), mascara.get(k))
            {
                *h1 = x & y & mk;
                *ht = (x ^ y) & mk;
            }
        }
        pop.push(planta);
    }

    let mut genetico = vec![0i64; n];
    let mut fenotipo = vec![0i64; n];
    let mut ordem: Vec<u32> = Vec::with_capacity(n);
    let mut linhas = Vec::with_capacity((passos as usize).saturating_add(1));
    for t in 0..=passos {
        let ultima = t == passos;
        // avaliar, na ordem do índice
        for ((plantas, gs), ps) in pop.chunks(BLOCO).zip(genetico.chunks_mut(BLOCO)).zip(fenotipo.chunks_mut(BLOCO)) {
            for ((planta, g), f) in plantas.iter().zip(gs.iter_mut()).zip(ps.iter_mut()) {
                let mut hom0 = [0u64; PALAVRAS];
                for ((h0, (&h1, &ht)), &mk) in hom0.iter_mut().zip(planta.hom1.iter().zip(&planta.het)).zip(&mascara) {
                    *h0 = mk & !(h1 | ht);
                }
                let potencial = BASE.saturating_add(soma(&tab, &planta.hom1)).saturating_sub(soma(&tab, &hom0)).max(0);
                // Gp <= 10^6 + 200 · 19998 < 2^23, e F <= 2^16
                *g = potencial.saturating_mul(i64::from(fator)) >> 16;
                let mut z = -MEDIA_IRWIN_HALL;
                for _ in 0..3 {
                    let x = rng.proximo();
                    for u in [x & 0xFFFF, (x >> 16) & 0xFFFF, (x >> 32) & 0xFFFF, x >> 48] {
                        z = z.saturating_add(i64::try_from(u).unwrap_or(0));
                    }
                }
                // |z| <= 393210 e sigma < 2^21: o produto fica abaixo de 2^40
                let ruido = z.saturating_mul(sigma_i).saturating_add(32_768) >> 16;
                *f = g.saturating_add(ruido).max(0);
            }
            avancar(continuar, (plantas.len() as u64).saturating_mul(custo_avaliar), false)?;
        }

        // selecionar: maior fenótipo primeiro, empate pelo menor índice
        ordem.clear();
        ordem.extend(0..u32::try_from(n).unwrap_or(u32::MAX));
        ordem.sort_unstable_by_key(|&j| (Reverse(fenotipo.get(j as usize).copied().unwrap_or(0)), j));
        let sel = ordem.get(..m_usize).unwrap_or(&[]);
        avancar(continuar, (n as u64).saturating_mul(log), ultima)?;

        let (media_g, var_g) = media_e_variancia(&genetico);
        let (media_p, var_p) = media_e_variancia(&fenotipo);
        let soma_sel = sel.iter().fold(0u128, |s, &j| {
            s.saturating_add(u128::from(fenotipo.get(j as usize).copied().unwrap_or(0).unsigned_abs()))
        });
        let mut fixo1 = mascara;
        let mut fixo0 = mascara;
        for planta in &pop {
            for (((f1, f0), &h1), &ht) in fixo1.iter_mut().zip(fixo0.iter_mut()).zip(&planta.hom1).zip(&planta.het) {
                *f1 &= h1;
                *f0 &= !(h1 | ht);
            }
        }
        linhas.push(Linha {
            media_g,
            var_g,
            media_p,
            var_p,
            media_sel: u64::try_from(soma_sel.checked_div(u128::from(m)).unwrap_or(0)).unwrap_or(u64::MAX),
            fixados_1: fixo1.iter().map(|w| w.count_ones()).sum(),
            fixados_0: fixo0.iter().map(|w| w.count_ones()).sum(),
        });
        if obs.quer() {
            let mut escolhida = vec![false; n];
            for &j in sel {
                if let Some(x) = escolhida.get_mut(j as usize) {
                    *x = true;
                }
            }
            let a_cada = n.div_ceil(400).max(1);
            let pontos = (0..n)
                .step_by(a_cada)
                .map(|j| {
                    (
                        genetico.get(j).copied().unwrap_or(0),
                        fenotipo.get(j).copied().unwrap_or(0),
                        escolhida.get(j).copied().unwrap_or(false),
                    )
                })
                .collect();
            obs.amostra(Amostra::Melhoramento {
                geracao: t,
                geracoes: passos,
                plantas: u32::try_from(n).unwrap_or(u32::MAX),
                pontos,
                a_cada: u32::try_from(a_cada).unwrap_or(u32::MAX),
            });
        }
        if ultima {
            break;
        }

        // cruzar: dois pais sorteados entre os selecionados, um gameta de cada
        let mut filhos: Vec<Planta> = Vec::with_capacity(n);
        let m64 = u64::from(m.max(1));
        while filhos.len() < n {
            let quantos = n.saturating_sub(filhos.len()).min(BLOCO);
            for _ in 0..quantos {
                let a = rng.proximo().checked_rem(m64).unwrap_or(0) as usize;
                let b = rng.proximo().checked_rem(m64).unwrap_or(0) as usize;
                let pa = sel.get(a).and_then(|&j| pop.get(j as usize)).copied().unwrap_or_default();
                let pb = sel.get(b).and_then(|&j| pop.get(j as usize)).copied().unwrap_or_default();
                let bits_a = rng.palavras(palavras);
                let bits_b = rng.palavras(palavras);
                let mut filho = Planta::default();
                for k in 0..PALAVRAS {
                    let em = |v: &[u64; PALAVRAS]| v.get(k).copied().unwrap_or(0);
                    let ga = em(&pa.hom1) | (em(&pa.het) & em(&bits_a));
                    let gb = em(&pb.hom1) | (em(&pb.het) & em(&bits_b));
                    if let (Some(h1), Some(ht)) = (filho.hom1.get_mut(k), filho.het.get_mut(k)) {
                        *h1 = ga & gb;
                        *ht = ga ^ gb;
                    }
                }
                filhos.push(filho);
            }
            avancar(continuar, (quantos as u64).saturating_mul(custo_cruzar), false)?;
        }
        pop = filhos;
    }

    let mut frequencias = vec![0u32; qtl];
    for planta in &pop {
        for (i, f) in frequencias.iter_mut().enumerate() {
            let palavra = i / 64;
            let bit = i % 64;
            let um = |v: &[u64; PALAVRAS]| u32::from(v.get(palavra).is_some_and(|w| (w >> bit) & 1 == 1));
            *f = f.saturating_add(um(&planta.hom1).saturating_mul(2)).saturating_add(um(&planta.het));
        }
    }

    Ok(Resultado { fator, limitante, sigma, selecionados: m, linhas, frequencias })
}

/// Executa a unidade.
pub fn executar(esp: &Especificacao, semente: &[u8], continuar: &mut dyn FnMut(u64) -> bool) -> Result<Execucao, ErroDeTrabalho> {
    executar_observado(esp, semente, continuar, &mut Nenhum)
}

/// Como [`executar`], entregando a população de cada geração a `obs`.
pub fn executar_observado(
    esp: &Especificacao,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
    obs: &mut dyn Observador,
) -> Result<Execucao, ErroDeTrabalho> {
    let r = simular_observado(esp, semente, continuar, obs)?;
    Ok(Execucao { resultado: r.codificar(), operacoes: operacoes(esp), curva: Vec::new() })
}

/// Confere um resultado por recomputação. Mesma convenção de
/// [`crate::trabalho::verificar_controlado`]: `Err(Cancelado)` é "ninguém
/// julgou"; `Ok(Err(recusa))` é "julgado e errado", com o campo que diverge.
pub fn verificar(
    esp: &Especificacao,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    let qtl = esp.parametros().first().copied().unwrap_or(0);
    // o tamanho primeiro: resultado que nem fecha as linhas não merece a recomputação
    let Some(alegado) = Resultado::decodificar(resultado, esp.passos(), qtl) else {
        return Ok(Err(Recusa("resultado com tamanho que não fecha as gerações e os QTL".into())));
    };
    let refeito = simular(esp, semente, continuar)?;
    Ok(comparar(&refeito, &alegado))
}

fn comparar(refeito: &Resultado, alegado: &Resultado) -> Result<(), Recusa> {
    if refeito == alegado {
        return Ok(());
    }
    let recusa = |motivo: String| Err(Recusa(motivo));
    if refeito.fator != alegado.fator {
        return recusa("fator ambiental difere da recomputação".into());
    }
    if refeito.limitante != alegado.limitante {
        return recusa("recurso limitante difere da recomputação".into());
    }
    if refeito.sigma != alegado.sigma {
        return recusa("desvio do ruído difere da recomputação".into());
    }
    if refeito.selecionados != alegado.selecionados {
        return recusa("número de selecionados difere da recomputação".into());
    }
    for (t, (a, b)) in refeito.linhas.iter().zip(&alegado.linhas).enumerate() {
        if let Some(nome) = a.campos().iter().zip(b.campos()).zip(CAMPOS).find(|((x, y), _)| **x != *y).map(|(_, nome)| nome) {
            return recusa(format!("geração {t}: {nome} difere da recomputação"));
        }
    }
    if let Some(i) = refeito.frequencias.iter().zip(&alegado.frequencias).position(|(a, b)| a != b) {
        return recusa(format!("QTL {i}: frequência final difere da recomputação"));
    }
    recusa("o resultado difere da recomputação".into())
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

    use super::*;
    use crate::trabalho::{self, TipoDeTrabalho};

    const PADRAO: [u32; PARAMETROS] = [50, 11, 20, 100, 100, 100, 100, 0];

    fn esp(tamanho: u32, passos: u32, parametros: &[u32]) -> Especificacao {
        Especificacao::nova_com(TipoDeTrabalho::Melhoramento, tamanho, passos, parametros).unwrap()
    }

    fn com(k: usize, v: u32) -> [u32; PARAMETROS] {
        let mut p = PADRAO;
        p[k] = v;
        p
    }

    #[test]
    fn gerador_de_referencia() {
        // os primeiros valores do xoshiro256** com estado (1, 2, 3, 4)
        let mut g = Gerador::novo([1, 2, 3, 4]);
        let v: Vec<u64> = (0..4).map(|_| g.proximo()).collect();
        assert_eq!(v, [11520, 0, 1_509_978_240, 1_215_971_899_390_074_240]);
        assert_eq!(Gerador::novo([0; 4]).estado(), [1, 0, 0, 0]);
    }

    #[test]
    fn liebig_e_gxe() {
        assert_eq!(fator_ambiental(100, 100, 100), (Q16, 3));
        assert_eq!(fator_ambiental(40, 100, 100), (32_768, 0));
        assert_eq!(fator_ambiental(40, 50, 100), (32_768, 0), "o nitrogênio a 0,65 não limita");
        assert_eq!(fator_ambiental(100, 20, 100), (28_835, 1));
        assert_eq!(fator_ambiental(100, 100, 0), (26_214, 2));
        assert_eq!(fator_ambiental(0, 0, 0), (0, 0));
        let arq = arquitetura(11, 40);
        let a: Vec<i64> = arq.iter().map(|q| q.a).collect();
        assert_eq!(efeitos(&arq, Q16, 100), a, "sem estresse não há G×E");
        assert_eq!(efeitos(&arq, 32_768, 0), a, "com G×E = 0 o estresse não muda os efeitos");
        let seca = efeitos(&arq, 32_768, 100);
        assert!(arq.iter().zip(&seca).all(|(q, &e)| (q.d == 0) == (e == q.a) || q.d.abs() < 2));
        assert!(seca.iter().any(|&e| e < 0));
        assert_eq!(arquitetura(11, 20), arquitetura(11, 40)[..20]);
    }

    #[test]
    fn determinismo_e_selecao() {
        let e = esp(300, 5, &PADRAO);
        let a = executar(&e, b"um", &mut |_| true).unwrap();
        assert_eq!(a, executar(&e, b"um", &mut |_| true).unwrap());
        assert_ne!(a.resultado, executar(&e, b"dois", &mut |_| true).unwrap().resultado);
        assert_eq!(a.resultado.len(), tamanho_do_resultado(5, 50));
        assert_eq!(a.operacoes, operacoes(&e));
        let r = Resultado::decodificar(&a.resultado, 5, 50).unwrap();
        assert_eq!(r.codificar(), a.resultado);
        assert_eq!(r.selecionados, 60);
        // seleção sobe a média genética; a variância cai
        assert!(r.linhas[5].media_g > r.linhas[0].media_g);
        assert!(r.linhas[5].var_g < r.linhas[0].var_g);
        assert!(r.linhas.iter().all(|l| l.media_sel >= l.media_p));
        assert!(r.frequencias.iter().all(|&f| f <= 600));
    }

    #[test]
    fn verificacao_pega_adulteracao() {
        let e = esp(40, 4, &PADRAO);
        let exec = executar(&e, b"adulterar", &mut |_| true).unwrap();
        assert_eq!(trabalho::verificar(&e, b"adulterar", &exec.resultado), Ok(()));
        let mut errado = exec.resultado.clone();
        errado[CABECALHO_BYTES + 2 * LINHA_BYTES + 7] ^= 1;
        let recusa = trabalho::verificar(&e, b"adulterar", &errado).unwrap_err();
        assert!(recusa.0.contains("geração 2"), "{recusa}");
        let mut errado = exec.resultado.clone();
        let fim = errado.len();
        errado[fim - 4 * 50 + 3 * 4 + 3] ^= 1;
        assert!(trabalho::verificar(&e, b"adulterar", &errado).unwrap_err().0.contains("QTL 3"));
        let mut errado = exec.resultado.clone();
        errado[12] ^= 1;
        assert!(trabalho::verificar(&e, b"adulterar", &errado).unwrap_err().0.contains("ruído"));
        assert!(trabalho::verificar(&e, b"adulterar", &exec.resultado[1..]).unwrap_err().0.contains("tamanho"));
        assert!(trabalho::verificar(&e, b"outra", &exec.resultado).is_err());
    }

    #[test]
    fn recusas() {
        assert!(Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 100, 10, &PADRAO).is_ok());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 100, 10, &PADRAO[..7]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 3, 10, &PADRAO).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 20_001, 10, &PADRAO).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 100, 0, &PADRAO).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 100, 1001, &PADRAO).is_err());
        for (k, v) in [(0, 0), (0, 201), (2, 0), (2, 101), (3, 1001), (4, 201), (5, 201), (6, 201), (7, 101)] {
            let r = Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 100, 10, &com(k, v));
            assert!(matches!(r, Err(ErroDeTrabalho::Parametros(_))), "parâmetro {k} = {v}: {r:?}");
        }
        let caro = Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 20_000, 1000, &com(0, 200));
        assert!(matches!(caro, Err(ErroDeTrabalho::Parametros(ref m)) if m.contains("teto")), "{caro:?}");
        assert!(Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 20_000, 100, &com(0, 200)).is_ok());
        // a especificação vai e volta pela codificação
        let e = esp(123, 7, &PADRAO);
        let mut w = hyurax_codec::Writer::new();
        e.codificar(&mut w);
        let bytes = w.into_bytes();
        let mut r = hyurax_codec::Reader::new(&bytes);
        assert_eq!(Especificacao::decodificar(&mut r).unwrap(), e);
    }

    #[test]
    fn cancelar_e_contar_operacoes() {
        let e = esp(2500, 3, &com(0, 30));
        let mut chamadas = Vec::new();
        executar(&e, b"ops", &mut |ops| {
            chamadas.push(ops);
            true
        })
        .unwrap();
        assert_eq!(chamadas.iter().sum::<u64>(), operacoes(&e));
        assert!(chamadas.iter().all(|&c| c <= 1024 * 62), "{chamadas:?}");

        let mut n = 0;
        let r = executar(&e, b"ops", &mut |_| {
            n += 1;
            n < 5
        });
        assert_eq!(r, Err(ErroDeTrabalho::Cancelado));
        assert_eq!(n, 5);

        // a última chamada conta, mas não cancela
        let total = chamadas.len();
        let mut k = 0;
        let ultima = executar(&e, b"ops", &mut |_| {
            k += 1;
            k < total
        })
        .unwrap();
        assert_eq!(ultima.resultado, executar(&e, b"ops", &mut |_| true).unwrap().resultado);

        // verificação interrompida não é recusa
        let exec = executar(&e, b"ops", &mut |_| true).unwrap();
        let mut feitas = 0;
        let v = trabalho::verificar_controlado(&e, b"ops", &exec.resultado, &mut |_| {
            feitas += 1;
            false
        });
        assert_eq!(v, Err(ErroDeTrabalho::Cancelado));
        assert_eq!(feitas, 1);
    }

    #[test]
    fn memoria_e_resumo() {
        let pequena = esp(100, 10, &PADRAO);
        let grande = esp(20_000, 100, &com(0, 200));
        assert!(memoria_bytes(&pequena) < memoria_bytes(&grande));
        // o teto honesto das duas populações e dos vetores por planta, duas vezes
        assert!(memoria_bytes(&grande) >= 2 * 20_000 * 148);
        assert!(memoria_bytes(&grande) < 16 * 1024 * 1024);
        assert_eq!(pequena.memoria_bytes(), memoria_bytes(&pequena));
        assert_eq!(pequena.resumo(), "100 plantas, 10 gerações, 50 QTL, 20 % selecionadas");
        assert_eq!(pequena.tipo().metodo(), trabalho::MetodoDeVerificacao::Recomputacao);
    }

    /// Mede a velocidade (rode com `--ignored --nocapture`, de preferência em release).
    #[test]
    #[ignore = "medida de tempo, não teste"]
    fn medir_velocidade() {
        let mut g = Gerador::novo([1, 2, 3, 4]);
        let inicio = std::time::Instant::now();
        let mut x = 0u64;
        for _ in 0..50_000_000u64 {
            x ^= g.proximo();
        }
        let s = inicio.elapsed().as_secs_f64();
        println!("gerador: {:.1} milhões de sorteios/s ({x})", 50.0 / s);
        for (n, passos, l) in [(1000u32, 20u32, 50u32), (20_000, 10, 200), (20_000, 100, 200)] {
            let e = esp(n, passos, &com(0, l));
            let inicio = std::time::Instant::now();
            let mut maior = std::time::Duration::ZERO;
            let mut antes = std::time::Instant::now();
            executar(&e, b"medida", &mut |_| {
                maior = maior.max(antes.elapsed());
                antes = std::time::Instant::now();
                true
            })
            .unwrap();
            let s = inicio.elapsed().as_secs_f64();
            let ops = operacoes(&e) as f64;
            println!(
                "N {n}, G {passos}, L {l}: {s:.2} s, {:.0} milhões de operações/s, maior pedaço {:.1} ms",
                ops / s / 1e6,
                maior.as_secs_f64() * 1e3
            );
        }
    }
}
