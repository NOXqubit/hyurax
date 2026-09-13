// ✝ Levítico 19:35-36 — “Não cometereis injustiça no juízo, nem na vara, nem no peso, nem na medida. Balanças justas tereis.”
//! Auron — transações (seção 5 da AURON-SPEC-01).
//!
//! Tradução de `reference/auron/tx.py`, conferida contra `vectors/transactions.json`
//! (o caminho feliz, byte a byte) e `vectors/transactions_edge.json` (as
//! recusas, com a mensagem exata do gabarito).
//!
//! Dois tipos, e só dois:
//!
//! - **Coinbase** (`kind = 0`): a recompensa do bloco. Não tem remetente nem
//!   assinatura, então não existe como forjar uma "de fora".
//! - **Transferência** (`kind = 1`, versão 2): assinada por uma conta, com de 1
//!   a 16 saídas, cada uma com destino, ativo e valor. Taxa sempre em AUR.
//!
//! O que fica fora daqui: saldo e nonce em sequência são regras do estado.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use auron_codec::{CodecError, Reader, Writer};
use auron_crypto::{
    ADDRESS_LEN, HASH_LEN, PUBKEY_LEN, SECRET_LEN, SIGNATURE_LEN, address_from_ed25519_pubkey,
    ed25519_public_key, ed25519_sign, ed25519_verify, sha512,
};
use auron_types::MAX_AMOUNT;

/// Versão da coinbase.
pub const TX_VERSION: u16 = 1;
/// Versão da transferência (multiativo).
pub const TRANSFER_VERSION: u16 = 2;
/// Tipo coinbase.
pub const KIND_COINBASE: u8 = 0;
/// Tipo transferência.
pub const KIND_TRANSFER: u8 = 1;
/// Código binário de `SIG-ED25519-V1`.
pub const SIG_CODE_ED25519: u8 = 1;
/// Maior `extra_nonce` da coinbase.
pub const MAX_EXTRA_NONCE: usize = 64;
/// Tamanho do identificador de ativo.
pub const ASSET_ID_LEN: usize = 32;
/// O AUR: identificador todo zero.
pub const AUR: [u8; ASSET_ID_LEN] = [0; ASSET_ID_LEN];
/// Máximo de saídas numa transferência.
pub const MAX_OUTPUTS: usize = 16;
/// Domínio da mensagem assinada. Muda junto com o formato.
pub const SIGNING_DOMAIN: &[u8] = b"AURON-TX-v2";

/// Endereço de conta.
pub type Endereco = [u8; ADDRESS_LEN];

/// Transação malformada. O texto é o do gabarito, letra por letra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TxError {
    /// A leitura ou escrita binária falhou.
    Codec(CodecError),
    /// Qualquer outra recusa, com a mensagem do gabarito.
    Regra(String),
}

impl fmt::Display for TxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "{e}"),
            Self::Regra(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for TxError {}

impl From<CodecError> for TxError {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

fn regra(m: impl Into<String>) -> TxError {
    TxError::Regra(m.into())
}

fn contagem_de_saidas(count: usize) -> Result<(), TxError> {
    if (1..=MAX_OUTPUTS).contains(&count) {
        Ok(())
    } else {
        Err(regra(format!("transferência precisa de 1 a {MAX_OUTPUTS} saídas, tem {count}")))
    }
}

// ---------------------------------------------------------------------------
// Coinbase
// ---------------------------------------------------------------------------

/// Recompensa do bloco. Paga sempre em AUR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coinbase {
    /// Altura do bloco.
    pub height: u64,
    /// Quem recebe.
    pub recipient: Endereco,
    /// Quanto, em unidades.
    pub amount: u64,
    /// Bytes livres para o minerador, até 64.
    pub extra_nonce: Vec<u8>,
}

impl Coinbase {
    /// Codificação canônica.
    pub fn encode(&self) -> Result<Vec<u8>, TxError> {
        if self.extra_nonce.len() > MAX_EXTRA_NONCE {
            return Err(regra("extra_nonce longo demais"));
        }
        let mut w = Writer::new();
        w.u8(KIND_COINBASE);
        w.u16(TX_VERSION);
        w.u64(self.height);
        w.fixed(&self.recipient);
        w.u64(self.amount);
        w.var_bytes(&self.extra_nonce)?;
        Ok(w.into_bytes())
    }

    fn decode_body(r: &mut Reader<'_>) -> Result<Self, TxError> {
        let version = r.u16()?;
        if version != TX_VERSION {
            return Err(regra(format!("versão de transação desconhecida: {version}")));
        }
        let height = r.u64()?;
        let recipient = r.fixed()?;
        let amount = r.u64()?;
        let extra_nonce = r.var_bytes()?;
        if extra_nonce.len() > MAX_EXTRA_NONCE {
            return Err(regra(format!(
                "extra_nonce tem {} bytes, máximo é {MAX_EXTRA_NONCE}",
                extra_nonce.len()
            )));
        }
        Ok(Self { height, recipient, amount, extra_nonce: extra_nonce.to_vec() })
    }

    /// `txid = SHA-512(codificação)`.
    pub fn txid(&self) -> Result<[u8; HASH_LEN], TxError> {
        Ok(sha512(&self.encode()?))
    }
}

// ---------------------------------------------------------------------------
// Transferência
// ---------------------------------------------------------------------------

/// Uma saída: quanto de qual ativo vai para quem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    /// Destino.
    pub recipient: Endereco,
    /// Ativo.
    pub asset_id: [u8; ASSET_ID_LEN],
    /// Valor, em unidades.
    pub amount: u64,
}

