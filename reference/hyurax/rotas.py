# ✝ Provérbios 16:9 — “O coração do homem planeja o seu caminho.”
"""HYURAX — motor de rotas do ULTRAX (fora do consenso).

Tipo de trabalho 7: caixeiro-viajante simétrico e euclidiano, resolvido por
2-opt. `tamanho` = n cidades (4 a 2000); `passos` = teto de passadas
completas do 2-opt (1 a 10000). Verificação: recomputação, e dois
certificados baratos (abaixo) para quem só quer conferir a rota.

O QUE É: busca local 2-opt numa instância sintética de pontos uniformes num
quadrado. Cada unidade parte de uma rota sorteada pela sua semente e desce
até um ótimo local do 2-opt (ou até esgotar as passadas). Várias unidades do
mesmo JOB resolvem a MESMA instância a partir de partidas diferentes: é o
"multi-partida" clássico, e o JOB fica com a melhor.

O QUE NÃO É: não é o ótimo global. O 2-opt para no primeiro ótimo local, que
em pontos uniformes costuma ficar alguns por cento acima do ótimo, e nada aqui
prova quanto. A instância é sorteada, não é um mapa real: não há ruas,
trânsito, janelas de horário nem capacidade de veículo. É resultado
computacional; nenhuma conclusão sobre uma operação real sai dele sem
validação.

Tudo o que é conferido é inteiro. Dois nós honestos chegam na mesma rota bit a
bit em qualquer máquina.

PARÂMETROS (1 inteiro u32)

  0 instancia   código da instância: qualquer u32. As coordenadas vêm dele, e
                não da semente da unidade, para que todas as unidades de um
                mesmo JOB resolvam as mesmas cidades.

Combinações cujo custo passa de TETO_AVALIACOES (modelo de `operacoes`, o
pior caso: todas as passadas até o fim) são recusadas. Ver o valor e a medida
junto da constante.

MODELO

1. Instância. XOF(u32 instancia big-endian, 16·n bytes, domínio
   ROTAS-INSTANCIA-v1). A cidade k lê dois u64 big-endian, nos bytes
   [16k, 16k+8) e [16k+8, 16k+16):
     x_k = u64 mod 10000,   y_k = u64 mod 10000
   (o viés do módulo é menor que 10^-15). O XOF é modo contador, então a
   instância de n cidades é o começo da de n+1: as cidades não dependem de n.

2. Distância. d(p, q) = floor(sqrt(dx² + dy²)), pela raiz quadrada inteira
   exata (dx² + dy² < 2·10^8, cabe em u64 com folga). Calculada na hora:
   nenhuma matriz n² é guardada.

3. Partida. Gerador xoshiro256** (Blackman e Vigna), com estado inicial nos
   quatro u64 big-endian de XOF(semente, 32, domínio ROTAS-PARTIDA-v1); se os
   quatro saírem zero, o primeiro vira 1. Fisher-Yates sobre [0, 1, …, n−1]:
     para k = n−1 descendo até 1: j = proximo() mod (k+1); troca t[k] e t[j]
   (viés do módulo menor que 2000/2^64).

4. 2-opt, primeira melhora, varredura fixa. A rota t tem as arestas
   e_k = (t[k], t[k+1 mod n]), k = 0..n−1, com os comprimentos guardados em
   aresta[k]. Cada passada percorre, nesta ordem,
     i = 0 .. n−3,  j = i+2 .. n−1,  pulando (i = 0, j = n−1)
   (os pares de arestas que não se tocam: n(n−3)/2 candidatos). Para cada
   candidato, com a = t[i], b = t[i+1], c = t[j], d = t[j+1 mod n] e
   D = aresta[i] + aresta[j]:
     dac = d(a, c)                         (uma avaliação)
     se dac >= D: segue (a troca não pode encurtar, porque d(b, d) >= 0)
     dbd = d(b, d)                         (segunda avaliação)
     se dac + dbd < D: aplica a troca:
        inverte t[i+1 ..= j] e aresta[i+1 ..= j−1];
        aresta[i] = dac; aresta[j] = dbd; comprimento −= D − dac − dbd
     e a varredura CONTINUA no mesmo i com o próximo j, já sobre a rota nova
     (b e aresta[i] mudaram).
   Uma passada sem nenhuma troca prova que a rota é um ótimo local do 2-opt
   e encerra (otimo_local = 1). Se `passos` passadas terminam todas com
   troca, o trabalho para ali (otimo_local = 0: não se afirma nada).
   Escolhemos primeira melhora, e não melhor melhora, porque cada troca sai
   no meio da varredura sem guardar candidato: é o 2-opt mais simples de
   especificar ao pé da letra e, em pontos uniformes, chega a ótimos locais
   da mesma qualidade.

5. Rota canônica. Gira a rota para começar na cidade 0 e, se rota[1] >
   rota[n−1], inverte o sentido (rota = [0] + rota[1:] ao contrário). Duas
   unidades que chegam no mesmo ciclo entregam os mesmos bytes.

OPERAÇÕES = avaliações de distância feitas: n no comprimento inicial, mais
uma ou duas por candidato, como no passo 4. É determinístico. O teto (pior
caso, o que `operacoes` devolve) é n + passos·n·(n−3). Inverter trechos custa
O(n) por troca e não entra na conta; cada troca encurta a rota em pelo menos
1, então há no máximo `comprimento inicial` delas.

`continuar(ops)` é chamada ao fim de cada linha i da varredura sempre que as
avaliações pendentes chegam a BLOCO_CONTINUAR, e uma última vez ao terminar,
com o resto (pode ser 0). A última chamada conta, mas não cancela: o trabalho
já acabou.

RESULTADO (bytes canônicos, big-endian), 25 + 2n bytes

  u64 comprimento         da rota final
  u64 comprimento inicial da partida sorteada
  u32 passadas            feitas, contando a última sem troca
  u32 melhorias           trocas aplicadas
  u8  otimo_local         1 = a última passada não achou troca; 0 = esgotou
  n × u16 rota            canônica (começa em 0, rota[1] < rota[n−1])

O QUE CADA CONFERÊNCIA PROVA

- `certificar` (O(n)): os bytes têm o formato; a rota é uma permutação de
  0..n−1 na forma canônica; o comprimento declarado é a soma exata das n
  arestas nesta instância; e os campos do cabeçalho são coerentes entre si.
  Prova que existe uma rota com aquele comprimento (um limite superior do
  ótimo). NÃO prova que a rota saiu desta semente, nem que o 2-opt rodou,
  nem nada sobre otimalidade.
- `certificar_otimo_local` (O(n²)): tudo o de cima e, além disso, que
  nenhuma troca 2-opt encurta a rota. Prova otimalidade LOCAL na vizinhança
  do 2-opt, com a métrica inteira. NÃO prova otimalidade global nem a
  origem da rota.
- `verificar` (recomputação): refaz a unidade e compara byte a byte. Prova
  que o resultado é exatamente o que a especificação e a semente produzem.
"""

