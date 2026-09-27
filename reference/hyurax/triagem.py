# ✝ Provérbios 25:2 — “A glória dos reis é investigar a coisa.”
"""HYURAX — triagem de moléculas reais do ULTRAX (tipo 8, fora do consenso).

Especificação de referência de `crates/hyurax-ultrax/src/triagem.rs`.

O que é: para cada molécula de uma faixa do catálogo, conferir filtros de
regra pelos descritores (massa, logP, doadores e aceptores de hidrogênio,
ligações giratórias, área polar: a "regra dos 5" de Lipinski e a de Veber, com
os limites de quem pediu) e prever a solubilidade na água (log S) com o
modelo de referência.

De onde vêm os números:
  - o catálogo (`crates/hyurax-ultrax/dados/catalogo.tsv`) são 8.289
    moléculas reais da AqSolDB (CC0), com os descritores e o log S **medido**
    que a AqSolDB publica; ver `tools/preparar_moleculas.py`;
  - o log S **previsto** sai da rede 10 -> 16 -> 1 de `hyurax/ia.py`, com os
    pesos de `crates/hyurax-ultrax/dados/modelo-referencia.txt`, treinados por
    `tools/treinar_modelo_referencia.py`. É um modelo estatístico pequeno, com
    erro típico conhecido e gravado no arquivo do modelo (raiz do erro
    quadrático médio contra o log S medido): uma previsão serve para ordenar
    candidatos, não substitui a medida.

O que NÃO é: a triagem não calcula afinidade com alvo, estabilidade,
toxicidade nem atividade biológica, porque não há modelo para isso no
programa. Nenhum resultado é "cura" ou "descoberta": é resultado
computacional, que precisa de validação científica.

Tudo em inteiros. A conta conferida é a inferência Q12 de `ia.prever` e
comparações de inteiros; dois nós honestos chegam no mesmo resultado bit a
bit. A semente não entra: a triagem é função só da especificação.

Especificação: `tamanho` = moléculas da faixa (1 a 4096), `passos` = 0, e
nove parâmetros `u32`, nesta ordem:

  0. catálogo: 0 = o catálogo AqSolDB embutido; outro valor é recusado;
  1. início da faixa: a faixa é `[início, início + tamanho)`, e precisa caber
     no catálogo;
  2. massa máxima, em milésimos de g/mol (0 a 2.000.000);
  3. logP máximo, em milésimos, deslocado de +20.000 (0 a 40.000, ou seja,
     logP de -20 a +20; logP 5 = 25.000);
  4. doadores de H máximos (0 a 100);
  5. aceptores de H máximos (0 a 100);
  6. ligações giratórias máximas (0 a 100);
  7. área polar (TPSA) máxima, em milésimos de Å² (0 a 1.000.000);
  8. log S previsto mínimo, em milésimos, deslocado de +20.000 (0 a 40.000,
     ou seja, log S de -20 a +20; log S -4 = 16.000).

Cada filtro é um bit da máscara, na ordem de `FILTROS` (bit 0 = massa ...
bit 6 = log S previsto). Os seis primeiros comparam os descritores brutos do
catálogo; o sétimo, o log S previsto. Passa quem fica dentro do limite, com
igualdade (`<=` nos máximos, `>=` no mínimo).

Nota de cada molécula (inteiro, maior é melhor): primeiro quantos filtros
passa, depois o log S previsto:
    nota = filtros_passados * 100.000 + prender(log S previsto + 50.000, 0, 99.999)
O log S previsto em milésimos vem de `ia.prever` pela normalização do alvo:
    log S = trunc(y * desvio / 4096) + média
com a divisão truncando para o zero (o `/` do Rust), e preso à faixa de i32.

Resultado canônico, tudo big-endian:
    u32 início || u32 quantidade || u32 aprovadas (máscara com os 7 bits)
    || quantidade x (u8 máscara || i32 log S previsto || u32 nota)
Conferência: recomputação.

Custo: por molécula, 176 multiplicações com soma da inferência e 7
comparações (183 operações). O teto é o da faixa máxima: 4096 moléculas,
749.568 operações; nenhuma combinação válida passa disso.

Unidades de um JOB: a unidade `i` usa a faixa que começa em
`início + i * tamanho`; a última é cortada no fim do catálogo, e unidade que
começaria depois dele não existe.
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

from . import ia

DADOS = Path(__file__).resolve().parents[2] / "crates" / "hyurax-ultrax" / "dados"
CATALOGO_ARQUIVO = DADOS / "catalogo.tsv"
MODELO_ARQUIVO = DADOS / "modelo-referencia.txt"

TIPO = 8
PARAMETROS = 9
TAMANHO = (1, 4096)
PASSOS = (0, 0)

CATALOGO_AQSOLDB = 0
DESLOCAMENTO = 20_000
MASSA_MAX = 2_000_000
LOGP_MAX = 40_000
CONTAGEM_MAX = 100
TPSA_MAX = 1_000_000
LOGS_MAX = 40_000

FILTROS = (
    "massa", "logP", "doadores de H", "aceptores de H",
    "ligações giratórias", "área polar", "log S previsto",
)
TODOS_OS_FILTROS = (1 << len(FILTROS)) - 1

# Colunas dos descritores brutos no catálogo (a ordem de ia.DESCRITORES).
MASSA, LOGP, MR, PESADOS, ACEPTORES, DOADORES, ROTAVEIS, AROMATICOS, ANEIS, TPSA = range(10)

OPERACOES_INFERENCIA = 176
OPERACOES_POR_MOLECULA = OPERACOES_INFERENCIA + len(FILTROS)
CUSTO_MAX = TAMANHO[1] * OPERACOES_POR_MOLECULA
NOTA_POR_FILTRO = 100_000
NOTA_LOGS_DESLOCAMENTO = 50_000
NOTA_LOGS_MAX = NOTA_POR_FILTRO - 1
BLOCO = 64  # moléculas entre duas chamadas de `continuar` no Rust
BYTES_CABECALHO = 12
BYTES_POR_MOLECULA = 9
I32_MIN, I32_MAX = -(1 << 31), (1 << 31) - 1


@dataclass(frozen=True)
class Molecula:
    id: str
    nome: str
    formula: str
    smiles: str
    logs_mili: int
    bruto: tuple[int, ...]  # 10 descritores brutos, na ordem de ia.DESCRITORES
    x: tuple[int, ...]  # 10 descritores normalizados, Q12


@dataclass(frozen=True)
class Catalogo:
    moleculas: tuple[Molecula, ...]
    media_mili: int
    desvio_mili: int


@dataclass(frozen=True)
class Modelo:
    pesos: tuple
    semente: bytes
    lote: int
    passos: int
    erro_validacao: int


def carregar_catalogo(caminho: Path = CATALOGO_ARQUIVO) -> Catalogo:
    moleculas = []
    media, desvio = 0, 1000
    with open(caminho, encoding="utf-8") as f:
        for linha in f:
            if linha.startswith("#normalizacao\t"):
                _, m, d = linha.rstrip("\n").split("\t")
                media, desvio = int(m), max(1, int(d))
                continue
            if linha.startswith("#") or not linha.strip():
                continue
            c = linha.rstrip("\n").split("\t")
            if len(c) != 25:
                raise ValueError(f"linha do catálogo com {len(c)} colunas")
            moleculas.append(Molecula(
                id=c[0], nome=c[1], formula=c[2], smiles=c[3], logs_mili=int(c[4]),
                bruto=tuple(int(v) for v in c[5:15]), x=tuple(int(v) for v in c[15:25]),
            ))
    return Catalogo(tuple(moleculas), media, desvio)


def ler_modelo(texto: str) -> Modelo:
    """Formato de `modelo-referencia.txt`: linhas `#chave\\tvalor` de
    metadados, comentários `# ...`, e as linhas de pesos `w1` (16 linhas de 10),
    `b1` (16), `w2` (16) e `b2` (1), separadas por tabulação."""
    meta = {}
    w1, b1, w2, b2 = [], None, None, None
    for linha in texto.splitlines():
        if linha.startswith("# ") or linha == "#" or not linha.strip():
            continue
        c = linha.split("\t")
        if c[0].startswith("#"):
            meta[c[0][1:]] = c[1]
            continue
        valores = [int(v) for v in c[1:]]
        if c[0] == "w1" and len(valores) == ia.ENTRADAS:
            w1.append(valores)
        elif c[0] == "b1" and b1 is None and len(valores) == ia.OCULTOS:
            b1 = valores
        elif c[0] == "w2" and w2 is None and len(valores) == ia.OCULTOS:
            w2 = valores
        elif c[0] == "b2" and b2 is None and len(valores) == 1:
            b2 = valores[0]
        else:
            raise ValueError(f"linha de pesos inválida: {c[0]}")
    if len(w1) != ia.OCULTOS or b1 is None or w2 is None or b2 is None:
        raise ValueError("modelo incompleto")
    return Modelo(
        pesos=(w1, b1, w2, b2),
        semente=bytes.fromhex(meta["semente"]),
        lote=int(meta["lote"]),
        passos=int(meta["passos"]),
        erro_validacao=int(meta["erro_validacao_q12"]),
    )


def carregar_modelo(caminho: Path = MODELO_ARQUIVO) -> Modelo:
    return ler_modelo(caminho.read_text(encoding="utf-8"))


@lru_cache(maxsize=1)
def catalogo_embutido() -> Catalogo:
    """O catálogo do arquivo, lido uma vez (o Rust o embute no programa)."""
    return carregar_catalogo()


@lru_cache(maxsize=1)
def modelo_embutido() -> Modelo:
    """O modelo de referência do arquivo, lido uma vez."""
    return carregar_modelo()


def escrever_modelo(pesos, semente: bytes, lote: int, passos: int, erro: int, comentarios: list[str]) -> str:
    """O inverso de `ler_modelo`. `comentarios` entram como linhas `# ...`."""
    w1, b1, w2, b2 = pesos
    linhas = [f"# {c}" if c else "#" for c in comentarios]
    linhas += [
        f"#semente\t{semente.hex()}",
        f"#lote\t{lote}",
        f"#passos\t{passos}",
        f"#erro_validacao_q12\t{erro}",
    ]
    linhas += ["w1\t" + "\t".join(str(v) for v in linha) for linha in w1]
    linhas += ["b1\t" + "\t".join(str(v) for v in b1), "w2\t" + "\t".join(str(v) for v in w2), f"b2\t{b2}"]
    return "\n".join(linhas) + "\n"


def _div_zero(a: int, b: int) -> int:
    """Divisão inteira que trunca para o zero, como o `/` do Rust."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def logs_de(y_q12: int, media_mili: int, desvio_mili: int) -> int:
    """Saída da rede (log S normalizado, Q12) para log S x 1000."""
    v = _div_zero(y_q12 * desvio_mili, ia.ESCALA) + media_mili
    return max(I32_MIN, min(I32_MAX, v))


