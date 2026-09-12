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

Transferência versão 2: vários ativos e várias saídas.

  A arquitetura do projeto (docs/AURON-DIRECT-RESONANCE.md, §19) exige que a
  transação nasça preparada para mais de um ativo. Cada saída diz quanto de
  qual ativo vai para quem, e uma transferência tem de 1 a 16 saídas. Hoje o
  consenso aceita só o AUR; os outros ativos esperam uma regra de emissão.
  A taxa é sempre em AUR, e o nonce continua sendo por conta.
"""

from __future__ import annotations

from dataclasses import dataclass, replace

from . import codec, crypto
from .units import MAX_AMOUNT

# A coinbase não mudou de formato. A transferência mudou, e por isso tem
# versão própria.
TX_VERSION = 1
TRANSFER_VERSION = 2

KIND_COINBASE = 0
KIND_TRANSFER = 1

# Códigos binários dos algoritmos de assinatura (AACL).
SIG_CODE_ED25519 = 1
SIG_CODES = {SIG_CODE_ED25519: crypto.SIG_ED25519_V1}
SIG_CODES_REV = {v: k for k, v in SIG_CODES.items()}

MAX_EXTRA_NONCE = 64

ASSET_ID_LEN = 32
# O AUR, ativo nativo, é o identificador todo zero. Ativos futuros vão ter um
# identificador derivado do hash da própria emissão, que nunca dá zero.
AUR = bytes(ASSET_ID_LEN)
# Ativos que o consenso aceita. Só o AUR, até existir regra de emissão.
KNOWN_ASSETS = frozenset({AUR})

MAX_OUTPUTS = 16

# O domínio muda junto com o formato: uma assinatura feita para a versão 1
# nunca pode ser lida como assinatura de uma transferência versão 2.
SIGNING_DOMAIN = b"AURON-TX-v2"


class TxError(Exception):
    """Transação malformada ou inválida."""


@dataclass(frozen=True)
class Coinbase:
    """Recompensa do bloco. Criada pela cadeia, nunca aceita de fora.

    Paga sempre em AUR: é o único ativo que tem emissão.
    """

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
        height = r.u64()
        recipient = r.fixed(crypto.ADDRESS_LEN)
        amount = r.u64()
        extra_nonce = r.var_bytes()
        # A leitura precisa recusar exatamente o que a escrita recusa. Sem isto,
        # uma coinbase com extra_nonce de 65 bytes era decodificada, e só
        # estourava TxError mais tarde, ao recodificar para o txid — fora de
        # qualquer caminho preparado para tratar o erro.
        if len(extra_nonce) > MAX_EXTRA_NONCE:
            raise TxError(
                f"extra_nonce tem {len(extra_nonce)} bytes, máximo é {MAX_EXTRA_NONCE}"
            )
        return Coinbase(
            height=height,
            recipient=recipient,
            amount=amount,
            extra_nonce=extra_nonce,
        )

    def txid(self) -> bytes:
        return crypto.H(self.encode())

    @property
    def fee(self) -> int:
        return 0


@dataclass(frozen=True)
class Output:
    """Uma saída da transferência: quanto de qual ativo vai para quem."""

    recipient: bytes
    asset_id: bytes
    amount: int

    def encode(self) -> bytes:
        if len(self.recipient) != crypto.ADDRESS_LEN:
            raise TxError("endereço de destino inválido")
        if len(self.asset_id) != ASSET_ID_LEN:
            raise TxError("identificador de ativo inválido")
        return (
            codec.enc_fixed(self.recipient, crypto.ADDRESS_LEN)
            + codec.enc_fixed(self.asset_id, ASSET_ID_LEN)
            + codec.enc_u64(self.amount)
        )

    @staticmethod
    def decode(r: codec.Reader) -> "Output":
        return Output(
            recipient=r.fixed(crypto.ADDRESS_LEN),
            asset_id=r.fixed(ASSET_ID_LEN),
            amount=r.u64(),
        )


def _check_output_count(count: int) -> None:
    if not 1 <= count <= MAX_OUTPUTS:
        raise TxError(f"transferência precisa de 1 a {MAX_OUTPUTS} saídas, tem {count}")


@dataclass(frozen=True)
class Transfer:
    """Transferência assinada de uma conta, com uma ou mais saídas."""

    sender: bytes
    outputs: tuple
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
        if self.sig_code not in SIG_CODES:
            raise TxError(f"algoritmo de assinatura desconhecido: {self.sig_code}")
        _check_output_count(len(self.outputs))
        return (
            codec.enc_u8(KIND_TRANSFER)
            + codec.enc_u16(TRANSFER_VERSION)
            + codec.enc_u8(self.sig_code)
            + codec.enc_fixed(self.sender, crypto.ADDRESS_LEN)
            + codec.enc_u64(self.fee)
            + codec.enc_u64(self.nonce)
            + codec.enc_list(list(self.outputs), Output.encode)
            + codec.enc_bytes(self.public_key)
        )

    def signing_payload(self, network_magic: bytes) -> bytes:
        """Mensagem assinada, com separação de domínio por versão e por rede.

        O magic da rede entra de propósito: sem ele, uma transação assinada na
        testnet é válida na mainnet. Foi um erro real em várias cadeias.
        """
        return SIGNING_DOMAIN + network_magic + self._body()

    def encode(self) -> bytes:
        return self._body() + codec.enc_bytes(self.signature)

    @staticmethod
    def decode_body(r: codec.Reader) -> "Transfer":
        version = r.u16()
        if version != TRANSFER_VERSION:
            raise TxError(f"versão de transação desconhecida: {version}")
        sig_code = r.u8()
        if sig_code not in SIG_CODES:
            raise TxError(f"algoritmo de assinatura desconhecido: {sig_code}")
        sender = r.fixed(crypto.ADDRESS_LEN)
        fee = r.u64()
        nonce = r.u64()
        # A contagem é recusada antes de ler qualquer saída: uma contagem
        # absurda não custa leitura nenhuma. Os bytes são os mesmos de
        # codec.enc_list.
        count = r.u32()
        _check_output_count(count)
        outputs = tuple(Output.decode(r) for _ in range(count))
        return Transfer(
            sender=sender,
            outputs=outputs,
            fee=fee,
            nonce=nonce,
            public_key=r.var_bytes(),
            signature=r.var_bytes(),
            sig_code=sig_code,
        )

    def txid(self) -> bytes:
        return crypto.H(self.encode())

    # -- custo --

    def costs(self) -> dict:
        """Quanto sai do remetente em cada ativo: as saídas, e a taxa no AUR.

        Levanta TxError se o total de algum ativo estourar a faixa.
        """
        totals: dict = {}
        if self.fee:
            totals[AUR] = self.fee
        for output in self.outputs:
            total = totals.get(output.asset_id, 0) + output.amount
            if total > MAX_AMOUNT:
                raise TxError("valor mais taxa estoura a faixa")
            totals[output.asset_id] = total
        return totals

    # -- validação criptográfica e estrutural (regras de saldo ficam no estado) --

    def check_signature(self, network_magic: bytes) -> tuple[bool, str]:
        algo = SIG_CODES[self.sig_code]

        count = len(self.outputs)
        if not 1 <= count <= MAX_OUTPUTS:
            return False, f"transferência precisa de 1 a {MAX_OUTPUTS} saídas, tem {count}"
        if not 0 <= self.fee <= MAX_AMOUNT:
            return False, "valor fora da faixa"
        for output in self.outputs:
            if output.amount <= 0:
                return False, "valor deve ser positivo"
            if output.amount > MAX_AMOUNT:
                return False, "valor fora da faixa"
            if output.asset_id not in KNOWN_ASSETS:
                return False, f"ativo desconhecido: {output.asset_id.hex()}"
            if output.recipient == self.sender:
                return False, "origem e destino iguais"
        # Ordem estrita: sem saída repetida, e um único jeito de escrever o
        # mesmo pagamento, como pede a regra de unicidade da codificação.
        keys = [(o.recipient, o.asset_id) for o in self.outputs]
        if any(a >= b for a, b in zip(keys, keys[1:])):
            return False, "saídas fora de ordem ou repetidas"
        try:
            self.costs()
        except TxError as exc:
            return False, str(exc)
        if len(self.public_key) != crypto.pubkey_size(algo):
            return False, "tamanho de chave pública inválido"
        if len(self.signature) != crypto.signature_size(algo):
            return False, "tamanho de assinatura inválido"
        if crypto.address_from_pubkey(self.public_key, algo) != self.sender:
            return False, "chave pública não corresponde ao remetente"
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


def sign_transfer_outputs(secret: bytes, network_magic: bytes, *, sender: bytes,
                          outputs, fee: int, nonce: int) -> Transfer:
    """Monta e assina uma transferência com as saídas dadas, na ordem dada.

    Não reordena nada: saídas fora de ordem geram uma transferência que o
    consenso recusa, e é assim que os testes provam a regra.
    """
    pub = crypto.public_key(secret)
    if crypto.address_from_pubkey(pub) != sender:
        raise TxError("segredo não corresponde ao endereço de origem")
    draft = Transfer(
        sender=sender,
        outputs=tuple(outputs),
        fee=fee,
        nonce=nonce,
        public_key=pub,
    )
    return replace(draft, signature=crypto.sign(secret, draft.signing_payload(network_magic)))


def sign_transfer(secret: bytes, network_magic: bytes, *, sender: bytes,
                  recipient: bytes, amount: int, fee: int, nonce: int,
                  asset_id: bytes = AUR) -> Transfer:
    """Atalho para a transferência mais comum: uma saída só."""
    return sign_transfer_outputs(
        secret, network_magic, sender=sender,
        outputs=(Output(recipient=recipient, asset_id=asset_id, amount=amount),),
        fee=fee, nonce=nonce,
    )
