# ✝ Compre terras. Plante. Busque abrigo. Planeje bunkers.
"""Persistencia: a cadeia sobrevive a reinicio e recusa arquivo adulterado."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import crypto, store  # noqa: E402
from auron.chain import Chain  # noqa: E402
from auron.consensus import REGTEST, TESTNET  # noqa: E402
from auron.store import StoreError  # noqa: E402
from auron.tx import sign_transfer  # noqa: E402
from auron.units import to_units  # noqa: E402

P = REGTEST


class Account:
    def __init__(self):
        self.secret = crypto.generate_secret()
        self.pub = crypto.public_key(self.secret)
        self.address = crypto.address_from_pubkey(self.pub)


def build_chain(blocks: int = 4) -> tuple[Chain, Account, Account]:
    alice, bob = Account(), Account()
    chain = Chain(params=P)
    for _ in range(blocks):
        ts = chain.tip.header.timestamp + P.target_spacing
        chain.accept_block(chain.mine(alice.address, timestamp=ts), now=ts + 10)

    tx = sign_transfer(
        alice.secret, P.magic, sender=alice.address, recipient=bob.address,
        amount=to_units("3"), fee=to_units("0.02"),
        nonce=chain.state.next_nonce(alice.address),
    )
    ts = chain.tip.header.timestamp + P.target_spacing
    chain.accept_block(chain.mine(alice.address, [tx], timestamp=ts), now=ts + 10)
    return chain, alice, bob


def test_roundtrip_preserves_state_exactly():
    chain, alice, bob = build_chain()
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "auron.db"
        written = store.save_chain(chain, path)
        assert written == chain.height

        reloaded = store.load_chain(path)
        assert reloaded.height == chain.height
        assert reloaded.tip_hash() == chain.tip_hash()
        assert reloaded.total_work == chain.total_work
        assert reloaded.state.balances == chain.state.balances
        assert reloaded.state.nonces == chain.state.nonces
        assert reloaded.state.total_emitted == chain.state.total_emitted
        assert reloaded.state.pending_coinbase == chain.state.pending_coinbase
        assert reloaded.state.balance(bob.address) == to_units("3")
    print("PASS cadeia sobrevive a reinicio com estado identico")


def test_reload_revalidates_every_block():
    """Arquivo adulterado no disco nao pode virar estado valido."""
    chain, _, _ = build_chain(3)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "auron.db"
        store.save_chain(chain, path)

        raw = bytearray(path.read_bytes())
        # vira um bit bem no meio do arquivo, dentro dos blocos
        raw[len(raw) // 2] ^= 0x01
        path.write_bytes(bytes(raw))

        try:
            store.load_chain(path)
            raise AssertionError("arquivo adulterado foi carregado")
        except StoreError:
            pass
    print("PASS recarga revalida e rejeita adulteracao")


def test_wrong_network_rejected():
    chain, _, _ = build_chain(2)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "auron.db"
        store.save_chain(chain, path)
        try:
            store.load_chain(path, params=TESTNET)
            raise AssertionError("cadeia de regtest carregada como testnet")
        except StoreError as exc:
            assert "rede" in str(exc)
    print("PASS arquivo de outra rede rejeitado")


def test_bad_magic_and_truncation_rejected():
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "auron.db"
        path.write_bytes(b"NAOEAURON" + b"\x00" * 40)
        try:
            store.load_chain(path)
            raise AssertionError("arquivo com magic errado foi aceito")
        except StoreError as exc:
            assert "não é uma cadeia" in str(exc)

        chain, _, _ = build_chain(2)
        store.save_chain(chain, path)
        raw = path.read_bytes()
        path.write_bytes(raw[: len(raw) // 2])
        try:
            store.load_chain(path)
            raise AssertionError("arquivo truncado foi aceito")
        except StoreError:
            pass
    print("PASS magic errado e arquivo truncado rejeitados")


def test_trusted_reload_still_checks_everything_else():
    """`trust_pow` pula so o Argon2id. Estado e assinatura continuam valendo."""
    chain, _, _ = build_chain(3)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "auron.db"
        store.save_chain(chain, path)

        fast = store.load_chain(path, trust_pow=True)
        assert fast.tip_hash() == chain.tip_hash()
        assert fast.state.balances == chain.state.balances

        # com o PoW confiado, um bloco com transacao adulterada AINDA cai
        raw = bytearray(path.read_bytes())
        raw[-20] ^= 0xFF
        path.write_bytes(bytes(raw))
        try:
            store.load_chain(path, trust_pow=True)
            raise AssertionError("adulteracao passou mesmo com trust_pow")
        except StoreError:
            pass
    print("PASS trust_pow pula so a prova de trabalho")


def test_genesis_not_stored():
    """A genese vem dos parametros da rede, nao do arquivo."""
    chain, _, _ = build_chain(2)
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "auron.db"
        count = store.save_chain(chain, path)
        assert count == chain.height, "contagem gravada inclui a genese"
        reloaded = store.load_chain(path)
        assert reloaded.block_at(0).block_hash() == chain.block_at(0).block_hash()
    print("PASS genese e derivada, nao lida do disco")


if __name__ == "__main__":
    test_roundtrip_preserves_state_exactly()
    test_reload_revalidates_every_block()
    test_wrong_network_rejected()
    test_bad_magic_and_truncation_rejected()
    test_trusted_reload_still_checks_everything_else()
    test_genesis_not_stored()
    print("=== STORE OK ===")
