# ✝ Gênesis 1:11 — “Produza a terra erva verde, erva que dê semente.”
"""HYURAX — motor de melhoramento de culturas do ULTRAX (fora do consenso).

Tipo de trabalho 6. `tamanho` = N indivíduos (plantas) da população;
`passos` = G gerações de seleção. Verificação: recomputação.

O QUE É: um modelo didático de genética quantitativa, com os parâmetros de
quem pediu. Mostra como a seleção muda uma população ao longo das gerações
(resposta à seleção, perda de variância, fixação de alelos) num ambiente com
água, nitrogênio e solo definidos.

O QUE NÃO É: não prevê safra real, não descreve uma variedade real e não
substitui ensaio de campo. Os efeitos dos QTL são sorteados, não medidos; a
resposta do ambiente é uma linha-platô com números escolhidos para o modelo.
É resultado computacional, e nenhuma conclusão sobre o mundo sai dele sem
validação científica.

Tudo o que é conferido é inteiro, com arredondamento escrito aqui. Dois nós
honestos chegam no mesmo resultado bit a bit em qualquer máquina.

PARÂMETROS (8 inteiros u32, nesta ordem)

  0 qtl          L, número de QTL: 1 a 200
  1 arquitetura  código que sorteia os efeitos dos QTL: qualquer u32. É
                 parâmetro, e não semente, para que todas as unidades de um
                 mesmo JOB tenham a MESMA arquitetura genética
  2 selecionados porcentagem selecionada a cada geração: 1 a 100
  3 ruido        desvio do ruído ambiental, em % do desvio genético inicial
                 esperado (no ambiente): 0 a 1000. 100 dá herdabilidade
                 inicial perto de 1/2; 0 dá herdabilidade 1
  4 agua         água disponível, em % da demanda da cultura: 0 a 200
  5 nitrogenio   nitrogênio disponível, em % da dose de referência: 0 a 200
  6 solo         qualidade do solo, em % da referência: 0 a 200
  7 gxe          intensidade da interação genótipo × ambiente, %: 0 a 100

Combinações cujo custo passa de TETO_OPERACOES (modelo de `operacoes`) são
recusadas: 4·10^9 operações do modelo. O Rust em release faz perto de 5·10^8
por segundo no Atom de desenvolvimento (medido em 27/09/2026), então o teto é
cerca de 8 s por unidade, e o dobro com a verificação.

MODELO (Q16 quer dizer inteiro com 1,0 = 65536)

1. Arquitetura. Para cada QTL i, quatro u32 big-endian de
   XOF(u32 arquitetura big-endian, domínio MELHORAMENTO-ARQUITETURA-v1):
     a_i = 1 + (r1 mod 10000) · (r2 mod 10000) // 10000      em [1, 9999]
   (produto de dois uniformes: muitos efeitos pequenos e poucos grandes);
     se r3 é ímpar, o QTL é sensível ao ambiente e
       d_i = (r4 mod 19999) − 9999                            em [−9999, 9999]
     senão d_i = 0.
   O alelo 1 soma a_i por cópia na condição sem estresse.

2. Ambiente, pela lei do mínimo de Liebig. Cada recurso responde em
   linha-platô: com intercepto I (resposta sem o recurso) e ponto crítico c
   (onde começa o platô),
     f(x) = min(Q16, I_q + (Q16 − I_q) · x // c),  I_q = I · Q16 // 100
   água: I = 0 %, c = 80 %; nitrogênio: I = 30 %, c = 100 %;
   solo: I = 40 %, c = 100 %. O fator é o MÍNIMO dos três:
     F = min(f_agua, f_nitrogenio, f_solo)   (Q16)
   O recurso limitante é o de menor f (empate: o de menor índice, na ordem
   água 0, nitrogênio 1, solo 2); 3 quando nenhum limita (F = Q16). Acima do
   ponto crítico a resposta fica no platô: o modelo não representa excesso
   (encharcamento, toxidez).

3. G×E. O estresse é S = Q16 − F. O efeito de cada QTL no ambiente é
     e_i = a_i + (d_i · S · gxe) // (100 · Q16)        (divisão com piso)
   Sem estresse, ou com gxe = 0, e_i = a_i. Com estresse, os QTL sensíveis
   ganham ou perdem valor, e o alelo 1 pode até passar a ser o pior.

4. Valor genético. Cada indivíduo é diploide; n_i ∈ {0, 1, 2} é o número de
   cópias do alelo 1 no QTL i. O potencial, em partes por milhão do
   potencial de referência (1.000.000 = 100 %), é
     Gp = max(0, 1.000.000 + Σ e_i · (n_i − 1))
   e o valor genético no ambiente, que é o que se reporta, é
     G = (Gp · F) >> 16          (piso)
   Os QTL são aditivos: sem dominância e sem epistasia.

5. Ruído ambiental. Desvio nominal, pela variância genética esperada com
   frequência 1/2 (Var n_i = 1/2):
     sigma = isqrt(Σ e_i² · F² · ruido² // (2 · Q16² · 100²))
   Para cada indivíduo, três sorteios de 64 bits dão doze uniformes de 16
   bits (bits 0-15, 16-31, 32-47, 48-63 de cada um, nessa ordem). A soma
   (Irwin-Hall) menos a média dá z = Σu − 393210, com desvio ≈ 65536 e
   forma perto da normal. O ruído é (z · sigma + 32768) >> 16 (arredonda
   para o mais perto) e o fenótipo é
     P = max(0, G + ruído)
   Os três sorteios acontecem sempre, mesmo com sigma = 0.

6. População inicial: frequência 1/2 em todo QTL, em Hardy-Weinberg. Para
   cada indivíduo j = 0..N−1, W = ceil(L/64) sorteios para o primeiro alelo
   e W para o segundo (bit i % 64 do sorteio i // 64 é o alelo do QTL i).

7. Cada geração t = 0..G: avalia (fenótipo de todos, na ordem do índice),
   registra as estatísticas, seleciona os M = ceil(N · selecionados / 100)
   de maior fenótipo (empate: o de menor índice) e, se t < G, cruza.
   Cruzamento: para cada filho k = 0..N−1, pai a = sel[x mod M] e pai
   b = sel[y mod M] (x, y sorteados nessa ordem; a = b é autofecundação,
   como em plantas), depois W sorteios para o gameta de a e W para o de b.
   Segregação mendeliana independente por locus: um pai homozigoto passa
   o alelo que tem; um heterozigoto passa o bit sorteado daquele locus.
   A geração G é avaliada e selecionada, mas não cruza: os selecionados
   dela são a elite que o programa entrega.

GERADOR: xoshiro256** (Blackman e Vigna), com estado inicial nos quatro u64
big-endian de XOF(semente, 32, domínio MELHORAMENTO-GERADOR-v1); se os
quatro saírem zero, o primeiro vira 1. Um único fluxo, consumido na ordem
acima: população inicial; e, por geração, ruído, e depois cruzamento.

RESULTADO (bytes canônicos, big-endian)

  u32 F (Q16) || u8 limitante || u64 sigma || u32 M
  G+1 linhas, uma por geração t = 0..G, 48 bytes cada:
    u64 média de G       (piso de ΣG / N)
    u64 variância de G   (piso de (N·ΣG² − (ΣG)²) / N², divisor N)
    u64 média de P
    u64 variância de P
    u64 média de P dos selecionados (piso de ΣP_sel / M)
    u32 QTL fixados no alelo 1
    u32 QTL fixados no alelo 0
  L × u32: cópias do alelo 1 na população final, por QTL (0 a 2N)

Tamanho: 17 + 48 · (G + 1) + 4 · L bytes.

CUSTO (operações do modelo)

  (G+1) · N · (L + 12)          avaliar: L locos e 12 uniformes por planta
  (G+1) · N · ceil(log2 N)      selecionar: a ordenação
  G · N · (2L + 2)              cruzar: dois pais e dois gametas por filho

`continuar(ops)` é chamada a cada bloco de 1024 plantas avaliadas, depois de
cada seleção e a cada bloco de 1024 filhos; a última chamada (a seleção da
geração G) conta, mas não cancela: o trabalho já acabou.
"""

