# ✝ Gálatas 6:7 — “Porque tudo o que o homem semear, isso também ceifará.”
"""Melhoramento de culturas do ULTRAX: o modelo confere com a genética
quantitativa, e o motor é determinístico, recusa o que deve e pega adulteração.

Os testes científicos são estatísticos: rodam réplicas independentes (sementes
diferentes, mesma arquitetura) e comparam a média com a previsão da teoria
usando o intervalo de confiança calculado das próprias réplicas (erro padrão
= desvio / raiz de K). Ponto flutuante aqui só serve para a estatística; o
que o motor produz é inteiro.
"""

import math
import statistics
import sys
from functools import lru_cache
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import crypto  # noqa: E402
from hyurax import melhoramento as mel  # noqa: E402

Q16 = mel.Q16
ARQ = 11
# qtl, arquitetura, % selecionada, ruído, água, nitrogênio, solo, G×E
PADRAO = [50, ARQ, 20, 100, 100, 100, 100, 0]
REPLICAS = 40


def _semente(nome: bytes, k: int) -> bytes:
    return crypto.H(nome + k.to_bytes(4, "big"))


def _com(**mudancas) -> list[int]:
    p = mel.Parametros(*PADRAO)._replace(**mudancas)
    return list(p)


def _ic(valores):
    """Média e erro padrão da média."""
    return statistics.mean(valores), statistics.stdev(valores) / math.sqrt(len(valores))


@lru_cache(maxsize=None)
def _replicas(nome: bytes, tamanho: int, passos: int, parametros: tuple):
    return [mel.simular(tamanho, passos, list(parametros), _semente(nome, k)) for k in range(REPLICAS)]


# ---------------------------------------------------------------------------
# Ciência
# ---------------------------------------------------------------------------


def test_equacao_do_criador():
    """R = h² · S na primeira geração, com h² = Var(G)/Var(P) e S = diferencial
    de seleção medidos em cada réplica. Os QTL são aditivos e o ruído é
    independente do genótipo: a regressão do valor genético no fenótipo é h²,
    e os filhos herdam a média dos pais."""
    desvios, respostas, previstas = [], [], []
    for sim in _replicas(b"criador", 400, 1, tuple(PADRAO)):
        l0, l1 = sim["linhas"]
        s = l0["media_sel"] - l0["media_p"]
        h2 = l0["var_g"] / l0["var_p"]
        r = l1["media_g"] - l0["media_g"]
        assert s > 0 and 0.2 < h2 < 0.8, (s, h2)
        desvios.append(r - h2 * s)
        respostas.append(r)
        previstas.append(h2 * s)
    media, ep = _ic(desvios)
    razao = statistics.mean(respostas) / statistics.mean(previstas)
    # só ASCII no terminal: o console do Windows não garante UTF-8
    print(f"  R medido {statistics.mean(respostas):.0f}, h2*S {statistics.mean(previstas):.0f}, "
          f"R - h2*S = {media:.0f} +- {ep:.0f} (EP, {REPLICAS} replicas), R/(h2*S) = {razao:.3f}")
    # 4 erros padrão: 99,99 % de confiança; e a razão perto de 1
    assert abs(media) <= 4 * ep, f"R - h²S = {media:.0f} fora de ±4 EP ({ep:.0f})"
    assert 0.9 < razao < 1.1, razao


def test_ganho_positivo_com_selecao_e_nulo_sem():
    ganhos = []
    for k in range(10):
        sim = mel.simular(200, 5, _com(qtl=40), _semente(b"ganho", k))
        medias = [linha["media_g"] for linha in sim["linhas"]]
        assert medias[-1] > medias[0], medias
        ganhos.append(medias[-1] - medias[0])
    g, ep_g = _ic(ganhos)
    assert g - 4 * ep_g > 0, (g, ep_g)

    # 100 % selecionados: só deriva, o ganho esperado é zero
    sem = [s["linhas"][1]["media_g"] - s["linhas"][0]["media_g"]
           for s in _replicas(b"sem selecao", 400, 1, tuple(_com(selecionados=100)))]
    r0, ep0 = _ic(sem)
    com = [s["linhas"][1]["media_g"] - s["linhas"][0]["media_g"]
           for s in _replicas(b"criador", 400, 1, tuple(PADRAO))]
    print(f"  ganho em 5 geracoes {g:.0f} +- {ep_g:.0f}; 1 geracao com 20 %: {statistics.mean(com):.0f}; "
          f"com 100 %: {r0:.0f} +- {ep0:.0f}")
    assert abs(r0) <= 4 * ep0, f"sem seleção o ganho {r0:.0f} fica fora de ±4 EP ({ep0:.0f})"
    assert abs(r0) < 0.05 * statistics.mean(com)