def validar(tamanho: int, passos: int, parametros) -> None:
    if not TAMANHO[0] <= tamanho <= TAMANHO[1]:
        raise ValueError(f"tamanho fora da faixa: {tamanho}")
    if not PASSOS[0] <= passos <= PASSOS[1]:
        raise ValueError(f"passos fora da faixa: {passos}")
    if len(parametros) != PARAMETROS:
        raise ValueError(f"a triagem leva {PARAMETROS} parâmetros, vieram {len(parametros)}")
    if any(not 0 <= p < 1 << 32 for p in parametros):
        raise ValueError("parâmetro fora de u32")
    catalogo, inicio, massa, logp, doadores, aceptores, rotaveis, tpsa, logs = parametros
    if catalogo != CATALOGO_AQSOLDB:
        raise ValueError(f"catálogo {catalogo} desconhecido: só existe o 0, AqSolDB embutido")
    n = len(catalogo_embutido().moleculas)
    if inicio + tamanho > n:
        raise ValueError(f"faixa [{inicio}, {inicio + tamanho}) fora do catálogo de {n} moléculas")
    for nome, valor, maximo in (
        ("massa máxima", massa, MASSA_MAX), ("logP máximo", logp, LOGP_MAX),
        ("doadores de H máximos", doadores, CONTAGEM_MAX), ("aceptores de H máximos", aceptores, CONTAGEM_MAX),
        ("ligações giratórias máximas", rotaveis, CONTAGEM_MAX), ("área polar máxima", tpsa, TPSA_MAX),
        ("log S previsto mínimo", logs, LOGS_MAX),
    ):
        if valor > maximo:
            raise ValueError(f"{nome} fora da faixa: {valor} (máximo {maximo})")
    if operacoes(tamanho) > CUSTO_MAX:
        raise ValueError("custo acima do teto")


