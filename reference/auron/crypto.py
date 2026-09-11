# ✝ Gênesis 41:35-36 — “Ajuntem toda a comida destes bons anos que vêm; e esta comida será para provimento da terra, para os sete anos de fome.”
"""AURON — AACL, Auron Adaptive Cryptographic Layer.

Nenhum algoritmo é regra absoluta. Cada primitiva tem identificador e versão,
e o protocolo carrega o identificador junto com o dado. Trocar de algoritmo é
registrar um novo identificador, não reescrever o protocolo.

Decisões desta versão:

  HASH-SHA512-V1   hash geral, identificadores, Merkle
  SIG-ED25519-V1   assinatura padrão (substitui Brainpool P-512)
  SIG-BP512-V1     registrado apenas como legado do protótipo, NÃO utilizável

Por que Ed25519 no lugar de Brainpool P-512:

  - Verificação em microssegundos, contra dezenas de milissegundos do
    Brainpool com a biblioteca `ecdsa` em Python puro. Era um teto de
    throughput: mil transações num bloco levavam dezenas de segundos só
    para conferir assinatura.
  - Determinístico por construção (RFC 8032), sem nonce aleatório. ECDSA
    com nonce ruim vaza a chave privada.
  - Suporte Rust de primeira linha (`ed25519-dalek`) para o nó de produção.
  - A mainnet ainda não existe, então trocar agora não custa migração.

A implementação de Ed25519 aqui é Python puro seguindo a RFC 8032. É lenta de
propósito: o papel deste módulo é ser oráculo determinístico para gerar vetores
de teste que o Rust precisa reproduzir byte a byte. Zero dependências externas
significa zero divergência por versão de biblioteca.
"""

from __future__ import annotations

import hashlib
import secrets

# ---------------------------------------------------------------------------
# Identificadores AACL
# ---------------------------------------------------------------------------

HASH_SHA512_V1 = "HASH-SHA512-V1"
SIG_ED25519_V1 = "SIG-ED25519-V1"
SIG_BP512_V1 = "SIG-BP512-V1"  # legado do protótipo, não utilizável

DEFAULT_HASH = HASH_SHA512_V1
DEFAULT_SIG = SIG_ED25519_V1

ADDRESS_LEN = 20  # bytes


class CryptoError(Exception):
    """Falha criptográfica ou algoritmo desconhecido."""


# ---------------------------------------------------------------------------
# Hash
# ---------------------------------------------------------------------------

def hash_bytes(data: bytes, algo: str = DEFAULT_HASH) -> bytes:
    if algo == HASH_SHA512_V1:
        return hashlib.sha512(data).digest()
    raise CryptoError(f"algoritmo de hash desconhecido: {algo}")


def H(data: bytes) -> bytes:
    """Atalho para o hash padrão."""
    return hash_bytes(data, DEFAULT_HASH)


def hash_size(algo: str = DEFAULT_HASH) -> int:
    if algo == HASH_SHA512_V1:
        return 64
    raise CryptoError(f"algoritmo de hash desconhecido: {algo}")


def xof(seed: bytes, n: int, domain: bytes = b"") -> bytes:
    """Função de expansão determinística em modo contador.

    Usada para derivar desafios de verificação. Deliberadamente não usa o
    gerador do numpy: o fluxo de bits do numpy é uma dependência de versão que
    não pode entrar em nada que dois nós precisem calcular igual.
    """
    if n < 0:
        raise CryptoError("tamanho negativo")
    out = bytearray()
    counter = 0
    while len(out) < n:
        out += hashlib.sha512(domain + seed + counter.to_bytes(8, "big")).digest()
        counter += 1
    return bytes(out[:n])


# ---------------------------------------------------------------------------
# Ed25519 — RFC 8032, Python puro
# ---------------------------------------------------------------------------

