"""Os vetores conferidos contra o manifesto e contra o oraculo atual.

O MANIFEST.json declara tamanho e hash de cada arquivo de vetor. Sem este
teste ninguem confere: o manifesto ja ficou desatualizado por um commit
inteiro (01d255a) sem nada acusar.

Os casos de borda (verificacao Ed25519 e leitura do codec) foram gerados pelo
oraculo. Aqui eles sao conferidos de novo contra o oraculo de agora: se alguem
mudar uma regra no Python sem regerar os vetores, este teste acusa.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import crypto  # noqa: E402

VECTORS = Path(__file__).resolve().parents[2] / "vectors"


def _manifest() -> dict:
    return json.loads((VECTORS / "MANIFEST.json").read_text(encoding="utf-8"))


def test_manifest_matches_files():
    files = _manifest()["files"]
    assert len(files) >= 13, f"poucos arquivos no manifesto: {len(files)}"
    for name, expected in sorted(files.items()):
        # Bytes crus: ler como texto esconderia uma conversao de fim de linha.
        raw = (VECTORS / name).read_bytes()
        assert len(raw) == expected["bytes"], (
            f"{name}: {len(raw)} bytes, o manifesto diz {expected['bytes']}"
        )
        assert crypto.H(raw).hex()[:32] == expected["sha512_16"], (
            f"{name}: hash diverge do manifesto"
        )
    print(f"PASS manifesto bate com os {len(files)} arquivos de vetores")


def test_manifest_lists_every_vector_file():
    listed = set(_manifest()["files"])
    on_disk = {p.name for p in VECTORS.glob("*.json")} - {"MANIFEST.json"}
    assert listed == on_disk, (
        f"fora do manifesto: {sorted(on_disk - listed)}; "
        f"no manifesto mas ausente: {sorted(listed - on_disk)}"
    )
    print("PASS nenhum arquivo de vetor fora do manifesto")


def test_verify_edge_vectors_match_oracle():
    doc = json.loads((VECTORS / "crypto_ed25519_verify.json").read_text(encoding="utf-8"))
    cases = doc["data"]
    accepted = rejected = 0
    for case in cases:
        got = crypto.verify(
            bytes.fromhex(case["public_key"]),
            bytes.fromhex(case["message"]),
            bytes.fromhex(case["signature"]),
        )
        assert got == case["valid"], (
            f"{case['label']}: o vetor diz {case['valid']}, o oraculo diz {got}"
        )
        if got:
            accepted += 1
        else:
            rejected += 1
    assert accepted >= 4, f"poucos casos aceitos: {accepted}"
    assert rejected >= 10, f"poucos casos recusados: {rejected}"
    print(f"PASS {len(cases)} casos de borda ed25519 batem com o oraculo "
          f"({accepted} aceitos, {rejected} recusados)")


def test_codec_edge_vectors_match_oracle():
    # Regerar e comparar e mais forte que repetir a leitura aqui: nao duplica
    # o interpretador de operacoes, e qualquer mudanca de regra no codec.py
    # aparece como diferenca.
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
    from gen_vectors import vec_codec_edge  # noqa: E402

    doc = json.loads((VECTORS / "codec_edge.json").read_text(encoding="utf-8"))
    fresh = vec_codec_edge()
    assert doc["data"] == fresh, (
        "codec_edge.json diverge do oraculo atual: rode tools/gen_vectors.py"
    )

    decode = fresh["decode"]
    accepted = sum(1 for c in decode if c["accepted"])
    rejected = len(decode) - accepted
    assert accepted >= 18, f"poucas leituras aceitas: {accepted}"
    assert rejected >= 20, f"poucas leituras recusadas: {rejected}"

    verify = fresh["merkle_verify"]
    valid = sum(1 for c in verify if c["valid"])
    assert valid >= 4, f"poucas provas aceitas: {valid}"
    assert len(verify) - valid >= 10, f"poucas provas recusadas: {len(verify) - valid}"
    print(f"PASS codec_edge bate com o oraculo ({accepted} leituras aceitas, "
          f"{rejected} recusadas; {valid} provas aceitas, {len(verify) - valid} recusadas)")


if __name__ == "__main__":
    test_manifest_matches_files()
    test_manifest_lists_every_vector_file()
    test_verify_edge_vectors_match_oracle()
    test_codec_edge_vectors_match_oracle()
    print("=== VETORES OK ===")
