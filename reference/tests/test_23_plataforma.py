# ✝ Lucas 16:10 — “Quem é fiel no mínimo, também é fiel no muito.”
"""Plataforma (V1): pedido assinado do agente, JOB da tarefa, preço,
liquidação e reputação."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import crypto, job, plataforma as p  # noqa: E402

SEGREDO = bytes(range(32))
CHAVE = crypto.ed25519_public_key(SEGREDO)
TABELA = {"cpu_nucleo_hora": 125_000, "ram_gib_hora": 100_000, "gpu_hora": 3_000_000,
          "ops_por_s_cpu": 1_000_000_000, "ops_por_s_gpu": 20_000_000_000}


def _recusa(f, *args) -> bool:
    try:
        f(*args)
    except ValueError:
        return True
    return False


def test_pedido_assinado_amarra_tudo():
    caminho, corpo, t = "/api/v1/agente/pulso", b'{"livre":4}', 1_791_000_000_000
    a = p.assinar_pedido(SEGREDO, "POST", caminho, t, corpo)
    assert p.conferir_pedido(CHAVE, "POST", caminho, t, corpo, a, t + 1000) is None
    assert p.conferir_pedido(CHAVE, "POST", caminho, t, corpo + b" ", a, t) == "assinatura do nó não confere"
    assert p.conferir_pedido(CHAVE, "POST", "/api/v1/agente/resultado", t, corpo, a, t) == "assinatura do nó não confere"
    assert p.conferir_pedido(CHAVE, "GET", caminho, t, corpo, a, t) == "assinatura do nó não confere"
    assert p.conferir_pedido(CHAVE, "POST", caminho, t, corpo, a, t + p.JANELA_MS) is None, "o limite entra"
    assert p.conferir_pedido(CHAVE, "POST", caminho, t, corpo, a, t + p.JANELA_MS + 1) == "pedido fora da janela de 5 minutos"
    assert p.conferir_pedido(CHAVE, "POST", caminho, t, corpo, a, t - p.JANELA_MS - 1) == "pedido fora da janela de 5 minutos"
    assert p.conferir_pedido(CHAVE, "PUT", caminho, t, corpo, a, t) == "método não aceito"
    assert p.conferir_pedido(CHAVE, "POST", "/api/v1/admin/x", t, corpo, a, t) == "caminho fora da área do agente"
    outra = crypto.ed25519_public_key(bytes(32))
    assert p.conferir_pedido(outra, "POST", caminho, t, corpo, a, t) == "assinatura do nó não confere"


def test_job_da_tarefa_separa_clientes():
    jid = crypto.H(b"job")
    a = p.job_da_tarefa(jid, 1, bytes(32))
    assert a != p.job_da_tarefa(jid, 2, bytes(32)), "outra tarefa, outro JOB"
    assert a != p.job_da_tarefa(jid, 1, bytes([1] * 32)), "outro nonce, outro JOB"
    assert p.semente(a, 0) == job.semente_da_unidade(a, 0)
    assert p.semente(a, 0) != p.semente(a, 1)
    assert _recusa(p.job_da_tarefa, jid[:32], 1, bytes(32))


def test_preco_por_hora_de_referencia():
    hora = 1_000_000_000 * 3600
    # uma hora de núcleo de referência, sem memória: R$ 0,125
    assert p.custo_da_execucao(TABELA, hora, 0, False) == 125_000
    # com 2 GiB por uma hora: + 2 x R$ 0,10
    assert p.custo_da_execucao(TABELA, hora, 2 * p.GIB, False) == 325_000
    # GPU: uma hora da GPU de referência
    assert p.custo_da_execucao(TABELA, 20_000_000_000 * 3600, 0, True) == 3_000_000
    # arredonda para cima: uma operação custa 1 micro
    assert p.custo_da_execucao(TABELA, 1, 0, False) == 1
    assert p.custo_da_execucao(TABELA, 0, 0, False) == 0
    assert p.centavos(1) == 1 and p.centavos(10_000) == 1 and p.centavos(10_001) == 2 and p.centavos(0) == 0
    assert _recusa(p.custo_da_execucao, TABELA, p.OPS_MAX + 1, 0, False)
    assert _recusa(p.custo_da_execucao, TABELA, 1, p.MEMORIA_MAX + 1, False)
    assert _recusa(p.custo_da_execucao, dict(TABELA, ops_por_s_cpu=0), 1, 0, False)
    assert _recusa(p.custo_da_execucao, dict(TABELA, gpu_hora=p.PRECO_MAX + 1), 1, 0, False)
    assert p.estimativa(TABELA, hora, 0, 10, 2, False) == 125_000 * 20
    assert _recusa(p.estimativa, dict(TABELA, ops_por_s_cpu=1), p.OPS_MAX, p.MEMORIA_MAX, 1 << 40, 7, False)


def test_liquidacao_fecha_ao_centavo():
    r = p.liquidar([(1, 333_334, "cliente"), (2, 333_333, "cliente"), (3, 333_333, "cliente")], 7_000)
    assert r["cobranca"] == 100
    assert sum(r["fornecedores"].values()) == 70
    assert r["fornecedores"] == {1: 24, 2: 23, 3: 23}
    assert r["plataforma"] == 30 and r["conferencia"] == 0
    # empate: o nó de número menor leva o centavo
    r = p.liquidar([(9, 10_000, "cliente"), (4, 10_000, "cliente")], 5_000)
    assert r["cobranca"] == 2 and r["fornecedores"] == {4: 1, 9: 0} and r["plataforma"] == 1
    # conferência paga pela plataforma
    r = p.liquidar([(1, 1_000_000, "cliente"), (2, 1_000_000, "plataforma")], 7_000)
    assert r == {"cobranca": 100, "fornecedores": {1: 70, 2: 70}, "conferencia": 70, "plataforma": -40}
    # nada aceito, nada cobrado
    assert p.liquidar([], 7_000) == {"cobranca": 0, "fornecedores": {}, "conferencia": 0, "plataforma": 0}
    assert _recusa(p.liquidar, [], 10_001)


def test_reputacao():
    assert p.pontuacao(0, 0, 0) == 70, "nó novo é normal"
    assert p.pontuacao(100, 0, 0) == 97
    assert p.pontuacao(1_000_000, 0, 0) == 99
    assert p.pontuacao(100, 0, 1) == 79
    assert p.pontuacao(0, 0, 1) == 20
    assert p.faixa(100) == "excelente" and p.faixa(90) == "confiavel" and p.faixa(70) == "normal"
    assert p.faixa(50) == "atencao" and p.faixa(49) == "risco"
    assert p.amostragem_bp(97, 100) == 1_000 and p.amostragem_bp(70, 10) == 2_500 and p.amostragem_bp(20, 50) == 10_000
    assert p.amostragem_bp(99, 9) == 10_000, "período de prova: tudo conferido"
    assert p.quarentena(49, 0) and p.quarentena(95, 3) and not p.quarentena(70, 2)
