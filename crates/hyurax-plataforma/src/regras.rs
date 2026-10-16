//! As regras que mexem com dinheiro e assinatura, traduzidas de
//! `reference/hyurax/plataforma.py` e conferidas byte a byte contra
//! `vectors/plataforma.json`.
//!
//! - [`conferir_pedido`]: o pedido assinado do agente;
//! - [`job_da_tarefa`]: o JOB próprio de cada tarefa (sementes que não se
//!   repetem entre clientes);
//! - [`custo_da_execucao`] e [`estimativa`]: o preço, por hora de núcleo de
//!   referência, em micro-unidades da moeda;
//! - [`liquidar`]: a cobrança final de uma tarefa e a divisão entre
//!   fornecedores e plataforma, fechando ao centavo;
//! - [`pontuacao`], [`amostragem_bp`] e [`quarentena`]: a reputação do nó.
//!
//! Toda conta é inteira. Onde um produto passa de 128 bits (a divisão pelo
//! maior resto), a conta é feita em 256 bits por [`mul_div`].

use std::collections::BTreeMap;

use hyurax_codec::Writer;
use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN, SIGNATURE_LEN, ed25519_sign, ed25519_verify, sha512};
use serde::{Deserialize, Serialize};

/// Domínio da assinatura do pedido do agente.
pub const DOMINIO_PEDIDO: &[u8] = concat!(hyurax_identidade::raiz!(), "-PLATAFORMA-PEDIDO-v1").as_bytes();
/// Domínio do JOB de uma tarefa.
pub const DOMINIO_JOB: &[u8] = concat!(hyurax_identidade::raiz!(), "-PLATAFORMA-JOB-v1").as_bytes();
/// Diferença máxima entre o relógio do agente e o da plataforma.
pub const JANELA_MS: u64 = 5 * 60_000;
/// Só caminhos daqui aceitam assinatura de nó.
pub const PREFIXO_DO_AGENTE: &str = "/api/v1/agente/";
/// Caminho mais longo aceito num pedido assinado.
pub const CAMINHO_MAX: usize = 512;

/// Micro-unidades num centavo (1 real = 1.000.000 micro).
pub const MICRO_POR_CENTAVO: u128 = 10_000;
/// Um gibibyte.
pub const GIB: u128 = 1 << 30;
/// Teto de operações de uma execução.
pub const OPS_MAX: u64 = 1 << 50;
/// Teto de memória de uma execução, em bytes.
pub const MEMORIA_MAX: u64 = 1 << 36;
/// Teto de cada preço da tabela, em micro por hora.
pub const PRECO_MAX: u64 = 1 << 40;
/// Teto do ritmo de referência, em operações por segundo.
pub const OPS_POR_S_MAX: u64 = 1 << 40;
/// Teto de uma estimativa, em micro.
pub const ESTIMATIVA_MAX: u128 = 1 << 100;
/// Pontos-base em 100%.
pub const BP: u32 = 10_000;
/// Entregas conferidas antes de o nó sair do período de prova.
pub const PERIODO_DE_PROVA: u64 = 10;

// ---------------------------------------------------------------------------
// 1. pedido do agente
// ---------------------------------------------------------------------------

/// `H(DOMINIO_PEDIDO || str metodo || str caminho || u64 tempo_ms || H(corpo))`.
pub fn mensagem_do_pedido(metodo: &str, caminho: &str, tempo_ms: u64, corpo: &[u8]) -> [u8; HASH_LEN] {
    let mut w = Writer::new();
    w.raw(DOMINIO_PEDIDO);
    // método e caminho curtos: a codificação de texto não falha aqui, e se
    // falhasse a mensagem sairia diferente e a assinatura não fecharia
    let _ = w.string(metodo);
    let _ = w.string(caminho);
    w.u64(tempo_ms);
    w.raw(&sha512(corpo));
    sha512(&w.into_bytes())
}

