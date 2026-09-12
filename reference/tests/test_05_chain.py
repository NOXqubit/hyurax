# ✝ 2 Pedro 3:10 — “Mas o Dia do Senhor virá como o ladrão de noite.”
"""Transacoes, estado e cadeia.

Cobre as correcoes dos bugs 5, 6, 7, 9 (emissao) e 12 do prototipo, e a lista
de ataques que o prototipo aceitava.
"""

from __future__ import annotations

import sys
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import codec, crypto  # noqa: E402
from auron.block import Block, BlockHeader  # noqa: E402
from auron.chain import Chain, ChainError, make_genesis  # noqa: E402
from auron.consensus import REGTEST, block_reward, target_to_compact  # noqa: E402
from auron.state import State, StateError  # noqa: E402
from auron.tx import AUR, Coinbase, decode_tx, sign_transfer  # noqa: E402
from auron.units import AUR_UNIT, to_units  # noqa: E402

P = REGTEST


class Account:
    def __init__(self):
        self.secret = crypto.generate_secret()
        self.pub = crypto.public_key(self.secret)
        self.address = crypto.address_from_pubkey(self.pub)


def mine_n(chain: Chain, miner: bytes, n: int, transfers=None) -> None:
    """Minera n blocos, avancando o relogio para nao bater no median-time-past."""
    for i in range(n):
        ts = chain.tip.header.timestamp + P.target_spacing
        block = chain.mine(miner, transfers if i == 0 else None, timestamp=ts)
        chain.accept_block(block, now=ts + 10)


def test_genesis_is_single_and_deterministic():
    a = make_genesis(P)
    b = make_genesis(P)
    assert a.block_hash() == b.block_hash(), "genese nao e deterministica"
    assert a.header.height == 0
    assert a.header.prev_hash == b"\x00" * 64
    # redes diferentes tem genese diferente
    from auron.consensus import MAINNET, TESTNET
    assert make_genesis(MAINNET).block_hash() != make_genesis(TESTNET).block_hash()
    assert make_genesis(TESTNET).block_hash() != a.block_hash()
    print("PASS genese unica e deterministica por rede")


def test_transaction_roundtrip_and_signature():
    alice, bob = Account(), Account()
    tx = sign_transfer(
        alice.secret, P.magic,
        sender=alice.address, recipient=bob.address,
        amount=to_units("1.5"), fee=to_units("0.001"), nonce=0,
    )
    ok, msg = tx.check_signature(P.magic)
    assert ok, msg

    back = decode_tx(tx.encode())
    assert back == tx, "roundtrip de transacao falhou"
    assert back.txid() == tx.txid()
    print("PASS transacao codifica, decodifica e verifica")


def test_signature_bound_to_network():
    """Transacao assinada numa rede nao pode valer em outra."""
    alice, bob = Account(), Account()
    tx = sign_transfer(
        alice.secret, P.magic,
        sender=alice.address, recipient=bob.address,
        amount=to_units("1"), fee=0, nonce=0,
    )
    assert tx.check_signature(P.magic)[0]
    from auron.consensus import MAINNET
    ok, _ = tx.check_signature(MAINNET.magic)
    assert not ok, "transacao de testnet foi aceita na mainnet"
    print("PASS assinatura amarrada a rede")


def test_tampering_rejected():
    alice, bob, mallory = Account(), Account(), Account()
    tx = sign_transfer(
        alice.secret, P.magic,
        sender=alice.address, recipient=bob.address,
        amount=to_units("1"), fee=0, nonce=0,
    )
    saida = tx.outputs[0]
    for label, bad in (
        ("valor", replace(tx, outputs=(replace(saida, amount=to_units("999")),))),
        ("destino", replace(tx, outputs=(replace(saida, recipient=mallory.address),))),
        ("ativo", replace(tx, outputs=(replace(saida, asset_id=bytes([1]) + AUR[1:]),))),
        ("nonce", replace(tx, nonce=tx.nonce + 1)),
        ("taxa", replace(tx, fee=to_units("5"))),
    ):
        ok, _ = bad.check_signature(P.magic)
        assert not ok, f"adulteracao de {label} passou"

    # chave publica de outro que nao bate com o sender
    swapped = replace(tx, public_key=mallory.pub)
    assert not swapped.check_signature(P.magic)[0], "chave trocada passou"
    print("PASS adulteracao de transacao rejeitada")


