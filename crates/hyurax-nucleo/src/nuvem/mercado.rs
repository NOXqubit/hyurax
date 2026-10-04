//! O mercado de máquinas: os anúncios assinados que circulam entre os nós.
//!
//! Cada nó guarda os anúncios válidos que viu (um por worker, o mais novo) e
//! repassa aos outros pares o que é novo: assim o mercado inteiro chega a
//! todos sem servidor. Quem recebe confere a assinatura e a validade; um
//! anúncio não confere nada sobre a máquina além do que o dono declarou (o
//! benchmark é ESTIMADO), e a reputação de cada worker é a que ESTE nó mediu
//! conferindo trabalho dele.

use std::collections::{BTreeMap, BTreeSet};

use hyurax_nuvem::anuncio::Anuncio;

/// Quantos anúncios o nó guarda.
pub const MAXIMO: usize = 500;
/// Anúncios aceitos de um mesmo par por minuto.
pub const POR_MINUTO_POR_PAR: u32 = 30;

/// O que aconteceu com um anúncio recebido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recebido {
    /// Novo (ou mais novo que o guardado): repassar.
    Novo,
    /// Já conhecido.
    Repetido,
    /// Recusado (assinatura, validade, ritmo, é o nosso).
    Recusado(&'static str),
}

/// O estado do mercado.
#[derive(Default)]
pub struct Mercado {
    /// Os anúncios, por worker.
    pub anuncios: BTreeMap<[u8; 32], Anuncio>,
    /// Ritmo por par: (minuto, quantos).
    ritmo: BTreeMap<u64, (u64, u32)>,
    /// Pares que já receberam o lote inicial.
    pub ja_receberam: BTreeSet<u64>,
}

impl Mercado {
    /// Recebe um anúncio de `par`.
    pub fn receber(&mut self, a: Anuncio, par: u64, meu_worker: &[u8; 32], agora_ms: u64) -> Recebido {
        let minuto = agora_ms / 60_000;
        let r = self.ritmo.entry(par).or_insert((minuto, 0));
        if r.0 != minuto {
            *r = (minuto, 0);
        }
        r.1 = r.1.saturating_add(1);
        if r.1 > POR_MINUTO_POR_PAR {
            return Recebido::Recusado("anúncios demais deste par neste minuto");
        }
        if &a.worker == meu_worker {
            return Recebido::Recusado("é o anúncio deste nó");
        }
        if !a.vale_em(agora_ms) {
            return Recebido::Recusado("anúncio vencido ou do futuro");
        }
        if !a.confere() {
            return Recebido::Recusado("assinatura não confere");
        }
        if let Some(velho) = self.anuncios.get(&a.worker)
            && velho.instante_ms >= a.instante_ms
        {
            return Recebido::Repetido;
        }
        self.anuncios.insert(a.worker, a);
        self.limpar(agora_ms);
        Recebido::Novo
    }

    /// Tira os vencidos e, acima do teto, os mais velhos.
    pub fn limpar(&mut self, agora_ms: u64) {
        self.anuncios.retain(|_, a| a.vale_em(agora_ms));
        while self.anuncios.len() > MAXIMO {
            let Some(mais_velho) = self.anuncios.iter().min_by_key(|(_, a)| a.instante_ms).map(|(w, _)| *w) else { break };
            self.anuncios.remove(&mais_velho);
        }
        self.ritmo.retain(|_, (m, _)| agora_ms / 60_000 <= m.saturating_add(1));
    }

    /// Os anúncios mais novos, até `n`.
    pub fn mais_novos(&self, n: usize) -> Vec<Anuncio> {
        let mut v: Vec<&Anuncio> = self.anuncios.values().collect();
        v.sort_by_key(|a| std::cmp::Reverse(a.instante_ms));
        v.into_iter().take(n).cloned().collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use hyurax_nuvem::anuncio::TipoDeAnuncio;

    use super::*;

    fn anuncio(segredo: u8, instante_ms: u64) -> Anuncio {
        Anuncio {
            worker: [0; 32],
            identidade: [segredo; 32],
            tipo: TipoDeAnuncio::Capacidade,
            instante_ms,
            validade_s: 3600,
            linhas: 2,
            ram_mib: 2048,
            gpu: String::new(),
            vram_mib: 0,
            disco_mib: 100,
            preco_credito_mili: 1000,
            preco_gb_mes_mili: 0,
            preco_venda_centavos: 0,
            descricao: String::new(),
            creditos_hora: 0,
            assinatura: [0; 64],
        }
        .assinar(&[segredo; 32])
        .unwrap()
    }

    #[test]
    fn guarda_o_mais_novo_e_recusa_o_ruim() {
        let mut m = Mercado::default();
        let agora = 1_790_000_000_000;
        let a = anuncio(1, agora);
        assert_eq!(m.receber(a.clone(), 1, &[0; 32], agora), Recebido::Novo);
        assert_eq!(m.receber(a.clone(), 2, &[0; 32], agora), Recebido::Repetido);
        assert_eq!(m.receber(anuncio(1, agora + 10), 2, &[0; 32], agora + 10), Recebido::Novo);
        assert_eq!(m.anuncios.len(), 1);
        let mut torto = anuncio(2, agora);
        torto.preco_credito_mili = 1;
        assert!(matches!(m.receber(torto, 3, &[0; 32], agora), Recebido::Recusado(_)));
        assert!(matches!(m.receber(a.clone(), 3, &a.worker, agora), Recebido::Recusado(_)), "o nosso não entra");
        assert!(matches!(m.receber(anuncio(3, agora), 3, &[0; 32], agora + 3_600_000), Recebido::Recusado(_)), "vencido");
        // ritmo
        for i in 0..POR_MINUTO_POR_PAR {
            let _ = m.receber(anuncio(4, agora + u64::from(i)), 9, &[0; 32], agora);
        }
        assert!(matches!(m.receber(anuncio(5, agora), 9, &[0; 32], agora), Recebido::Recusado(_)));
        m.limpar(agora + 3_700_000);
        assert!(m.anuncios.is_empty(), "vencidos saem");
    }
}
