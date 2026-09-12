// ✝ Gênesis 41:35-36 — “Ajuntem toda a comida destes bons anos que vêm; e esta comida será para provimento da terra, para os sete anos de fome.”
//! Tipos base do Auron: dinheiro.
//!
//! Uma regra manda em tudo aqui: **dinheiro é inteiro sem sinal, sempre**.
//! Não existe `f32` nem `f64` neste crate, e nem pode existir.
//!
//! Este módulo é a tradução direta de `reference/auron/units.py`. Cada
//! comportamento é conferido contra `vectors/units.json`, gerado pela
//! implementação Python. Se os dois discordarem sobre uma única entrada, o
//! teste falha. Isso vale tanto para o que é aceito quanto para o que é
//! recusado: um parser mais permissivo de um lado é uma divergência de
//! consenso esperando acontecer.

#![forbid(unsafe_code)]
// Em teste, entrar em pânico é como se sinaliza falha. Os lints que proíbem
// isso valem para o código de produção, não para a suíte.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

use core::fmt;

/// Casas decimais de 1 AUR.
pub const AUR_DECIMALS: u32 = 8;

/// Unidades internas em 1 AUR. Igual ao satoshi do Bitcoin.
pub const AUR_UNIT: u64 = 100_000_000;

/// Teto absoluto de emissão, em unidades internas.
pub const MAX_SUPPLY: u64 = 21_000_000 * AUR_UNIT;

/// Maior valor representável. Todo campo monetário é `u64` na codificação.
pub const MAX_AMOUNT: u64 = u64::MAX;

/// Tamanho máximo do texto aceito por [`Amount::from_aur_str`].
///
/// Vinte dígitos inteiros e oito decimais cobrem qualquer valor possível; 64
/// dá folga para sinal, ponto e zeros à esquerda. Ver seção 1 da
/// `spec/AURON-SPEC-01.md`.
pub const MAX_AMOUNT_TEXT: usize = 64;

/// Motivo pelo qual um valor monetário foi recusado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmountError {
    /// A entrada tem caractere fora do ASCII.
    NaoAscii,
    /// A entrada ficou vazia depois de aparar espaços.
    Vazio,
    /// A entrada não tem nenhum dígito.
    SemDigitos,
    /// Valor monetário negativo. Não existe no protocolo.
    Negativo,
    /// Caractere que não é dígito ASCII na parte inteira ou fracionária.
    CaractereInvalido,
    /// Mais de um separador decimal.
    SeparadorRepetido,
    /// Mais de oito casas decimais. Truncar seria perder dinheiro.
    PrecisaoDemais,
    /// O valor não cabe em `u64`.
    ForaDaFaixa,
    /// O texto passa de [`MAX_AMOUNT_TEXT`] caracteres.
    TextoLongoDemais,
}

impl fmt::Display for AmountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let texto = match self {
            Self::NaoAscii => "valor deve conter apenas caracteres ASCII",
            Self::Vazio => "valor vazio",
            Self::SemDigitos => "valor sem dígitos",
            Self::Negativo => {
                "valor monetário não pode ser negativo \
                 (saldo, valor e taxa são sem sinal no protocolo)"
            }
            Self::CaractereInvalido => "caractere que não é dígito ASCII",
            Self::TextoLongoDemais => "texto de valor longo demais",
            Self::SeparadorRepetido => "mais de um separador decimal",
            Self::PrecisaoDemais => {
                "mais de 8 casas decimais (truncar seria perder dinheiro)"
            }
            Self::ForaDaFaixa => "valor fora da faixa representável",
        };
        f.write_str(texto)
    }
}

impl core::error::Error for AmountError {}

/// Um valor monetário, em unidades internas.
///
/// Não implementa `Add` nem `Sub` de propósito. Toda aritmética passa por
/// [`Amount::checked_add`] e [`Amount::checked_sub`], que devolvem `Result`.
/// Estouro silencioso em dinheiro é como se cria moeda do nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Amount(u64);

impl Amount {
    /// Zero.
    pub const ZERO: Self = Self(0);

    /// Teto de emissão.
    pub const MAX_SUPPLY: Self = Self(MAX_SUPPLY);

    /// Constrói a partir de unidades internas.
    #[must_use]
    pub const fn from_units(unidades: u64) -> Self {
        Self(unidades)
    }

    /// Devolve as unidades internas.
    #[must_use]
    pub const fn units(self) -> u64 {
        self.0
    }