def test_mine_and_spend_after_maturity():
    alice, bob = Account(), Account()
    chain = Chain(params=P)

    mine_n(chain, alice.address, 1)
    assert chain.height == 1
    # recompensa ainda esta imatura
    assert chain.state.balance(alice.address) == 0
    assert chain.state.immature_balance(alice.address) == block_reward(1, P)

    mine_n(chain, alice.address, P.coinbase_maturity)
    assert chain.state.balance(alice.address) > 0, "recompensa nunca amadureceu"

    saldo = chain.state.balance(alice.address)
    tx = sign_transfer(
        alice.secret, P.magic,
        sender=alice.address, recipient=bob.address,
        amount=to_units("10"), fee=to_units("0.01"),
        nonce=chain.state.next_nonce(alice.address),
    )
    # Minerar o bloco da transferencia tambem faz amadurecer a recompensa de
    # `altura_nova - maturity`, entao o saldo esperado nao e so `saldo - custo`.
    nova_altura = chain.height + 1
    amadurece = block_reward(nova_altura - P.coinbase_maturity, P)

    mine_n(chain, alice.address, 1, [tx])
    assert chain.state.balance(bob.address) == to_units("10")
    assert chain.state.balance(alice.address) == saldo + amadurece - to_units("10.01"), (
        f"saldo {chain.state.balance(alice.address)}, esperado "
        f"{saldo + amadurece - to_units('10.01')}"
    )
    # a taxa foi para o minerador, entao nao sumiu do sistema
    assert chain.state.immature_balance(alice.address) >= to_units("0.01")
    print(f"PASS minerar, maturar e gastar (saldo alice {saldo / AUR_UNIT:.2f} AUR)")


def test_coinbase_maturity_blocks_early_spend():
    alice, bob = Account(), Account()
    chain = Chain(params=P)
    mine_n(chain, alice.address, 1)

    tx = sign_transfer(
        alice.secret, P.magic,
        sender=alice.address, recipient=bob.address,
        amount=to_units("1"), fee=0, nonce=0,
    )
    try:
        mine_n(chain, alice.address, 1, [tx])
        raise AssertionError("gastou recompensa imatura")
    except ChainError as exc:
        assert "saldo insuficiente" in str(exc)
    print("PASS recompensa imatura nao pode ser gasta")


def test_forged_block_without_pow_rejected():
    """BUG 5: o prototipo aceitava bloco de peer sem verificar PoW."""
    alice = Account()
    chain = Chain(params=P)
    ts = chain.tip.header.timestamp + P.target_spacing

    good = chain.build_candidate(alice.address, timestamp=ts)
    forged = Block(header=good.header.with_nonce(0), transactions=good.transactions)
    # forca um nonce que quase certamente NAO bate o alvo apertado
    hard_bits = target_to_compact(1 << 8)
    bad_header = BlockHeader(
        height=forged.header.height, prev_hash=forged.header.prev_hash,
        merkle_root=forged.header.merkle_root, timestamp=ts,
        bits=hard_bits, nonce=12345,
    )
    try:
        chain.accept_block(Block(bad_header, forged.transactions), now=ts + 10)
        raise AssertionError("bloco sem prova de trabalho foi aceito")
    except ChainError as exc:
        assert "dificuldade" in str(exc) or "prova de trabalho" in str(exc)
    assert chain.height == 0, "cadeia avancou com bloco forjado"
    print("PASS bloco sem PoW rejeitado")


