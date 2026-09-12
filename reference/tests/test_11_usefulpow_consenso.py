# ✝ 2 Tessalonicenses 3:10 — “Se alguém não quer trabalhar, também não coma.”
"""Consenso hibrido: trabalho util exigido em todo bloco, e politica criptografica.

Cobre os testes obrigatorios da correcao de especificacao de 12/09/2026, e as
invariantes que ja existem em codigo:

  I1  bloco sem trabalho util suficiente nao e valido
  I2  o intervalo de bloco nao define a dificuldade do trabalho
  I3  a dificuldade e ajustavel
  I4  o trabalho util tem prova conferivel
  I5  seguranca criptografica nao e um campo arbitrario de 8 bits
  I6  a meta de arquitetura e "classe 1024 bits", declarada como META
  I7  toda primitiva declara o algoritmo de verdade e o nivel real
  I10 o que e planejado nao aparece como ativo

I8 (dado pessoal fora da cadeia) e I9 (chave de identidade separada da chave de
carteira) sao regras da especificacao para a Data Chain e o Auron Identity,
que ainda nao existem em codigo. Nao ha teste de mentira para eles aqui.
"""

from __future__ import annotations

import sys
from dataclasses import replace
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import codec, consensus, crypto, tx, usefulpow, utrax  # noqa: E402
from auron.block import Block, BlockHeader  # noqa: E402
from auron.chain import Chain, ChainError  # noqa: E402
from auron.consensus import REGTEST, check_pow_target, compact_to_target  # noqa: E402
from auron.tx import sign_transfer  # noqa: E402
from auron.units import to_units  # noqa: E402

P = REGTEST


class Conta:
    def __init__(self):
        self.secret = crypto.generate_secret()
        self.pub = crypto.public_key(self.secret)
        self.address = crypto.address_from_pubkey(self.pub)


def mina(chain: Chain, minerador: bytes, n: int = 1, transfers=None) -> None:
    for i in range(n):
        ts = chain.tip.header.timestamp + P.target_spacing
        bloco = chain.mine(minerador, transfers if i == 0 else None, timestamp=ts)
        chain.accept_block(bloco, now=ts + 10)


def acha_nonce(header: BlockHeader, passar: bool = True) -> BlockHeader:
    """Nonce que bate (ou, com passar=False, que NAO bate) o alvo do Argon2id."""
    alvo = compact_to_target(header.bits)
    for nonce in range(1 << 20):
        h = header.with_nonce(nonce)
        if check_pow_target(h.pow_hash(P), alvo) == passar:
            return h
    raise AssertionError("nao achou nonce")


def bloco_com_prova(chain: Chain, minerador: bytes, prova, ts: int) -> Block:
    """Bloco legitimo em tudo, menos na prova de trabalho util dada.

    O useful_root e recalculado a partir da prova, para a recusa vir da
    CONFERENCIA do trabalho, e nao de um compromisso que nao bate.
    """
    candidato = chain.build_candidate(minerador, timestamp=ts)
    header = replace(candidato.header, useful_root=prova.commitment())
    return Block(header=acha_nonce(header), transactions=candidato.transactions,
                 useful_proof=prova)


def espera_recusa(chain: Chain, bloco: Block, trecho: str, ts: int) -> None:
    try:
        chain.accept_block(bloco, now=ts + 10)
    except ChainError as exc:
        assert trecho in str(exc), f"recusado por outro motivo: {exc}"
        return
    raise AssertionError(f"bloco aceito, esperava recusa por: {trecho}")


# ============================================================ CONSENSO =====

def test_bloco_valido_com_trabalho_util_e_aceito():
    chain = Chain(params=P)
    mina(chain, Conta().address, 3)
    assert chain.height == 3
    for entrada in chain.entries[1:]:
        assert entrada.block.useful_proof is not None
        assert entrada.block.header.useful_root == entrada.block.useful_proof.commitment()
    print("PASS bloco com trabalho util valido e aceito")


