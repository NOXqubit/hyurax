# ✝ 2 Pedro 3:10 — “Mas o Dia do Senhor virá como o ladrão de noite.”
"""Grava um versículo, ou uma frase do autor, no topo de cada arquivo de código.

Uso:
    python scripts/versiculos.py              grava ou atualiza a linha de cada arquivo
    python scripts/versiculos.py --conferir   só confere; sai com erro se faltar alguma

A linha é um comentário em texto aberto. O compilador do Rust e o
interpretador do Python a ignoram: ela não muda o programa, o consenso nem os
vetores. A lista completa, com o porquê, está em docs/MENSAGEM.md.

Cada arquivo recebe sempre a mesma mensagem, escolhida pelo caminho dele.
Assim, um arquivo novo não embaralha a mensagem dos outros.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
MARCA = "✝"
# Marca de ordem de bytes do UTF-8. Escrita por codigo, e nao como caractere,
# porque o caractere e invisivel no editor.
BOM = chr(0xFEFF)
COMENTARIO = {".rs": "//", ".py": "#", ".ps1": "#"}

# Versículos na tradução de João Ferreira de Almeida, às vezes encurtados.
# Referência None é frase do autor.
MENSAGENS: list[tuple[str | None, str]] = [
    ("Daniel 12:4", "Tu, porém, Daniel, fecha estas palavras e sela este livro, até ao fim "
                    "do tempo; muitos correrão de uma parte para outra, e a ciência se "
                    "multiplicará."),
    ("Mateus 24:36", "Porém daquele dia e hora ninguém sabe, nem os anjos do céu, mas "
                     "unicamente meu Pai."),
    ("Lucas 21:28", "Quando essas coisas começarem a acontecer, olhai para cima e levantai a "
                    "vossa cabeça, porque a vossa redenção está próxima."),
    ("2 Pedro 3:10", "Mas o Dia do Senhor virá como o ladrão de noite."),
    ("Apocalipse 21:4", "E Deus limpará de seus olhos toda lágrima, e não haverá mais morte, "
                        "nem pranto, nem clamor, nem dor."),
    ("Jeremias 32:15", "Ainda se comprarão casas, e campos, e vinhas nesta terra."),
    ("Gênesis 41:35-36", "Ajuntem toda a comida destes bons anos que vêm; e esta comida será "
                         "para provimento da terra, para os sete anos de fome."),
    ("Provérbios 22:3", "O prudente vê o mal e esconde-se; mas os simples passam e sofrem a pena."),
    ("Isaías 26:20", "Vai, pois, povo meu, entra nos teus quartos e fecha as tuas portas sobre "
                     "ti; esconde-te só por um momento, até que passe a ira."),
    ("Provérbios 6:6-8", "Vai ter com a formiga, ó preguiçoso; olha para os seus caminhos e sê "
                         "sábio: no verão prepara o seu pão."),
    (None, "Compre terras. Plante. Busque abrigo. Planeje bunkers."),
]


def linha(comentario: str, referencia: str | None, texto: str) -> str:
    if referencia is None:
        return f"{comentario} {MARCA} {texto}"
    return f"{comentario} {MARCA} {referencia} — “{texto}”"


def indice(caminho: str) -> int:
    resumo = hashlib.sha256(caminho.encode("utf-8")).digest()
    return int.from_bytes(resumo[:4], "big") % len(MENSAGENS)


def arquivos_de_codigo() -> list[Path]:
    saida = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=RAIZ, capture_output=True, check=True,
    ).stdout
    nomes = sorted({n.decode("utf-8") for n in saida.split(b"\0") if n})
    return [RAIZ / n for n in nomes if Path(n).suffix in COMENTARIO and (RAIZ / n).is_file()]


def aplicar(so_conferir: bool) -> int:
    arquivos = arquivos_de_codigo()
    faltando = alterados = 0
    for arquivo in arquivos:
        caminho = arquivo.relative_to(RAIZ).as_posix()
        comentario = COMENTARIO[arquivo.suffix]
        desejada = linha(comentario, *MENSAGENS[indice(caminho)])

        conteudo = arquivo.read_text(encoding="utf-8")
        # A marca de ordem de bytes (BOM), que alguns editores do Windows
        # gravam, só é aceita pelo Python como primeiro caractere do arquivo.
        # Inserir a linha antes dela quebrou a sintaxe do test_04 uma vez; ela
        # fica sempre no começo, antes do versículo.
        bom = BOM if conteudo.startswith(BOM) else ""
        linhas = conteudo[len(bom):].split("\n")
        # Linha "#!" precisa continuar sendo a primeira do arquivo.
        pos = 1 if linhas and linhas[0].startswith("#!") else 0
        atual = linhas[pos] if pos < len(linhas) else ""
        ja_tem = atual.startswith(f"{comentario} {MARCA} ")

        if ja_tem and atual == desejada:
            continue
        if so_conferir:
            faltando += 1
            print(f"sem a linha certa: {caminho}")
            continue
        if ja_tem:
            linhas[pos] = desejada
        else:
            linhas.insert(pos, desejada)
        arquivo.write_text(bom + "\n".join(linhas), encoding="utf-8", newline="\n")
        alterados += 1

    if so_conferir:
        print(f"{len(arquivos) - faltando} de {len(arquivos)} arquivos com a linha certa")
        return 1 if faltando else 0
    print(f"{alterados} arquivos gravados, {len(arquivos) - alterados} já estavam certos")
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description="Versículos no topo de cada arquivo de código.")
    parser.add_argument("--conferir", action="store_true", help="só confere, sem mudar nada")
    sys.exit(aplicar(parser.parse_args().conferir))


if __name__ == "__main__":
    main()