/// Assina um pedido com a chave do worker.
pub fn assinar_pedido(segredo: &[u8; SECRET_LEN], metodo: &str, caminho: &str, tempo_ms: u64, corpo: &[u8]) -> [u8; SIGNATURE_LEN] {
    ed25519_sign(segredo, &mensagem_do_pedido(metodo, caminho, tempo_ms, corpo))
}

/// Confere um pedido do agente. `Err` traz o motivo, com as palavras do gabarito.
pub fn conferir_pedido(
    chave: &[u8],
    metodo: &str,
    caminho: &str,
    tempo_ms: u64,
    corpo: &[u8],
    assinatura: &[u8],
    agora_ms: u64,
) -> Result<(), &'static str> {
    if metodo != "GET" && metodo != "POST" {
        return Err("método não aceito");
    }
    if !caminho.starts_with(PREFIXO_DO_AGENTE) || caminho.len() > CAMINHO_MAX || !caminho.bytes().all(|b| (0x21..=0x7E).contains(&b)) {
        return Err("caminho fora da área do agente");
    }
    if agora_ms.abs_diff(tempo_ms) > JANELA_MS {
        return Err("pedido fora da janela de 5 minutos");
    }
    if chave.len() != PUBKEY_LEN || assinatura.len() != SIGNATURE_LEN {
        return Err("assinatura do nó não confere");
    }
    if !ed25519_verify(chave, &mensagem_do_pedido(metodo, caminho, tempo_ms, corpo), assinatura) {
        return Err("assinatura do nó não confere");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. JOB da tarefa
// ---------------------------------------------------------------------------

/// `H(DOMINIO_JOB || JOB_ID || u64 tarefa || nonce)`.
pub fn job_da_tarefa(job_id: &[u8; HASH_LEN], tarefa: u64, nonce: &[u8; 32]) -> [u8; HASH_LEN] {
    let mut dados = Vec::with_capacity(DOMINIO_JOB.len().saturating_add(HASH_LEN + 8 + 32));
    dados.extend_from_slice(DOMINIO_JOB);
    dados.extend_from_slice(job_id);
    dados.extend_from_slice(&tarefa.to_be_bytes());
    dados.extend_from_slice(nonce);
    sha512(&dados)
}

/// A semente da unidade `indice` do JOB da tarefa: a mesma derivação do ULTRAX.
pub fn semente(job_plataforma: &[u8; HASH_LEN], indice: u64) -> [u8; HASH_LEN] {
    hyurax_ultrax::job::semente_da_unidade(job_plataforma, indice)
}

// ---------------------------------------------------------------------------
// 3. preço
// ---------------------------------------------------------------------------

/// A tabela de preços de uma moeda. Preços em micro-unidades por hora.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tabela {
    /// Uma hora de um núcleo de referência.
    pub cpu_nucleo_hora: u64,
    /// Um GiB de memória reservada por uma hora.
    pub ram_gib_hora: u64,
    /// Uma hora da GPU de referência.
    pub gpu_hora: u64,
    /// Operações por segundo do núcleo de referência.
    pub ops_por_s_cpu: u64,
    /// Operações por segundo da GPU de referência.
    pub ops_por_s_gpu: u64,
}

impl Tabela {
    /// A proposta da V1 em reais: CPU R$ 0,50/h por 4 núcleos, RAM R$ 0,10 por
    /// GB-hora, GPU R$ 3,00/h (seção 24 do documento da V1).
    pub const REAIS: Self = Self {
        cpu_nucleo_hora: 125_000,
        ram_gib_hora: 100_000,
        gpu_hora: 3_000_000,
        ops_por_s_cpu: 1_000_000_000,
        ops_por_s_gpu: 20_000_000_000,
    };
    /// A mesma tabela em dólares, na proporção de R$ 5 por dólar.
    pub const DOLARES: Self = Self {
        cpu_nucleo_hora: 25_000,
        ram_gib_hora: 20_000,
        gpu_hora: 600_000,
        ops_por_s_cpu: 1_000_000_000,
        ops_por_s_gpu: 20_000_000_000,
    };