def operacoes(tamanho: int) -> int:
    return tamanho * OPERACOES_POR_MOLECULA


def avaliar(m: Molecula, parametros, pesos, media_mili: int, desvio_mili: int) -> tuple[int, int, int]:
    """(máscara, log S previsto em milésimos, nota) de uma molécula."""
    _, _, massa, logp, doadores, aceptores, rotaveis, tpsa, logs_min = parametros
    y, _ = ia.prever(pesos, list(m.x))
    previsto = logs_de(y, media_mili, desvio_mili)
    b = m.bruto
    testes = (
        b[MASSA] <= massa,
        b[LOGP] <= logp - DESLOCAMENTO,
        b[DOADORES] <= doadores,
        b[ACEPTORES] <= aceptores,
        b[ROTAVEIS] <= rotaveis,
        b[TPSA] <= tpsa,
        previsto >= logs_min - DESLOCAMENTO,
    )
    mascara = sum(1 << k for k, ok in enumerate(testes) if ok)
    passados = sum(testes)
    nota = passados * NOTA_POR_FILTRO + max(0, min(NOTA_LOGS_MAX, previsto + NOTA_LOGS_DESLOCAMENTO))
    return mascara, previsto, nota


def executar(tamanho: int, passos: int, parametros, semente: bytes = b"", catalogo: Catalogo | None = None,
             modelo: Modelo | None = None) -> tuple[bytes, int]:
    """(resultado, operações). A semente não entra na conta."""
    del semente
    validar(tamanho, passos, parametros)
    catalogo = catalogo or catalogo_embutido()
    modelo = modelo or modelo_embutido()
    inicio = parametros[1]
    corpo = bytearray()
    aprovadas = 0
    for m in catalogo.moleculas[inicio:inicio + tamanho]:
        mascara, previsto, nota = avaliar(m, parametros, modelo.pesos, catalogo.media_mili, catalogo.desvio_mili)
        aprovadas += mascara == TODOS_OS_FILTROS
        corpo += bytes([mascara]) + previsto.to_bytes(4, "big", signed=True) + nota.to_bytes(4, "big")
    cabecalho = inicio.to_bytes(4, "big") + tamanho.to_bytes(4, "big") + aprovadas.to_bytes(4, "big")
    return cabecalho + bytes(corpo), operacoes(tamanho)


