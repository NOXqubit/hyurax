# ✝ Eclesiastes 3:11 — “Tudo fez formoso em seu tempo.”
"""Gera os tokens do Cloud Design 2.0 para as três superfícies.

Fonte única: design/cloud-design-2.json. Saídas:

  crates/hyurax-interface/ui/tokens.css   o programa do PC (painel local)
  site/styles/tokens.css                  o site
  mobile/src/tema/tokens.ts               o aplicativo do celular

    python scripts/gerar-tokens.py            grava as três
    python scripts/gerar-tokens.py --conferir só confere (o CI usa: sai com 1
                                              se alguma saída não bate)

Sem dependência: só a biblioteca padrão.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
FONTE = RAIZ / "design" / "cloud-design-2.json"
CABECALHO = "GERADO por scripts/gerar-tokens.py a partir de design/cloud-design-2.json. Não edite à mão."


def _css_vars(pares: dict, recuo: str = "  ") -> str:
    return "\n".join(f"{recuo}--{k}: {v};" for k, v in pares.items())


def css(t: dict) -> str:
    padrao = t.get("padrao", "escuro")
    outro = "claro" if padrao == "escuro" else "escuro"
    base, alt = t["temas"][padrao], t["temas"][outro]
    esquema = {"claro": "light", "escuro": "dark"}
    medidas = dict(t["medidas"])
    fontes = {"fonte-texto": t["fontes"]["texto"], "fonte-mono": t["fontes"]["mono"]}
    return (
        f"/* {t['nome']} {t['versao']} — {CABECALHO}\n"
        f"   Tema {padrao} por padrão; data-tema=\"{outro}\" no <html> troca tudo de uma vez.\n"
        "   Com data-tema=\"sistema\", segue o sistema operacional. */\n\n"
        ":root {\n" + _css_vars(base) + "\n" + _css_vars(medidas) + "\n" + _css_vars(fontes)
        + f"\n  color-scheme: {esquema[padrao]};\n}}\n\n"
        f":root[data-tema=\"{outro}\"] {{\n" + _css_vars(alt) + f"\n  color-scheme: {esquema[outro]};\n}}\n\n"
        f"@media (prefers-color-scheme: {esquema[outro]}) {{\n  :root[data-tema=\"sistema\"] {{\n"
        + _css_vars(alt, "    ") + f"\n    color-scheme: {esquema[outro]};\n  }}\n}}\n"
    )


def _ts_nome(k: str) -> str:
    partes = k.split("-")
    return partes[0] + "".join(p.capitalize() for p in partes[1:])


def _ts_obj(pares: dict, recuo: str = "  ") -> str:
    linhas = []
    for k, v in pares.items():
        valor = v
        if isinstance(v, str) and v.endswith("px") and v[:-2].replace(".", "", 1).isdigit():
            valor = float(v[:-2]) if "." in v else int(v[:-2])
        linhas.append(f"{recuo}{_ts_nome(k)}: {json.dumps(valor, ensure_ascii=False)},")
    return "\n".join(linhas)


def ts(t: dict) -> str:
    escuro = {k: v for k, v in t["temas"]["escuro"].items() if not k.startswith("sombra")}
    claro = {k: v for k, v in t["temas"]["claro"].items() if not k.startswith("sombra")}
    return (
        f"// {t['nome']} {t['versao']} — {CABECALHO}\n\n"
        "export const escuro = {\n" + _ts_obj(escuro) + "\n} as const;\n\n"
        "export type Tema = { readonly [K in keyof typeof escuro]: string };\n\n"
        "export const claro: Tema = {\n" + _ts_obj(claro) + "\n};\n\n"
        "export const medidas = {\n" + _ts_obj(t["medidas"]) + "\n} as const;\n\n"
        "export const fontes = {\n  texto: \"Archivo\",\n  mono: \"IBM Plex Mono\",\n} as const;\n"
    )


SAIDAS = {
    RAIZ / "crates" / "hyurax-interface" / "ui" / "tokens.css": css,
    RAIZ / "site" / "styles" / "tokens.css": css,
    RAIZ / "mobile" / "src" / "tema" / "tokens.ts": ts,
}


def main() -> int:
    t = json.loads(FONTE.read_text(encoding="utf-8"))
    conferir = "--conferir" in sys.argv
    diferentes = []
    for caminho, gerar in SAIDAS.items():
        texto = gerar(t)
        atual = caminho.read_text(encoding="utf-8") if caminho.exists() else None
        if conferir:
            if atual != texto:
                diferentes.append(caminho.relative_to(RAIZ).as_posix())
            continue
        caminho.parent.mkdir(parents=True, exist_ok=True)
        caminho.write_bytes(texto.encode("utf-8"))
        print(f"  gravado {caminho.relative_to(RAIZ).as_posix()}")
    if diferentes:
        print("tokens fora de dia (rode python scripts/gerar-tokens.py):", ", ".join(diferentes))
        return 1
    if conferir:
        print("tokens em dia com design/cloud-design-2.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
