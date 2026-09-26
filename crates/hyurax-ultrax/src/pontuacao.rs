// ✝ Provérbios 16:11 — “O peso e a balança justos pertencem ao Senhor.”
//! Work Score: a medida de contribuição computacional.
//!
//! **Não é dinheiro e não é HYX.** Nenhuma função daqui converte pontos em
//! valor. Uma recompensa, se existir, sai de regra econômica publicada à parte.
//!
//! Versão 1, a regra inteira:
//!
//! - cada tipo declara um modelo de custo em operações inteiras
//!   (ver [`crate::trabalho::Especificacao::operacoes_fixas`]);
//! - **só tarefa verificada soma**; recusada, divergente ou em disputa soma zero;
//! - 1 ponto = [`OPERACOES_POR_PONTO`] operações.
//!
//! Outros fatores (qualidade, disponibilidade, custo de verificação) entram
//! quando houver medição que os sustente, cada um com versão nova.

/// Versão da regra de pontuação.
pub const VERSAO_WORK_SCORE: u16 = 1;
/// Operações que valem um ponto.
pub const OPERACOES_POR_PONTO: u64 = 1_000_000;

/// O placar acumulado. Guarda operações, não pontos, para não perder fração.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkScore {
    /// Operações de tarefas verificadas.
    pub operacoes_verificadas: u64,
    /// Operações de tarefas que não passaram. Aparecem na tela, não pontuam.
    pub operacoes_sem_credito: u64,
}

impl WorkScore {
    /// Soma uma tarefa encerrada.
    pub fn somar(&mut self, operacoes: u64, verificada: bool) {
        let campo = if verificada { &mut self.operacoes_verificadas } else { &mut self.operacoes_sem_credito };
        *campo = campo.saturating_add(operacoes);
    }

    /// Pontos, em centésimos: `1234` quer dizer `12,34` pontos.
    pub fn centesimos(&self) -> u64 {
        // um centésimo de ponto são OPERACOES_POR_PONTO / 100 operações
        self.operacoes_verificadas.checked_div(OPERACOES_POR_PONTO / 100).unwrap_or(0)
    }

    /// Pontos com duas casas, para a tela: `12.34`.
    pub fn texto(&self) -> String {
        let c = self.centesimos();
        format!("{}.{:02}", c / 100, c % 100)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_verificada_pontua() {
        let mut s = WorkScore::default();
        s.somar(1024 * 1024 * 1024, true);
        s.somar(5_000_000, false);
        assert_eq!(s.texto(), "1073.74");
        assert_eq!(s.operacoes_sem_credito, 5_000_000);
    }

    #[test]
    fn nao_estoura() {
        let mut s = WorkScore::default();
        s.somar(u64::MAX, true);
        s.somar(u64::MAX, true);
        assert_eq!(s.operacoes_verificadas, u64::MAX);
        assert!(s.centesimos() > 0);
    }
}