from __future__ import annotations

import math
from collections import namedtuple

from . import crypto, identidade

PARAMETROS = 1
TAMANHO = (4, 2_000)
PASSOS = (1, 10_000)
LADO = 10_000
# Teto de avaliações de distância por unidade (o pior caso de `operacoes`).
# O Rust em release faz perto de 2·10^8 avaliações por segundo no Atom de
# desenvolvimento (medido em 27/09/2026), então o teto é cerca de 20 s por
# unidade, e o dobro com a verificação. Com n = 2000 cabem 1251 passadas; uma
# partida sorteada converge bem antes disso.
TETO_AVALIACOES = 5_000_000_000
BLOCO_CONTINUAR = 1 << 20
CABECALHO = 25
MASCARA64 = (1 << 64) - 1

DOMINIO_INSTANCIA = identidade.rotulo("ULTRAX-ROTAS-INSTANCIA-v1")
DOMINIO_PARTIDA = identidade.rotulo("ULTRAX-ROTAS-PARTIDA-v1")


class Recusa(ValueError):
    """O resultado foi julgado e está errado; o texto é a evidência."""


class Cancelado(Exception):
    """Quem pediu mandou parar: ninguém julgou nada."""


Resolucao = namedtuple(
    "Resolucao",
    "comprimento inicial passadas melhorias otimo_local rota operacoes",
)


