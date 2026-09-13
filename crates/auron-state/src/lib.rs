// ✝ Lucas 16:10 — “Quem é fiel no mínimo também é fiel no muito.”
//! Auron — estado de contas (seção 13 da AURON-SPEC-01).
//!
//! Tradução de `reference/auron/state.py`, conferida contra `vectors/state.json`:
//! uma sequência de blocos aplicados, recusados e desfeitos, com a mensagem de
//! cada recusa e a fotografia do estado depois de cada passo.
//!
//! - Saldo por conta **e por ativo**; nonce por conta.
//! - Coinbase retida por `coinbase_maturity` blocos antes de virar saldo.
//! - Bloco aplicado por inteiro ou nada: qualquer falha devolve o estado como
//!   estava.
//! - `apply_block` devolve o registro para desfazer exatamente aquele bloco.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use auron_consensus::{ParametrosRede, block_reward};
use auron_tx::{ASSET_ID_LEN, AUR, Coinbase, Endereco, Transfer, Tx};
use auron_types::MAX_SUPPLY;

/// Identificador de ativo.
pub type Ativo = [u8; ASSET_ID_LEN];

/// Regra de estado violada. O texto é o do gabarito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateError(pub String);

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StateError {}

fn erro(m: impl Into<String>) -> StateError {
    StateError(m.into())
}

fn soma(a: u64, b: u64) -> Result<u64, StateError> {
    a.checked_add(b).ok_or_else(|| erro(format!("overflow na soma: {a} + {b}")))
}

fn subtracao(a: u64, b: u64) -> Result<u64, StateError> {
    a.checked_sub(b).ok_or_else(|| erro(format!("underflow na subtração: {a} - {b}")))
}

/// Tudo o que é preciso para desfazer exatamente um bloco.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Undo {
    /// Altura do bloco.
    pub height: u64,
    /// Saldo anterior de cada chave tocada (`None` = não existia).
    pub balances: BTreeMap<(Endereco, Ativo), Option<u64>>,
    /// Nonce anterior de cada conta tocada.
    pub nonces: BTreeMap<Endereco, Option<u64>>,
    /// Altura cuja coinbase amadureceu neste bloco.
    pub matured_at: Option<u64>,
    /// As entradas que amadureceram.
    pub matured_entries: Vec<(Endereco, u64)>,
    /// Altura adicionada à fila de maturação.
    pub pending_added: Option<u64>,
    /// Emissão total antes do bloco.
    pub total_emitted_before: u64,
}

/// Estado de contas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    /// Parâmetros da rede.
    pub params: ParametrosRede,
    /// Saldo por `(endereço, ativo)`. Saldo zero não fica guardado.
    pub balances: BTreeMap<(Endereco, Ativo), u64>,
    /// Próximo nonce de cada conta.
    pub nonces: BTreeMap<Endereco, u64>,
    /// Altura → recompensas esperando maturar.
    pub pending_coinbase: BTreeMap<u64, Vec<(Endereco, u64)>>,
    /// Emissão nova acumulada.
    pub total_emitted: u64,
}

impl State {
    /// Estado vazio.
    pub fn novo(params: ParametrosRede) -> Self {
        Self {
            params,
            balances: BTreeMap::new(),
            nonces: BTreeMap::new(),
            pending_coinbase: BTreeMap::new(),
            total_emitted: 0,
        }
    }

    // -- leitura --

    /// Saldo gastável.
    pub fn balance(&self, endereco: &Endereco, ativo: &Ativo) -> u64 {
        self.balances.get(&(*endereco, *ativo)).copied().unwrap_or(0)
    }

    /// Nonce esperado da próxima transferência.
    pub fn next_nonce(&self, endereco: &Endereco) -> u64 {
        self.nonces.get(endereco).copied().unwrap_or(0)
    }

    /// Recompensas ainda não maturadas de uma conta.
    pub fn immature_balance(&self, endereco: &Endereco) -> u128 {
        self.pending_coinbase
            .values()
            .flatten()
            .filter(|(a, _)| a == endereco)
            .map(|(_, v)| u128::from(*v))
            .sum()
    }

