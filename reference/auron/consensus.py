# ✝ 2 Pedro 3:10 — “Mas o Dia do Senhor virá como o ladrão de noite.”
"""AURON — parâmetros de consenso, PoW, alvo, retarget e emissão.

Aqui moram as correções de dois bugs do protótipo.

BUG 3 — dificuldade e tamanho do problema eram o mesmo número.

  No protótipo, `difficulty` alimentava ao mesmo tempo `hash_meets_target`
  (probabilidade de achar um nonce) e `difficulty_to_params` (tamanho da
  matriz, número de itens da mochila). Subir a dificuldade encarecia cada
  tentativa E multiplicava o número de tentativas. O custo crescia de forma
  super-exponencial e não existia curva de retarget utilizável.

  Aqui os três conceitos são separados, como você especificou:

    CONSENSUS_DIFFICULTY  -> `target`, um número de 256 bits. Só isso muda a
                             cada bloco. Só isso o retarget mexe.
    WORK_SIZE             -> parâmetros do Argon2id (memória, passadas,
                             faixas). Fixos por rede, nunca derivados da
                             dificuldade.
    VERIFICATION_COST     -> exatamente uma avaliação Argon2id por cabeçalho,
                             constante e conhecida. Não depende do alvo.

BUG 4 — não existia ajuste de dificuldade.

  `self.difficulty` era fixado no construtor e nunca mudava. Aqui há LWMA,
  que retarget a cada bloco. O algoritmo do Bitcoin (janela de 2016 blocos)
  é perigoso numa rede pequena: um minerador grande chega, minera rápido,
  vai embora, e a cadeia trava por semanas até o próximo ajuste. LWMA reage
  em blocos, não em semanas.
"""

from __future__ import annotations

from dataclasses import dataclass

from . import argon2
from .units import AUR_UNIT, MAX_SUPPLY

# ---------------------------------------------------------------------------
# Alvo em formato compacto (estilo nBits do Bitcoin)
# ---------------------------------------------------------------------------

POW_HASH_BITS = 256
MAX_TARGET_LIMIT = (1 << POW_HASH_BITS) - 1


class ConsensusError(Exception):
    """Regra de consenso violada."""


