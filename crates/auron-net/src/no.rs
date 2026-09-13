// ✝ Salmos 133:1 — “Oh! quão bom e quão suave é que os irmãos vivam em união!”
//! O estado de um nó: a cadeia, o mempool e o que fazer com cada mensagem.
//!
//! Esta parte não toca em socket. Ela recebe uma [`Message`] já decodificada e
//! devolve uma [`Reacao`]: o que responder ao par, o que difundir para os
//! outros, e se o par se comportou mal. O transporte (servidor.rs) cuida do I/O
//! e da pontuação; aqui mora a decisão.

use std::collections::BTreeMap;

use auron_block::Block;
use auron_chain::{Chain, GENESIS_PREV_HASH};
use auron_tx::{Endereco, Transfer, Tx};
use auron_wire::{Hash, MAX_GET_BLOCKS, MAX_HEADERS, Message};

/// O que o nó decidiu fazer com uma mensagem.
#[derive(Debug, Default)]
pub struct Reacao {
    /// Mensagens de volta para o par que falou.
    pub respostas: Vec<Message>,
    /// Mensagens para difundir aos outros pares (bloco ou tx novos).
    pub difundir: Vec<Message>,
}

/// Por que uma mensagem foi recusada como comportamento ruim do par.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malicia(pub String);

impl std::fmt::Display for Malicia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// O nó: cadeia mais mempool.
pub struct No {
    /// A cadeia validada.
    pub chain: Chain,
    /// Transferências esperando entrar num bloco, por `(remetente, nonce)`.
    mempool: BTreeMap<(Endereco, u64), Transfer>,
}

impl No {
    /// Nó novo em cima de uma cadeia.
    pub fn novo(chain: Chain) -> Self {
        Self { chain, mempool: BTreeMap::new() }
    }

    fn magic(&self) -> [u8; 4] {
        self.chain.params.magic
    }

    /// As transferências do mempool, em ordem de `(remetente, nonce)`, para
    /// montar um bloco.
    pub fn mempool_ordenado(&self) -> Vec<Transfer> {
        self.mempool.values().cloned().collect()
    }

    /// Quantas transações há no mempool.
    pub fn mempool_len(&self) -> usize {
        self.mempool.len()
    }

    /// Tenta pôr uma transferência no mempool.
    ///
    /// - `Ok(true)`: nova e aceita (vale difundir).
    /// - `Ok(false)`: já conhecida, ou o lugar `(remetente, nonce)` já está
    ///   tomado — não é nova, não difunde. É aqui que o gasto duplo no mempool
    ///   para: a segunda transferência com o mesmo `(remetente, nonce)` não
    ///   desaloja a primeira.
    /// - `Err`: inválida de verdade (assinatura, ativo, saldo) — o par que
    ///   mandou se comportou mal.
    pub fn adicionar_tx(&mut self, tx: Transfer) -> Result<bool, Malicia> {
        tx.check_signature(&self.magic()).map_err(Malicia)?;
        let chave = (tx.sender, tx.nonce);

        // Precisa poder entrar sobre o estado confirmado: nonce em sequência e
        // saldo que cobre. Sem isso, o mempool viraria depósito de lixo.
        let esperado = self.chain.state.next_nonce(&tx.sender);
        if tx.nonce < esperado {
            return Ok(false); // já passou; nem é gasto duplo, é obsoleta
        }
        let custos = tx.costs().map_err(|e| Malicia(e.to_string()))?;
        for (ativo, custo) in &custos {
            if self.chain.state.balance(&tx.sender, ativo) < *custo {
                return Err(Malicia("saldo insuficiente para o mempool".into()));
            }
        }

        if self.mempool.contains_key(&chave) {
            // O lugar (remetente, nonce) já está tomado. Seja a mesma
            // transação de novo ou um gasto duplo, não é novidade e não
            // desaloja a que chegou primeiro: devolve "não é nova".
            return Ok(false);
        }
        self.mempool.insert(chave, tx);
        Ok(true)
    }

    /// Aplica um bloco na cadeia. Se avançar a ponta, tira do mempool o que o
    /// bloco gastou. Devolve `true` se a ponta avançou.
    pub fn aceitar_bloco(&mut self, bloco: Block) -> Result<bool, Malicia> {
        let antes = self.chain.tip_hash();
        self.chain.accept_block(bloco, None).map_err(|e| Malicia(e.to_string()))?;
        let avancou = self.chain.tip_hash() != antes;
        if avancou {
            self.limpar_mempool();
        }
        Ok(avancou)
    }

