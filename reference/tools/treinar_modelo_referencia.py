# ✝ Provérbios 18:15 — “O coração do entendido adquire o conhecimento, e o ouvido dos sábios busca a ciência.”
"""Treina o modelo de referência da triagem do ULTRAX e grava
`crates/hyurax-ultrax/dados/modelo-referencia.txt`.

Uso:
    python tools/treinar_modelo_referencia.py             # treina e grava
    python tools/treinar_modelo_referencia.py --conferir  # treina de novo e compara com o arquivo

O treino é o de `hyurax/ia.py`, sem nada a mais: a mesma rede 10 -> 16 -> 1,
em inteiros Q12, sobre as 2048 moléculas de `moleculas.tsv` (1638 de treino e
410 de validação). Qualquer nó refaz o mesmo modelo bit a bit com a semente,
o lote e os passos gravados no arquivo.

Como a configuração foi escolhida (27/09/2026, Python 3.13 num Atom): lotes
de 32, 64, 128 e 256 com o teto de 4096 passos, e sementes 1 a 3 na melhor
configuração; ficou a de menor erro de validação. Escolher pelo erro de
validação deixa esse erro um pouco otimista. Por isso o arquivo também
reporta o erro nas moléculas do catálogo que ficaram **fora** da base do
treino, que nem o treino nem a escolha viram: é o número honesto.

Todos os erros reportados são contra o log S **medido** da AqSolDB, em log S
(log10 mol/L): raiz do erro quadrático médio e erro absoluto médio. Para
comparar, o arquivo traz também o erro de quem prevê sempre a média.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from hyurax import crypto, ia, triagem  # noqa: E402

ROTULO_SEMENTE = "hyurax triagem modelo de referencia {n}"
SEMENTE_N = 1
LOTE = 64
PASSOS = 4096


def semente() -> bytes:
    return crypto.H(ROTULO_SEMENTE.format(n=SEMENTE_N).encode("ascii"))


def _log_s(mili: int) -> str:
    """Milésimos de log S como texto com vírgula decimal (só para mostrar)."""
    sinal = "-" if mili < 0 else ""
    return f"{sinal}{abs(mili) // 1000},{abs(mili) % 1000:03d}"


def erros(pesos, catalogo: triagem.Catalogo) -> dict[str, dict[str, int]]:
    """Por grupo do catálogo: quantas, raiz do erro quadrático médio, erro
    absoluto médio e a raiz do erro de quem prevê sempre a média, em milésimos."""
    grupos: dict[str, list[tuple[int, int]]] = {"treino": [], "validacao": [], "fora": [], "catalogo": []}
    base = len(ia.carregar())
    for i, m in enumerate(catalogo.moleculas):
        y, _ = ia.prever(pesos, list(m.x))
        par = (triagem.logs_de(y, catalogo.media_mili, catalogo.desvio_mili), m.logs_mili)
        grupo = "fora" if i >= base else ("validacao" if i % 5 == 0 else "treino")
        grupos[grupo].append(par)
        grupos["catalogo"].append(par)
    saida = {}
    for nome, pares in grupos.items():
        saida[nome] = {
            "n": len(pares),
            "rmse": triagem.rmse_mili(pares),
            "mae": sum(abs(a - b) for a, b in pares) // max(1, len(pares)),
            "rmse_media": triagem.rmse_mili((catalogo.media_mili, b) for _, b in pares),
        }
    return saida


def texto_do_modelo(pesos, erro: int, catalogo: triagem.Catalogo) -> str:
    e = erros(pesos, catalogo)
    rotulos = {
        "treino": "treino",
        "validacao": "validação",
        "fora": "fora da base (nunca vistas)",
        "catalogo": "catálogo inteiro",
    }
    comentarios = [
        "Modelo de referência da triagem do ULTRAX: a rede 10 -> 16 -> 1 de reference/hyurax/ia.py, pesos em Q12 (1,0 = 4096).",
        "Gerado por reference/tools/treinar_modelo_referencia.py. Não editar à mão.",
        f"Treino: ia.treinar(semente, {LOTE}, {PASSOS}) sobre as 2048 moléculas de moleculas.tsv (1638 de treino, 410 de validação).",
        f"Semente: H(\"{ROTULO_SEMENTE.format(n=SEMENTE_N)}\").",
        f"Erro de validação do treino: {erro} em Q12 (erro quadrático médio do alvo normalizado).",
        "Erro contra o log S medido da AqSolDB, em log S: raiz do erro quadrático médio | erro absoluto médio | raiz de quem prevê sempre a média",
    ]
    for chave, rotulo in rotulos.items():
        g = e[chave]
        comentarios.append(
            f"  {rotulo} ({g['n']}): {_log_s(g['rmse'])} | {_log_s(g['mae'])} | {_log_s(g['rmse_media'])}"
        )
    comentarios += [
        "É um modelo estatístico pequeno sobre 10 descritores: ordena candidatos, não substitui a medida.",
        "Não prevê afinidade com alvo, estabilidade, toxicidade nem atividade biológica.",
    ]
    return triagem.escrever_modelo(pesos, semente(), LOTE, PASSOS, erro, comentarios)


def principal(argv: list[str]) -> int:
    base = ia.carregar()
    pesos, erro, _ = ia.treinar(semente(), LOTE, PASSOS, base)
    texto = texto_do_modelo(pesos, erro, triagem.carregar_catalogo())
    if "--conferir" in argv:
        atual = triagem.MODELO_ARQUIVO.read_text(encoding="utf-8")
        if atual != texto:
            print("o modelo refeito DIFERE do arquivo")
            return 1
        print("o modelo refeito é igual ao arquivo, byte a byte")
        return 0
    triagem.MODELO_ARQUIVO.write_bytes(texto.encode("utf-8"))
    print(texto[: texto.index("#semente")])
    return 0


if __name__ == "__main__":
    sys.exit(principal(sys.argv[1:]))
