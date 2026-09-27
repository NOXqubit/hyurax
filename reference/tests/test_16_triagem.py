# ✝ Provérbios 25:2 — “A glória dos reis é investigar a coisa.”
"""Triagem de moléculas reais do ULTRAX: catálogo, filtros, previsão,
faixas, recusas e o erro do modelo de referência."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import ia, triagem  # noqa: E402

N = 8289
LIPINSKI_VEBER = [0, 0, 500_000, 25_000, 5, 10, 10, 140_000, 16_000]  # log S previsto >= -4


def _parametros(inicio: int, base=LIPINSKI_VEBER) -> list[int]:
    p = list(base)
    p[1] = inicio
    return p


def _colunas_do_arquivo() -> list[dict[str, str]]:
    """O catálogo lido de novo, direto do arquivo e pelos nomes do cabeçalho,
    sem passar por `triagem.carregar_catalogo`."""
    nomes, linhas = None, []
    for linha in triagem.CATALOGO_ARQUIVO.read_text(encoding="utf-8").splitlines():
        c = linha.split("\t")
        if c[0] == "#colunas":
            nomes = c[1:]
        elif not linha.startswith("#") and linha:
            linhas.append(dict(zip(nomes, c)))
    return linhas


def test_catalogo_e_as_2048_da_ia():
    cat = triagem.carregar_catalogo()
    assert len(cat.moleculas) == N
    base = ia.carregar()
    assert len(base) == 2048
    # as 2048 primeiras do catálogo são a base da IA, com as mesmas features
    for m, (x, _) in zip(cat.moleculas, base):
        assert list(m.x) == x
    cabecalho = [l for l in ia.ARQUIVO.read_text(encoding="utf-8").splitlines() if l.startswith("#normalizacao")]
    assert cabecalho == [f"#normalizacao\t{cat.media_mili}\t{cat.desvio_mili}"]
    assert all(m.smiles and m.formula for m in cat.moleculas)
    assert all(abs(v) <= 8 * ia.ESCALA for m in cat.moleculas for v in m.x)


def test_filtros_batem_com_os_valores_brutos():
    brutas = _colunas_do_arquivo()
    # limites de Lipinski e Veber, e outros apertados em cima do valor de uma molécula
    m7 = brutas[7]
    apertado = [0, 0, int(m7["massa_mili"]), int(m7["logp_mili"]) + 20_000, int(m7["doadores"]),
                int(m7["aceptores"]), int(m7["rotaveis"]), int(m7["tpsa_mili"]), 20_000]
    for limites in (LIPINSKI_VEBER, apertado):
        inicio, tamanho = 0, 300
        resultado, _ = triagem.executar(tamanho, 0, _parametros(inicio, limites))
        _, quantidade, aprovadas, linhas = triagem.decodificar(resultado)
        assert quantidade == tamanho
        contagem = 0
        for k, (mascara, previsto, nota) in enumerate(linhas):
            b = brutas[inicio + k]
            # a conferência independente, pelos valores do arquivo em unidades de verdade;
            # o log S previsto tem teste próprio, abaixo
            esperado = (
                float(b["massa_mili"]) / 1000 <= limites[2] / 1000,
                float(b["logp_mili"]) / 1000 <= (limites[3] - 20_000) / 1000,
                int(b["doadores"]) <= limites[4],
                int(b["aceptores"]) <= limites[5],
                int(b["rotaveis"]) <= limites[6],
                float(b["tpsa_mili"]) / 1000 <= limites[7] / 1000,
                previsto >= limites[8] - 20_000,
            )
            assert mascara == sum(1 << i for i, ok in enumerate(esperado) if ok), f"molécula {k}: {b['id']}"
            assert nota == sum(esperado) * 100_000 + max(0, min(99_999, previsto + 50_000))
            contagem += all(esperado)
        assert aprovadas == contagem
    # com os limites em cima da molécula 7, ela passa nos seis filtros de descritor (igualdade passa)
    resultado, _ = triagem.executar(1, 0, _parametros(7, apertado))
    assert triagem.decodificar(resultado)[3][0][0] & 0b0111111 == 0b0111111


def test_previsao_igual_a_inferencia():
    modelo = triagem.carregar_modelo()
    cat = triagem.carregar_catalogo()
    inicio, tamanho = 2000, 200  # atravessa o fim da base da IA (2048)
    resultado, _ = triagem.executar(tamanho, 0, _parametros(inicio))
    for k, (_, previsto, _) in enumerate(triagem.decodificar(resultado)[3]):
        y, _ = ia.prever(modelo.pesos, list(cat.moleculas[inicio + k].x))
        # y * desvio < 2^53: a conta em ponto flutuante é exata aqui, e int() trunca para o zero
        assert previsto == int(y * cat.desvio_mili / ia.ESCALA) + cat.media_mili


def test_modelo_de_referencia_confere():
    modelo = triagem.carregar_modelo()
    _, validacao = ia.treino_e_validacao(ia.carregar())
    assert ia.erro_de_validacao(modelo.pesos, validacao) == modelo.erro_validacao
    assert 1 <= modelo.lote <= ia.LOTE_MAX and 1 <= modelo.passos <= ia.PASSOS_MAX
    texto = triagem.MODELO_ARQUIVO.read_text(encoding="utf-8")
    comentarios = [l[2:] if l.startswith("# ") else "" for l in texto.splitlines() if l.startswith("# ") or l == "#"]
    refeito = triagem.escrever_modelo(modelo.pesos, modelo.semente, modelo.lote, modelo.passos,
                                      modelo.erro_validacao, comentarios)
    assert refeito == texto, "escrever_modelo(ler_modelo(x)) == x"


def test_faixa_na_borda_do_catalogo():
    for inicio, tamanho in ((N - 1, 1), (N - 37, 37), (N - 4096, 4096)):
        resultado, ops = triagem.executar(tamanho, 0, _parametros(inicio))
        assert len(resultado) == 12 + 9 * tamanho
        assert ops == tamanho * 183
        assert triagem.decodificar(resultado)[:2] == (inicio, tamanho)
    for inicio, tamanho in ((N, 1), (N - 36, 37), (N - 4095, 4096), (2**32 - 1, 1)):
        try:
            triagem.validar(tamanho, 0, _parametros(inicio))
        except ValueError:
            continue
        raise AssertionError(f"aceitou a faixa [{inicio}, {inicio + tamanho})")


def test_unidades_de_um_job():
    # modelo com 1000 moléculas por unidade, começando em 5000
    base = _parametros(5000)
    assert triagem.derivar_unidade(1000, base, 0, N) == (1000, base)
    tamanho, p = triagem.derivar_unidade(1000, base, 3, N)
    assert (tamanho, p[1]) == (289, 8000), "a última unidade é cortada no fim do catálogo"
    try:
        triagem.derivar_unidade(1000, base, 4, N)
    except ValueError:
        pass
    else:
        raise AssertionError("unidade depois do fim do catálogo")


def test_parametros_recusados():
    ruins = []
    for k, valor in ((0, 1), (0, 7), (2, 2_000_001), (3, 40_001), (4, 101), (5, 101), (6, 101),
                     (7, 1_000_001), (8, 40_001)):
        p = list(LIPINSKI_VEBER)
        p[k] = valor
        ruins.append((10, 0, p))
    ruins += [(0, 0, LIPINSKI_VEBER), (4097, 0, LIPINSKI_VEBER), (10, 1, LIPINSKI_VEBER),
              (10, 0, LIPINSKI_VEBER[:8]), (10, 0, LIPINSKI_VEBER + [0])]
    for tamanho, passos, p in ruins:
        try:
            triagem.validar(tamanho, passos, p)
        except ValueError:
            continue
        raise AssertionError(f"aceitou tamanho {tamanho}, passos {passos}, parâmetros {p}")
    # os tetos em si são aceitos
    triagem.validar(4096, 0, [0, 0, 2_000_000, 40_000, 100, 100, 100, 1_000_000, 40_000])


def test_deterministico_e_sem_semente():
    p = _parametros(123)
    a, _ = triagem.executar(64, 0, p, b"uma semente")
    b, _ = triagem.executar(64, 0, p, b"outra semente")
    assert a == b


def test_adulteracao_recusada():
    p = _parametros(40)
    resultado, _ = triagem.executar(50, 0, p)
    assert triagem.verificar(50, 0, p, resultado) is None
    for posicao, motivo in ((12 + 9 * 3, "43"), (12 + 9 * 10 + 2, "50"), (12 + 9 * 49 + 8, "89"), (11, "cabeçalho")):
        errado = bytearray(resultado)
        errado[posicao] ^= 1
        recusa = triagem.verificar(50, 0, p, bytes(errado))
        assert recusa is not None and motivo in recusa, recusa
    assert "tamanho" in triagem.verificar(50, 0, p, resultado[:-1])
    # o resultado de outra faixa não serve
    assert triagem.verificar(50, 0, _parametros(41), resultado) is not None


def test_erro_do_modelo_contra_o_medido():
    """Só o reporte: a raiz do erro quadrático médio do log S previsto contra o
    medido, no catálogo inteiro e nas moléculas que o treino nunca viu."""
    modelo = triagem.carregar_modelo()
    cat = triagem.carregar_catalogo()
    pares = []
    for m in cat.moleculas:
        y, _ = ia.prever(modelo.pesos, list(m.x))
        pares.append((triagem.logs_de(y, cat.media_mili, cat.desvio_mili), m.logs_mili))
    assert len(pares) == N
    todos, fora = triagem.rmse_mili(pares), triagem.rmse_mili(pares[2048:])
    print(f"  erro do modelo de referência (raiz do EQM, log S): catálogo {todos / 1000:.3f}, "
          f"fora da base da IA ({N - 2048}) {fora / 1000:.3f}")


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
