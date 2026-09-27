# ✝ Gênesis 1:24 — “Produza a terra alma vivente conforme a sua espécie.”
"""HYURAX — genética de populações do ULTRAX (fora do consenso): o modelo de
Wright-Fisher com deriva, seleção, dominância e mutação, em loci
independentes.

O QUE É: o modelo estatístico clássico da genética de populações, com os
parâmetros de quem pediu. Cada locus é uma réplica independente do mesmo
processo; uma unidade de trabalho devolve a trajetória inteira das contagens
de alelos, e é dela que saem médias, fixação e heterozigosidade.

O QUE NÃO É: previsão do comportamento de uma população real. O modelo supõe
população de tamanho fixo, gerações que não se sobrepõem, cruzamento ao acaso
(Hardy-Weinberg), loci sem ligação e um só par de alelos por locus. Os
números valem para o modelo, não para um organismo; nada aqui é descoberta,
e todo resultado é "resultado computacional, que precisa de validação
científica".

Especificação (tipo 5): `tamanho` = N indivíduos diploides (M = 2N alelos por
locus), `passos` = G gerações, e cinco parâmetros u32:

  [0] loci L, 1..=64;
  [1] mutação µ em partes por milhão, 0..=10_000: a taxa por alelo por
      geração, simétrica (A vira a e a vira A com a mesma taxa);
  [2] seleção codificada como s·10^4 + 10_000, 0..=20_000 (s em [-1, +1]);
  [3] dominância h em %, 0..=100;
  [4] frequência inicial p0 por 10^4, 0..=10_000.

Custo: 2N·L·G sorteios, com teto de 2^33 (ver SORTEIOS_MAX).

Aptidões pelo número de alelos A no genótipo, na escala 10^6. Com
`sel` = s·10^4 + 10^4, todas são inteiras e nunca negativas:

  w2 (AA) = 10^6·(1 + s)   = 100·sel
  w1 (Aa) = 10^6·(1 + h·s) = 10^4·(100 - h) + h·sel
  w0 (aa) = 10^6

Contagem inicial, igual em todos os loci: k0 = floor((M·p0 + 5000) / 10^4).

Uma geração, em cada locus, com k alelos A e j = M - k alelos a:

  1. Seleção diploide sobre Hardy-Weinberg, em fração EXATA de inteiros
     (as frequências de genótipo multiplicadas por M²):
        num = k²·w2 + k·j·w1
        den = k²·w2 + 2·k·j·w1 + j²·w0
     den = 0 só acontece com j = 0 e w2 = 0 (a população inteira é AA e AA é
     letal). Sem outro alelo para ocupar o lugar, a seleção não muda p:
     num/den = k/M.
  2. Mutação simétrica, ainda exata:
        p = a / b,  a = num·(10^6 - µ) + (den - num)·µ,  b = den·10^6
  3. Um único arredondamento, para ponto fixo Q32 (1,0 = 2^32), com a metade
     para cima:
        P = floor((a·2^32 + floor(b/2)) / b), sempre em [0, 2^32]
  4. Amostragem binomial: M sorteios u32; cada sorteio u < P é um alelo A.
     P = 0 nunca dá A e P = 2^32 sempre dá, então p = 0 e p = 1 (sem
     mutação) são absorventes pela própria regra, sem exceção.

Sorteios: splitmix64 por contador. s0 = os 8 primeiros bytes (big-endian) de
xof(semente, domínio GENETICA-SORTEIO-v1). A saída número n (n >= 0) é

    z = s0 + (n + 1)·0x9E3779B97F4A7C15            (mod 2^64)
    z = (z ^ (z >> 30))·0xBF58476D1CE4E5B9         (mod 2^64)
    z = (z ^ (z >> 27))·0x94D049BB133111EB         (mod 2^64)
    saída = z ^ (z >> 31)

que é o splitmix64 de sempre (estado += gama; misturar). Cada saída dá dois
sorteios u32: primeiro os 32 bits altos, depois os 32 baixos. A geração g
(0 a G-1) no locus l usa as N saídas de n = (g·L + l)·N até n + N - 1. Como a
saída n só depende de s0 e de n, um locus fixado pode pular os sorteios sem
mudar os outros, e a conta cabe em vetor (numpy aqui, laço simples no Rust).

Resultado, em bytes canônicos (inteiros big-endian):

    u8  versão (1)
    u32 N
    u32 G
    u8  L
    u32 k0
    G·L × u32   contagem de A depois da geração g (1..=G) no locus l,
                na ordem: geração, depois locus

Conferência: recomputação. Operações declaradas: 2N·L·G sorteios, mesmo os
que um locus fixado pula.
"""

