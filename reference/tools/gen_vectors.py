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
from dataclasses import replace
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
OUT_DIR = ROOT.parent / "vectors"

from hyurax import argon2, codec, consensus, crypto, endereco, genetica, plano, ia, identidade, job, malha, melhoramento, nuvem, rede_ultrax, rotas, triagem, usefulpow, ultrax  # noqa: E402
from hyurax.block import BlockHeader  # noqa: E402
from hyurax.chain import Chain, make_genesis  # noqa: E402
from hyurax.consensus import MAINNET, REGTEST, TESTNET  # noqa: E402
from hyurax.tx import HYX, Coinbase, Output, sign_transfer, sign_transfer_outputs  # noqa: E402
from hyurax.units import MAX_SUPPLY as MAX_SUPPLY_UNITS  # noqa: E402
from hyurax.units import to_hyx_str, to_units  # noqa: E402

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
                      "units": str(units), "formatted": to_hyx_str(units)})
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
        {"units": str(u), "formatted": to_hyx_str(u)}
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
        "case": "to_hyx_str(-150000000)",
        "formatted": to_hyx_str(-150_000_000),
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
        for msg in (b"", b"a", b"HYURAX", bytes(range(64))):
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
    for data in (b"", b"a", b"HYURAX", bytes(range(256))):
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
            "source": "hyurax regtest",
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
            # Argon2id com os parâmetros de verdade de cada rede (32 MiB na
            # mainnet): é o que o minerador em Rust precisa reproduzir.
            "pow_hash": h(g.header.pow_hash(p)),
            "pow_memory_kib": p.pow_memory_kib,
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
        (Output(recipient=addr_b, asset_id=HYX, amount=to_units("2")),
         Output(recipient=addr_c, asset_id=HYX, amount=to_units("0.5"))),
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


def vec_transactions_edge() -> list[dict]:
    """Transações nas bordas: o que decodifica, o que não, e por quê.

    Cada caso grava os bytes e o veredito do próprio gabarito, em duas etapas:
    `decode_error` (a leitura recusou, com a mensagem) e, se a leitura aceitou
    uma transferência, `check` (a conferência de assinatura e estrutura). O
    Rust precisa chegar ao mesmo veredito, pelo mesmo motivo.
    """
    from hyurax.tx import MAX_EXTRA_NONCE, Transfer, TxError, decode_tx
    from hyurax.codec import CodecError

    p = REGTEST
    addr_a = crypto.address_from_pubkey(crypto.public_key(SEED_A))
    addr_b = crypto.address_from_pubkey(crypto.public_key(SEED_B))
    addr_c = crypto.address_from_pubkey(crypto.public_key(bytes.fromhex("33" * 32)))
    outro_ativo = b"\x01" * 32

    def assinada(outputs, fee=0, nonce=0, magic=p.magic):
        return sign_transfer_outputs(SEED_A, magic, sender=addr_a, outputs=outputs,
                                     fee=fee, nonce=nonce)

    def saida(dest, valor, ativo=HYX):
        return Output(recipient=dest, asset_id=ativo, amount=valor)

    valida = assinada([saida(addr_b, to_units("1"))], fee=to_units("0.001"))
    corpo_valido = valida.encode()

    # posições dentro da transferência: kind(1) version(2) sig_code(1) sender(20)
    # fee(8) nonce(8) contagem(4)
    POS_VERSAO, POS_SIG, POS_CONTAGEM = 1, 3, 1 + 2 + 1 + 20 + 8 + 8

    def troca(dados: bytes, pos: int, novo: bytes) -> bytes:
        return dados[:pos] + novo + dados[pos + len(novo):]

    b_ordem = sorted([addr_b, addr_c])
    casos: list[tuple[str, bytes]] = [
        ("transferencia_valida", corpo_valido),
        ("tipo_desconhecido", b"\x02" + corpo_valido[1:]),
        ("versao_transferencia_1", troca(corpo_valido, POS_VERSAO, (1).to_bytes(2, "big"))),
        ("algoritmo_desconhecido", troca(corpo_valido, POS_SIG, b"\x02")),
        ("zero_saidas", corpo_valido[:POS_CONTAGEM] + (0).to_bytes(4, "big")
         + corpo_valido[POS_CONTAGEM + 4 + 60:]),
        ("dezessete_saidas", troca(corpo_valido, POS_CONTAGEM, (17).to_bytes(4, "big"))),
        ("contagem_gigante_sem_itens", corpo_valido[:POS_CONTAGEM] + b"\xff\xff\xff\xff"),
        ("truncada", corpo_valido[:-1]),
        ("byte_sobrando", corpo_valido + b"\x00"),
        ("vazia", b""),
        ("valor_zero", assinada([saida(addr_b, 0)]).encode()),
        ("taxa_maxima_mais_valor_estoura",
         assinada([saida(addr_b, 1)], fee=MAX_AMOUNT_U64).encode()),
        ("soma_das_saidas_estoura",
         assinada([saida(b_ordem[0], MAX_AMOUNT_U64), saida(b_ordem[1], 1)]).encode()),
        ("ativo_desconhecido", assinada([saida(addr_b, 5, outro_ativo)]).encode()),
        ("origem_igual_destino", assinada([saida(addr_a, 5)]).encode()),
        ("saidas_fora_de_ordem",
         assinada([saida(b_ordem[1], 5), saida(b_ordem[0], 5)]).encode()),
        ("saidas_repetidas", assinada([saida(addr_b, 5), saida(addr_b, 5)]).encode()),
        ("chave_publica_curta",
         Transfer(sender=addr_a, outputs=(saida(addr_b, 5),), fee=0, nonce=0,
                  public_key=valida.public_key[:31], signature=valida.signature).encode()),
        ("assinatura_curta",
         Transfer(sender=addr_a, outputs=(saida(addr_b, 5),), fee=0, nonce=0,
                  public_key=valida.public_key, signature=valida.signature[:63]).encode()),
        ("chave_de_outra_conta",
         Transfer(sender=addr_a, outputs=(saida(addr_b, 5),), fee=0, nonce=0,
                  public_key=crypto.public_key(SEED_B), signature=valida.signature).encode()),
        ("assinada_em_outra_rede",
         assinada([saida(addr_b, to_units("1"))], fee=to_units("0.001"),
                  magic=MAINNET.magic).encode()),
        ("assinatura_com_bit_trocado", corpo_valido[:-1] + bytes([corpo_valido[-1] ^ 1])),
        ("duas_saidas_validas",
         assinada([saida(b_ordem[0], 5), saida(b_ordem[1], 7)], fee=3, nonce=9).encode()),
        ("coinbase_valida", Coinbase(height=3, recipient=addr_a, amount=5,
                                     extra_nonce=b"x" * MAX_EXTRA_NONCE).encode()),
        ("coinbase_versao_2", troca(Coinbase(height=3, recipient=addr_a, amount=5).encode(),
                                    POS_VERSAO, (2).to_bytes(2, "big"))),
        ("coinbase_extra_nonce_65",
         b"\x00" + (1).to_bytes(2, "big") + (3).to_bytes(8, "big") + addr_a
         + (5).to_bytes(8, "big") + (65).to_bytes(4, "big") + b"x" * 65),
    ]

    out = []
    for label, dados in casos:
        caso = {"label": label, "encoded": h(dados), "magic": h(p.magic),
                "decode_error": None, "kind": None, "check": None, "txid": None}
        try:
            tx = decode_tx(dados)
        except (TxError, CodecError) as exc:
            caso["decode_error"] = str(exc)
        else:
            caso["kind"] = "transfer" if isinstance(tx, Transfer) else "coinbase"
            caso["txid"] = h(tx.txid())
            if isinstance(tx, Transfer):
                ok, motivo = tx.check_signature(p.magic)
                caso["check"] = "ok" if ok else motivo
        out.append(caso)
    return out


MAX_AMOUNT_U64 = 2**64 - 1