    /// Tira do mempool o que o estado confirmado já tornou obsoleto (nonce
    /// consumido) ou impagável (saldo).
    fn limpar_mempool(&mut self) {
        let state = &self.chain.state;
        self.mempool.retain(|(sender, nonce), tx| {
            if *nonce < state.next_nonce(sender) {
                return false;
            }
            tx.costs().is_ok_and(|custos| {
                custos.iter().all(|(ativo, custo)| state.balance(sender, ativo) >= *custo)
            })
        });
    }

    /// Decide o que fazer com uma mensagem recebida de um par.
    pub fn tratar(&mut self, msg: Message) -> Result<Reacao, Malicia> {
        let mut r = Reacao::default();
        match msg {
            // O aperto de mão é tratado antes, no servidor; se chegar aqui,
            // é repetição — ignora sem punir.
            Message::Hello(_) | Message::HelloAck { .. } => {}

            Message::GetHeaders { inicio, quantidade } => {
                let max = (quantidade as usize).min(MAX_HEADERS as usize);
                r.respostas.push(Message::Headers(self.chain.headers_a_partir_de(&inicio, max)));
            }

            Message::Headers(cabecalhos) => {
                // Peço os blocos dos cabeçalhos que ainda não tenho, em ordem.
                let faltam: Vec<Hash> = cabecalhos
                    .iter()
                    .map(auron_block::BlockHeader::block_hash)
                    .filter(|h| self.chain.altura_de(h).is_none())
                    .take(MAX_GET_BLOCKS as usize)
                    .collect();
                if !faltam.is_empty() {
                    r.respostas.push(Message::GetBlocks(faltam));
                }
            }

            Message::GetBlocks(hashes) => {
                for h in hashes.iter().take(MAX_GET_BLOCKS as usize) {
                    if let Some(bloco) = self.chain.bloco_por_hash(h) {
                        r.respostas.push(Message::Block(Box::new(bloco.clone())));
                    }
                }
            }

            Message::Block(bloco) => {
                let estende = bloco.header.prev_hash == self.chain.tip_hash();
                let ja_tenho = self.chain.altura_de(&bloco.block_hash()).is_some();
                if ja_tenho {
                    // repetição de algo que já validei; sem novidade
                } else if estende {
                    if self.aceitar_bloco(*bloco.clone())? {
                        r.difundir.push(Message::Block(bloco));
                    }
                } else {
                    // Não encadeia na minha ponta: estou atrás ou é bifurcação.
                    // Não é malícia; peço a sequência a partir do que tenho.
                    r.respostas.push(Message::GetHeaders {
                        inicio: self.chain.tip_hash(),
                        quantidade: MAX_HEADERS,
                    });
                }
            }

            Message::Tx(tx) => {
                if self.adicionar_tx(*tx.clone())? {
                    r.difundir.push(Message::Tx(tx));
                }
            }

            Message::GetAddrs => {
                r.respostas.push(Message::Addrs(Vec::new()));
            }
            Message::Addrs(_) => {}
            Message::Ping(n) => r.respostas.push(Message::Pong(n)),
            Message::Pong(_) => {}

            // Tipo que não conheço: a regra da seção 21 manda ignorar, para
            // versões futuras poderem crescer sem quebrar esta.
            Message::Desconhecida { .. } => {}
        }
        Ok(r)
    }

    /// A mensagem `GetHeaders` para começar a sincronizar a partir da própria
    /// ponta. Se a ponta for a gênese, pede tudo do começo.
    pub fn pedir_sincronizacao(&self) -> Message {
        let inicio = if self.chain.height() == 0 { GENESIS_PREV_HASH } else { self.chain.tip_hash() };
        Message::GetHeaders { inicio, quantidade: MAX_HEADERS }
    }
}

// A conversão de Tx para Transfer, para o mempool.
impl No {
    /// Aceita uma transação de qualquer tipo, recusando coinbase (que nunca
    /// viaja solta na rede).
    pub fn adicionar_qualquer_tx(&mut self, tx: Tx) -> Result<bool, Malicia> {
        match tx {
            Tx::Transfer(t) => self.adicionar_tx(t),
            Tx::Coinbase(_) => Err(Malicia("coinbase não entra no mempool".into())),
        }
    }
}
