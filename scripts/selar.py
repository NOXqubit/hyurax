"""Grava e lê os selos do Auron: uma mensagem cifrada, em comentário, no topo
de cada arquivo de código.

Uso:
    python scripts/selar.py          grava um selo em cada arquivo que ainda
                                     não tem, e mantém os que já estão certos
    python scripts/selar.py --ler    mostra a mensagem de cada arquivo

A senha é pedida no terminal e nunca é gravada em lugar nenhum. As mensagens
vêm de um arquivo local, fora do repositório: .toolchain/selos/mensagens.txt,
procurado na raiz do projeto e nas pastas acima dela.

O formato está em docs/SELOS.md. Os selos são só comentário: nenhuma linha do
nó os lê, e eles não mudam o programa compilado, o consenso nem os vetores.
"""

from __future__ import annotations

import argparse
import getpass
import re
import secrets
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RAIZ / "reference"))

from auron import argon2, crypto  # noqa: E402

VERSAO = "AURON-SELO-v1"
DOMINIO = VERSAO.encode("ascii")

# Derivação lenta de propósito: as cifras ficam públicas no repositório, então
# a senha precisa resistir a quem tentar adivinhar testando palavras em massa.
# Uns 2 minutos no Atom de desenvolvimento, uma vez por uso.
SAL = DOMINIO + b"|sal"
TEMPO = 3
MEMORIA_KIB = 16384
FAIXAS = 1
SENHA_MINIMA = 12

COMENTARIO = {".rs": "//", ".py": "#", ".ps1": "#"}
DOC = RAIZ / "docs" / "SELOS.md"
LINHA_VERIFICADOR = re.compile(r"^Verificador da senha: .*$", re.MULTILINE)


def achar_mensagens() -> Path:
    for base in (RAIZ, *RAIZ.parents):
        candidato = base / ".toolchain" / "selos" / "mensagens.txt"
        if candidato.is_file():
            return candidato
    sys.exit("Não achei .toolchain/selos/mensagens.txt na raiz do projeto nem acima dela.")


def ler_mensagens(arquivo: Path) -> list[bytes]:
    mensagens = []
    for linha in arquivo.read_text(encoding="utf-8").splitlines():
        linha = linha.strip()
        if linha and not linha.startswith("#"):
            mensagens.append(linha.encode("utf-8"))
    if not mensagens:
        sys.exit(f"{arquivo} não tem nenhuma mensagem.")
    return mensagens


def derivar_chave(senha: str) -> bytes:
    print("Derivando a chave com Argon2id. Leva uns 2 minutos nesta máquina...", flush=True)
    return argon2.argon2id(
        senha.encode("utf-8"), SAL,
        time_cost=TEMPO, memory_kib=MEMORIA_KIB, parallelism=FAIXAS, tag_length=64,
    )


def verificador(chave: bytes) -> str:
    """Confirma que a senha é a mesma dos selos existentes, sem revelá-la."""
    return crypto.H(DOMINIO + b"|verificador|" + chave)[:16].hex()


def fluxo(chave: bytes, nonce: bytes, tamanho: int) -> bytes:
    # O XOF da AURON-SPEC-01, com o nonce no domínio: cada selo tem o próprio
    # fluxo, e a mesma mensagem em dois arquivos dá duas cifras diferentes.
    return crypto.xof(chave, tamanho, DOMINIO + b"|fluxo|" + nonce)


def etiqueta(chave: bytes, nonce: bytes, mensagem: bytes) -> bytes:
    # Depende da chave: sem a senha, ninguém consegue testar as mensagens
    # candidatas contra a etiqueta para descobrir qual é qual.
    return crypto.H(DOMINIO + b"|etiqueta|" + chave + nonce + mensagem)[:16]


def xor(a: bytes, b: bytes) -> bytes:
    return bytes(x ^ y for x, y in zip(a, b))


def selar(chave: bytes, mensagem: bytes) -> str:
    nonce = secrets.token_bytes(16)
    cifra = xor(mensagem, fluxo(chave, nonce, len(mensagem)))
    return f"{VERSAO} {nonce.hex()} {cifra.hex()} {etiqueta(chave, nonce, mensagem).hex()}"