def target_to_compact(target: int) -> int:
    """Empacota um alvo de 256 bits em 4 bytes.

    Formato: expoente no byte alto, mantissa nos três bytes baixos.
    O bit de sinal (0x00800000) é sempre zero — alvo é sempre positivo.
    """
    if target <= 0:
        raise ConsensusError("alvo deve ser positivo")
    if target > MAX_TARGET_LIMIT:
        raise ConsensusError("alvo acima do limite representável")

    raw = target.to_bytes((target.bit_length() + 7) // 8, "big")
    size = len(raw)
    if raw[0] & 0x80:
        # bit alto ligado colidiria com o bit de sinal; empurra um byte
        raw = b"\x00" + raw
        size += 1
    mantissa = int.from_bytes(raw[:3].ljust(3, b"\x00"), "big")
    return (size << 24) | mantissa


def compact_to_target(bits: int) -> int:
    """Desempacota 4 bytes para um alvo de 256 bits, recusando forma inválida."""
    if not 0 <= bits <= 0xFFFFFFFF:
        raise ConsensusError("bits fora da faixa de u32")
    size = bits >> 24
    mantissa = bits & 0x007FFFFF
    if bits & 0x00800000:
        raise ConsensusError("bit de sinal ligado no alvo compacto")
    if mantissa == 0:
        raise ConsensusError("mantissa zero")

    if size <= 3:
        target = mantissa >> (8 * (3 - size))
    else:
        target = mantissa << (8 * (size - 3))

    if target == 0 or target > MAX_TARGET_LIMIT:
        raise ConsensusError("alvo fora da faixa")
    # Forma canônica: reempacotar precisa devolver o mesmo valor. Sem isso,
    # dois `bits` diferentes representariam o mesmo alvo e o cabeçalho
    # deixaria de ter representação única.
    if target_to_compact(target) != bits:
        raise ConsensusError("alvo compacto não está na forma canônica")
    return target


def normalize_target(target: int) -> int:
    """Reduz um alvo ao valor canônico mais próximo que cabe em 4 bytes.

    A forma compacta guarda só os três bytes mais significativos, então nem
    todo inteiro de 256 bits é representável. Todo alvo que vai parar num
    cabeçalho precisa passar por aqui, senão codificar e decodificar devolve
    um número diferente e o bloco muda de identidade. O arredondamento é para
    baixo, ou seja, nunca deixa o alvo mais fácil do que o pretendido.
    """
    return compact_to_target(target_to_compact(target))


def target_to_work(target: int) -> int:
    """Trabalho acumulado esperado para bater este alvo.

    Usado na escolha de ponta da cadeia. Comparar altura é errado: uma cadeia
    longa de blocos fáceis não pode ganhar de uma curta de blocos difíceis.
    """
    if target <= 0:
        raise ConsensusError("alvo deve ser positivo")
    return (1 << POW_HASH_BITS) // (target + 1)


# ---------------------------------------------------------------------------
# Parâmetros de rede
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class ChainParams:
    """Tudo que distingue uma rede da outra.

    NOTA DE ESCOPO: os números de emissão abaixo são provisórios. Eles ficam
    congelados só quando o documento oficial do projeto definir a tokenomics.
    Estão marcados para não passarem por decididos.
    """

    name: str
    magic: bytes

    # --- WORK_SIZE: fixo por rede, nunca derivado da dificuldade ---
    pow_memory_kib: int
    pow_time_cost: int
    pow_lanes: int

    # --- CONSENSUS_DIFFICULTY ---
    max_target: int              # alvo mais fácil permitido (dificuldade mínima)
    target_spacing: int          # segundos desejados entre blocos
    lwma_window: int             # janela do retarget, em blocos

    # --- emissão (PROVISÓRIO, aguardando o documento oficial) ---
    initial_reward: int
    halving_interval: int

    # --- limites ---
    max_block_bytes: int
    max_future_drift: int        # quanto um timestamp pode furar o relógio
    median_time_span: int        # blocos usados no median-time-past
    coinbase_maturity: int       # blocos até a recompensa poder ser gasta

    def __post_init__(self) -> None:
        # O alvo máximo precisa ser canônico, senão ele não sobrevive à ida e
        # volta pela forma compacta do cabeçalho.
        object.__setattr__(self, "max_target", normalize_target(self.max_target))
        if self.total_emission() > MAX_SUPPLY:
            raise ConsensusError(
                f"{self.name}: emissão total {self.total_emission()} passa do teto "
                f"de {MAX_SUPPLY}"
            )

    def pow_params(self) -> dict:
        return {
            "memory_kib": self.pow_memory_kib,
            "time_cost": self.pow_time_cost,
            "parallelism": self.pow_lanes,
        }

    def total_emission(self) -> int:
        """Soma da emissão até a recompensa zerar. Precisa caber no teto."""
        total = 0
        reward = self.initial_reward
        while reward > 0:
            total += reward * self.halving_interval
            reward //= 2
        return total


POW_SALT = b"AURON-POW-v1\x00\x00\x00\x00"  # 16 bytes, fixo e público

# Alvo inicial: 2^240. Deixa 16 bits de trabalho, suficiente para arrancar
# uma rede nova sem que o primeiro minerador espere horas.
_GENESIS_TARGET = (1 << 240) - 1

MAINNET = ChainParams(
    name="auron-mainnet",
    magic=b"AURM",
    pow_memory_kib=32 * 1024,   # 32 MiB: pressiona memória, atrapalha ASIC
    pow_time_cost=1,
    pow_lanes=1,
    max_target=_GENESIS_TARGET,
    target_spacing=120,
    lwma_window=60,
    initial_reward=50 * AUR_UNIT,     # PROVISÓRIO
    halving_interval=210_000,         # PROVISÓRIO
    max_block_bytes=1_000_000,
    max_future_drift=120,
    median_time_span=11,
    coinbase_maturity=100,
)

TESTNET = ChainParams(
    name="auron-testnet",
    magic=b"AURT",
    pow_memory_kib=32 * 1024,
    pow_time_cost=1,
    pow_lanes=1,
    max_target=(1 << 248) - 1,   # bem mais fácil, para a testnet andar
    target_spacing=120,
    lwma_window=60,
    initial_reward=50 * AUR_UNIT,
    halving_interval=210_000,
    max_block_bytes=1_000_000,
    max_future_drift=120,
    median_time_span=11,
    coinbase_maturity=20,
)

# Rede de laboratório. A memória do Argon2id cai para 32 KiB porque a
# implementação Python leva dezenas de segundos em 32 MiB. O algoritmo é o
# mesmo; só o WORK_SIZE muda. É exatamente por isso que WORK_SIZE precisava
# estar separado da dificuldade.
REGTEST = ChainParams(
    name="auron-regtest",
    magic=b"AURR",
    pow_memory_kib=32,
    pow_time_cost=1,
    pow_lanes=1,
    # Alvo bem frouxo: a implementação Python leva ~20ms por avaliação, então
    # ~4 tentativas por bloco mantém a suíte de testes rodando em segundos.
    max_target=(1 << 254) - 1,
    target_spacing=120,
    lwma_window=30,
    initial_reward=50 * AUR_UNIT,
    halving_interval=210_000,
    max_block_bytes=1_000_000,
    max_future_drift=120,
    median_time_span=11,
    coinbase_maturity=2,
)

NETWORKS = {p.name: p for p in (MAINNET, TESTNET, REGTEST)}


# ---------------------------------------------------------------------------
# PoW
# ---------------------------------------------------------------------------

def pow_hash(header_bytes: bytes, params: ChainParams) -> bytes:
    """Hash de prova de trabalho: uma avaliação Argon2id do cabeçalho.

    VERIFICATION_COST é exatamente isto: uma avaliação, sempre. Não cresce
    com a dificuldade. É o que torna a verificação previsível.
    """
    return argon2.argon2id(
        header_bytes,
        POW_SALT,
        tag_length=32,
        variant=argon2.TYPE_ID,
        **params.pow_params(),
    )


def check_pow_target(hash_bytes: bytes, target: int) -> bool:
    """O hash bate o alvo? Comparação numérica direta, sem contar zeros.

    O protótipo usava `hash_meets_target(h, difficulty)` contando bits zero,
    o que só permite dificuldade em potências de 2. Com alvo numérico o
    retarget pode ser fino.
    """
    if len(hash_bytes) * 8 != POW_HASH_BITS:
        raise ConsensusError(
            f"hash de PoW deve ter {POW_HASH_BITS // 8} bytes, veio {len(hash_bytes)}"
        )
    return int.from_bytes(hash_bytes, "big") <= target


# ---------------------------------------------------------------------------
# Retarget — LWMA-1
# ---------------------------------------------------------------------------

def next_target(timestamps: list[int], targets: list[int],
                params: ChainParams) -> int:
    """Alvo do próximo bloco, por média móvel linearmente ponderada.

    `timestamps` e `targets` são dos últimos blocos, do mais antigo para o
    mais recente, e precisam ter o mesmo tamanho.

    Blocos recentes pesam mais que antigos, então a rede reage rápido a
    entrada e saída de mineradores. O tempo de solução de cada bloco é
    limitado a [-6T, +6T] para que um timestamp mentiroso não consiga
    empurrar a dificuldade para o chão.
    """
    n = len(timestamps)
    if n != len(targets):
        raise ConsensusError("timestamps e targets com tamanhos diferentes")
    if n == 0:
        return params.max_target
    if n <= 1:
        return normalize_target(min(targets[-1], params.max_target))

    window = min(n - 1, params.lwma_window)
    ts = timestamps[-(window + 1):]
    tg = targets[-window:]

    spacing = params.target_spacing
    k = window * (window + 1) // 2 * spacing

    weighted = 0
    for i in range(1, window + 1):
        solvetime = ts[i] - ts[i - 1]
        # Timestamps não são monotônicos por regra de rede; só o
        # median-time-past é. Então o tempo negativo precisa ser tolerado e
        # limitado, não rejeitado.
        solvetime = max(-6 * spacing, min(solvetime, 6 * spacing))
        weighted += solvetime * i

    # Piso do denominador: sem ele, uma sequência de timestamps colados faz o
    # alvo explodir e a dificuldade sumir.
    floor_weighted = k // 3
    if weighted < floor_weighted:
        weighted = floor_weighted

    avg_target = sum(tg) // window
    candidate = avg_target * weighted // k

    # Trava de variação por bloco, nos dois sentidos.
    previous = tg[-1]
    candidate = max(candidate, previous // 2)
    candidate = min(candidate, previous * 2)

    if candidate < 1:
        candidate = 1
    return normalize_target(min(candidate, params.max_target))


def median_time_past(timestamps: list[int], params: ChainParams) -> int:
    """Mediana dos últimos N timestamps.

    O protótipo não tinha regra de tempo nenhuma. A mediana é o que impede um
    minerador de mentir no relógio para manipular o retarget: ele precisaria
    controlar a maioria da janela, não só o próprio bloco.
    """
    if not timestamps:
        return 0
    recent = sorted(timestamps[-params.median_time_span:])
    return recent[len(recent) // 2]


# ---------------------------------------------------------------------------
# Emissão
# ---------------------------------------------------------------------------

def block_reward(height: int, params: ChainParams) -> int:
    """Recompensa do bloco na altura dada, com halving e teto duro.

    O protótipo pagava 50 AUR para sempre, sem halving e sem limite. Emissão
    infinita.
    """
    if height < 0:
        raise ConsensusError("altura negativa")
    halvings = height // params.halving_interval
    if halvings >= 64:
        return 0
    return params.initial_reward >> halvings


def cumulative_emission(height: int, params: ChainParams) -> int:
    """Total emitido até a altura dada, inclusive. Nunca passa de MAX_SUPPLY."""
    if height < 0:
        raise ConsensusError("altura negativa")
    total = 0
    era = 0
    remaining = height + 1
    while remaining > 0 and era < 64:
        reward = params.initial_reward >> era
        if reward == 0:
            break
        count = min(remaining, params.halving_interval)
        total += reward * count
        remaining -= count
        era += 1
    return min(total, MAX_SUPPLY)
