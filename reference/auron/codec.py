# ✝ Isaías 26:20 — “Vai, pois, povo meu, entra nos teus quartos e fecha as tuas portas sobre ti; esconde-te só por um momento, até que passe a ira.”
"""AURON — codificação canônica binária e árvore de Merkle.

O protótipo assinava JSON (`json.dumps` com sort_keys). Isso foi descartado.

Por que JSON não serve para consenso:

  - Escapamento de Unicode, formatação de número e ordenação de chave variam
    entre linguagens. Reproduzir o JSON do Python exatamente em Rust é uma
    fonte permanente de divergência.
  - Inteiro grande em JSON não tem tamanho definido.
  - Toda a segurança passa a depender de duas bibliotecas de serialização
    concordarem, em vez de depender de uma especificação.

A codificação aqui é binária, posicional e de tamanho fixo. Cabe em meia
página de especificação e é trivial de reproduzir em qualquer linguagem.

Regras:

  - Inteiros são big-endian, largura fixa e explícita.
  - Bytes variáveis levam prefixo de tamanho u32 big-endian.
  - Struct é a concatenação dos campos, na ordem declarada. Sem nomes de
    campo, sem separadores, sem opcionalidade implícita.
  - Não existe representação alternativa do mesmo valor. Uma estrutura, uma
    sequência de bytes.
"""

from __future__ import annotations

from .crypto import H

MAX_BYTES_LEN = 2**32 - 1
MAX_LIST_LEN = 2**32 - 1


class CodecError(Exception):
    """Dado malformado ou fora da faixa."""


# ---------------------------------------------------------------------------
# Escrita
# ---------------------------------------------------------------------------

def enc_u8(value: int) -> bytes:
    return _enc_uint(value, 1)


def enc_u16(value: int) -> bytes:
    return _enc_uint(value, 2)


def enc_u32(value: int) -> bytes:
    return _enc_uint(value, 4)


def enc_u64(value: int) -> bytes:
    return _enc_uint(value, 8)


def _enc_uint(value: int, width: int) -> bytes:
    if isinstance(value, bool) or not isinstance(value, int):
        raise CodecError(f"esperava int, veio {type(value).__name__}")
    if value < 0 or value >= 1 << (width * 8):
        raise CodecError(f"valor {value} não cabe em u{width * 8}")
    return value.to_bytes(width, "big")


def enc_bytes(data: bytes) -> bytes:
    """Bytes de tamanho variável: u32 big-endian de tamanho + conteúdo."""
    if not isinstance(data, (bytes, bytearray)):
        raise CodecError(f"esperava bytes, veio {type(data).__name__}")
    if len(data) > MAX_BYTES_LEN:
        raise CodecError("bytes longos demais")
    return enc_u32(len(data)) + bytes(data)


def enc_fixed(data: bytes, size: int) -> bytes:
    """Bytes de tamanho fixo, sem prefixo. Usado em hash e endereço."""
    if not isinstance(data, (bytes, bytearray)):
        raise CodecError(f"esperava bytes, veio {type(data).__name__}")
    if len(data) != size:
        raise CodecError(f"esperava {size} bytes, veio {len(data)}")
    return bytes(data)


def enc_str(text: str) -> bytes:
    if not isinstance(text, str):
        raise CodecError(f"esperava str, veio {type(text).__name__}")
    return enc_bytes(text.encode("utf-8"))


def enc_list(items: list, encode_item) -> bytes:
    if len(items) > MAX_LIST_LEN:
        raise CodecError("lista longa demais")
    out = [enc_u32(len(items))]
    out.extend(encode_item(item) for item in items)
    return b"".join(out)


# ---------------------------------------------------------------------------
# Leitura
# ---------------------------------------------------------------------------

