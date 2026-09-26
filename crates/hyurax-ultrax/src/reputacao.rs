// ✝ Provérbios 20:11 — “Até a criança se dará a conhecer pelas suas ações.”
//! A reputação de um worker: o que ele entregou e como foi julgado.
//!
//! A nota é pública e simples, para qualquer um refazer a conta:
//!
//! ```text
//! nota = (verificadas + 1) / (verificadas + recusadas + 2), em milésimos
//! ```
//!
//! Worker novo começa em 500 (meio a meio, sem histórico). Só recusa com
//! evidência baixa a nota; divergência sem prova e disputa não contam contra.

use crate::validador::Parecer;

/// Os números de um worker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reputacao {
    /// Entregas feitas.
    pub enviadas: u64,
    /// Aceitas na conferência.
    pub verificadas: u64,
    /// Recusadas com evidência.
    pub recusadas: u64,
    /// Diferentes da maioria, sem prova de erro.
    pub divergentes: u64,
    /// Em tarefas que terminaram sem maioria.
    pub disputadas: u64,
    /// Segundos em que o worker esteve disponível.
    pub segundos_ativo: u64,
    /// Soma das latências medidas, em milissegundos.
    pub latencia_total_ms: u64,
    /// Quantas latências foram somadas.
    pub latencias: u64,
}

impl Reputacao {
    /// Conta um parecer do validador.
    pub fn registrar(&mut self, parecer: &Parecer) {
        let campo = match parecer {
            Parecer::Aceito => &mut self.verificadas,
            Parecer::Recusado(_) => &mut self.recusadas,
            Parecer::Divergente => &mut self.divergentes,
            // entrega repetida não é trabalho novo
            Parecer::Repetida => return,
        };
        *campo = campo.saturating_add(1);
        self.enviadas = self.enviadas.saturating_add(1);
    }

    /// Conta uma tarefa que terminou em disputa.
    pub fn registrar_disputa(&mut self) {
        self.disputadas = self.disputadas.saturating_add(1);
    }

    /// Soma uma latência medida.
    pub fn medir_latencia(&mut self, ms: u64) {
        self.latencia_total_ms = self.latencia_total_ms.saturating_add(ms);
        self.latencias = self.latencias.saturating_add(1);
    }

    /// Latência média, em milissegundos.
    pub fn latencia_media_ms(&self) -> Option<u64> {
        self.latencia_total_ms.checked_div(self.latencias)
    }

    /// Verificadas sobre julgadas (verificadas + recusadas), em milésimos.
    pub fn taxa_de_acerto(&self) -> Option<u64> {
        let julgadas = self.verificadas.saturating_add(self.recusadas);
        self.verificadas.saturating_mul(1000).checked_div(julgadas)
    }

    /// A nota, de 0 a 1000. Ver a fórmula no topo do módulo.
    pub fn nota(&self) -> u64 {
        let acima = self.verificadas.saturating_add(1).saturating_mul(1000);
        let abaixo = self.verificadas.saturating_add(self.recusadas).saturating_add(2);
        acima.checked_div(abaixo).unwrap_or(500)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::trabalho::Recusa;

    #[test]
    fn novo_comeca_no_meio() {
        let r = Reputacao::default();
        assert_eq!(r.nota(), 500);
        assert_eq!(r.taxa_de_acerto(), None);
        assert_eq!(r.latencia_media_ms(), None);
    }

    #[test]
    fn so_recusa_com_evidencia_baixa_a_nota() {
        let mut r = Reputacao::default();
        for _ in 0..8 {
            r.registrar(&Parecer::Aceito);
        }
        let antes = r.nota();
        r.registrar(&Parecer::Divergente);
        r.registrar_disputa();
        r.registrar(&Parecer::Repetida);
        assert_eq!(r.nota(), antes, "divergência, disputa e repetição não pesam");
        r.registrar(&Parecer::Recusado(Recusa("errado".into())));
        assert!(r.nota() < antes);
        assert_eq!(r.enviadas, 10);
        assert_eq!(r.taxa_de_acerto(), Some(888));
    }

    #[test]
    fn nota_confere_com_a_formula() {
        let r = Reputacao { verificadas: 181, recusadas: 3, ..Reputacao::default() };
        assert_eq!(r.nota(), 182 * 1000 / 186);
    }
}