    // -- escrita, com registro de desfazer --

    fn set_balance(&mut self, undo: &mut Undo, endereco: Endereco, ativo: Ativo, valor: u64) {
        let chave = (endereco, ativo);
        undo.balances.entry(chave).or_insert_with(|| self.balances.get(&chave).copied());
        if valor == 0 {
            self.balances.remove(&chave);
        } else {
            self.balances.insert(chave, valor);
        }
    }

    fn set_nonce(&mut self, undo: &mut Undo, endereco: Endereco, valor: u64) {
        undo.nonces.entry(endereco).or_insert_with(|| self.nonces.get(&endereco).copied());
        self.nonces.insert(endereco, valor);
    }

    // -- aplicação --

    /// Aplica um bloco inteiro. Ou tudo entra, ou nada entra.
    ///
    /// Ordem: a coinbase antiga amadurece, as transferências rodam, a coinbase
    /// nova entra na fila.
    pub fn apply_block(
        &mut self,
        height: u64,
        transactions: &[Tx],
        network_magic: &[u8; 4],
    ) -> Result<Undo, StateError> {
        let Some((primeira, resto)) = transactions.split_first() else {
            return Err(erro("bloco sem transações; falta a coinbase"));
        };
        let Tx::Coinbase(coinbase) = primeira else {
            return Err(erro("a primeira transação do bloco precisa ser a coinbase"));
        };
        let mut transferencias = Vec::with_capacity(resto.len());
        for tx in resto {
            match tx {
                Tx::Coinbase(_) => return Err(erro("coinbase extra rejeitada")),
                Tx::Transfer(t) => transferencias.push(t),
            }
        }

        let mut undo = Undo { height, total_emitted_before: self.total_emitted, ..Undo::default() };
        // Fotografia inteira: um bloco meio aplicado é pior que um recusado.
        let fotografia = self.clone();
        let resultado = self
            .mature(&mut undo, height)
            .and_then(|()| self.apply_transfers(&mut undo, &transferencias, network_magic))
            .and_then(|taxas| self.apply_coinbase(&mut undo, coinbase, height, taxas));
        match resultado {
            Ok(()) => Ok(undo),
            Err(e) => {
                *self = fotografia;
                Err(e)
            }
        }
    }

    fn mature(&mut self, undo: &mut Undo, height: u64) -> Result<(), StateError> {
        let Some(alvo) = height.checked_sub(self.params.coinbase_maturity) else {
            return Ok(());
        };
        let Some(entradas) = self.pending_coinbase.remove(&alvo) else {
            return Ok(());
        };
        undo.matured_at = Some(alvo);
        undo.matured_entries = entradas.clone();
        for (endereco, valor) in entradas {
            let novo = soma(self.balance(&endereco, &AUR), valor)?;
            self.set_balance(undo, endereco, AUR, novo);
        }
        Ok(())
    }

    fn apply_transfers(
        &mut self,
        undo: &mut Undo,
        transferencias: &[&Transfer],
        network_magic: &[u8; 4],
    ) -> Result<u64, StateError> {
        let mut taxas = 0u64;
        let mut vistas = BTreeSet::new();
        for tx in transferencias {
            let txid = tx.txid().map_err(|e| erro(e.to_string()))?;
            if !vistas.insert(txid) {
                return Err(erro("transação duplicada dentro do mesmo bloco"));
            }
            tx.check_signature(network_magic)
                .map_err(|motivo| erro(format!("transação inválida: {motivo}")))?;

            let esperado = self.next_nonce(&tx.sender);
            if tx.nonce != esperado {
                return Err(erro(format!(
                    "nonce inválido: esperado {esperado}, veio {} (replay?)",
                    tx.nonce
                )));
            }

            // Confere todos os ativos antes de debitar qualquer um.
            let custos = tx.costs().map_err(|e| erro(e.to_string()))?;
            for (ativo, custo) in &custos {
                if self.balance(&tx.sender, ativo) < *custo {
                    return Err(erro("saldo insuficiente"));
                }
            }
            for (ativo, custo) in &custos {
                let novo = subtracao(self.balance(&tx.sender, ativo), *custo)?;
                self.set_balance(undo, tx.sender, *ativo, novo);
            }
            for saida in &tx.outputs {
                let novo = soma(self.balance(&saida.recipient, &saida.asset_id), saida.amount)?;
                self.set_balance(undo, saida.recipient, saida.asset_id, novo);
            }
            self.set_nonce(undo, tx.sender, soma(esperado, 1)?);
            taxas = soma(taxas, tx.fee)?;
        }
        Ok(taxas)
    }

