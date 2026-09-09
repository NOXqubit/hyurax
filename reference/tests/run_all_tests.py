"""Executa toda a suite da implementacao de referencia.

Uso:
    python tests/run_all_tests.py

Cada modulo roda isolado. Uma falha nao esconde as outras: o executor segue
ate o fim e so entao devolve codigo de saida diferente de zero.
"""

from __future__ import annotations

import importlib.util
import sys
import time
import traceback
from pathlib import Path

TESTS_DIR = Path(__file__).resolve().parent
ROOT = TESTS_DIR.parent
sys.path.insert(0, str(ROOT))

MODULES = [
    ("test_01_crypto.py", "AACL / Ed25519 (RFC 8032)"),
    ("test_02_codec.py", "codificacao canonica / Merkle (RFC 6962)"),
    ("test_03_argon2.py", "Argon2id (RFC 9106)"),
    ("test_04_consensus.py", "alvo, retarget LWMA, emissao"),
    ("test_05_chain.py", "transacoes, estado, cadeia, ataques"),
    ("test_06_utrax.py", "trabalho util fora do consenso"),
    ("test_07_store.py", "persistencia"),
]


def load_module(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    total_passed = 0
    failures: list[tuple[str, str, str]] = []
    started = time.perf_counter()

    for filename, description in MODULES:
        path = TESTS_DIR / filename
        print(f"\n{'=' * 68}")
        print(f"  {filename}  —  {description}")
        print("=" * 68)

        if not path.exists():
            failures.append((filename, "<arquivo>", "arquivo nao encontrado"))
            print("  FALTANDO")
            continue

        try:
            module = load_module(path)
        except Exception:
            failures.append((filename, "<import>", traceback.format_exc()))
            print("  ERRO AO IMPORTAR")
            continue

        names = [n for n in dir(module) if n.startswith("test_")]
        names.sort(key=lambda n: getattr(module, n).__code__.co_firstlineno)

        for name in names:
            fn = getattr(module, name)
            if not callable(fn):
                continue
            t0 = time.perf_counter()
            try:
                fn()
                total_passed += 1
            except Exception:
                failures.append((filename, name, traceback.format_exc()))
                print(f"  FALHOU {name} ({(time.perf_counter() - t0) * 1000:.0f}ms)")

    elapsed = time.perf_counter() - started
    print(f"\n{'=' * 68}")
    if failures:
        print(f"  {len(failures)} FALHA(S), {total_passed} passaram, {elapsed:.1f}s")
        print("=" * 68)
        for filename, name, tb in failures:
            print(f"\n--- {filename}::{name} ---")
            print(tb)
        return 1

    print(f"  TODOS OS {total_passed} TESTES PASSARAM em {elapsed:.1f}s")
    print("=" * 68)
    return 0


if __name__ == "__main__":
    sys.exit(main())
