# ✝ Gênesis 41:35-36 — “Ajuntem toda a comida destes bons anos que vêm; e esta comida será para provimento da terra, para os sete anos de fome.”
"""AURON — Utrax, camada de trabalho útil. FORA DO CONSENSO.

Decisão híbrida: a segurança da cadeia vem do PoW convencional em
`consensus.py`. O Utrax é uma camada econômica de tarefas verificáveis.
A cadeia não precisa do Utrax para sobreviver; o Utrax tem espaço para
evoluir até, um dia, provar que merece função maior.

Isso muda o cálculo de projeto de forma importante: como o Utrax não segura
o consenso, quem paga a verificação é quem publicou a tarefa. Então uma
verificação cara é uma decisão de negócio do publicador, não um vetor de
negação de serviço contra a rede inteira.

Os três conceitos que você mandou separar aparecem explicitamente aqui:

  WORK_SIZE          -> `WorkSpec.size`, o tamanho do problema
  VERIFICATION_COST  -> `WorkSpec.verification_cost()`, declarado e conferível
  CONSENSUS_DIFFICULTY -> não existe nesta camada, de propósito

BUG 1 — verificação de matriz não determinística.

  `freivalds_verify` chamava `np.random.default_rng()` sem semente. Cada nó
  sorteava vetores diferentes, então dois nós honestos podiam discordar sobre
  o mesmo resultado. O `auron_reference.py` usava `default_rng(0)`, que
  resolve o determinismo mas deixa os vetores públicos e fixos: dá para
  procurar uma matriz errada que passe naqueles vetores específicos.

  Aqui o desafio é derivado de H(domínio || semente || resultado). É
  determinístico, todo nó calcula o mesmo, e depende do próprio resultado
  alegado. Para trapacear seria preciso achar um C errado cujo desafio,
  derivado dele mesmo, o aceite. Com vetores em [0, 2^20) e 4 rodadas isso
  é da ordem de 2^-80.

  Também não usa o gerador do numpy. O fluxo de bits do numpy é uma
  dependência de versão, e nada que dois nós precisem calcular igual pode
  depender da versão de uma biblioteca.

BUG 2 — a mochila aceitava qualquer solução viável.

  `verify_knapsack_witness` conferia só que a máscara cabia na capacidade e
  que o valor declarado batia com a soma da máscara. Máscara vazia com valor
  zero passava: 40 bytes de zeros eram trabalho válido. O executor sorteava
  subconjuntos baratos em vez de resolver a programação dinâmica.

  Aqui a especificação diz explicitamente o que é exigido: o ÓTIMO. E a
  verificação recomputa a programação dinâmica. O custo é O(n · capacidade),
  declarado no `WorkSpec`, e é o publicador que paga.

BUG 11 — o verificador fazia o trabalho.

  `submit_solution` recebia só um nonce e rodava `run_work` no próprio nó.
  Quem verificava pagava o custo de executar. E o escrow escrevia endereços
  sintéticos direto em `balances`, sem passar pelas regras de estado.

  Aqui o executor entrega o RESULTADO. O verificador só confere. E o escrow
  é explícito, com contabilidade fechada.

Ponto flutuante removido da difusão. O protótipo usava `float64`. Mesmo que
o numpy seja consistente hoje, amarrar um protocolo a arredondamento de
ponto flutuante entre plataformas é uma dívida que cobra juros. A difusão
agora é aritmética inteira de ponto fixo, exata e igual em qualquer máquina.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum

import numpy as np

from . import crypto
from .units import checked_add, checked_sub

# ---------------------------------------------------------------------------
# Geração determinística de instâncias — sem o gerador do numpy
# ---------------------------------------------------------------------------

DOMAIN_INSTANCE = b"AURON-UTRAX-INSTANCE-v1"
DOMAIN_FREIVALDS = b"AURON-UTRAX-FREIVALDS-v1"

FREIVALDS_ROUNDS = 4
FREIVALDS_BITS = 20          # vetores em [0, 2^20)
FREIVALDS_ERROR_BITS = FREIVALDS_ROUNDS * FREIVALDS_BITS  # 80


def _ints_from_seed(seed: bytes, count: int, modulus: int, domain: bytes) -> list[int]:
    """Inteiros determinísticos em [0, modulus), derivados por SHA-512.

    Quatro bytes por valor. O viés de módulo é desprezível para os módulos
    usados aqui e, o que importa, é IDÊNTICO em qualquer implementação, que é
    o requisito real.
    """
    raw = crypto.xof(seed, count * 4, domain=domain)
    return [
        int.from_bytes(raw[i * 4 : i * 4 + 4], "big") % modulus for i in range(count)
    ]


class WorkType(str, Enum):
    MATRIX = "matrix"
    KNAPSACK = "knapsack"
    DIFFUSION = "diffusion"


@dataclass(frozen=True)
class WorkSpec:
    """O que a tarefa exige, quanto custa executar e quanto custa verificar.

    Nada aqui é derivado da dificuldade de consenso. Era esse acoplamento o
    bug 3 do protótipo.
    """

    work_type: WorkType
    size: int                # WORK_SIZE
    steps: int = 0           # só para difusão

    def execution_cost(self) -> str:
        if self.work_type == WorkType.MATRIX:
            return f"O(n^3) com n={self.size}"
        if self.work_type == WorkType.KNAPSACK:
            return f"O(n*C) com n={self.size}"
        return f"O(g^2*s) com g={self.size}, s={self.steps}"

    def verification_cost(self) -> str:
        """VERIFICATION_COST, declarado. Quem publica a tarefa sabe o que paga."""
        if self.work_type == WorkType.MATRIX:
            return (
                f"O(n^2) com n={self.size}, {FREIVALDS_ROUNDS} rodadas, "
                f"erro <= 2^-{FREIVALDS_ERROR_BITS}"
            )
        if self.work_type == WorkType.KNAPSACK:
            return f"O(n*C) com n={self.size} — igual à execução, por exigir o ótimo"
        return f"O(g^2*s) com g={self.size}, s={self.steps} — reexecução"

    def encode(self) -> bytes:
        return (
            self.work_type.value.encode("ascii")
            + b"|"
            + str(self.size).encode()
            + b"|"
            + str(self.steps).encode()
        )


# ---------------------------------------------------------------------------
# Tipo 1 — multiplicação de matrizes, verificada por Freivalds
# ---------------------------------------------------------------------------

MATRIX_ENTRY_MAX = 1000
MATRIX_MAX_SIZE = 1024  # acima disso a checagem inteira pode estourar int64


def generate_matrices(seed: bytes, size: int):
    if not 1 <= size <= MATRIX_MAX_SIZE:
        raise ValueError(f"tamanho de matriz fora da faixa: {size}")
    values = _ints_from_seed(seed, 2 * size * size, MATRIX_ENTRY_MAX, DOMAIN_INSTANCE)
    half = size * size
    a = np.array(values[:half], dtype=np.int64).reshape(size, size)
    b = np.array(values[half:], dtype=np.int64).reshape(size, size)
    return a, b


def matrix_work(seed: bytes, size: int) -> bytes:
    a, b = generate_matrices(seed, size)
    return (a @ b).tobytes()


def freivalds_verify(a: np.ndarray, b: np.ndarray, c: np.ndarray,
                     challenge_seed: bytes) -> bool:
    """Verifica C = A·B em O(n²), com desafio determinístico e imprevisível.

    O desafio vem de `challenge_seed`, que o chamador deriva incluindo o
    próprio C. Isso é o que impede tanto a divergência entre nós honestos
    (bug 1) quanto o grinding contra vetores fixos.
    """
    if a.shape != b.shape or a.shape != c.shape:
        return False
    if a.ndim != 2 or a.shape[0] != a.shape[1]:
        return False
    n = a.shape[0]
    if n > MATRIX_MAX_SIZE:
        return False

    # C precisa estar na faixa que um produto honesto pode gerar. Sem isto, um
    # executor entrega entradas perto de 2^63: a conta `c @ r` estoura o int64
    # e o numpy dá a volta em silêncio, então um resultado errado pode bater
    # com o certo naquilo que sobra. A faixa fecha essa porta e custa O(n²).
    limite = n * (MATRIX_ENTRY_MAX - 1) ** 2
    if int(c.min()) < 0 or int(c.max()) > limite:
        return False

    modulus = 1 << FREIVALDS_BITS
    values = _ints_from_seed(
        challenge_seed, FREIVALDS_ROUNDS * n, modulus, DOMAIN_FREIVALDS
    )
    for rnd in range(FREIVALDS_ROUNDS):
        r = np.array(values[rnd * n : (rnd + 1) * n], dtype=np.int64)
        if not np.array_equal(a @ (b @ r), c @ r):
            return False
    return True


def verify_matrix(seed: bytes, size: int, result: bytes) -> bool:
    if len(result) != size * size * 8:
        return False
    a, b = generate_matrices(seed, size)
    c = np.frombuffer(result, dtype=np.int64).reshape(size, size)
    # O desafio inclui o resultado alegado: sem isso, dá para procurar um C
    # errado que passe nos vetores fixos.
    challenge_seed = crypto.H(seed + size.to_bytes(4, "big") + result)
    return bool(freivalds_verify(a, b, c, challenge_seed))


# ---------------------------------------------------------------------------
# Tipo 2 — mochila 0/1. A especificação exige o ÓTIMO.
# ---------------------------------------------------------------------------

KNAPSACK_MAX_ITEMS = 256  # a máscara cabe em 32 bytes


def generate_knapsack(seed: bytes, n_items: int):
    if not 1 <= n_items <= KNAPSACK_MAX_ITEMS:
        raise ValueError(f"número de itens fora da faixa: {n_items}")
    weights = _ints_from_seed(seed + b"w", n_items, 99, DOMAIN_INSTANCE)
    values = _ints_from_seed(seed + b"v", n_items, 99, DOMAIN_INSTANCE)
    weights = [w + 1 for w in weights]
    values = [v + 1 for v in values]
    capacity = sum(weights) // 2
    return weights, values, capacity


def solve_knapsack(weights: list[int], values: list[int],
                   capacity: int) -> tuple[int, int]:
    """Programação dinâmica exata. Devolve (valor ótimo, máscara dos itens)."""
    n = len(weights)
    dp = np.zeros(capacity + 1, dtype=np.int64)
    taken = np.zeros((n, capacity + 1), dtype=np.uint8)
    for i in range(n):
        w, v = weights[i], values[i]
        if w > capacity:
            continue
        previous = dp.copy()
        dp[w:] = np.maximum(dp[w:], previous[: capacity + 1 - w] + v)
        taken[i] = dp != previous

    cap = capacity
    mask = 0
    for i in range(n - 1, -1, -1):
        if taken[i, cap]:
            mask |= 1 << i
            cap -= weights[i]
    return int(dp[capacity]), mask


def knapsack_work(seed: bytes, n_items: int) -> bytes:
    weights, values, capacity = generate_knapsack(seed, n_items)
    best, mask = solve_knapsack(weights, values, capacity)
    return best.to_bytes(8, "big") + mask.to_bytes(32, "big")


def verify_knapsack(seed: bytes, n_items: int, result: bytes) -> bool:
    """Confere que o resultado é o ÓTIMO, não apenas viável.

    Aceitar qualquer solução viável era o bug 2. Com aquela regra, 40 bytes
    de zeros passavam como trabalho válido.
    """
    if len(result) != 40:
        return False
    claimed = int.from_bytes(result[:8], "big")
    mask = int.from_bytes(result[8:], "big")
    if mask >> n_items:
        return False  # bits acima do número de itens

    weights, values, capacity = generate_knapsack(seed, n_items)

    total_w = total_v = 0
    for i in range(n_items):
        if mask & (1 << i):
            total_w += weights[i]
            total_v += values[i]
    if total_w > capacity:
        return False
    if total_v != claimed:
        return False

    best, _ = solve_knapsack(weights, values, capacity)
    return claimed == best


# ---------------------------------------------------------------------------
# Tipo 3 — difusão, em aritmética inteira de ponto fixo
# ---------------------------------------------------------------------------

DIFFUSION_SCALE = 1 << 20     # ponto fixo
DIFFUSION_RATE_NUM = 1        # taxa = 1/5 = 0.2
DIFFUSION_RATE_DEN = 5
DIFFUSION_BOUNDARY = DIFFUSION_SCALE // 2
DIFFUSION_MAX_GRID = 256
DIFFUSION_MAX_STEPS = 1024


def _trunc_div(values: np.ndarray, denominator: int) -> np.ndarray:
    """Divisão inteira truncando na direção do zero, sem passar por float.

    `//` do numpy arredonda para baixo, então -7 // 5 dá -2, e não -1. Numa
    grade com valores negativos isso vira um viés sistemático. E fazer
    `x * n / d` produziria float64, que é exatamente o que este módulo não
    pode ter.
    """
    negative = values < 0
    magnitude = np.abs(values) // denominator
    return np.where(negative, -magnitude, magnitude)


def diffusion_work(seed: bytes, grid_size: int, steps: int) -> bytes:
    """Difusão em ponto fixo inteiro. Sem float, sem divergência de plataforma."""
    if not 2 <= grid_size <= DIFFUSION_MAX_GRID:
        raise ValueError(f"grade fora da faixa: {grid_size}")
    if not 1 <= steps <= DIFFUSION_MAX_STEPS:
        raise ValueError(f"passos fora da faixa: {steps}")

    values = _ints_from_seed(
        seed, grid_size * grid_size, DIFFUSION_SCALE, DOMAIN_INSTANCE
    )
    grid = np.array(values, dtype=np.int64).reshape(grid_size, grid_size)

    for _ in range(steps):
        up = np.roll(grid, -1, axis=0)
        down = np.roll(grid, 1, axis=0)
        left = np.roll(grid, -1, axis=1)
        right = np.roll(grid, 1, axis=1)
        laplacian = up + down + left + right - 4 * grid
        # Aritmética inteira do começo ao fim. Nenhum float entra aqui.
        delta = _trunc_div(laplacian * DIFFUSION_RATE_NUM, DIFFUSION_RATE_DEN)
        grid = (grid + delta).astype(np.int64)
        grid[0, :] = grid[-1, :] = DIFFUSION_BOUNDARY
        grid[:, 0] = grid[:, -1] = DIFFUSION_BOUNDARY

    return grid.tobytes()


def verify_diffusion(seed: bytes, grid_size: int, steps: int, result: bytes) -> bool:
    """Sem prova curta: reexecuta. O custo está declarado no WorkSpec."""
    if len(result) != grid_size * grid_size * 8:
        return False
    return diffusion_work(seed, grid_size, steps) == result


# ---------------------------------------------------------------------------
# Despacho
# ---------------------------------------------------------------------------

def run_work(spec: WorkSpec, seed: bytes) -> bytes:
    if spec.work_type == WorkType.MATRIX:
        return matrix_work(seed, spec.size)
    if spec.work_type == WorkType.KNAPSACK:
        return knapsack_work(seed, spec.size)
    if spec.work_type == WorkType.DIFFUSION:
        return diffusion_work(seed, spec.size, spec.steps)
    raise ValueError(f"tipo de trabalho desconhecido: {spec.work_type}")


def verify_work(spec: WorkSpec, seed: bytes, result: bytes) -> bool:
    if spec.work_type == WorkType.MATRIX:
        return verify_matrix(seed, spec.size, result)
    if spec.work_type == WorkType.KNAPSACK:
        return verify_knapsack(seed, spec.size, result)
    if spec.work_type == WorkType.DIFFUSION:
        return verify_diffusion(seed, spec.size, spec.steps, result)
    return False


# ---------------------------------------------------------------------------
# Marketplace
# ---------------------------------------------------------------------------

DOMAIN_TASK_SEED = b"AURON-UTRAX-TASK-v1"


class TaskStatus(str, Enum):
    OPEN = "OPEN"
    SETTLED = "SETTLED"
    REFUNDED = "REFUNDED"


@dataclass
class Task:
    task_id: bytes
    poster: bytes
    spec: WorkSpec
    bounty: int
    status: TaskStatus = TaskStatus.OPEN
    solver: bytes | None = None
    result: bytes | None = None
    rejected_attempts: int = 0

    def seed_for(self, executor: bytes) -> bytes:
        """Semente da instância, amarrada ao executor.

        Sem o endereço do executor, dois executores recebem a mesma instância
        e o segundo copia a resposta do primeiro sem trabalhar.
        """
        return crypto.H(DOMAIN_TASK_SEED + self.task_id + executor)


@dataclass
class Marketplace:
    """Tarefas com bounty. A contabilidade fecha: nada some, nada aparece.

    Este é o modelo de referência. No Rust, o escrow vira regra de estado na
    cadeia, e não um dicionário à parte. O que precisa sobreviver à tradução
    são as regras: o executor entrega o resultado, submissão inválida não
    trava a tarefa, e o dinheiro sempre tem dono.
    """

    balances: dict = field(default_factory=dict)
    escrow: dict = field(default_factory=dict)     # task_id -> valor retido
    tasks: dict = field(default_factory=dict)

    def credit(self, address: bytes, amount: int) -> None:
        if amount <= 0:
            raise ValueError("crédito deve ser positivo")
        self.balances[address] = checked_add(self.balances.get(address, 0), amount)

    def balance(self, address: bytes) -> int:
        return self.balances.get(address, 0)

    def total_locked(self) -> int:
        return sum(self.escrow.values())

    def post_task(self, poster: bytes, spec: WorkSpec, bounty: int) -> Task:
        if bounty <= 0:
            raise ValueError("bounty deve ser inteiro positivo")
        if self.balance(poster) < bounty:
            raise ValueError("saldo insuficiente para o escrow")

        task_id = crypto.H(
            DOMAIN_TASK_SEED
            + poster
            + spec.encode()
            + bounty.to_bytes(8, "big")
            + len(self.tasks).to_bytes(8, "big")
        )[:32]

        self.balances[poster] = checked_sub(self.balance(poster), bounty)
        self.escrow[task_id] = bounty
        task = Task(task_id=task_id, poster=poster, spec=spec, bounty=bounty)
        self.tasks[task_id] = task
        return task

    def solve(self, task_id: bytes, executor: bytes) -> bytes:
        """Executa de fato o trabalho. Quem chama é o executor, não o verificador."""
        task = self.tasks[task_id]
        return run_work(task.spec, task.seed_for(executor))

    def submit(self, task_id: bytes, executor: bytes,
               result: bytes) -> tuple[bool, str]:
        """O executor entrega o RESULTADO. O verificador só confere.

        Submissão inválida não trava a tarefa: ela continua aberta. No
        protótipo, `submit` gravava o resultado antes de verificar e a tarefa
        ficava travada com lixo dentro.
        """
        task = self.tasks.get(task_id)
        if task is None:
            return False, "tarefa inexistente"
        if task.status != TaskStatus.OPEN:
            return False, f"tarefa não está aberta ({task.status.value})"

        seed = task.seed_for(executor)
        if not verify_work(task.spec, seed, result):
            task.rejected_attempts += 1
            return False, "resultado inválido; a tarefa continua aberta"

        payout = self.escrow.pop(task_id)
        self.credit(executor, payout)
        task.status = TaskStatus.SETTLED
        task.solver = executor
        task.result = result
        return True, "aceito e pago"

    def refund(self, task_id: bytes) -> int:
        """Devolve o bounty ao publicador. Só enquanto a tarefa está aberta."""
        task = self.tasks[task_id]
        if task.status != TaskStatus.OPEN:
            raise ValueError("só tarefa aberta pode ser devolvida")
        amount = self.escrow.pop(task_id)
        self.credit(task.poster, amount)
        task.status = TaskStatus.REFUNDED
        return amount

    def check_accounting(self, expected_total: int) -> None:
        """Saldos livres mais escrow precisam dar exatamente o total emitido."""
        circulating = sum(self.balances.values()) + self.total_locked()
        if circulating != expected_total:
            raise ValueError(
                f"contabilidade não fecha: {circulating} != {expected_total}"
            )
