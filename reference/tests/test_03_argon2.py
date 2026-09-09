"""Argon2 contra os vetores oficiais da RFC 9106, secao 5.

Se este teste passar, a implementacao Python pura esta correta e o Rust
(crate `argon2` da RustCrypto) tem com o que ser comparado.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import argon2  # noqa: E402

# RFC 9106, secao 5: mesmos parametros para as tres variantes.
PASSWORD = b"\x01" * 32
SALT = b"\x02" * 16
SECRET = b"\x03" * 8
AD = b"\x04" * 12

PARAMS = dict(
    time_cost=3,
    memory_kib=32,
    parallelism=4,
    tag_length=32,
    secret=SECRET,
    associated_data=AD,
)

# Secao 5.1 (Argon2d), 5.2 (Argon2i), 5.3 (Argon2id)
VECTORS = {
    argon2.TYPE_D: "512b391b6f1162975371d30919734294f868e3be3984f3c1a13a4db9fabe4acb",
    argon2.TYPE_I: "c814d9d1dc7f37aa13f0d77f2494bda1c8de6b016dd388d29952a4c4672b6ce8",
    argon2.TYPE_ID: "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659",
}

NAMES = {argon2.TYPE_D: "Argon2d", argon2.TYPE_I: "Argon2i", argon2.TYPE_ID: "Argon2id"}


def test_rfc9106_vectors():
    for variant, expected in VECTORS.items():
        t0 = time.perf_counter()
        tag = argon2.argon2id(PASSWORD, SALT, variant=variant, **PARAMS)
        ms = (time.perf_counter() - t0) * 1000
        assert tag.hex() == expected, (
            f"{NAMES[variant]}: esperado {expected}, veio {tag.hex()}"
        )
        print(f"PASS {NAMES[variant]} RFC 9106 ({ms:.0f}ms em 32 KiB)")


def test_parameters_change_output():
    base = argon2.argon2id(PASSWORD, SALT, **PARAMS)

    other = dict(PARAMS)
    other["time_cost"] = 4
    assert argon2.argon2id(PASSWORD, SALT, **other) != base

    other = dict(PARAMS)
    other["memory_kib"] = 64
    assert argon2.argon2id(PASSWORD, SALT, **other) != base

    other = dict(PARAMS)
    other["parallelism"] = 2
    assert argon2.argon2id(PASSWORD, SALT, **other) != base

    assert argon2.argon2id(PASSWORD, b"\x05" * 16, **PARAMS) != base
    assert argon2.argon2id(b"\x09" * 32, SALT, **PARAMS) != base
    print("PASS cada parametro muda a saida")


def test_deterministic():
    a = argon2.argon2id(PASSWORD, SALT, **PARAMS)
    b = argon2.argon2id(PASSWORD, SALT, **PARAMS)
    assert a == b, "Argon2id nao e deterministico"
    print("PASS deterministico")


def test_tag_length_respected():
    for n in (4, 16, 32, 64, 65, 100):
        tag = argon2.argon2id(
            PASSWORD, SALT,
            time_cost=1, memory_kib=32, parallelism=1, tag_length=n,
        )
        assert len(tag) == n, f"tag_length={n} devolveu {len(tag)} bytes"
    print("PASS tag de tamanho variavel (inclui o ramo H' > 64 bytes)")


def test_invalid_parameters_rejected():
    for bad in (
        dict(time_cost=0, memory_kib=32, parallelism=1),
        dict(time_cost=1, memory_kib=32, parallelism=0),
        dict(time_cost=1, memory_kib=4, parallelism=1),   # m < 8*p
        dict(time_cost=1, memory_kib=32, parallelism=1, tag_length=2),
    ):
        try:
            argon2.argon2id(PASSWORD, SALT, **bad)
            raise AssertionError(f"parametro invalido aceito: {bad}")
        except argon2.Argon2Error:
            pass
    print("PASS parametros invalidos rejeitados")


if __name__ == "__main__":
    test_rfc9106_vectors()
    test_deterministic()
    test_parameters_change_output()
    test_tag_length_respected()
    test_invalid_parameters_rejected()
    print("=== ARGON2 OK ===")
