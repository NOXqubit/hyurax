# ✝ Daniel 12:4 — “Tu, porém, Daniel, fecha estas palavras e sela este livro, até ao fim do tempo; muitos correrão de uma parte para outra, e a ciência se multiplicará.”
"""Transferencia versao 2: varios ativos e varias saidas.

A arquitetura (docs/AURON-DIRECT-RESONANCE.md, paragrafo 19) exige que a
transacao nasca preparada para mais de um ativo. Estes testes cobrem as regras
novas e os ataques contra elas: contagem de saidas, ordem e repeticao, ativo
desconhecido, saida para si mesmo, soma que estoura, saldo que so falta na
soma, dominio de assinatura e desfazer.
"""

from __future__ import annotations

import sys
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import codec, crypto  # noqa: E402
from auron.chain import Chain, ChainError  # noqa: E402
from auron.consensus import REGTEST, block_reward  # noqa: E402
from auron.state import State, StateError  # noqa: E402
from auron.tx import (  # noqa: E402
    AUR, MAX_OUTPUTS, Output, Transfer, TxError, decode_tx, sign_transfer,
    sign_transfer_outputs,
)
from auron.units import MAX_AMOUNT, to_units  # noqa: E402

P = REGTEST
OUTRO_ATIVO = bytes([1]) + bytes(31)


class Account:
    def __init__(self):
        self.secret = crypto.generate_secret()
        self.pub = crypto.public_key(self.secret)
        self.address = crypto.address_from_pubkey(self.pub)


def mine_n(chain: Chain, miner: bytes, n: int, transfers=None) -> None:
    for i in range(n):
        ts = chain.tip.header.timestamp + P.target_spacing
        block = chain.mine(miner, transfers if i == 0 else None, timestamp=ts)
        chain.accept_block(block, now=ts + 10)


def ordenadas(*saidas: Output) -> tuple:
    return tuple(sorted(saidas, key=lambda o: (o.recipient, o.asset_id)))


def cadeia_com_saldo(dono: Account) -> Chain:
    chain = Chain(params=P)
    mine_n(chain, dono.address, P.coinbase_maturity + 1)
    return chain


def test_multi_output_roundtrip_and_apply():
    alice, bob, carol = Account(), Account(), Account()
    chain = cadeia_com_saldo(alice)
    saidas = ordenadas(
        Output(bob.address, AUR, to_units("2")),
        Output(carol.address, AUR, to_units("0.5")),
    )
    tx = sign_transfer_outputs(
        alice.secret, P.magic, sender=alice.address, outputs=saidas,
        fee=to_units("0.01"), nonce=chain.state.next_nonce(alice.address),
    )
    ok, msg = tx.check_signature(P.magic)
    assert ok, msg
    assert decode_tx(tx.encode()) == tx, "roundtrip com varias saidas falhou"
    assert tx.costs() == {AUR: to_units("2.51")}

    antes = chain.state.balance(alice.address)
    # Alice minera o bloco da propria transferencia, e ele faz amadurecer uma
    # recompensa antiga dela. A taxa volta para ela como coinbase imatura.
    amadurece = block_reward(chain.height + 1 - P.coinbase_maturity, P)
    mine_n(chain, alice.address, 1, [tx])
    assert chain.state.balance(bob.address) == to_units("2")
    assert chain.state.balance(carol.address) == to_units("0.5")
    esperado = antes + amadurece - to_units("2.51")
    assert chain.state.balance(alice.address) == esperado, (
        f"alice tem {chain.state.balance(alice.address)}, esperado {esperado}"
    )
    print("PASS transferencia com varias saidas codifica, decodifica e se aplica")


