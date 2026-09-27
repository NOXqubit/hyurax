# ✝ Gênesis 1:24 — “Produza a terra alma vivente conforme a sua espécie.”
"""Genética de populações do ULTRAX: a especificação ao pé da letra, as
recusas e, principalmente, a ciência. O modelo precisa reproduzir o que a
teoria de Wright-Fisher diz, e não só repetir a si mesmo.

Os testes estatísticos usam sementes fixas (então o resultado é
determinístico) e tolerância de Z = 4 erros-padrão, com o erro-padrão
calculado dos próprios dados ou da teoria exata do modelo discreto. Para um
modelo certo, a chance de uma comparação cair fora é de 6,3·10^-5; a
tolerância não foi ajustada para passar. Cada locus é uma réplica
independente (sorteios disjuntos do mesmo fluxo), e as sementes somam réplicas.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import crypto  # noqa: E402
from hyurax import genetica as gen  # noqa: E402

Z = 4.0
NEUTRO = gen.SELECAO_ZERO
SEMENTE = crypto.H(b"teste genetica")


def _replicas(tamanho, passos, parametros, sementes, rotulo):
    """Trajetória [k_0, k_1, ..., k_G] de cada locus de cada execução."""
    loci = parametros[0]
    saida = []
    for i in range(sementes):
        r = gen.executar(tamanho, passos, parametros, crypto.H(rotulo + i.to_bytes(4, "big")))
        t = gen.decodificar(r)
        for l in range(loci):
            saida.append([t["inicial"]] + t["contagens"][l::loci])
    return saida


def _media_e_dp(xs):
    n = len(xs)
    m = sum(xs) / n
    return m, math.sqrt(sum((x - m) ** 2 for x in xs) / (n - 1))


# ---------------------------------------------------------------------------
# A especificação
# ---------------------------------------------------------------------------

def test_splitmix64_e_o_de_sempre():
    # os primeiros valores do SplitMix64 (Steele, Lea e Flood, 2014) com estado 0,
    # os mesmos que o gerador de semente do xoshiro publica
    assert [gen.splitmix64(0, n) for n in range(4)] == [
        0xE220A8397B1DCDAF, 0x6E789E6AA1B965F4, 0x06C45D188009454F, 0xF88BB8A8724C81EC,
    ]


def test_execucao_rapida_e_a_especificacao_ao_pe_da_letra():
    # a vetorizada pula loci fixados e sorteia em bloco; a de referência faz
    # cada sorteio, um por um. As duas precisam dar os mesmos bytes.
    casos = [
        (5, 6, [3, 100, 10_500, 50, 3000]),
        (20, 15, [4, 0, NEUTRO, 0, 5000]),
        (3, 10, [2, 0, 0, 100, 10_000]),       # AA letal com a população toda AA
        (7, 9, [5, 10_000, 20_000, 100, 0]),   # mutação e seleção no máximo
        (2, 1, [1, 0, NEUTRO, 50, 5000]),      # o menor caso
        (4, 40, [6, 0, NEUTRO, 50, 2500]),     # fixa no meio do caminho
    ]
    for tamanho, passos, parametros in casos:
        rapido = gen.executar(tamanho, passos, parametros, SEMENTE)
        assert rapido == gen.executar_referencia(tamanho, passos, parametros, SEMENTE), (tamanho, passos, parametros)
        assert len(rapido) == gen.CABECALHO + 4 * passos * parametros[0]


def test_aptidoes_e_contagem_inicial():
    # s = 0: todos iguais; s = +1, h = 50: AA = 2, Aa = 1,5, aa = 1
    assert gen.aptidoes(NEUTRO, 37) == (10**6, 10**6, 10**6)
    assert gen.aptidoes(20_000, 50) == (10**6, 1_500_000, 2_000_000)
    assert gen.aptidoes(0, 100) == (10**6, 0, 0)      # A dominante e letal
    assert gen.aptidoes(0, 0) == (10**6, 10**6, 0)    # A recessivo e letal
    assert gen.contagem_inicial(8, 2500) == 4
    assert gen.contagem_inicial(10, 2250) == 5        # 4,5 arredonda para cima
    assert gen.contagem_inicial(10, 2249) == 4
    assert gen.contagem_inicial(100_000, 10_000) == 200_000


def test_limiar_nas_bordas():
    neutro = gen.aptidoes(NEUTRO, 50)
    q32 = gen.Q32
    assert gen.limiar(0, 16, neutro, 0) == 0            # perdido: nunca sorteia A
    assert gen.limiar(16, 16, neutro, 0) == q32         # fixado: sempre sorteia A
    assert gen.limiar(4, 16, neutro, 0) == q32 // 4     # neutro: p exato
    assert gen.limiar(1, 3, neutro, 0) == (q32 + 1) // 3  # 1/3 com a metade para cima
    # só mutação: de p = 0, a frequência vira µ
    assert gen.limiar(0, 10, neutro, 10_000) == (q32 * 10_000 + 500_000) // 10**6
    # AA letal com a população toda AA: den = 0, a seleção não mexe
    letal = gen.aptidoes(0, 50)
    assert gen.limiar(10, 10, letal, 0) == q32
    assert gen.limiar(10, 10, letal, 100) == (q32 * (10**6 - 100) + 500_000) // 10**6
    # e, com um alelo a só, o letal some numa geração: p' = k·j·w1 / (2·k·j·w1 + j²·w0)
    k, j = 9, 1
    w0, w1, _ = letal
    assert gen.limiar(k, 10, letal, 0) == (k * j * w1 * q32 + (2 * k * j * w1 + j * j * w0) // 2) // (2 * k * j * w1 + j * j * w0)


def test_deterministico_e_a_semente_importa():
    parametros = [8, 100, 10_500, 50, 5000]
    a = gen.executar(40, 30, parametros, SEMENTE)
    assert a == gen.executar(40, 30, parametros, SEMENTE)
    assert a != gen.executar(40, 30, parametros, crypto.H(b"outra"))
    assert gen.verificar(40, 30, parametros, SEMENTE, a) == (True, "")


def test_parametros_recusados():
    bom = [4, 0, NEUTRO, 50, 5000]
    recusados = [
        (1, 10, bom), (100_001, 10, bom), (10, 0, bom), (10, 10_001, bom),
        (10, 10, [0, 0, NEUTRO, 50, 5000]), (10, 10, [65, 0, NEUTRO, 50, 5000]),
        (10, 10, [4, 10_001, NEUTRO, 50, 5000]), (10, 10, [4, 0, 20_001, 50, 5000]),
        (10, 10, [4, 0, NEUTRO, 101, 5000]), (10, 10, [4, 0, NEUTRO, 50, 10_001]),
        (10, 10, bom[:4]), (10, 10, bom + [0]),
        (10, 10, [4, 0, NEUTRO, 50, 0.5]), (10, 10, [True, 0, NEUTRO, 50, 5000]),
        # custo: 2·100000·64·672 = 8 601 600 000 > 2^33 = 8 589 934 592
        (100_000, 672, [64, 0, NEUTRO, 50, 5000]),
    ]
    for tamanho, passos, parametros in recusados:
        try:
            gen.validar(tamanho, passos, parametros)
        except ValueError:
            continue
        raise AssertionError(f"aceitou {tamanho}, {passos}, {parametros}")
    # e o que fica no limite passa: 2·100000·64·671 < 2^33
    gen.validar(100_000, 671, [64, 10_000, 20_000, 100, 10_000])
    gen.validar(2, 1, [1, 0, 0, 0, 0])


def test_resultado_adulterado_e_recusado():
    parametros = [3, 50, 11_000, 25, 4000]
    bom = gen.executar(30, 12, parametros, SEMENTE)
    assert gen.verificar(30, 12, parametros, SEMENTE, bom)[0]

    errado = bytearray(bom)
    i = 7 * 3 + 2  # geração 8, locus 2
    errado[gen.CABECALHO + 4 * i + 3] ^= 1
    ok, motivo = gen.verificar(30, 12, parametros, SEMENTE, bytes(errado))
    assert not ok and "geração 8, locus 2" in motivo, motivo

    ok, motivo = gen.verificar(30, 12, parametros, SEMENTE, bom[:-4])
    assert not ok and "tamanho" in motivo

    cabeca = bytearray(bom)
    cabeca[13] ^= 1  # k0
    ok, motivo = gen.verificar(30, 12, parametros, SEMENTE, bytes(cabeca))
    assert not ok and "cabeçalho" in motivo

    assert not gen.verificar(30, 12, parametros, crypto.H(b"outra"), bom)[0]
    # outra seleção. (Mudar h de 25 para 26 pode não mudar nenhuma contagem: o
    # limiar anda pouco e nenhum sorteio cai no meio. O resultado continua
    # sendo o daquela especificação, então aceitar ali não é erro.)
    assert not gen.verificar(30, 12, [3, 50, 19_000, 25, 4000], SEMENTE, bom)[0]
    # contagem acima de 2N nem se lê como trajetória
    impossivel = bytearray(bom)
    impossivel[gen.CABECALHO:gen.CABECALHO + 4] = (61).to_bytes(4, "big")
    assert gen.decodificar(bytes(impossivel)) is None


def test_cancelamento():
    parametros = [4, 0, NEUTRO, 50, 5000]
    feitas = []

    def para_na_terceira(ops):
        feitas.append(ops)
        return len(feitas) < 3

    assert gen.executar(10, 8, parametros, SEMENTE, para_na_terceira) is None
    assert feitas == [2 * 10 * 4] * 3

    # parar depois da última geração não desfaz o trabalho
    chamadas = []

    def para_no_fim(ops):
        chamadas.append(ops)
        return len(chamadas) < 8

    r = gen.executar(10, 8, parametros, SEMENTE, para_no_fim)
    assert r == gen.executar(10, 8, parametros, SEMENTE)
    assert sum(chamadas) == gen.operacoes(10, 8, parametros)


def test_estatisticas_batem_com_a_trajetoria():
    parametros = [5, 200, 10_300, 60, 3000]
    r = gen.executar(12, 20, parametros, SEMENTE)
    t = gen.decodificar(r)
    e = gen.estatisticas(r)
    assert len(e["soma_por_geracao"]) == 21
    assert e["soma_por_geracao"][0] == 5 * t["inicial"]
    finais = t["contagens"][-5:]
    assert e["soma_por_geracao"][-1] == sum(finais)
    assert e["soma_quadrados_por_geracao"][-1] == sum(k * k for k in finais)
    assert e["fixados"] == sum(k == 24 for k in finais)
    assert e["perdidos"] == sum(k == 0 for k in finais)
    assert gen.estatisticas(r[:-1]) is None


# ---------------------------------------------------------------------------
# A ciência
# ---------------------------------------------------------------------------

def test_neutro_a_fracao_que_fixa_tende_a_p0():
    """Sem seleção nem mutação, p é martingale: a chance de fixar A é a
    frequência inicial. Com 2N = 16 e 200 gerações, a chance de um locus
    ainda não ter fixado é da ordem de (15/16)^200 ≈ 2·10^-6."""
    n, g = 8, 200
    m = 2 * n
    for p0 in (2500, 6250):
        k0 = gen.contagem_inicial(n, p0)
        p = k0 / m
        finais = [r[-1] for r in _replicas(n, g, [64, 0, NEUTRO, 50, p0], 24, b"neutro" + p0.to_bytes(2, "big"))]
        assert all(k in (0, m) for k in finais), "todos os loci precisam ter fixado ou perdido A"
        fracao = sum(k == m for k in finais) / len(finais)
        erro_padrao = math.sqrt(p * (1 - p) / len(finais))
        assert abs(fracao - p) <= Z * erro_padrao, (p0, fracao, p, erro_padrao)


def test_heterozigosidade_decai_como_a_teoria():
    """E[2p(1-p)] cai exatamente por (1 - 1/(2N)) a cada geração na deriva
    pura: H_t = H_0·(1 - 1/(2N))^t."""
    n, g = 20, 40
    m = 2 * n
    reps = _replicas(n, g, [64, 0, NEUTRO, 50, 5000], 24, b"heterozigosidade")
    h0 = 2 * (reps[0][0] / m) * (1 - reps[0][0] / m)
    medias = {}
    for t in (5, 10, 20, 40):
        hs = [2 * (r[t] / m) * (1 - r[t] / m) for r in reps]
        media, dp = _media_e_dp(hs)
        teoria = h0 * (1 - 1 / m) ** t
        assert abs(media - teoria) <= Z * dp / math.sqrt(len(hs)), (t, media, teoria)
        medias[t] = media
    assert medias[40] < medias[20] < medias[10] < medias[5] < h0


def test_selecao_positiva_forte_aumenta_p_em_media():
    """s = +0,2 aditivo, de p0 = 0,1: a média sobe. E o espelho: s = -0,2 de
    p0 = 0,9 desce. A comparação é com p0, que é a média exata sem seleção."""
    n, g = 50, 30
    m = 2 * n
    sobe = [r[-1] / m for r in _replicas(n, g, [64, 0, 12_000, 50, 1000], 8, b"selecao+")]
    media, dp = _media_e_dp(sobe)
    assert media - Z * dp / math.sqrt(len(sobe)) > 0.1, (media, dp)

    desce = [r[-1] / m for r in _replicas(n, g, [64, 0, 8_000, 50, 9000], 8, b"selecao-")]
    media, dp = _media_e_dp(desce)
    assert media + Z * dp / math.sqrt(len(desce)) < 0.9, (media, dp)


def test_dominancia_alelo_raro_dominante_sobe_mais_rapido():
    """Com A raro quase só existe em heterozigoto: se A é dominante (h = 1) a
    seleção o enxerga; se é recessivo (h = 0), quase não."""
    n, g = 50, 10
    m = 2 * n
    dominante = [r[-1] / m for r in _replicas(n, g, [64, 0, 12_000, 100, 1000], 8, b"dominante")]
    recessivo = [r[-1] / m for r in _replicas(n, g, [64, 0, 12_000, 0, 1000], 8, b"recessivo")]
    md, dd = _media_e_dp(dominante)
    mr, dr = _media_e_dp(recessivo)
    erro = math.sqrt(dd ** 2 / len(dominante) + dr ** 2 / len(recessivo))
    assert md - mr > Z * erro, (md, mr, erro)


def _momentos_mutacao(p0, u, m, t):
    """E[p_t] e Var[p_t] exatos do modelo discreto com mutação simétrica u e
    amostragem binomial de m alelos: a mutação é linear em p e, depois dela,
    E[p''²] = (1 - 1/m)·E[p'²] + E[p']/m."""
    m1, m2 = p0, p0 * p0
    for _ in range(t):
        m2 = (1 - 2 * u) ** 2 * m2 + 2 * u * (1 - 2 * u) * m1 + u * u
        m1 = (1 - 2 * u) * m1 + u
        m2 = (1 - 1 / m) * m2 + m1 / m
    return m1, m2 - m1 * m1


def _confere_momentos(ps, esperado, variancia):
    """Média pela variância exata; variância amostral pelo quarto momento
    amostral (Var(s²) ≈ (µ4 - σ⁴)/R, sem supor normalidade)."""
    r = len(ps)
    media = sum(ps) / r
    assert abs(media - esperado) <= Z * math.sqrt(variancia / r), (media, esperado)
    s2 = sum((p - media) ** 2 for p in ps) / (r - 1)
    m4 = sum((p - media) ** 4 for p in ps) / r
    assert abs(s2 - variancia) <= Z * math.sqrt(max(m4 - s2 * s2, 0.0) / r), (s2, variancia)
    return media, s2


def test_so_mutacao_simetrica_leva_p_a_meio():
    """Sem seleção, µ = 1% simétrico, de p0 = 0 e de p0 = 1: a média vai a
    1/2 como 1/2·(1 - (1 - 2µ)^t), dos dois lados. Com N grande (4Nµ = 20)
    as réplicas ficam juntas perto de 1/2; com N pequeno (4Nµ = 0,4) a média
    é a mesma, mas cada réplica passa o tempo perto de 0 ou de 1."""
    u, g = 0.01, 200
    n = 500
    m = 2 * n
    subida = [r[-1] / m for r in _replicas(n, g, [64, 10_000, NEUTRO, 50, 0], 2, b"mutacao0")]
    esperado, variancia = _momentos_mutacao(0.0, u, m, g)
    assert abs(esperado - 0.5) <= 0.5 * (1 - 2 * u) ** g + 1e-12
    media, _ = _confere_momentos(subida, esperado, variancia)
    assert abs(media - 0.5) <= 0.5 * (1 - 2 * u) ** g + Z * math.sqrt(variancia / len(subida))
    assert variancia < 0.01, "com N grande as réplicas ficam juntas"

    descida = [r[-1] / m for r in _replicas(n, g, [64, 10_000, NEUTRO, 50, 10_000], 1, b"mutacao1")]
    esperado1, variancia1 = _momentos_mutacao(1.0, u, m, g)
    assert abs(esperado1 - (1 - esperado)) < 1e-12
    _confere_momentos(descida, esperado1, variancia1)

    # N pequeno: mesma média, variância muito maior
    pequeno = 10
    ps = [r[-1] / (2 * pequeno) for r in _replicas(pequeno, g, [64, 10_000, NEUTRO, 50, 0], 4, b"mutacao-pequeno")]
    esperado_p, variancia_p = _momentos_mutacao(0.0, u, 2 * pequeno, g)
    _confere_momentos(ps, esperado_p, variancia_p)
    assert variancia_p > 10 * variancia


def test_sem_mutacao_perdido_e_fixado_nao_voltam():
    for p0, k in ((0, 0), (10_000, 60)):
        r = gen.decodificar(gen.executar(30, 50, [8, 0, 15_000, 50, p0], SEMENTE))
        assert set(r["contagens"]) == {k}


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