def decodificar(resultado: bytes):
    """(início, quantidade, aprovadas, [(máscara, previsto, nota)]), ou None."""
    if len(resultado) < BYTES_CABECALHO or (len(resultado) - BYTES_CABECALHO) % BYTES_POR_MOLECULA:
        return None
    inicio, quantidade, aprovadas = (int.from_bytes(resultado[k:k + 4], "big") for k in (0, 4, 8))
    if quantidade * BYTES_POR_MOLECULA != len(resultado) - BYTES_CABECALHO:
        return None
    linhas = []
    for k in range(quantidade):
        p = BYTES_CABECALHO + k * BYTES_POR_MOLECULA
        linhas.append((resultado[p], int.from_bytes(resultado[p + 1:p + 5], "big", signed=True),
                       int.from_bytes(resultado[p + 5:p + 9], "big")))
    return inicio, quantidade, aprovadas, linhas


def verificar(tamanho: int, passos: int, parametros, resultado: bytes, semente: bytes = b"") -> str | None:
    """None se confere; senão, o motivo da recusa."""
    if len(resultado) != BYTES_CABECALHO + tamanho * BYTES_POR_MOLECULA:
        return "resultado com tamanho que não fecha a faixa de moléculas"
    refeito, _ = executar(tamanho, passos, parametros, semente)
    if refeito[:BYTES_CABECALHO] != resultado[:BYTES_CABECALHO]:
        return "o cabeçalho (faixa ou aprovadas) difere da recomputação"
    for k in range(tamanho):
        p = BYTES_CABECALHO + k * BYTES_POR_MOLECULA
        if refeito[p:p + BYTES_POR_MOLECULA] != resultado[p:p + BYTES_POR_MOLECULA]:
            return f"a molécula {parametros[1] + k} do catálogo difere da recomputação"
    return None


def derivar_unidade(tamanho: int, parametros, indice: int, n_catalogo: int | None = None) -> tuple[int, list[int]]:
    """(tamanho, parâmetros) da unidade `indice` de um JOB com este modelo."""
    n = n_catalogo if n_catalogo is not None else len(catalogo_embutido().moleculas)
    inicio = parametros[1] + indice * tamanho
    if inicio >= n:
        raise ValueError(f"a unidade {indice} começaria em {inicio}, depois do fim do catálogo ({n})")
    novos = list(parametros)
    novos[1] = inicio
    return min(tamanho, n - inicio), novos


def rmse_mili(pares) -> int:
    """Raiz do erro quadrático médio, em milésimos de log S, arredondada para
    baixo (raiz inteira): só para reportar."""
    pares = list(pares)
    if not pares:
        return 0
    soma = sum((a - b) ** 2 for a, b in pares)
    return _raiz_inteira(soma // len(pares))


def _raiz_inteira(n: int) -> int:
    if n < 2:
        return n
    x = 1 << ((n.bit_length() + 1) // 2)
    while True:
        y = (x + n // x) // 2
        if y >= x:
            return x
        x = y