from __future__ import annotations

import math
from collections import namedtuple

from . import crypto, identidade

PARAMETROS = 8
TAMANHO = (4, 20_000)
PASSOS = (1, 1_000)
QTL_MAX = 200
RUIDO_MAX = 1_000
RECURSO_MAX = 200
TETO_OPERACOES = 4_000_000_000

Q16 = 1 << 16
BASE = 1_000_000
EFEITO_MAX = 10_000
GXE_MAX = 9_999
UNIFORMES = 12
MEDIA_IRWIN_HALL = UNIFORMES * 0xFFFF // 2  # 393210
BLOCO = 1024
M64 = (1 << 64) - 1

# (intercepto %, ponto crítico %) de cada recurso: água, nitrogênio, solo
RECURSOS = ((0, 80), (30, 100), (40, 100))
NOMES_DOS_RECURSOS = ("água", "nitrogênio", "solo", "nenhum")

DOMINIO_ARQUITETURA = identidade.rotulo("UTRAX-MELHORAMENTO-ARQUITETURA-v1")
DOMINIO_GERADOR = identidade.rotulo("UTRAX-MELHORAMENTO-GERADOR-v1")

LINHA_BYTES = 48
CABECALHO_BYTES = 17

Parametros = namedtuple(
    "Parametros",
    "qtl arquitetura selecionados ruido agua nitrogenio solo gxe",
)


