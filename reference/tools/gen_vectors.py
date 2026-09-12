# ✝ Isaías 26:20 — “Vai, pois, povo meu, entra nos teus quartos e fecha as tuas portas sobre ti; esconde-te só por um momento, até que passe a ira.”
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
from auron.tx import AUR, Coinbase, Output, sign_transfer, sign_transfer_outputs  # noqa: E402
from auron.units import MAX_SUPPLY as MAX_SUPPLY_UNITS  # noqa: E402
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
               "+2", ".5", "7.", "123.45678901", "  3.25  ",
               "0" * 55 + "21000000")   # 63 caracteres: no limite, aceito
    recusados = (
        "-1", "-1.5",            # negativo: dinheiro e u64
        "1.123456789",           # mais de 8 casas: recusar em vez de truncar
        "", "   ", ".", "abc",   # malformado
        "1.2.3", "1,5", "1 5",
        "0x10", "1e8",
        "１",                # digito de largura completa: Python aceitava
        "١",                # algarismo indo-arabico oriental
        " 1.5",          # espaco inseparavel: strip() do Python removia
        "1.5 ",
        "1​.5",          # espaco de largura zero no meio
        "184467440737.09551616",  # acima de u64
        "1" * 65,        # texto longo demais: o limite e 64 caracteres
        "0" * 70 + "1",  # zeros a esquerda nao compram tamanho
    )

    parse = []
    for text in aceitos:
        units = to_units(text)
        parse.append({"input": text, "accepted": True,
                      "units": str(units), "formatted": to_aur_str(units)})
    for text in recusados:
        try:
            units = to_units(text)
            parse.append({"input": text, "accepted": True,
                          "units": str(units), "UNEXPECTED": True})
        except Exception as exc:
            parse.append({"input": text, "accepted": False,
                          "error": type(exc).__name__})

    # unidades -> texto. Sem sinal, que e o que o tipo Amount do Rust cobre.
    fmt = [
        {"units": str(u), "formatted": to_aur_str(u)}
        for u in (0, 1, 10_000_000, 100_000_000, 150_000_000,
                  MAX_SUPPLY_UNITS, 2**64 - 1)
    ]

    # Casos que so existem no Python e o Rust nao consegue nem expressar.
    # Ficam registrados para nao parecer esquecimento, e o harness os ignora.
    python_only = []
    for valor, rotulo in ((0.1, "float 0.1"), (1.0, "float 1.0")):
        try:
            to_units(valor)  # type: ignore[arg-type]
            python_only.append({"case": rotulo, "accepted": True,
                                "UNEXPECTED": True})
        except Exception as exc:
            python_only.append({"case": rotulo, "accepted": False,
                                "error": type(exc).__name__,
                                "note": "no Rust o tipo f64 nem compila aqui"})
    python_only.append({
        "case": "to_aur_str(-150000000)",
        "formatted": to_aur_str(-150_000_000),
        "note": "formatacao aceita negativo por ser exibicao de diferenca; "
                "o tipo Amount do Rust e u64 e nao representa isto",
    })

    return {"parse": parse, "format": fmt, "python_only": python_only}


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

    # Horarios que andam para tras. O retarget usa a sequencia tornada nao
    # decrescente, entao estes casos travam a regra nova: o tempo negativo
    # conta como zero, e a alternacao nao derruba a dificuldade.
    base = 1_000_000
    passo = p.target_spacing
    alternado = []
    for i in range(n):
        t = base + i * passo
        alternado.append(t - 3 * passo if i % 2 else t + 3 * passo)
    para_tras = [base + i * passo for i in range(n)]
    para_tras[n // 2] = base  # um bloco no meio com horario bem antigo
    iguais = [base] * n       # todos no mesmo segundo
    for name, ts in (("horario_alternado", alternado),
                     ("horario_para_tras", para_tras),
                     ("horarios_iguais", iguais)):
        scenarios.append({
            "name": name,
            "network": p.name,
            "timestamps": ts,
            "targets_in": [hex(start)] * n,
            "next_target": hex(consensus.next_target(ts, [start] * n, p)),
            "note": "timestamps nao monotonicos; a regra os torna nao decrescentes",
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

    # Terceiro destinatario, so para o caso com varias saidas.
    addr_c = crypto.address_from_pubkey(crypto.public_key(bytes.fromhex("33" * 32)))

    casos = [
        (sign_transfer(SEED_A, p.magic, sender=addr_a, recipient=addr_b,
                       amount=amount, fee=fee, nonce=nonce))
        for amount, fee, nonce in (
            (to_units("1"), 0, 0),
            (to_units("1.5"), to_units("0.001"), 1),
            (to_units("21000000"), to_units("0.00000001"), 2),
        )
    ]
    # Varias saidas, ja na ordem estrita que o consenso exige.
    saidas = sorted(
        (Output(recipient=addr_b, asset_id=AUR, amount=to_units("2")),
         Output(recipient=addr_c, asset_id=AUR, amount=to_units("0.5"))),
        key=lambda o: (o.recipient, o.asset_id),
    )
    casos.append(sign_transfer_outputs(SEED_A, p.magic, sender=addr_a, outputs=saidas,
                                       fee=to_units("0.01"), nonce=3))

    transfers = []
    for tx in casos:
        transfers.append({
            "network": p.name,
            "sender": h(tx.sender),
            "outputs": [{"recipient": h(o.recipient), "asset_id": h(o.asset_id),
                         "amount": str(o.amount)} for o in tx.outputs],
            "fee": str(tx.fee), "nonce": tx.nonce,
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
            # Chave "endereco:ativo", porque o saldo e por conta e por ativo.
            "balances": {f"{a.hex()}:{ativo.hex()}": str(v)
                         for (a, ativo), v in sorted(chain.state.balances.items())},
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


def vec_crypto_verify() -> list[dict]:
    """Casos de borda da verificacao Ed25519, com o veredito do proprio oraculo.

    `vec_crypto` so tem assinaturas honestas, e por isso nao pega um no que
    aceite ou recuse mais do que o oraculo. Estes pegam, nos dois sentidos:

    - chave publica em encoding nao-canonico e recusada (o ed25519-dalek,
      sozinho, aceita);
    - ponto de ordem pequena em A ou em R e aceito (o verify_strict do dalek
      recusa). Regra escrita na AURON-SPEC-01, secao 2.

    O campo "valid" sai de crypto.verify, nunca de uma suposicao.
    """
    P, Q = crypto._P, crypto._Q

    def le(n: int) -> bytes:
        return n.to_bytes(32, "little")

    def enc(y: int, sign: int) -> bytes:
        return (y | (sign << 255)).to_bytes(32, "little")

    def forge(pub: bytes, point, msg: bytes) -> bytes:
        # Sem segredo nenhum: fecha sempre que [k]A for a identidade. O laco e
        # deterministico (s = 1, 2, 3...), entao o vetor sai sempre igual.
        for s in range(1, 5000):
            r = crypto._point_compress(crypto._point_mul(s, crypto._G))
            k = crypto._sha512_int(r + pub + msg) % Q
            if crypto._point_equal(crypto._point_mul(k, point), crypto._IDENTITY):
                return r + le(s)
        raise RuntimeError("forja nao encontrada em 5000 tentativas")

    msg = b"AURON vetor de borda"
    pub = crypto.public_key(SEED_A)
    a, _ = crypto._secret_expand(SEED_A)
    sig = crypto.sign(SEED_A, msg)
    s = int.from_bytes(sig[32:], "little")

    def with_r(r: bytes) -> bytes:
        # Assinatura legitima da chave A, com R escolhido a dedo.
        k = crypto._sha512_int(r + pub + msg) % Q
        return r + le(k * a % Q)

    identity = crypto._point_decompress(le(1))
    order2 = crypto._point_decompress(le(P - 1))
    order4 = crypto._point_decompress(le(0))

    cases = [
        ("honesta", pub, msg, sig),
        ("mensagem trocada", pub, msg + b"!", sig),
        ("R adulterado em 1 bit", pub, msg, bytes([sig[0] ^ 1]) + sig[1:]),
        ("S + L (maleavel)", pub, msg, sig[:32] + le(s + Q)),
        ("S = L", pub, msg, sig[:32] + le(Q)),
        ("R identidade, chave legitima", pub, msg, with_r(le(1))),
        ("R identidade nao-canonica (y = p + 1)", pub, msg, with_r(le(P + 1))),
        ("A identidade, forjada", le(1), msg, forge(le(1), identity, msg)),
        ("A de ordem 2, forjada", le(P - 1), msg, forge(le(P - 1), order2, msg)),
        ("A de ordem 4, forjada", le(0), msg, forge(le(0), order4, msg)),
        ("A identidade nao-canonica (y = p + 1), forjada", le(P + 1), msg,
         forge(le(P + 1), identity, msg)),
        ("A identidade com bit de sinal, forjada", enc(1, 1), msg,
         forge(enc(1, 1), identity, msg)),
        ("A de ordem 2 com bit de sinal, forjada", enc(P - 1, 1), msg,
         forge(enc(P - 1, 1), order2, msg)),
        ("A de ordem 4 nao-canonica (y = p), forjada", le(P), msg,
         forge(le(P), order4, msg)),
        ("A fora da curva (y = 2)", le(2), msg, sig),
        ("assinatura de 63 bytes", pub, msg, sig[:63]),
        ("assinatura de 65 bytes", pub, msg, sig + bytes(1)),
        ("chave de 31 bytes", pub[:31], msg, sig),
        ("chave de 33 bytes", pub + bytes(1), msg, sig),
    ]
    return [
        {
            "label": label,
            "public_key": h(p),
            "message": h(m),
            "signature": h(sg),
            "valid": crypto.verify(p, m, sg),
        }
        for label, p, m, sg in cases
    ]


def vec_codec_edge() -> dict:
    """Leitura e provas de Merkle nos casos de borda, com o veredito do oraculo.

    codec.json so tem o caminho feliz da escrita e raizes de Merkle. Um Reader
    mais permissivo que o do Python (aceitar sobra, ler alem do fim, UTF-8
    frouxo), ou uma verificacao de prova que aceite ou recuse diferente,
    passaria nele sem acusar nada. Estes pegam, nos dois sentidos.

    Veredito, ponto de falha e mensagem saem do proprio Reader, nunca de
    suposicao. String sai como hex do UTF-8, para o JSON nao reinterpretar.
    """

    def run(ops: list[str], data: bytes) -> dict:
        r = codec.Reader(data)
        values: list = []
        for i, op in enumerate(ops):
            try:
                if op == "finish":
                    r.finish()
                    values.append(None)
                elif op == "string":
                    values.append(h(r.string().encode("utf-8")))
                elif op == "var_bytes":
                    values.append(h(r.var_bytes()))
                elif op.startswith("fixed:"):
                    values.append(h(r.fixed(int(op[len("fixed:"):]))))
                elif op == "list_u32":
                    values.append(r.read_list(lambda rd: rd.u32()))
                elif op == "list_var_bytes":
                    values.append([h(v) for v in r.read_list(lambda rd: rd.var_bytes())])
                elif op in ("u8", "u16", "u32", "u64"):
                    values.append(getattr(r, op)())
                else:
                    # Erro do gerador, nao do Reader: nao pode virar veredito.
                    raise ValueError(f"operacao desconhecida: {op}")
            except codec.CodecError as exc:
                return {"accepted": False, "failed_at": i, "error": str(exc),
                        "values": values}
        return {"accepted": True, "values": values}

    x = bytes.fromhex
    roundtrip = (codec.enc_u64(2**63) + codec.enc_bytes(b"\x00\x01\x02")
                 + codec.enc_str("acentuacao: ção") + codec.enc_fixed(b"A" * 20, 20)
                 + codec.enc_list([1, 2, 3], codec.enc_u32))
    reads = [  # (rotulo, operacoes, entrada); todo caso ganha "finish" no fim
        ("u8 maximo", ["u8"], x("ff")),
        ("u16 e big-endian", ["u16"], x("0102")),
        ("u32 um", ["u32"], x("00000001")),
        ("u64 2^63", ["u64"], x("8000000000000000")),
        ("u64 maximo", ["u64"], x("ff" * 8)),
        ("bytes vazios", ["var_bytes"], x("00000000")),
        ("bytes 000102", ["var_bytes"], x("00000003000102")),
        ("string ção", ["string"], x("00000005c3a7c3a36f")),
        ("string vazia", ["string"], x("00000000")),
        ("string com NUL", ["string"], x("0000000100")),
        ("string so com BOM, que nao e removido", ["string"], x("00000003efbbbf")),
        ("string U+FFFF, nao-caractere valido", ["string"], x("00000003efbfbf")),
        ("string U+10FFFF", ["string"], x("00000004f48fbfbf")),
        ("string de 4 bytes", ["string"], x("00000004f09f9880")),
        ("fixed 0 em entrada vazia", ["fixed:0"], b""),
        ("fixed 20", ["fixed:20"], b"A" * 20),
        ("lista de u32", ["list_u32"], x("00000003000000010000000200000003")),
        ("lista vazia", ["list_u32"], x("00000000")),
        ("lista de bytes", ["list_var_bytes"], x("00000002000000000000000161")),
        ("entrada vazia, so finish", [], b""),
        ("roundtrip do test_02", ["u64", "var_bytes", "string", "fixed:20", "list_u32"],
         roundtrip),
        ("u8 sem nada", ["u8"], b""),
        ("u16 com 1 byte", ["u16"], x("01")),
        ("u32 com 2 bytes", ["u32"], x("0000")),
        ("u64 com 7 bytes", ["u64"], bytes(7)),
        ("sobra depois do u32", ["u32"], codec.enc_u32(7) + b"sobra"),
        ("sobra de 1 byte", [], x("00")),
        ("prefixo mente sobre o conteudo", ["var_bytes"], codec.enc_u32(1000) + b"curto"),
        ("prefixo de 4 GiB com 3 bytes", ["var_bytes"], x("ffffffff000000")),
        ("prefixo truncado", ["var_bytes"], x("000000")),
        ("bytes com 1 a menos", ["var_bytes"], x("0000000261")),
        ("bytes com sobra", ["var_bytes"], x("000000016162")),
        ("string: continuacao solta", ["string"], x("0000000180")),
        ("string: NUL overlong", ["string"], x("00000002c080")),
        ("string: overlong C1", ["string"], x("00000002c1bf")),
        ("string: surrogate U+D800", ["string"], x("00000003eda080")),
        ("string: acima de U+10FFFF", ["string"], x("00000004f4908080")),
        ("string: prefixo corta o multibyte", ["string"], x("00000001c3a7")),
        ("string: byte FF", ["string"], x("00000001ff")),
        ("string: prefixo alem do fim", ["string"], x("00000005c3")),
        ("fixed 20 com 19", ["fixed:20"], b"A" * 19),
        ("lista de 2^32-1 sem itens", ["list_u32"], x("ffffffff")),
        ("lista de 3 com 2", ["list_u32"], x("000000030000000100000002")),
        ("lista com sobra", ["list_u32"], x("0000000100000001ff")),
        ("lista com item mentiroso", ["list_var_bytes"], x("0000000200000001610000000561")),
    ]
    decode = []
    for label, ops, data in reads:
        ops = ops + ["finish"]
        decode.append({"label": label, "ops": ops, "input": h(data), **run(ops, data)})

    def folhas(n: int) -> list[bytes]:
        return [f"folha-{i}".encode() for i in range(n)]

    proofs = []
    for total in (1, 2, 3, 4, 5, 6, 7, 8, 9, 16, 17):
        leaves = folhas(total)
        root = codec.merkle_root(leaves)
        paths = [codec.merkle_path(leaves, i) for i in range(total)]
        proofs.append({
            "leaves": [h(v) for v in leaves],
            "root": h(root),
            "paths": [[h(s) for s in p] for p in paths],
            "valid": [codec.merkle_verify_path(leaves[i], i, total, paths[i], root)
                      for i in range(total)],
        })

    path_errors = []
    for total, index in ((0, 0), (1, 1), (3, 3), (17, 17), (17, 1000)):
        try:
            codec.merkle_path(folhas(total), index)
            path_errors.append({"total": total, "index": index, "accepted": True})
        except codec.CodecError as exc:
            path_errors.append({"total": total, "index": index, "accepted": False,
                                "error": str(exc)})

    l5 = folhas(5)
    r5 = codec.merkle_root(l5)
    p5 = {i: codec.merkle_path(l5, i) for i in range(5)}
    l3 = [b"a", b"b", b"c"]
    r3 = codec.merkle_root(l3)
    p3 = codec.merkle_path(l3, 0)
    lh = codec.merkle_leaf_hash
    verify_cases = [
        ("honesta, ultima folha promovida", l5[4], 4, 5, p5[4], r5),
        ("honesta, primeira folha", l5[0], 0, 5, p5[0], r5),
        ("uma folha, caminho vazio", b"a", 0, 1, [], codec.merkle_root([b"a"])),
        # A prova nao amarra o total: o oraculo aceita. Travado aqui de proposito.
        ("total errado com a mesma forma (3 -> 4)", b"a", 0, 4, p3, r3),
        ("folha impostora", b"impostora", 4, 5, p5[4], r5),
        ("indice trocado", l5[0], 1, 5, p5[0], r5),
        ("indice igual ao total", l5[4], 5, 5, p5[4], r5),
        ("total zero", b"", 0, 0, [], codec.merkle_root([])),
        ("uma folha com um irmao a mais", b"a", 0, 1, [r3], codec.merkle_root([b"a"])),
        ("caminho com um a menos", l5[0], 0, 5, p5[0][:-1], r5),
        ("caminho com um a mais", l5[0], 0, 5, p5[0] + [r5], r5),
        ("caminho em ordem invertida", l5[0], 0, 5, list(reversed(p5[0])), r5),
        ("raiz de outra arvore", l5[0], 0, 5, p5[0], r3),
        ("total errado com outra forma (3 -> 5)", b"a", 0, 5, p3, r3),
        ("no interno apresentado como folha", lh(b"a") + lh(b"b"), 0, 2, [lh(b"c")], r3),
        ("indice e total no teto do u64", l5[0], 2**64 - 2, 2**64 - 1, [r5] * 64, r5),
    ]
    verify = [
        {"label": lb, "leaf": h(lf), "index": i, "total": t,
         "path": [h(s) for s in p], "root": h(rt),
         "valid": codec.merkle_verify_path(lf, i, t, p, rt)}
        for lb, lf, i, t, p, rt in verify_cases
    ]

    return {"decode": decode, "merkle_proofs": proofs,
            "merkle_path_errors": path_errors, "merkle_verify": verify}


FILES = {
    "units.json": vec_units,
    "crypto_ed25519.json": vec_crypto,
    "crypto_ed25519_verify.json": vec_crypto_verify,
    "hash.json": vec_hash,
    "codec.json": vec_codec,
    "codec_edge.json": vec_codec_edge,
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

    # write_bytes, e nao write_text: em modo texto o Python do Windows
    # converte o fim de linha, e o manifesto sairia com bytes diferentes dos
    # que a mesma execucao produz no Linux. Os vetores ja sao escritos em
    # bytes pelo mesmo motivo; o manifesto tinha ficado de fora.
    (OUT_DIR / "MANIFEST.json").write_bytes(
        json.dumps({"spec": "AURON-SPEC-01", "files": manifest},
                   indent=2, sort_keys=True).encode("utf-8")
    )
    print(f"\n{len(FILES)} arquivos de vetores em {OUT_DIR}")
    print("O Rust precisa reproduzir cada um byte a byte.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