def test_bloco_sem_trabalho_util_recusado():
    """I1."""
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    candidato = chain.build_candidate(minerador, timestamp=ts)
    sem = Block(header=acha_nonce(candidato.header), transactions=candidato.transactions)
    espera_recusa(chain, sem, "sem prova de trabalho útil", ts)
    print("PASS bloco sem trabalho util recusado")


def test_dificuldade_declarada_errada_recusada():
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    candidato = chain.build_candidate(minerador, timestamp=ts)
    errado = replace(candidato.header, bits=consensus.target_to_compact(P.max_target >> 1))
    bloco = Block(header=errado, transactions=candidato.transactions,
                  useful_proof=candidato.useful_proof)
    espera_recusa(chain, bloco, "dificuldade declarada", ts)
    print("PASS dificuldade invalida recusada")


def test_trabalho_de_consenso_insuficiente_recusado():
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    candidato = chain.build_candidate(minerador, timestamp=ts)
    fraco = Block(header=acha_nonce(candidato.header, passar=False),
                  transactions=candidato.transactions, useful_proof=candidato.useful_proof)
    espera_recusa(chain, fraco, "prova de trabalho não bate o alvo", ts)
    print("PASS trabalho insuficiente recusado")


def test_hash_anterior_invalido_recusado():
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    candidato = chain.build_candidate(minerador, timestamp=ts)
    torto = replace(candidato.header, prev_hash=b"\x07" * 64)
    bloco = Block(header=torto, transactions=candidato.transactions,
                  useful_proof=candidato.useful_proof)
    espera_recusa(chain, bloco, "prev_hash", ts)
    print("PASS hash anterior invalido recusado")


def test_transicao_de_estado_invalida_recusada():
    alice, bob = Conta(), Conta()
    chain = Chain(params=P)
    mina(chain, alice.address, P.coinbase_maturity + 1)
    demais = sign_transfer(alice.secret, P.magic, sender=alice.address, recipient=bob.address,
                           # a margem cobre a recompensa que amadurece neste bloco
                           amount=chain.state.balance(alice.address)
                           + consensus.block_reward(chain.height + 1, P) + 1, fee=0,
                           nonce=chain.state.next_nonce(alice.address))
    try:
        mina(chain, alice.address, 1, [demais])
    except ChainError as exc:
        assert "saldo" in str(exc), exc
    else:
        raise AssertionError("gasto acima do saldo foi aceito")
    print("PASS transicao de estado invalida recusada")


def test_replay_recusado():
    alice, bob = Conta(), Conta()
    chain = Chain(params=P)
    mina(chain, alice.address, P.coinbase_maturity + 1)
    envio = sign_transfer(alice.secret, P.magic, sender=alice.address, recipient=bob.address,
                          amount=to_units("1"), fee=0, nonce=chain.state.next_nonce(alice.address))
    mina(chain, alice.address, 1, [envio])
    try:
        mina(chain, alice.address, 1, [envio])
    except ChainError as exc:
        assert "nonce" in str(exc).lower(), exc
    else:
        raise AssertionError("a mesma transferencia valeu duas vezes")
    print("PASS replay recusado")


# =========================================================== USEFULPOW =====

def test_resultado_fraudulento_detectado():
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    certo = chain.build_candidate(minerador, timestamp=ts).useful_proof
    bruto = bytearray(certo.result)
    bruto[5 * usefulpow.ENTRY_BYTES + 3] ^= 0x01            # uma entrada de C errada
    fraude = replace(certo, result=bytes(bruto))
    espera_recusa(chain, bloco_com_prova(chain, minerador, fraude, ts), "não confere", ts)
    print("PASS resultado fraudulento detectado por Freivalds")


def test_resultado_fora_da_faixa_recusado():
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    certo = chain.build_candidate(minerador, timestamp=ts).useful_proof
    bruto = bytearray(certo.result)
    bruto[0:4] = (2**32 - 1).to_bytes(4, "big")
    fora = replace(certo, result=bytes(bruto))
    espera_recusa(chain, bloco_com_prova(chain, minerador, fora, ts), "fora da faixa", ts)
    print("PASS resultado fora da faixa recusado")