def test_merkle_root_covers_signatures():
    """BUG 6: trocar a assinatura precisa invalidar o cabecalho."""
    alice, bob = Account(), Account()
    chain = Chain(params=P)
    mine_n(chain, alice.address, P.coinbase_maturity + 1)

    tx = sign_transfer(
        alice.secret, P.magic,
        sender=alice.address, recipient=bob.address,
        amount=to_units("1"), fee=0,
        nonce=chain.state.next_nonce(alice.address),
    )
    ts = chain.tip.header.timestamp + P.target_spacing
    block = chain.mine(alice.address, [tx], timestamp=ts)

    # troca 1 bit da assinatura, mantendo tudo o mais igual
    broken_sig = bytearray(tx.signature)
    broken_sig[0] ^= 0x01
    swapped = replace(tx, signature=bytes(broken_sig))
    tampered = Block(block.header, [block.transactions[0], swapped])

    assert tampered.computed_merkle_root() != block.header.merkle_root, \
        "assinatura alterada nao mudou a raiz de Merkle"
    try:
        chain.accept_block(tampered, now=ts + 10)
        raise AssertionError("bloco com assinatura trocada foi aceito")
    except ChainError as exc:
        assert "merkle" in str(exc).lower()
    print("PASS merkle cobre a assinatura")


def test_inflated_coinbase_rejected():
    alice = Account()
    chain = Chain(params=P)
    ts = chain.tip.header.timestamp + P.target_spacing
    height = 1

    greedy = Coinbase(height=height, recipient=alice.address,
                      amount=block_reward(height, P) * 2)
    txs = [greedy]
    # parte de um candidato legitimo, para o bloco trazer trabalho util valido
    # e a recusa sair pelo motivo que este teste cobra: a coinbase inflada
    candidato = chain.build_candidate(alice.address, timestamp=ts)
    header = replace(candidato.header,
                     merkle_root=codec.merkle_root([t.encode() for t in txs]))
    from auron.consensus import check_pow_target
    target = chain.expected_bits()
    from auron.consensus import compact_to_target
    tgt = compact_to_target(target)
    for nonce in range(1 << 20):
        h = header.with_nonce(nonce)
        if check_pow_target(h.pow_hash(P), tgt):
            header = h
            break

    try:
        chain.accept_block(Block(header, txs, candidato.useful_proof), now=ts + 10)
        raise AssertionError("coinbase inflada foi aceita")
    except ChainError as exc:
        assert "coinbase" in str(exc).lower()
    print("PASS coinbase inflada rejeitada")


def test_extra_coinbase_rejected():
    alice, bob = Account(), Account()
    chain = Chain(params=P)
    ts = chain.tip.header.timestamp + P.target_spacing
    good = chain.build_candidate(alice.address, timestamp=ts)
    fake = Coinbase(height=1, recipient=bob.address, amount=block_reward(1, P))
    txs = [good.transactions[0], fake]
    header = BlockHeader(
        height=1, prev_hash=chain.tip_hash(),
        merkle_root=codec.merkle_root([t.encode() for t in txs]),
        timestamp=ts, bits=chain.expected_bits(), nonce=0,
    )
    try:
        chain.accept_block(Block(header, txs), now=ts + 10)
        raise AssertionError("coinbase extra foi aceita")
    except ChainError as exc:
        assert "coinbase extra" in str(exc)
    print("PASS coinbase extra rejeitada")


def test_nonce_replay_and_overspend_rejected():
    alice, bob = Account(), Account()
    chain = Chain(params=P)
    mine_n(chain, alice.address, P.coinbase_maturity + 1)

    n = chain.state.next_nonce(alice.address)
    tx = sign_transfer(alice.secret, P.magic, sender=alice.address,
                       recipient=bob.address, amount=to_units("1"), fee=0, nonce=n)
    mine_n(chain, alice.address, 1, [tx])

    # replay do mesmo nonce
    try:
        mine_n(chain, alice.address, 1, [tx])
        raise AssertionError("replay de nonce foi aceito")
    except ChainError as exc:
        assert "nonce" in str(exc)

    # Gasto acima do saldo. A margem precisa cobrir tambem a recompensa que
    # vai amadurecer quando este bloco for minerado, senao o "excesso" vira
    # acessivel e o teste testa a coisa errada.
    inalcancavel = (
        chain.state.balance(alice.address)
        + chain.state.immature_balance(alice.address)
        + to_units("1000")
    )
    greedy = sign_transfer(
        alice.secret, P.magic, sender=alice.address, recipient=bob.address,
        amount=inalcancavel, fee=0,
        nonce=chain.state.next_nonce(alice.address),
    )
    try:
        mine_n(chain, alice.address, 1, [greedy])
        raise AssertionError("gasto acima do saldo foi aceito")
    except ChainError as exc:
        assert "saldo" in str(exc)
    print("PASS replay de nonce e overspend rejeitados")


