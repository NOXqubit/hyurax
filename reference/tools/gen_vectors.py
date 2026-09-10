"""Gera os vetores de teste que a implementacao Rust precisa reproduzir.

Uso:
    python tools/gen_vectors.py

Escreve JSON em `vectors/`. Cada arquivo tem entradas com ENTRADA e SAIDA
esperada, em hexadecimal, sem ambiguidade de tipo.

Regra do projeto: um modulo so e considerado migrado para Rust quando cada
vetor daqui bater byte a byte. "Compilou e os testes locais passaram" nao e
migracao; e coincidencia ate prova em contrario.

O proprio arquivo carrega o hash do seu conteudo, entao da para detectar que os
vetores mudaram sem que ninguem tenha percebido.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
OUT_DIR = ROOT.parent / "vectors"

from auron import argon2, codec, consensus, crypto, utrax  # noqa: E402
from auron.block import BlockHeader  # noqa: E402
from auron.chain import Chain, make_genesis  # noqa: E402
from auron.consensus import MAINNET, REGTEST, TESTNET  # noqa: E402
from auron.tx import Coinbase, sign_transfer  # noqa: E402
from auron.units import to_aur_str, to_units  # noqa: E402

# Segredos FIXOS, so para vetores. Nunca usar em rede de verdade.
SEED_A = bytes.fromhex(
    "1111111111111111111111111111111111111111111111111111111111111111"
)
SEED_B = bytes.fromhex(
    "2222222222222222222222222222222222222222222222222222222222222222"
)


def h(data: bytes) -> str:
    return data.hex()


def vec_units() -> list[dict]:
    """Valores aceitos e, igualmente importante, valores RECUSADOS.

    O Rust precisa recusar exatamente os mesmos. Um parser mais permissivo de
    um lado que do outro e uma divergencia de consenso esperando acontecer.
    """
    aceitos = ("0", "1", "0.1", "0.00000001", "1.5", "50", "21000000",
               "+2", ".5", "7.", "123.45678901", "  3.25  ")
    recusados = (
        "-1", "-1.5",            # negativo: dinheiro e u64
        "1.123456789",           # mais de 8 casas: recusar em vez de truncar
        "", "   ", ".", "abc",   # malformado
        "1.2.3", "1,5", "1 5",
        "0x10", "1e8",
        "１",                # digito de largura completa: Python aceitava
        "١",                # algarismo indo-arabico oriental
        "184467440737.09551616",  # acima de u64
    )

    out = []
    for text in aceitos:
        units = to_units(text)
        out.append({"input": text, "accepted": True,
                    "units": str(units), "formatted": to_aur_str(units)})
    for text in recusados:
        try:
            units = to_units(text)
            out.append({"input": text, "accepted": True,
                        "units": str(units), "UNEXPECTED": True})
        except Exception as exc:
            out.append({"input": text, "accepted": False,
                        "error": type(exc).__name__})

    # float precisa ser recusado, nao aceito silenciosamente
    for valor, rotulo in ((0.1, "float 0.1"), (1.0, "float 1.0")):
        try:
            to_units(valor)  # type: ignore[arg-type]
            out.append({"input": rotulo, "accepted": True, "UNEXPECTED": True})
        except Exception as exc:
            out.append({"input": rotulo, "accepted": False,
                        "error": type(exc).__name__})

    # formatacao aceita negativo de proposito: e exibicao de diferenca
    out.append({"input": "to_aur_str(-150000000)", "accepted": True,
                "formatted": to_aur_str(-150_000_000)})
    return out


def vec_crypto() -> list[dict]:
    out = []
    for label, seed in (("A", SEED_A), ("B", SEED_B)):
        pub = crypto.public_key(seed)
        addr = crypto.address_from_pubkey(pub)
        entry = {
            "label": label,
            "secret": h(seed),
            "public_key": h(pub),
            "address": h(addr),
            "signatures": [],
        }
        for msg in (b"", b"a", b"AURON", bytes(range(64))):
            sig = crypto.sign(seed, msg)
            entry["signatures"].append({
                "message": h(msg),
                "signature": h(sig),
                "valid": crypto.verify(pub, msg, sig),
            })
        out.append(entry)
    return out


def vec_hash() -> list[dict]:
    out = []
    for data in (b"", b"a", b"AURON", bytes(range(256))):
        out.append({"input": h(data), "sha512": h(crypto.H(data))})
    for n in (1, 32, 100):
        out.append({
            "xof_seed": h(b"semente"), "xof_domain": h(b"DOM"), "xof_len": n,
            "output": h(crypto.xof(b"semente", n, domain=b"DOM")),
        })
    return out


def vec_codec() -> dict:
    leaves_sets = [
        [],
        [b"a"],
        [b"a", b"b"],
        [b"a", b"b", b"c"],
        [f"folha-{i}".encode() for i in range(7)],
        [f"folha-{i}".encode() for i in range(16)],
    ]
    merkle = [
        {"leaves": [h(x) for x in leaves], "root": h(codec.merkle_root(leaves))}
        for leaves in leaves_sets
    ]
    encoding = [
        {"kind": "u8", "value": 255, "bytes": h(codec.enc_u8(255))},
        {"kind": "u32", "value": 1, "bytes": h(codec.enc_u32(1))},
        {"kind": "u64", "value": 2**63, "bytes": h(codec.enc_u64(2**63))},
        {"kind": "bytes", "value": h(b"\x00\x01\x02"),
         "bytes": h(codec.enc_bytes(b"\x00\x01\x02"))},
        {"kind": "str", "value": "ção", "bytes": h(codec.enc_str("ção"))},
    ]
    return {"merkle": merkle, "encoding": encoding}


def vec_argon2() -> list[dict]:
    out = []
    for label, variant in (("argon2d", argon2.TYPE_D),
                           ("argon2i", argon2.TYPE_I),
                           ("argon2id", argon2.TYPE_ID)):
        out.append({
            "source": "RFC 9106 secao 5",
            "variant": label,
            "password": h(b"\x01" * 32),
            "salt": h(b"\x02" * 16),
            "secret": h(b"\x03" * 8),
            "associated_data": h(b"\x04" * 12),
            "m_kib": 32, "t": 3, "p": 4, "tag_len": 32,
            "tag": h(argon2.argon2id(
                b"\x01" * 32, b"\x02" * 16, time_cost=3, memory_kib=32,
                parallelism=4, tag_length=32, secret=b"\x03" * 8,
                associated_data=b"\x04" * 12, variant=variant,
            )),
        })
    # parametros do regtest, que e o que o Rust vai exercitar nos testes
    p = REGTEST
    for msg in (b"", b"cabecalho de teste"):
        out.append({
            "source": "auron regtest",
            "variant": "argon2id",
            "password": h(msg), "salt": h(consensus.POW_SALT),
            "m_kib": p.pow_memory_kib, "t": p.pow_time_cost,
            "p": p.pow_lanes, "tag_len": 32,
            "tag": h(consensus.pow_hash(msg, p)),
        })
    return out


def vec_targets() -> dict:
    compact = []
    for target in (1, 0x1234, 0xFFFFFF, 1 << 32, (1 << 200) - 1,
                   MAINNET.max_target, TESTNET.max_target, REGTEST.max_target):
        canonical = consensus.normalize_target(target)
        bits = consensus.target_to_compact(canonical)
        compact.append({
            "target_in": hex(target),
            "canonical": hex(canonical),
            "bits": f"{bits:#010x}",
            "work": str(consensus.target_to_work(canonical)),
        })

    p = REGTEST
    start = consensus.normalize_target((1 << 240) - 1)
    n = p.lwma_window + 1
    scenarios = []
    for name, spacing in (("rapido", p.target_spacing // 4),
                          ("no_ritmo", p.target_spacing),
                          ("lento", p.target_spacing * 4)):
        ts = [1_000_000 + i * spacing for i in range(n)]
        scenarios.append({
            "name": name,
            "network": p.name,
            "timestamps": ts,
            "targets_in": [hex(start)] * n,
            "next_target": hex(consensus.next_target(ts, [start] * n, p)),
        })
    return {"compact": compact, "lwma": scenarios}


def vec_emission() -> list[dict]:
    out = []
    p = MAINNET
    for height in (0, 1, 209_999, 210_000, 420_000, 6_930_000, 13_440_000):
        out.append({
            "network": p.name, "height": height,
            "reward": str(consensus.block_reward(height, p)),
            "cumulative": str(consensus.cumulative_emission(height, p)),
        })
    out.append({"network": p.name, "total_emission": str(p.total_emission())})
    return out


def vec_genesis() -> list[dict]:
    out = []
    for p in (MAINNET, TESTNET, REGTEST):
        g = make_genesis(p)
        out.append({
            "network": p.name,
            "header_bytes": h(g.header.encode()),
            "block_hash": h(g.block_hash()),
            "merkle_root": h(g.header.merkle_root),
            "block_bytes": h(g.encode()),
        })
    return out


def vec_transactions() -> dict:
    p = REGTEST
    addr_a = crypto.address_from_pubkey(crypto.public_key(SEED_A))
    addr_b = crypto.address_from_pubkey(crypto.public_key(SEED_B))

    transfers = []
    for amount, fee, nonce in (
        (to_units("1"), 0, 0),
        (to_units("1.5"), to_units("0.001"), 1),
        (to_units("21000000"), to_units("0.00000001"), 2),
    ):
        tx = sign_transfer(SEED_A, p.magic, sender=addr_a, recipient=addr_b,
                           amount=amount, fee=fee, nonce=nonce)
        transfers.append({
            "network": p.name,
            "sender": h(addr_a), "recipient": h(addr_b),
            "amount": str(amount), "fee": str(fee), "nonce": nonce,
            "signing_payload": h(tx.signing_payload(p.magic)),
            "signature": h(tx.signature),
            "encoded": h(tx.encode()),
            "txid": h(tx.txid()),
        })

    coinbases = []
    for height, amount in ((0, 1), (1, 50 * 100_000_000), (7, 12345)):
        cb = Coinbase(height=height, recipient=addr_a, amount=amount,
                      extra_nonce=b"vetor")
        coinbases.append({
            "height": height, "recipient": h(addr_a), "amount": str(amount),
            "extra_nonce": h(b"vetor"),
            "encoded": h(cb.encode()), "txid": h(cb.txid()),
        })

    return {"transfers": transfers, "coinbases": coinbases}


def vec_chain() -> dict:
    """Minera uma cadeia curta e congela cada bloco. Se o Rust divergir, aparece aqui."""
    p = REGTEST
    addr_a = crypto.address_from_pubkey(crypto.public_key(SEED_A))
    chain = Chain(params=p)

    blocks = []
    for _ in range(4):
        ts = chain.tip.header.timestamp + p.target_spacing
        block = chain.mine(addr_a, timestamp=ts)
        chain.accept_block(block, now=ts + 10)
        blocks.append({
            "height": block.header.height,
            "header_bytes": h(block.header.encode()),
            "block_hash": h(block.block_hash()),
            "pow_hash": h(block.header.pow_hash(p)),
            "bits": f"{block.header.bits:#010x}",
            "nonce": block.header.nonce,
            "timestamp": block.header.timestamp,
            "block_bytes": h(block.encode()),
            "total_work_after": str(chain.total_work),
        })

    return {
        "network": p.name,
        "blocks": blocks,
        "final_state": {
            "height": chain.height,
            "tip_hash": h(chain.tip_hash()),
            "total_emitted": str(chain.state.total_emitted),
            "balances": {a.hex(): str(v)
                         for a, v in sorted(chain.state.balances.items())},
            "pending_coinbase": {
                str(height): [[a.hex(), str(v)] for a, v in entries]
                for height, entries in sorted(chain.state.pending_coinbase.items())
            },
        },
    }


def vec_utrax() -> dict:
    seed = crypto.H(b"vetor utrax")
    matrix = []
    for size in (4, 8, 16):
        result = utrax.matrix_work(seed, size)
        matrix.append({
            "seed": h(seed), "size": size,
            "result": h(result), "verifies": utrax.verify_matrix(seed, size, result),
        })

    knapsack = []
    for n in (5, 12, 20):
        weights, values, capacity = utrax.generate_knapsack(seed, n)
        result = utrax.knapsack_work(seed, n)
        knapsack.append({
            "seed": h(seed), "n_items": n,
            "weights": weights, "values": values, "capacity": capacity,
            "result": h(result), "verifies": utrax.verify_knapsack(seed, n, result),
            "all_zeros_rejected": not utrax.verify_knapsack(seed, n, b"\x00" * 40),
        })

    diffusion = []
    for grid, steps in ((4, 2), (8, 5), (16, 10)):
        result = utrax.diffusion_work(seed, grid, steps)
        diffusion.append({
            "seed": h(seed), "grid_size": grid, "steps": steps,
            "result": h(result),
        })

    return {"matrix": matrix, "knapsack": knapsack, "diffusion": diffusion}


FILES = {
    "units.json": vec_units,
    "crypto_ed25519.json": vec_crypto,
    "hash.json": vec_hash,
    "codec.json": vec_codec,
    "argon2.json": vec_argon2,
    "targets.json": vec_targets,
    "emission.json": vec_emission,
    "genesis.json": vec_genesis,
    "transactions.json": vec_transactions,
    "chain.json": vec_chain,
    "utrax.json": vec_utrax,
}


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    manifest = {}

    for filename, builder in FILES.items():
        print(f"  gerando {filename} ...", end=" ", flush=True)
        payload = {
            "spec": "AURON-SPEC-01",
            "generator": "reference/tools/gen_vectors.py",
            "data": builder(),
        }
        raw = json.dumps(payload, indent=2, ensure_ascii=False,
                         sort_keys=True).encode("utf-8")
        (OUT_DIR / filename).write_bytes(raw)
        digest = crypto.H(raw).hex()[:32]
        manifest[filename] = {"sha512_16": digest, "bytes": len(raw)}
        print(f"{len(raw):,} bytes  {digest}")

    (OUT_DIR / "MANIFEST.json").write_text(
        json.dumps({"spec": "AURON-SPEC-01", "files": manifest},
                   indent=2, sort_keys=True),
        encoding="utf-8",
    )
    print(f"\n{len(FILES)} arquivos de vetores em {OUT_DIR}")
    print("O Rust precisa reproduzir cada um byte a byte.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