def test_output_count_bounds():
    alice, bob = Account(), Account()
    vazia = Transfer(sender=alice.address, outputs=(), fee=0, nonce=0,
                     public_key=alice.pub)
    for label, func in (("codificar zero saidas", vazia.encode),
                        ("codificar 17 saidas", replace(
                            vazia, outputs=tuple(Output(bob.address, AUR, 1)
                                                 for _ in range(MAX_OUTPUTS + 1))).encode)):
        try:
            func()
            raise AssertionError(f"{label} passou")
        except TxError as exc:
            assert "saídas" in str(exc)
    assert not vazia.check_signature(P.magic)[0]

    # O leitor recusa a contagem antes de ler qualquer saida.
    cabeca = (codec.enc_u8(1) + codec.enc_u16(2) + codec.enc_u8(1)
              + codec.enc_fixed(alice.address, 20) + codec.enc_u64(0) + codec.enc_u64(0))
    for contagem in (0, MAX_OUTPUTS + 1, 2**32 - 1):
        try:
            decode_tx(cabeca + codec.enc_u32(contagem))
            raise AssertionError(f"contagem {contagem} foi aceita")
        except TxError as exc:
            assert f"tem {contagem}" in str(exc), str(exc)
    print("PASS de 1 a 16 saidas, contagem recusada antes de ler")


def test_outputs_must_be_sorted_and_unique():
    alice, bob, carol = Account(), Account(), Account()
    certas = ordenadas(Output(bob.address, AUR, 1), Output(carol.address, AUR, 1))
    invertidas = (certas[1], certas[0])
    repetidas = (certas[0], certas[0])
    for label, saidas in (("fora de ordem", invertidas), ("repetidas", repetidas)):
        tx = sign_transfer_outputs(alice.secret, P.magic, sender=alice.address,
                                   outputs=saidas, fee=0, nonce=0)
        ok, msg = tx.check_signature(P.magic)
        assert not ok and "fora de ordem ou repetidas" in msg, f"{label}: {msg}"
    tx = sign_transfer_outputs(alice.secret, P.magic, sender=alice.address,
                               outputs=certas, fee=0, nonce=0)
    assert tx.check_signature(P.magic)[0]
    print("PASS saidas em ordem estrita, sem repeticao")


def test_unknown_asset_rejected():
    alice, bob = Account(), Account()
    tx = sign_transfer(alice.secret, P.magic, sender=alice.address,
                       recipient=bob.address, amount=1, fee=0, nonce=0,
                       asset_id=OUTRO_ATIVO)
    ok, msg = tx.check_signature(P.magic)
    assert not ok and "ativo desconhecido" in msg, msg

    chain = cadeia_com_saldo(alice)
    tx = sign_transfer(alice.secret, P.magic, sender=alice.address,
                       recipient=bob.address, amount=1, fee=0,
                       nonce=chain.state.next_nonce(alice.address), asset_id=OUTRO_ATIVO)
    try:
        mine_n(chain, alice.address, 1, [tx])
        raise AssertionError("ativo sem regra de emissao foi aceito")
    except ChainError as exc:
        assert "ativo desconhecido" in str(exc)
    print("PASS so o AUR e aceito ate existir regra de emissao")


def test_output_to_self_rejected():
    alice, bob = Account(), Account()
    saidas = ordenadas(Output(bob.address, AUR, 1), Output(alice.address, AUR, 1))
    tx = sign_transfer_outputs(alice.secret, P.magic, sender=alice.address,
                               outputs=saidas, fee=0, nonce=0)
    ok, msg = tx.check_signature(P.magic)
    assert not ok and "origem e destino iguais" in msg, msg
    print("PASS nenhuma saida para o proprio remetente")


def test_sum_overflow_rejected():
    alice, bob, carol = Account(), Account(), Account()
    saidas = ordenadas(Output(bob.address, AUR, MAX_AMOUNT), Output(carol.address, AUR, 1))
    tx = sign_transfer_outputs(alice.secret, P.magic, sender=alice.address,
                               outputs=saidas, fee=0, nonce=0)
    ok, msg = tx.check_signature(P.magic)
    assert not ok and "estoura" in msg, msg

    # A taxa entra na soma do AUR.
    tx = sign_transfer(alice.secret, P.magic, sender=alice.address, recipient=bob.address,
                       amount=MAX_AMOUNT, fee=1, nonce=0)
    ok, msg = tx.check_signature(P.magic)
    assert not ok and "estoura" in msg, msg
    print("PASS soma das saidas e da taxa nao estoura")


