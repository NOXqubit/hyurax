"""Codificacao canonica e arvore de Merkle."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import codec  # noqa: E402
from auron.codec import CodecError  # noqa: E402


def test_uint_widths_and_bounds():
    assert codec.enc_u8(0) == b"\x00"
    assert codec.enc_u8(255) == b"\xff"
    assert codec.enc_u32(1) == b"\x00\x00\x00\x01"
    assert codec.enc_u64(1) == b"\x00" * 7 + b"\x01"

    for fn, width in ((codec.enc_u8, 1), (codec.enc_u16, 2),
                      (codec.enc_u32, 4), (codec.enc_u64, 8)):
        assert len(fn(0)) == width
        try:
            fn(1 << (width * 8))
            raise AssertionError(f"u{width * 8} aceitou valor grande demais")
        except CodecError:
            pass
        try:
            fn(-1)
            raise AssertionError(f"u{width * 8} aceitou negativo")
        except CodecError:
            pass

    # bool nao e int aqui, de proposito
    try:
        codec.enc_u8(True)
        raise AssertionError("bool foi aceito como inteiro")
    except CodecError:
        pass
    print("PASS inteiros de largura fixa")


def test_roundtrip():
    blob = (
        codec.enc_u64(2**63)
        + codec.enc_bytes(b"\x00\x01\x02")
        + codec.enc_str("acentuacao: ção")
        + codec.enc_fixed(b"A" * 20, 20)
        + codec.enc_list([1, 2, 3], codec.enc_u32)
    )
    r = codec.Reader(blob)
    assert r.u64() == 2**63
    assert r.var_bytes() == b"\x00\x01\x02"
    assert r.string() == "acentuacao: ção"
    assert r.fixed(20) == b"A" * 20
    assert r.read_list(lambda rd: rd.u32()) == [1, 2, 3]
    r.finish()
    print("PASS roundtrip")


def test_trailing_bytes_rejected():
    r = codec.Reader(codec.enc_u32(7) + b"sobra")
    assert r.u32() == 7
    try:
        r.finish()
        raise AssertionError("sobra de bytes foi aceita")
    except CodecError:
        pass
    print("PASS sobra de bytes rejeitada")


def test_truncated_input_rejected():
    r = codec.Reader(b"\x00\x00")
    try:
        r.u32()
        raise AssertionError("leitura alem do fim foi aceita")
    except CodecError:
        pass

    # prefixo de tamanho mente sobre o conteudo
    r2 = codec.Reader(codec.enc_u32(1000) + b"curto")
    try:
        r2.var_bytes()
        raise AssertionError("prefixo mentiroso foi aceito")
    except CodecError:
        pass
    print("PASS entrada truncada rejeitada")


def test_merkle_basics():
    assert codec.merkle_root([]) != b""
    one = codec.merkle_root([b"a"])
    assert one == codec.merkle_leaf_hash(b"a")
    # folha nunca colide com no interno, por causa do prefixo de dominio
    assert codec.merkle_root([b"a"]) != codec.merkle_root([b"a", b"a"])
    print("PASS merkle basico")


def test_merkle_order_matters():
    assert codec.merkle_root([b"a", b"b"]) != codec.merkle_root([b"b", b"a"])
    print("PASS ordem importa")


def test_merkle_no_duplicate_last_collision():
    """CVE-2012-2459: duplicar o ultimo elemento nao pode gerar a mesma raiz.

    Com a regra do Bitcoin, [a,b,c] e [a,b,c,c] dao a mesma raiz. Com a
    RFC 6962 elas precisam divergir.
    """
    a, b, c = b"a", b"b", b"c"
    assert codec.merkle_root([a, b, c]) != codec.merkle_root([a, b, c, c])
    assert codec.merkle_root([a, b]) != codec.merkle_root([a, b, a, b])
    print("PASS sem colisao por duplicacao do ultimo")


def test_merkle_inclusion_proofs():
    for total in range(1, 18):
        leaves = [f"folha-{i}".encode() for i in range(total)]
        root = codec.merkle_root(leaves)
        for i in range(total):
            path = codec.merkle_path(leaves, i)
            assert codec.merkle_verify_path(leaves[i], i, total, path, root), \
                f"prova valida falhou (total={total}, i={i})"
            # folha trocada precisa falhar
            assert not codec.merkle_verify_path(b"impostora", i, total, path, root), \
                f"folha falsa passou (total={total}, i={i})"
            # indice trocado precisa falhar
            if total > 1:
                other = (i + 1) % total
                assert not codec.merkle_verify_path(
                    leaves[i], other, total, path, root
                ), f"indice errado passou (total={total}, i={i})"
    print("PASS provas de inclusao em 1..17 folhas")


if __name__ == "__main__":
    test_uint_widths_and_bounds()
    test_roundtrip()
    test_trailing_bytes_rejected()
    test_truncated_input_rejected()
    test_merkle_basics()
    test_merkle_order_matters()
    test_merkle_no_duplicate_last_collision()
    test_merkle_inclusion_proofs()
    print("=== CODEC OK ===")
