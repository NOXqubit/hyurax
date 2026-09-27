# ✝ Gênesis 2:19 — “E tudo o que Adão chamou a toda a alma vivente, isso foi o seu nome.”
"""Prepara a base de moléculas do trabalho de IA e o catálogo da triagem do ULTRAX.

Fonte: AqSolDB (Sorkun, Khetan e Er, Scientific Data 6, 143, 2019),
doi:10.7910/DVN/OVHAW8, licença CC0 1.0 (domínio público). Solubilidade na
água medida em laboratório, como log S (log10 de mol/L). Os descritores
(massa, logP, área polar...) são os que a própria AqSolDB publica, calculados
pelos autores com o RDKit: este programa não os recalcula.

Uso:
    python tools/preparar_moleculas.py caminho/curated-solubility-dataset.tab

Escreve dois arquivos, que o gabarito em Python e o Rust leem igual. Tudo o
que o cálculo usa sai daqui em inteiros: nenhum ponto flutuante atravessa.

1. `crates/hyurax-ultrax/dados/moleculas.tsv`, a base do treino de IA
   (`hyurax/ia.py`): as primeiras 2048 moléculas.
2. `crates/hyurax-ultrax/dados/catalogo.tsv`, o catálogo da triagem
   (`hyurax/triagem.py`): **todas** as moléculas que passam o filtro, na mesma
   ordem. As 2048 primeiras linhas são as de `moleculas.tsv`. Cada linha leva
   os descritores brutos em inteiros (massa, logP, refratividade molar e área
   polar em milésimos; contagens em unidades), o log S medido em milésimos, e
   os mesmos descritores normalizados em Q12 com a média e o desvio **das
   2048** (os da base do treino): é o que deixa o modelo de IA valer para o
   catálogo inteiro.

Filtro estrutural, para a molécula caber no programa e na tela:
  - sem ponto no SMILES (sais e misturas ficam de fora);
  - SMILES com até 90 caracteres, de 3 a 40 átomos pesados;
  - desvio entre medições (SD) até 0,5;
  - ordem pelo InChIKey (sem repetição na AqSolDB).

`moleculas.tsv` não pode mudar: o treino de IA e os vetores dele dependem de
cada byte. Quem rodar este script confere com `git diff`.
"""

from __future__ import annotations

import csv
import statistics
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
DADOS = RAIZ / "crates" / "hyurax-ultrax" / "dados"
SAIDA = DADOS / "moleculas.tsv"
SAIDA_CATALOGO = DADOS / "catalogo.tsv"
QUANTAS = 2048
ESCALA = 4096  # Q12: 1,0 vale 4096
DESCRITORES = [
    "MolWt", "MolLogP", "MolMR", "HeavyAtomCount", "NumHAcceptors",
    "NumHDonors", "NumRotatableBonds", "NumAromaticRings", "RingCount", "TPSA",
]
# Descritores contínuos: gravados em milésimos. Os outros são contagens.
EM_MILESIMOS = {"MolWt", "MolLogP", "MolMR", "TPSA"}
COLUNAS_BRUTAS = [
    "massa_mili", "logp_mili", "mr_mili", "pesados", "aceptores",
    "doadores", "rotaveis", "aromaticos", "aneis", "tpsa_mili",
]


def _bruto(linha: dict, d: str) -> int:
    v = float(linha[d])
    if d in EM_MILESIMOS:
        return round(v * 1000)
    if not v.is_integer():
        raise ValueError(f"{linha['ID']}: {d} = {v} não é contagem inteira")
    return int(v)