def vec_state() -> dict:
    """Estado de contas: uma sequência de blocos aplicados e desfeitos.

    Cada passo grava as transações do bloco, o resultado (`ok` ou a mensagem
    da recusa) e a fotografia do estado depois. Bloco recusado precisa deixar
    o estado exatamente como estava; desfazer precisa voltar byte a byte.
    """
    from hyurax.state import State, StateError
    from hyurax.tx import Transfer, TxError
    from hyurax.units import AmountError

    p = REGTEST
    a = crypto.address_from_pubkey(crypto.public_key(SEED_A))
    b = crypto.address_from_pubkey(crypto.public_key(SEED_B))
    c = crypto.address_from_pubkey(crypto.public_key(bytes.fromhex("33" * 32)))
    estado = State(params=p)
    desfazer = []
    passos = []

    def foto():
        return {
            "balances": [[k[0].hex(), k[1].hex(), str(v)] for k, v in sorted(estado.balances.items())],
            "nonces": [[k.hex(), v] for k, v in sorted(estado.nonces.items())],
            "pending_coinbase": [[h, [[x.hex(), str(v)] for x, v in e]]
                                 for h, e in sorted(estado.pending_coinbase.items())],
            "total_emitted": str(estado.total_emitted),
        }

    def cb(altura, valor=None, dest=a):
        return Coinbase(height=altura, recipient=dest,
                        amount=consensus.block_reward(altura, p) if valor is None else valor,
                        extra_nonce=b"estado")

    def envio(saidas, fee, nonce, segredo=SEED_A, origem=a):
        return sign_transfer_outputs(segredo, p.magic, sender=origem, outputs=saidas,
                                     fee=fee, nonce=nonce)

    def aplicar(nome, altura, txs):
        try:
            desfazer.append(estado.apply_block(altura, txs, p.magic))
            resultado = "ok"
        except (StateError, TxError, AmountError) as exc:
            resultado = str(exc)
        passos.append({"name": nome, "op": "apply", "height": altura,
                       "transactions": [h(t.encode()) for t in txs],
                       "result": resultado, "state": foto()})

    def reverter(nome):
        estado.revert_block(desfazer.pop())
        passos.append({"name": nome, "op": "revert", "state": foto()})

    um = to_units("1")
    aplicar("coinbase_0", 0, [cb(0)])
    aplicar("coinbase_1", 1, [cb(1)])
    aplicar("coinbase_2_amadurece_0", 2, [cb(2)])
    t1 = envio([Output(recipient=b, asset_id=HYX, amount=10 * um)], fee=um, nonce=0)
    aplicar("transferencia_com_taxa", 3, [cb(3, consensus.block_reward(3, p) + um), t1])
    aplicar("replay_recusado", 4, [cb(4), t1])
    aplicar("saldo_insuficiente", 4,
            [cb(4), envio([Output(recipient=b, asset_id=HYX, amount=500 * um)], fee=0, nonce=1)])
    t2 = envio([Output(recipient=c, asset_id=HYX, amount=um)], fee=0, nonce=1)
    aplicar("duplicada_no_bloco", 4, [cb(4), t2, t2])
    aplicar("coinbase_acima_do_permitido", 4, [cb(4, consensus.block_reward(4, p) + 1)])
    aplicar("coinbase_altura_errada", 4, [cb(5)])
    aplicar("bloco_vazio", 4, [])
    aplicar("primeira_nao_e_coinbase", 4, [t2])
    aplicar("coinbase_extra", 4, [cb(4), cb(4)])
    assinatura_ruim = Transfer(sender=a, outputs=t2.outputs, fee=0, nonce=1,
                               public_key=t2.public_key,
                               signature=t2.signature[:-1] + bytes([t2.signature[-1] ^ 1]))
    aplicar("assinatura_invalida", 4, [cb(4), assinatura_ruim])
    saidas = sorted([Output(recipient=b, asset_id=HYX, amount=2 * um),
                     Output(recipient=c, asset_id=HYX, amount=3 * um)],
                    key=lambda o: (o.recipient, o.asset_id))
    aplicar("duas_saidas_e_coinbase_zero", 4, [cb(4, 0), envio(saidas, fee=5, nonce=1)])
    aplicar("b_gasta_o_que_recebeu", 5,
            [cb(5, dest=c), envio([Output(recipient=a, asset_id=HYX, amount=um)], fee=0,
                                  nonce=0, segredo=SEED_B, origem=b)])
    reverter("desfaz_5")
    reverter("desfaz_4")
    aplicar("reaplica_4", 4, [cb(4, 0), envio(saidas, fee=5, nonce=1)])
    aplicar("ativo_desconhecido", 5,
            [cb(5), envio([Output(recipient=b, asset_id=b"\x07" * 32, amount=1)], fee=0, nonce=2)])
    return {"network": p.name, "steps": passos}


def vec_wire() -> dict:
    """Formato das mensagens da rede (HYURAX-WIRE-v1, seção 21).

    Quadros válidos, para o Rust decodificar e recodificar byte a byte, e
    quadros inválidos com o motivo da recusa. O corpo de cada mensagem usa a
    codificação canônica da seção 3.
    """
    p = REGTEST
    minerador = crypto.address_from_pubkey(crypto.public_key(SEED_A))

    # Uma cadeia curta de verdade, para HEADERS e BLOCK levarem dados reais.
    chain = Chain(params=p)
    for _ in range(2):
        ts = chain.tip.header.timestamp + p.target_spacing
        chain.accept_block(chain.mine(minerador, timestamp=ts), now=ts + 10)
    ponta = chain.tip
    trabalho = chain.total_work.to_bytes(32, "big")
    cabecalhos = [e.block.header.encode() for e in chain.entries]

    PROTO = 2
    MAGIC = p.magic

    def quadro(tipo: int, corpo: bytes) -> bytes:
        return (codec.enc_fixed(MAGIC, 4) + codec.enc_u16(PROTO)
                + codec.enc_u16(tipo) + codec.enc_u32(len(corpo)) + corpo)

    hello = (codec.enc_u16(PROTO) + codec.enc_fixed(MAGIC, 4)
             + codec.enc_u64(chain.height) + codec.enc_fixed(trabalho, 32)
             + codec.enc_u64(0x0102030405060708) + codec.enc_u16(8333))
    hello_ack = hello + codec.enc_u64(0x1122334455667788)
    # locator: hashes da ponta para trás, mais a gênese
    locator = [e.block.block_hash() for e in reversed(chain.entries)]
    get_headers = (codec.enc_list(locator, lambda x: codec.enc_fixed(x, 64))
                   + codec.enc_u32(500))
    headers = codec.enc_list(cabecalhos, lambda c: c)
    get_blocks = codec.enc_list([ponta.block_hash()], lambda x: codec.enc_fixed(x, 64))
    bloco = ponta.encode()
    addrs = codec.enc_list(
        [(4, bytes([203, 0, 113, 7]), 8790, 1_788_912_500),
         (6, bytes(range(16)), 40001, 1_788_912_600)],
        lambda a: codec.enc_u8(a[0]) + codec.enc_bytes(a[1]) + codec.enc_u16(a[2]) + codec.enc_u64(a[3]),
    )
    tx = sign_transfer(SEED_A, p.magic, sender=minerador,
                       recipient=crypto.address_from_pubkey(crypto.public_key(SEED_B)),
                       amount=to_units("1"), fee=0, nonce=0).encode()
    ping = codec.enc_u64(0xDEADBEEF)

    validos = [
        {"name": "hello", "type": 1, "body": h(hello)},
        {"name": "hello_ack", "type": 2, "body": h(hello_ack)},
        {"name": "get_headers", "type": 3, "body": h(get_headers)},
        {"name": "headers", "type": 4, "body": h(headers)},
        {"name": "get_blocks", "type": 5, "body": h(get_blocks)},
        {"name": "block", "type": 6, "body": h(bloco)},
        {"name": "get_addrs", "type": 7, "body": h(b"")},
        {"name": "addrs", "type": 8, "body": h(addrs)},
        {"name": "tx", "type": 9, "body": h(tx)},
        {"name": "ping", "type": 10, "body": h(ping)},
        {"name": "pong", "type": 11, "body": h(ping)},
        # tipo desconhecido: quadro bem formado, decodifica, e a rede IGNORA
        {"name": "desconhecido", "type": 999, "body": h(b"qualquer coisa")},
    ]
    for caso in validos:
        caso["frame"] = h(quadro(caso["type"], bytes.fromhex(caso["body"])))

    MAX_FRAME_BODY = 2 * 1024 * 1024
    q_ok = quadro(1, hello)
    invalidos = [
        {"name": "magic_errado", "frame": h(b"XXXX" + q_ok[4:]),
         "error": "magic da rede não confere"},
        # A versão 1 era a do protocolo sem cifra: um nó atual não fala com ela.
        {"name": "versao_1_sem_cifra",
         "frame": h(q_ok[:4] + codec.enc_u16(1) + q_ok[6:]),
         "error": "versão de protocolo desconhecida: 1"},
        {"name": "versao_futura",
         "frame": h(q_ok[:4] + codec.enc_u16(PROTO + 1) + q_ok[6:]),
         "error": f"versão de protocolo desconhecida: {PROTO + 1}"},
        {"name": "corpo_grande_demais",
         "frame": h(codec.enc_fixed(MAGIC, 4) + codec.enc_u16(PROTO) + codec.enc_u16(1)
                    + codec.enc_u32(MAX_FRAME_BODY + 1)),
         "error": "corpo do quadro maior que o máximo"},
        {"name": "corpo_truncado",
         "frame": h(codec.enc_fixed(MAGIC, 4) + codec.enc_u16(PROTO) + codec.enc_u16(1)
                    + codec.enc_u32(100) + b"\x00" * 40),
         "error": "quadro incompleto"},
        {"name": "cabecalho_truncado", "frame": h(q_ok[:6]),
         "error": "quadro incompleto"},
    ]

    return {
        "network": p.name, "magic": h(MAGIC), "protocol": PROTO,
        "max_frame_body": MAX_FRAME_BODY,
        "valid": validos, "invalid": invalidos,
    }