    fn apply_coinbase(
        &mut self,
        undo: &mut Undo,
        coinbase: &Coinbase,
        height: u64,
        taxas: u64,
    ) -> Result<(), StateError> {
        if coinbase.height != height {
            return Err(erro(format!(
                "coinbase declara altura {}, bloco está em {height}",
                coinbase.height
            )));
        }
        // 0 <= amount <= subsídio + taxas. Zero é válido (seção 12).
        let subsidio = block_reward(height, &self.params);
        let permitido = soma(subsidio, taxas)?;
        if coinbase.amount > permitido {
            return Err(erro(format!(
                "coinbase pede {}, máximo é {permitido} (subsídio {subsidio} + taxas {taxas})",
                coinbase.amount
            )));
        }
        let emitido = coinbase.amount.min(subsidio);
        if soma(self.total_emitted, emitido)? > MAX_SUPPLY {
            return Err(erro("emissão passaria do teto de supply"));
        }
        self.total_emitted = soma(self.total_emitted, emitido)?;
        undo.pending_added = Some(height);
        self.pending_coinbase.entry(height).or_default().push((coinbase.recipient, coinbase.amount));
        Ok(())
    }

    // -- desfazer --

    /// Desfaz exatamente um bloco, na ordem inversa.
    pub fn revert_block(&mut self, undo: &Undo) {
        if let Some(altura) = undo.pending_added
            && let Some(entradas) = self.pending_coinbase.get_mut(&altura)
        {
            entradas.pop();
            if entradas.is_empty() {
                self.pending_coinbase.remove(&altura);
            }
        }
        for (chave, anterior) in &undo.balances {
            match anterior {
                None => self.balances.remove(chave),
                Some(v) => self.balances.insert(*chave, *v),
            };
        }
        for (endereco, anterior) in &undo.nonces {
            match anterior {
                None => self.nonces.remove(endereco),
                Some(v) => self.nonces.insert(*endereco, *v),
            };
        }
        if let Some(altura) = undo.matured_at {
            self.pending_coinbase.insert(altura, undo.matured_entries.clone());
        }
        self.total_emitted = undo.total_emitted_before;
    }

    /// Invariantes que precisam valer depois de qualquer bloco.
    pub fn check_invariants(&self) -> Result<(), StateError> {
        for (_, ativo) in self.balances.keys() {
            if *ativo != AUR {
                return Err(erro(format!(
                    "saldo em ativo sem regra de emissão: {}",
                    ativo.iter().map(|b| format!("{b:02x}")).collect::<String>()
                )));
            }
        }
        if self.total_emitted > MAX_SUPPLY {
            return Err(erro("emissão total passou do teto"));
        }
        let livres: u128 = self
            .balances
            .iter()
            .filter(|((_, ativo), _)| *ativo == AUR)
            .map(|(_, v)| u128::from(*v))
            .sum();
        let retidos: u128 = self.pending_coinbase.values().flatten().map(|(_, v)| u128::from(*v)).sum();
        let circulando = livres.saturating_add(retidos);
        if circulando > u128::from(MAX_SUPPLY) {
            return Err(erro(format!(
                "moeda em circulação ({circulando}) passou do teto ({MAX_SUPPLY})"
            )));
        }
        Ok(())
    }
}
