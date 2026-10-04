//! O anúncio de uma máquina no mercado, assinado pela chave do worker.
//!
//! ```text
//! worker[32] || identidade[32] || u8 tipo || u64 instante_ms || u32 validade_s
//! || u16 linhas || u32 ram_mib || str gpu || u32 vram_mib || u64 disco_mib
//! || u64 preco_credito_mili || u64 preco_gb_mes_mili || u64 preco_venda_centavos
//! || str descricao || u64 creditos_hora || assinatura[64]
//! ```
//!
//! `assinatura = Ed25519(worker, H(DOMINIO_ANUNCIO || tudo antes dela))`.
//! `creditos_hora` vem do benchmark do próprio dono: ESTIMADO, ninguém confere.

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{PUBKEY_LEN, SECRET_LEN, SIGNATURE_LEN, ed25519_public_key, ed25519_sign, ed25519_verify, sha512};

use crate::{DOMINIO_ANUNCIO, ErroNuvem, ler_texto, texto_limitado};

/// Maior validade de um anúncio (um dia): anúncio velho some sozinho.
pub const VALIDADE_MAX_S: u32 = 86_400;
/// Maior nome de GPU, em bytes.
pub const GPU_MAX: usize = 64;
/// Maior descrição, em bytes.
pub const DESCRICAO_MAX: usize = 280;

/// O que o anúncio oferece.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoDeAnuncio {
    /// Aluga capacidade: CPU, GPU, memória, armazenamento.
    Capacidade,
    /// Aluga a máquina inteira.
    MaquinaInteira,
    /// Vende a máquina (só a listagem: a venda acontece fora da rede).
    Venda,
}

impl TipoDeAnuncio {
    /// O número no fio.
    pub fn codigo(self) -> u8 {
        match self {
            Self::Capacidade => 1,
            Self::MaquinaInteira => 2,
            Self::Venda => 3,
        }
    }

    /// Do número no fio.
    pub fn de_codigo(c: u8) -> Option<Self> {
        match c {
            1 => Some(Self::Capacidade),
            2 => Some(Self::MaquinaInteira),
            3 => Some(Self::Venda),
            _ => None,
        }
    }

    /// Nome para a tela e a API.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Capacidade => "capacidade",
            Self::MaquinaInteira => "maquina_inteira",
            Self::Venda => "venda",
        }
    }
}

/// Um anúncio de máquina.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anuncio {
    /// Chave do worker (quem assina, a mesma do registro de prova).
    pub worker: [u8; PUBKEY_LEN],
    /// Identidade do nó na rede (a chave estática da cifra).
    pub identidade: [u8; 32],
    /// O que oferece.
    pub tipo: TipoDeAnuncio,
    /// Quando foi feito, em ms desde 1970.
    pub instante_ms: u64,
    /// Por quantos segundos vale.
    pub validade_s: u32,
    /// Linhas de CPU oferecidas.
    pub linhas: u16,
    /// Memória, em MiB.
    pub ram_mib: u32,
    /// Nome da GPU (vazio: sem GPU).
    pub gpu: String,
    /// Memória da GPU, em MiB.
    pub vram_mib: u32,
    /// Disco oferecido para guardar fragmentos, em MiB.
    pub disco_mib: u64,
    /// Preço: milicréditos por crédito de computação verificado (0: não aluga).
    pub preco_credito_mili: u64,
    /// Preço do armazenamento: milicréditos por GB por mês.
    pub preco_gb_mes_mili: u64,
    /// Preço de venda, em centavos de real (só no tipo venda).
    pub preco_venda_centavos: u64,
    /// Texto livre do dono.
    pub descricao: String,
    /// Créditos por hora medidos no benchmark do dono (ESTIMADO).
    pub creditos_hora: u64,
    /// Assinatura do worker.
    pub assinatura: [u8; SIGNATURE_LEN],
}

impl Anuncio {
    /// Os bytes assinados (tudo antes da assinatura).
    ///
    /// # Errors
    /// Validade fora da faixa ou texto longo demais.
    pub fn corpo(&self) -> Result<Vec<u8>, ErroNuvem> {
        if !(1..=VALIDADE_MAX_S).contains(&self.validade_s) {
            return Err(ErroNuvem::Invalida("validade fora da faixa"));
        }
        let mut w = Writer::new();
        w.fixed(&self.worker);
        w.fixed(&self.identidade);
        w.u8(self.tipo.codigo());
        w.u64(self.instante_ms);
        w.u32(self.validade_s);
        w.u16(self.linhas);
        w.u32(self.ram_mib);
        texto_limitado(&mut w, &self.gpu, GPU_MAX)?;
        w.u32(self.vram_mib);
        w.u64(self.disco_mib);
        w.u64(self.preco_credito_mili);
        w.u64(self.preco_gb_mes_mili);
        w.u64(self.preco_venda_centavos);
        texto_limitado(&mut w, &self.descricao, DESCRICAO_MAX)?;
        w.u64(self.creditos_hora);
        Ok(w.into_bytes())
    }

