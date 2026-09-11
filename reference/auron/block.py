# ✝ Jeremias 32:15 — “Ainda se comprarão casas, e campos, e vinhas nesta terra.”
"""AURON — cabeçalho e bloco.

O cabeçalho tem tamanho fixo e cobre tudo que define o bloco. São dois hashes
diferentes sobre ele, de propósito:

  block_hash  = SHA-512(cabeçalho)     -> identidade, ligação entre blocos
  pow_hash    = Argon2id(cabeçalho)    -> prova de trabalho, comparada ao alvo

Separar os dois importa. A identidade precisa ser barata de calcular, porque
todo nó calcula milhões delas ao sincronizar. A prova precisa ser cara, porque
é ela que custa energia. Usar a mesma função para as duas coisas obriga a
escolher entre um índice lento e uma prova barata.

BUG 6 fechado aqui: `merkle_root` é calculada sobre a codificação COMPLETA de
cada transação, assinatura inclusive.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from . import codec, crypto
from .consensus import ChainParams, compact_to_target, pow_hash
from .tx import Coinbase, decode_tx_from, encode_tx

BLOCK_VERSION = 1
HASH_LEN = 64  # SHA-512


@dataclass(frozen=True)
class BlockHeader:
    height: int
    prev_hash: bytes
    merkle_root: bytes
    timestamp: int
    bits: int
    nonce: int
    version: int = BLOCK_VERSION

    def encode(self) -> bytes:
        return (
            codec.enc_u16(self.version)
            + codec.enc_u64(self.height)
            + codec.enc_fixed(self.prev_hash, HASH_LEN)
            + codec.enc_fixed(self.merkle_root, HASH_LEN)
            + codec.enc_u64(self.timestamp)
            + codec.enc_u32(self.bits)
            + codec.enc_u64(self.nonce)
        )

    @staticmethod
    def decode_from(r: codec.Reader) -> "BlockHeader":
        return BlockHeader(
            version=r.u16(),
            height=r.u64(),
            prev_hash=r.fixed(HASH_LEN),
            merkle_root=r.fixed(HASH_LEN),
            timestamp=r.u64(),
            bits=r.u32(),
            nonce=r.u64(),
        )

    @staticmethod
    def decode(data: bytes) -> "BlockHeader":
        r = codec.Reader(data)
        header = BlockHeader.decode_from(r)
        r.finish()
        return header

    def block_hash(self) -> bytes:
        """Identidade do bloco. Barata."""
        return crypto.H(self.encode())

    def pow_hash(self, params: ChainParams) -> bytes:
        """Prova de trabalho. Cara, por definição."""
        return pow_hash(self.encode(), params)

    def target(self) -> int:
        return compact_to_target(self.bits)

    def with_nonce(self, nonce: int) -> "BlockHeader":
        return BlockHeader(
            height=self.height,
            prev_hash=self.prev_hash,
            merkle_root=self.merkle_root,
            timestamp=self.timestamp,
            bits=self.bits,
            nonce=nonce,
            version=self.version,
        )


@dataclass
class Block:
    header: BlockHeader
    transactions: list = field(default_factory=list)

    def encode(self) -> bytes:
        return self.header.encode() + codec.enc_list(self.transactions, encode_tx)

    @staticmethod
    def decode(data: bytes) -> "Block":
        r = codec.Reader(data)
        header = BlockHeader.decode_from(r)
        txs = r.read_list(decode_tx_from)
        r.finish()
        return Block(header=header, transactions=txs)

    def block_hash(self) -> bytes:
        return self.header.block_hash()

    def size(self) -> int:
        return len(self.encode())

    def merkle_leaves(self) -> list[bytes]:
        """Folhas da árvore: a transação inteira, assinatura inclusive."""
        return [encode_tx(tx) for tx in self.transactions]

    def computed_merkle_root(self) -> bytes:
        return codec.merkle_root(self.merkle_leaves())

    def coinbase(self) -> Coinbase:
        if not self.transactions:
            raise ValueError("bloco sem transações")
        first = self.transactions[0]
        if not isinstance(first, Coinbase):
            raise ValueError("a primeira transação não é coinbase")
        return first

    def total_fees(self) -> int:
        return sum(tx.fee for tx in self.transactions[1:])
