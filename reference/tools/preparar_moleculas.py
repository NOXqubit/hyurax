# ✝ Gênesis 2:19 — “E tudo o que Adão chamou a toda a alma vivente, isso foi o seu nome.”
"""Prepara a base de moléculas do trabalho de IA do ULTRAX.

Fonte: AqSolDB (Sorkun, Khetan e Er, Scientific Data 6, 143, 2019),
doi:10.7910/DVN/OVHAW8, licença CC0 1.0 (domínio público). Solubilidade na
água medida em laboratório, como log S (log10 de mol/L).

Uso:
    python tools/preparar_moleculas.py caminho/curated-solubility-dataset.tab

Escreve `crates/hyurax-ultrax/dados/moleculas.tsv`, que o gabarito em Python
(`hyurax/ia.py`) e o Rust leem igual. Tudo o que o treino usa sai daqui em
inteiros: nenhum ponto flutuante atravessa para o cálculo.

Filtro, para a base caber no programa e a molécula caber na tela:
  - sem ponto no SMILES (sais e misturas ficam de fora);
  - SMILES com até 90 caracteres, de 3 a 40 átomos pesados;
  - desvio entre medições (SD) até 0,5;
  - ordem pelo InChIKey, e as primeiras 2048.
"""

from __future__ import annotations

import csv
import statistics
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
SAIDA = RAIZ / "crates" / "hyurax-ultrax" / "dados" / "moleculas.tsv"
QUANTAS = 2048
ESCALA = 4096  # Q12: 1,0 vale 4096
DESCRITORES = [
    "MolWt", "MolLogP", "MolMR", "HeavyAtomCount", "NumHAcceptors",
    "NumHDonors", "NumRotatableBonds", "NumAromaticRings", "RingCount", "TPSA",
]


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
    escolhidas = boas[:QUANTAS]

    medias = {d: statistics.fmean(float(l[d]) for l in escolhidas) for d in DESCRITORES}
    desvios = {d: statistics.pstdev(float(l[d]) for l in escolhidas) or 1.0 for d in DESCRITORES}
    logs = [float(l["Solubility"]) for l in escolhidas]
    media_s, desvio_s = statistics.fmean(logs), statistics.pstdev(logs)

    def q12(v: float) -> int:
        return max(-8 * ESCALA, min(8 * ESCALA, round(v * ESCALA)))

    SAIDA.parent.mkdir(parents=True, exist_ok=True)
    with open(SAIDA, "w", encoding="utf-8", newline="\n") as s:
        s.write("# Moléculas do trabalho de IA do ULTRAX. Fonte: AqSolDB, doi:10.7910/DVN/OVHAW8, CC0 1.0.\n")
        s.write("# Gerado por reference/tools/preparar_moleculas.py. Não editar à mão.\n")
        s.write(f"# alvo: log S normalizado em Q12; log S = alvo/4096 * {round(desvio_s * 1000)}/1000 + {round(media_s * 1000)}/1000\n")
        s.write(f"#normalizacao\t{round(media_s * 1000)}\t{round(desvio_s * 1000)}\n")
        s.write("#colunas\tid\tnome\tformula\tsmiles\tmassa_mili\tlogs_mili\talvo_q12\t" + "\t".join(DESCRITORES) + "\n")
        for l in escolhidas:
            formula = l["InChI"].split("/")[1] if "/" in l["InChI"] else ""
            nome = l["Name"].strip().replace("\n", " ")[:80]
            alvo = q12((float(l["Solubility"]) - media_s) / desvio_s)
            x = [str(q12((float(l[d]) - medias[d]) / desvios[d])) for d in DESCRITORES]
            s.write("\t".join([
                l["ID"], nome, formula, l["SMILES"],
                str(round(float(l["MolWt"]) * 1000)), str(round(float(l["Solubility"]) * 1000)), str(alvo),
            ] + x) + "\n")
    print(f"{len(escolhidas)} moléculas de {len(linhas)} em {SAIDA}")


if __name__ == "__main__":
    principal(sys.argv[1])