    fn mensagem(corpo: &[u8]) -> [u8; 64] {
        let mut m = Vec::with_capacity(DOMINIO_ANUNCIO.len().saturating_add(corpo.len()));
        m.extend_from_slice(DOMINIO_ANUNCIO);
        m.extend_from_slice(corpo);
        sha512(&m)
    }

    /// Assina: `worker` sai do segredo e a assinatura é refeita.
    ///
    /// # Errors
    /// Campo fora da faixa.
    pub fn assinar(mut self, segredo: &[u8; SECRET_LEN]) -> Result<Self, ErroNuvem> {
        self.worker = ed25519_public_key(segredo);
        let corpo = self.corpo()?;
        self.assinatura = ed25519_sign(segredo, &Self::mensagem(&corpo));
        Ok(self)
    }

    /// A assinatura confere com o worker.
    pub fn confere(&self) -> bool {
        self.corpo().is_ok_and(|c| ed25519_verify(&self.worker, &Self::mensagem(&c), &self.assinatura))
    }

    /// Vale em `agora_ms`: já começou (com 5 min de folga para relógio
    /// adiantado) e não venceu.
    pub fn vale_em(&self, agora_ms: u64) -> bool {
        let fim = self.instante_ms.saturating_add(u64::from(self.validade_s).saturating_mul(1000));
        self.instante_ms <= agora_ms.saturating_add(300_000) && agora_ms < fim
    }

    /// Os bytes inteiros (corpo e assinatura).
    ///
    /// # Errors
    /// Campo fora da faixa.
    pub fn bytes(&self) -> Result<Vec<u8>, ErroNuvem> {
        let mut b = self.corpo()?;
        b.extend_from_slice(&self.assinatura);
        Ok(b)
    }

    /// Lê um anúncio (a assinatura não é conferida aqui: ver [`Self::confere`]).
    ///
    /// # Errors
    /// Bytes que não fecham ou campo fora da faixa.
    pub fn ler(r: &mut Reader<'_>) -> Result<Self, ErroNuvem> {
        let worker = r.fixed::<32>()?;
        let identidade = r.fixed::<32>()?;
        let tipo = TipoDeAnuncio::de_codigo(r.u8()?).ok_or(ErroNuvem::Invalida("tipo de anúncio desconhecido"))?;
        let instante_ms = r.u64()?;
        let validade_s = r.u32()?;
        if !(1..=VALIDADE_MAX_S).contains(&validade_s) {
            return Err(ErroNuvem::Invalida("validade fora da faixa"));
        }
        let linhas = r.u16()?;
        let ram_mib = r.u32()?;
        let gpu = ler_texto(r, GPU_MAX)?;
        let vram_mib = r.u32()?;
        let disco_mib = r.u64()?;
        let preco_credito_mili = r.u64()?;
        let preco_gb_mes_mili = r.u64()?;
        let preco_venda_centavos = r.u64()?;
        let descricao = ler_texto(r, DESCRICAO_MAX)?;
        let creditos_hora = r.u64()?;
        let assinatura = r.fixed::<64>()?;
        Ok(Self {
            worker,
            identidade,
            tipo,
            instante_ms,
            validade_s,
            linhas,
            ram_mib,
            gpu,
            vram_mib,
            disco_mib,
            preco_credito_mili,
            preco_gb_mes_mili,
            preco_venda_centavos,
            descricao,
            creditos_hora,
            assinatura,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    pub(crate) fn exemplo() -> Anuncio {
        Anuncio {
            worker: [0; 32],
            identidade: [9; 32],
            tipo: TipoDeAnuncio::Capacidade,
            instante_ms: 1_790_000_000_000,
            validade_s: 3600,
            linhas: 4,
            ram_mib: 8192,
            gpu: "Intel HD 400".into(),
            vram_mib: 512,
            disco_mib: 10_240,
            preco_credito_mili: 1200,
            preco_gb_mes_mili: 50_000,
            preco_venda_centavos: 0,
            descricao: "PC de casa".into(),
            creditos_hora: 3600,
            assinatura: [0; 64],
        }
    }

    #[test]
    fn assina_confere_e_adulterado_nao() {
        let a = exemplo().assinar(&[7; 32]).unwrap();
        assert!(a.confere());
        let b = a.bytes().unwrap();
        let mut r = Reader::new(&b);
        let lido = Anuncio::ler(&mut r).unwrap();
        r.finish().unwrap();
        assert_eq!(lido, a);
        let mut torto = a.clone();
        torto.preco_credito_mili = 1;
        assert!(!torto.confere());
    }

    #[test]
    fn validade() {
        let a = exemplo();
        assert!(a.vale_em(a.instante_ms + 1000));
        assert!(!a.vale_em(a.instante_ms + 3_600_000));
        assert!(!a.vale_em(a.instante_ms - 400_000), "do futuro além da folga");
        let mut longo = exemplo();
        longo.validade_s = VALIDADE_MAX_S + 1;
        assert!(longo.corpo().is_err());
    }
}