def abrir(chave: bytes, selo: str) -> bytes | None:
    partes = selo.split()
    if len(partes) != 4 or partes[0] != VERSAO:
        return None
    try:
        nonce, cifra, marca = (bytes.fromhex(p) for p in partes[1:])
    except ValueError:
        return None
    mensagem = xor(cifra, fluxo(chave, nonce, len(cifra)))
    return mensagem if etiqueta(chave, nonce, mensagem) == marca else None


def arquivos_de_codigo() -> list[Path]:
    saida = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=RAIZ, capture_output=True, check=True,
    ).stdout
    nomes = sorted({n.decode("utf-8") for n in saida.split(b"\0") if n})
    return [RAIZ / n for n in nomes if Path(n).suffix in COMENTARIO and (RAIZ / n).is_file()]


def selo_do_arquivo(arquivo: Path, texto: str) -> str | None:
    """O selo da primeira linha do arquivo, ou None se ela não for um selo."""
    prefixo = f"{COMENTARIO[arquivo.suffix]} {VERSAO} "
    primeira = texto.partition("\n")[0]
    if primeira.startswith(prefixo):
        return primeira[len(COMENTARIO[arquivo.suffix]) + 1:]
    return None


def verificador_registrado() -> str | None:
    if not DOC.is_file():
        return None
    achado = LINHA_VERIFICADOR.search(DOC.read_text(encoding="utf-8"))
    if achado is None:
        return None
    valor = achado.group(0).split(":", 1)[1].strip().strip("`.")
    return valor if re.fullmatch(r"[0-9a-f]{32}", valor) else None


def registrar_verificador(valor: str) -> None:
    texto = DOC.read_text(encoding="utf-8")
    if LINHA_VERIFICADOR.search(texto) is None:
        sys.exit(f"{DOC} não tem a linha 'Verificador da senha:'.")
    novo = LINHA_VERIFICADOR.sub(f"Verificador da senha: `{valor}`", texto, count=1)
    DOC.write_text(novo, encoding="utf-8", newline="\n")


def pedir_chave(confirmar: bool) -> bytes:
    senha = getpass.getpass("Senha dos selos: ")
    if confirmar:
        if len(senha) < SENHA_MINIMA:
            sys.exit(f"Use uma senha de pelo menos {SENHA_MINIMA} caracteres.")
        if getpass.getpass("Repita a senha: ") != senha:
            sys.exit("As duas senhas não conferem.")
    chave = derivar_chave(senha)
    registrado = verificador_registrado()
    if registrado is not None and registrado != verificador(chave):
        sys.exit("Esta senha é diferente da usada nos selos que já existem.")
    return chave


def gravar() -> None:
    mensagens = ler_mensagens(achar_mensagens())
    chave = pedir_chave(confirmar=True)
    arquivos = arquivos_de_codigo()
    novos = mantidos = 0
    for i, arquivo in enumerate(arquivos):
        texto = arquivo.read_text(encoding="utf-8")
        selo = selo_do_arquivo(arquivo, texto)
        if selo is not None:
            if abrir(chave, selo) in mensagens:
                mantidos += 1
                continue
            # Selo inválido, ou de uma mensagem que saiu da lista: regrava.
            texto = texto.partition("\n")[2]
        linha = f"{COMENTARIO[arquivo.suffix]} {selar(chave, mensagens[i % len(mensagens)])}\n"
        arquivo.write_text(linha + texto, encoding="utf-8", newline="\n")
        novos += 1
    registrar_verificador(verificador(chave))
    print(f"{novos} selos gravados e {mantidos} mantidos, em {len(arquivos)} arquivos de código.")


def ler() -> None:
    chave = pedir_chave(confirmar=False)
    for arquivo in arquivos_de_codigo():
        nome = arquivo.relative_to(RAIZ).as_posix()
        selo = selo_do_arquivo(arquivo, arquivo.read_text(encoding="utf-8"))
        if selo is None:
            print(f"{nome}: sem selo")
            continue
        mensagem = abrir(chave, selo)
        print(f"{nome}: {mensagem.decode('utf-8') if mensagem else 'SELO INVÁLIDO'}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Selos do Auron.")
    parser.add_argument("--ler", action="store_true", help="mostra a mensagem de cada arquivo")
    if parser.parse_args().ler:
        ler()
    else:
        gravar()


if __name__ == "__main__":
    main()
