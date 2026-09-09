"""Utrax: trabalho util fora do consenso.

Testes de regressao para os bugs 1, 2, 8 e 11 do prototipo.
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import crypto, utrax  # noqa: E402
from auron.utrax import (  # noqa: E402
    Marketplace,
    TaskStatus,
    WorkSpec,
    WorkType,
)

SEED = crypto.H(b"instancia de teste")


# ---------------------------------------------------------------------------
# BUG 1 — Freivalds
# ---------------------------------------------------------------------------

def test_freivalds_is_deterministic():
    """Dois nos honestos precisam chegar ao MESMO veredicto.

    No prototipo, `np.random.default_rng()` sem semente fazia cada no sortear
    vetores diferentes.
    """
    result = utrax.matrix_work(SEED, 16)
    verdicts = {utrax.verify_matrix(SEED, 16, result) for _ in range(20)}
    assert verdicts == {True}, "verificacao nao e determinista"

    # e um resultado errado precisa ser rejeitado sempre, nao as vezes
    a, b = utrax.generate_matrices(SEED, 16)
    wrong = (a @ b)
    wrong[0, 0] += 1
    verdicts = {utrax.verify_matrix(SEED, 16, wrong.tobytes()) for _ in range(20)}
    assert verdicts == {False}, "resultado errado passou em alguma execucao"
    print("PASS Freivalds determinista em 20 execucoes")


def test_freivalds_does_not_use_numpy_rng():
    """O fluxo de bits do numpy e dependencia de versao. Nada de consenso pode usar."""
    import inspect
    src = inspect.getsource(utrax.freivalds_verify) + inspect.getsource(utrax._ints_from_seed)
    for banned in ("default_rng", "np.random", "RandomState"):
        assert banned not in src, f"{banned} presente no caminho de verificacao"
    print("PASS verificacao nao depende do gerador do numpy")


def test_freivalds_challenge_depends_on_claimed_result():
    """O desafio precisa mudar quando o resultado muda, senao da para fazer grinding."""
    good = utrax.matrix_work(SEED, 8)
    a, b = utrax.generate_matrices(SEED, 8)
    bad = (a @ b)
    bad[3, 3] += 7
    bad_bytes = bad.tobytes()

    s1 = crypto.H(SEED + (8).to_bytes(4, "big") + good)
    s2 = crypto.H(SEED + (8).to_bytes(4, "big") + bad_bytes)
    assert s1 != s2, "desafio nao depende do resultado alegado"
    assert utrax.verify_matrix(SEED, 8, good)
    assert not utrax.verify_matrix(SEED, 8, bad_bytes)
    print("PASS desafio derivado do proprio resultado")


def test_freivalds_catches_every_single_cell_error():
    size = 12
    a, b = utrax.generate_matrices(SEED, size)
    c = a @ b
    for i in range(size):
        for j in range(size):
            bad = c.copy()
            bad[i, j] += 1
            assert not utrax.verify_matrix(SEED, size, bad.tobytes()), \
                f"erro em ({i},{j}) passou"
    assert utrax.verify_matrix(SEED, size, c.tobytes())
    print(f"PASS Freivalds pega os {size * size} erros de celula unica")


def test_matrix_rejects_wrong_length():
    assert not utrax.verify_matrix(SEED, 8, b"")
    assert not utrax.verify_matrix(SEED, 8, b"\x00" * 10)
    print("PASS matriz de tamanho errado rejeitada")


# ---------------------------------------------------------------------------
# BUG 2 e BUG 8 — mochila
# ---------------------------------------------------------------------------

def test_knapsack_rejects_all_zeros():
    """BUG 2: 40 bytes de zeros eram aceitos como trabalho valido.

    Mascara vazia, peso 0 <= capacidade, valor 0 == valor declarado 0.
    Passava. Este e o teste que faltava.
    """
    zeros = b"\x00" * 40
    for n in (5, 12, 20, 40):
        assert not utrax.verify_knapsack(SEED, n, zeros), \
            f"40 bytes de zeros aceitos com n={n}"
    print("PASS mochila rejeita 40 bytes de zeros")


def test_knapsack_rejects_feasible_but_not_optimal():
    """Solucao viavel qualquer nao e trabalho. So o otimo conta."""
    n = 16
    weights, values, capacity = utrax.generate_knapsack(SEED, n)
    best, best_mask = utrax.solve_knapsack(weights, values, capacity)

    # constroi uma solucao viavel pior: tira um item da mascara otima
    for i in range(n):
        if best_mask & (1 << i):
            worse_mask = best_mask & ~(1 << i)
            break
    else:
        raise AssertionError("mascara otima vazia; instancia degenerada")

    worse_value = sum(values[i] for i in range(n) if worse_mask & (1 << i))
    assert worse_value < best
    forged = worse_value.to_bytes(8, "big") + worse_mask.to_bytes(32, "big")
    assert not utrax.verify_knapsack(SEED, n, forged), \
        "solucao viavel porem sub-otima foi aceita"

    honest = utrax.knapsack_work(SEED, n)
    assert utrax.verify_knapsack(SEED, n, honest)
    print("PASS mochila exige o otimo, nao so viabilidade")


def test_knapsack_rejects_infeasible_and_lying_value():
    n = 14
    weights, values, capacity = utrax.generate_knapsack(SEED, n)
    best, _ = utrax.solve_knapsack(weights, values, capacity)

    # todos os itens: quase certamente estoura a capacidade
    full_mask = (1 << n) - 1
    total_v = sum(values)
    infeasible = total_v.to_bytes(8, "big") + full_mask.to_bytes(32, "big")
    assert not utrax.verify_knapsack(SEED, n, infeasible), "solucao inviavel aceita"

    # valor declarado maior que o otimo, com mascara otima
    honest = utrax.knapsack_work(SEED, n)
    lying = (best + 1).to_bytes(8, "big") + honest[8:]
    assert not utrax.verify_knapsack(SEED, n, lying), "valor inflado aceito"

    # bits acima do numero de itens
    overflow_mask = 1 << (n + 3)
    bogus = (0).to_bytes(8, "big") + overflow_mask.to_bytes(32, "big")
    assert not utrax.verify_knapsack(SEED, n, bogus), "mascara fora da faixa aceita"
    print("PASS mochila rejeita inviavel, valor inflado e mascara fora da faixa")


# ---------------------------------------------------------------------------
# Difusao sem ponto flutuante
# ---------------------------------------------------------------------------

def test_diffusion_is_integer_only():
    """Nenhuma divisao verdadeira e nenhum literal float no caminho da difusao.

    A checagem e feita na arvore sintatica, nao por substring: comentario que
    fala de float nao pode reprovar o teste, e um `/` escondido dentro de uma
    string nao pode aprovar.
    """
    import ast
    import inspect
    import textwrap

    for fn in (utrax.diffusion_work, utrax._trunc_div):
        tree = ast.parse(textwrap.dedent(inspect.getsource(fn)))
        for node in ast.walk(tree):
            assert not isinstance(node, ast.Div), \
                f"{fn.__name__} usa divisao verdadeira, que produz float"
            if isinstance(node, ast.Constant):
                assert not isinstance(node.value, float), \
                    f"{fn.__name__} tem literal float: {node.value}"

    # e o resultado tem que sair inteiro de verdade
    out = utrax.diffusion_work(SEED, 8, 5)
    grid = np.frombuffer(out, dtype=np.int64)
    assert grid.dtype == np.int64
    assert len(out) == 8 * 8 * 8
    print("PASS difusao e inteira do comeco ao fim")


def test_diffusion_deterministic_and_verified():
    a = utrax.diffusion_work(SEED, 10, 7)
    b = utrax.diffusion_work(SEED, 10, 7)
    assert a == b, "difusao nao e determinista"
    assert utrax.verify_diffusion(SEED, 10, 7, a)
    assert not utrax.verify_diffusion(SEED, 10, 7, b"\x00" * len(a))
    assert not utrax.verify_diffusion(SEED, 10, 8, a), "passos diferentes bateram"
    print("PASS difusao determinista e verificavel")


def test_trunc_div_rounds_toward_zero():
    values = np.array([-7, -5, -3, -1, 0, 1, 3, 5, 7], dtype=np.int64)
    got = utrax._trunc_div(values, 5)
    expected = np.array([-1, -1, 0, 0, 0, 0, 0, 1, 1], dtype=np.int64)
    assert np.array_equal(got, expected), f"{got} != {expected}"
    # `//` do numpy arredondaria para baixo e daria -2 no primeiro
    assert (values // 5)[0] == -2
    print("PASS divisao trunca na direcao do zero")


# ---------------------------------------------------------------------------
# BUG 11 — marketplace
# ---------------------------------------------------------------------------

def test_task_seed_bound_to_executor():
    """Sem o endereco do executor na semente, o segundo copia o primeiro."""
    mkt = Marketplace()
    poster = b"\x01" * 20
    mkt.credit(poster, 1000)
    task = mkt.post_task(poster, WorkSpec(WorkType.MATRIX, 8), 100)

    exec_a, exec_b = b"\x0a" * 20, b"\x0b" * 20
    assert task.seed_for(exec_a) != task.seed_for(exec_b)

    result_a = mkt.solve(task.task_id, exec_a)
    # copiar o resultado do outro nao funciona
    ok, msg = mkt.submit(task.task_id, exec_b, result_a)
    assert not ok, "copia do resultado alheio foi aceita"
    assert task.status == TaskStatus.OPEN
    print("PASS semente amarrada ao executor bloqueia copia")


def test_invalid_submission_does_not_lock_task():
    """No prototipo, `submit` gravava antes de verificar e travava a tarefa."""
    mkt = Marketplace()
    poster = b"\x01" * 20
    mkt.credit(poster, 1000)
    task = mkt.post_task(poster, WorkSpec(WorkType.KNAPSACK, 12), 100)
    executor = b"\x0a" * 20

    for _ in range(5):
        ok, msg = mkt.submit(task.task_id, executor, b"\x00" * 40)
        assert not ok
        assert "continua aberta" in msg
        assert task.status == TaskStatus.OPEN, "tarefa travou com lixo"

    assert task.rejected_attempts == 5
    good = mkt.solve(task.task_id, executor)
    ok, msg = mkt.submit(task.task_id, executor, good)
    assert ok, msg
    assert task.status == TaskStatus.SETTLED
    assert mkt.balance(executor) == 100
    print("PASS submissao invalida nao trava a tarefa")


def test_marketplace_accounting_closes():
    """Nada some, nada aparece. O escrow do prototipo escrevia direto em balances."""
    mkt = Marketplace()
    poster = b"\x01" * 20
    executor = b"\x0a" * 20
    total = 10_000
    mkt.credit(poster, total)
    mkt.check_accounting(total)

    tasks = []
    for i, spec in enumerate([
        WorkSpec(WorkType.MATRIX, 8),
        WorkSpec(WorkType.KNAPSACK, 10),
        WorkSpec(WorkType.DIFFUSION, 6, steps=3),
    ]):
        tasks.append(mkt.post_task(poster, spec, 500 + i))
        mkt.check_accounting(total)

    # uma resolvida, uma devolvida, uma deixada aberta
    result = mkt.solve(tasks[0].task_id, executor)
    ok, msg = mkt.submit(tasks[0].task_id, executor, result)
    assert ok, msg
    mkt.check_accounting(total)

    mkt.refund(tasks[1].task_id)
    mkt.check_accounting(total)

    assert tasks[2].status == TaskStatus.OPEN
    assert mkt.total_locked() == tasks[2].bounty
    mkt.check_accounting(total)
    print("PASS contabilidade do marketplace fecha em todos os caminhos")


def test_cannot_post_without_funds():
    mkt = Marketplace()
    poster = b"\x01" * 20
    mkt.credit(poster, 10)
    try:
        mkt.post_task(poster, WorkSpec(WorkType.MATRIX, 8), 100)
        raise AssertionError("tarefa publicada sem saldo")
    except ValueError:
        pass
    assert mkt.balance(poster) == 10, "saldo mexido numa publicacao que falhou"
    print("PASS nao publica tarefa sem lastro")


def test_settled_task_cannot_be_paid_twice():
    mkt = Marketplace()
    poster, executor = b"\x01" * 20, b"\x0a" * 20
    mkt.credit(poster, 1000)
    task = mkt.post_task(poster, WorkSpec(WorkType.MATRIX, 8), 300)
    result = mkt.solve(task.task_id, executor)
    assert mkt.submit(task.task_id, executor, result)[0]

    ok, msg = mkt.submit(task.task_id, executor, result)
    assert not ok and "não está aberta" in msg
    assert mkt.balance(executor) == 300, "pagou duas vezes"
    print("PASS tarefa liquidada nao paga de novo")


def test_workspec_declares_costs():
    """VERIFICATION_COST precisa ser declarado, nao descoberto na hora."""
    for spec in (
        WorkSpec(WorkType.MATRIX, 64),
        WorkSpec(WorkType.KNAPSACK, 32),
        WorkSpec(WorkType.DIFFUSION, 16, steps=8),
    ):
        assert spec.execution_cost()
        assert spec.verification_cost()
    # a matriz e o unico tipo com verificacao assintoticamente mais barata
    assert "n^2" in WorkSpec(WorkType.MATRIX, 64).verification_cost()
    assert "n^3" in WorkSpec(WorkType.MATRIX, 64).execution_cost()
    print("PASS WorkSpec declara custo de execucao e de verificacao")


if __name__ == "__main__":
    test_freivalds_is_deterministic()
    test_freivalds_does_not_use_numpy_rng()
    test_freivalds_challenge_depends_on_claimed_result()
    test_freivalds_catches_every_single_cell_error()
    test_matrix_rejects_wrong_length()
    test_knapsack_rejects_all_zeros()
    test_knapsack_rejects_feasible_but_not_optimal()
    test_knapsack_rejects_infeasible_and_lying_value()
    test_diffusion_is_integer_only()
    test_diffusion_deterministic_and_verified()
    test_trunc_div_rounds_toward_zero()
    test_task_seed_bound_to_executor()
    test_invalid_submission_does_not_lock_task()
    test_marketplace_accounting_closes()
    test_cannot_post_without_funds()
    test_settled_task_cannot_be_paid_twice()
    test_workspec_declares_costs()
    print("=== UTRAX OK ===")