def test_estresse_de_agua_reduz_pelo_fator_de_liebig():
    """Com G×E = 0, o fenótipo médio sob estresse é o sem estresse vezes F. As
    mesmas sementes dão os mesmos genótipos, então a comparação é por planta."""
    casos = (
        ((40, 100, 100), 32768, 0),       # água 40 % de 80 %: 1/2
        ((100, 20, 100), 28835, 1),       # nitrogênio: 30 % + 70 % · 0,2
        ((100, 100, 0), 26214, 2),        # solo pobre: o intercepto de 40 %
        ((40, 50, 100), 32768, 0),        # nitrogênio 0,65 não limita: vale a água
        ((150, 200, 120), Q16, 3),        # tudo no platô
    )
    for k in range(5):
        semente = _semente(b"liebig", k)
        livre = mel.simular(300, 1, PADRAO, semente)
        assert (livre["fator"], livre["limitante"]) == (Q16, 3)
        for (agua, nitro, solo), fator, limitante in casos:
            sob = mel.simular(300, 1, _com(agua=agua, nitrogenio=nitro, solo=solo), semente)
            assert (sob["fator"], sob["limitante"]) == (fator, limitante), (agua, nitro, solo)
            esperado = livre["linhas"][0]["media_p"] * fator / Q16
            assert abs(sob["linhas"][0]["media_p"] - esperado) <= 1e-4 * esperado, (sob["linhas"][0], esperado)
            esperado_g = livre["linhas"][0]["media_g"] * fator / Q16
            # o piso por planta e o piso da média: no máximo 2 unidades
            assert abs(sob["linhas"][0]["media_g"] - esperado_g) <= 2, (sob["linhas"][0], esperado_g)
    # a lei do mínimo: o recurso que não limita não muda nada, byte a byte
    semente = _semente(b"liebig", 0)
    so_agua = mel.executar(300, 2, _com(agua=40), semente)
    assert so_agua == mel.executar(300, 2, _com(agua=40, nitrogenio=50, solo=90), semente)
    assert so_agua != mel.executar(300, 2, _com(agua=40, nitrogenio=20), semente)
    # sem água nenhuma, nada cresce
    seca = mel.simular(50, 1, _com(agua=0), semente)
    assert seca["fator"] == 0 and all(linha["media_p"] == 0 for linha in seca["linhas"])