class Cancelado(Exception):
    """Quem pediu mandou parar. Ninguém julgou nada."""


# ---------------------------------------------------------------------------
# Gerador
# ---------------------------------------------------------------------------


class Gerador:
    """xoshiro256**: rápido, 256 bits de estado, igual nos dois lados."""

    def __init__(self, estado):
        s = list(estado)
        if not any(s):
            s[0] = 1
        self.s = s

    def proximo(self) -> int:
        # as rotações de `_rotl` escritas no lugar: é o laço mais quente
        s0, s1, s2, s3 = self.s
        x = (s1 * 5) & M64
        resultado = ((((x << 7) | (x >> 57)) & M64) * 9) & M64
        t = (s1 << 17) & M64
        s2 ^= s0
        s3 ^= s1
        s1 ^= s2
        s0 ^= s3
        s2 ^= t
        s3 = ((s3 << 45) | (s3 >> 19)) & M64
        self.s = [s0, s1, s2, s3]
        return resultado


def gerador_da_semente(semente: bytes) -> Gerador:
    bruto = crypto.xof(semente, 32, domain=DOMINIO_GERADOR)
    return Gerador(int.from_bytes(bruto[i * 8:i * 8 + 8], "big") for i in range(4))


# ---------------------------------------------------------------------------
# Parâmetros
# ---------------------------------------------------------------------------


def selecionados(tamanho: int, porcentagem: int) -> int:
    """M = ceil(N · porcentagem / 100)."""
    return (tamanho * porcentagem + 99) // 100


def operacoes(tamanho: int, passos: int, parametros) -> int:
    n, g, q = tamanho, passos, parametros[0]
    log = (n - 1).bit_length()
    return (g + 1) * n * (q + UNIFORMES) + (g + 1) * n * log + g * n * (2 * q + 2)


