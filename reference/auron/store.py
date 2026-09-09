"""AURON — persistência da cadeia.

BUG 9 (parte) — o protótipo não persistia nada. `blocks`, `balances` e
`nonces` eram dicionários em memória. Reiniciar o processo apagava a cadeia
inteira. Uma moeda que perde o livro-razão quando o computador desliga não é
uma moeda.

Formato: cabeçalho de arquivo, depois os blocos em sequência, cada um com
prefixo de tamanho. É o mesmo formato canônico do `codec`, então o arquivo
gravado pelo Python é lido pelo Rust sem tradução.

Ao carregar, a cadeia é RECONSTRUÍDA passando por `accept_block`, ou seja,
tudo é revalidado. Um arquivo adulterado no disco não vira estado válido.
`trust_pow=True` existe só para recarga local rápida, e está separado de
propósito: confiar no próprio disco é uma decisão, não um padrão.
"""

from __future__ import annotations

from pathlib import Path

from . import codec
from .block import Block
from .chain import Chain, ChainError
from .consensus import NETWORKS, ChainParams

STORE_MAGIC = b"AURONDB1"


class StoreError(Exception):
    """Arquivo de cadeia corrompido ou incompatível."""


def save_chain(chain: Chain, path: str | Path) -> int:
    """Grava a cadeia. Devolve quantos blocos foram escritos.

    A gênese não é gravada: ela é derivada dos parâmetros da rede. Gravá-la
    permitiria trocar a gênese editando o arquivo.
    """
    path = Path(path)
    parts = [
        STORE_MAGIC,
        codec.enc_str(chain.params.name),
        codec.enc_u64(len(chain.entries) - 1),
    ]
    for entry in chain.entries[1:]:
        parts.append(codec.enc_bytes(entry.block.encode()))

    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_bytes(b"".join(parts))
    tmp.replace(path)  # troca atômica: nunca deixa um arquivo meio escrito
    return len(chain.entries) - 1


def load_chain(path: str | Path, *, trust_pow: bool = False,
               params: ChainParams | None = None) -> Chain:
    """Lê e RECONSTRÓI a cadeia, revalidando cada bloco."""
    path = Path(path)
    if not path.exists():
        raise StoreError(f"arquivo não encontrado: {path}")

    data = path.read_bytes()
    if not data.startswith(STORE_MAGIC):
        raise StoreError("arquivo não é uma cadeia Auron")

    r = codec.Reader(data[len(STORE_MAGIC):])
    try:
        network = r.string()
        count = r.u64()
    except codec.CodecError as exc:
        raise StoreError(f"cabeçalho corrompido: {exc}") from exc

    if params is None:
        params = NETWORKS.get(network)
        if params is None:
            raise StoreError(f"rede desconhecida no arquivo: {network}")
    elif params.name != network:
        raise StoreError(
            f"arquivo é da rede {network}, mas foi pedida {params.name}"
        )

    chain = Chain(params=params)
    for index in range(count):
        try:
            raw = r.var_bytes()
            block = Block.decode(raw)
        except codec.CodecError as exc:
            raise StoreError(f"bloco {index + 1} corrompido: {exc}") from exc

        try:
            if trust_pow:
                _accept_trusting_pow(chain, block)
            else:
                # `now` no futuro distante: ao recarregar do disco, a regra de
                # "timestamp no futuro" não faz sentido, mas todas as outras
                # continuam valendo.
                chain.accept_block(block, now=block.header.timestamp + 10**9)
        except ChainError as exc:
            raise StoreError(
                f"bloco {index + 1} rejeitado ao recarregar: {exc}"
            ) from exc

    try:
        r.finish()
    except codec.CodecError as exc:
        raise StoreError(f"sobra de dados no fim do arquivo: {exc}") from exc

    return chain


def _accept_trusting_pow(chain: Chain, block: Block) -> None:
    """Recarga rápida: pula só o Argon2id, mantendo todo o resto.

    Estado, assinaturas, Merkle, coinbase e encadeamento continuam sendo
    verificados. O único atalho é não refazer a prova de trabalho, porque ela
    já foi feita quando o bloco entrou pela primeira vez.
    """
    from .consensus import check_pow_target

    original = check_pow_target
    import auron.chain as chain_module

    chain_module.check_pow_target = lambda *_args, **_kwargs: True
    try:
        chain.accept_block(block, now=block.header.timestamp + 10**9)
    finally:
        chain_module.check_pow_target = original
