// ✝ Lucas 21:28 — “Olhai para cima e levantai a vossa cabeça, porque a vossa redenção está próxima.”
//! Auron — a cadeia (seções 14, 15 e 16 da AURON-SPEC-01).
//!
//! Tradução de `reference/auron/chain.py`, conferida contra `vectors/genesis.json`,
//! `vectors/chain.json` (os mesmos blocos minerados, byte a byte) e
//! `vectors/chain_edge.json` (cada recusa, com a mensagem exata).
//!
//! Existe **um** caminho de entrada, [`Chain::accept_block`], e ele valida tudo
//! antes de tocar no estado. A validação vai do mais barato para o mais caro:
//! cabeçalho, transações, trabalho útil, e só no fim o Argon2id. Um bloco
//! malformado é recusado sem gastar 32 MiB de memória.

#![forbid(unsafe_code)]

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use auron_block::{BLOCK_VERSION, Block, BlockHeader};
use auron_codec::merkle_root;
use auron_consensus::{
    Alvo, ParametrosRede, U512, alvo_de_bits, bits_de_alvo, block_reward, median_time_past,
    next_target, target_to_work,
};
use auron_crypto::{ADDRESS_LEN, HASH_LEN};
use auron_pow::{Calculadora, ConfigMineracao, bate_alvo, minerar};
use auron_state::{State, Undo};
use auron_tx::{Coinbase, Endereco, Transfer, Tx};

/// `prev_hash` da gênese.
pub const GENESIS_PREV_HASH: [u8; HASH_LEN] = [0; HASH_LEN];
/// Horário da gênese: 2026-09-09T00:00:00Z, congelado.
pub const GENESIS_TIMESTAMP: u64 = 1_788_912_000;

/// Bloco recusado. O texto é o do gabarito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainError(pub String);

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ChainError {}

fn erro(m: impl Into<String>) -> ChainError {
    ChainError(m.into())
}

fn texto<E: fmt::Display>(e: E) -> ChainError {
    erro(e.to_string())
}

/// Gênese determinística, uma só por rede.
pub fn make_genesis(p: &ParametrosRede) -> Result<Block, ChainError> {
    let mut extra = b"AURON GENESIS ".to_vec();
    extra.extend_from_slice(p.nome.as_bytes());
    let coinbase = Coinbase { height: 0, recipient: [0; ADDRESS_LEN], amount: 1, extra_nonce: extra };
    let merkle = merkle_root(&[coinbase.encode().map_err(texto)?]);
    let prova = auron_usefulpow::solve(&p.uteis, 0, &GENESIS_PREV_HASH, &coinbase.recipient, &p.max_target)
        .map_err(texto)?;
    let header = BlockHeader {
        version: BLOCK_VERSION,
        height: 0,
        prev_hash: GENESIS_PREV_HASH,
        merkle_root: merkle,
        useful_root: prova.commitment().map_err(texto)?,
        timestamp: GENESIS_TIMESTAMP,
        bits: bits_de_alvo(&p.max_target),
        nonce: 0,
    };
    Ok(Block { header, transactions: vec![Tx::Coinbase(coinbase)], useful_proof: Some(prova) })
}

/// Um bloco ativo, com o trabalho acumulado até ele e o registro de desfazer.
#[derive(Clone, Debug)]
pub struct ChainEntry {
    /// O bloco.
    pub block: Block,
    /// Trabalho acumulado da gênese até aqui.
    pub total_work: U512,
    /// Para desfazer este bloco.
    pub undo: Undo,
}

/// Cadeia com um ramo ativo.
#[derive(Clone, Debug)]
pub struct Chain {
    /// Parâmetros da rede.
    pub params: ParametrosRede,
    /// Blocos ativos, da gênese à ponta.
    pub entries: Vec<ChainEntry>,
    /// Estado depois da ponta.
    pub state: State,
}

fn alvo_do(header: &BlockHeader) -> Result<Alvo, ChainError> {
    alvo_de_bits(header.bits).map_err(texto)
}