_P = 2**255 - 19
_Q = 2**252 + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _P - 2, _P)) % _P
_SQRT_M1 = pow(2, (_P - 1) // 4, _P)

_GY = 4 * pow(5, _P - 2, _P) % _P
_GX = 0  # calculado abaixo


def _recover_x(y: int, sign: int) -> int | None:
    """Recupera x a partir de y na curva de Edwards, ou None se não existir."""
    if y >= _P:
        return None
    x2 = (y * y - 1) * pow(_D * y * y + 1, _P - 2, _P) % _P
    if x2 == 0:
        if sign:
            return None
        return 0
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P != 0:
        x = x * _SQRT_M1 % _P
    if (x * x - x2) % _P != 0:
        return None
    if (x & 1) != sign:
        x = _P - x
    return x


_GX = _recover_x(_GY, 0)
if _GX is None:
    raise AssertionError("ponto base Ed25519 inválido")

# Pontos em coordenadas estendidas (X, Y, Z, T), com x = X/Z, y = Y/Z, xy = T/Z.
_G = (_GX, _GY, 1, _GX * _GY % _P)
_IDENTITY = (0, 1, 1, 0)


def _point_add(p, q):
    px, py, pz, pt = p
    qx, qy, qz, qt = q
    a = (py - px) * (qy - qx) % _P
    b = (py + px) * (qy + qx) % _P
    c = 2 * pt * qt * _D % _P
    d = 2 * pz * qz % _P
    e, f, g, h = b - a, d - c, d + c, b + a
    return (e * f % _P, g * h % _P, f * g % _P, e * h % _P)


def _point_mul(s: int, p):
    r = _IDENTITY
    while s > 0:
        if s & 1:
            r = _point_add(r, p)
        p = _point_add(p, p)
        s >>= 1
    return r


def _point_equal(p, q) -> bool:
    px, py, pz, _ = p
    qx, qy, qz, _ = q
    if (px * qz - qx * pz) % _P != 0:
        return False
    return (py * qz - qy * pz) % _P == 0


def _point_compress(p) -> bytes:
    x, y, z, _ = p
    zinv = pow(z, _P - 2, _P)
    x = x * zinv % _P
    y = y * zinv % _P
    return int.to_bytes(y | ((x & 1) << 255), 32, "little")


def _point_decompress(data: bytes):
    if len(data) != 32:
        return None
    y = int.from_bytes(data, "little")
    sign = (y >> 255) & 1
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    if x is None:
        return None
    return (x, y, 1, x * y % _P)


def _sha512_int(data: bytes) -> int:
    return int.from_bytes(hashlib.sha512(data).digest(), "little")


def _secret_expand(secret: bytes):
    if len(secret) != 32:
        raise CryptoError("chave privada Ed25519 deve ter 32 bytes")
    h = hashlib.sha512(secret).digest()
    a = int.from_bytes(h[:32], "little")
    a &= (1 << 254) - 8   # zera os 3 bits baixos
    a |= 1 << 254         # fixa o bit 254
    return a, h[32:]


def ed25519_public_key(secret: bytes) -> bytes:
    a, _ = _secret_expand(secret)
    return _point_compress(_point_mul(a, _G))


def ed25519_sign(secret: bytes, msg: bytes) -> bytes:
    a, prefix = _secret_expand(secret)
    pub = _point_compress(_point_mul(a, _G))
    r = _sha512_int(prefix + msg) % _Q
    big_r = _point_compress(_point_mul(r, _G))
    k = _sha512_int(big_r + pub + msg) % _Q
    s = (r + k * a) % _Q
    return big_r + int.to_bytes(s, 32, "little")


def ed25519_verify(pub: bytes, msg: bytes, sig: bytes) -> bool:
    if len(sig) != 64 or len(pub) != 32:
        return False
    point_a = _point_decompress(pub)
    if point_a is None:
        return False
    point_r = _point_decompress(sig[:32])
    if point_r is None:
        return False
    s = int.from_bytes(sig[32:], "little")
    if s >= _Q:
        # Recusar S fora da faixa fecha a maleabilidade de assinatura.
        return False
    # Ponto de ordem pequena em A ou em R NAO e recusado, de proposito: e a
    # regra da AURON-SPEC-01, secao 2. O Rust reproduz, e
    # vectors/crypto_ed25519_verify.json trava a regra nos dois lados.
    k = _sha512_int(sig[:32] + pub + msg) % _Q
    left = _point_mul(s, _G)
    right = _point_add(point_r, _point_mul(k, point_a))
    return _point_equal(left, right)


# ---------------------------------------------------------------------------
# Interface AACL de assinatura
# ---------------------------------------------------------------------------

def generate_secret() -> bytes:
    """32 bytes de segredo, do gerador criptográfico do sistema."""
    return secrets.token_bytes(32)


def public_key(secret: bytes, algo: str = DEFAULT_SIG) -> bytes:
    if algo == SIG_ED25519_V1:
        return ed25519_public_key(secret)
    if algo == SIG_BP512_V1:
        raise CryptoError("SIG-BP512-V1 é legado do protótipo e não é utilizável")
    raise CryptoError(f"algoritmo de assinatura desconhecido: {algo}")


def sign(secret: bytes, msg: bytes, algo: str = DEFAULT_SIG) -> bytes:
    if algo == SIG_ED25519_V1:
        return ed25519_sign(secret, msg)
    if algo == SIG_BP512_V1:
        raise CryptoError("SIG-BP512-V1 é legado do protótipo e não é utilizável")
    raise CryptoError(f"algoritmo de assinatura desconhecido: {algo}")


def verify(pub: bytes, msg: bytes, sig: bytes, algo: str = DEFAULT_SIG) -> bool:
    if algo == SIG_ED25519_V1:
        return ed25519_verify(pub, msg, sig)
    if algo == SIG_BP512_V1:
        return False
    raise CryptoError(f"algoritmo de assinatura desconhecido: {algo}")


def pubkey_size(algo: str = DEFAULT_SIG) -> int:
    if algo == SIG_ED25519_V1:
        return 32
    raise CryptoError(f"algoritmo de assinatura desconhecido: {algo}")


def signature_size(algo: str = DEFAULT_SIG) -> int:
    if algo == SIG_ED25519_V1:
        return 64
    raise CryptoError(f"algoritmo de assinatura desconhecido: {algo}")


# ---------------------------------------------------------------------------
# Endereços
# ---------------------------------------------------------------------------

def address_from_pubkey(pub: bytes, sig_algo: str = DEFAULT_SIG) -> bytes:
    """Endereço = primeiros 20 bytes de H(sig_algo || pubkey).

    O identificador do algoritmo entra no hash de propósito: a mesma chave sob
    algoritmos diferentes precisa dar endereços diferentes, senão trocar de
    algoritmo no futuro cria colisão de identidade.
    """
    if len(pub) != pubkey_size(sig_algo):
        raise CryptoError("tamanho de chave pública inválido")
    return H(sig_algo.encode("ascii") + pub)[:ADDRESS_LEN]


def address_to_hex(addr: bytes) -> str:
    if len(addr) != ADDRESS_LEN:
        raise CryptoError("endereço com tamanho inválido")
    return addr.hex()


def address_from_hex(text: str) -> bytes:
    try:
        addr = bytes.fromhex(text.strip())
    except ValueError as exc:
        raise CryptoError(f"endereço hex inválido: {text!r}") from exc
    if len(addr) != ADDRESS_LEN:
        raise CryptoError("endereço com tamanho inválido")
    return addr
