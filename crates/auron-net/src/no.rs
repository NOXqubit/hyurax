// ✝ Salmos 133:1 — “Oh! quão bom e quão suave é que os irmãos vivam em união!”
//! O estado de um nó: a cadeia, o mempool e o que fazer com cada mensagem.
//!
//! Esta parte não toca em socket. Ela recebe uma [`Message`] já decodificada e
//! devolve uma [`Reacao`]: o que responder ao par, o que difundir para os
//! outros, e se o par se comportou mal. O transporte (servidor.rs) cuida do I/O
//! e da pontuação; aqui mora a decisão.

use std::collections::BTreeMap;

use auron_block::Block;
use auron_chain::Chain;
use auron_consensus::{U512, alvo_de_bits, target_to_work};
use auron_tx::{Endereco, Transfer, Tx};
use auron_wire::{Hash, MAX_GET_BLOCKS, MAX_HEADERS, Message};

/// Teto de blocos guardados que ainda não encaixam na cadeia.
///
/// É defesa: um par hostil poderia mandar órfãos sem parar para encher a
/// memória. Ao estourar, o nó **descarta todos** e recomeça a sincronizar —
/// perder um ramo em construção é barato; ficar sem memória, não.
const MAX_ORFAOS: usize = 512;

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

/// O nó: cadeia, mempool e os blocos que ainda não encaixam.
pub struct No {
    /// A cadeia validada.
    pub chain: Chain,
    /// Transferências esperando entrar num bloco, por `(remetente, nonce)`.
    mempool: BTreeMap<(Endereco, u64), Transfer>,
    /// Blocos recebidos que não estendem a ponta, por `block_hash`. Podem ser
    /// de um ramo concorrente que ainda está chegando.
    orfaos: BTreeMap<Hash, Block>,
}

impl No {
    /// Nó novo em cima de uma cadeia.
    pub fn novo(chain: Chain) -> Self {
        Self { chain, mempool: BTreeMap::new(), orfaos: BTreeMap::new() }
    }

    /// Quantos blocos estão guardados esperando encaixe.
    pub fn orfaos_len(&self) -> usize {
        self.orfaos.len()
    }

    fn magic(&self) -> [u8; 4] {
        self.chain.params.magic
    }

    /// As transferências do mempool, em ordem de `(remetente, nonce)`, para
    /// montar um bloco.
    pub fn mempool_ordenado(&self) -> Vec<Transfer> {
        self.mempool.values().cloned().collect()
    }

