# ✝ Provérbios 1:5 — “Ouça o sábio e cresça em conhecimento.”
"""HYURAX — trabalho de IA do ULTRAX (fora do consenso): treinar uma rede
neural pequena para prever a solubilidade de moléculas reais na água.

A base é a AqSolDB (CC0), preparada por `tools/preparar_moleculas.py` em
`crates/hyurax-ultrax/dados/moleculas.tsv`. Prever solubilidade é uma etapa
de triagem na pesquisa de remédios: não é descoberta de remédio, e o
programa não diz que é.

Tudo em inteiros de ponto fixo Q12 (1,0 = 4096), com deslocamento aritmético
para a direita (arredonda para baixo, igual em Python e em Rust). Dois
workers honestos chegam no mesmo modelo bit a bit, em qualquer máquina: é o
que deixa conferir por recomputação e, na TESTNET, por redundância.

Rede: 10 descritores -> 16 neurônios (ReLU) -> 1 saída (log S normalizado).

Uma tarefa de treino:
  1. pesos iniciais sorteados pela semente (XOF, domínio IA-PESOS-v1);
  2. `passos` passos de descida do gradiente, cada um com um lote de
     `lote` moléculas de treino sorteadas pela semente (IA-LOTE-v1), com a
     taxa de aprendizado caindo em três degraus (1/64, 1/128, 1/256): taxa
     fixa faz o treino longo pular sem assentar;
  3. erro de validação nas moléculas de validação (índice % 5 == 0).
O resultado são os pesos finais (int32 big-endian) e o erro quadrático médio
de validação (u64 big-endian, Q12). Conferência: recomputação.
"""

from __future__ import annotations

from pathlib import Path

from . import crypto, identidade

ESCALA_BITS = 12
ESCALA = 1 << ESCALA_BITS
ENTRADAS = 10
OCULTOS = 16
LOTE_MAX = 256
PASSOS_MAX = 4096
TAXA_BITS = 6  # taxa de aprendizado inicial = 1/64; cai para 1/128 e 1/256
LIMITE = (1 << 31) - 1

DOMINIO_PESOS = identidade.rotulo("ULTRAX-IA-PESOS-v1")
DOMINIO_LOTE = identidade.rotulo("ULTRAX-IA-LOTE-v1")

ARQUIVO = Path(__file__).resolve().parents[2] / "crates" / "hyurax-ultrax" / "dados" / "moleculas.tsv"


def carregar(caminho: Path = ARQUIVO):
    """Lista de (x[10], alvo) em Q12, na ordem do arquivo."""
    base = []
    with open(caminho, encoding="utf-8") as f:
        for linha in f:
            if linha.startswith("#") or not linha.strip():
                continue
            c = linha.rstrip("\n").split("\t")
            alvo = int(c[6])
            x = [int(v) for v in c[7:17]]
            base.append((x, alvo))
    return base


def treino_e_validacao(base):
    treino = [m for i, m in enumerate(base) if i % 5 != 0]
    validacao = [m for i, m in enumerate(base) if i % 5 == 0]
    return treino, validacao


def _prender(v: int) -> int:
    return max(-LIMITE, min(LIMITE, v))


def _inteiros(semente: bytes, quantos: int, dominio: bytes) -> list[int]:
    bruto = crypto.xof(semente, quantos * 4, domain=dominio)
    return [int.from_bytes(bruto[i * 4:i * 4 + 4], "big") for i in range(quantos)]


def pesos_iniciais(semente: bytes):
    """W1 16x10 em [-1024, 1023], b1 zero, W2 16 em [-1024, 1023], b2 zero."""
    v = _inteiros(semente, OCULTOS * ENTRADAS + OCULTOS, DOMINIO_PESOS)
    w1 = [[(v[j * ENTRADAS + i] % 2048) - 1024 for i in range(ENTRADAS)] for j in range(OCULTOS)]
    w2 = [(v[OCULTOS * ENTRADAS + j] % 2048) - 1024 for j in range(OCULTOS)]
    return w1, [0] * OCULTOS, w2, 0


def prever(pesos, x):
    """Devolve (saída, pré-ativações) em Q12."""
    w1, b1, w2, b2 = pesos
    pre = [_prender((sum(x[i] * w1[j][i] for i in range(ENTRADAS)) >> ESCALA_BITS) + b1[j]) for j in range(OCULTOS)]
    h = [p if p > 0 else 0 for p in pre]
    y = _prender((sum(h[j] * w2[j] for j in range(OCULTOS)) >> ESCALA_BITS) + b2)
    return y, pre


def erro_de_validacao(pesos, validacao) -> int:
    """Erro quadrático médio em Q12 (a média trunca para baixo)."""
    soma = 0
    for x, alvo in validacao:
        y, _ = prever(pesos, x)
        e = y - alvo
        soma += (e * e) >> ESCALA_BITS
    return soma // len(validacao)


def treinar(semente: bytes, lote: int, passos: int, base=None):
    if not 1 <= lote <= LOTE_MAX:
        raise ValueError(f"lote fora da faixa: {lote}")
    if not 1 <= passos <= PASSOS_MAX:
        raise ValueError(f"passos fora da faixa: {passos}")
    base = base if base is not None else carregar()
    treino, validacao = treino_e_validacao(base)
    w1, b1, w2, b2 = pesos_iniciais(semente)
    sorteio = _inteiros(semente, lote * passos, DOMINIO_LOTE)
    curva = []
    for p in range(passos):
        g1 = [[0] * ENTRADAS for _ in range(OCULTOS)]
        gb1 = [0] * OCULTOS
        g2 = [0] * OCULTOS
        gb2 = 0
        perda = 0
        for k in range(lote):
            x, alvo = treino[sorteio[p * lote + k] % len(treino)]
            y, pre = prever((w1, b1, w2, b2), x)
            e = y - alvo
            perda += (e * e) >> ESCALA_BITS
            gb2 += e
            for j in range(OCULTOS):
                if pre[j] > 0:
                    g2[j] += (e * pre[j]) >> ESCALA_BITS
                    dh = (e * w2[j]) >> ESCALA_BITS
                    gb1[j] += dh
                    for i in range(ENTRADAS):
                        g1[j][i] += (dh * x[i]) >> ESCALA_BITS
        # média do lote e taxa de aprendizado, pela divisão inteira que arredonda para baixo
        div = lote << (TAXA_BITS + (3 * p) // passos)
        for j in range(OCULTOS):
            for i in range(ENTRADAS):
                w1[j][i] = _prender(w1[j][i] - g1[j][i] // div)
            b1[j] = _prender(b1[j] - gb1[j] // div)
            w2[j] = _prender(w2[j] - g2[j] // div)
        b2 = _prender(b2 - gb2 // div)
        curva.append(perda // lote)
    pesos = (w1, b1, w2, b2)
    return pesos, erro_de_validacao(pesos, validacao), curva


def codificar(pesos, erro: int) -> bytes:
    w1, b1, w2, b2 = pesos
    todos = [v for linha in w1 for v in linha] + b1 + w2 + [b2]
    return b"".join(v.to_bytes(4, "big", signed=True) for v in todos) + erro.to_bytes(8, "big")


def operacoes(lote: int, passos: int, validacao: int) -> int:
    """Modelo de custo: multiplicações com soma. Por amostra de treino, 176
    na ida (10x16 + 16) e 192 na volta; por passo, 193 na atualização; por
    amostra de validação, 176."""
    return passos * (lote * (176 + 192) + 193) + validacao * 176
