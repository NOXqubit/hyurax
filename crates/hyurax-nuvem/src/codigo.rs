//! Reed–Solomon sistemático sobre GF(2^8) (polinômio 0x11D), com matriz de
//! Cauchy na paridade: quaisquer `k` dos `k + m` fragmentos reconstroem os
//! dados. Os `k` primeiros fragmentos são os próprios dados, em fatias.
//!
//! ```text
//! C[r][j] = 1 / ((k + r) XOR j)       0 ≤ r < m, 0 ≤ j < k
//! fragmento k + r, byte p = XOR_j  C[r][j] · dado_j[p]
//! ```
//!
//! Toda submatriz quadrada de uma matriz de Cauchy é invertível, e as linhas
//! da identidade (os dados) com quaisquer linhas de Cauchy também: é isso que
//! garante a reconstrução com qualquer escolha de `k` fragmentos.
//!
//! Não confere integridade: um fragmento adulterado reconstrói lixo. Quem
//! confere é o hash de cada fragmento, guardado pelo dono no manifesto.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::{ErroNuvem, K_MAX, M_MAX};

struct Tabelas {
    exp: [u8; 512],
    log: [u8; 256],
}

fn tabelas() -> &'static Tabelas {
    static T: OnceLock<Tabelas> = OnceLock::new();
    T.get_or_init(|| {
        let mut exp = [0u8; 512];
        let mut log = [0u8; 256];
        let mut x: u16 = 1;
        for i in 0..255u8 {
            let byte = u8::try_from(x & 0xFF).unwrap_or(0);
            if let Some(e) = exp.get_mut(usize::from(i)) {
                *e = byte;
            }
            if let Some(l) = log.get_mut(usize::from(byte)) {
                *l = i;
            }
            x = x.wrapping_shl(1);
            if x & 0x100 != 0 {
                x ^= 0x11D;
            }
        }
        for i in 255..512usize {
            let v = exp.get(i.wrapping_sub(255)).copied().unwrap_or(0);
            if let Some(e) = exp.get_mut(i) {
                *e = v;
            }
        }
        Tabelas { exp, log }
    })
}

/// Produto em GF(2^8).
pub fn gf_mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let t = tabelas();
    let la = usize::from(t.log.get(usize::from(a)).copied().unwrap_or(0));
    let lb = usize::from(t.log.get(usize::from(b)).copied().unwrap_or(0));
    t.exp.get(la.saturating_add(lb)).copied().unwrap_or(0)
}

/// Inverso em GF(2^8); zero não tem.
pub fn gf_inv(a: u8) -> Option<u8> {
    if a == 0 {
        return None;
    }
    let t = tabelas();
    let la = usize::from(t.log.get(usize::from(a)).copied().unwrap_or(0));
    t.exp.get(255usize.saturating_sub(la)).copied()
}

/// A tabela de multiplicação por `c` (256 entradas): o laço interno vira
/// uma consulta por byte.
fn linha_de_produtos(c: u8) -> [u8; 256] {
    let mut v = [0u8; 256];
    for (b, saida) in (0..=255u8).zip(v.iter_mut()) {
        *saida = gf_mul(c, b);
    }
    v
}

fn conferir(k: usize, m: usize) -> Result<(), ErroNuvem> {
    if (1..=K_MAX).contains(&k) && m <= M_MAX {
        Ok(())
    } else {
        Err(ErroNuvem::Invalida("k ou m fora da faixa"))
    }
}

/// A matriz de Cauchy da paridade (`m` linhas de `k`).
///
/// # Errors
/// `k` ou `m` fora da faixa.
pub fn cauchy(k: usize, m: usize) -> Result<Vec<Vec<u8>>, ErroNuvem> {
    conferir(k, m)?;
    let mut c = Vec::with_capacity(m);
    for r in 0..m {
        let x = u8::try_from(k.saturating_add(r)).map_err(|_| ErroNuvem::Invalida("k ou m fora da faixa"))?;
        let mut linha = Vec::with_capacity(k);
        for j in 0..k {
            let y = u8::try_from(j).map_err(|_| ErroNuvem::Invalida("k ou m fora da faixa"))?;
            linha.push(gf_inv(x ^ y).ok_or(ErroNuvem::Invalida("matriz de Cauchy com divisão por zero"))?);
        }
        c.push(linha);
    }
    Ok(c)
}

