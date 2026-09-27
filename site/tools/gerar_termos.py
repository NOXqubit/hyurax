# Gera site/termos.html a partir de docs/TERMOS-DE-USO.md, o mesmo texto que o
# instalador e o programa mostram. Rode de novo sempre que os termos mudarem:
#   python site/tools/gerar_termos.py
# O conversor é o mínimo que o texto usa (títulos, parágrafos, listas com um
# nível de sub-lista, negrito e código), igual ao de crates/hyurax-no/src/termos.rs.

from __future__ import annotations

import html
import re
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
ORIGEM = RAIZ / "docs" / "TERMOS-DE-USO.md"
DESTINO = RAIZ / "site" / "termos.html"


def em_linha(t: str) -> str:
    t = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", t)
    return re.sub(r"`(.+?)`", r"<code>\1</code>", t)


def converter(md: str) -> str:
    """Igual a `termos::html` do programa: o item da lista é juntado inteiro
    antes de converter (negrito que quebra a linha fecha no lugar certo), e
    há um nível de sub-lista."""
    if "-->" in md:
        md = md.split("-->", 1)[1]
    saida: list[str] = []
    paragrafo: list[str] = []
    em_lista = False
    item: list[str] = []
    subitens: list[list[str]] = []

    def fechar_paragrafo():
        if paragrafo:
            saida.append(f"<p>{em_linha(' '.join(paragrafo))}</p>")
            paragrafo.clear()

    def escrever_item():
        if not item and not subitens:
            return
        s = f"<li>{em_linha(' '.join(item))}"
        if subitens:
            s += "<ul>" + "".join(f"<li>{em_linha(' '.join(x))}</li>" for x in subitens) + "</ul>"
        saida.append(s + "</li>")
        item.clear()
        subitens.clear()

    for linha in md.splitlines():
        crua = linha.rstrip()
        recuo = len(crua) - len(crua.lstrip())
        l = html.escape(crua.strip(), quote=True)
        if recuo == 0 and l.startswith("- "):
            fechar_paragrafo()
            if em_lista:
                escrever_item()
            else:
                saida.append("<ul>")
                em_lista = True
            item.append(l[2:])
            continue
        if em_lista and recuo >= 2 and l:
            if l.startswith("- ") and recuo < 4:
                subitens.append([l[2:]])
            elif subitens and recuo >= 4:
                subitens[-1].append(l)
            else:
                item.append(l)
            continue
        if em_lista:
            escrever_item()
            saida.append("</ul>")
            em_lista = False
        if l.startswith("## "):
            fechar_paragrafo()
            saida.append(f"<h2>{em_linha(l[3:])}</h2>")
        elif l.startswith("# "):
            fechar_paragrafo()
            saida.append(f"<h1>{em_linha(l[2:])}</h1>")
        elif not l:
            fechar_paragrafo()
        else:
            paragrafo.append(l)
    fechar_paragrafo()
    if em_lista:
        escrever_item()
        saida.append("</ul>")
    return "\n".join(saida)


PAGINA = """<!doctype html>
<html lang="pt-BR">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Termos de uso · Hyurax</title>
<meta name="description" content="Termos de uso e isenção de responsabilidade do programa Hyurax: rede de teste, HYX sem valor, sem garantia.">
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'none'; style-src 'self'; font-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; form-action 'none'">
<meta name="referrer" content="strict-origin-when-cross-origin">
<meta name="color-scheme" content="light">
<link rel="icon" href="assets/favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="styles/main.css">
</head>
<body class="texto-legal">
<main class="dentro">
<p><a class="voltar" href="./">← Hyurax</a></p>
<!-- Gerado por site/tools/gerar_termos.py a partir de docs/TERMOS-DE-USO.md. Não editar à mão. -->
{corpo}
<p class="versao">O texto de referência está em docs/TERMOS-DE-USO.md, no repositório do projeto. O instalador e o programa mostram este mesmo texto.</p>
</main>
</body>
</html>
"""


def principal() -> None:
    DESTINO.write_text(PAGINA.replace("{corpo}", converter(ORIGEM.read_text(encoding="utf-8"))), encoding="utf-8", newline="\n")
    print(f"escrito {DESTINO}")


if __name__ == "__main__":
    principal()