# ---------------------------------------------------------------------------
# faixas e custo
# ---------------------------------------------------------------------------

def operacoes(n: int, passos: int) -> int:
    """Teto de avaliações: n no comprimento inicial e, por passada, até duas
    por candidato, com n(n−3)/2 candidatos."""
    return n + passos * n * (n - 3)


def operacoes_de_verificacao(n: int, passos: int) -> int:
    """O certificado (n avaliações) antes da recomputação."""
    return operacoes(n, passos) + n


def validar(tamanho: int, passos: int, parametros) -> None:
    if len(parametros) != PARAMETROS:
        raise ValueError(f"rotas leva {PARAMETROS} parâmetro, vieram {len(parametros)}")
    if not TAMANHO[0] <= tamanho <= TAMANHO[1]:
        raise ValueError(f"cidades fora da faixa ({TAMANHO[0]} a {TAMANHO[1]}): {tamanho}")
    if not PASSOS[0] <= passos <= PASSOS[1]:
        raise ValueError(f"passadas fora da faixa ({PASSOS[0]} a {PASSOS[1]}): {passos}")
    if not 0 <= parametros[0] < 1 << 32:
        raise ValueError("instância fora de u32")
    custo = operacoes(tamanho, passos)
    if custo > TETO_AVALIACOES:
        raise ValueError(f"custo de {custo} avaliações passa do teto de {TETO_AVALIACOES}")


# ---------------------------------------------------------------------------
# instância, distância e partida
# ---------------------------------------------------------------------------

def coordenadas(instancia: int, n: int) -> list[tuple[int, int]]:
    bruto = crypto.xof(instancia.to_bytes(4, "big"), 16 * n, domain=DOMINIO_INSTANCIA)
    cidades = []
    for k in range(n):
        x = int.from_bytes(bruto[16 * k:16 * k + 8], "big") % LADO
        y = int.from_bytes(bruto[16 * k + 8:16 * k + 16], "big") % LADO
        cidades.append((x, y))
    return cidades


def distancia(p: tuple[int, int], q: tuple[int, int]) -> int:
    dx = p[0] - q[0]
    dy = p[1] - q[1]
    return math.isqrt(dx * dx + dy * dy)


def comprimento_da_rota(cidades, rota) -> int:
    n = len(rota)
    return sum(distancia(cidades[rota[k]], cidades[rota[(k + 1) % n]]) for k in range(n))


def _rotl(x: int, k: int) -> int:
    return ((x << k) | (x >> (64 - k))) & MASCARA64


class Gerador:
    """xoshiro256** (Blackman e Vigna, 2018), tudo módulo 2^64."""

    def __init__(self, estado):
        self.s = [v & MASCARA64 for v in estado]
        if not any(self.s):
            self.s[0] = 1

    @classmethod
    def da_semente(cls, semente: bytes) -> "Gerador":
        bruto = crypto.xof(semente, 32, domain=DOMINIO_PARTIDA)
        return cls([int.from_bytes(bruto[8 * i:8 * i + 8], "big") for i in range(4)])

    def proximo(self) -> int:
        s = self.s
        resultado = (_rotl((s[1] * 5) & MASCARA64, 7) * 9) & MASCARA64
        t = (s[1] << 17) & MASCARA64
        s[2] ^= s[0]
        s[3] ^= s[1]
        s[1] ^= s[2]
        s[0] ^= s[3]
        s[2] ^= t
        s[3] = _rotl(s[3], 45)
        return resultado


def partida(semente: bytes, n: int) -> list[int]:
    """Fisher-Yates com o gerador da semente."""
    g = Gerador.da_semente(semente)
    t = list(range(n))
    for k in range(n - 1, 0, -1):
        j = g.proximo() % (k + 1)
        t[k], t[j] = t[j], t[k]
    return t