    /// Confere os tetos.
    pub fn validar(&self) -> Result<(), &'static str> {
        if [self.cpu_nucleo_hora, self.ram_gib_hora, self.gpu_hora].iter().any(|&p| p > PRECO_MAX) {
            return Err("preço acima do teto");
        }
        if [self.ops_por_s_cpu, self.ops_por_s_gpu].iter().any(|&r| !(1..=OPS_POR_S_MAX).contains(&r)) {
            return Err("ritmo de referência fora da faixa");
        }
        Ok(())
    }
}

fn teto(a: u128, b: u128) -> Option<u128> {
    let q = a.checked_div(b)?;
    if a.checked_rem(b)? == 0 { Some(q) } else { q.checked_add(1) }
}

/// Micro-unidades da moeda por uma execução.
pub fn custo_da_execucao(t: &Tabela, operacoes: u64, memoria: u64, gpu: bool) -> Result<u128, &'static str> {
    t.validar()?;
    if operacoes > OPS_MAX {
        return Err("operações acima do teto");
    }
    if memoria > MEMORIA_MAX {
        return Err("memória acima do teto");
    }
    let ops = u128::from(operacoes);
    // com os tetos, o maior numerador é 2^50 · (2^40 · 2^30 + 2^36 · 2^40) < 2^127
    let (num, den) = if gpu {
        (ops.checked_mul(u128::from(t.gpu_hora)), u128::from(t.ops_por_s_gpu).checked_mul(3600))
    } else {
        let por_hora = u128::from(t.cpu_nucleo_hora)
            .checked_mul(GIB)
            .and_then(|c| u128::from(memoria).checked_mul(u128::from(t.ram_gib_hora)).and_then(|r| c.checked_add(r)));
        (por_hora.and_then(|p| ops.checked_mul(p)), u128::from(t.ops_por_s_cpu).checked_mul(3600).and_then(|d| d.checked_mul(GIB)))
    };
    match (num, den) {
        (Some(n), Some(d)) => teto(n, d).ok_or("operações acima do teto"),
        _ => Err("operações acima do teto"),
    }
}

/// O que se reserva do saldo: o custo pelo teto de operações, vezes unidades e cópias.
pub fn estimativa(t: &Tabela, operacoes_maximas: u64, memoria: u64, unidades: u64, copias: u64, gpu: bool) -> Result<u128, &'static str> {
    let por_unidade = custo_da_execucao(t, operacoes_maximas, memoria, gpu)?;
    por_unidade
        .checked_mul(u128::from(unidades))
        .and_then(|x| x.checked_mul(u128::from(copias)))
        .filter(|&x| x <= ESTIMATIVA_MAX)
        .ok_or("estimativa acima do teto")
}

/// Micro para centavos, arredondando para cima.
pub fn centavos(micro: u128) -> u128 {
    teto(micro, MICRO_POR_CENTAVO).unwrap_or(u128::MAX)
}

// ---------------------------------------------------------------------------
// 4. liquidação
// ---------------------------------------------------------------------------

/// `floor(a · b / c)` e `(a · b) mod c`, exatos em 256 bits. `None` quando
/// `c` é zero ou o quociente não cabe em 128 bits.
pub fn mul_div(a: u128, b: u128, c: u128) -> Option<(u128, u128)> {
    if c == 0 {
        return None;
    }
    let (alto, baixo) = mul_256(a, b);
    let mut q_alto: u128 = 0;
    let mut q_baixo: u128 = 0;
    let mut resto: u128 = 0;
    for bit in (0..256u32).rev() {
        let leva = resto >> 127;
        let entra = if bit >= 128 { (alto >> (bit.wrapping_sub(128))) & 1 } else { (baixo >> bit) & 1 };
        resto = resto.wrapping_shl(1) | entra;
        if leva == 1 || resto >= c {
            resto = resto.wrapping_sub(c);
            if bit >= 128 {
                q_alto |= 1u128.wrapping_shl(bit.wrapping_sub(128));
            } else {
                q_baixo |= 1u128.wrapping_shl(bit);
            }
        }
    }
    (q_alto == 0).then_some((q_baixo, resto))
}