impl Output {
    fn encode(&self, w: &mut Writer) {
        w.fixed(&self.recipient);
        w.fixed(&self.asset_id);
        w.u64(self.amount);
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, TxError> {
        Ok(Self { recipient: r.fixed()?, asset_id: r.fixed()?, amount: r.u64()? })
    }
}

/// Transferência assinada de uma conta, com uma ou mais saídas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transfer {
    /// Conta que paga.
    pub sender: Endereco,
    /// Saídas, em ordem estrita de `(destino, ativo)`.
    pub outputs: Vec<Output>,
    /// Taxa, em unidades de AUR.
    pub fee: u64,
    /// Nonce da conta.
    pub nonce: u64,
    /// Chave pública. O tamanho é conferido em `check_signature`, como no gabarito.
    pub public_key: Vec<u8>,
    /// Assinatura. Idem.
    pub signature: Vec<u8>,
    /// Algoritmo de assinatura.
    pub sig_code: u8,
}

impl Transfer {
    /// Tudo menos a assinatura: é isto que é assinado.
    fn body(&self) -> Result<Writer, TxError> {
        if self.sig_code != SIG_CODE_ED25519 {
            return Err(regra(format!("algoritmo de assinatura desconhecido: {}", self.sig_code)));
        }
        contagem_de_saidas(self.outputs.len())?;
        let mut w = Writer::new();
        w.u8(KIND_TRANSFER);
        w.u16(TRANSFER_VERSION);
        w.u8(self.sig_code);
        w.fixed(&self.sender);
        w.u64(self.fee);
        w.u64(self.nonce);
        w.list(&self.outputs, |w, o| {
            o.encode(w);
            Ok::<(), TxError>(())
        })?;
        w.var_bytes(&self.public_key)?;
        Ok(w)
    }

    /// Mensagem assinada: `SIGNING_DOMAIN || magic da rede || corpo`.
    ///
    /// O magic entra para uma transação da rede de teste nunca valer na
    /// principal.
    pub fn signing_payload(&self, network_magic: &[u8; 4]) -> Result<Vec<u8>, TxError> {
        let mut m = SIGNING_DOMAIN.to_vec();
        m.extend_from_slice(network_magic);
        m.extend(self.body()?.into_bytes());
        Ok(m)
    }

    /// Codificação canônica, assinatura inclusive.
    pub fn encode(&self) -> Result<Vec<u8>, TxError> {
        let mut w = self.body()?;
        w.var_bytes(&self.signature)?;
        Ok(w.into_bytes())
    }

    fn decode_body(r: &mut Reader<'_>) -> Result<Self, TxError> {
        let version = r.u16()?;
        if version != TRANSFER_VERSION {
            return Err(regra(format!("versão de transação desconhecida: {version}")));
        }
        let sig_code = r.u8()?;
        if sig_code != SIG_CODE_ED25519 {
            return Err(regra(format!("algoritmo de assinatura desconhecido: {sig_code}")));
        }
        let sender = r.fixed()?;
        let fee = r.u64()?;
        let nonce = r.u64()?;
        // A contagem é recusada antes de ler qualquer saída: uma contagem
        // absurda não custa leitura nem memória.
        let count = r.u32()?;
        contagem_de_saidas(usize::try_from(count).unwrap_or(usize::MAX))?;
        let mut outputs = Vec::new();
        for _ in 0..count {
            outputs.push(Output::decode(r)?);
        }
        let public_key = r.var_bytes()?.to_vec();
        let signature = r.var_bytes()?.to_vec();
        Ok(Self { sender, outputs, fee, nonce, public_key, signature, sig_code })
    }

    /// `txid = SHA-512(codificação)`.
    pub fn txid(&self) -> Result<[u8; HASH_LEN], TxError> {
        Ok(sha512(&self.encode()?))
    }