def canonica(rota) -> list[int]:
    k = rota.index(0)
    r = list(rota[k:]) + list(rota[:k])
    if len(r) > 2 and r[1] > r[-1]:
        r = [r[0]] + r[1:][::-1]
    return r


# ---------------------------------------------------------------------------
# 2-opt
# ---------------------------------------------------------------------------

def resolver(n: int, passos: int, instancia: int, semente: bytes, continuar=None) -> Resolucao:
    """A unidade inteira, como no passo 4 do modelo. `continuar(ops)`, se
    vier, recebe as avaliações em pedaços; devolver False levanta
    `Cancelado`."""
    cidades = coordenadas(instancia, n)
    t = partida(semente, n)
    aresta = [distancia(cidades[t[k]], cidades[t[(k + 1) % n]]) for k in range(n)]
    ops = n
    pendente = n
    comprimento = inicial = sum(aresta)
    passadas = melhorias = 0
    otimo = False
    while passadas < passos:
        passadas += 1
        melhorou = False
        for i in range(n - 2):
            a = cidades[t[i]]
            fim = n - 1 if i > 0 else n - 2
            for j in range(i + 2, fim + 1):
                dab = aresta[i]
                dcd = aresta[j]
                limite = dab + dcd
                dac = distancia(a, cidades[t[j]])
                ops += 1
                pendente += 1
                if dac >= limite:
                    continue
                dbd = distancia(cidades[t[i + 1]], cidades[t[(j + 1) % n]])
                ops += 1
                pendente += 1
                if dac + dbd < limite:
                    t[i + 1:j + 1] = t[i + 1:j + 1][::-1]
                    aresta[i + 1:j] = aresta[i + 1:j][::-1]
                    aresta[i] = dac
                    aresta[j] = dbd
                    comprimento -= limite - dac - dbd
                    melhorias += 1
                    melhorou = True
            if continuar is not None and pendente >= BLOCO_CONTINUAR:
                if not continuar(pendente):
                    raise Cancelado()
                pendente = 0
        if not melhorou:
            otimo = True
            break
    if continuar is not None:
        continuar(pendente)  # a última conta, mas não cancela
    return Resolucao(comprimento, inicial, passadas, melhorias, otimo, canonica(t), ops)


# ---------------------------------------------------------------------------
# bytes, certificados e verificação
# ---------------------------------------------------------------------------

def codificar(r: Resolucao) -> bytes:
    return (
        r.comprimento.to_bytes(8, "big")
        + r.inicial.to_bytes(8, "big")
        + r.passadas.to_bytes(4, "big")
        + r.melhorias.to_bytes(4, "big")
        + bytes([1 if r.otimo_local else 0])
        + b"".join(c.to_bytes(2, "big") for c in r.rota)
    )


def decodificar(dados: bytes, n: int):
    """(comprimento, inicial, passadas, melhorias, otimo_local, rota), sem
    julgar nada além do tamanho."""
    if len(dados) != CABECALHO + 2 * n:
        raise Recusa(f"resultado com {len(dados)} bytes; {n} cidades pedem {CABECALHO + 2 * n}")
    comprimento = int.from_bytes(dados[0:8], "big")
    inicial = int.from_bytes(dados[8:16], "big")
    passadas = int.from_bytes(dados[16:20], "big")
    melhorias = int.from_bytes(dados[20:24], "big")
    otimo = dados[24]
    rota = [int.from_bytes(dados[CABECALHO + 2 * k:CABECALHO + 2 * k + 2], "big") for k in range(n)]
    return comprimento, inicial, passadas, melhorias, otimo, rota