/// `a · b` em 256 bits: `(alto, baixo)`.
fn mul_256(a: u128, b: u128) -> (u128, u128) {
    const M: u128 = u64::MAX as u128;
    let (a1, a0) = (a >> 64, a & M);
    let (b1, b0) = (b >> 64, b & M);
    // cada produto de duas metades de 64 bits cabe em 128
    let p00 = a0.wrapping_mul(b0);
    let p01 = a0.wrapping_mul(b1);
    let p10 = a1.wrapping_mul(b0);
    let p11 = a1.wrapping_mul(b1);
    let (meio, carrega_meio) = p01.overflowing_add(p10);
    let (baixo, carrega_baixo) = p00.overflowing_add(meio.wrapping_shl(64));
    let alto = p11
        .wrapping_add(meio >> 64)
        .wrapping_add(if carrega_meio { 1u128.wrapping_shl(64) } else { 0 })
        .wrapping_add(u128::from(carrega_baixo));
    (alto, baixo)
}

/// Quem paga uma execução.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pagador {
    /// O cliente: a execução que ele pediu.
    Cliente,
    /// A plataforma: a conferência por amostragem.
    Plataforma,
}

/// O resultado da liquidação de uma tarefa, em centavos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Liquidacao {
    /// O que o cliente paga.
    pub cobranca: u128,
    /// O que cada nó recebe.
    pub fornecedores: BTreeMap<u64, u128>,
    /// A parte das conferências, paga pela plataforma.
    pub conferencia: u128,
    /// O que fica com a plataforma; pode ser negativo.
    pub plataforma: i128,
}

/// Maior resto (Hamilton); empate vai para o nó de número menor.
pub fn repartir(total: u128, pesos: &BTreeMap<u64, u128>) -> Option<BTreeMap<u64, u128>> {
    let soma = pesos.values().try_fold(0u128, |s, &p| s.checked_add(p))?;
    if soma == 0 || total == 0 {
        return Some(pesos.keys().map(|&no| (no, 0)).collect());
    }
    let mut partes = BTreeMap::new();
    let mut restos: Vec<(u128, u64)> = Vec::with_capacity(pesos.len());
    let mut dado: u128 = 0;
    for (&no, &p) in pesos {
        let (q, r) = mul_div(total, p, soma)?;
        partes.insert(no, q);
        restos.push((r, no));
        dado = dado.checked_add(q)?;
    }
    let sobra = usize::try_from(total.checked_sub(dado)?).ok()?;
    // maior resto primeiro; no empate, o nó menor
    restos.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, no) in restos.into_iter().take(sobra) {
        if let Some(v) = partes.get_mut(&no) {
            *v = v.checked_add(1)?;
        }
    }
    Some(partes)
}

/// A liquidação de uma tarefa: `execucoes` são `(nó, micro, pagador)`.
pub fn liquidar(execucoes: &[(u64, u128, Pagador)], fornecedor_bp: u32) -> Result<Liquidacao, &'static str> {
    if fornecedor_bp > BP {
        return Err("parte do fornecedor fora da faixa");
    }
    let estouro = "liquidação acima do teto";
    let mut do_cliente: BTreeMap<u64, u128> = BTreeMap::new();
    let mut da_plataforma: BTreeMap<u64, u128> = BTreeMap::new();
    for &(no, micro, pagador) in execucoes {
        let alvo = if pagador == Pagador::Cliente { &mut do_cliente } else { &mut da_plataforma };
        let v = alvo.entry(no).or_insert(0);
        *v = v.checked_add(micro).ok_or(estouro)?;
    }
    let soma = |m: &BTreeMap<u64, u128>| m.values().try_fold(0u128, |s, &p| s.checked_add(p));
    let cobranca = centavos(soma(&do_cliente).ok_or(estouro)?);
    let bp = u128::from(fornecedor_bp);
    let bp_total = u128::from(BP);
    let pool_cliente = cobranca.checked_mul(bp).and_then(|x| x.checked_div(bp_total)).ok_or(estouro)?;
    let pool_conferencia = centavos(soma(&da_plataforma).ok_or(estouro)?).checked_mul(bp).and_then(|x| x.checked_div(bp_total)).ok_or(estouro)?;
    let mut fornecedores: BTreeMap<u64, u128> = BTreeMap::new();
    for (pool, pesos) in [(pool_cliente, &do_cliente), (pool_conferencia, &da_plataforma)] {
        for (no, v) in repartir(pool, pesos).ok_or(estouro)? {
            let t = fornecedores.entry(no).or_insert(0);
            *t = t.checked_add(v).ok_or(estouro)?;
        }
    }
    let plataforma = i128::try_from(cobranca)
        .ok()
        .and_then(|c| c.checked_sub(i128::try_from(pool_cliente).ok()?))
        .and_then(|c| c.checked_sub(i128::try_from(pool_conferencia).ok()?))
        .ok_or(estouro)?;
    Ok(Liquidacao { cobranca, fornecedores, conferencia: pool_conferencia, plataforma })
}