from __future__ import annotations

import numpy as np

from . import crypto, identidade

PARAMETROS = 5
TAMANHO = (2, 100_000)
PASSOS = (1, 10_000)
LOCI_MAX = 64
MUTACAO_MAX_PPM = 10_000
SELECAO_ZERO = 10_000
SELECAO_MAX = 20_000
DOMINANCIA_MAX = 100
P0_ESCALA = 10_000
# 2^33 sorteios: no Rust release, num Atom x5-Z8350 ocupado, cerca de 100
# milhões de sorteios por segundo, então perto de 85 s. Acima disso a unidade
# deixa de caber no prazo de uma tarefa; um estudo maior vira várias unidades.
SORTEIOS_MAX = 1 << 33

APTIDAO_ESCALA = 10**6
MUTACAO_ESCALA = 10**6
Q32 = 1 << 32
VERSAO = 1
CABECALHO = 1 + 4 + 4 + 1 + 4

DOMINIO_SORTEIO = identidade.rotulo("UTRAX-GENETICA-SORTEIO-v1")

MASCARA = (1 << 64) - 1
GAMA = 0x9E3779B97F4A7C15
MISTURA_1 = 0xBF58476D1CE4E5B9
MISTURA_2 = 0x94D049BB133111EB

# teto de saídas por bloco vetorizado, só para a memória do numpy
_BLOCO = 1 << 16


# ---------------------------------------------------------------------------
# Parâmetros
# ---------------------------------------------------------------------------

def validar(tamanho: int, passos: int, parametros) -> None:
    """Confere faixas e custo; recusa com ValueError e o motivo."""
    for nome, v in (("tamanho", tamanho), ("passos", passos)):
        if not isinstance(v, int) or isinstance(v, bool):
            raise ValueError(f"{nome} precisa ser inteiro")
    if not TAMANHO[0] <= tamanho <= TAMANHO[1]:
        raise ValueError(f"tamanho {tamanho} fora da faixa {TAMANHO[0]}..={TAMANHO[1]}")
    if not PASSOS[0] <= passos <= PASSOS[1]:
        raise ValueError(f"passos {passos} fora da faixa {PASSOS[0]}..={PASSOS[1]}")
    if len(parametros) != PARAMETROS:
        raise ValueError(f"a genética leva {PARAMETROS} parâmetros, vieram {len(parametros)}")
    loci, mutacao, selecao, dominancia, p0 = parametros
    faixas = (
        ("loci", loci, 1, LOCI_MAX),
        ("mutação (ppm)", mutacao, 0, MUTACAO_MAX_PPM),
        ("seleção (s·10^4 + 10^4)", selecao, 0, SELECAO_MAX),
        ("dominância (%)", dominancia, 0, DOMINANCIA_MAX),
        ("frequência inicial (por 10^4)", p0, 0, P0_ESCALA),
    )
    for nome, v, minimo, maximo in faixas:
        if not isinstance(v, int) or isinstance(v, bool):
            raise ValueError(f"{nome} precisa ser inteiro")
        if not minimo <= v <= maximo:
            raise ValueError(f"{nome} = {v} fora da faixa {minimo}..={maximo}")
    custo = operacoes(tamanho, passos, parametros)
    if custo > SORTEIOS_MAX:
        raise ValueError(f"custo de {custo} sorteios passa do teto de {SORTEIOS_MAX} (2^33)")


def operacoes(tamanho: int, passos: int, parametros) -> int:
    """Modelo de custo: um sorteio por alelo, por locus, por geração."""
    return 2 * tamanho * parametros[0] * passos


def aptidoes(selecao: int, dominancia: int) -> tuple[int, int, int]:
    """(w0, w1, w2): aptidão de aa, Aa e AA, na escala 10^6."""
    w2 = 100 * selecao
    w1 = 10_000 * (100 - dominancia) + dominancia * selecao
    return APTIDAO_ESCALA, w1, w2


