# ✝ Provérbios 22:1 — “Mais digno de ser escolhido é o bom nome do que as muitas riquezas.”
"""Identidade do projeto, num lugar só. Espelho de `crates/auron-identidade`.

Dois papéis que não se misturam:

- EXIBIÇÃO: o que pessoas leem. Mudar não mexe em consenso.
- PROTOCOLO: rótulos que entram em hash, assinatura e aperto de mão. Todos
  derivam de RAIZ. Mudar RAIZ é criar uma rede nova: os vetores são regerados
  e a gênese muda.

Nada daqui vem de variável de ambiente nem de arquivo de configuração: dois
nós configurados diferente se partiriam em duas redes.
"""

# --- exibição -----------------------------------------------------------------
PROJETO = "Auron"
REDE = "Auron Network"
MOEDA = "Auron"
TICKER = "AUR"

# --- protocolo ----------------------------------------------------------------
RAIZ = "AURON"
PREFIXO_REDE = "auron"
MAGIC_MAINNET = b"AURM"
MAGIC_TESTNET = b"AURT"
MAGIC_REGTEST = b"AURR"
STORE_MAGIC = b"AURONDB1"
ESPEC = f"{RAIZ}-SPEC-01"


def rotulo(sufixo: str) -> bytes:
    """Rótulo de protocolo: `RAIZ-sufixo`, em ASCII."""
    return f"{RAIZ}-{sufixo}".encode("ascii")


def preencher_16(dados: bytes) -> bytes:
    """Completa com zeros até 16 bytes; recusa o que passa de 16."""
    if len(dados) > 16:
        raise ValueError("rótulo de protocolo maior que 16 bytes")
    return dados + bytes(16 - len(dados))


def nome_da_rede(tipo: str) -> str:
    """`mainnet` vira `<prefixo>-mainnet`."""
    return f"{PREFIXO_REDE}-{tipo}"
