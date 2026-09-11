# ✝ Lucas 21:28 — “Quando essas coisas começarem a acontecer, olhai para cima e levantai a vossa cabeça, porque a vossa redenção está próxima.”
"""AURON — a cadeia: gênese única, validação completa, reorg por trabalho.

Corrige dois bugs graves do protótipo.

BUG 5 — o nó aceitava bloco de peer sem verificar prova de trabalho.

  `Network.broadcast_block` conferia altura e hash anterior e chamava
  `_commit` direto. `_commit` só aplicava transações. Qualquer bloco forjado,
  com hash inventado e sem trabalho nenhum, entrava na cadeia dos vizinhos.

  Aqui existe UM caminho de entrada, `accept_block`, e ele valida tudo antes
  de tocar no estado. Não há função que aplique um bloco sem validar.

BUG 7 — duas definições incompatíveis de gênese.

  `auron_core.Blockchain` começava com a lista vazia e o primeiro bloco
  minerado ficava na altura 0. `auron_reference.Blockchain` criava a gênese na
  altura 0 e o primeiro minerado ia para a altura 1. Como o tipo de trabalho
  era escolhido por `height % 3`, as duas cadeias divergiam já no primeiro
  bloco.

  Aqui a gênese é uma só, determinística, derivada dos parâmetros da rede.

Escolha de ponta por TRABALHO ACUMULADO, não por altura. Comparar altura
deixa uma cadeia longa de blocos fáceis ganhar de uma curta de blocos
difíceis, que é exatamente o que um atacante quer.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field

from . import codec, crypto
from .block import Block, BlockHeader
from .consensus import (
    ChainParams,
    ConsensusError,
    check_pow_target,
    compact_to_target,
    median_time_past,
    next_target,
    target_to_compact,
    target_to_work,
)
from .state import State, StateError
from .tx import Coinbase, Transfer

GENESIS_PREV_HASH = b"\x00" * 64


class ChainError(Exception):
    """Bloco rejeitado."""


@dataclass
class ChainEntry:
    block: Block
    total_work: int
    undo: object | None = None


def make_genesis(params: ChainParams) -> Block:
    """Gênese determinística, uma só por rede.

    A recompensa da gênese vai para o endereço nulo e é inalcançável de
    propósito: ninguém começa com dinheiro. Quem quiser AUR minera.
    """
    coinbase = Coinbase(
        height=0,
        recipient=b"\x00" * crypto.ADDRESS_LEN,
        amount=1,  # simbólico; ninguém tem a chave do endereço nulo
        extra_nonce=b"AURON GENESIS " + params.name.encode("ascii"),
    )
    merkle = codec.merkle_root([coinbase.encode()])
    header = BlockHeader(
        height=0,
        prev_hash=GENESIS_PREV_HASH,
        merkle_root=merkle,
        timestamp=1_788_912_000,  # 2026-09-09T00:00:00Z, congelado
        bits=target_to_compact(params.max_target),
        nonce=0,
    )
    return Block(header=header, transactions=[coinbase])


@dataclass
class Chain:
    """Cadeia com um único ramo ativo e histórico de desfazer para reorg."""

    params: ChainParams
    entries: list = field(default_factory=list)          # ChainEntry, ativos
    by_hash: dict = field(default_factory=dict)          # hash -> ChainEntry
    orphans: dict = field(default_factory=dict)          # hash -> Block
    state: State = None  # type: ignore[assignment]

    def __post_init__(self) -> None:
        if self.state is None:
            self.state = State(params=self.params)
        if not self.entries:
            self._install_genesis()

    def _install_genesis(self) -> None:
        genesis = make_genesis(self.params)
        undo = self.state.apply_block(0, genesis.transactions, self.params.magic)
        entry = ChainEntry(
            block=genesis,
            total_work=target_to_work(genesis.header.target()),
            undo=undo,
        )
        self.entries.append(entry)
        self.by_hash[genesis.block_hash()] = entry

    # -- leitura --

    @property
    def height(self) -> int:
        return self.entries[-1].block.header.height

    @property
    def tip(self) -> Block:
        return self.entries[-1].block

    def tip_hash(self) -> bytes:
        return self.tip.block_hash()

    @property
    def total_work(self) -> int:
        return self.entries[-1].total_work

    def block_at(self, height: int) -> Block:
        if not 0 <= height < len(self.entries):
            raise ChainError(f"altura {height} fora da cadeia")
        return self.entries[height].block

    def recent_timestamps(self, count: int) -> list[int]:
        return [e.block.header.timestamp for e in self.entries[-count:]]

    def recent_targets(self, count: int) -> list[int]:
        return [e.block.header.target() for e in self.entries[-count:]]

    def expected_bits(self) -> int:
        """Alvo que o PRÓXIMO bloco precisa declarar."""
        window = self.params.lwma_window + 1
        return target_to_compact(
            next_target(
                self.recent_timestamps(window),
                self.recent_targets(window),
                self.params,
            )
        )

    def median_time_past(self) -> int:
        return median_time_past(
            self.recent_timestamps(self.params.median_time_span), self.params
        )

    # -- validação --

    def validate_header(self, header: BlockHeader, *, now: int | None = None) -> None:
        """Cabeçalho completo, prova de trabalho inclusive.

        Verificar o PoW era exatamente o que o protótipo pulava ao receber
        bloco de peer.
        """
        self._check_header_cheap(header, now=now)
        self._check_header_pow(header)

    def _check_header_cheap(self, header: BlockHeader, *,
                            now: int | None = None) -> None:
        """Tudo do cabeçalho que custa quase nada de conferir."""
        if header.version != 1:
            raise ChainError(f"versão de bloco desconhecida: {header.version}")

        if header.height != self.height + 1:
            raise ChainError(
                f"altura {header.height} não estende a ponta {self.height}"
            )
        if header.prev_hash != self.tip_hash():
            raise ChainError("prev_hash não aponta para a ponta atual")

        if len(header.merkle_root) != 64:
            raise ChainError("merkle_root com tamanho inválido")

        expected = self.expected_bits()
        if header.bits != expected:
            raise ChainError(
                f"dificuldade declarada {header.bits:#010x} difere da esperada "
                f"{expected:#010x}"
            )

        try:
            compact_to_target(header.bits)
        except ConsensusError as exc:
            raise ChainError(f"alvo inválido: {exc}") from exc

        mtp = self.median_time_past()
        if header.timestamp <= mtp:
            raise ChainError(
                f"timestamp {header.timestamp} não passa do median-time-past {mtp}"
            )
        now = int(time.time()) if now is None else now
        if header.timestamp > now + self.params.max_future_drift:
            raise ChainError(
                f"timestamp {header.timestamp} está no futuro além do tolerado"
            )

    def _check_header_pow(self, header: BlockHeader) -> None:
        """A parte cara. Sempre por último.

        Um Argon2id custa dezenas de milissegundos. Gastar isso num bloco que
        já dava para rejeitar por estar malformado é um vetor de negação de
        serviço: o atacante manda lixo barato e o nó paga caro para recusar.
        """
        target = compact_to_target(header.bits)
        if not check_pow_target(header.pow_hash(self.params), target):
            raise ChainError("prova de trabalho não bate o alvo")

    def validate_block(self, block: Block, *, now: int | None = None) -> None:
        """Valida na ordem do mais barato para o mais caro.

        Todas as checagens estruturais acontecem ANTES da prova de trabalho.
        Um bloco malformado é recusado sem gastar um Argon2id.
        """
        self._check_header_cheap(block.header, now=now)

        size = block.size()
        if size > self.params.max_block_bytes:
            raise ChainError(
                f"bloco tem {size} bytes, máximo é {self.params.max_block_bytes}"
            )

        if not block.transactions:
            raise ChainError("bloco sem transações")
        if not isinstance(block.transactions[0], Coinbase):
            raise ChainError("a primeira transação precisa ser a coinbase")
        for tx in block.transactions[1:]:
            if isinstance(tx, Coinbase):
                raise ChainError("coinbase extra rejeitada")
            if not isinstance(tx, Transfer):
                raise ChainError("tipo de transação inesperado")

        if block.computed_merkle_root() != block.header.merkle_root:
            raise ChainError("merkle_root não corresponde às transações")

        # A codificação precisa ser canônica: decodificar e recodificar tem
        # que dar exatamente os mesmos bytes. Sem isso, duas sequências
        # diferentes representariam o mesmo bloco.
        raw = block.encode()
        if Block.decode(raw).encode() != raw:
            raise ChainError("codificação do bloco não é canônica")

        # Só agora, com tudo o mais conferido, vale gastar o Argon2id.
        self._check_header_pow(block.header)

    # -- entrada única --

    def accept_block(self, block: Block, *, now: int | None = None) -> None:
        """Único caminho para um bloco entrar na cadeia. Valida antes de aplicar."""
        self.validate_block(block, now=now)

        height = block.header.height
        try:
            undo = self.state.apply_block(height, block.transactions, self.params.magic)
        except (StateError, ValueError) as exc:
            raise ChainError(f"estado rejeitou o bloco: {exc}") from exc

        try:
            self.state.check_invariants()
        except StateError as exc:
            self.state.revert_block(undo)
            raise ChainError(f"invariante quebrada: {exc}") from exc

        entry = ChainEntry(
            block=block,
            total_work=self.total_work + target_to_work(block.header.target()),
            undo=undo,
        )
        self.entries.append(entry)
        self.by_hash[block.block_hash()] = entry

    def rollback(self, count: int = 1) -> list[Block]:
        """Remove os últimos `count` blocos, desfazendo o estado."""
        if count < 0 or count >= len(self.entries):
            raise ChainError("rollback inválido; a gênese não sai")
        removed = []
        for _ in range(count):
            entry = self.entries.pop()
            self.state.revert_block(entry.undo)
            self.by_hash.pop(entry.block.block_hash(), None)
            removed.append(entry.block)
        return removed

    # -- mineração de laboratório --

    def build_candidate(self, miner: bytes, transfers: list | None = None,
                        *, timestamp: int | None = None,
                        extra_nonce: bytes = b"") -> Block:
        """Monta o próximo bloco, pronto para receber o nonce."""
        from .consensus import block_reward

        transfers = list(transfers or [])
        height = self.height + 1
        fees = sum(tx.fee for tx in transfers)
        coinbase = Coinbase(
            height=height,
            recipient=miner,
            amount=block_reward(height, self.params) + fees,
            extra_nonce=extra_nonce,
        )
        txs = [coinbase, *transfers]

        if timestamp is None:
            timestamp = max(int(time.time()), self.median_time_past() + 1)

        header = BlockHeader(
            height=height,
            prev_hash=self.tip_hash(),
            merkle_root=codec.merkle_root([tx.encode() for tx in txs]),
            timestamp=timestamp,
            bits=self.expected_bits(),
            nonce=0,
        )
        return Block(header=header, transactions=txs)

    def mine(self, miner: bytes, transfers: list | None = None,
             *, max_nonce: int = 1 << 32, timestamp: int | None = None) -> Block:
        """Procura um nonce que bata o alvo. Lento em Python, e tudo bem."""
        candidate = self.build_candidate(
            miner, transfers, timestamp=timestamp
        )
        target = candidate.header.target()
        for nonce in range(max_nonce):
            header = candidate.header.with_nonce(nonce)
            if check_pow_target(header.pow_hash(self.params), target):
                return Block(header=header, transactions=candidate.transactions)
        raise ChainError(f"nenhum nonce válido em {max_nonce} tentativas")


def compare_chains(a: Chain, b: Chain) -> int:
    """Qual cadeia vence. Positivo se `a` ganha.

    Critério é trabalho acumulado. Empate desempata pelo hash menor, que é
    arbitrário mas determinístico: dois nós honestos precisam escolher a mesma
    ponta sem conversar.
    """
    if a.total_work != b.total_work:
        return 1 if a.total_work > b.total_work else -1
    if a.tip_hash() != b.tip_hash():
        return 1 if a.tip_hash() < b.tip_hash() else -1
    return 0