fn agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Chain {
    /// Cadeia nova, com a gênese da rede já aplicada.
    pub fn nova(params: ParametrosRede) -> Result<Self, ChainError> {
        let genesis = make_genesis(&params)?;
        let mut state = State::novo(params);
        let undo = state
            .apply_block(0, &genesis.transactions, &params.magic)
            .map_err(texto)?;
        let total_work = target_to_work(&alvo_do(&genesis.header)?).map_err(texto)?;
        Ok(Self { params, entries: vec![ChainEntry { block: genesis, total_work, undo }], state })
    }

    fn ponta(&self) -> Result<&ChainEntry, ChainError> {
        self.entries.last().ok_or_else(|| erro("cadeia sem gênese"))
    }

    /// Altura da ponta.
    pub fn height(&self) -> u64 {
        self.entries.last().map_or(0, |e| e.block.header.height)
    }

    /// O bloco da ponta.
    pub fn tip(&self) -> Option<&Block> {
        self.entries.last().map(|e| &e.block)
    }

    /// `block_hash` da ponta.
    pub fn tip_hash(&self) -> [u8; HASH_LEN] {
        self.entries.last().map_or(GENESIS_PREV_HASH, |e| e.block.block_hash())
    }

    /// Trabalho acumulado até a ponta.
    pub fn total_work(&self) -> U512 {
        self.entries.last().map_or(U512::ZERO, |e| e.total_work)
    }

    fn recentes(&self, quantos: usize) -> &[ChainEntry] {
        let inicio = self.entries.len().saturating_sub(quantos);
        self.entries.get(inicio..).unwrap_or(&[])
    }

    /// `bits` que o PRÓXIMO bloco precisa declarar.
    pub fn expected_bits(&self) -> Result<u32, ChainError> {
        let janela = self.recentes(self.params.lwma_window.saturating_add(1));
        let ts: Vec<u64> = janela.iter().map(|e| e.block.header.timestamp).collect();
        let alvos = janela.iter().map(|e| alvo_do(&e.block.header)).collect::<Result<Vec<_>, _>>()?;
        Ok(bits_de_alvo(&next_target(&ts, &alvos, &self.params).map_err(texto)?))
    }

    /// Mediana dos últimos horários.
    pub fn median_time_past(&self) -> u64 {
        let ts: Vec<u64> = self
            .recentes(self.params.median_time_span)
            .iter()
            .map(|e| e.block.header.timestamp)
            .collect();
        median_time_past(&ts, &self.params)
    }

    // -- validação --

    fn check_header_cheap(&self, header: &BlockHeader, now: u64) -> Result<(), ChainError> {
        if header.version != BLOCK_VERSION {
            return Err(erro(format!("versão de bloco desconhecida: {}", header.version)));
        }
        if Some(header.height) != self.height().checked_add(1) {
            return Err(erro(format!(
                "altura {} não estende a ponta {}",
                header.height,
                self.height()
            )));
        }
        if header.prev_hash != self.tip_hash() {
            return Err(erro("prev_hash não aponta para a ponta atual"));
        }
        let esperado = self.expected_bits()?;
        if header.bits != esperado {
            return Err(erro(format!(
                "dificuldade declarada {:#010x} difere da esperada {esperado:#010x}",
                header.bits
            )));
        }
        alvo_de_bits(header.bits).map_err(|e| erro(format!("alvo inválido: {e}")))?;
        let mtp = self.median_time_past();
        if header.timestamp <= mtp {
            return Err(erro(format!(
                "timestamp {} não passa do median-time-past {mtp}",
                header.timestamp
            )));
        }
        if header.timestamp > now.saturating_add(self.params.max_future_drift) {
            return Err(erro(format!(
                "timestamp {} está no futuro além do tolerado",
                header.timestamp
            )));
        }
        Ok(())
    }

    fn check_header_pow(&self, header: &BlockHeader) -> Result<(), ChainError> {
        let alvo = alvo_do(header)?;
        let hash = Calculadora::nova(self.params.pow)
            .and_then(|mut c| c.pow_hash(&header.encode()))
            .map_err(texto)?;
        if !bate_alvo(&hash, &alvo) {
            return Err(erro("prova de trabalho não bate o alvo"));
        }
        Ok(())
    }

    fn validate_transactions(&self, block: &Block) -> Result<(), ChainError> {
        let bruto = block
            .encode()
            .map_err(|e| erro(format!("transação inválida no bloco: {e}")))?;
        if bruto.len() > self.params.max_block_bytes {
            return Err(erro(format!(
                "bloco tem {} bytes, máximo é {}",
                bruto.len(),
                self.params.max_block_bytes
            )));
        }
        match block.transactions.split_first() {
            None => return Err(erro("bloco sem transações")),
            Some((Tx::Transfer(_), _)) => {
                return Err(erro("a primeira transação precisa ser a coinbase"));
            }
            Some((Tx::Coinbase(_), resto)) => {
                if resto.iter().any(|t| matches!(t, Tx::Coinbase(_))) {
                    return Err(erro("coinbase extra rejeitada"));
                }
            }
        }
        let raiz = block
            .computed_merkle_root()
            .map_err(|e| erro(format!("transação inválida no bloco: {e}")))?;
        if raiz != block.header.merkle_root {
            return Err(erro("merkle_root não corresponde às transações"));
        }
        let recodificado = Block::decode(&bruto)
            .and_then(|b| b.encode())
            .map_err(|e| erro(format!("transação inválida no bloco: {e}")))?;
        if recodificado != bruto {
            return Err(erro("codificação do bloco não é canônica"));
        }
        Ok(())
    }

    fn check_useful_work(&self, block: &Block) -> Result<(), ChainError> {
        let Some(prova) = &block.useful_proof else {
            return Err(erro("bloco sem prova de trabalho útil"));
        };
        let compromisso = prova
            .commitment()
            .map_err(|e| erro(format!("prova de trabalho útil inválida: {e}")))?;
        if compromisso != block.header.useful_root {
            return Err(erro("useful_root do cabeçalho não corresponde à prova"));
        }
        let invalida = |m: String| erro(format!("prova de trabalho útil inválida: {m}"));
        let alvo = alvo_de_bits(block.header.bits).map_err(|e| invalida(e.to_string()))?;
        let coinbase = block.coinbase().map_err(|e| invalida(e.to_string()))?;
        auron_usefulpow::verify(
            prova,
            &self.params.uteis,
            block.header.height,
            &block.header.prev_hash,
            &coinbase.recipient,
            &alvo,
        )
        .map_err(invalida)
    }

    /// Valida um bloco inteiro, do mais barato para o mais caro.
    pub fn validate_block(&self, block: &Block, now: Option<u64>) -> Result<(), ChainError> {
        self.check_header_cheap(&block.header, now.unwrap_or_else(agora))?;
        self.validate_transactions(block)?;
        self.check_useful_work(block)?;
        self.check_header_pow(&block.header)
    }

    /// Único caminho para um bloco entrar na cadeia.
    pub fn accept_block(&mut self, block: Block, now: Option<u64>) -> Result<(), ChainError> {
        self.validate_block(&block, now)?;
        let undo = self
            .state
            .apply_block(block.header.height, &block.transactions, &self.params.magic)
            .map_err(|e| erro(format!("estado rejeitou o bloco: {e}")))?;
        if let Err(e) = self.state.check_invariants() {
            self.state.revert_block(&undo);
            return Err(erro(format!("invariante quebrada: {e}")));
        }
        let trabalho = target_to_work(&alvo_do(&block.header)?).map_err(texto)?;
        let total_work = self
            .ponta()?
            .total_work
            .checked_add(&trabalho)
            .ok_or_else(|| erro("trabalho acumulado estourou"))?;
        self.entries.push(ChainEntry { block, total_work, undo });
        Ok(())
    }

    /// Remove os últimos `count` blocos, desfazendo o estado.
    pub fn rollback(&mut self, count: usize) -> Result<Vec<Block>, ChainError> {
        if count >= self.entries.len() {
            return Err(erro("rollback inválido; a gênese não sai"));
        }
        let mut removidos = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(entrada) = self.entries.pop() {
                self.state.revert_block(&entrada.undo);
                removidos.push(entrada.block);
            }
        }
        Ok(removidos)
    }

    // -- mineração --

    /// Monta o próximo bloco, com a prova útil pronta e nonce zero.
    pub fn build_candidate(
        &self,
        miner: Endereco,
        transfers: Vec<Transfer>,
        timestamp: Option<u64>,
        extra_nonce: Vec<u8>,
    ) -> Result<Block, ChainError> {
        let altura = self.height().checked_add(1).ok_or_else(|| erro("altura estourou"))?;
        let taxas = transfers.iter().try_fold(0u64, |acc, t| acc.checked_add(t.fee));
        let taxas = taxas.ok_or_else(|| erro("taxas estouraram"))?;
        let amount = block_reward(altura, &self.params)
            .checked_add(taxas)
            .ok_or_else(|| erro("recompensa estourou"))?;
        let coinbase = Coinbase { height: altura, recipient: miner, amount, extra_nonce };
        let mut txs = vec![Tx::Coinbase(coinbase)];
        txs.extend(transfers.into_iter().map(Tx::Transfer));

        let timestamp = timestamp
            .unwrap_or_else(|| agora().max(self.median_time_past().saturating_add(1)));
        let bits = self.expected_bits()?;
        let alvo = alvo_de_bits(bits).map_err(texto)?;
        let prova = auron_usefulpow::solve(&self.params.uteis, altura, &self.tip_hash(), &miner, &alvo)
            .map_err(texto)?;
        let folhas = txs.iter().map(Tx::encode).collect::<Result<Vec<_>, _>>().map_err(texto)?;
        let header = BlockHeader {
            version: BLOCK_VERSION,
            height: altura,
            prev_hash: self.tip_hash(),
            merkle_root: merkle_root(&folhas),
            useful_root: prova.commitment().map_err(texto)?,
            timestamp,
            bits,
            nonce: 0,
        };
        Ok(Block { header, transactions: txs, useful_proof: Some(prova) })
    }

    /// Monta e minera o próximo bloco. Com `linhas = 1` acha o mesmo nonce do
    /// gabarito (busca sequencial a partir de zero).
    pub fn mine(
        &self,
        miner: Endereco,
        transfers: Vec<Transfer>,
        timestamp: Option<u64>,
        linhas: u32,
    ) -> Result<Block, ChainError> {
        let candidato = self.build_candidate(miner, transfers, timestamp, Vec::new())?;
        let config = ConfigMineracao {
            linhas,
            nonce_inicial: 0,
            limite: Some(1u64 << 32),
            pausa: Duration::ZERO,
        };
        let resultado = minerar(
            &candidato.header.encode(),
            self.params.pow,
            config,
            &AtomicBool::new(false),
            &AtomicU64::new(0),
        )
        .map_err(texto)?;
        let achado = resultado
            .achado
            .ok_or_else(|| erro(format!("nenhum nonce válido em {} tentativas", 1u64 << 32)))?;
        Ok(Block { header: candidato.header.with_nonce(achado.nonce), ..candidato })
    }
}

/// Qual cadeia vence: positivo se `a` ganha. Trabalho acumulado, e empate
/// desempata pelo hash menor.
pub fn compare_chains(a: &Chain, b: &Chain) -> i8 {
    match a.total_work().cmp(&b.total_work()) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => match a.tip_hash().cmp(&b.tip_hash()) {
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Greater => -1,
            std::cmp::Ordering::Equal => 0,
        },
    }
}
