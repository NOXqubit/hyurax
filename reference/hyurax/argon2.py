# ✝ Gênesis 41:35-36 — “Ajuntem toda a comida destes bons anos que vêm; e esta comida será para provimento da terra, para os sete anos de fome.”
"""Argon2id em Python puro — RFC 9106.

Este módulo existe para que a implementação de referência não dependa de
nenhuma biblioteca externa. O nó Rust usará a crate `argon2` da RustCrypto;
o papel daqui é ser o oráculo que prova que as duas concordam.

É lento de propósito. Em parâmetros de produção (32 MiB) uma avaliação leva
dezenas de segundos em Python. Os vetores de teste do protocolo são gerados
em parâmetros pequenos: o algoritmo não muda com o parâmetro, então validar
em 32 KiB valida a implementação inteira.

Validado contra o vetor oficial da RFC 9106, seção 5.3.
"""

from __future__ import annotations

import hashlib
import struct

MASK64 = (1 << 64) - 1
MASK32 = (1 << 32) - 1
BLOCK_SIZE = 1024        # bytes
BLOCK_WORDS = 128        # u64 por bloco
SYNC_POINTS = 4          # fatias por passada
ARGON2_VERSION = 0x13

TYPE_D = 0
TYPE_I = 1
TYPE_ID = 2


class Argon2Error(Exception):
    """Parâmetro inválido."""


def _le32(value: int) -> bytes:
    return struct.pack("<I", value)


def _le64(value: int) -> bytes:
    return struct.pack("<Q", value)


def _blake2b(data: bytes, outlen: int) -> bytes:
    return hashlib.blake2b(data, digest_size=outlen).digest()


def _h_prime(data: bytes, outlen: int) -> bytes:
    """H' — hash de saída variável da RFC 9106, seção 3.3."""
    if outlen <= 64:
        return _blake2b(_le32(outlen) + data, outlen)

    r = (outlen + 31) // 32 - 2
    out = bytearray()
    v = _blake2b(_le32(outlen) + data, 64)
    out += v[:32]
    for _ in range(r - 1):
        v = _blake2b(v, 64)
        out += v[:32]
    last = outlen - 32 * r
    v = _blake2b(v, last)
    out += v
    return bytes(out[:outlen])


def _rotr64(value: int, n: int) -> int:
    return ((value >> n) | (value << (64 - n))) & MASK64


def _gb(v: list[int], a: int, b: int, c: int, d: int) -> None:
    """Função G do Argon2: Blake2b round com a multiplicação extra (BlaMka)."""
    va, vb, vc, vd = v[a], v[b], v[c], v[d]

    va = (va + vb + 2 * (va & MASK32) * (vb & MASK32)) & MASK64
    vd = _rotr64(vd ^ va, 32)
    vc = (vc + vd + 2 * (vc & MASK32) * (vd & MASK32)) & MASK64
    vb = _rotr64(vb ^ vc, 24)
    va = (va + vb + 2 * (va & MASK32) * (vb & MASK32)) & MASK64
    vd = _rotr64(vd ^ va, 16)
    vc = (vc + vd + 2 * (vc & MASK32) * (vd & MASK32)) & MASK64
    vb = _rotr64(vb ^ vc, 63)

    v[a], v[b], v[c], v[d] = va, vb, vc, vd


def _permute(v: list[int]) -> None:
    """Permutação P sobre 16 palavras de 64 bits, no lugar."""
    _gb(v, 0, 4, 8, 12)
    _gb(v, 1, 5, 9, 13)
    _gb(v, 2, 6, 10, 14)
    _gb(v, 3, 7, 11, 15)
    _gb(v, 0, 5, 10, 15)
    _gb(v, 1, 6, 11, 12)
    _gb(v, 2, 7, 8, 13)
    _gb(v, 3, 4, 9, 14)


def _compress(x: list[int], y: list[int]) -> list[int]:
    """Função de compressão G(X, Y) sobre blocos de 128 palavras."""
    r = [x[i] ^ y[i] for i in range(BLOCK_WORDS)]
    q = list(r)

    # Passo 1: permutação nas 8 linhas de 16 palavras.
    for i in range(8):
        chunk = q[i * 16 : i * 16 + 16]
        _permute(chunk)
        q[i * 16 : i * 16 + 16] = chunk

    # Passo 2: permutação nas 8 colunas. Cada coluna toma pares de palavras
    # espaçados de 16 em 16.
    for i in range(8):
        idx = [2 * i + 16 * k + off for k in range(8) for off in (0, 1)]
        chunk = [q[j] for j in idx]
        _permute(chunk)
        for pos, j in enumerate(idx):
            q[j] = chunk[pos]

    return [q[i] ^ r[i] for i in range(BLOCK_WORDS)]


def _block_to_bytes(block: list[int]) -> bytes:
    return struct.pack("<128Q", *block)


def _bytes_to_block(data: bytes) -> list[int]:
    return list(struct.unpack("<128Q", data))


