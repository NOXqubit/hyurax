//! Moeda, contas do livro e lançamentos em partida dobrada.
//!
//! Todo valor é inteiro, em centavos da moeda (`i64`). Cada lançamento tem
//! linhas que somam **zero** em cada moeda: o dinheiro só muda de conta, nunca
//! aparece nem some. A soma de todas as contas é zero o tempo todo, e é isso
//! que o teste do livro confere depois de cada fluxo.
//!
//! Sinais: o saldo de cada conta é o que a plataforma **deve** àquela parte
//! (o cliente que pagou e ainda não gastou, o fornecedor que ganhou e ainda não
//! sacou). O `caixa` é a contrapartida do dinheiro de verdade: quando entra
//! um pagamento, o caixa desce; quando sai um saque ou uma taxa, sobe. O
//! dinheiro que a empresa tem em mãos é o caixa com o sinal trocado.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// As moedas da V1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Moeda {
    /// Real.
    #[serde(rename = "BRL")]
    Brl,
    /// Dólar dos Estados Unidos.
    #[serde(rename = "USD")]
    Usd,
}

impl Moeda {
    /// Código ISO 4217.
    pub fn codigo(self) -> &'static str {
        match self {
            Self::Brl => "BRL",
            Self::Usd => "USD",
        }
    }

    /// Lê o código ISO.
    pub fn de_codigo(texto: &str) -> Option<Self> {
        match texto.trim().to_ascii_uppercase().as_str() {
            "BRL" => Some(Self::Brl),
            "USD" => Some(Self::Usd),
            _ => None,
        }
    }

    /// Valor para leitura: `R$ 1.234,56` ou `US$ 1,234.56`.
    pub fn formatar(self, centavos: i64) -> String {
        let negativo = centavos < 0;
        let abs = centavos.unsigned_abs();
        let (inteiro, fracao) = (abs / 100, abs % 100);
        let (milhar, decimal, simbolo) = match self {
            Self::Brl => ('.', ',', "R$"),
            Self::Usd => (',', '.', "US$"),
        };
        let digitos = inteiro.to_string();
        let mut agrupado = String::new();
        for (i, c) in digitos.chars().enumerate() {
            if i > 0 && (digitos.len().saturating_sub(i)) % 3 == 0 {
                agrupado.push(milhar);
            }
            agrupado.push(c);
        }
        format!("{}{simbolo} {agrupado}{decimal}{fracao:02}", if negativo { "−" } else { "" })
    }
}

/// Uma conta do livro.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Conta {
    /// Dinheiro do cliente, livre para usar.
    ClienteDisponivel(u64),
    /// Dinheiro do cliente preso a tarefas em andamento.
    ClienteReservado(u64),
    /// Ganho do fornecedor ainda no período de retenção.
    FornecedorPendente(u64),
    /// Ganho do fornecedor liberado para saque.
    FornecedorDisponivel(u64),
    /// Saque pedido, esperando o pagamento.
    FornecedorSacando(u64),
    /// Receita da plataforma (comissão menos conferências e taxas).
    PlataformaReceita,
    /// Contrapartida do dinheiro de verdade (pagamentos, saques e taxas).
    Caixa,
}

impl fmt::Display for Conta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClienteDisponivel(u) => write!(f, "cliente:{u}:disponivel"),
            Self::ClienteReservado(u) => write!(f, "cliente:{u}:reservado"),
            Self::FornecedorPendente(u) => write!(f, "fornecedor:{u}:pendente"),
            Self::FornecedorDisponivel(u) => write!(f, "fornecedor:{u}:disponivel"),
            Self::FornecedorSacando(u) => write!(f, "fornecedor:{u}:sacando"),
            Self::PlataformaReceita => f.write_str("plataforma:receita"),
            Self::Caixa => f.write_str("caixa"),
        }
    }
}