// ---------------------------------------------------------------------------
// 5. reputação
// ---------------------------------------------------------------------------

/// `floor(100 · (v + 7) / (v + 10 + 2f + 25i))`, de 0 a 100.
pub fn pontuacao(verificadas: u64, falhas: u64, incorretas: u64) -> u8 {
    let v = u128::from(verificadas);
    let num = v.saturating_add(7).saturating_mul(100);
    let den = v
        .saturating_add(10)
        .saturating_add(u128::from(falhas).saturating_mul(2))
        .saturating_add(u128::from(incorretas).saturating_mul(25));
    u8::try_from(num.checked_div(den).unwrap_or(0).min(100)).unwrap_or(100)
}

/// A faixa da pontuação.
pub fn faixa(p: u8) -> &'static str {
    match p {
        100.. => "excelente",
        90..=99 => "confiavel",
        70..=89 => "normal",
        50..=69 => "atencao",
        _ => "risco",
    }
}

/// Chance, em pontos-base, de uma entrega ser conferida por outro nó.
pub fn amostragem_bp(p: u8, verificadas: u64) -> u32 {
    if verificadas < PERIODO_DE_PROVA {
        return 10_000;
    }
    match p {
        90.. => 1_000,
        70..=89 => 2_500,
        50..=69 => 5_000,
        _ => 10_000,
    }
}

/// O nó vai para a quarentena?
pub fn quarentena(p: u8, incorretas: u64) -> bool {
    p < 50 || incorretas >= 3
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

    use super::*;

    #[test]
    fn mul_div_confere_com_a_conta_em_128_bits() {
        for &(a, b, c) in &[(7u128, 9u128, 4u128), (u64::MAX as u128, u64::MAX as u128, 3), (1 << 100, 1 << 27, 1 << 60), (12345, 0, 7)] {
            if let Some(p) = a.checked_mul(b) {
                assert_eq!(mul_div(a, b, c), Some((p / c, p % c)));
            }
        }
        // produto de 2^187: só a conta em 256 bits acerta
        let (q, r) = mul_div(1 << 87, 1 << 100, 1 << 90).unwrap();
        assert_eq!((q, r), (1 << 97, 0));
        let (q, r) = mul_div(u128::MAX, u128::MAX, u128::MAX).unwrap();
        assert_eq!((q, r), (u128::MAX, 0));
        assert_eq!(mul_div(u128::MAX, 2, 1), None, "quociente não cabe");
        assert_eq!(mul_div(1, 1, 0), None);
        // 3 · (2^128 − 1) = 7q + r, com 2^128 − 1 = 7k + 3: q = 3k + 1 e r = 2
        let d: u128 = 7;
        let (q, r) = mul_div(u128::MAX, 3, d).unwrap();
        let k = u128::MAX / d;
        assert_eq!(u128::MAX % d, 3);
        assert_eq!((q, r), (3 * k + 1, 2));
    }
}