def certificar(n: int, passos: int, instancia: int, resultado: bytes) -> int:
    """O(n): formato, permutação canônica e comprimento exato. Devolve o
    comprimento; levanta `Recusa` com o motivo."""
    comprimento, inicial, passadas, melhorias, otimo, rota = decodificar(resultado, n)
    if otimo not in (0, 1):
        raise Recusa(f"marca de ótimo local inválida: {otimo}")
    if not 1 <= passadas <= passos:
        raise Recusa(f"{passadas} passadas, fora de 1 a {passos}")
    if otimo == 0 and passadas != passos:
        raise Recusa("parou antes do teto de passadas sem declarar ótimo local")
    visto = [False] * n
    for c in rota:
        if c >= n:
            raise Recusa(f"a cidade {c} não existe")
        if visto[c]:
            raise Recusa(f"a cidade {c} aparece duas vezes")
        visto[c] = True
    if rota[0] != 0 or rota[1] > rota[-1]:
        raise Recusa("rota fora da forma canônica")
    medido = comprimento_da_rota(coordenadas(instancia, n), rota)
    if medido != comprimento:
        raise Recusa(f"comprimento declarado {comprimento}, a rota mede {medido}")
    if comprimento > inicial:
        raise Recusa(f"a rota final ({comprimento}) é mais longa que a partida ({inicial})")
    if (melhorias == 0) != (comprimento == inicial) or melhorias > inicial - comprimento:
        raise Recusa(f"{melhorias} melhorias não fecham com a queda de {inicial - comprimento}")
    return comprimento


def troca_que_melhora(cidades, rota):
    """A primeira troca 2-opt (i, j, ganho) que encurta a rota, na ordem da
    varredura; None se não há (ótimo local)."""
    n = len(rota)
    for i in range(n - 2):
        fim = n - 1 if i > 0 else n - 2
        a = cidades[rota[i]]
        b = cidades[rota[i + 1]]
        dab = distancia(a, b)
        for j in range(i + 2, fim + 1):
            c = cidades[rota[j]]
            d = cidades[rota[(j + 1) % n]]
            limite = dab + distancia(c, d)
            dac = distancia(a, c)
            if dac >= limite:
                continue
            dbd = distancia(b, d)
            if dac + dbd < limite:
                return i, j, limite - dac - dbd
    return None


def certificar_otimo_local(n: int, passos: int, instancia: int, resultado: bytes) -> int:
    """O(n²): o certificado de cima e nenhuma troca 2-opt que encurte."""
    comprimento = certificar(n, passos, instancia, resultado)
    rota = decodificar(resultado, n)[5]
    troca = troca_que_melhora(coordenadas(instancia, n), rota)
    if troca is not None:
        i, j, ganho = troca
        raise Recusa(f"a troca 2-opt das arestas {i} e {j} encurta a rota em {ganho}")
    return comprimento


def executar(tamanho: int, passos: int, parametros, semente: bytes, continuar=None) -> tuple[bytes, int]:
    validar(tamanho, passos, parametros)
    r = resolver(tamanho, passos, parametros[0], semente, continuar)
    return codificar(r), r.operacoes


def verificar(tamanho: int, passos: int, parametros, semente: bytes, resultado: bytes) -> None:
    """Certificado barato primeiro, depois recomputação. Levanta `Recusa`."""
    validar(tamanho, passos, parametros)
    certificar(tamanho, passos, parametros[0], resultado)
    refeito, _ = executar(tamanho, passos, parametros, semente)
    if refeito == resultado:
        return
    campos = (("comprimento", 0, 8), ("comprimento inicial", 8, 16), ("passadas", 16, 20),
              ("melhorias", 20, 24), ("marca de ótimo local", 24, 25))
    for nome, a, b in campos:
        if refeito[a:b] != resultado[a:b]:
            raise Recusa(f"{nome} difere da recomputação")
    for k in range(tamanho):
        p = CABECALHO + 2 * k
        if refeito[p:p + 2] != resultado[p:p + 2]:
            raise Recusa(f"a rota difere da recomputação na posição {k}")
    raise Recusa("o resultado difere da recomputação")


def resumo(tamanho: int, passos: int, parametros) -> str:
    return f"rotas: {tamanho} cidades da instância {parametros[0]}, até {passos} passadas do 2-opt"