impl Conta {
    /// O inverso de `Display`.
    pub fn ler(texto: &str) -> Option<Self> {
        match texto {
            "plataforma:receita" => return Some(Self::PlataformaReceita),
            "caixa" => return Some(Self::Caixa),
            _ => {}
        }
        let mut partes = texto.split(':');
        let (lado, id, tipo) = (partes.next()?, partes.next()?.parse::<u64>().ok()?, partes.next()?);
        if partes.next().is_some() {
            return None;
        }
        match (lado, tipo) {
            ("cliente", "disponivel") => Some(Self::ClienteDisponivel(id)),
            ("cliente", "reservado") => Some(Self::ClienteReservado(id)),
            ("fornecedor", "pendente") => Some(Self::FornecedorPendente(id)),
            ("fornecedor", "disponivel") => Some(Self::FornecedorDisponivel(id)),
            ("fornecedor", "sacando") => Some(Self::FornecedorSacando(id)),
            _ => None,
        }
    }
}

impl Serialize for Conta {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Conta {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texto = String::deserialize(d)?;
        Self::ler(&texto).ok_or_else(|| serde::de::Error::custom(format!("conta desconhecida: {texto}")))
    }
}

/// Uma linha de lançamento.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Linha {
    /// A conta.
    pub conta: Conta,
    /// A moeda.
    pub moeda: Moeda,
    /// Centavos; positivo aumenta o que a plataforma deve à conta.
    pub valor: i64,
}

/// O motivo de um lançamento, para extrato e auditoria.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    /// Pagamento confirmado: entra saldo.
    Pagamento,
    /// Taxa do provedor de pagamento.
    TaxaDePagamento,
    /// Estorno de pagamento (chargeback).
    Estorno,
    /// Reserva do saldo ao criar uma tarefa.
    Reserva,
    /// Liquidação de uma tarefa: cobrança, ganhos e devolução do que sobrou.
    Liquidacao,
    /// Ganho que saiu da retenção.
    Liberacao,
    /// Saque pedido.
    SaquePedido,
    /// Saque pago.
    SaquePago,
    /// Saque recusado: volta para o disponível.
    SaqueRecusado,
    /// Ajuste manual do administrador.
    Ajuste,
}

/// Um lançamento: linhas que somam zero em cada moeda.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lancamento {
    /// Número, em ordem.
    pub id: u64,
    /// Quando.
    pub t: u64,
    /// Por quê.
    pub motivo: Motivo,
    /// A que se refere (`tarefa:12`, `pagamento:3`...).
    pub referencia: String,
    /// As linhas.
    pub linhas: Vec<Linha>,
}

impl Lancamento {
    /// Confere que as linhas fecham em zero em cada moeda e não estouram.
    pub fn fecha(&self) -> bool {
        let mut somas: BTreeMap<Moeda, i64> = BTreeMap::new();
        for l in &self.linhas {
            let s = somas.entry(l.moeda).or_insert(0);
            match s.checked_add(l.valor) {
                Some(v) => *s = v,
                None => return false,
            }
        }
        !self.linhas.is_empty() && somas.values().all(|&s| s == 0)
    }
}

/// Os saldos de todas as contas.
#[derive(Clone, Debug, Default)]
pub struct Livro {
    saldos: BTreeMap<(Conta, Moeda), i64>,
    /// Lançamentos, em ordem.
    pub lancamentos: Vec<Lancamento>,
}

impl Livro {
    /// O saldo de uma conta.
    pub fn saldo(&self, conta: Conta, moeda: Moeda) -> i64 {
        self.saldos.get(&(conta, moeda)).copied().unwrap_or(0)
    }

    /// Aplica um lançamento: confere que fecha em zero e que nenhuma conta
    /// estoura antes de mexer em qualquer saldo.
    pub fn aplicar(&mut self, l: Lancamento) -> Result<(), String> {
        if !l.fecha() {
            return Err(format!("lançamento {} não fecha em zero", l.id));
        }
        let mut deltas: BTreeMap<(Conta, Moeda), i64> = BTreeMap::new();
        for linha in &l.linhas {
            let d = deltas.entry((linha.conta, linha.moeda)).or_insert(0);
            *d = d.checked_add(linha.valor).ok_or_else(|| format!("lançamento {} estoura", l.id))?;
        }
        let mut novos = Vec::with_capacity(deltas.len());
        for (chave, delta) in deltas {
            let novo = self.saldo(chave.0, chave.1).checked_add(delta).ok_or_else(|| format!("lançamento {} estoura a conta {}", l.id, chave.0))?;
            novos.push((chave, novo));
        }
        for (chave, novo) in novos {
            self.saldos.insert(chave, novo);
        }
        self.lancamentos.push(l);
        Ok(())
    }