def test_prova_malformada_recusada():
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    certo = chain.build_candidate(minerador, timestamp=ts).useful_proof
    casos = [
        (replace(certo, family=9), "família de trabalho útil desconhecida"),
        (replace(certo, version=7), "versão de prova desconhecida"),
        (replace(certo, result=certo.result[:-4]), "não fecha n·n"),
    ]
    for prova, trecho in casos:
        espera_recusa(chain, bloco_com_prova(chain, minerador, prova, ts), trecho, ts)
    print("PASS prova malformada recusada")


def test_dificuldade_do_trabalho_util_imposta():
    """Trabalho menor que o exigido e recusado, mesmo que o produto esteja certo."""
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    candidato = chain.build_candidate(minerador, timestamp=ts)
    exigido = candidato.useful_proof.n
    menor = exigido - 1
    seed = usefulpow.task_seed(P, candidato.header.height, candidato.header.prev_hash, minerador)
    a, b = utrax.generate_matrices(seed, menor)
    pequena = usefulpow.UsefulWorkProof(family=usefulpow.FAMILY_MATRIX_FREIVALDS, n=menor,
                                        result=(a @ b).astype(">u4").tobytes())
    espera_recusa(chain, bloco_com_prova(chain, minerador, pequena, ts), "difere do exigido", ts)
    print("PASS tamanho do trabalho util imposto pelo consenso")


def test_prova_de_outro_minerador_recusada():
    """A instancia e presa ao minerador: copiar a prova de outro nao serve."""
    chain = Chain(params=P)
    honesto, copiador = Conta().address, Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    alheia = chain.build_candidate(honesto, timestamp=ts).useful_proof
    espera_recusa(chain, bloco_com_prova(chain, copiador, alheia, ts), "não confere", ts)
    print("PASS prova de outro minerador recusada")


def test_prova_reaproveitada_de_bloco_anterior_recusada():
    """A instancia depende do bloco anterior: nao da para resolver antes nem reusar."""
    chain = Chain(params=P)
    minerador = Conta().address
    mina(chain, minerador, 1)
    velha = chain.tip.useful_proof
    ts = chain.tip.header.timestamp + P.target_spacing
    espera_recusa(chain, bloco_com_prova(chain, minerador, velha, ts), "não confere", ts)
    print("PASS prova reaproveitada recusada")


def test_compromisso_no_cabecalho_obrigatorio():
    chain = Chain(params=P)
    minerador = Conta().address
    ts = chain.tip.header.timestamp + P.target_spacing
    candidato = chain.build_candidate(minerador, timestamp=ts)
    trocado = replace(candidato.header, useful_root=b"\x05" * 64)
    bloco = Block(header=acha_nonce(trocado), transactions=candidato.transactions,
                  useful_proof=candidato.useful_proof)
    espera_recusa(chain, bloco, "useful_root", ts)
    print("PASS useful_root precisa bater com a prova")


def test_regra_de_tamanho_dinamica_e_separada_do_intervalo():
    """I2 e I3: o tamanho do trabalho segue o trabalho validado, nao o relogio."""
    base = consensus.MAINNET
    # mais trabalho de consenso nunca diminui o trabalho util
    anterior = 0
    for bits in range(0, 12):
        n = usefulpow.useful_work_size(base.max_target >> bits, base)
        assert n >= anterior, f"tamanho caiu com mais trabalho: {anterior} -> {n}"
        anterior = n
    assert usefulpow.useful_work_size(base.max_target, base) == base.useful_size_base
    assert usefulpow.useful_work_size(base.max_target >> 3, base) == 2 * base.useful_size_base
    assert usefulpow.useful_work_size(1, base) == base.useful_size_max
    assert usefulpow.useful_work_size(base.max_target, base) >= base.useful_size_min

    # mudar o intervalo-alvo de bloco nao mexe no tamanho do trabalho util
    lento = replace(base, target_spacing=600)
    rapido = replace(base, target_spacing=30)
    for bits in (0, 4, 8):
        alvo = base.max_target >> bits
        assert (usefulpow.useful_work_size(alvo, base)
                == usefulpow.useful_work_size(alvo, lento)
                == usefulpow.useful_work_size(alvo, rapido))
    print("PASS tamanho do trabalho util dinamico e separado do intervalo")


