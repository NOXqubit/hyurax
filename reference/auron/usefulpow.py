# ✝ Provérbios 14:23 — “Em todo trabalho há proveito; meras palavras, porém, levam à penúria.”
"""AURON — trabalho útil dentro do consenso (UsefulPoW híbrido, família 1).

Todo bloco precisa trazer uma prova de que o minerador fez um trabalho
computacional útil e conferível. A prova entra no cabeçalho por um compromisso
(`useful_root`), então o Argon2id, a camada complementar de segurança, sorteia
sobre um cabeçalho que já inclui o trabalho útil. Um não substitui o outro.

QUATRO GRANDEZAS SEPARADAS, cada uma com a sua regra:

  TARGET_BLOCK_INTERVAL  alvo de tempo entre blocos (ChainParams.target_spacing).
                         É só o que o retarget persegue.
  CONSENSUS_DIFFICULTY   alvo do Argon2id, ajustado a cada bloco pelo LWMA.
  USEFUL_WORK_SIZE       lado n das matrizes. Cresce com o trabalho validado da
                         rede, pela regra de useful_work_size().
  VERIFICATION_COST      O(n² · rodadas) para conferir, contra O(n³) para fazer.

FAMÍLIA 1 — MATRIX-FREIVALDS-V1

  Tarefa     C = A · B, com A e B matrizes n×n de inteiros em [0, 999].
  Instância  derivada do estado anterior do consenso:
               semente = H(domínio || magic || altura || prev_hash || minerador)
             prev_hash só existe depois do bloco anterior, então ninguém
             resolve antes; o endereço do minerador prende a prova a ele, então
             ninguém copia a prova de outro; e n sai da regra, não da escolha
             de quem minera. Não há como escolher tarefa fácil.
  Prova      a matriz C inteira, em inteiros de 32 bits sem sinal.
  Conferir   faixa de C, depois Freivalds com vetores tirados de H(semente || C),
             que o minerador não conhece antes de fixar C.

O QUE ESTA PROVA DEMONSTRA E O QUE NÃO DEMONSTRA — dito com todas as letras:

  Demonstra que quem minerou calculou um produto de matrizes do tamanho exigido
  para aquele bloco, que é o mesmo tipo de conta que sustenta treino de IA.
  NÃO demonstra utilidade externa: a instância é gerada pela rede, não trazida
  por um cliente. Problemas de fora (remédio, genética, logística) continuam no
  mercado UTRAX, fora do consenso, porque nenhum bloco pode depender de alguém
  de fora aparecer com uma tarefa na hora certa.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from . import codec, crypto
from .utrax import MATRIX_ENTRY_MAX, generate_matrices

FAMILY_MATRIX_FREIVALDS = 1
PROOF_VERSION = 1

DOMAIN_TASK = b"AURON-UPOW-TASK-v1"
DOMAIN_COMMIT = b"AURON-UPOW-PROOF-v1"
DOMAIN_CHALLENGE = b"AURON-UPOW-CHALLENGE-v1"

ENTRY_BYTES = 4
CHALLENGE_BITS = 20

# 2^(k/3) para k = 0, 1, 2, em milésimos. Inteiro de propósito: a regra precisa
# dar o mesmo n em qualquer linguagem, sem ponto flutuante.
_RAIZ_CUBICA_DE_2_MILESIMOS = (1000, 1260, 1587)


class UsefulWorkError(Exception):
    """Prova de trabalho útil malformada."""


@dataclass(frozen=True)
class UsefulWorkProof:
    family: int
    n: int
    result: bytes            # n·n entradas, u32 big-endian, linha a linha
    version: int = PROOF_VERSION

    def encode(self) -> bytes:
        return (
            codec.enc_u8(self.family)
            + codec.enc_u16(self.version)
            + codec.enc_u32(self.n)
            + codec.enc_bytes(self.result)
        )

    @staticmethod
    def decode_from(r: codec.Reader) -> "UsefulWorkProof":
        family = r.u8()
        version = r.u16()
        n = r.u32()
        result = r.var_bytes()
        return UsefulWorkProof(family=family, n=n, result=result, version=version)

    @staticmethod
    def decode(data: bytes) -> "UsefulWorkProof":
        r = codec.Reader(data)
        proof = UsefulWorkProof.decode_from(r)
        r.finish()
        return proof

    def commitment(self) -> bytes:
        """O que vai no cabeçalho, no campo useful_root."""
        return crypto.H(DOMAIN_COMMIT + self.encode())


# ---------------------------------------------------------------------------
# Dificuldade do trabalho útil
# ---------------------------------------------------------------------------

def _bits_de_trabalho(target: int) -> int:
    """log2 aproximado do trabalho esperado para bater o alvo."""
    return 256 - target.bit_length()


def useful_work_size(target: int, params) -> int:
    """Lado n exigido para um bloco com este alvo de consenso.

    O alvo reflete o trabalho que a rede realmente validou, porque o retarget
    o ajusta pelo ritmo observado dos blocos. Rede mais forte, alvo menor,
    mais trabalho por bloco, e o trabalho útil acompanha na mesma proporção.

    Multiplicar matrizes custa n³. Para o custo do trabalho útil crescer na
    mesma razão que o trabalho de consenso, n cresce com a raiz cúbica:

        n = base · 2^(delta / 3),   delta = bits de trabalho acima do mínimo

    limitado a [useful_size_min, useful_size_max]. Tudo em inteiro.
    """
    delta = max(0, _bits_de_trabalho(target) - _bits_de_trabalho(params.max_target))
    n = params.useful_size_base * (1 << (delta // 3))
    n = n * _RAIZ_CUBICA_DE_2_MILESIMOS[delta % 3] // 1000
    return max(params.useful_size_min, min(params.useful_size_max, n))


# ---------------------------------------------------------------------------
# Tarefa, solução e conferência
# ---------------------------------------------------------------------------

def task_seed(params, height: int, prev_hash: bytes, miner: bytes) -> bytes:
    """Semente da instância, derivada do estado anterior do consenso."""
    if len(prev_hash) != crypto.hash_size():
        raise UsefulWorkError("prev_hash com tamanho inválido")
    if len(miner) != crypto.ADDRESS_LEN:
        raise UsefulWorkError("endereço do minerador com tamanho inválido")
    return crypto.H(
        DOMAIN_TASK
        + codec.enc_fixed(params.magic, 4)
        + codec.enc_u64(height)
        + codec.enc_fixed(prev_hash, crypto.hash_size())
        + codec.enc_fixed(miner, crypto.ADDRESS_LEN)
    )


def _limite_de_entrada(n: int) -> int:
    return n * (MATRIX_ENTRY_MAX - 1) ** 2


def solve(params, height: int, prev_hash: bytes, miner: bytes, target: int) -> UsefulWorkProof:
    """Faz o trabalho: calcula C = A · B. Custo O(n³)."""
    n = useful_work_size(target, params)
    a, b = generate_matrices(task_seed(params, height, prev_hash, miner), n)
    c = a @ b
    return UsefulWorkProof(
        family=FAMILY_MATRIX_FREIVALDS,
        n=n,
        result=c.astype(">u4").tobytes(),
    )


def _desafio(seed: bytes, result: bytes, n: int, rodadas: int) -> list[int]:
    raw = crypto.xof(crypto.H(seed + result), rodadas * n * 4, domain=DOMAIN_CHALLENGE)
    modulo = 1 << CHALLENGE_BITS
    return [int.from_bytes(raw[i * 4:i * 4 + 4], "big") % modulo for i in range(rodadas * n)]


def verify(proof, params, height: int, prev_hash: bytes, miner: bytes,
           target: int) -> tuple[bool, str]:
    """Confere a prova sem refazer o trabalho. Custo O(n² · rodadas).

    Devolve (válida, motivo). O motivo existe para a mensagem de recusa dizer
    exatamente o que falhou.
    """
    if proof is None:
        return False, "bloco sem prova de trabalho útil"
    if proof.family != FAMILY_MATRIX_FREIVALDS:
        return False, f"família de trabalho útil desconhecida: {proof.family}"
    if proof.version != PROOF_VERSION:
        return False, f"versão de prova desconhecida: {proof.version}"

    esperado = useful_work_size(target, params)
    if proof.n != esperado:
        # Menor que o exigido é trabalho insuficiente; maior também é recusado,
        # para a regra ser uma só e o tamanho do bloco ficar previsível.
        return False, f"tamanho do trabalho {proof.n} difere do exigido {esperado}"
    if len(proof.result) != esperado * esperado * ENTRY_BYTES:
        return False, "resultado com tamanho que não fecha n·n"

    try:
        seed = task_seed(params, height, prev_hash, miner)
    except UsefulWorkError as exc:
        return False, str(exc)

    n = esperado
    c = np.frombuffer(proof.result, dtype=">u4").astype(np.int64).reshape(n, n)
    # Faixa antes de qualquer conta: fecha o estouro de inteiro que deixaria
    # um resultado errado coincidir com o certo.
    if int(c.max()) > _limite_de_entrada(n):
        return False, "resultado fora da faixa possível de um produto honesto"

    a, b = generate_matrices(seed, n)
    valores = _desafio(seed, proof.result, n, params.useful_rounds)
    for rodada in range(params.useful_rounds):
        r = np.array(valores[rodada * n:(rodada + 1) * n], dtype=np.int64)
        if not np.array_equal(a @ (b @ r), c @ r):
            return False, f"o resultado não confere na rodada {rodada + 1} de Freivalds"
    return True, "ok"
