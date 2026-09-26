# ✝ Provérbios 1:5 — “Ouça o sábio e cresça em conhecimento.”
"""Treino de IA do ULTRAX: determinismo, aprendizado e codificação."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import crypto, ia  # noqa: E402


def test_base_tem_2048_moleculas():
    base = ia.carregar()
    treino, validacao = ia.treino_e_validacao(base)
    assert len(base) == 2048
    assert (len(treino), len(validacao)) == (1638, 410)


def test_treino_e_deterministico_e_aprende():
    base = ia.carregar()
    semente = crypto.H(b"teste ia")
    a = ia.treinar(semente, 16, 60, base)
    b = ia.treinar(semente, 16, 60, base)
    assert ia.codificar(a[0], a[1]) == ia.codificar(b[0], b[1])
    inicial = ia.erro_de_validacao(ia.pesos_iniciais(semente), ia.treino_e_validacao(base)[1])
    assert a[1] < inicial, "o erro de validação precisa cair"


def test_faixas():
    for lote, passos in ((0, 1), (257, 1), (1, 0), (1, 4097)):
        try:
            ia.treinar(b"x", lote, passos)
        except ValueError:
            continue
        raise AssertionError(f"aceitou lote {lote}, passos {passos}")


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
