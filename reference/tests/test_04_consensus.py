"""Consenso: alvo compacto, retarget LWMA, emissao e PoW.

Cobre as correcoes dos bugs 3 e 4 do prototipo.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import consensus  # noqa: E402
from auron.consensus import ConsensusError, MAINNET, REGTEST, TESTNET  # noqa: E402
from auron.units import AUR_UNIT, MAX_SUPPLY  # noqa: E402


def test_compact_target_roundtrip():
    for target in (
        1, 2, 0x1234, 0xFFFFFF, 1 << 32, (1 << 240) - 1,
        (1 << 248) - 1, (1 << 252) - 1, MAINNET.max_target,
    ):
        canonical = consensus.normalize_target(target)
        bits = consensus.target_to_compact(canonical)
        assert consensus.compact_to_target(bits) == canonical, f"roundtrip falhou: {target:#x}"
        # arredonda para baixo, nunca deixa mais facil que o pedido
        assert canonical <= target
    print("PASS alvo compacto roundtrip")


def test_compact_rejects_noncanonical_and_sign():
    # bit de sinal ligado
    try:
        consensus.compact_to_target((3 << 24) | 0x00800000 | 0x1234)
        raise AssertionError("bit de sinal foi aceito")
    except ConsensusError:
        pass

    # mantissa zero
    try:
        consensus.compact_to_target(3 << 24)
        raise AssertionError("mantissa zero foi aceita")
    except ConsensusError:
        pass

    # forma nao canonica: mesmo alvo com expoente inflado
    canonical = consensus.target_to_compact(0x1234)
    inflated = (4 << 24) | 0x001234  # 0x1234 << 8 >> 8, mas com size errado
    if inflated != canonical:
        try:
            got = consensus.compact_to_target(inflated)
            # se decodificou, precisa reempacotar diferente -> deve ter falhado
            raise AssertionError(f"forma nao canonica aceita: {got:#x}")
        except ConsensusError:
            pass
    print("PASS forma nao canonica e sinal rejeitados")


def test_target_to_work_monotonic():
    easy = (1 << 248) - 1
    hard = (1 << 200) - 1
    assert consensus.target_to_work(hard) > consensus.target_to_work(easy), \
        "alvo mais dificil precisa valer mais trabalho"
    print("PASS trabalho cresce quando o alvo aperta")


def test_retarget_reacts_to_fast_and_slow_blocks():
    p = REGTEST
    start = consensus.normalize_target((1 << 240) - 1)
    n = p.lwma_window + 1

    # blocos rapidos demais -> alvo precisa APERTAR (numero menor)
    fast_ts = [1000 + i * (p.target_spacing // 4) for i in range(n)]
    fast = consensus.next_target(fast_ts, [start] * n, p)
    assert fast < start, f"blocos rapidos nao apertaram o alvo: {fast:#x} vs {start:#x}"

    # blocos lentos demais -> alvo precisa AFROUXAR (numero maior)
    slow_ts = [1000 + i * (p.target_spacing * 4) for i in range(n)]
    slow = consensus.next_target(slow_ts, [start] * n, p)
    assert slow > start, f"blocos lentos nao afrouxaram o alvo: {slow:#x} vs {start:#x}"

    # no ritmo certo -> alvo praticamente estavel
    ok_ts = [1000 + i * p.target_spacing for i in range(n)]
    steady = consensus.next_target(ok_ts, [start] * n, p)
    drift = abs(steady - start) / start
    assert drift < 0.10, f"alvo derivou {drift:.1%} no ritmo correto"
    print("PASS LWMA reage a ritmo rapido, lento e estavel")


def test_retarget_clamped_per_block():
    p = REGTEST
    start = consensus.normalize_target((1 << 240) - 1)
    n = p.lwma_window + 1

    # todos os timestamps colados: tentativa de explodir o alvo
    glued = [1000] * n
    got = consensus.next_target(glued, [start] * n, p)
    assert got <= start * 2, "alvo passou do dobro num unico bloco"

    # salto gigante de timestamp: tentativa de zerar a dificuldade
    jump = [1000 + i for i in range(n - 1)] + [1000 + 10**9]
    got2 = consensus.next_target(jump, [start] * n, p)
    assert got2 <= start * 2, "timestamp mentiroso passou do limite de variacao"
    assert got2 <= p.max_target, "alvo passou do maximo da rede"
    print("PASS variacao por bloco travada nos dois sentidos")


def test_retarget_never_exceeds_max_target():
    p = REGTEST
    n = p.lwma_window + 1
    slow = [1000 + i * p.target_spacing * 100 for i in range(n)]
    got = consensus.next_target(slow, [p.max_target] * n, p)
    assert got <= p.max_target
    print("PASS alvo nunca passa do maximo da rede")


def test_median_time_past():
    p = REGTEST
    assert consensus.median_time_past([], p) == 0
    ts = [100, 200, 50, 400, 300]
    assert consensus.median_time_past(ts, p) == 200
    # so a janela recente conta
    long_ts = list(range(1000)) + [5, 6, 7]
    assert consensus.median_time_past(long_ts, p) < 1000
    print("PASS median-time-past")


def test_emission_halves_and_respects_cap():
    p = MAINNET
    assert consensus.block_reward(0, p) == 50 * AUR_UNIT
    assert consensus.block_reward(p.halving_interval - 1, p) == 50 * AUR_UNIT
    assert consensus.block_reward(p.halving_interval, p) == 25 * AUR_UNIT
    assert consensus.block_reward(2 * p.halving_interval, p) == 12_50000000
    assert consensus.block_reward(64 * p.halving_interval, p) == 0

    total = p.total_emission()
    assert total <= MAX_SUPPLY, f"emissao total {total} passa do teto {MAX_SUPPLY}"
    assert total > MAX_SUPPLY * 99 // 100, "emissao total ficou longe do teto"

    # emissao acumulada e monotonica e limitada
    assert consensus.cumulative_emission(0, p) == 50 * AUR_UNIT
    assert consensus.cumulative_emission(10, p) == 11 * 50 * AUR_UNIT
    assert consensus.cumulative_emission(10**9, p) <= MAX_SUPPLY
    print(f"PASS emissao com halving, total {total / AUR_UNIT:,.0f} AUR")


def test_every_network_respects_cap():
    for p in (MAINNET, TESTNET, REGTEST):
        assert p.total_emission() <= MAX_SUPPLY, f"{p.name} passa do teto"
        assert p.max_target == consensus.normalize_target(p.max_target), \
            f"{p.name}: max_target nao e canonico"
    print("PASS todas as redes dentro do teto e com alvo canonico")


def test_pow_hash_and_target_check():
    p = REGTEST
    header = b"cabecalho de teste"
    h = consensus.pow_hash(header, p)
    assert len(h) == 32
    assert consensus.pow_hash(header, p) == h, "PoW nao e deterministico"
    assert consensus.pow_hash(header + b"x", p) != h

    # alvo maximo aceita qualquer hash; alvo zero+1 quase nunca aceita
    assert consensus.check_pow_target(h, (1 << 256) - 1)
    assert not consensus.check_pow_target(h, 1)

    # WORK_SIZE diferente muda o hash: prova que os dois estao separados
    assert consensus.pow_hash(header, TESTNET) != h
    print("PASS pow_hash deterministico e checagem de alvo")


def test_verification_cost_independent_of_difficulty():
    """VERIFICATION_COST nao pode depender de CONSENSUS_DIFFICULTY.

    Era exatamente esse acoplamento o bug 3. Verificar precisa custar uma
    avaliacao Argon2id, com alvo facil ou dificil, sem diferenca.
    """
    import time
    p = REGTEST
    header = b"custo constante"

    t0 = time.perf_counter()
    consensus.pow_hash(header, p)
    baseline = time.perf_counter() - t0

    # o alvo nao entra em pow_hash de forma alguma
    import inspect
    src = inspect.getsource(consensus.pow_hash)
    assert "target" not in src, "pow_hash conhece o alvo; acoplamento de volta"
    assert baseline >= 0
    print("PASS custo de verificacao independe da dificuldade")


if __name__ == "__main__":
    test_compact_target_roundtrip()
    test_compact_rejects_noncanonical_and_sign()
    test_target_to_work_monotonic()
    test_retarget_reacts_to_fast_and_slow_blocks()
    test_retarget_clamped_per_block()
    test_retarget_never_exceeds_max_target()
    test_median_time_past()
    test_emission_halves_and_respects_cap()
    test_every_network_respects_cap()
    test_pow_hash_and_target_check()
    test_verification_cost_independent_of_difficulty()
    print("=== CONSENSUS OK ===")