    /// Quanto sai do remetente em cada ativo: as saídas, e a taxa no AUR.
    pub fn costs(&self) -> Result<BTreeMap<[u8; ASSET_ID_LEN], u64>, TxError> {
        let mut totais = BTreeMap::new();
        if self.fee != 0 {
            totais.insert(AUR, self.fee);
        }
        for saida in &self.outputs {
            let atual = totais.get(&saida.asset_id).copied().unwrap_or(0);
            // MAX_AMOUNT é u64::MAX: passar dele é exatamente o estouro da soma
            const _: () = assert!(MAX_AMOUNT == u64::MAX);
            let total = atual
                .checked_add(saida.amount)
                .ok_or_else(|| regra("valor mais taxa estoura a faixa"))?;
            totais.insert(saida.asset_id, total);
        }
        Ok(totais)
    }

    /// Estrutura e assinatura. Saldo e nonce ficam para o estado.
    ///
    /// A ordem das checagens é a do gabarito, porque ela decide qual motivo
    /// aparece.
    pub fn check_signature(&self, network_magic: &[u8; 4]) -> Result<(), String> {
        contagem_de_saidas(self.outputs.len()).map_err(|e| e.to_string())?;
        for saida in &self.outputs {
            if saida.amount == 0 {
                return Err("valor deve ser positivo".into());
            }
            if saida.asset_id != AUR {
                return Err(format!("ativo desconhecido: {}", hex(&saida.asset_id)));
            }
            if saida.recipient == self.sender {
                return Err("origem e destino iguais".into());
            }
        }
        let fora_de_ordem = self
            .outputs
            .windows(2)
            .any(|par| matches!(par, [a, b] if (a.recipient, a.asset_id) >= (b.recipient, b.asset_id)));
        if fora_de_ordem {
            return Err("saídas fora de ordem ou repetidas".into());
        }
        self.costs().map_err(|e| e.to_string())?;
        let Ok(chave) = <[u8; PUBKEY_LEN]>::try_from(self.public_key.as_slice()) else {
            return Err("tamanho de chave pública inválido".into());
        };
        if self.signature.len() != SIGNATURE_LEN {
            return Err("tamanho de assinatura inválido".into());
        }
        if address_from_ed25519_pubkey(&chave) != self.sender {
            return Err("chave pública não corresponde ao remetente".into());
        }
        let mensagem = self.signing_payload(network_magic).map_err(|e| e.to_string())?;
        if !ed25519_verify(&chave, &mensagem, &self.signature) {
            return Err("assinatura inválida".into());
        }
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Qualquer transação
// ---------------------------------------------------------------------------

/// Uma transação de qualquer tipo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tx {
    /// Recompensa do bloco.
    Coinbase(Coinbase),
    /// Transferência assinada.
    Transfer(Transfer),
}

impl Tx {
    /// Codificação canônica.
    pub fn encode(&self) -> Result<Vec<u8>, TxError> {
        match self {
            Self::Coinbase(c) => c.encode(),
            Self::Transfer(t) => t.encode(),
        }
    }

    /// `txid = SHA-512(codificação)`.
    pub fn txid(&self) -> Result<[u8; HASH_LEN], TxError> {
        Ok(sha512(&self.encode()?))
    }
}

/// Lê uma transação de dentro de um fluxo maior (o corpo do bloco).
pub fn decode_tx_from(r: &mut Reader<'_>) -> Result<Tx, TxError> {
    match r.u8()? {
        KIND_COINBASE => Ok(Tx::Coinbase(Coinbase::decode_body(r)?)),
        KIND_TRANSFER => Ok(Tx::Transfer(Transfer::decode_body(r)?)),
        outro => Err(regra(format!("tipo de transação desconhecido: {outro}"))),
    }
}

/// Lê uma transação isolada, sem aceitar byte sobrando.
pub fn decode_tx(dados: &[u8]) -> Result<Tx, TxError> {
    let mut r = Reader::new(dados);
    let tx = decode_tx_from(&mut r)?;
    r.finish()?;
    Ok(tx)
}

/// Monta e assina uma transferência com as saídas na ordem dada.
///
/// Não reordena: saída fora de ordem gera uma transferência que o consenso
/// recusa, e é assim que os testes provam a regra.
pub fn sign_transfer_outputs(
    secret: &[u8; SECRET_LEN],
    network_magic: &[u8; 4],
    sender: Endereco,
    outputs: Vec<Output>,
    fee: u64,
    nonce: u64,
) -> Result<Transfer, TxError> {
    let chave = ed25519_public_key(secret);
    if address_from_ed25519_pubkey(&chave) != sender {
        return Err(regra("segredo não corresponde ao endereço de origem"));
    }
    let mut t = Transfer {
        sender,
        outputs,
        fee,
        nonce,
        public_key: chave.to_vec(),
        signature: Vec::new(),
        sig_code: SIG_CODE_ED25519,
    };
    t.signature = ed25519_sign(secret, &t.signing_payload(network_magic)?).to_vec();
    Ok(t)
}