def test_conferir_e_mais_barato_que_fazer():
    """A conferencia nao multiplica as matrizes: roda so produtos por vetor."""
    n = 64
    trabalho = n ** 3
    conferencia = P.useful_rounds * 3 * n ** 2
    assert conferencia * 5 < trabalho, (conferencia, trabalho)
    print(f"PASS conferir custa {conferencia} operacoes contra {trabalho} para fazer (n={n})")


# ======================================================== CRIPTOGRAFIA =====

def test_assinatura_confere_e_mensagem_alterada_recusada():
    conta = Conta()
    mensagem = b"AURON teste de assinatura"
    assinatura = crypto.sign(conta.secret, mensagem)
    assert crypto.verify(conta.pub, mensagem, assinatura)
    assert not crypto.verify(conta.pub, mensagem + b"!", assinatura)
    torta = bytearray(assinatura)
    torta[10] ^= 0x01
    assert not crypto.verify(conta.pub, mensagem, bytes(torta))
    print("PASS assinatura confere e mensagem alterada e recusada")


def test_separacao_de_dominios():
    """Nenhum dominio de assinatura, desafio ou compromisso se repete."""
    dominios = [
        tx.SIGNING_DOMAIN, consensus.POW_SALT.rstrip(b"\x00"),
        usefulpow.DOMAIN_TASK, usefulpow.DOMAIN_COMMIT, usefulpow.DOMAIN_CHALLENGE,
        utrax.DOMAIN_INSTANCE, utrax.DOMAIN_FREIVALDS, utrax.DOMAIN_TASK_SEED,
    ]
    assert len(set(dominios)) == len(dominios), "dominio repetido"
    for a in dominios:
        for b in dominios:
            if a != b:
                assert not b.startswith(a), f"{b!r} comeca com {a!r}"
    print("PASS dominios criptograficos separados")


def test_politica_criptografica_com_numeros_reais():
    """I5, I6, I7 e I10."""
    assert crypto.politica_valida() == [], crypto.politica_valida()
    ativas = {k: v for k, v in crypto.PRIMITIVAS.items() if v["estado"] == crypto.ATIVA}
    assert set(ativas) == {crypto.HASH_SHA512_V1, crypto.SIG_ED25519_V1}, ativas.keys()
    # nenhuma primitiva ativa com nivel de "8 bits" ou abaixo do minimo
    for nome, p in ativas.items():
        assert p["classico_bits"] >= crypto.NIVEL_MINIMO_CLASSICO_BITS, nome
    # a meta de 1024 bits e META: nenhuma primitiva afirma entregar isso
    assert "meta" in crypto.META_DE_ARQUITETURA
    assert all(p["classico_bits"] < 1024 for p in crypto.PRIMITIVAS.values())
    # o que ainda nao existe em codigo continua marcado como planejado ou legado
    assert crypto.PRIMITIVAS["SIG-MLDSA65-V1"]["estado"] == crypto.PLANEJADA
    assert crypto.PRIMITIVAS[crypto.SIG_BP512_V1]["estado"] == crypto.LEGADO
    print("PASS politica criptografica com numeros reais e sem 1024 bits de mentira")


def test_versoes_de_regra_declaradas():
    for rede in consensus.NETWORKS.values():
        assert rede.consensus_version == 2
        for campo in ("task_rules_version", "verification_rules_version",
                      "reward_schedule_version", "crypto_policy_version"):
            assert getattr(rede, campo) >= 1, (rede.name, campo)
        assert rede.useful_size_min <= rede.useful_size_base <= rede.useful_size_max
    assert crypto.POLITICA_VERSAO == consensus.MAINNET.crypto_policy_version
    print("PASS versoes de regra declaradas em todas as redes")


TESTS = [v for k, v in sorted(globals().items()) if k.startswith("test_")]

if __name__ == "__main__":
    for teste in TESTS:
        teste()
    print(f"\n{len(TESTS)} testes de consenso hibrido e criptografia passaram")