def test_variancia_genetica_cai_com_as_geracoes():
    """Efeito Bulmer na primeira geração: pelo modelo infinitesimal, a
    variância cai para 1 - h² k / 2, com k = i (i - x) do truncamento. Depois,
    a fixação de alelos consome o resto: com seleção e sem mutação, um QTL
    fixado não volta."""
    p = 0.2
    x = statistics.NormalDist().inv_cdf(1 - p)
    i = statistics.NormalDist().pdf(x) / p
    k_trunc = i * (i - x)
    razoes, previstas = [], []
    for sim in _replicas(b"criador", 400, 1, tuple(PADRAO)):
        l0, l1 = sim["linhas"]
        razoes.append(l1["var_g"] / l0["var_g"])
        previstas.append(1 - (l0["var_g"] / l0["var_p"]) * k_trunc / 2)
    r, ep = _ic(razoes)
    prevista = statistics.mean(previstas)
    sem = [s["linhas"][1]["var_g"] / s["linhas"][0]["var_g"]
           for s in _replicas(b"sem selecao", 400, 1, tuple(_com(selecionados=100)))]
    r0, ep0 = _ic(sem)
    print(f"  Var(G1)/Var(G0) com 20 %: {r:.3f} +- {ep:.3f} (Bulmer preve {prevista:.3f}); "
          f"com 100 %: {r0:.3f} +- {ep0:.3f}")
    # a previsão infinitesimal ignora deriva e mudança de frequência: folga de 0,05
    assert abs(r - prevista) <= 0.05 + 3 * ep, (r, prevista, ep)
    assert r + 4 * ep < r0 - 4 * ep0, "com seleção a variância cai mais que só pela deriva"

    finais = []
    for k in range(5):
        sim = mel.simular(200, 20, _com(qtl=40), _semente(b"bulmer", k))
        fixados = [linha["fixados_1"] + linha["fixados_0"] for linha in sim["linhas"]]
        assert fixados == sorted(fixados), f"QTL fixado voltou a segregar: {fixados}"
        assert fixados[-1] > fixados[0]
        variancias = [linha["var_g"] for linha in sim["linhas"]]
        assert variancias[-1] < variancias[0]
        finais.append(variancias[-1] / variancias[0])
        # a frequência final confere com a contagem de fixados
        f = sim["frequencias"]
        assert sum(1 for c in f if c in (0, 400)) == fixados[-1]
    print(f"  Var(G20)/Var(G0): {statistics.mean(finais):.3f} (5 replicas)")
    assert max(finais) < 0.5


def test_gxe_o_melhor_na_seca_nao_e_o_melhor_sem_estresse():
    """Com G×E, uma população selecionada na seca rende mais na seca que uma
    selecionada sem estresse, e o contrário no ambiente sem estresse. O valor
    esperado de uma população num ambiente sai das frequências finais:
    E[G] = (BASE + Σ e_i (2 p_i - 1)) · F."""
    qtl = 40
    arq = mel.arquitetura(ARQ, qtl)
    seca_fator, _ = mel.fator_ambiental(40, 100, 100)
    em_seca = mel.efeitos(arq, seca_fator, 100)
    sem_estresse = mel.efeitos(arq, Q16, 100)
    assert sem_estresse == [a for a, _ in arq]
    assert mel.efeitos(arq, seca_fator, 0) == [a for a, _ in arq]
    assert any(e < 0 for e in em_seca), "na seca algum alelo 1 passa a ser o pior"

    def esperado(freq, efs, fator, n):
        return (mel.BASE + sum(e * (c - n) / n for e, c in zip(efs, freq))) * fator / Q16

    for k in range(3):
        semente = _semente(b"gxe", k)
        na_seca = mel.simular(200, 15, _com(qtl=qtl, agua=40, gxe=100), semente)["frequencias"]
        no_bom = mel.simular(200, 15, _com(qtl=qtl, gxe=100), semente)["frequencias"]
        assert esperado(na_seca, em_seca, seca_fator, 200) > esperado(no_bom, em_seca, seca_fator, 200)
        assert esperado(no_bom, sem_estresse, Q16, 200) > esperado(na_seca, sem_estresse, Q16, 200)


# ---------------------------------------------------------------------------
# Motor
# ---------------------------------------------------------------------------


def test_determinismo_e_arquitetura_do_job():
    a = mel.executar(60, 3, PADRAO, b"unidade 1")
    assert a == mel.executar(60, 3, PADRAO, b"unidade 1")
    b = mel.executar(60, 3, PADRAO, b"unidade 2")
    assert a != b
    assert len(a) == mel.tamanho_do_resultado(3, PADRAO[0])
    # a arquitetura vem do parâmetro: as unidades de um JOB têm os mesmos efeitos
    da, db = mel.decodificar(a, 3, 50), mel.decodificar(b, 3, 50)
    assert (da["fator"], da["sigma"], da["selecionados"]) == (db["fator"], db["sigma"], db["selecionados"])
    assert mel.arquitetura(ARQ, 50) == mel.arquitetura(ARQ, 80)[:50]
    assert mel.arquitetura(ARQ, 50) != mel.arquitetura(ARQ + 1, 50)
    assert all(1 <= a_ <= 9999 and -9999 <= d <= 9999 for a_, d in mel.arquitetura(0xFFFF_FFFF, 200))
    # o gerador é o xoshiro256** de referência: estado (1, 2, 3, 4) dá 11520, 0, 1509978240
    g = mel.Gerador([1, 2, 3, 4])
    assert [g.proximo() for _ in range(3)] == [11520, 0, 1509978240]
    assert mel.Gerador([0, 0, 0, 0]).s == [1, 0, 0, 0]