    /// A soma de todas as contas numa moeda: zero, sempre.
    pub fn soma(&self, moeda: Moeda) -> i64 {
        self.saldos.iter().filter(|((_, m), _)| *m == moeda).map(|(_, v)| *v).fold(0i64, i64::saturating_add)
    }

    /// Todas as contas com saldo diferente de zero.
    pub fn contas(&self) -> impl Iterator<Item = (Conta, Moeda, i64)> + '_ {
        self.saldos.iter().filter(|(_, v)| **v != 0).map(|((c, m), v)| (*c, *m, *v))
    }
}

/// Junta linhas da mesma conta e moeda e tira as que dão zero.
pub fn normalizar(linhas: Vec<Linha>) -> Vec<Linha> {
    let mut somas: BTreeMap<(Conta, Moeda), i64> = BTreeMap::new();
    for l in linhas {
        let s = somas.entry((l.conta, l.moeda)).or_insert(0);
        *s = s.saturating_add(l.valor);
    }
    somas.into_iter().filter(|(_, v)| *v != 0).map(|((conta, moeda), valor)| Linha { conta, moeda, valor }).collect()
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn contas_ida_e_volta() {
        for c in [
            Conta::ClienteDisponivel(7),
            Conta::ClienteReservado(7),
            Conta::FornecedorPendente(1),
            Conta::FornecedorDisponivel(1),
            Conta::FornecedorSacando(1),
            Conta::PlataformaReceita,
            Conta::Caixa,
        ] {
            assert_eq!(Conta::ler(&c.to_string()), Some(c));
        }
        assert_eq!(Conta::ler("cliente:7:outra"), None);
        assert_eq!(Conta::ler("cliente:x:disponivel"), None);
        assert_eq!(Conta::ler("cliente:7:disponivel:9"), None);
    }

    #[test]
    fn formatar_dinheiro() {
        assert_eq!(Moeda::Brl.formatar(123_456), "R$ 1.234,56");
        assert_eq!(Moeda::Brl.formatar(5), "R$ 0,05");
        assert_eq!(Moeda::Usd.formatar(100_000_000), "US$ 1,000,000.00");
        assert_eq!(Moeda::Brl.formatar(-250), "−R$ 2,50");
    }

    #[test]
    fn lancamento_que_nao_fecha_e_recusado() {
        let mut livro = Livro::default();
        let ruim = Lancamento {
            id: 1,
            t: 0,
            motivo: Motivo::Ajuste,
            referencia: String::new(),
            linhas: vec![Linha { conta: Conta::Caixa, moeda: Moeda::Brl, valor: -10 }, Linha { conta: Conta::ClienteDisponivel(1), moeda: Moeda::Brl, valor: 9 }],
        };
        assert!(livro.aplicar(ruim).is_err());
        let bom = Lancamento {
            id: 2,
            t: 0,
            motivo: Motivo::Pagamento,
            referencia: String::new(),
            linhas: normalizar(vec![
                Linha { conta: Conta::Caixa, moeda: Moeda::Brl, valor: -10 },
                Linha { conta: Conta::ClienteDisponivel(1), moeda: Moeda::Brl, valor: 4 },
                Linha { conta: Conta::ClienteDisponivel(1), moeda: Moeda::Brl, valor: 6 },
            ]),
        };
        livro.aplicar(bom).unwrap();
        assert_eq!(livro.saldo(Conta::ClienteDisponivel(1), Moeda::Brl), 10);
        assert_eq!(livro.soma(Moeda::Brl), 0);
    }
}