def validar(tamanho: int, passos: int, parametros) -> Parametros:
    """Confere as faixas e o teto de custo. `ValueError` traz o motivo."""
    if len(parametros) != PARAMETROS:
        raise ValueError(f"melhoramento leva {PARAMETROS} parâmetros, vieram {len(parametros)}")
    if not TAMANHO[0] <= tamanho <= TAMANHO[1]:
        raise ValueError(f"indivíduos fora da faixa: {tamanho} ({TAMANHO[0]} a {TAMANHO[1]})")
    if not PASSOS[0] <= passos <= PASSOS[1]:
        raise ValueError(f"gerações fora da faixa: {passos} ({PASSOS[0]} a {PASSOS[1]})")
    p = Parametros(*parametros)
    faixas = (
        ("QTL", p.qtl, 1, QTL_MAX),
        ("arquitetura", p.arquitetura, 0, 0xFFFF_FFFF),
        ("porcentagem selecionada", p.selecionados, 1, 100),
        ("ruído ambiental", p.ruido, 0, RUIDO_MAX),
        ("água", p.agua, 0, RECURSO_MAX),
        ("nitrogênio", p.nitrogenio, 0, RECURSO_MAX),
        ("solo", p.solo, 0, RECURSO_MAX),
        ("intensidade G×E", p.gxe, 0, 100),
    )
    for nome, valor, minimo, maximo in faixas:
        if not minimo <= valor <= maximo:
            raise ValueError(f"{nome} fora da faixa: {valor} ({minimo} a {maximo})")
    custo = operacoes(tamanho, passos, parametros)
    if custo > TETO_OPERACOES:
        raise ValueError(f"custo de {custo} operações passa do teto de {TETO_OPERACOES}")
    return p


# ---------------------------------------------------------------------------
# Arquitetura e ambiente
# ---------------------------------------------------------------------------


def arquitetura(codigo: int, qtl: int) -> list[tuple[int, int]]:
    """(a_i, d_i) de cada QTL. Só depende do código e de L, nunca da semente."""
    bruto = crypto.xof(codigo.to_bytes(4, "big"), 16 * qtl, domain=DOMINIO_ARQUITETURA)
    v = [int.from_bytes(bruto[k * 4:k * 4 + 4], "big") for k in range(4 * qtl)]
    arq = []
    for i in range(qtl):
        r1, r2, r3, r4 = v[4 * i:4 * i + 4]
        a = 1 + (r1 % EFEITO_MAX) * (r2 % EFEITO_MAX) // EFEITO_MAX
        d = (r4 % (2 * GXE_MAX + 1)) - GXE_MAX if r3 % 2 == 1 else 0
        arq.append((a, d))
    return arq