def contagem_inicial(tamanho: int, p0: int) -> int:
    """k0 = round(2N·p0 / 10^4), com a metade para cima."""
    return (2 * tamanho * p0 + P0_ESCALA // 2) // P0_ESCALA


def limiar(k: int, m: int, w: tuple[int, int, int], mutacao: int) -> int:
    """P em Q32: a frequência de A depois de seleção e mutação, com um único
    arredondamento (metade para cima). Ver o cabeçalho do módulo."""
    w0, w1, w2 = w
    j = m - k
    num = k * k * w2 + k * j * w1
    den = num + k * j * w1 + j * j * w0
    if den == 0:
        num, den = k, m
    a = num * (MUTACAO_ESCALA - mutacao) + (den - num) * mutacao
    b = den * MUTACAO_ESCALA
    return (a * Q32 + b // 2) // b


# ---------------------------------------------------------------------------
# Sorteios
# ---------------------------------------------------------------------------

def estado_inicial(semente: bytes) -> int:
    return int.from_bytes(crypto.xof(semente, 8, domain=DOMINIO_SORTEIO), "big")


def splitmix64(s0: int, n: int) -> int:
    """A saída número n do fluxo (a especificação, em Python puro)."""
    z = (s0 + (n + 1) * GAMA) & MASCARA
    z = ((z ^ (z >> 30)) * MISTURA_1) & MASCARA
    z = ((z ^ (z >> 27)) * MISTURA_2) & MASCARA
    return z ^ (z >> 31)


def contar_referencia(s0: int, inicio: int, n: int, p: int) -> int:
    """Alelos A em 2n sorteios a partir da saída `inicio`, um por um."""
    c = 0
    for i in range(n):
        x = splitmix64(s0, inicio + i)
        c += (x >> 32) < p
        c += (x & 0xFFFFFFFF) < p
    return c


_U = np.uint64


def _contar_loci(s0: int, primeiras: list[int], n: int, limiares: list[int], passo: np.ndarray) -> list[int]:
    """O mesmo que `contar_referencia` para vários loci de uma vez, em uint64
    do numpy (que dá a volta em 2^64 sem aviso nas operações de vetor).
    `primeiras[i]` é a primeira saída do locus i; `passo` é i·GAMA para
    i = 0..n-1, já reduzido mod 2^64."""
    z0 = np.array([(s0 + (p + 1) * GAMA) & MASCARA for p in primeiras], dtype=np.uint64)
    z = z0[:, None] + passo[None, :]
    z ^= z >> _U(30)
    z *= _U(MISTURA_1)
    z ^= z >> _U(27)
    z *= _U(MISTURA_2)
    z ^= z >> _U(31)
    p = np.array(limiares, dtype=np.uint64)[:, None]
    altos = np.count_nonzero((z >> _U(32)) < p, axis=1)
    z &= _U(0xFFFFFFFF)
    return [int(a) + int(b) for a, b in zip(altos, np.count_nonzero(z < p, axis=1))]


# ---------------------------------------------------------------------------
# Execução
# ---------------------------------------------------------------------------

def executar(tamanho: int, passos: int, parametros, semente: bytes, continuar=None) -> bytes | None:
    """Roda a unidade. `continuar(ops)`, se vier, é chamada a cada geração com
    os sorteios dela; devolver False cancela (e a função devolve None), a não
    ser na última geração, quando o trabalho já acabou."""
    validar(tamanho, passos, parametros)
    loci, mutacao, selecao, dominancia, p0 = parametros
    m = 2 * tamanho
    w = aptidoes(selecao, dominancia)
    s0 = estado_inicial(semente)
    k0 = contagem_inicial(tamanho, p0)
    ks = [k0] * loci
    trajetoria: list[int] = []
    por_bloco = max(1, _BLOCO // tamanho)
    passo = np.arange(tamanho, dtype=np.uint64) * _U(GAMA)
    memo: dict[int, int] = {}  # o limiar só depende de k nesta unidade
    for g in range(passos):
        ps = []
        for k in ks:
            p = memo.get(k)
            if p is None:
                p = memo[k] = limiar(k, m, w, mutacao)
            ps.append(p)
        novos = [0 if p == 0 else m if p == Q32 else -1 for p in ps]
        ativos = [l for l, v in enumerate(novos) if v < 0]
        for i in range(0, len(ativos), por_bloco):
            pedaco = ativos[i:i + por_bloco]
            primeiras = [(g * loci + l) * tamanho for l in pedaco]
            contagens = _contar_loci(s0, primeiras, tamanho, [ps[l] for l in pedaco], passo)
            for l, c in zip(pedaco, contagens):
                novos[l] = c
        ks = novos
        trajetoria.extend(ks)
        if continuar is not None and not continuar(m * loci) and g + 1 < passos:
            return None
    return codificar(tamanho, passos, loci, k0, trajetoria)


def executar_referencia(tamanho: int, passos: int, parametros, semente: bytes) -> bytes:
    """A especificação ao pé da letra: todos os sorteios, um por um, sem pular
    locus fixado nem vetorizar. Lenta; serve para conferir `executar`."""
    validar(tamanho, passos, parametros)
    loci, mutacao, selecao, dominancia, p0 = parametros
    m = 2 * tamanho
    w = aptidoes(selecao, dominancia)
    s0 = estado_inicial(semente)
    k0 = contagem_inicial(tamanho, p0)
    ks = [k0] * loci
    trajetoria: list[int] = []
    for g in range(passos):
        ks = [contar_referencia(s0, (g * loci + l) * tamanho, tamanho, limiar(k, m, w, mutacao))
              for l, k in enumerate(ks)]
        trajetoria.extend(ks)
    return codificar(tamanho, passos, loci, k0, trajetoria)


# ---------------------------------------------------------------------------
# Resultado
# ---------------------------------------------------------------------------

def codificar(tamanho: int, passos: int, loci: int, k0: int, trajetoria) -> bytes:
    return (bytes([VERSAO]) + tamanho.to_bytes(4, "big") + passos.to_bytes(4, "big")
            + bytes([loci]) + k0.to_bytes(4, "big")
            + np.asarray(trajetoria, dtype=">u4").tobytes())


def decodificar(resultado: bytes) -> dict | None:
    """Lê um resultado bem formado; None se a estrutura não fecha."""
    if len(resultado) < CABECALHO or resultado[0] != VERSAO:
        return None
    tamanho = int.from_bytes(resultado[1:5], "big")
    passos = int.from_bytes(resultado[5:9], "big")
    loci = resultado[9]
    k0 = int.from_bytes(resultado[10:14], "big")
    m = 2 * tamanho
    if not (TAMANHO[0] <= tamanho <= TAMANHO[1] and PASSOS[0] <= passos <= PASSOS[1]
            and 1 <= loci <= LOCI_MAX and k0 <= m):
        return None
    if len(resultado) != CABECALHO + 4 * passos * loci:
        return None
    contagens = [int(v) for v in np.frombuffer(resultado[CABECALHO:], dtype=">u4")]
    if any(c > m for c in contagens):
        return None
    return {"tamanho": tamanho, "passos": passos, "loci": loci, "inicial": k0, "contagens": contagens}


def verificar(tamanho: int, passos: int, parametros, semente: bytes, resultado: bytes) -> tuple[bool, str]:
    """Recomputação. Devolve (aceito, motivo da recusa). Especificação
    inválida não é julgada: levanta ValueError, como em `executar`."""
    validar(tamanho, passos, parametros)
    loci = parametros[0]
    if len(resultado) != CABECALHO + 4 * passos * loci:
        return False, "resultado com tamanho que não fecha G × L contagens"
    refeito = executar(tamanho, passos, parametros, semente)
    if refeito == resultado:
        return True, ""
    if refeito[:CABECALHO] != resultado[:CABECALHO]:
        return False, "o cabeçalho não confere com a especificação"
    for i in range(passos * loci):
        a = CABECALHO + 4 * i
        if refeito[a:a + 4] != resultado[a:a + 4]:
            return False, f"a contagem da geração {i // loci + 1}, locus {i % loci} difere da recomputação"
    return False, "difere da recomputação"


def estatisticas(resultado: bytes) -> dict | None:
    """O que o relatório e o agregador do JOB precisam, em inteiros exatos.

    Por geração t = 0..=G (t = 0 é o início): a soma das contagens nos loci
    (S1) e a soma dos quadrados (S2). Com M = 2N, a frequência média é
    S1/(L·M) e a heterozigosidade esperada média, 2p(1-p) nos loci, é
    (2·M·S1 - 2·S2)/(L·M²). No fim: loci fixados (k = M) e perdidos (k = 0).
    """
    t = decodificar(resultado)
    if t is None:
        return None
    loci, m = t["loci"], 2 * t["tamanho"]
    linhas = [[t["inicial"]] * loci] + [t["contagens"][g * loci:(g + 1) * loci] for g in range(t["passos"])]
    final = linhas[-1]
    return {
        "fixados": sum(1 for k in final if k == m),
        "perdidos": sum(1 for k in final if k == 0),
        "soma_por_geracao": [sum(linha) for linha in linhas],
        "soma_quadrados_por_geracao": [sum(k * k for k in linha) for linha in linhas],
    }