    /// Verdadeiro se o valor é zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Converte texto em AUR para unidades internas.
    ///
    /// Espelha `to_units` da referência Python, incluindo o que ela recusa.
    /// As regras estão em `spec/AURON-SPEC-01.md`, seção 1.
    ///
    /// # Erros
    ///
    /// Devolve [`AmountError`] para entrada não-ASCII, negativa, malformada,
    /// com mais de oito casas decimais, ou que não caiba em `u64`.
    pub fn from_aur_str(entrada: &str) -> Result<Self, AmountError> {
        // Só ASCII. O `strip()` do Python e o `trim()` do Rust removem
        // espaço em branco Unicode por tabelas que não são idênticas em toda
        // versão. Em vez de tentar casar duas tabelas Unicode, o protocolo
        // simplesmente não aceita nada fora do ASCII.
        if !entrada.is_ascii() {
            return Err(AmountError::NaoAscii);
        }

        // Limite de tamanho antes de qualquer conversão. O maior valor
        // representável tem 20 dígitos inteiros e 8 decimais, então 64 dá
        // folga de sobra. No Python, sem este corte, uma entrada com milhões
        // de dígitos levantava `ValueError` no `int()` em vez do erro de
        // valor, e gastava CPU à toa. Os dois lados recusam pelo mesmo motivo.
        if entrada.len() > MAX_AMOUNT_TEXT {
            return Err(AmountError::TextoLongoDemais);
        }

        let texto = entrada.trim_matches([' ', '\t', '\n', '\r', '\x0c', '\x0b']);
        if texto.is_empty() {
            return Err(AmountError::Vazio);
        }

        // Negativo é recusado antes de qualquer coisa: `u64` não o representa.
        let texto = match texto.as_bytes().first() {
            Some(b'-') => return Err(AmountError::Negativo),
            Some(b'+') => texto.get(1..).unwrap_or_default(),
            _ => texto,
        };

        let (inteira, fracionaria) = match texto.split_once('.') {
            Some((antes, depois)) => (antes, depois),
            None => (texto, ""),
        };

        if fracionaria.contains('.') {
            return Err(AmountError::SeparadorRepetido);
        }
        if !inteira.is_empty() && !so_digitos_ascii(inteira) {
            return Err(AmountError::CaractereInvalido);
        }
        if !fracionaria.is_empty() && !so_digitos_ascii(fracionaria) {
            return Err(AmountError::CaractereInvalido);
        }
        if inteira.is_empty() && fracionaria.is_empty() {
            return Err(AmountError::SemDigitos);
        }
        if fracionaria.len() > AUR_DECIMALS as usize {
            return Err(AmountError::PrecisaoDemais);
        }

        let inteiro: u64 = if inteira.is_empty() {
            0
        } else {
            inteira.parse().map_err(|_| AmountError::ForaDaFaixa)?
        };

        // Completa a fração à direita até oito casas: "5" vira "50000000".
        let mut casas = [b'0'; AUR_DECIMALS as usize];
        for (destino, origem) in casas.iter_mut().zip(fracionaria.bytes()) {
            *destino = origem;
        }
        let fracao: u64 = core::str::from_utf8(&casas)
            .map_err(|_| AmountError::CaractereInvalido)?
            .parse()
            .map_err(|_| AmountError::ForaDaFaixa)?;

        inteiro
            .checked_mul(AUR_UNIT)
            .and_then(|v| v.checked_add(fracao))
            .map(Self)
            .ok_or(AmountError::ForaDaFaixa)
    }

    /// Formata como decimal com oito casas, sem passar por ponto flutuante.
    #[must_use]
    pub fn to_aur_string(self) -> String {
        let inteiro = self.0 / AUR_UNIT;
        let fracao = self.0 % AUR_UNIT;
        format!("{inteiro}.{fracao:0width$}", width = AUR_DECIMALS as usize)
    }

    /// Soma que devolve erro em vez de dar a volta.
    ///
    /// # Erros
    ///
    /// [`AmountError::ForaDaFaixa`] se o resultado não couber em `u64`.
    pub fn checked_add(self, outro: Self) -> Result<Self, AmountError> {
        self.0
            .checked_add(outro.0)
            .map(Self)
            .ok_or(AmountError::ForaDaFaixa)
    }

    /// Subtração que recusa resultado negativo. Saldo não fica devendo.
    ///
    /// # Erros
    ///
    /// [`AmountError::Negativo`] se `outro` for maior que `self`.
    pub fn checked_sub(self, outro: Self) -> Result<Self, AmountError> {
        self.0
            .checked_sub(outro.0)
            .map(Self)
            .ok_or(AmountError::Negativo)
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_aur_string())
    }
}

