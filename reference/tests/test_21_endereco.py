# ✝ Provérbios 22:28 — “Não removas os limites antigos que fizeram teus pais.”
"""Endereço Bech32m: BIP-350, ida e volta, rede no prefixo e erro de digitação."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import endereco  # noqa: E402

VETORES = Path(__file__).resolve().parents[2] / "vectors" / "enderecos.json"


def _recusa(f, *args) -> bool:
    try:
        f(*args)
    except ValueError:
        return True
    return False


def test_vetores_validos_do_bip350():
    for v in ["A1LQFN3A", "a1lqfn3a", "abcdef1l7aum6echk45nj3s0wdvt2fg8x9yrzpqzd3ryx",
              "split1checkupstagehandshakeupstreamerranterredcaperredlc445v", "?1v759aa"]:
        endereco.ler_bech32m(v)
    for v in ["abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw", "a1lqfn3b", "A1lqfn3a"]:
        assert _recusa(endereco.ler_bech32m, v), v


def test_ida_e_volta_nas_tres_redes():
    e = bytes([0xC0, 0xDE, 0x48, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0x1B, 0x0E])
    for rede in ("hyurax-mainnet", "hyurax-testnet", "hyurax-regtest"):
        texto = endereco.mostrar(e, rede)
        assert texto.startswith(endereco.prefixo(rede) + "1")
        assert endereco.ler(texto, rede) == e
        assert endereco.ler(texto.upper(), rede) == e
    assert len(endereco.mostrar(e, "hyurax-testnet")) == 4 + 1 + 32 + 6


def test_qualquer_caractere_trocado_e_pego():
    e = bytes([7] * 20)
    certo = endereco.mostrar(e, "testnet")
    for i in range(5, len(certo)):
        errado = certo[:i] + ("p" if certo[i] == "q" else "q") + certo[i + 1:]
        assert _recusa(endereco.ler, errado, "testnet"), errado


def test_vetores_batem_com_o_gerador():
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
    import gen_vectors  # noqa: E402

    gravado = json.loads(VETORES.read_text(encoding="utf-8"))["data"]
    assert gravado == json.loads(json.dumps(gen_vectors.vec_enderecos()))
