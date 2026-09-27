# Gera site/termos.html a partir de docs/TERMOS-DE-USO.md, o mesmo texto que o
# instalador e o programa mostram. Rode de novo sempre que os termos mudarem:
#   python site/tools/gerar_termos.py
# O conversor é o mínimo que o texto usa (títulos, parágrafos, listas,
# negrito e código), igual ao de crates/hyurax-no/src/termos.rs.

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
    if "-->" in md:
        md = md.split("-->", 1)[1]
    saida, paragrafo, em_lista = [], [], False

    def fechar():
        if paragrafo:
            saida.append(f"<p>{em_linha(' '.join(paragrafo))}</p>")
            paragrafo.clear()

    for linha in md.splitlines():
        crua = linha.rstrip()
        l = html.escape(crua.strip(), quote=True)
        if l.startswith("- "):
            fechar()
            if not em_lista:
                saida.append("<ul>")
                em_lista = True
            saida.append(f"<li>{em_linha(l[2:])}")
            continue
        if em_lista and crua.startswith("  ") and l:
            saida[-1] += " " + em_linha(l)
            continue
        if em_lista:
            saida.append("</ul>")
            em_lista = False
        if l.startswith("## "):
            fechar()
            saida.append(f"<h2>{em_linha(l[3:])}</h2>")
        elif l.startswith("# "):
            fechar()
            saida.append(f"<h1>{em_linha(l[2:])}</h1>")
        elif not l:
            fechar()
        else:
            paragrafo.append(l)
    fechar()
    if em_lista:
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