def test_balance_checked_against_the_sum():
    """Cada saida sozinha cabe no saldo; a soma nao. Nada pode ser debitado."""
    alice, bob, carol = Account(), Account(), Account()
    chain = cadeia_com_saldo(alice)
    saldo = chain.state.balance(alice.address) + chain.state.immature_balance(alice.address)
    metade = saldo // 2 + to_units("100")
    saidas = ordenadas(Output(bob.address, AUR, metade), Output(carol.address, AUR, metade))
    tx = sign_transfer_outputs(alice.secret, P.magic, sender=alice.address, outputs=saidas,
                               fee=0, nonce=chain.state.next_nonce(alice.address))
    antes = dict(chain.state.balances)
    try:
        mine_n(chain, alice.address, 1, [tx])
        raise AssertionError("soma acima do saldo foi aceita")
    except ChainError as exc:
        assert "saldo insuficiente" in str(exc)
    assert chain.state.balances == antes, "bloco recusado mexeu no saldo"
    print("PASS saldo conferido contra a soma das saidas")


def test_v1_domain_signature_not_valid():
    """Uma assinatura feita com o dominio antigo nunca vale na versao 2."""
    alice, bob = Account(), Account()
    tx = sign_transfer(alice.secret, P.magic, sender=alice.address, recipient=bob.address,
                       amount=1, fee=0, nonce=0)
    antigo = b"AURON-TX-v1" + P.magic + tx._body()
    forjada = replace(tx, signature=crypto.sign(alice.secret, antigo))
    ok, msg = forjada.check_signature(P.magic)
    assert not ok and msg == "assinatura inválida", msg
    print("PASS dominio de assinatura separado por versao")


def test_state_rejects_non_native_asset():
    alice = Account()
    state = State(params=P)
    state.balances[(alice.address, OUTRO_ATIVO)] = 1
    try:
        state.check_invariants()
        raise AssertionError("saldo em ativo sem emissao passou na invariante")
    except StateError as exc:
        assert "ativo sem regra de emissão" in str(exc)
    print("PASS invariante recusa ativo sem regra de emissao")


def test_rollback_multi_output():
    alice, bob, carol = Account(), Account(), Account()
    chain = cadeia_com_saldo(alice)
    antes_saldos = dict(chain.state.balances)
    antes_nonces = dict(chain.state.nonces)
    saidas = ordenadas(Output(bob.address, AUR, to_units("1")),
                       Output(carol.address, AUR, to_units("2")))
    tx = sign_transfer_outputs(alice.secret, P.magic, sender=alice.address, outputs=saidas,
                               fee=to_units("0.01"),
                               nonce=chain.state.next_nonce(alice.address))
    mine_n(chain, alice.address, 1, [tx])
    assert chain.state.balance(carol.address) == to_units("2")
    chain.rollback(1)
    assert chain.state.balances == antes_saldos, "saldos nao voltaram"
    assert chain.state.nonces == antes_nonces, "nonces nao voltaram"
    print("PASS desfazer bloco com varias saidas restaura o estado")


if __name__ == "__main__":
    test_multi_output_roundtrip_and_apply()
    test_output_count_bounds()
    test_outputs_must_be_sorted_and_unique()
    test_unknown_asset_rejected()
    test_output_to_self_rejected()
    test_sum_overflow_rejected()
    test_balance_checked_against_the_sum()
    test_v1_domain_signature_not_valid()
    test_state_rejects_non_native_asset()
    test_rollback_multi_output()
    print("=== MULTIATIVO OK ===")
