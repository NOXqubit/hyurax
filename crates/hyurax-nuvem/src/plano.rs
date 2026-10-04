//! Planos: o catálogo e o voucher de plano assinado (`docs/MONETIZACAO.md`).
//!
//! Tradução de `reference/hyurax/plano.py`, conferida por
//! `vectors/plano.json`. Fora do consenso. O preço é em reais e é cobrado fora
//! do programa, por uma empresa; o voucher só diz qual plano um nó tem, de
//! quando a quando, assinado pela chave de planos do projeto. Sem dado
//! pessoal: o beneficiário é a chave pública do worker do nó.
//!
//! ```text
//! u8 VERSAO || u8 plano || beneficiario[32] || u64 inicio_ms || u64 fim_ms
//! || u64 creditos_mes || u32 armazenamento_gib || u16 comissao_bp
//! || u32 serie || assinatura[64]
//! ```
//!
//! `assinatura = Ed25519(emissor, H(DOMINIO_PLANO || tudo antes dela))`.

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{PUBKEY_LEN, SECRET_LEN, SIGNATURE_LEN, ed25519_sign, ed25519_verify, sha512};

use crate::ErroNuvem;

/// Domínio da assinatura do voucher.
pub const DOMINIO_PLANO: &[u8] = concat!(hyurax_identidade::raiz!(), "-PLANO-v1").as_bytes();
/// Versão do voucher.
pub const VERSAO_PLANO: u8 = 1;
/// Prefixo do voucher em texto.
pub const PREFIXO: &str = "hyurax-plano:";
/// Duração máxima de um voucher.
pub const DURACAO_MAX_MS: u64 = 400 * 86_400_000;
/// Tamanho do voucher em bytes.
pub const TAMANHO: usize = 1 + 1 + 32 + 8 + 8 + 8 + 4 + 2 + 4 + SIGNATURE_LEN;

/// Um plano do catálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plano {
    /// Identificador (vai no voucher).
    pub id: u8,
    /// Nome para a tela.
    pub nome: &'static str,
    /// Preço por mês, em centavos de real (0: grátis; Empresa: "a partir de").
    pub mensal_centavos: u64,
    /// Preço por ano, em centavos de real.
    pub anual_centavos: u64,
    /// Créditos por mês na capacidade gerenciada.
    pub creditos_mes: u64,
    /// Armazenamento gerenciado, em GiB.
    pub armazenamento_gib: u32,
    /// Comissão da plataforma no mercado de máquinas, em pontos-base.
    pub comissao_bp: u16,
}

/// O plano grátis.
pub const COMUNIDADE: u8 = 0;
/// Pro.
pub const PRO: u8 = 1;
/// Equipe.
pub const EQUIPE: u8 = 2;
/// Empresa.
pub const EMPRESA: u8 = 3;

/// O catálogo. Mudar um preço aqui vale para vouchers novos; os já emitidos
/// levam os benefícios escritos neles.
pub const CATALOGO: [Plano; 4] = [
    Plano { id: COMUNIDADE, nome: "Comunidade", mensal_centavos: 0, anual_centavos: 0, creditos_mes: 0, armazenamento_gib: 0, comissao_bp: 1500 },
    Plano { id: PRO, nome: "Pro", mensal_centavos: 2_900, anual_centavos: 29_000, creditos_mes: 150_000, armazenamento_gib: 50, comissao_bp: 1000 },
    Plano {
        id: EQUIPE,
        nome: "Equipe",
        mensal_centavos: 14_900,
        anual_centavos: 149_000,
        creditos_mes: 1_000_000,
        armazenamento_gib: 500,
        comissao_bp: 800,
    },
    Plano {
        id: EMPRESA,
        nome: "Empresa",
        mensal_centavos: 99_000,
        anual_centavos: 990_000,
        creditos_mes: 10_000_000,
        armazenamento_gib: 5_000,
        comissao_bp: 500,
    },
];

/// O plano do catálogo com este id.
pub fn do_catalogo(id: u8) -> Option<Plano> {
    CATALOGO.iter().find(|p| p.id == id).copied()
}

/// Um voucher de plano.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voucher {
    /// Qual plano.
    pub plano: u8,
    /// A chave do worker do nó que recebe o plano.
    pub beneficiario: [u8; PUBKEY_LEN],
    /// Começo, em ms desde 1970 (incluído).
    pub inicio_ms: u64,
    /// Fim, em ms desde 1970 (excluído).
    pub fim_ms: u64,
    /// Créditos por mês na capacidade gerenciada.
    pub creditos_mes: u64,
    /// Armazenamento gerenciado, em GiB.
    pub armazenamento_gib: u32,
    /// Comissão da plataforma no mercado, em pontos-base.
    pub comissao_bp: u16,
    /// Número de série (de quem emite).
    pub serie: u32,
    /// Assinatura da chave de planos.
    pub assinatura: [u8; SIGNATURE_LEN],
}