def principal(arquivo: str) -> None:
    with open(arquivo, encoding="utf-8", newline="") as f:
        linhas = list(csv.DictReader(f, delimiter="\t"))
    boas = [
        l for l in linhas
        if "." not in l["SMILES"]
        and len(l["SMILES"]) <= 90
        and 3 <= float(l["HeavyAtomCount"]) <= 40
        and float(l["SD"]) <= 0.5
        and "\t" not in l["Name"]
    ]
    boas.sort(key=lambda l: l["InChIKey"])
    if len({l["InChIKey"] for l in boas}) != len(boas):
        raise ValueError("InChIKey repetido: a ordem do catálogo ficaria ambígua")
    escolhidas = boas[:QUANTAS]

    medias = {d: statistics.fmean(float(l[d]) for l in escolhidas) for d in DESCRITORES}
    desvios = {d: statistics.pstdev(float(l[d]) for l in escolhidas) or 1.0 for d in DESCRITORES}
    logs = [float(l["Solubility"]) for l in escolhidas]
    media_s, desvio_s = statistics.fmean(logs), statistics.pstdev(logs)

    def q12(v: float) -> int:
        return max(-8 * ESCALA, min(8 * ESCALA, round(v * ESCALA)))

    def formula(l: dict) -> str:
        return l["InChI"].split("/")[1] if "/" in l["InChI"] else ""

    def nome(l: dict) -> str:
        return l["Name"].strip().replace("\n", " ")[:80]

    def features(l: dict) -> list[str]:
        return [str(q12((float(l[d]) - medias[d]) / desvios[d])) for d in DESCRITORES]

    SAIDA.parent.mkdir(parents=True, exist_ok=True)
    with open(SAIDA, "w", encoding="utf-8", newline="\n") as s:
        s.write("# Moléculas do trabalho de IA do ULTRAX. Fonte: AqSolDB, doi:10.7910/DVN/OVHAW8, CC0 1.0.\n")
        s.write("# Gerado por reference/tools/preparar_moleculas.py. Não editar à mão.\n")
        s.write(f"# alvo: log S normalizado em Q12; log S = alvo/4096 * {round(desvio_s * 1000)}/1000 + {round(media_s * 1000)}/1000\n")
        s.write(f"#normalizacao\t{round(media_s * 1000)}\t{round(desvio_s * 1000)}\n")
        s.write("#colunas\tid\tnome\tformula\tsmiles\tmassa_mili\tlogs_mili\talvo_q12\t" + "\t".join(DESCRITORES) + "\n")
        for l in escolhidas:
            alvo = q12((float(l["Solubility"]) - media_s) / desvio_s)
            s.write("\t".join([
                l["ID"], nome(l), formula(l), l["SMILES"],
                str(round(float(l["MolWt"]) * 1000)), str(round(float(l["Solubility"]) * 1000)), str(alvo),
            ] + features(l)) + "\n")
    print(f"{len(escolhidas)} moléculas de {len(linhas)} em {SAIDA}")

    with open(SAIDA_CATALOGO, "w", encoding="utf-8", newline="\n") as s:
        s.write("# Catálogo da triagem do ULTRAX. Fonte: AqSolDB, doi:10.7910/DVN/OVHAW8, CC0 1.0.\n")
        s.write("# Gerado por reference/tools/preparar_moleculas.py. Não editar à mão.\n")
        s.write(f"# {len(boas)} moléculas, todas as que passam o filtro estrutural, na ordem do InChIKey;"
                f" as {QUANTAS} primeiras são as de moleculas.tsv, na mesma ordem.\n")
        s.write("# Descritores da AqSolDB (RDKit), brutos: massa (g/mol), logP, refratividade molar e área polar"
                " (TPSA, Å²) em milésimos; contagens em unidades. logs_mili: log S medido (log10 mol/L) em milésimos.\n")
        s.write(f"# x_*: os mesmos descritores normalizados em Q12 com a média e o desvio das {QUANTAS} de"
                " moleculas.tsv, os do modelo de IA.\n")
        s.write(f"#normalizacao\t{round(media_s * 1000)}\t{round(desvio_s * 1000)}\n")
        s.write("#colunas\tid\tnome\tformula\tsmiles\tlogs_mili\t" + "\t".join(COLUNAS_BRUTAS) + "\t"
                + "\t".join(f"x_{d}" for d in DESCRITORES) + "\n")
        for l in boas:
            s.write("\t".join(
                [l["ID"], nome(l), formula(l), l["SMILES"], str(round(float(l["Solubility"]) * 1000))]
                + [str(_bruto(l, d)) for d in DESCRITORES]
                + features(l)
            ) + "\n")
    print(f"{len(boas)} moléculas de {len(linhas)} em {SAIDA_CATALOGO}")


if __name__ == "__main__":
    principal(sys.argv[1])