def test_recusas():
    bons = (100, 10, PADRAO)
    mel.validar(*bons)
    ruins = [
        (3, 10, PADRAO), (20_001, 10, PADRAO), (100, 0, PADRAO), (100, 1001, PADRAO),
        (100, 10, PADRAO[:7]), (100, 10, PADRAO + [0]),
        (100, 10, _com(qtl=0)), (100, 10, _com(qtl=201)),
        (100, 10, _com(arquitetura=1 << 32)),
        (100, 10, _com(selecionados=0)), (100, 10, _com(selecionados=101)),
        (100, 10, _com(ruido=1001)), (100, 10, _com(agua=201)),
        (100, 10, _com(nitrogenio=201)), (100, 10, _com(solo=201)), (100, 10, _com(gxe=101)),
        # o teto de custo: 20.000 plantas, 1.000 gerações e 200 QTL
        (20_000, 1000, _com(qtl=200)),
    ]
    for tamanho, passos, parametros in ruins:
        try:
            mel.validar(tamanho, passos, parametros)
        except ValueError:
            continue
        raise AssertionError(f"aceitou {tamanho}, {passos}, {parametros}")
    assert mel.operacoes(20_000, 1000, _com(qtl=200)) > mel.TETO_OPERACOES
    assert mel.operacoes(20_000, 100, _com(qtl=200)) <= mel.TETO_OPERACOES
    mel.validar(20_000, 100, _com(qtl=200))


def test_verificacao_pega_adulteracao():
    semente = b"adulterar"
    resultado = mel.executar(40, 4, PADRAO, semente)
    assert mel.verificar(40, 4, PADRAO, semente, resultado) is None
    # um byte da média do valor genético da geração 2
    errado = bytearray(resultado)
    errado[mel.CABECALHO_BYTES + 2 * mel.LINHA_BYTES + 7] ^= 1
    assert "geração 2" in mel.verificar(40, 4, PADRAO, semente, bytes(errado))
    # a frequência final do QTL 3
    errado = bytearray(resultado)
    errado[-4 * 50 + 3 * 4 + 3] ^= 1
    assert "QTL 3" in mel.verificar(40, 4, PADRAO, semente, bytes(errado))
    # o desvio do ruído no cabeçalho
    errado = bytearray(resultado)
    errado[12] ^= 1
    assert "ruído" in mel.verificar(40, 4, PADRAO, semente, bytes(errado))
    assert "tamanho" in mel.verificar(40, 4, PADRAO, semente, resultado[:-1])
    assert mel.verificar(40, 4, PADRAO, b"outra semente", resultado) is not None


def test_cancelamento_e_contagem_de_operacoes():
    parametros = _com(qtl=30)
    total = mel.operacoes(2500, 3, parametros)
    chamadas = []
    mel.simular(2500, 3, parametros, b"ops", lambda ops: chamadas.append(ops) or True)
    assert sum(chamadas) == total
    # pedaços pequenos: 1024 plantas no máximo por chamada
    assert max(chamadas) <= max(1024 * (2 * 30 + 2), 2500 * (2500 - 1).bit_length())

    feitas = []

    def parar(ops):
        feitas.append(ops)
        return len(feitas) < 5

    try:
        mel.simular(2500, 3, parametros, b"ops", parar)
    except mel.Cancelado:
        assert len(feitas) == 5
    else:
        raise AssertionError("não cancelou")

    # a última chamada conta, mas não cancela: o trabalho já acabou
    n = len(chamadas)
    contador = []

    def so_a_ultima(ops):
        contador.append(ops)
        return len(contador) < n

    sim = mel.simular(2500, 3, parametros, b"ops", so_a_ultima)
    assert mel.codificar(sim) == mel.executar(2500, 3, parametros, b"ops")


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