def resposta(recurso: int, x: int) -> int:
    """Linha-platô de um recurso, em Q16."""
    intercepto, critico = RECURSOS[recurso]
    base = intercepto * Q16 // 100
    return min(Q16, base + (Q16 - base) * x // critico)


def fator_ambiental(agua: int, nitrogenio: int, solo: int) -> tuple[int, int]:
    """(F em Q16, recurso limitante): a lei do mínimo de Liebig."""
    f = [resposta(0, agua), resposta(1, nitrogenio), resposta(2, solo)]
    fator = min(f)
    limitante = 3 if fator == Q16 else f.index(fator)
    return fator, limitante


def efeitos(arq, fator: int, gxe: int) -> list[int]:
    """e_i = a_i + (d_i · S · gxe) // (100 · Q16), S = Q16 − F."""
    estresse = Q16 - fator
    return [a + (d * estresse * gxe) // (100 * Q16) for a, d in arq]


def desvio_do_ruido(efs, fator: int, ruido: int) -> int:
    soma = sum(e * e for e in efs)
    return math.isqrt(soma * fator * fator * ruido * ruido // (2 * Q16 * Q16 * 100 * 100))


def _tabela(efs) -> list[list[int]]:
    """Por byte da máscara, a soma dos efeitos dos bits ligados (256 valores)."""
    qtl = len(efs)
    tabela = []
    for b in range((qtl + 7) // 8):
        linha = [0] * 256
        for v in range(1, 256):
            baixo = (v & -v).bit_length() - 1
            i = 8 * b + baixo
            linha[v] = linha[v & (v - 1)] + (efs[i] if i < qtl else 0)
        tabela.append(linha)
    return tabela


def _soma(tabela, nbytes: int, mascara: int) -> int:
    """Σ e_i dos bits ligados: byte b da máscara (bits 8b a 8b+7) na tabela b."""
    return sum(map(list.__getitem__, tabela, mascara.to_bytes(nbytes, "little")))


# ---------------------------------------------------------------------------
# Simulação
# ---------------------------------------------------------------------------


def _avancar(continuar, ops: int, fim: bool) -> None:
    if continuar is not None and not continuar(ops) and not fim:
        raise Cancelado()


def _media_e_variancia(valores) -> tuple[int, int]:
    n = len(valores)
    soma = sum(valores)
    quadrados = sum(v * v for v in valores)
    return soma // n, (n * quadrados - soma * soma) // (n * n)


def simular(tamanho: int, passos: int, parametros, semente: bytes, continuar=None) -> dict:
    """Roda a unidade e devolve o resultado decodificado (ver `codificar`)."""
    p = validar(tamanho, passos, parametros)
    n, qtl = tamanho, p.qtl
    palavras = (qtl + 63) // 64
    nbytes = (qtl + 7) // 8
    mascara = (1 << qtl) - 1
    fator, limitante = fator_ambiental(p.agua, p.nitrogenio, p.solo)
    efs = efeitos(arquitetura(p.arquitetura, qtl), fator, p.gxe)
    sigma = desvio_do_ruido(efs, fator, p.ruido)
    m = selecionados(n, p.selecionados)
    tabela = _tabela(efs)
    rng = gerador_da_semente(semente)
    log = (n - 1).bit_length()

    def bits() -> int:
        v = 0
        for k in range(palavras):
            v |= rng.proximo() << (64 * k)
        return v

    hom1, het = [], []
    for _ in range(n):
        a = bits() & mascara
        b = bits() & mascara
        hom1.append(a & b)
        het.append(a ^ b)

    linhas = []
    for t in range(passos + 1):
        ultima = t == passos
        genetico = [0] * n
        fenotipo = [0] * n
        for inicio in range(0, n, BLOCO):
            fim = min(n, inicio + BLOCO)
            for j in range(inicio, fim):
                h1, ht = hom1[j], het[j]
                h0 = mascara & ~(h1 | ht)
                potencial = max(0, BASE + _soma(tabela, nbytes, h1) - _soma(tabela, nbytes, h0))
                g = (potencial * fator) >> 16
                z = -MEDIA_IRWIN_HALL
                for _ in range(3):
                    x = rng.proximo()
                    z += (x & 0xFFFF) + ((x >> 16) & 0xFFFF) + ((x >> 32) & 0xFFFF) + (x >> 48)
                genetico[j] = g
                fenotipo[j] = max(0, g + ((z * sigma + 32768) >> 16))
            _avancar(continuar, (fim - inicio) * (qtl + UNIFORMES), False)

        ordem = sorted(range(n), key=lambda j: (-fenotipo[j], j))
        sel = ordem[:m]
        _avancar(continuar, n * log, ultima)

        media_g, var_g = _media_e_variancia(genetico)
        media_p, var_p = _media_e_variancia(fenotipo)
        fixo1 = fixo0 = mascara
        for h1, ht in zip(hom1, het):
            fixo1 &= h1
            fixo0 &= ~(h1 | ht)
        linhas.append({
            "media_g": media_g, "var_g": var_g,
            "media_p": media_p, "var_p": var_p,
            "media_sel": sum(fenotipo[j] for j in sel) // m,
            "fixados_1": fixo1.bit_count(), "fixados_0": (fixo0 & mascara).bit_count(),
        })
        if ultima:
            break

        novo_hom1, novo_het = [], []
        for inicio in range(0, n, BLOCO):
            fim = min(n, inicio + BLOCO)
            for _ in range(inicio, fim):
                pa = sel[rng.proximo() % m]
                pb = sel[rng.proximo() % m]
                ga = hom1[pa] | (het[pa] & bits())
                gb = hom1[pb] | (het[pb] & bits())
                novo_hom1.append(ga & gb)
                novo_het.append(ga ^ gb)
            _avancar(continuar, (fim - inicio) * (2 * qtl + 2), False)
        hom1, het = novo_hom1, novo_het

    frequencias = [0] * qtl
    for h1, ht in zip(hom1, het):
        for i in range(qtl):
            frequencias[i] += 2 * ((h1 >> i) & 1) + ((ht >> i) & 1)

    return {
        "fator": fator, "limitante": limitante, "sigma": sigma, "selecionados": m,
        "linhas": linhas, "frequencias": frequencias,
    }


# ---------------------------------------------------------------------------
# Bytes, execução e verificação
# ---------------------------------------------------------------------------

CAMPOS = (
    ("media_g", 8, "média do valor genético"),
    ("var_g", 8, "variância do valor genético"),
    ("media_p", 8, "média do fenótipo"),
    ("var_p", 8, "variância do fenótipo"),
    ("media_sel", 8, "média dos selecionados"),
    ("fixados_1", 4, "QTL fixados no alelo 1"),
    ("fixados_0", 4, "QTL fixados no alelo 0"),
)


def tamanho_do_resultado(passos: int, qtl: int) -> int:
    return CABECALHO_BYTES + LINHA_BYTES * (passos + 1) + 4 * qtl


def codificar(sim: dict) -> bytes:
    saida = bytearray()
    saida += sim["fator"].to_bytes(4, "big")
    saida += sim["limitante"].to_bytes(1, "big")
    saida += sim["sigma"].to_bytes(8, "big")
    saida += sim["selecionados"].to_bytes(4, "big")
    for linha in sim["linhas"]:
        for campo, largura, _ in CAMPOS:
            saida += linha[campo].to_bytes(largura, "big")
    for f in sim["frequencias"]:
        saida += f.to_bytes(4, "big")
    return bytes(saida)


def decodificar(dados: bytes, passos: int, qtl: int) -> dict:
    if len(dados) != tamanho_do_resultado(passos, qtl):
        raise ValueError("resultado com tamanho que não fecha as gerações e os QTL")
    pos = 0

    def ler(largura: int) -> int:
        nonlocal pos
        v = int.from_bytes(dados[pos:pos + largura], "big")
        pos += largura
        return v

    sim = {"fator": ler(4), "limitante": ler(1), "sigma": ler(8), "selecionados": ler(4)}
    sim["linhas"] = [{campo: ler(largura) for campo, largura, _ in CAMPOS} for _ in range(passos + 1)]
    sim["frequencias"] = [ler(4) for _ in range(qtl)]
    return sim


def executar(tamanho: int, passos: int, parametros, semente: bytes, continuar=None) -> bytes:
    return codificar(simular(tamanho, passos, parametros, semente, continuar))


def verificar(tamanho: int, passos: int, parametros, semente: bytes, resultado: bytes, continuar=None):
    """None se confere; senão, o motivo da recusa. `Cancelado` sobe: ninguém julgou."""
    qtl = parametros[0]
    if len(resultado) != tamanho_do_resultado(passos, qtl):
        return "resultado com tamanho que não fecha as gerações e os QTL"
    refeito = executar(tamanho, passos, parametros, semente, continuar)
    if refeito == resultado:
        return None
    a = decodificar(refeito, passos, qtl)
    b = decodificar(resultado, passos, qtl)
    for campo, nome in (("fator", "fator ambiental"), ("limitante", "recurso limitante"),
                        ("sigma", "desvio do ruído"), ("selecionados", "número de selecionados")):
        if a[campo] != b[campo]:
            return f"{nome} difere da recomputação"
    for t, (la, lb) in enumerate(zip(a["linhas"], b["linhas"])):
        for campo, _, nome in CAMPOS:
            if la[campo] != lb[campo]:
                return f"geração {t}: {nome} difere da recomputação"
    for i, (fa, fb) in enumerate(zip(a["frequencias"], b["frequencias"])):
        if fa != fb:
            return f"QTL {i}: frequência final difere da recomputação"
    return "o resultado difere da recomputação"
