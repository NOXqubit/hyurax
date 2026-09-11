# ✝ Provérbios 6:6-8 — “Vai ter com a formiga, ó preguiçoso; olha para os seus caminhos e sê sábio: no verão prepara o seu pão.”
"""AURON — transações.

Corrige o BUG 6 do protótipo: o cabeçalho do bloco não cobria as assinaturas.

  No `auron_reference.py`, a impressão digital das transações incluía
  remetente, destinatário, valor e nonce, mas deixava de fora chave pública e
  assinatura. Trocar a assinatura não mudava o cabeçalho, então o PoW não
  estava amarrado a ela.

  Aqui a folha de Merkle é a codificação COMPLETA da transação, assinatura
  inclusive. Mudou um bit da assinatura, muda a raiz, muda o cabeçalho,
  invalida o PoW.

Corrige também a coinbase-como-string do protótipo.

  Antes, coinbase era uma transação normal com `sender="COINBASE"`, e havia
  uma checagem espalhada para impedir que ela chegasse de fora. Se alguém
  esquecesse a checagem num caminho, virava impressão de dinheiro.

  Aqui coinbase é um TIPO diferente de transação. Ela não tem campo de
  assinatura nem de remetente para preencher. Não existe como forjar uma
  coinbase "de fora" porque a estrutura simplesmente não tem esses campos.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from . import codec, crypto
from .codec import CodecError
from .units import MAX_AMOUNT

TX_VERSION = 1

KIND_COINBASE = 0
KIND_TRANSFER = 1

# Códigos binários dos algoritmos de assinatura (AACL).
SIG_CODE_ED25519 = 1
SIG_CODES = {SIG_CODE_ED25519: crypto.SIG_ED25519_V1}
SIG_CODES_REV = {v: k for k, v in SIG_CODES.items()}

MAX_EXTRA_NONCE = 64


class TxError(Exception):
    """Transação malformada ou inválida."""


@dataclass(frozen=True)
class Coinbase:
    """Recompensa do bloco. Criada pela cadeia, nunca aceita de fora."""

    height: int
    recipient: bytes
    amount: int
    extra_nonce: bytes = b""

    def encode(self) -> bytes:
        if len(self.recipient) != crypto.ADDRESS_LEN:
            raise TxError("endereço de destino inválido")
        if len(self.extra_nonce) > MAX_EXTRA_NONCE:
            raise TxError("extra_nonce longo demais")
        return (
            codec.enc_u8(KIND_COINBASE)
            + codec.enc_u16(TX_VERSION)
            + codec.enc_u64(self.height)
            + codec.enc_fixed(self.recipient, crypto.ADDRESS_LEN)
            + codec.enc_u64(self.amount)
            + codec.enc_bytes(self.extra_nonce)
        )

    @staticmethod
    def decode_body(r: codec.Reader) -> "Coinbase":
        version = r.u16()
        if version != TX_VERSION:
            raise TxError(f"versão de transação desconhecida: {version}")
        return Coinbase(
            height=r.u64(),
            recipient=r.fixed(crypto.ADDRESS_LEN),
            amount=r.u64(),
            extra_nonce=r.var_bytes(),
        )

    def txid(self) -> bytes:
        return crypto.H(self.encode())

    @property
    def fee(self) -> int:
        return 0


@dataclass(frozen=True)
class Transfer:
    """Transferência assinada entre contas."""

    sender: bytes
    recipient: bytes
    amount: int
    fee: int
    nonce: int
    public_key: bytes
    signature: bytes = b""
    sig_code: int = SIG_CODE_ED25519

    # -- codificação --

    def _body(self) -> bytes:
        """Tudo menos a assinatura. É isto que é assinado."""
        if len(self.sender) != crypto.ADDRESS_LEN:
            raise TxError("endereço de origem inválido")
        if len(self.recipient) != crypto.ADDRESS_LEN:
            raise TxError("endereço de destino inválido")
        if self.sig_code not in SIG_CODES:
            raise TxError(f"algoritmo de assinatura desconhecido: {self.sig_code}")
        return (
            codec.enc_u8(KIND_TRANSFER)
            + codec.enc_u16(TX_VERSION)
            + codec.enc_u8(self.sig_code)
            + codec.enc_fixed(self.sender, crypto.ADDRESS_LEN)
            + codec.enc_fixed(self.recipient, crypto.ADDRESS_LEN)
            + codec.enc_u64(self.amount)
            + codec.enc_u64(self.fee)
            + codec.enc_u64(self.nonce)
            + codec.enc_bytes(self.public_key)
        )

    def signing_payload(self, network_magic: bytes) -> bytes:
        """Mensagem assinada, com separação de domínio por rede.

        O magic da rede entra de propósito: sem ele, uma transação assinada na
        testnet é válida na mainnet. Foi um erro real em várias cadeias.
        """
        return b"AURON-TX-v1" + network_magic + self._body()

    def encode(self) -> bytes:
        return self._body() + codec.enc_bytes(self.signature)

    @staticmethod
    def decode_body(r: codec.Reader) -> "Transfer":
        version = r.u16()
        if version != TX_VERSION:
            raise TxError(f"versão de transação desconhecida: {version}")
        sig_code = r.u8()
        if sig_code not in SIG_CODES:
            raise TxError(f"algoritmo de assinatura desconhecido: {sig_code}")
        return Transfer(
            sender=r.fixed(crypto.ADDRESS_LEN),
            recipient=r.fixed(crypto.ADDRESS_LEN),
            amount=r.u64(),
            fee=r.u64(),
            nonce=r.u64(),
            public_key=r.var_bytes(),
            signature=r.var_bytes(),
            sig_code=sig_code,
        )

    def txid(self) -> bytes:
        return crypto.H(self.encode())

    # -- validação criptográfica (regras de saldo ficam no estado) --

    def check_signature(self, network_magic: bytes) -> tuple[bool, str]:
        algo = SIG_CODES[self.sig_code]

        if self.amount <= 0:
            return False, "valor deve ser positivo"
        if self.amount > MAX_AMOUNT or self.fee > MAX_AMOUNT:
            return False, "valor fora da faixa"
        if self.amount + self.fee > MAX_AMOUNT:
            return False, "valor mais taxa estoura a faixa"
        if len(self.public_key) != crypto.pubkey_size(algo):
            return False, "tamanho de chave pública inválido"
        if len(self.signature) != crypto.signature_size(algo):
            return False, "tamanho de assinatura inválido"
        if crypto.address_from_pubkey(self.public_key, algo) != self.sender:
            return False, "chave pública não corresponde ao remetente"
        if self.sender == self.recipient:
            return False, "origem e destino iguais"
        if not crypto.verify(
            self.public_key, self.signing_payload(network_magic), self.signature, algo
        ):
            return False, "assinatura inválida"
        return True, "ok"


def encode_tx(tx) -> bytes:
    return tx.encode()


def decode_tx(data: bytes):
    r = codec.Reader(data)
    tx = decode_tx_from(r)
    r.finish()
    return tx


def decode_tx_from(r: codec.Reader):
    kind = r.u8()
    if kind == KIND_COINBASE:
        return Coinbase.decode_body(r)
    if kind == KIND_TRANSFER:
        return Transfer.decode_body(r)
    raise TxError(f"tipo de transação desconhecido: {kind}")


def sign_transfer(secret: bytes, network_magic: bytes, *, sender: bytes,
                  recipient: bytes, amount: int, fee: int, nonce: int) -> Transfer:
    """Monta e assina uma transferência."""
    pub = crypto.public_key(secret)
    if crypto.address_from_pubkey(pub) != sender:
        raise TxError("segredo não corresponde ao endereço de origem")
    draft = Transfer(
        sender=sender,
        recipient=recipient,
        amount=amount,
        fee=fee,
        nonce=nonce,
        public_key=pub,
    )
    sig = crypto.sign(secret, draft.signing_payload(network_magic))
    return Transfer(
        sender=sender,
        recipient=recipient,
        amount=amount,
        fee=fee,
        nonce=nonce,
        public_key=pub,
        signature=sig,
    )
