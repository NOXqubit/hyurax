# ✝ Provérbios 27:12 — “O prudente vê o mal e esconde-se; mas os simples passam e sofrem a pena.”
"""As seis correcoes da revisao de ataque de 11/09/2026.

Cada teste reproduz o ataque ou a divergencia que a revisao encontrou, e
verifica que agora o programa recusa do jeito certo: com o erro que quem chama
sabe tratar, e igual dos dois lados (gabarito e Rust).

1. extra_nonce longo demais entrava pela leitura e so estourava depois.
2. LWMA aceitava tempo negativo, e alternar horarios afrouxava a dificuldade.
3. texto de valor gigante fazia o Python levantar ValueError, nao AmountError.
4. coinbase de valor zero: a especificacao aceitava e o gabarito recusava.
5. recarga do disco nao tratava erro de transacao.
6. Freivalds aceitava C fora da faixa, e a conta estourava o int64 em silencio.
"""

from __future__ import annotations

import sys
import tempfile
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402

from auron import codec, consensus, crypto, store, utrax  # noqa: E402
from auron.block import Block  # noqa: E402
from auron.chain import Chain, ChainError  # noqa: E402
from auron.consensus import REGTEST, block_reward, check_pow_target  # noqa: E402
from auron.store import StoreError, save_chain  # noqa: E402
from auron.tx import MAX_EXTRA_NONCE, Coinbase, TxError, decode_tx  # noqa: E402
from auron.units import MAX_AMOUNT_TEXT, AmountError, to_units  # noqa: E402

P = REGTEST
ENDERECO = bytes(crypto.ADDRESS_LEN)


def mina(chain: Chain, minerador: bytes, n: int = 1) -> None:
    for _ in range(n):
        ts = chain.tip.header.timestamp + P.target_spacing
        bloco = chain.mine(minerador, None, timestamp=ts)
        chain.accept_block(bloco, now=ts + 10)


def bloco_com_coinbase(chain: Chain, coinbase: Coinbase) -> Block:
    """Monta um bloco valido na ponta, com a coinbase dada, e acha o nonce."""
    candidato = chain.build_candidate(
        coinbase.recipient, None,
        timestamp=chain.tip.header.timestamp + P.target_spacing,
    )
    txs = [coinbase] + list(candidato.transactions[1:])
    raiz = Block(header=candidato.header, transactions=txs).computed_merkle_root()
    cabecalho = replace(candidato.header, merkle_root=raiz)
    alvo = cabecalho.target()
    for nonce in range(1 << 32):
        h = cabecalho.with_nonce(nonce)
        if check_pow_target(h.pow_hash(P), alvo):
            return Block(header=h, transactions=txs)
    raise AssertionError("nao achou nonce")


def coinbase_bruta(altura: int, valor: int, extra: bytes) -> bytes:
    return (
        codec.enc_u8(0)
        + codec.enc_u16(1)
        + codec.enc_u64(altura)
        + codec.enc_fixed(ENDERECO, crypto.ADDRESS_LEN)
        + codec.enc_u64(valor)
        + codec.enc_bytes(extra)
    )


# ---------------------------------------------------------------- 1. coinbase
def test_extra_nonce_longo_recusado_na_leitura():
    """A leitura recusa o mesmo que a escrita, e no mesmo lugar."""
    try:
        decode_tx(coinbase_bruta(7, 50, b"A" * (MAX_EXTRA_NONCE + 1)))
    except TxError as exc:
        assert "extra_nonce" in str(exc), exc
    else:
        raise AssertionError("extra_nonce de 65 bytes foi aceito na leitura")

    aceita = decode_tx(coinbase_bruta(7, 50, b"A" * MAX_EXTRA_NONCE))
    assert isinstance(aceita, Coinbase)
    assert len(aceita.extra_nonce) == MAX_EXTRA_NONCE
    print("PASS extra_nonce acima de 64 bytes e recusado na leitura")


