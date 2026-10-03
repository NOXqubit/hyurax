//! Pacote do Éter dentro do nó (`docs/HYURAX-MALHA.md`, etapa M5): as
//! transações e os blocos saem num arquivo que atravessa pendrive, Bluetooth,
//! som ou rádio (`eter enviar`), e entram em qualquer nó que o receba.
//!
//! Quem importa não confia no pacote: cada quadro passa pela mesma porta que
//! um quadro de par (prova de trabalho fora da trava, validação completa,
//! mempool com as mesmas regras). Quadro ruim é recusado sozinho, sem banir
//! ninguém: o pacote pode ter passado por muitas mãos. O que entra é
//! repassado aos pares conectados, e assim uma transação feita num aparelho
//! sem internet chega à rede pelo primeiro nó conectado que receber o pacote.

use std::sync::Arc;

use hyurax_wire::{Message, decode_frame, encode_frame};

use super::Rede;
use crate::malha::{MAX_QUADROS_NO_PACOTE, Pacote};

/// O que a importação fez.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResultadoDoPacote {
    /// Blocos que entraram (ou ficaram guardados esperando encaixe).
    pub blocos: usize,
    /// Transações que entraram no mempool.
    pub transacoes: usize,
    /// Já conhecidos (nada a fazer).
    pub repetidos: usize,
    /// Recusados (inválidos, de outra rede, ou de tipo que não viaja em pacote).
    pub recusados: usize,
}

impl Rede {
    /// Monta um pacote com os últimos `blocos` blocos da cadeia, em ordem, e
    /// as transações que esperam no mempool.
    ///
    /// # Errors
    /// Nó travado, ou pacote grande demais.
    pub fn exportar_pacote(&self, blocos: usize) -> Result<Vec<u8>, String> {
        let mensagens: Vec<Message> = {
            let no = self.no.lock().map_err(|_| "nó travado".to_string())?;
            let inicio = no.chain.entries.len().saturating_sub(blocos);
            let mut m: Vec<Message> =
                no.chain.entries.get(inicio..).unwrap_or_default().iter().map(|e| Message::Block(Box::new(e.block.clone()))).collect();
            m.extend(no.mempool_ordenado().into_iter().map(|tx| Message::Tx(Box::new(tx))));
            m
        };
        let mut quadros = Vec::new();
        for m in mensagens.iter().take(MAX_QUADROS_NO_PACOTE) {
            quadros.push(encode_frame(&self.magic, m).map_err(|e| e.to_string())?);
        }
        Pacote { magic: self.magic, quadros }.bytes().map_err(|e| e.to_string())
    }

    /// Confere e aplica um pacote; repassa aos pares o que entrou.
    ///
    /// # Errors
    /// Não é pacote, ou é de outra rede.
    pub fn importar_pacote(self: &Arc<Self>, dados: &[u8]) -> Result<ResultadoDoPacote, String> {
        let pacote = Pacote::ler(dados).map_err(|e| format!("pacote ilegível: {e}"))?;
        if pacote.magic != self.magic {
            return Err("pacote de outra rede".into());
        }
        let mut r = ResultadoDoPacote::default();
        for q in &pacote.quadros {
            let Ok(lido) = decode_frame(&self.magic, q) else {
                r.recusados = r.recusados.saturating_add(1);
                continue;
            };
            if !lido.resto.is_empty() {
                r.recusados = r.recusados.saturating_add(1);
                continue;
            }
            let msg = lido.message;
            let (bloco, pow) = match &msg {
                Message::Block(b) => {
                    let (params, ja_tenho) = match self.no.lock() {
                        Ok(no) => (no.chain.params, no.chain.altura_de(&b.block_hash()).is_some()),
                        Err(_) => return Err("nó travado".into()),
                    };
                    if ja_tenho {
                        r.repetidos = r.repetidos.saturating_add(1);
                        continue;
                    }
                    // o Argon2id fora da trava, como para um par
                    match hyurax_chain::conferir_pow(&params, &b.header) {
                        Ok(recibo) => (true, Some(recibo)),
                        Err(_) => {
                            r.recusados = r.recusados.saturating_add(1);
                            continue;
                        }
                    }
                }
                Message::Tx(_) => (false, None),
                _ => {
                    r.recusados = r.recusados.saturating_add(1);
                    continue;
                }
            };
            let reacao = match self.no.lock() {
                Ok(mut no) => no.tratar_com_pow(msg, pow),
                Err(_) => return Err("nó travado".into()),
            };
            match reacao {
                Ok(reacao) => {
                    let entrou = !reacao.difundir.is_empty() || bloco;
                    for anuncio in &reacao.difundir {
                        self.difundir(u64::MAX, anuncio);
                    }
                    match (entrou, bloco) {
                        (true, true) => r.blocos = r.blocos.saturating_add(1),
                        (true, false) => r.transacoes = r.transacoes.saturating_add(1),
                        (false, _) => r.repetidos = r.repetidos.saturating_add(1),
                    }
                }
                Err(_) => r.recusados = r.recusados.saturating_add(1),
            }
        }
        Ok(r)
    }
}
