// ✝ Provérbios 22:1 — “Mais digno de ser escolhido é o bom nome do que as muitas riquezas.”
//! Identidade do projeto, num lugar só.
//!
//! O nome aparece em dois papéis que **não** podem ser tratados igual:
//!
//! 1. **Exibição** — o que pessoas leem: nome do projeto, da rede, da moeda e
//!    do ticker. Trocar é mudar as constantes de [`exibicao`], e nada no
//!    consenso muda.
//! 2. **Protocolo** — rótulos que entram em hash, assinatura e aperto de mão:
//!    o sal do PoW, o domínio de assinatura das transações, as marcas de rede,
//!    a gênese, o prólogo da cifra. Todos derivam de [`raiz!`] e das
//!    constantes de [`protocolo`], resolvidos **na compilação**.
//!
//! Os rótulos de protocolo nunca vêm de variável de ambiente nem de arquivo de
//! configuração: dois nós configurados diferente calculariam hashes
//! diferentes e a rede se partiria em duas. Trocar a raiz é uma decisão
//! consciente, feita uma vez, que regera os vetores e cria uma gênese nova.

#![no_std]
#![forbid(unsafe_code)]

/// Raiz de todos os rótulos de protocolo, como texto literal.
///
/// É macro, e não constante, para poder entrar em `concat!` e virar um
/// `&'static str` resolvido na compilação: `concat!(raiz!(), "-TX-v2")`.
#[macro_export]
macro_rules! raiz {
    () => {
        "HYURAX"
    };
}

/// Prefixo dos nomes de rede (`<prefixo>-mainnet`, `-testnet`, `-regtest`).
#[macro_export]
macro_rules! prefixo_rede {
    () => {
        "hyurax"
    };
}

/// O que pessoas leem. Mudar aqui não mexe em consenso.
pub mod exibicao {
    /// Nome do projeto e da empresa.
    pub const PROJETO: &str = "Hyurax";
    /// Nome da rede.
    pub const REDE: &str = "Hyurax Network";
    /// Nome da moeda.
    pub const MOEDA: &str = "Hyurax";
    /// Ticker da moeda.
    pub const TICKER: &str = "HYX";
}

/// Rótulos de protocolo. Mudar qualquer um destes é criar uma rede nova.
pub mod protocolo {
    /// Marca da rede principal, no início de cada quadro e na assinatura.
    pub const MAGIC_MAINNET: [u8; 4] = *b"HYXM";
    /// Marca da rede de teste.
    pub const MAGIC_TESTNET: [u8; 4] = *b"HYXT";
    /// Marca da rede local de desenvolvimento.
    pub const MAGIC_REGTEST: [u8; 4] = *b"HYXR";
    /// Primeiros 8 bytes do arquivo da cadeia em disco.
    pub const STORE_MAGIC: [u8; 8] = *b"HYURXDB1";

    /// Preenche um rótulo com zeros até 16 bytes, na compilação.
    ///
    /// Rótulo maior que 16 bytes para a compilação, em vez de cortar em
    /// silêncio: o sal do PoW precisa de exatamente 16.
    #[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
    pub const fn preencher_16(rotulo: &[u8]) -> [u8; 16] {
        assert!(rotulo.len() <= 16, "rótulo de protocolo maior que 16 bytes");
        let mut saida = [0u8; 16];
        let mut i = 0;
        while i < rotulo.len() {
            saida[i] = rotulo[i];
            i += 1;
        }
        saida
    }
}

#[cfg(test)]
mod testes {
    use super::protocolo::preencher_16;

    #[test]
    fn preencher_completa_com_zeros() {
        assert_eq!(&preencher_16(b"ABC")[..4], b"ABC\0");
        assert_eq!(preencher_16(b"0123456789abcdef"), *b"0123456789abcdef");
    }

    #[test]
    fn rotulos_derivam_da_raiz() {
        assert!(concat!(raiz!(), "-TX-v2").starts_with(raiz!()));
        assert!(concat!(prefixo_rede!(), "-testnet").ends_with("-testnet"));
    }
}