/// Só `0` a `9` de verdade.
///
/// `char::is_numeric` e `str::isdigit()` do Python aceitam dígito de largura
/// completa (`１`), algarismo indo-arábico oriental (`١`) e dezenas de outros.
/// Aceitar qualquer um deles faria `１` valer 1 AUR num lado e erro no outro.
/// Também fecha uma porta de falsificação visual: `１.5` e `1.5` são idênticos
/// na tela.
fn so_digitos_ascii(texto: &str) -> bool {
    !texto.is_empty() && texto.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn conversao_basica() {
        let casos = [
            ("0", 0_u64),
            ("1", AUR_UNIT),
            ("0.1", 10_000_000),
            ("0.00000001", 1),
            ("1.5", 150_000_000),
            ("21000000", MAX_SUPPLY),
            ("+2", 2 * AUR_UNIT),
            (".5", 50_000_000),
            ("7.", 7 * AUR_UNIT),
            ("  3.25  ", 325_000_000),
        ];
        for (texto, esperado) in casos {
            let obtido = Amount::from_aur_str(texto)
                .unwrap_or_else(|e| panic!("{texto:?} deveria ser aceito: {e}"));
            assert_eq!(obtido.units(), esperado, "entrada {texto:?}");
        }
    }

    #[test]
    fn recusa_o_que_deve_recusar() {
        let casos = [
            ("-1", AmountError::Negativo),
            ("-1.5", AmountError::Negativo),
            ("", AmountError::Vazio),
            ("   ", AmountError::Vazio),
            (".", AmountError::SemDigitos),
            ("abc", AmountError::CaractereInvalido),
            ("1.2.3", AmountError::SeparadorRepetido),
            ("1,5", AmountError::CaractereInvalido),
            ("1 5", AmountError::CaractereInvalido),
            ("0x10", AmountError::CaractereInvalido),
            ("1e8", AmountError::CaractereInvalido),
            ("1.123456789", AmountError::PrecisaoDemais),
            ("１", AmountError::NaoAscii),
            ("١", AmountError::NaoAscii),
            ("184467440737.09551616", AmountError::ForaDaFaixa),
        ];
        // Texto longo demais: o limite é conferido antes de qualquer conversão.
        let longo = "1".repeat(MAX_AMOUNT_TEXT + 1);
        let zeros = format!("{}1", "0".repeat(70));
        for texto in [longo.as_str(), zeros.as_str()] {
            assert_eq!(
                Amount::from_aur_str(texto),
                Err(AmountError::TextoLongoDemais),
                "texto de {} caracteres deveria ser recusado pelo tamanho",
                texto.len()
            );
        }
        for (texto, esperado) in casos {
            match Amount::from_aur_str(texto) {
                Ok(v) => panic!("{texto:?} deveria falhar, devolveu {}", v.units()),
                Err(e) => assert_eq!(e, esperado, "entrada {texto:?}"),
            }
        }
    }

    #[test]
    fn soma_de_centavos_nao_acumula_erro() {
        let dez_centavos = Amount::from_aur_str("0.1").expect("0.1 é válido");
        let mut acumulado = Amount::ZERO;
        for _ in 0..10 {
            acumulado = acumulado.checked_add(dez_centavos).expect("sem estouro");
        }
        assert_eq!(acumulado, Amount::from_aur_str("1").expect("1 é válido"));
    }

    #[test]
    fn aritmetica_conferida() {
        let a = Amount::from_units(5);
        let b = Amount::from_units(3);
        assert_eq!(a.checked_sub(b).map(Amount::units), Ok(2));
        assert_eq!(b.checked_sub(a), Err(AmountError::Negativo));
        assert_eq!(
            Amount::from_units(MAX_AMOUNT).checked_add(Amount::from_units(1)),
            Err(AmountError::ForaDaFaixa)
        );
    }

    #[test]
    fn formatacao() {
        assert_eq!(Amount::ZERO.to_aur_string(), "0.00000000");
        assert_eq!(Amount::from_units(1).to_aur_string(), "0.00000001");
        assert_eq!(Amount::from_units(AUR_UNIT).to_aur_string(), "1.00000000");
        assert_eq!(Amount::MAX_SUPPLY.to_aur_string(), "21000000.00000000");
    }
}