impl Voucher {
    /// Os bytes assinados, com as regras de quem emite.
    ///
    /// # Errors
    /// Plano grátis ou desconhecido, datas fora de ordem, duração ou comissão
    /// fora da faixa.
    pub fn corpo(&self) -> Result<Vec<u8>, ErroNuvem> {
        if self.plano == COMUNIDADE || do_catalogo(self.plano).is_none() {
            return Err(ErroNuvem::Invalida("plano desconhecido"));
        }
        if self.fim_ms <= self.inicio_ms {
            return Err(ErroNuvem::Invalida("o voucher acaba antes de começar"));
        }
        if self.fim_ms.saturating_sub(self.inicio_ms) > DURACAO_MAX_MS {
            return Err(ErroNuvem::Invalida("voucher mais longo que 400 dias"));
        }
        if self.comissao_bp > 10_000 {
            return Err(ErroNuvem::Invalida("comissão acima de 100%"));
        }
        let mut w = Writer::new();
        w.u8(VERSAO_PLANO);
        w.u8(self.plano);
        w.fixed(&self.beneficiario);
        w.u64(self.inicio_ms);
        w.u64(self.fim_ms);
        w.u64(self.creditos_mes);
        w.u32(self.armazenamento_gib);
        w.u16(self.comissao_bp);
        w.u32(self.serie);
        Ok(w.into_bytes())
    }

    fn mensagem(&self) -> Result<[u8; 64], ErroNuvem> {
        let mut m = DOMINIO_PLANO.to_vec();
        m.extend(self.corpo()?);
        Ok(sha512(&m))
    }

    /// Emite um voucher com os benefícios do catálogo de hoje.
    ///
    /// # Errors
    /// As mesmas regras de [`Voucher::corpo`].
    pub fn emitir(
        segredo: &[u8; SECRET_LEN],
        plano: u8,
        beneficiario: [u8; PUBKEY_LEN],
        inicio_ms: u64,
        fim_ms: u64,
        serie: u32,
    ) -> Result<Self, ErroNuvem> {
        let p = do_catalogo(plano).ok_or(ErroNuvem::Invalida("plano desconhecido"))?;
        let mut v = Self {
            plano,
            beneficiario,
            inicio_ms,
            fim_ms,
            creditos_mes: p.creditos_mes,
            armazenamento_gib: p.armazenamento_gib,
            comissao_bp: p.comissao_bp,
            serie,
            assinatura: [0; SIGNATURE_LEN],
        };
        v.assinatura = ed25519_sign(segredo, &v.mensagem()?);
        Ok(v)
    }

    /// Bytes completos, assinatura inclusive.
    ///
    /// # Errors
    /// As mesmas regras de [`Voucher::corpo`].
    pub fn bytes(&self) -> Result<Vec<u8>, ErroNuvem> {
        let mut b = self.corpo()?;
        b.extend_from_slice(&self.assinatura);
        Ok(b)
    }

    /// `hyurax-plano:` e os bytes em hexadecimal.
    ///
    /// # Errors
    /// As mesmas regras de [`Voucher::corpo`].
    pub fn texto(&self) -> Result<String, ErroNuvem> {
        Ok(format!("{PREFIXO}{}", self.bytes()?.iter().map(|b| format!("{b:02x}")).collect::<String>()))
    }

    /// Lê os bytes (a assinatura não é conferida aqui).
    ///
    /// # Errors
    /// Bytes que não fecham, sobram ou ferem as regras de quem emite.
    pub fn ler(dados: &[u8]) -> Result<Self, ErroNuvem> {
        let mut r = Reader::new(dados);
        if r.u8()? != VERSAO_PLANO {
            return Err(ErroNuvem::Invalida("versão de voucher desconhecida"));
        }
        let v = Self {
            plano: r.u8()?,
            beneficiario: r.fixed::<32>()?,
            inicio_ms: r.u64()?,
            fim_ms: r.u64()?,
            creditos_mes: r.u64()?,
            armazenamento_gib: r.u32()?,
            comissao_bp: r.u16()?,
            serie: r.u32()?,
            assinatura: r.fixed::<64>()?,
        };
        r.finish()?;
        v.corpo()?;
        Ok(v)
    }

    /// Lê o texto `hyurax-plano:…`.
    ///
    /// # Errors
    /// Prefixo errado, tamanho errado ou caractere que não é hexadecimal, e os
    /// erros de [`Voucher::ler`].
    pub fn ler_texto(texto: &str) -> Result<Self, ErroNuvem> {
        let t = texto.trim();
        let h = t.strip_prefix(PREFIXO).ok_or(ErroNuvem::Invalida("isto não é um voucher de plano do Hyurax"))?;
        if h.len() != TAMANHO.saturating_mul(2) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ErroNuvem::Invalida("voucher com tamanho errado ou caractere inválido"));
        }
        let bytes: Vec<u8> = (0..TAMANHO)
            .map(|i| h.get(i.saturating_mul(2)..i.saturating_mul(2).saturating_add(2)).and_then(|x| u8::from_str_radix(x, 16).ok()).unwrap_or(0))
            .collect();
        Self::ler(&bytes)
    }

    /// A assinatura confere com a chave de planos?
    pub fn assinatura_confere(&self, emissor: &[u8; PUBKEY_LEN]) -> bool {
        self.mensagem().is_ok_and(|m| ed25519_verify(emissor, &m, &self.assinatura))
    }

    /// Vale para este worker, agora?
    pub fn vale(&self, emissor: &[u8; PUBKEY_LEN], worker: &[u8; PUBKEY_LEN], agora_ms: u64) -> bool {
        self.assinatura_confere(emissor) && self.beneficiario == *worker && (self.inicio_ms..self.fim_ms).contains(&agora_ms)
    }
}
