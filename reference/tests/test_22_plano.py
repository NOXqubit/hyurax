# ✝ Lucas 14:28 — “Qual de vós, querendo edificar uma torre, não se assenta primeiro a fazer as contas dos gastos?”
"""Planos: voucher assinado, quando vale, e o catálogo."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import codec, crypto, plano  # noqa: E402

EMISSOR = bytes(range(32))
PUB = crypto.ed25519_public_key(EMISSOR)
W = bytes([5] * 32)
DIA = 86_400_000


def _recusa(f, *args) -> bool:
    try:
        f(*args)
    except (ValueError, codec.CodecError):
        return True
    return False


def test_ida_e_volta_e_quando_vale():
    b = plano.emitir(EMISSOR, plano.PRO, W, 1000, 1000 + 30 * DIA, 7)
    assert len(b) == plano.TAMANHO
    v = plano.ler_texto(plano.texto(b))
    assert (v["plano"], v["serie"], v["creditos_mes"]) == (plano.PRO, 7, 150_000)
    assert plano.vale(v, PUB, W, 1000) and plano.vale(v, PUB, W, 1000 + 30 * DIA - 1)
    assert not plano.vale(v, PUB, W, 999), "antes do início"
    assert not plano.vale(v, PUB, W, 1000 + 30 * DIA), "o fim não está incluído"
    assert not plano.vale(v, PUB, bytes(32), 2000), "outro nó"
    assert not plano.vale(v, crypto.ed25519_public_key(bytes(32)), W, 2000), "outra chave de planos"


def test_emitir_recusa_o_que_nao_e_venda():
    assert _recusa(plano.emitir, EMISSOR, plano.COMUNIDADE, W, 0, DIA, 1), "o grátis não tem voucher"
    assert _recusa(plano.emitir, EMISSOR, 42, W, 0, DIA, 1)
    assert _recusa(plano.emitir, EMISSOR, plano.PRO, W, DIA, DIA, 1)
    assert _recusa(plano.emitir, EMISSOR, plano.PRO, W, 0, 401 * DIA, 1)


def test_precos_coerentes():
    for k, (_, mensal, anual, creditos, gib, bp) in plano.CATALOGO.items():
        assert anual <= mensal * 12, "o anual nunca sai mais caro"
        assert 0 <= bp <= 10_000
        if k != plano.COMUNIDADE:
            assert mensal > 0 and creditos > 0 and gib > 0


def test_vetores_batem_com_o_gerador():
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
    import gen_vectors  # noqa: E402

    gravado = json.loads((Path(__file__).resolve().parents[2] / "vectors" / "plano.json").read_text(encoding="utf-8"))["data"]
    assert gravado == json.loads(json.dumps(gen_vectors.vec_plano()))