    /// O próximo nonce livre de uma conta: o da cadeia, ou depois do último que
    /// já espera no mempool.
    pub fn proximo_nonce(&self, endereco: &auron_tx::Endereco) -> u64 {
        let da_cadeia = self.chain.state.next_nonce(endereco);
        self.mempool
            .range((*endereco, 0)..=(*endereco, u64::MAX))
            .next_back()
            .map_or(da_cadeia, |((_, n), _)| da_cadeia.max(n.saturating_add(1)))
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

    /// Guarda um bloco que ainda não encaixa, respeitando o teto.
    fn guardar_orfao(&mut self, bloco: Block) {
        if self.orfaos.len() >= MAX_ORFAOS {
            self.orfaos.clear();
        }
        self.orfaos.insert(bloco.block_hash(), bloco);
    }

    /// Aplica, em sequência, os órfãos que já encaixam na ponta atual.
    fn encaixar_orfaos(&mut self) {
        loop {
            let ponta = self.chain.tip_hash();
            let Some(hash) = self
                .orfaos
                .iter()
                .find(|(_, b)| b.header.prev_hash == ponta)
                .map(|(h, _)| *h)
            else {
                return;
            };
            let Some(bloco) = self.orfaos.remove(&hash) else {
                return;
            };
            if self.chain.accept_block(bloco, None).is_err() {
                return; // não encaixou de verdade; para aqui
            }
            self.limpar_mempool();
        }
    }

    /// O trabalho somado de um ramo de blocos.
    fn trabalho_do_ramo(ramo: &[Block]) -> Option<U512> {
        let mut total = U512::ZERO;
        for bloco in ramo {
            let alvo = alvo_de_bits(bloco.header.bits).ok()?;
            total = total.checked_add(&target_to_work(&alvo).ok()?)?;
        }
        Some(total)
    }

    /// Monta, a partir de um órfão, o ramo que vai de um ancestral da minha
    /// cadeia até ele. Devolve `(altura do ancestral, ramo em ordem)`.
    fn montar_ramo(&self, folha: &Hash) -> Option<(u64, Vec<Block>)> {
        let mut ramo = Vec::new();
        let mut atual = *folha;
        for _ in 0..=MAX_ORFAOS {
            let bloco = self.orfaos.get(&atual)?;
            ramo.push(bloco.clone());
            let prev = bloco.header.prev_hash;
            if let Some(altura) = self.chain.altura_de(&prev) {
                ramo.reverse();
                return Some((altura, ramo));
            }
            if !self.orfaos.contains_key(&prev) {
                return None; // ramo incompleto: faltam blocos no meio
            }
            atual = prev;
        }
        None
    }

    /// Se algum ramo guardado tiver MAIS trabalho que a minha cauda desde o
    /// ancestral comum, troca de cadeia (seção 15 e 21.5).
    ///
    /// Se a cadeia nova falhar no meio da aplicação, o nó volta para a que
    /// tinha: uma reorganização que não completa não pode deixar o nó pior.
    fn tentar_reorganizar(&mut self) -> Result<bool, Malicia> {
        // Escolhe o ramo completo de maior trabalho.
        let folhas: Vec<Hash> = self.orfaos.keys().copied().collect();
        let mut melhor: Option<(u64, Vec<Block>, U512)> = None;
        for folha in folhas {
            let Some((altura_ancestral, ramo)) = self.montar_ramo(&folha) else {
                continue;
            };
            let Some(trabalho) = Self::trabalho_do_ramo(&ramo) else {
                continue;
            };
            if melhor.as_ref().is_none_or(|(_, _, t)| trabalho > *t) {
                melhor = Some((altura_ancestral, ramo, trabalho));
            }
        }
        let Some((altura_ancestral, ramo, trabalho_ramo)) = melhor else {
            return Ok(false);
        };

        // A minha cauda desde o ancestral: o que eu perderia na troca.
        let Some(trabalho_ate_ancestral) = self.chain.trabalho_ate(altura_ancestral) else {
            return Ok(false);
        };
        let minha_cauda = self.chain.total_work();
        // trabalho da cauda = total - até o ancestral. Comparo sem subtrair:
        // ramo vence se (até o ancestral + ramo) > meu total.
        let Some(novo_total) = trabalho_ate_ancestral.checked_add(&trabalho_ramo) else {
            return Ok(false);
        };
        if novo_total <= minha_cauda {
            return Ok(false); // não vale trocar
        }

        // Desfaz até o ancestral, guardando o que sai para poder voltar.
        let quantos = self.chain.height().saturating_sub(altura_ancestral);
        let removidos = self
            .chain
            .rollback(usize::try_from(quantos).unwrap_or(0))
            .map_err(|e| Malicia(e.to_string()))?;

        // Aplica o ramo novo. Se falhar no meio, volta tudo.
        let mut aplicados = 0usize;
        for bloco in &ramo {
            if self.chain.accept_block(bloco.clone(), None).is_err() {
                let _ = self.chain.rollback(aplicados);
                for antigo in removidos.iter().rev() {
                    if self.chain.accept_block(antigo.clone(), None).is_err() {
                        // Não deveria acontecer: eram blocos já validados.
                        return Err(Malicia("falha ao restaurar a cadeia antiga".into()));
                    }
                }
                return Ok(false);
            }
            aplicados = aplicados.saturating_add(1);
        }

        // Deu certo: os blocos aplicados saem dos órfãos, e o que foi desfeito
        // vira órfão (pode voltar a valer se aquele ramo crescer de novo).
        for bloco in &ramo {
            self.orfaos.remove(&bloco.block_hash());
        }
        for antigo in removidos {
            self.guardar_orfao(antigo);
        }
        self.limpar_mempool();
        Ok(true)
    }

    /// Decide o que fazer com uma mensagem recebida de um par.
    pub fn tratar(&mut self, msg: Message) -> Result<Reacao, Malicia> {
        let mut r = Reacao::default();
        match msg {
            // O aperto de mão é tratado antes, no servidor; se chegar aqui,
            // é repetição — ignora sem punir.
            Message::Hello(_) | Message::HelloAck { .. } => {}

            Message::GetHeaders { locator, quantidade } => {
                let max = (quantidade as usize).min(MAX_HEADERS as usize);
                r.respostas.push(Message::Headers(self.chain.headers_do_locator(&locator, max)));
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
                        // Blocos podem chegar fora de ordem: o que estava
                        // guardado e agora encaixa entra na sequência.
                        self.encaixar_orfaos();
                        r.difundir.push(Message::Block(bloco));
                    }
                } else {
                    // Não encadeia na minha ponta: estou atrás, ou é um ramo
                    // concorrente chegando. Guardo e tento trocar de cadeia se
                    // o ramo já tiver mais trabalho que o meu.
                    self.guardar_orfao(*bloco);
                    if self.tentar_reorganizar()? {
                        // Trocou de cadeia: anuncia a ponta nova aos outros.
                        if let Some(ponta) = self.chain.tip().cloned() {
                            r.difundir.push(Message::Block(Box::new(ponta)));
                        }
                    } else {
                        // Ainda falta blocos no meio: peço a sequência com o
                        // locator, que acha o ancestral comum.
                        r.respostas.push(self.pedir_sincronizacao());
                    }
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

    /// A mensagem `GetHeaders` para sincronizar: manda o locator da própria
    /// cadeia, para o par achar o ancestral comum mesmo se houve bifurcação.
    pub fn pedir_sincronizacao(&self) -> Message {
        Message::GetHeaders { locator: self.chain.locator(), quantidade: MAX_HEADERS }
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