class Reader:
    """Leitor posicional. Recusa sobra de bytes no fim, via `finish()`."""

    __slots__ = ("_data", "_pos")

    def __init__(self, data: bytes):
        self._data = bytes(data)
        self._pos = 0

    @property
    def remaining(self) -> int:
        return len(self._data) - self._pos

    def take(self, n: int) -> bytes:
        if n < 0:
            raise CodecError("tamanho negativo")
        if self.remaining < n:
            raise CodecError(
                f"fim inesperado: pediu {n} bytes, restam {self.remaining}"
            )
        chunk = self._data[self._pos : self._pos + n]
        self._pos += n
        return chunk

    def u8(self) -> int:
        return int.from_bytes(self.take(1), "big")

    def u16(self) -> int:
        return int.from_bytes(self.take(2), "big")

    def u32(self) -> int:
        return int.from_bytes(self.take(4), "big")

    def u64(self) -> int:
        return int.from_bytes(self.take(8), "big")

    def var_bytes(self) -> bytes:
        return self.take(self.u32())

    def fixed(self, size: int) -> bytes:
        return self.take(size)

    def string(self) -> str:
        raw = self.var_bytes()
        try:
            return raw.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise CodecError("string UTF-8 inválida") from exc

    def read_list(self, decode_item) -> list:
        return [decode_item(self) for _ in range(self.u32())]

    def finish(self) -> None:
        """Exige que a entrada tenha sido consumida por inteiro.

        Sobra de bytes é rejeitada de propósito: aceitar sobra deixa duas
        sequências diferentes decodificarem para a mesma estrutura, e aí o
        hash da estrutura deixa de identificá-la de forma única.
        """
        if self.remaining:
            raise CodecError(f"{self.remaining} bytes não consumidos no fim")


# ---------------------------------------------------------------------------
# Árvore de Merkle — RFC 6962
# ---------------------------------------------------------------------------

_LEAF_PREFIX = b"\x00"
_NODE_PREFIX = b"\x01"


def merkle_leaf_hash(data: bytes) -> bytes:
    return H(_LEAF_PREFIX + data)


def merkle_root(leaves: list[bytes]) -> bytes:
    """Raiz de Merkle no formato da RFC 6962 (Certificate Transparency).

    O protótipo concatenava todas as transações e tirava um SHA-512 do bolo.
    Isso não é árvore, não permite prova de inclusão, e não separa folha de
    nó interno.

    O formato da RFC 6962 resolve as duas coisas de uma vez:

      - Prefixo 0x00 em folha e 0x01 em nó interno impede que o hash de uma
        folha seja confundido com o de uma subárvore.
      - Em contagem ímpar, o último elemento é promovido, não duplicado.
        Duplicar o último é o bug clássico do Bitcoin (CVE-2012-2459), em que
        duas listas diferentes produzem a mesma raiz.
    """
    n = len(leaves)
    if n == 0:
        return H(b"")
    if n == 1:
        return merkle_leaf_hash(leaves[0])
    k = 1
    while k * 2 < n:
        k *= 2
    return H(_NODE_PREFIX + merkle_root(leaves[:k]) + merkle_root(leaves[k:]))


def merkle_path(leaves: list[bytes], index: int) -> list[bytes]:
    """Caminho de auditoria da folha `index` até a raiz."""
    n = len(leaves)
    if not 0 <= index < n:
        raise CodecError("índice de folha fora da faixa")
    if n == 1:
        return []
    k = 1
    while k * 2 < n:
        k *= 2
    if index < k:
        return merkle_path(leaves[:k], index) + [merkle_root(leaves[k:])]
    return merkle_path(leaves[k:], index - k) + [merkle_root(leaves[:k])]


def merkle_verify_path(leaf: bytes, index: int, total: int, path: list[bytes],
                       root: bytes) -> bool:
    """Confere uma prova de inclusão sem precisar da lista inteira.

    `merkle_path` devolve o caminho de baixo para cima, mas a divisão da
    árvore só é conhecida de cima para baixo. Então a descida é feita
    primeiro, registrando de que lado a folha cai em cada nível, e o caminho
    é consumido contra essa descida invertida.
    """
    if not 0 <= index < total or total < 1:
        return False

    sides: list[bool] = []  # True = folha está à esquerda
    idx, size = index, total
    while size > 1:
        k = 1
        while k * 2 < size:
            k *= 2
        if idx < k:
            sides.append(True)
            size = k
        else:
            sides.append(False)
            idx -= k
            size -= k

    if len(sides) != len(path):
        return False

    node = merkle_leaf_hash(leaf)
    for is_left, sibling in zip(reversed(sides), path):
        if is_left:
            node = H(_NODE_PREFIX + node + sibling)
        else:
            node = H(_NODE_PREFIX + sibling + node)
    return node == root
