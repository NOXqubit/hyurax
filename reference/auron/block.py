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

VERSÃO 2 (consenso híbrido): o cabeçalho ganha `useful_root`, o compromisso da
prova de trabalho útil que vem no corpo do bloco. Como o Argon2id é calculado
sobre o cabeçalho inteiro, a prova útil fica presa ao sorteio: trocar a prova
depois de achar o nonce muda o cabeçalho e invalida o Argon2id.

  u16  version = 2
  u64  height
  [64] prev_hash
  [64] merkle_root
  [64] useful_root   = H("AURON-UPOW-PROOF-v1" || prova)
  u64  timestamp
  u32  bits
  u64  nonce
                     total: 222 bytes

Corpo: cabeçalho || prova útil (com prefixo de tamanho) || lista de transações.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from . import codec, crypto
from .consensus import ChainParams, compact_to_target, pow_hash
from .tx import Coinbase, decode_tx_from, encode_tx
from .usefulpow import UsefulWorkProof

BLOCK_VERSION = 2
HASH_LEN = 64  # SHA-512
HEADER_LEN = 222
SEM_PROVA = b"\x00" * HASH_LEN


@dataclass(frozen=True)
class BlockHeader:
    height: int
    prev_hash: bytes
    merkle_root: bytes
    timestamp: int
    bits: int
    nonce: int
    useful_root: bytes = SEM_PROVA
    version: int = BLOCK_VERSION

    def encode(self) -> bytes:
        return (
            codec.enc_u16(self.version)
            + codec.enc_u64(self.height)
            + codec.enc_fixed(self.prev_hash, HASH_LEN)
            + codec.enc_fixed(self.merkle_root, HASH_LEN)
            + codec.enc_fixed(self.useful_root, HASH_LEN)
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
            useful_root=r.fixed(HASH_LEN),
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
            useful_root=self.useful_root,
            version=self.version,
        )


@dataclass
class Block:
    header: BlockHeader
    transactions: list = field(default_factory=list)
    useful_proof: UsefulWorkProof | None = None

    def encode(self) -> bytes:
        # Sem prova, o corpo leva um campo vazio: o bloco continua decodificável
        # e é a validação que o recusa, com o motivo certo.
        prova = self.useful_proof.encode() if self.useful_proof is not None else b""
        return (
            self.header.encode()
            + codec.enc_bytes(prova)
            + codec.enc_list(self.transactions, encode_tx)
        )

    @staticmethod
    def decode(data: bytes) -> "Block":
        r = codec.Reader(data)
        header = BlockHeader.decode_from(r)
        bruto = r.var_bytes()
        prova = UsefulWorkProof.decode(bruto) if bruto else None
        txs = r.read_list(decode_tx_from)
        r.finish()
        return Block(header=header, transactions=txs, useful_proof=prova)

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