def argon2id(
    password: bytes,
    salt: bytes,
    *,
    time_cost: int,
    memory_kib: int,
    parallelism: int,
    tag_length: int = 32,
    secret: bytes = b"",
    associated_data: bytes = b"",
    variant: int = TYPE_ID,
) -> bytes:
    """Argon2id conforme RFC 9106.

    `memory_kib` é em KiB. `parallelism` são as faixas (lanes).
    """
    if parallelism < 1:
        raise Argon2Error("parallelism deve ser >= 1")
    if time_cost < 1:
        raise Argon2Error("time_cost deve ser >= 1")
    if tag_length < 4:
        raise Argon2Error("tag_length deve ser >= 4")
    if memory_kib < 8 * parallelism:
        raise Argon2Error("memory_kib deve ser >= 8 * parallelism")

    # ---- H0 ----
    h0 = _blake2b(
        _le32(parallelism)
        + _le32(tag_length)
        + _le32(memory_kib)
        + _le32(time_cost)
        + _le32(ARGON2_VERSION)
        + _le32(variant)
        + _le32(len(password)) + password
        + _le32(len(salt)) + salt
        + _le32(len(secret)) + secret
        + _le32(len(associated_data)) + associated_data,
        64,
    )

    # ---- geometria da memória ----
    m_prime = (memory_kib // (SYNC_POINTS * parallelism)) * (SYNC_POINTS * parallelism)
    lane_len = m_prime // parallelism           # q
    segment_len = lane_len // SYNC_POINTS       # colunas por fatia

    # B[lane][col]
    memory = [[None] * lane_len for _ in range(parallelism)]
    for lane in range(parallelism):
        memory[lane][0] = _bytes_to_block(
            _h_prime(h0 + _le32(0) + _le32(lane), BLOCK_SIZE)
        )
        memory[lane][1] = _bytes_to_block(
            _h_prime(h0 + _le32(1) + _le32(lane), BLOCK_SIZE)
        )

    zero_block = [0] * BLOCK_WORDS

    for passno in range(time_cost):
        for slice_no in range(SYNC_POINTS):
            for lane in range(parallelism):
                # Argon2id: indexação independente de dado só na primeira
                # metade da primeira passada. Depois vira dependente.
                data_independent = variant == TYPE_I or (
                    variant == TYPE_ID and passno == 0 and slice_no < SYNC_POINTS // 2
                )

                address_block: list[int] | None = None
                address_counter = 0
                if data_independent:
                    address_block, address_counter = _next_address_block(
                        zero_block, passno, lane, slice_no,
                        m_prime, time_cost, variant, 0,
                    )

                start_col = 2 if (passno == 0 and slice_no == 0) else 0
                for idx in range(start_col, segment_len):
                    col = slice_no * segment_len + idx
                    prev_col = (col - 1) if col > 0 else (lane_len - 1)
                    prev = memory[lane][prev_col]

                    if data_independent:
                        pos = idx % BLOCK_WORDS
                        if pos == 0 and idx != 0:
                            address_block, address_counter = _next_address_block(
                                zero_block, passno, lane, slice_no,
                                m_prime, time_cost, variant, address_counter,
                            )
                        word = address_block[pos]
                    else:
                        word = prev[0]

                    j1 = word & MASK32
                    j2 = (word >> 32) & MASK32

                    ref_lane = lane if (passno == 0 and slice_no == 0) else (j2 % parallelism)
                    ref_col = _ref_index(
                        j1, passno, slice_no, idx, lane, ref_lane,
                        segment_len, lane_len,
                    )

                    new = _compress(prev, memory[ref_lane][ref_col])
                    if passno == 0:
                        memory[lane][col] = new
                    else:
                        old = memory[lane][col]
                        memory[lane][col] = [new[i] ^ old[i] for i in range(BLOCK_WORDS)]

    # ---- final ----
    final = list(memory[0][lane_len - 1])
    for lane in range(1, parallelism):
        other = memory[lane][lane_len - 1]
        final = [final[i] ^ other[i] for i in range(BLOCK_WORDS)]

    return _h_prime(_block_to_bytes(final), tag_length)


def _next_address_block(zero_block, passno, lane, slice_no, m_prime,
                        time_cost, variant, counter):
    """Gera o próximo bloco de endereços para a indexação independente."""
    counter += 1
    z = [0] * BLOCK_WORDS
    z[0] = passno
    z[1] = lane
    z[2] = slice_no
    z[3] = m_prime
    z[4] = time_cost
    z[5] = variant
    z[6] = counter
    return _compress(zero_block, _compress(zero_block, z)), counter


def _ref_index(j1, passno, slice_no, idx, lane, ref_lane,
               segment_len, lane_len) -> int:
    """Índice do bloco de referência, RFC 9106 seção 3.4.1.2."""
    same_lane = ref_lane == lane

    if passno == 0:
        if slice_no == 0:
            window = idx - 1
        elif same_lane:
            window = slice_no * segment_len + idx - 1
        else:
            window = slice_no * segment_len - (1 if idx == 0 else 0)
        start = 0
    else:
        if same_lane:
            window = lane_len - segment_len + idx - 1
        else:
            window = lane_len - segment_len - (1 if idx == 0 else 0)
        start = 0 if slice_no == SYNC_POINTS - 1 else (slice_no + 1) * segment_len

    if window < 1:
        window = 1

    x = (j1 * j1) >> 32
    y = (window * x) >> 32
    z = window - 1 - y
    return (start + z) % lane_len
