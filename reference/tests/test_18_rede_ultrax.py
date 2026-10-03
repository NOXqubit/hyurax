# ✝ Eclesiastes 4:9 — “Melhor é serem dois do que um, porque têm melhor paga do seu trabalho.”
"""Mensagens do ULTRAX entre nós: ida e volta, oferta assinada e recusas."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import codec, crypto, rede_ultrax  # noqa: E402


def _recusada(dados: bytes) -> bool:
    try:
        rede_ultrax.ler(dados)
    except (ValueError, codec.CodecError):
        return True
    return False


def test_oferta_assinada_e_forjada():
    corpo = rede_ultrax.oferta(bytes(range(32)), 0xFF, 2, 256, 10)
    assert rede_ultrax.ler(corpo)["assinatura_confere"]
    forjada = bytearray(corpo)
    forjada[2 + 32] ^= 1  # tipos
    assert not rede_ultrax.ler(bytes(forjada))["assinatura_confere"]


def test_pedido_vai_e_volta():
    esp = rede_ultrax.especificacao(7, 200, 1000, [11])
    corpo = rede_ultrax.pedido(9, crypto.H(b"j"), 4, esp, crypto.H(b"s"), 99, True)
    m = rede_ultrax.ler(corpo)
    assert (m["pedido"], m["indice"], m["tipo"], m["tamanho"], m["passos"], m["parametros"], m["com_compromisso"]) == (
        9, 4, 7, 200, 1000, [11], True)


def test_corpos_ruins_sao_recusados():
    bom = rede_ultrax.revelar(3)
    for ruim in (bom + b"\x00", bom[:-1], bytes([1]) + bom[1:], bytes([3]) + bom[1:], bytes([2, 99]), b""):
        assert _recusada(ruim), ruim.hex()
    try:
        rede_ultrax.resultado(1, b"", bytes(64), bytes(rede_ultrax.RESULTADO_MAX + 1))
    except ValueError:
        pass
    else:
        raise AssertionError("resultado acima do teto foi aceito")


def test_compromisso_amarra_worker():
    r = crypto.H(b"x")
    assert rede_ultrax.compromisso_de(r, bytes(32)) != rede_ultrax.compromisso_de(r, bytes([1]) + bytes(31))


def test_compromisso_assinado_vale_so_para_a_unidade_do_pedido():
    segredo = bytes(range(5, 37))
    worker = crypto.ed25519_public_key(segredo)
    valor = rede_ultrax.compromisso_de(crypto.H(b"r"), worker)
    m = rede_ultrax.ler(rede_ultrax.compromisso(segredo, crypto.H(b"j"), 4, 9, valor))
    assert (m["pedido"], m["worker"], m["compromisso"]) == (9, worker, valor)
    assert rede_ultrax.compromisso_confere(m, crypto.H(b"j"), 4)
    assert not rede_ultrax.compromisso_confere(m, crypto.H(b"j"), 5)
    assert not rede_ultrax.compromisso_confere(m, crypto.H(b"k"), 4)
    outro = dict(m, compromisso=rede_ultrax.compromisso_de(crypto.H(b"s"), worker))
    assert not rede_ultrax.compromisso_confere(outro, crypto.H(b"j"), 4)


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