def test_erro_de_transacao_no_bloco_vira_ChainError():
    """Erro de transacao dentro do bloco sai como ChainError, nao como TxError."""
    chain = Chain(params=P)
    # Um bloco legitimo, com a coinbase trocada por uma que nao se deixa
    # codificar. E o que um peer malicioso manda pela rede.
    valido = bloco_com_coinbase(chain, Coinbase(height=1, recipient=ENDERECO, amount=1))
    impossivel = Coinbase(height=1, recipient=ENDERECO, amount=1,
                          extra_nonce=b"x" * (MAX_EXTRA_NONCE + 1))
    bloco = Block(header=valido.header,
                  transactions=[impossivel] + list(valido.transactions[1:]))
    try:
        chain.validate_block(bloco, now=bloco.header.timestamp + 10)
    except ChainError as exc:
        assert "transação inválida" in str(exc), exc
    except TxError as exc:  # pragma: no cover — era exatamente o bug
        raise AssertionError(f"TxError escapou de validate_block: {exc}")
    else:
        raise AssertionError("bloco com coinbase impossivel foi aceito")
    print("PASS erro de transacao no bloco vira ChainError")


# -------------------------------------------------------------------- 2. LWMA
def lwma_antigo(ts, tg, p):
    """A formula anterior a correcao, para medir o tamanho do ataque."""
    janela = min(len(ts) - 1, p.lwma_window)
    ts = ts[-(janela + 1):]
    tg = tg[-janela:]
    k = janela * (janela + 1) // 2 * p.target_spacing
    somado = 0
    for i in range(1, janela + 1):
        solvetime = ts[i] - ts[i - 1]
        solvetime = max(-6 * p.target_spacing, min(solvetime, 6 * p.target_spacing))
        somado += solvetime * i
    somado = max(somado, k // 3)
    candidato = (sum(tg) // janela) * somado // k
    anterior = tg[-1]
    candidato = min(max(candidato, anterior // 2), anterior * 2)
    return consensus.normalize_target(min(candidato, p.max_target))


def test_lwma_ignora_horario_para_tras():
    """Horario para tras nao pode inflar a dificuldade da rede inteira.

    Alvo menor quer dizer dificuldade maior. Na regra antiga, intercalar
    blocos com horario antigo — cada um valido, porque o median-time-past
    continua andando — cortava o alvo ate o limite de metade por bloco, o que
    trava a cadeia para todo mundo sem precisar de maioria de poder.
    """
    n = P.lwma_window + 1
    alvo = consensus.normalize_target((1 << 240) - 1)
    s = P.target_spacing
    base = 1_000_000
    honesto = [base + i * s for i in range(n)]
    alternado = [t - 3 * s if i % 2 else t + 3 * s for i, t in enumerate(honesto)]
    fundo = [base + i * s if i % 2 else base for i in range(n)]

    referencia = consensus.next_target(honesto, [alvo] * n, P)

    # A regra antiga cedia ao ataque; a nova quase nao se move.
    for nome, ts in (("alternado", alternado), ("fundo", fundo)):
        antigo = lwma_antigo(ts, [alvo] * n, P) / referencia
        novo = consensus.next_target(ts, [alvo] * n, P) / referencia
        assert antigo <= 0.7, f"{nome}: a regra antiga deveria ceder, deu {antigo:.3f}"
        assert 0.9 <= novo <= 1.1, (
            f"{nome}: o alvo andou {novo:.3f} do honesto com a regra nova"
        )

    # Todos os blocos no mesmo segundo: tempo zero, nunca negativo.
    iguais = [base] * n
    assert consensus.next_target(iguais, [alvo] * n, P) > 0

    # Sequencia honesta nao muda de valor com a regra nova.
    assert lwma_antigo(honesto, [alvo] * n, P) == referencia
    print("PASS LWMA usa horarios nao decrescentes")


# ---------------------------------------------------------------- 3. unidades
def test_texto_de_valor_gigante_e_AmountError():
    """Antes: ValueError do int() do Python, que ninguem tratava."""
    for texto in ("1" * (MAX_AMOUNT_TEXT + 1), "9" * 5000, "0" * 4400 + "1"):
        try:
            to_units(texto)
        except AmountError as exc:
            assert "caracteres" in str(exc), exc
        else:
            raise AssertionError(f"texto de {len(texto)} caracteres foi aceito")

    # O limite nao atrapalha valor legitimo: 63 caracteres passam.
    assert to_units("0" * 55 + "21000000") == to_units("21000000")
    print("PASS texto de valor acima de 64 caracteres e AmountError")


# ---------------------------------------------------------------- 4. coinbase
def test_coinbase_de_valor_zero_e_valida():
    """A especificacao so fala em maximo; o gabarito recusava o zero."""
    chain = Chain(params=P)
    emitido_antes = chain.state.total_emitted   # a genese ja emitiu 1 unidade
    bloco = bloco_com_coinbase(chain, Coinbase(height=1, recipient=ENDERECO, amount=0))
    chain.accept_block(bloco, now=bloco.header.timestamp + 10)
    assert chain.height == 1
    assert chain.state.total_emitted == emitido_antes,         "coinbase de valor zero nao pode emitir nada"

    demais = Coinbase(height=2, recipient=ENDERECO, amount=block_reward(2, P) + 1)
    bloco2 = bloco_com_coinbase(chain, demais)
    try:
        chain.accept_block(bloco2, now=bloco2.header.timestamp + 10)
    except ChainError as exc:
        assert "máximo" in str(exc), exc
    else:
        raise AssertionError("coinbase acima do subsidio foi aceita")
    print("PASS coinbase aceita 0 <= amount <= subsidio + taxas")


# ------------------------------------------------------------------- 5. store
def test_arquivo_adulterado_vira_StoreError():
    """Transacao malformada no arquivo e erro de arquivo, nao queda do programa."""
    chain = Chain(params=P)
    mina(chain, ENDERECO, 2)
    with tempfile.TemporaryDirectory() as pasta:
        caminho = Path(pasta) / "cadeia.auron"
        save_chain(chain, caminho)
        bruto = caminho.read_bytes()

        # A ultima coinbase e achada pelos proprios bytes, e o prefixo de
        # tamanho do extra_nonce vira 256, acima do maximo. Sem a correcao,
        # isto saia como TxError e derrubava quem estivesse recarregando.
        ultima = chain.tip.transactions[0].encode()
        pos = bruto.rfind(ultima)
        assert pos > 0, "nao achei a coinbase no arquivo"
        fim = pos + len(ultima)
        caminho.write_bytes(bruto[:fim - 4] + codec.enc_u32(256) + bruto[fim:])

        try:
            store.load_chain(caminho)
        except StoreError as exc:
            assert "bloco" in str(exc) or "cabeçalho" in str(exc), exc
        except TxError as exc:  # pragma: no cover — era exatamente o bug
            raise AssertionError(f"TxError escapou de load_chain: {exc}")
        else:
            raise AssertionError("arquivo adulterado foi aceito")
    print("PASS arquivo adulterado vira StoreError")


# ------------------------------------------------------------------- 6. UTRAX
def test_freivalds_recusa_resultado_fora_da_faixa():
    """C perto de 2^63 fazia a conta estourar o int64 e dar a volta em silencio."""
    semente = b"ataque"
    tamanho = 8
    a, b = utrax.generate_matrices(semente, tamanho)
    certo = a @ b

    gigante = certo.copy()
    gigante[0, 0] = np.int64(2**62)
    assert not utrax.verify_matrix(semente, tamanho, gigante.tobytes()), \
        "resultado com entrada gigante foi aceito"

    negativo = certo.copy()
    negativo[1, 1] = np.int64(-1)
    assert not utrax.verify_matrix(semente, tamanho, negativo.tobytes()), \
        "resultado com entrada negativa foi aceito"

    assert utrax.verify_matrix(semente, tamanho, certo.tobytes()), \
        "resultado honesto foi recusado"

    limite = tamanho * (utrax.MATRIX_ENTRY_MAX - 1) ** 2
    assert int(certo.max()) <= limite, "a faixa nao cobre o produto honesto"
    print("PASS Freivalds recusa C fora da faixa possivel")


TESTS = [v for k, v in sorted(globals().items()) if k.startswith("test_")]

if __name__ == "__main__":
    for teste in TESTS:
        teste()
    print(f"\n{len(TESTS)} testes da revisao de seguranca passaram")
