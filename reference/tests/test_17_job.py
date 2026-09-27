# ✝ Neemias 4:6 — “Assim edificamos o muro, porque o povo tinha ânimo para trabalhar.”
"""JOB do ULTRAX: validação, sementes das unidades, intervalos e resumo."""

import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import crypto, job  # noqa: E402


def _job(**mudar):
    campos = dict(dominio=7, tipo=1, tamanho=64, passos=0, parametros=(), unidades=10, nivel=2,
                  redundancia=1, prazo_s=0, orcamento_milicreditos=0, descricao="teste")
    campos.update(mudar)
    return job.Job(**campos)


def test_dominio_sem_motor_e_recusado():
    for dominio in (8, 9, 10, 11):
        try:
            _job(dominio=dominio).validar()
        except ValueError as e:
            assert "sem motor" in str(e)
            continue
        raise AssertionError(f"domínio {dominio} sem motor foi aceito")


def test_concordancia_exige_redundancia():
    for campos in (dict(nivel=3, redundancia=1), dict(unidades=0), dict(unidades=job.UNIDADES_MAX + 1),
                   dict(descricao="x" * 2001), dict(descricao="a\x00b"), dict(tipo=5)):
        try:
            _job(**campos).validar()
        except ValueError:
            continue
        raise AssertionError(f"aceitou {campos}")
    _job(nivel=3, redundancia=2).validar()


def test_id_muda_com_qualquer_campo_e_semente_com_o_indice():
    base = _job()
    assert base.id() != _job(unidades=11).id()
    assert base.id() != _job(descricao="teste.").id()
    jid = base.id()
    sementes = {job.semente_da_unidade(jid, i) for i in range(1000)}
    assert len(sementes) == 1000


def test_intervalos_iguais_a_um_conjunto():
    rng = random.Random(7)
    iv = job.Intervalos()
    feitas = set()
    for _ in range(3000):
        a = rng.randrange(0, 500)
        b = a + rng.randrange(0, 6)
        iv.inserir(a, b)
        feitas.update(range(a, b))
        # forma canônica: ordenadas, disjuntas, sem encostar
        for (x1, y1), (x2, y2) in zip(iv.faixas, iv.faixas[1:]):
            assert x1 < y1 < x2 < y2
    assert iv.concluidas() == len(feitas)
    for i in range(520):
        assert iv.contem(i) == (i in feitas)
    faltantes = [i for i in range(510) if i not in feitas]
    assert iv.primeira_faltante(0, 510) == (faltantes[0] if faltantes else None)


def test_resumo_nao_depende_da_ordem_e_acusa_troca():
    pares = [(i, crypto.H(bytes([i]))) for i in range(40)]
    embaralhados = pares[:]
    random.Random(3).shuffle(embaralhados)
    assert job.resumo(pares) == job.resumo(embaralhados)
    trocado = pares[:]
    trocado[5] = (5, crypto.H(b"outro"))
    assert job.resumo(pares) != job.resumo(trocado)


def test_creditos_v1():
    assert job.milicreditos(999_999, 0) == 0
    assert job.milicreditos(10**9, 0) == 1000


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