def vec_chain_edge() -> dict:
    """A cadeia recebendo blocos: aceitos, recusados por cada regra, e rollback.

    Cada recusa é montada a partir de um candidato honesto, com UMA coisa
    errada, e com o nonce do Argon2id achado de novo quando a regra testada
    vem depois dele na ordem de validação. Assim o motivo gravado é o da regra,
    e não um efeito colateral.
    """
    from dataclasses import replace as trocar
    from hyurax.block import Block
    from hyurax.chain import ChainError
    from hyurax.consensus import check_pow_target, compact_to_target, target_to_compact
    from hyurax.tx import Transfer

    p = REGTEST
    a = crypto.address_from_pubkey(crypto.public_key(SEED_A))
    b = crypto.address_from_pubkey(crypto.public_key(SEED_B))
    cadeia = Chain(params=p)
    passos = []

    def minerar_cabecalho(cab, passar=True):
        alvo = compact_to_target(cab.bits)
        for nonce in range(1 << 20):
            c = cab.with_nonce(nonce)
            if check_pow_target(c.pow_hash(p), alvo) == passar:
                return c
        raise AssertionError("sem nonce")

    def ponta():
        return {"height": cadeia.height, "tip_hash": h(cadeia.tip_hash()),
                "total_work": str(cadeia.total_work)}

    def tentar(nome, bloco, agora):
        try:
            cadeia.accept_block(bloco, now=agora)
            resultado = "ok"
        except ChainError as exc:
            resultado = str(exc)
        passos.append({"name": nome, "op": "accept", "block": h(bloco.encode()),
                       "now": agora, "result": resultado, "tip": ponta()})

    def proximo_ts():
        return cadeia.tip.header.timestamp + p.target_spacing

    def honesto(transfers=None, minerador=a):
        ts = proximo_ts()
        return cadeia.mine(minerador, transfers, timestamp=ts), ts + 10

    def com_cabecalho(nome, mudanca, minerar=True):
        ts = proximo_ts()
        cand = cadeia.build_candidate(a, timestamp=ts)
        cab = trocar(cand.header, **mudanca)
        cab = minerar_cabecalho(cab) if minerar else cab
        tentar(nome, Block(cab, cand.transactions, cand.useful_proof), ts + 10)

    for nome in ("bloco_1", "bloco_2"):
        bloco, agora = honesto()
        tentar(nome, bloco, agora)

    ts = proximo_ts()
    cand = cadeia.build_candidate(a, timestamp=ts)
    esperado = cadeia.expected_bits()
    outros_bits = target_to_compact(compact_to_target(esperado) // 2)

    com_cabecalho("versao_1", {"version": 1}, minerar=False)
    com_cabecalho("altura_errada", {"height": cadeia.height + 3}, minerar=False)
    com_cabecalho("prev_hash_errado", {"prev_hash": b"\x07" * 64}, minerar=False)
    com_cabecalho("dificuldade_errada", {"bits": outros_bits}, minerar=False)
    com_cabecalho("timestamp_no_passado", {"timestamp": cadeia.median_time_past()}, minerar=False)
    tentar("timestamp_no_futuro", Block(trocar(cand.header, timestamp=ts + 10_000),
                                        cand.transactions, cand.useful_proof), ts)

    def com_corpo(nome, txs, prova, merkle=None, useful_root=None):
        cab = trocar(cand.header,
                     merkle_root=codec.merkle_root([t.encode() for t in txs]) if merkle is None else merkle,
                     useful_root=(prova.commitment() if prova else cand.header.useful_root)
                     if useful_root is None else useful_root)
        tentar(nome, Block(minerar_cabecalho(cab), txs, prova), ts + 10)

    cb = cand.transactions[0]
    com_corpo("sem_transacoes", [], cand.useful_proof)
    envio = sign_transfer(SEED_A, p.magic, sender=a, recipient=b, amount=1, fee=0, nonce=0)
    com_corpo("primeira_nao_e_coinbase", [envio], cand.useful_proof)
    com_corpo("coinbase_extra", [cb, cb], cand.useful_proof)
    com_corpo("merkle_errado", [cb], cand.useful_proof, merkle=b"\x05" * 64)
    com_corpo("sem_prova_util", [cb], None, useful_root=cand.header.useful_root)
    com_corpo("useful_root_divergente", [cb], cand.useful_proof, useful_root=b"\x06" * 64)
    bruto = bytearray(cand.useful_proof.result)
    bruto[-1] ^= 1
    com_corpo("prova_util_fraudada", [cb], trocar(cand.useful_proof, result=bytes(bruto)))
    tentar("argon2_nao_bate", Block(minerar_cabecalho(cand.header, passar=False),
                                    cand.transactions, cand.useful_proof), ts + 10)
    gulosa = Coinbase(height=cb.height, recipient=a, amount=cb.amount + 1)
    com_corpo("coinbase_inflada", [gulosa], cand.useful_proof)

    # recompensa da altura 1 amadurece na 3: dá para gastar
    bloco3, agora3 = honesto([sign_transfer(SEED_A, p.magic, sender=a, recipient=b,
                                            amount=to_units("7"), fee=to_units("0.5"), nonce=0)])
    tentar("bloco_3_com_transferencia", bloco3, agora3)
    cadeia.rollback(1)
    passos.append({"name": "rollback_1", "op": "rollback", "count": 1, "tip": ponta()})
    tentar("bloco_3_de_novo", bloco3, agora3)
    return {"network": p.name, "steps": passos}


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

    # O arquivo de persistência dessa mesma cadeia, e versões adulteradas com a
    # mensagem de recusa do gabarito ao recarregar.
    import tempfile
    from hyurax.store import StoreError, load_chain, save_chain

    with tempfile.TemporaryDirectory() as pasta:
        arquivo = Path(pasta) / "cadeia.bin"
        save_chain(chain, arquivo)
        gravado = arquivo.read_bytes()

        def recusa(dados: bytes) -> str:
            arquivo.write_bytes(dados)
            try:
                load_chain(arquivo)
            except StoreError as exc:
                return str(exc)
            return "ok"

        ultimo_bloco = len(gravado) - 1
        # onde começa o cabeçalho do último bloco no arquivo
        inicio = len(gravado) - len(chain.entries[-1].block.encode())
        merkle = inicio + 2 + 8 + 64
        store_edge = [
            {"label": label, "file": h(dados), "error": recusa(dados)}
            for label, dados in (
                ("magic_errado", identidade.STORE_MAGIC[:7] + b"2" + gravado[8:]),
                ("rede_desconhecida", gravado[:12] + b"hyurax-xxxxxxx" + gravado[25:]),
                ("truncado", gravado[:-1]),
                ("sobra_no_fim", gravado + b"\x00"),
                ("ultimo_byte_trocado_quebra_a_leitura",
                 gravado[:ultimo_bloco] + bytes([gravado[ultimo_bloco] ^ 1])),
                ("prev_hash_trocado", gravado[:inicio + 20]
                 + bytes([gravado[inicio + 20] ^ 1]) + gravado[inicio + 21:]),
                ("merkle_trocado", gravado[:merkle]
                 + bytes([gravado[merkle] ^ 1]) + gravado[merkle + 1:]),
            )
        ]

    return {
        "network": p.name,
        "store_file": h(gravado),
        "store_edge": store_edge,
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


def vec_ultrax() -> dict:
    seed = crypto.H(b"vetor ultrax")
    matrix = []
    for size in (4, 8, 16):
        result = ultrax.matrix_work(seed, size)
        matrix.append({
            "seed": h(seed), "size": size,
            "result": h(result), "verifies": ultrax.verify_matrix(seed, size, result),
        })

    knapsack = []
    for n in (5, 12, 20):
        weights, values, capacity = ultrax.generate_knapsack(seed, n)
        result = ultrax.knapsack_work(seed, n)
        knapsack.append({
            "seed": h(seed), "n_items": n,
            "weights": weights, "values": values, "capacity": capacity,
            "result": h(result), "verifies": ultrax.verify_knapsack(seed, n, result),
            "all_zeros_rejected": not ultrax.verify_knapsack(seed, n, b"\x00" * 40),
        })

    diffusion = []
    for grid, steps in ((4, 2), (8, 5), (16, 10)):
        result = ultrax.diffusion_work(seed, grid, steps)
        diffusion.append({
            "seed": h(seed), "grid_size": grid, "steps": steps,
            "result": h(result),
        })

    return {"matrix": matrix, "knapsack": knapsack, "diffusion": diffusion}


def vec_ia() -> dict:
    """Treino de rede neural do ULTRAX: pesos iniciais, curva, pesos finais e
    erro, para lotes e passos pequenos. O hash da base trava as moléculas: se o
    arquivo mudar, o vetor acusa."""
    semente = crypto.H(b"vetor ia")
    base = ia.carregar()
    casos = []
    for lote, passos in ((4, 3), (8, 10), (16, 25)):
        pesos, erro, curva = ia.treinar(semente, lote, passos, base)
        casos.append({
            "lote": lote, "passos": passos,
            "resultado": h(ia.codificar(pesos, erro)), "erro": erro, "curva": curva,
            "operacoes": ia.operacoes(lote, passos, len(ia.treino_e_validacao(base)[1])),
        })
    iniciais = ia.pesos_iniciais(semente)
    return {
        "semente": h(semente),
        "base_sha512": crypto.H(ia.ARQUIVO.read_bytes()).hex(),
        "moleculas": len(base),
        "pesos_iniciais": h(ia.codificar(iniciais, 0)),
        "erro_inicial": ia.erro_de_validacao(iniciais, ia.treino_e_validacao(base)[1]),
        "treinos": casos,
    }


def vec_job() -> dict:
    """JOB do ULTRAX: codificação, JOB_ID, semente das unidades, intervalos,
    resumo aditivo e créditos v1 (reference/hyurax/job.py)."""
    jobs = [
        job.Job(7, 1, 64, 0, (), 1000, 2, 1, 0, 0, "matriz 64, mil unidades"),
        job.Job(7, 3, 32, 50, (), 1 << 40, 3, 3, 86_400, 5_000_000, "difusão; concordância entre 3 nós"),
        job.Job(5, 2, 40, 0, (), 12, 5, 2, 3600, 0, "alocação com orçamento livre\ne quebra de linha"),
    ]
    saida = []
    for j in jobs:
        j.validar()
        jid = j.id()
        saida.append({
            "dominio": j.dominio, "tipo": j.tipo, "tamanho": j.tamanho, "passos": j.passos,
            "unidades": j.unidades, "nivel": j.nivel, "redundancia": j.redundancia,
            "prazo_s": j.prazo_s, "orcamento_milicreditos": j.orcamento_milicreditos,
            "descricao": j.descricao,
            "codificacao": h(j.codificar()), "id": h(jid),
            "sementes": [{"indice": i, "semente": h(job.semente_da_unidade(jid, i))}
                         for i in (0, 1, j.unidades - 1)],
        })
    pares = [(i, crypto.H(b"resultado %d" % i)) for i in range(20)]
    intervalos = job.Intervalos()
    insercoes = [(10, 20), (30, 40), (20, 30), (0, 5), (5, 6), (100, 101), (50, 60)]
    for a, b in insercoes:
        intervalos.inserir(a, b)
    return {
        "jobs": saida,
        "resumo": {"pares": [{"indice": i, "hash": h(x)} for i, x in pares], "soma": h(job.resumo(pares))},
        "intervalos": {
            "insercoes": [list(x) for x in insercoes],
            "faixas": [list(x) for x in intervalos.faixas],
            "codificacao": h(intervalos.codificar()),
            "concluidas": intervalos.concluidas(),
            "primeira_faltante": [[d, 200, intervalos.primeira_faltante(d, 200)] for d in (0, 12, 45, 101, 150)],
        },
        "creditos": [[o, v, job.milicreditos(o, v)] for o, v in ((999_999, 0), (10**9, 10**9), (123_456_789, 987_654))],
    }


def vec_genetica() -> dict:
    """Genética de populações (Wright-Fisher) do ULTRAX: o fluxo de sorteios,
    o limiar em Q32 nas bordas, trajetórias inteiras e as recusas. Os casos
    passam pelas bordas: AA letal com a população toda AA (den = 0), mutação
    e seleção no máximo, o menor tamanho e 64 loci."""
    semente = crypto.H(b"vetor genetica")
    s0 = genetica.estado_inicial(semente)
    fluxo = [{"n": n, "saida": f"{genetica.splitmix64(s0, n):016x}"}
             for n in (0, 1, 2, 3, 1000, 2**32 - 1, 2**40 + 7)]
    padrao = [f"{genetica.splitmix64(0, n):016x}" for n in range(4)]

    limiares = []
    for k, m, selecao, dominancia, mutacao in (
        (0, 16, 10_000, 50, 0), (16, 16, 10_000, 50, 0), (4, 16, 10_000, 50, 0),
        (1, 3, 10_000, 50, 0), (0, 10, 10_000, 50, 10_000), (10, 10, 0, 50, 0),
        (10, 10, 0, 50, 100), (9, 10, 0, 50, 0), (9, 10, 0, 100, 0), (1, 10, 0, 0, 0),
        (123, 1000, 20_000, 100, 10_000), (77_777, 200_000, 13_579, 37, 4321),
        (199_999, 200_000, 20_000, 0, 1), (1, 200_000, 0, 100, 10_000),
    ):
        limiares.append({
            "k": k, "m": m, "selecao": selecao, "dominancia": dominancia, "mutacao": mutacao,
            "limiar": genetica.limiar(k, m, genetica.aptidoes(selecao, dominancia), mutacao),
        })

    casos = []
    for tamanho, passos, parametros in (
        (5, 6, [3, 100, 10_500, 50, 3000]),
        (20, 15, [4, 0, 10_000, 0, 5000]),
        (3, 10, [2, 0, 0, 100, 10_000]),
        (7, 9, [5, 10_000, 20_000, 100, 0]),
        (2, 1, [1, 0, 10_000, 50, 5000]),
        (100, 40, [8, 50, 12_000, 25, 1000]),
        (1000, 3, [2, 1, 9_000, 75, 9999]),
        (33, 12, [64, 0, 15_000, 0, 100]),
    ):
        resultado = genetica.executar(tamanho, passos, parametros, semente)
        assert resultado == genetica.executar_referencia(tamanho, passos, parametros, semente)
        casos.append({
            "tamanho": tamanho, "passos": passos, "parametros": parametros,
            "resultado": h(resultado),
            "operacoes": genetica.operacoes(tamanho, passos, parametros),
            "estatisticas": genetica.estatisticas(resultado),
        })

    recusados = []
    for tamanho, passos, parametros in (
        (1, 10, [4, 0, 10_000, 50, 5000]), (100_001, 10, [4, 0, 10_000, 50, 5000]),
        (10, 0, [4, 0, 10_000, 50, 5000]), (10, 10_001, [4, 0, 10_000, 50, 5000]),
        (10, 10, [0, 0, 10_000, 50, 5000]), (10, 10, [65, 0, 10_000, 50, 5000]),
        (10, 10, [4, 10_001, 10_000, 50, 5000]), (10, 10, [4, 0, 20_001, 50, 5000]),
        (10, 10, [4, 0, 10_000, 101, 5000]), (10, 10, [4, 0, 10_000, 50, 10_001]),
        (100_000, 672, [64, 0, 10_000, 50, 5000]),
    ):
        try:
            genetica.validar(tamanho, passos, parametros)
            recusado = False
        except ValueError:
            recusado = True
        assert recusado, (tamanho, passos, parametros)
        recusados.append({"tamanho": tamanho, "passos": passos, "parametros": parametros})

    return {
        "semente": h(semente),
        "estado_inicial": f"{s0:016x}",
        "fluxo": fluxo,
        "splitmix64_estado_zero": padrao,
        "limiares": limiares,
        "casos": casos,
        "recusados": recusados,
        "no_limite": {"tamanho": 100_000, "passos": 671, "parametros": [64, 10_000, 20_000, 100, 10_000]},
    }


def vec_melhoramento() -> dict:
    """Melhoramento de culturas do ULTRAX (tipo 6): o gerador xoshiro256**, a
    arquitetura dos QTL, o fator de Liebig, unidades inteiras (bytes do
    resultado, operacoes e o pedaco de cada chamada de `continuar`) e as
    especificacoes que precisam ser recusadas, com a borda do teto de custo."""
    semente = crypto.H(b"vetor melhoramento")
    g = melhoramento.gerador_da_semente(semente)
    gerador = [g.proximo() for _ in range(8)]

    fatores = []
    for agua in (0, 40, 79, 80, 200):
        for nitrogenio in (0, 50, 100):
            for solo in (0, 55, 150):
                f, lim = melhoramento.fator_ambiental(agua, nitrogenio, solo)
                fatores.append([agua, nitrogenio, solo, f, lim])

    casos = []
    for tamanho, passos, parametros in (
        (8, 2, [5, 1, 50, 100, 100, 100, 100, 0]),
        (16, 3, [64, 7, 25, 50, 40, 100, 60, 100]),
        (10, 4, [130, 0xDEADBEEF, 100, 0, 200, 20, 100, 50]),
        (20, 1, [200, 0xFFFFFFFF, 1, 1000, 0, 100, 100, 100]),
        (4, 6, [1, 3, 1, 300, 100, 100, 100, 0]),
        (1100, 1, [3, 9, 10, 100, 70, 90, 110, 30]),
    ):
        chamadas = []
        sim = melhoramento.simular(tamanho, passos, parametros, semente,
                                   lambda ops: chamadas.append(ops) or True)
        casos.append({
            "tamanho": tamanho, "passos": passos, "parametros": parametros,
            "fator": sim["fator"], "limitante": sim["limitante"],
            "sigma": sim["sigma"], "selecionados": sim["selecionados"],
            "operacoes": melhoramento.operacoes(tamanho, passos, parametros),
            "chamadas": chamadas,
            "resultado": h(melhoramento.codificar(sim)),
        })

    # a borda do teto: a maior quantidade de geracoes aceita com 20.000
    # plantas e 200 QTL, e uma a mais
    grande = [200, 5, 10, 100, 100, 100, 100, 0]
    passos_max = max(p for p in range(1, 1001)
                     if melhoramento.operacoes(20_000, p, grande) <= melhoramento.TETO_OPERACOES)
    aceitas = [
        {"tamanho": 20_000, "passos": passos_max, "parametros": grande},
        {"tamanho": 4, "passos": 1000, "parametros": [1, 0, 1, 0, 0, 0, 0, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [200, 0xFFFFFFFF, 100, 1000, 200, 200, 200, 100]},
    ]
    recusadas = [
        {"tamanho": 20_000, "passos": passos_max + 1, "parametros": grande},
        {"tamanho": 100, "passos": 10, "parametros": [0, 1, 20, 100, 100, 100, 100, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [201, 1, 20, 100, 100, 100, 100, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [50, 1, 0, 100, 100, 100, 100, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [50, 1, 101, 100, 100, 100, 100, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [50, 1, 20, 1001, 100, 100, 100, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [50, 1, 20, 100, 201, 100, 100, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [50, 1, 20, 100, 100, 201, 100, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [50, 1, 20, 100, 100, 100, 201, 0]},
        {"tamanho": 100, "passos": 10, "parametros": [50, 1, 20, 100, 100, 100, 100, 101]},
        {"tamanho": 3, "passos": 10, "parametros": [50, 1, 20, 100, 100, 100, 100, 0]},
        {"tamanho": 100, "passos": 0, "parametros": [50, 1, 20, 100, 100, 100, 100, 0]},
    ]
    for caso in aceitas:
        melhoramento.validar(caso["tamanho"], caso["passos"], caso["parametros"])
    for caso in recusadas:
        try:
            melhoramento.validar(caso["tamanho"], caso["passos"], caso["parametros"])
        except ValueError:
            continue
        raise AssertionError(f"o gabarito aceitou {caso}")

    return {
        "semente": h(semente),
        "gerador": gerador,
        "arquitetura": {"codigo": 12345, "qtl": 16,
                        "efeitos": [list(e) for e in melhoramento.arquitetura(12345, 16)]},
        "fatores": fatores,
        "teto_operacoes": melhoramento.TETO_OPERACOES,
        "casos": casos,
        "aceitas": aceitas,
        "recusadas": recusadas,
    }


def vec_rotas() -> dict:
    """Rotas do ULTRAX (reference/hyurax/rotas.py): o gerador, a instância, a
    partida, o 2-opt inteiro e os certificados, com adulterações recusadas."""
    g = rotas.Gerador([1, 2, 3, 4])
    xoshiro = [g.proximo() for _ in range(6)]
    semente_partida = crypto.H(b"vetor rotas partida")
    casos = []
    for n, passos, instancia in ((4, 10, 1), (8, 50, 2), (12, 100, 3), (30, 200, 4), (60, 3, 5), (120, 1000, 6)):
        semente = crypto.H(b"vetor rotas %d" % n)
        resultado, ops = rotas.executar(n, passos, [instancia], semente)
        otimo = resultado[24] == 1
        # a marca diz "a ultima passada nao achou troca"; o certificado diz
        # "nenhuma troca encurta". Com o teto de passadas, a rota pode ficar
        # otima justo na ultima passada: marca 0 e certificado aceito.
        try:
            rotas.certificar_otimo_local(n, passos, instancia, resultado)
            certificado = True
        except rotas.Recusa:
            certificado = False
        casos.append({
            "tamanho": n, "passos": passos, "instancia": instancia, "semente": h(semente),
            "resultado": h(resultado), "operacoes": ops,
            "comprimento": rotas.certificar(n, passos, instancia, resultado),
            "otimo_local": otimo,
            "otimo_local_certificado": certificado,
        })
    base = casos[3]
    n, passos, inst = base["tamanho"], base["passos"], base["instancia"]
    bom = bytes.fromhex(base["resultado"])
    cab = rotas.CABECALHO
    repetida = bytearray(bom)
    repetida[cab + 2:cab + 4] = repetida[cab + 4:cab + 6]
    mentirosa = bytearray(bom)
    mentirosa[0:8] = (int.from_bytes(bom[0:8], "big") - 1).to_bytes(8, "big")
    adulterados = []
    for nome, dados in (("cidade repetida", bytes(repetida)), ("comprimento mentiroso", bytes(mentirosa)), ("curto", bom[:-1])):
        try:
            rotas.certificar(n, passos, inst, dados)
            recusado = False
        except rotas.Recusa:
            recusado = True
        adulterados.append({"nome": nome, "resultado": h(dados), "recusado": recusado})
    return {
        "xoshiro256ss_estado_1234": xoshiro,
        "coordenadas_7_12": [list(c) for c in rotas.coordenadas(7, 12)],
        "partida": {"semente": h(semente_partida), "n": 20, "rota": rotas.partida(semente_partida, 20)},
        "casos": casos,
        "adulterados": {"tamanho": n, "passos": passos, "instancia": inst, "casos": adulterados},
    }


def vec_triagem() -> dict:
    """Triagem de moléculas reais do ULTRAX (reference/hyurax/triagem.py): o
    catálogo e o modelo travados por hash, faixas com filtros diferentes (a
    borda do catálogo inclusive), adulteração e as unidades de um JOB."""
    cat = triagem.catalogo_embutido()
    n = len(cat.moleculas)
    lipinski_veber = [0, 0, 500_000, 25_000, 5, 10, 10, 140_000, 16_000]
    aberto = [0, 0, triagem.MASSA_MAX, triagem.LOGP_MAX, 100, 100, 100, triagem.TPSA_MAX, 0]
    fechado = [0, 0, 150_000, 20_000, 1, 2, 2, 20_000, 20_000]
    casos = []
    for tamanho, inicio, limites in ((64, 0, lipinski_veber), (200, 1500, lipinski_veber), (37, n - 37, lipinski_veber),
                                     (100, 4000, aberto), (100, 6000, fechado), (1, 8000, lipinski_veber)):
        parametros = list(limites)
        parametros[1] = inicio
        resultado, ops = triagem.executar(tamanho, 0, parametros)
        casos.append({"tamanho": tamanho, "parametros": parametros, "resultado": h(resultado), "operacoes": ops})
    base = casos[0]
    bom = bytes.fromhex(base["resultado"])
    ruim = bytearray(bom)
    ruim[triagem.BYTES_CABECALHO + 3 * triagem.BYTES_POR_MOLECULA + 2] ^= 1
    unidades = []
    for indice in (0, 3, (n - 1) // 1000):
        tamanho, parametros = triagem.derivar_unidade(1000, lipinski_veber, indice)
        unidades.append({"indice": indice, "tamanho": tamanho, "parametros": parametros})
    return {
        "catalogo_sha512": crypto.H(triagem.CATALOGO_ARQUIVO.read_bytes()).hex(),
        "modelo_sha512": crypto.H(triagem.MODELO_ARQUIVO.read_bytes()).hex(),
        "moleculas": n,
        "normalizacao": [cat.media_mili, cat.desvio_mili],
        "casos": casos,
        "adulterado": {"tamanho": base["tamanho"], "parametros": base["parametros"], "resultado": h(bytes(ruim)),
                       "motivo": triagem.verificar(base["tamanho"], 0, base["parametros"], bytes(ruim))},
        "unidades_do_job": {"tamanho": 1000, "parametros": lipinski_veber, "unidades": unidades,
                            "depois_do_fim": (n - 1) // 1000 + 1},
    }


def vec_rede_ultrax() -> dict:
    """Mensagens do ULTRAX entre nós (reference/hyurax/rede_ultrax.py): uma
    de cada subtipo, a oferta assinada, e corpos recusados na leitura."""
    segredo = bytes(range(1, 33))
    worker = crypto.ed25519_public_key(segredo)
    esp_matriz = rede_ultrax.especificacao(1, 64, 0, [])
    esp_genetica = rede_ultrax.especificacao(5, 100, 20, [4, 0, 10100, 50, 1000])
    resultado_hash = crypto.H(b"resultado")
    mensagens = [
        ("oferta", rede_ultrax.oferta(segredo, 0b11111111, 2, 256, 1_790_000_000_000)),
        ("pedido matriz", rede_ultrax.pedido(7, crypto.H(b"job"), 3, esp_matriz, crypto.H(b"semente"), 1_790_000_060_000, True)),
        ("pedido genetica", rede_ultrax.pedido(8, crypto.H(b"job 2"), 2**40 - 1, esp_genetica, crypto.H(b"s2"), 5, False)),
        ("recusa", rede_ultrax.recusa(7, "memória acima do teto deste worker")),
        ("compromisso", rede_ultrax.compromisso(segredo, crypto.H(b"job"), 3, 7, rede_ultrax.compromisso_de(resultado_hash, worker))),
        ("revelar", rede_ultrax.revelar(7)),
        ("resultado", rede_ultrax.resultado(7, b"registro de prova codificado", bytes(64), b"\x01\x02\x03")),
        ("cancelar", rede_ultrax.cancelar(7)),
    ]
    casos = []
    for nome, corpo in mensagens:
        m = rede_ultrax.ler(corpo)
        casos.append({"nome": nome, "corpo": h(corpo), "subtipo": m["subtipo"],
                      "assinatura_confere": m.get("assinatura_confere")})
    oferta = bytearray(mensagens[0][1])
    oferta[2 + 32 + 2 + 1] ^= 1  # a memória muda: a assinatura não confere mais
    recusados = [
        ("versao", bytes([1]) + mensagens[5][1][1:]),
        ("subtipo", bytes([1, 9]) + bytes(8)),
        ("sobra", mensagens[5][1] + b"\x00"),
        ("curto", mensagens[1][1][:-1]),
        ("marca de compromisso", mensagens[1][1][:-1] + b"\x02"),
    ]
    return {
        "tipo_ultrax": rede_ultrax.TIPO_ULTRAX,
        "worker": h(worker),
        "compromisso": h(rede_ultrax.compromisso_de(resultado_hash, worker)),
        "resultado_hash": h(resultado_hash),
        "mensagens": casos,
        "oferta_adulterada": {"corpo": h(bytes(oferta)), "assinatura_confere": rede_ultrax.ler(bytes(oferta))["assinatura_confere"]},
        # a assinatura do compromisso vale só para a unidade do pedido
        "compromisso_assinado": {
            "job": h(crypto.H(b"job")),
            "indice": 3,
            "confere": rede_ultrax.compromisso_confere(rede_ultrax.ler(mensagens[4][1]), crypto.H(b"job"), 3),
            "outro_indice_confere": rede_ultrax.compromisso_confere(rede_ultrax.ler(mensagens[4][1]), crypto.H(b"job"), 4),
            "outro_job_confere": rede_ultrax.compromisso_confere(rede_ultrax.ler(mensagens[4][1]), crypto.H(b"outro"), 3),
        },
        "recusados": [{"nome": n, "corpo": h(c)} for n, c in recusados],
    }


def vec_usefulpow() -> dict:
    """Trabalho util no consenso: regra do tamanho, semente, prova e recusas."""
    tamanhos = []
    for p in (MAINNET, TESTNET, REGTEST):
        for delta in range(0, 13):
            alvo = p.max_target >> delta
            tamanhos.append({"network": p.name, "delta_bits": delta,
                             "target": f"{alvo:#x}", "n": usefulpow.useful_work_size(alvo, p)})

    p = REGTEST
    minerador = crypto.address_from_pubkey(crypto.public_key(SEED_A))
    outro = crypto.address_from_pubkey(crypto.public_key(SEED_B))
    prev = crypto.H(b"vetor usefulpow prev")
    provas = []
    for height, delta in ((1, 0), (2, 3), (9, 7)):
        alvo = p.max_target >> delta
        prova = usefulpow.solve(p, height, prev, minerador, alvo)
        provas.append({
            "network": p.name, "height": height, "prev_hash": h(prev),
            "miner": h(minerador), "target": f"{alvo:#x}",
            "seed": h(usefulpow.task_seed(p, height, prev, minerador)),
            "n": prova.n, "encoded": h(prova.encode()), "commitment": h(prova.commitment()),
            "verifies": usefulpow.verify(prova, p, height, prev, minerador, alvo)[0],
        })

    alvo = p.max_target
    certa = usefulpow.solve(p, 1, prev, minerador, alvo)
    bruto = bytearray(certa.result)
    bruto[-1] ^= 1
    fora = bytearray(certa.result)
    fora[0:4] = b"\xff\xff\xff\xff"
    recusas = []
    for nome, prova, miner in (
        ("resultado_alterado", replace(certa, result=bytes(bruto)), minerador),
        ("fora_da_faixa", replace(certa, result=bytes(fora)), minerador),
        ("outro_minerador", certa, outro),
        ("familia_desconhecida", replace(certa, family=2), minerador),
        ("versao_desconhecida", replace(certa, version=2), minerador),
        ("resultado_truncado", replace(certa, result=certa.result[:-4]), minerador),
    ):
        ok, motivo = usefulpow.verify(prova, p, 1, prev, miner, alvo)
        recusas.append({"case": nome, "encoded": h(prova.encode()), "miner": h(miner),
                        "prev_hash": h(prev), "height": 1, "target": f"{alvo:#x}",
                        "verifies": ok, "reason": motivo})
    return {"sizes": tamanhos, "proofs": provas, "rejections": recusas}


def vec_crypto_verify() -> list[dict]:
    """Casos de borda da verificacao Ed25519, com o veredito do proprio oraculo.

    `vec_crypto` so tem assinaturas honestas, e por isso nao pega um no que
    aceite ou recuse mais do que o oraculo. Estes pegam, nos dois sentidos:

    - chave publica em encoding nao-canonico e recusada (o ed25519-dalek,
      sozinho, aceita);
    - ponto de ordem pequena em A ou em R e aceito (o verify_strict do dalek
      recusa). Regra escrita na HYURAX-SPEC-01, secao 2.

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

    msg = b"HYURAX vetor de borda"
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


def vec_malha() -> dict:
    """Malha (reference/hyurax/malha.py): uma mensagem de cada subtipo, os
    prefixos de conexão, o anúncio na rede local e corpos recusados."""
    token = bytes(range(16))
    alvo = crypto.H(b"alvo")[:32]
    mensagens = [
        ("pedir_alcance", malha.pedir_alcance(8790, token)),
        ("alcance v4", malha.alcance(4, bytes([200, 10, 20, 30]), 8790, True)),
        ("alcance v6", malha.alcance(6, bytes(range(16)), 65535, False)),
        ("reservar", malha.reservar(token)),
        ("reserva", malha.reserva(token, True)),
        ("pontes", malha.pontes([(alvo, 4, bytes([177, 1, 2, 3]), 8790), (alvo, 6, bytes(range(16)), 1)])),
        ("pontes vazia", malha.pontes([])),
    ]
    casos = [{"nome": n, "corpo": h(c), "subtipo": malha.ler(c)["subtipo"]} for n, c in mensagens]
    bom = mensagens[3][1]
    recusados = [
        ("versao", bytes([2]) + bom[1:]),
        ("subtipo", bytes([1, 9])),
        ("sobra", bom + b"\x00"),
        ("curto", bom[:-1]),
        ("familia", bytes([1, 2, 5]) + bytes(6) + b"\x01"),
        ("marca", mensagens[4][1][:-1] + b"\x02"),
        ("pontes demais", bytes([1, 5, 33])),
    ]
    vizinho = malha.anuncio_vizinho(b"HYXT", 8790, alvo)
    return {
        "tipo_malha": malha.TIPO_MALHA,
        "porta_vizinhos": malha.PORTA_VIZINHOS,
        "mensagens": casos,
        "recusados": [{"nome": n, "corpo": h(c)} for n, c in recusados],
        "vizinho": {"corpo": h(vizinho), "magic": h(b"HYXT"), "porta": 8790, "identidade": h(alvo)},
        "pacote": {
            "corpo": h(malha.pacote(b"HYXT", [b"quadro um", b"", bytes(300)])),
            "quadros": [h(b"quadro um"), h(b""), h(bytes(300))],
            "recusado_sobra": h(malha.pacote(b"HYXT", [b"x"]) + bytes(1)),
        },
        "prefixos": {
            "verificar": h(malha.PREFIXO_VERIFICAR + token),
            "reserva": h(malha.PREFIXO_RESERVA + token),
            "circuito": h(malha.PREFIXO_CIRCUITO + alvo),
        },
    }


def vec_nuvem() -> dict:
    """Nuvem (reference/hyurax/nuvem.py): Reed–Solomon, anúncio de máquina,
    recibo de aluguel, mensagens de armazenamento, livro e corpos recusados."""
    segredo = bytes(range(32))
    cliente = bytes(range(100, 132))
    dados = bytes((i * 37 + 11) % 256 for i in range(777))
    codigos = []
    for k, m in [(1, 0), (1, 1), (2, 1), (3, 2), (4, 4), (5, 3)]:
        frags = nuvem.codificar(dados, k, m)
        perdidos = list(range(min(m, k + m - k)))  # perde os m primeiros
        sobra = {i: frags[i] for i in range(k + m) if i not in perdidos}
        assert nuvem.reconstruir(sobra, k, m, len(dados)) == dados
        codigos.append({"k": k, "m": m, "fragmentos": [h(f) for f in frags], "reconstroi_sem": perdidos})
    vazio = nuvem.codificar(b"", 2, 1)
    a = {
        "identidade": crypto.H(b"identidade")[:32], "tipo": 1, "instante_ms": 1_790_000_000_000, "validade_s": 3600,
        "linhas": 4, "ram_mib": 8192, "gpu": "Intel HD Graphics 400", "vram_mib": 512, "disco_mib": 10_240,
        "preco_credito_mili": 1200, "preco_gb_mes_mili": 50_000, "preco_venda_centavos": 0,
        "descricao": "PC de casa, ligado à noite — só capacidade", "creditos_hora": 3_600,
    }
    venda = dict(a, tipo=3, preco_venda_centavos=150_000, descricao="Notebook usado, pronto como nó", preco_credito_mili=0)
    anuncios = [nuvem.msg_anuncio(nuvem.anuncio(segredo, a)), nuvem.msg_anuncio(nuvem.anuncio(segredo, venda))]
    job_id = crypto.H(b"job de teste")
    fornecedor = crypto.ed25519_public_key(segredo)
    recibo = nuvem.msg_recibo(nuvem.recibo(cliente, fornecedor, job_id, 2_500, 1_200, 1_790_000_000_500))
    arq = crypto.H(b"arquivo")[:32]
    fh = crypto.H(b"fragmento")
    nonce = crypto.H(b"nonce")[:32]
    mensagens = [
        ("anuncio capacidade", anuncios[0]),
        ("anuncio venda", anuncios[1]),
        ("guardar", nuvem.guardar(arq, 2, 4096, fh)),
        ("parte", nuvem.parte(arq, 2, 1024, b"bytes cifrados")),
        ("guardado ok", nuvem.guardado(arq, 2, True, "")),
        ("guardado recusado", nuvem.guardado(arq, 2, False, "sem espaço oferecido")),
        ("desafio", nuvem.desafio(arq, 2, nonce)),
        ("prova", nuvem.msg_prova(arq, 2, nonce, nuvem.prova(nonce, b"fragmento"))),
        ("buscar", nuvem.buscar(arq, 63)),
        ("entrega", nuvem.entrega(arq, 0, True, 4096, fh)),
        ("entrega sem", nuvem.entrega(arq, 0, False, 0, bytes(64))),
        ("apagar", nuvem.apagar(arq, 1)),
        ("recibo", recibo),
    ]
    casos = [{"nome": n, "corpo": h(c), "subtipo": nuvem.ler(c)["subtipo"]} for n, c in mensagens]
    torto_assinatura = bytearray(anuncios[0])
    torto_assinatura[-1] ^= 1
    torto_total = bytearray(recibo)
    torto_total[2 + 32 + 32 + 64 + 16 + 7] ^= 1
    recusados = [
        ("versao", bytes([2]) + mensagens[8][1][1:]),
        ("subtipo", bytes([1, 11])),
        ("sobra", mensagens[8][1] + b"\x00"),
        ("curto", mensagens[8][1][:-1]),
        ("indice", mensagens[8][1][:-1] + bytes([64])),
        ("parte vazia", bytes([1, 3]) + arq + bytes([0]) + bytes(4) + codec.enc_bytes(b"")),
        ("guardar zero", bytes([1, 2]) + arq + bytes([0]) + codec.enc_u32(0) + fh),
        ("marca", mensagens[4][1][:2 + 33] + bytes([2]) + codec.enc_str("")),
        ("total do recibo", bytes(torto_total)),
        ("tipo de anuncio", None),
    ]
    # tipo de anúncio 4: o corpo é montado à mão (o gabarito recusa assinar)
    corpo4 = bytearray(anuncios[0])
    corpo4[2 + 64] = 4
    recusados[-1] = ("tipo de anuncio", bytes(corpo4))
    for _, c in recusados:
        try:
            nuvem.ler(c)
        except (ValueError, codec.CodecError):
            continue
        raise AssertionError("corpo recusado foi aceito")
    zero = bytes(64)
    linhas = []
    anterior = zero
    for seq, (tipo, valor, nota) in enumerate([("credito", 10_000, "limite da conta 1"), ("consumo", 3_000, "JOB"),
                                               ("provedor", 2_400, ""), ("plataforma", 450, ""), ("reserva", 150, "")], start=1):
        hh = nuvem.hash_da_linha(anterior, seq, 1_790_000_000, tipo, 1, job_id, fornecedor, valor, nota)
        linhas.append({"seq": seq, "instante": 1_790_000_000, "tipo": tipo, "conta": 1, "job": h(job_id),
                       "contraparte": h(fornecedor), "valor_mili": valor, "nota": nota, "hash": h(hh)})
        anterior = hh
    return {
        "tipo_nuvem": nuvem.TIPO_NUVEM,
        "dados": h(dados),
        "codigos": codigos,
        "vazio": [h(f) for f in vazio],
        "cauchy_3_2": nuvem.cauchy(3, 2),
        "anuncio_assinatura_adulterada": h(bytes(torto_assinatura)),
        "anuncio_worker": h(fornecedor),
        "recibo_cliente": h(crypto.ed25519_public_key(cliente)),
        "mensagens": casos,
        "recusados": [{"nome": n, "corpo": h(c)} for n, c in recusados],
        "prova": {"nonce": h(nonce), "fragmento": h(b"fragmento"), "h": h(nuvem.prova(nonce, b"fragmento"))},
        "livro": linhas,
        "repartir": [{"total": t, "provedor_pct": 80, "plataforma_pct": 15, "partes": list(nuvem.repartir(t, 80, 15))}
                     for t in (0, 1, 7, 999, 3_000, 10**12 + 7)],
    }


def vec_enderecos() -> dict:
    """Endereços em Bech32m (reference/hyurax/endereco.py): os textos nas
    três redes e as recusas, com a mensagem exata."""
    cargas = [bytes(20), bytes([255] * 20), bytes(range(20)),
              bytes([0xC0, 0xDE, 0x48, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0x1B, 0x0E])]
    for s in (bytes([0x11] * 32), bytes([0x22] * 32), bytes(range(32))):
        cargas.append(crypto.address_from_pubkey(crypto.ed25519_public_key(s)))
    validos = []
    for c in cargas:
        validos.append({"endereco": h(c), **{rede: endereco.mostrar(c, "hyurax-" + rede) for rede in ("mainnet", "testnet", "regtest")}})
    teste = endereco.mostrar(cargas[3], "hyurax-testnet")
    trocado = teste[:10] + ("q" if teste[10] != "q" else "p") + teste[11:]
    entradas = [
        (teste, "hyurax-mainnet"),
        (teste, "hyurax-regtest"),
        (endereco.mostrar(cargas[3], "hyurax-mainnet"), "hyurax-testnet"),
        (endereco.codificar("btc", cargas[3]), "hyurax-testnet"),
        (trocado, "hyurax-testnet"),
        (teste[:5] + teste[5:].upper(), "hyurax-testnet"),
        (teste.replace(teste[8], "b", 1) if teste[8] != "b" else teste, "hyurax-testnet"),
        ("thyx1", "hyurax-testnet"),
        ("thyxqqqqqqqqqqqqq", "hyurax-testnet"),
        (endereco.codificar("thyx", bytes(19)), "hyurax-testnet"),
        (endereco.codificar("thyx", bytes(21)), "hyurax-testnet"),
        ("x" * 91, "hyurax-testnet"),
    ]
    recusas = []
    for texto, rede in entradas:
        try:
            endereco.ler(texto, rede)
        except ValueError as e:
            recusas.append({"texto": texto, "rede": rede, "erro": str(e)})
        else:
            raise AssertionError(f"devia recusar: {texto}")
    aceitos = [
        {"texto": teste.upper(), "rede": "hyurax-testnet", "endereco": h(cargas[3])},
        {"texto": "  " + teste + "\n", "rede": "hyurax-testnet", "endereco": h(cargas[3])},
        {"texto": h(cargas[3]), "rede": "hyurax-mainnet", "endereco": h(cargas[3])},
        {"texto": h(cargas[3]).upper(), "rede": "hyurax-testnet", "endereco": h(cargas[3])},
    ]
    for a in aceitos:
        assert endereco.ler(a["texto"], a["rede"]).hex() == a["endereco"]
    return {"validos": validos, "aceitos": aceitos, "recusas": recusas}


def vec_plano() -> dict:
    """Planos (reference/hyurax/plano.py): catálogo, vouchers assinados,
    quando valem e as recusas com a mensagem exata."""
    emissor = bytes(range(32))
    pub = crypto.ed25519_public_key(emissor)
    w1, w2 = crypto.H(b"worker-1")[:32], crypto.H(b"worker-2")[:32]
    dia = 86_400_000
    inicio = 1_791_000_000_000
    catalogo = [{"id": k, "nome": n, "mensal_centavos": m, "anual_centavos": a, "creditos_mes": c,
                 "armazenamento_gib": g, "comissao_bp": bp} for k, (n, m, a, c, g, bp) in sorted(plano.CATALOGO.items())]
    vouchers = []
    for id_, quem, dias, serie in [(plano.PRO, w1, 30, 1), (plano.EQUIPE, w2, 365, 2), (plano.EMPRESA, w1, 31, 3)]:
        b = plano.emitir(emissor, id_, quem, inicio, inicio + dias * dia, serie)
        v = plano.ler(b)
        momentos = [inicio - 1, inicio, inicio + dia, inicio + dias * dia - 1, inicio + dias * dia]
        vouchers.append({
            "bytes": h(b), "texto": plano.texto(b), "plano": id_, "beneficiario": h(quem), "inicio_ms": inicio,
            "fim_ms": inicio + dias * dia, "serie": serie, "creditos_mes": v["creditos_mes"],
            "armazenamento_gib": v["armazenamento_gib"], "comissao_bp": v["comissao_bp"],
            "vale": [{"worker": h(w), "agora_ms": m, "vale": plano.vale(v, pub, w, m)} for w in (w1, w2) for m in momentos],
        })
    base = bytes.fromhex(vouchers[0]["bytes"])
    adulterado = bytearray(base)
    adulterado[60] ^= 1  # créditos mudados: a assinatura não fecha
    falsificado = plano.emitir(bytes([7] * 32), plano.PRO, w1, inicio, inicio + 30 * dia, 9)
    nao_confere = [
        {"bytes": h(bytes(adulterado)), "motivo": "créditos alterados depois de assinar"},
        {"bytes": h(falsificado), "motivo": "assinado por outra chave"},
    ]
    for c in nao_confere:
        v = plano.ler(bytes.fromhex(c["bytes"]))
        assert not plano.assinatura_confere(v, pub)

    def trocar(pos: int, novo: bytes) -> bytes:
        return base[:pos] + novo + base[pos + len(novo):]

    entradas = [
        trocar(0, b"\x02"),                                    # versão
        trocar(1, b"\x00"),                                    # plano gratuito
        trocar(1, b"\x09"),                                    # plano que não existe
        trocar(34, codec.enc_u64(inicio + 30 * dia)),            # inicio == fim
        trocar(42, codec.enc_u64(inicio + 401 * dia)),           # mais de 400 dias
        trocar(62, codec.enc_u16(10_001)),                       # comissão > 100%
        base[:-1],                                              # truncado
        base + b"\x00",                                         # sobra
    ]
    recusas = []
    for e in entradas:
        try:
            plano.ler(e)
        except (ValueError, codec.CodecError) as erro:
            recusas.append({"bytes": h(e), "erro": str(erro)})
        else:
            raise AssertionError("devia recusar")
    textos_ruins = ["hyurax-plano:zz", "plano:" + vouchers[0]["bytes"], "hyurax-plano:" + vouchers[0]["bytes"][:-2]]
    recusas_texto = []
    for x in textos_ruins:
        try:
            plano.ler_texto(x)
        except ValueError as erro:
            recusas_texto.append({"texto": x, "erro": str(erro)})
        else:
            raise AssertionError("devia recusar")
    return {"emissor_publica": h(pub), "catalogo": catalogo, "vouchers": vouchers,
            "nao_confere": nao_confere, "recusas": recusas, "recusas_texto": recusas_texto}


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
    "transactions_edge.json": vec_transactions_edge,
    "state.json": vec_state,
    "chain_edge.json": vec_chain_edge,
    "wire.json": vec_wire,
    "chain.json": vec_chain,
    "ultrax.json": vec_ultrax,
    "usefulpow.json": vec_usefulpow,
    "ia.json": vec_ia,
    "job.json": vec_job,
    "genetica.json": vec_genetica,
    "melhoramento.json": vec_melhoramento,
    "rotas.json": vec_rotas,
    "triagem.json": vec_triagem,
    "rede_ultrax.json": vec_rede_ultrax,
    "malha.json": vec_malha,
    "nuvem.json": vec_nuvem,
    "enderecos.json": vec_enderecos,
    "plano.json": vec_plano,
}


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    manifest = {}

    for filename, builder in FILES.items():
        print(f"  gerando {filename} ...", end=" ", flush=True)
        payload = {
            "spec": identidade.ESPEC,
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
        json.dumps({"spec": identidade.ESPEC, "files": manifest},
                   indent=2, sort_keys=True).encode("utf-8")
    )
    print(f"\n{len(FILES)} arquivos de vetores em {OUT_DIR}")
    print("O Rust precisa reproduzir cada um byte a byte.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