/// Tamanho de cada fragmento para `n` bytes em `k` fatias: `max(1, ceil(n/k))`.
pub fn tamanho_do_fragmento(n: usize, k: usize) -> usize {
    n.div_ceil(k.max(1)).max(1)
}

/// Os `k + m` fragmentos de `dados`.
///
/// # Errors
/// `k` ou `m` fora da faixa.
pub fn codificar(dados: &[u8], k: usize, m: usize) -> Result<Vec<Vec<u8>>, ErroNuvem> {
    conferir(k, m)?;
    let largura = tamanho_do_fragmento(dados.len(), k);
    let mut fragmentos: Vec<Vec<u8>> = dados.chunks(largura).map(<[u8]>::to_vec).collect();
    fragmentos.resize(k, Vec::new());
    for f in &mut fragmentos {
        f.resize(largura, 0);
    }
    let c = cauchy(k, m)?;
    for linha in &c {
        let mut saida = vec![0u8; largura];
        for (coef, bloco) in linha.iter().zip(fragmentos.iter()) {
            let produtos = linha_de_produtos(*coef);
            for (s, b) in saida.iter_mut().zip(bloco.iter()) {
                *s ^= produtos.get(usize::from(*b)).copied().unwrap_or(0);
            }
        }
        fragmentos.push(saida);
    }
    Ok(fragmentos)
}

