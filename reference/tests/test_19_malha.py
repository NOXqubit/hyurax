# ✝ Eclesiastes 4:12 — “O cordão de três dobras não se quebra tão depressa.”
"""Malha: mensagens, prefixos de conexão e anúncio na rede local."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import codec, malha  # noqa: E402


def _recusada(f, dados: bytes) -> bool:
    try:
        f(dados)
    except (ValueError, codec.CodecError):
        return True
    return False


def test_cada_mensagem_vai_e_volta():
    t = bytes(range(16))
    assert malha.ler(malha.pedir_alcance(8790, t)) == {"subtipo": "pedir_alcance", "porta": 8790, "token": t}
    a = malha.ler(malha.alcance(4, bytes([200, 1, 2, 3]), 8790, True))
    assert (a["familia"], a["ip"], a["porta"], a["tentou"]) == (4, bytes([200, 1, 2, 3]), 8790, True)
    assert malha.ler(malha.reservar(t))["token"] == t
    assert malha.ler(malha.reserva(t, False))["aceita"] is False
    lista = [(bytes([7]) * 32, 6, bytes(range(16)), 8790), (bytes([8]) * 32, 4, bytes([10, 0, 0, 1]), 1)]
    assert malha.ler(malha.pontes(lista))["lista"] == lista


def test_recusas():
    bom = malha.reservar(bytes(16))
    for ruim in (bom + b"\x00", bom[:-1], bytes([2]) + bom[1:], bytes([1, 9]), b""):
        assert _recusada(malha.ler, ruim), ruim.hex()
    # família 5 e marca 2
    assert _recusada(malha.ler, bytes([1, 2, 5]) + bytes(6) + b"\x01")
    assert _recusada(malha.ler, malha.alcance(4, bytes(4), 1, True)[:-1] + b"\x02")
    # 33 pontes
    assert _recusada(malha.ler, bytes([1, 5, 33]))


def test_anuncio_de_vizinho_e_prefixos():
    a = malha.anuncio_vizinho(b"HYXT", 8790, bytes([9]) * 32)
    assert len(a) == 42
    assert malha.ler_anuncio_vizinho(a) == {"magic": b"HYXT", "porta": 8790, "identidade": bytes([9]) * 32}
    assert _recusada(malha.ler_anuncio_vizinho, a[:-1])
    assert _recusada(malha.ler_anuncio_vizinho, malha.anuncio_vizinho(b"HYXT", 0, bytes(32)))
    assert malha.prefixo(b"HXC1" + bytes(32))["tipo"] == "circuito"
    assert malha.prefixo(b"HXR1" + bytes(16))["tipo"] == "reserva"
    assert _recusada(malha.prefixo, b"HXC1" + bytes(31))
    # o primeiro quadro do Noise (tamanho 32) nunca parece um prefixo
    assert b"\x00\x20"[:2] != b"HX"


def test_pacote_do_eter():
    q = [b"quadro um", b"", bytes(300)]
    p = malha.pacote(b"HYXT", q)
    assert malha.ler_pacote(p) == {"magic": b"HYXT", "quadros": q}
    assert _recusada(malha.ler_pacote, p + b"\x00")
    assert _recusada(malha.ler_pacote, p[:-1])
    assert _recusada(malha.ler_pacote, b"HXP1HYXT" + codec.enc_u32(4097))


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