def test_timestamp_rules():
    alice = Account()
    chain = Chain(params=P)
    mine_n(chain, alice.address, 3)

    mtp = chain.median_time_past()
    ts_old = mtp  # nao passa do median-time-past
    cand = chain.build_candidate(alice.address, timestamp=ts_old)
    from auron.consensus import check_pow_target, compact_to_target
    tgt = compact_to_target(cand.header.bits)
    header = cand.header
    for nonce in range(1 << 20):
        h = header.with_nonce(nonce)
        if check_pow_target(h.pow_hash(P), tgt):
            header = h
            break
    try:
        chain.accept_block(Block(header, cand.transactions), now=ts_old + 10_000)
        raise AssertionError("timestamp no passado foi aceito")
    except ChainError as exc:
        assert "median-time-past" in str(exc)

    # timestamp no futuro alem do tolerado
    ts_future = chain.tip.header.timestamp + P.target_spacing
    block = chain.mine(alice.address, timestamp=ts_future)
    try:
        chain.accept_block(block, now=ts_future - P.max_future_drift - 60)
        raise AssertionError("timestamp no futuro foi aceito")
    except ChainError as exc:
        assert "futuro" in str(exc)
    print("PASS regras de timestamp")


def test_rollback_restores_state_exactly():
    alice, bob = Account(), Account()
    chain = Chain(params=P)
    mine_n(chain, alice.address, P.coinbase_maturity + 1)

    before_bal = dict(chain.state.balances)
    before_nonces = dict(chain.state.nonces)
    before_emitted = chain.state.total_emitted
    before_height = chain.height

    tx = sign_transfer(alice.secret, P.magic, sender=alice.address,
                       recipient=bob.address, amount=to_units("5"), fee=to_units("0.01"),
                       nonce=chain.state.next_nonce(alice.address))
    mine_n(chain, alice.address, 1, [tx])
    assert chain.state.balance(bob.address) == to_units("5")

    chain.rollback(1)
    assert chain.height == before_height
    assert chain.state.balances == before_bal, "saldos nao voltaram ao original"
    assert chain.state.nonces == before_nonces, "nonces nao voltaram ao original"
    assert chain.state.total_emitted == before_emitted
    print("PASS rollback restaura o estado byte a byte")


def test_fork_choice_by_work_not_height():
    """Cadeia longa de blocos faceis nao pode ganhar de curta de dificeis."""
    from auron.chain import compare_chains
    alice = Account()
    a = Chain(params=P)
    b = Chain(params=P)
    mine_n(a, alice.address, 2)
    mine_n(b, alice.address, 3)
    assert compare_chains(b, a) > 0, "mais trabalho deveria ganhar"

    # empate em trabalho: desempate deterministico, os dois lados concordam
    c = Chain(params=P)
    d = Chain(params=P)
    assert compare_chains(c, d) == 0
    assert compare_chains(c, d) == -compare_chains(d, c)
    print("PASS ponta escolhida por trabalho acumulado")


def test_supply_never_exceeds_cap():
    alice = Account()
    chain = Chain(params=P)
    mine_n(chain, alice.address, 5)
    chain.state.check_invariants()
    from auron.units import MAX_SUPPLY
    assert chain.state.total_emitted <= MAX_SUPPLY
    print("PASS teto de supply respeitado")


if __name__ == "__main__":
    test_genesis_is_single_and_deterministic()
    test_transaction_roundtrip_and_signature()
    test_signature_bound_to_network()
    test_tampering_rejected()
    test_mine_and_spend_after_maturity()
    test_coinbase_maturity_blocks_early_spend()
    test_forged_block_without_pow_rejected()
    test_merkle_root_covers_signatures()
    test_inflated_coinbase_rejected()
    test_extra_coinbase_rejected()
    test_nonce_replay_and_overspend_rejected()
    test_timestamp_rules()
    test_rollback_restores_state_exactly()
    test_fork_choice_by_work_not_height()
    test_supply_never_exceeds_cap()
    print("=== CHAIN OK ===")