fn inverter(mut a: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, ErroNuvem> {
    let n = a.len();
    for (i, linha) in a.iter_mut().enumerate() {
        linha.resize(n.saturating_mul(2), 0);
        if let Some(c) = linha.get_mut(n.saturating_add(i)) {
            *c = 1;
        }
    }
    for col in 0..n {
        let piv = (col..n)
            .find(|&r| a.get(r).and_then(|l| l.get(col)).is_some_and(|v| *v != 0))
            .ok_or(ErroNuvem::Invalida("matriz singular"))?;
        a.swap(col, piv);
        let diagonal = a.get(col).and_then(|l| l.get(col)).copied().unwrap_or(0);
        let inv = gf_inv(diagonal).ok_or(ErroNuvem::Invalida("matriz singular"))?;
        if let Some(l) = a.get_mut(col) {
            for v in l.iter_mut() {
                *v = gf_mul(*v, inv);
            }
        }
        let pivo: Vec<u8> = a.get(col).cloned().unwrap_or_default();
        for (r, l) in a.iter_mut().enumerate() {
            if r == col {
                continue;
            }
            let f = l.get(col).copied().unwrap_or(0);
            if f != 0 {
                for (v, w) in l.iter_mut().zip(pivo.iter()) {
                    *v ^= gf_mul(f, *w);
                }
            }
        }
    }
    Ok(a.into_iter().map(|l| l.get(n..).map(<[u8]>::to_vec).unwrap_or_default()).collect())
}

/// Os dados originais (`tamanho` bytes) a partir de quaisquer `k`
/// fragmentos (índice → bytes). Usa os `k` de menor índice.
///
/// # Errors
/// `k` ou `m` fora da faixa, fragmentos insuficientes ou de tamanho errado.
pub fn reconstruir(fragmentos: &BTreeMap<usize, Vec<u8>>, k: usize, m: usize, tamanho: usize) -> Result<Vec<u8>, ErroNuvem> {
    conferir(k, m)?;
    let largura = tamanho_do_fragmento(tamanho, k);
    let total = k.saturating_add(m);
    let usados: Vec<(usize, &Vec<u8>)> = fragmentos.iter().filter(|(i, _)| **i < total).take(k).map(|(i, f)| (*i, f)).collect();
    if usados.len() < k {
        return Err(ErroNuvem::Invalida("fragmentos insuficientes"));
    }
    if usados.iter().any(|(_, f)| f.len() != largura) {
        return Err(ErroNuvem::Invalida("fragmento de tamanho errado"));
    }
    // todos os de dados presentes: é só juntar
    if usados.iter().enumerate().all(|(j, (i, _))| *i == j) {
        let mut saida: Vec<u8> = usados.iter().flat_map(|(_, f)| f.iter().copied()).collect();
        saida.truncate(tamanho);
        return Ok(saida);
    }
    let c = cauchy(k, m)?;
    let mut matriz = Vec::with_capacity(k);
    for (i, _) in &usados {
        if *i < k {
            matriz.push((0..k).map(|j| u8::from(j == *i)).collect());
        } else {
            matriz.push(c.get(i.saturating_sub(k)).cloned().ok_or(ErroNuvem::Invalida("índice de fragmento fora da faixa"))?);
        }
    }
    let inv = inverter(matriz)?;
    let mut saida = Vec::with_capacity(largura.saturating_mul(k));
    for linha in &inv {
        let mut bloco = vec![0u8; largura];
        for (coef, (_, frag)) in linha.iter().zip(usados.iter()) {
            if *coef == 0 {
                continue;
            }
            let produtos = linha_de_produtos(*coef);
            for (s, b) in bloco.iter_mut().zip(frag.iter()) {
                *s ^= produtos.get(usize::from(*b)).copied().unwrap_or(0);
            }
        }
        saida.extend_from_slice(&bloco);
    }
    saida.truncate(tamanho);
    Ok(saida)
}

/// A prova de guarda: `H(DOMINIO_PROVA || nonce || fragmento)`. Só quem tem
/// o fragmento inteiro responde certo a um nonce que nunca viu.
pub fn prova(nonce: &[u8; 32], fragmento: &[u8]) -> [u8; hyurax_crypto::HASH_LEN] {
    let mut m = Vec::with_capacity(crate::DOMINIO_PROVA.len().saturating_add(32).saturating_add(fragmento.len()));
    m.extend_from_slice(crate::DOMINIO_PROVA);
    m.extend_from_slice(nonce);
    m.extend_from_slice(fragmento);
    hyurax_crypto::sha512(&m)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod testes {
    use super::*;

    #[test]
    fn inverso_e_produto() {
        for a in 1..=255u8 {
            assert_eq!(gf_mul(a, gf_inv(a).unwrap()), 1);
        }
        assert_eq!(gf_mul(2, 0x80), 0x1D);
        assert_eq!(gf_inv(0), None);
    }

    #[test]
    fn qualquer_escolha_de_k_reconstroi() {
        let dados: Vec<u8> = (0..1000u32).map(|i| ((i * 37 + 11) % 256) as u8).collect();
        for (k, m) in [(1, 0), (1, 2), (2, 1), (3, 2), (4, 4)] {
            let f = codificar(&dados, k, m).unwrap();
            let n = k + m;
            for mascara in 0u32..(1 << n) {
                if mascara.count_ones() as usize != k {
                    continue;
                }
                let escolhidos: BTreeMap<usize, Vec<u8>> = (0..n).filter(|i| mascara & (1 << i) != 0).map(|i| (i, f[i].clone())).collect();
                assert_eq!(reconstruir(&escolhidos, k, m, dados.len()).unwrap(), dados, "k={k} m={m} mascara={mascara:b}");
            }
        }
    }

    #[test]
    fn poucos_fragmentos_e_tamanho_errado_sao_recusados() {
        let f = codificar(b"abcdef", 3, 2).unwrap();
        let dois: BTreeMap<usize, Vec<u8>> = [(0, f[0].clone()), (4, f[4].clone())].into();
        assert!(reconstruir(&dois, 3, 2, 6).is_err());
        let torto: BTreeMap<usize, Vec<u8>> = [(0, f[0].clone()), (1, f[1].clone()), (3, vec![1])].into();
        assert!(reconstruir(&torto, 3, 2, 6).is_err());
        assert!(codificar(b"x", 0, 1).is_err());
        assert!(codificar(b"x", 33, 0).is_err());
        assert!(codificar(b"x", 2, 33).is_err());
    }
}
