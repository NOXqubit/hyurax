# Gera site/downloads/payload.js e site/downloads/manifesto.js.
#
# O pacote do código sai de `git archive HEAD`, então só entra o que está commitado.
# A pasta site/downloads fica de fora do zip, para o pacote não conter a si mesmo.
# Rodar da raiz do repositório:  python site/tools/build_downloads.py
import base64
import io
import json
import subprocess
import zipfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
SAIDA = RAIZ / "site" / "downloads"


def git(*args: str) -> bytes:
    return subprocess.run(["git", "-C", str(RAIZ), *args], capture_output=True, check=True).stdout


def main() -> None:
    commit = git("rev-parse", "--short", "HEAD").decode().strip()
    zip_bytes = git("archive", "--format=zip", "--prefix=auron/", "HEAD", "--", ".", ":(exclude)site/downloads")
    with zipfile.ZipFile(io.BytesIO(zip_bytes)) as z:
        arquivos = len([n for n in z.namelist() if not n.endswith("/")])

    def doc(caminho: str) -> str:
        return git("show", f"HEAD:{caminho}").decode("utf-8")

    carga = {
        "codigo": {"nome": "auron-codigo.zip", "b64": base64.b64encode(zip_bytes).decode()},
        "apresentacao": {"nome": "AURON-APRESENTACAO.md", "texto": doc("docs/AURON-APRESENTACAO.md")},
        "roteiro": {"nome": "ROTEIRO-VIDEO.md", "texto": doc("docs/ROTEIRO-VIDEO.md")},
    }
    manifesto = {"commit": commit, "kb": round(len(zip_bytes) / 1024), "arquivos": arquivos}

    SAIDA.mkdir(parents=True, exist_ok=True)
    cabecalho = "// Gerado por site/tools/build_downloads.py. Não editar à mão.\n"
    (SAIDA / "payload.js").write_text(cabecalho + "export default " + json.dumps(carga, ensure_ascii=False) + ";\n", encoding="utf-8")
    (SAIDA / "manifesto.js").write_text(cabecalho + "export default " + json.dumps(manifesto) + ";\n", encoding="utf-8")
    print(f"commit {commit} · zip {len(zip_bytes)} bytes · {arquivos} arquivos")


if __name__ == "__main__":
    main()
