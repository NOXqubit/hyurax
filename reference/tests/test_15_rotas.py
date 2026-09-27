# ✝ Provérbios 16:9 — “O coração do homem planeja o seu caminho.”
"""Rotas do ULTRAX: gerador, instância, 2-opt contra o ótimo por força bruta,
certificados, recusas e cancelamento."""

import itertools
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import crypto, rotas  # noqa: E402


def test_xoshiro256ss_bate_com_os_valores_publicados():
    # estado {1, 2, 3, 4}: as saídas de referência do xoshiro256**
    g = rotas.Gerador([1, 2, 3, 4])
    assert [g.proximo() for _ in range(4)] == [11520, 0, 1509978240, 1215971899390074240]
    assert rotas.Gerador([0, 0, 0, 0]).s == [1, 0, 0, 0], "estado todo zero vira [1, 0, 0, 0]"


def test_instancia_nao_depende_de_n_e_distancia_e_inteira():
    doze = rotas.coordenadas(7, 12)
    assert rotas.coordenadas(7, 20)[:12] == doze
    assert all(0 <= x < rotas.LADO and 0 <= y < rotas.LADO for x, y in doze)
    assert rotas.distancia((0, 0), (3, 4)) == 5
    assert rotas.distancia((0, 0), (1, 1)) == 1, "piso da raiz de 2"


def test_partida_e_permutacao_e_depende_da_semente():
    a = rotas.partida(crypto.H(b"a"), 50)
    b = rotas.partida(crypto.H(b"b"), 50)
    assert sorted(a) == list(range(50)) and sorted(b) == list(range(50))
    assert a != b


def _otimo_por_forca_bruta(cidades):
    n = len(cidades)
    melhor = None
    for resto in itertools.permutations(range(1, n)):
        rota = [0, *resto]
        c = rotas.comprimento_da_rota(cidades, rota)
        melhor = c if melhor is None else min(melhor, c)
    return melhor


def test_dois_opt_contra_o_otimo_em_instancias_pequenas():
    acertos = 0
    for instancia in range(12):
        n = 7
        cidades = rotas.coordenadas(instancia, n)
        otimo = _otimo_por_forca_bruta(cidades)
        melhor = None
        for k in range(6):
            bytes_, _ = rotas.executar(n, 100, [instancia], crypto.H(b"partida %d" % k))
            comprimento = rotas.certificar_otimo_local(n, 100, instancia, bytes_)
            assert comprimento >= otimo, "nenhum 2-opt fica abaixo do ótimo"
            melhor = comprimento if melhor is None else min(melhor, comprimento)
        acertos += melhor == otimo
    # com 6 partidas em 7 cidades, o multi-partida quase sempre acha o ótimo;
    # o teste só exige a maioria, e o número aparece para quem roda
    print(f"  multi-partida achou o ótimo em {acertos} de 12 instâncias de 7 cidades")
    assert acertos >= 8


def test_resultado_e_otimo_local_canonico_e_encurta():
    n, passos, inst = 60, 1000, 3
    bytes_, ops = rotas.executar(n, passos, [inst], crypto.H(b"x"))
    comprimento, inicial, passadas, melhorias, otimo, rota = rotas.decodificar(bytes_, n)
    assert otimo == 1 and passadas < passos
    assert comprimento < inicial and melhorias > 0
    assert rota[0] == 0 and rota[1] < rota[-1] and sorted(rota) == list(range(n))
    assert rotas.certificar_otimo_local(n, passos, inst, bytes_) == comprimento
    assert ops > n and bytes_ == rotas.executar(n, passos, [inst], crypto.H(b"x"))[0], "determinístico"


def test_passadas_esgotadas_nao_afirmam_otimo():
    n, inst = 80, 5
    bytes_, _ = rotas.executar(n, 1, [inst], crypto.H(b"curta"))
    _, _, passadas, _, otimo, _ = rotas.decodificar(bytes_, n)
    assert (passadas, otimo) == (1, 0)
    rotas.certificar(n, 1, inst, bytes_)


def _recusa(funcao, *args):
    try:
        funcao(*args)
    except rotas.Recusa as e:
        return str(e)
    raise AssertionError("devia recusar")


def test_adulteracoes_sao_recusadas():
    n, passos, inst = 20, 100, 9
    semente = crypto.H(b"honesta")
    bom, _ = rotas.executar(n, passos, [inst], semente)
    rotas.verificar(n, passos, [inst], semente, bom)
    cab = rotas.CABECALHO
    repetida = bytearray(bom)
    repetida[cab + 2:cab + 4] = repetida[cab + 4:cab + 6]
    assert "duas vezes" in _recusa(rotas.certificar, n, passos, inst, bytes(repetida))
    mentirosa = bytearray(bom)
    mentirosa[0:8] = (int.from_bytes(bom[0:8], "big") - 1).to_bytes(8, "big")
    assert "mede" in _recusa(rotas.certificar, n, passos, inst, bytes(mentirosa))
    _recusa(rotas.certificar, n, passos, inst, bom[:-1])
    # rota válida de outra semente: passa no certificado, mas não é a desta semente
    outra, _ = rotas.executar(n, passos, [inst], crypto.H(b"outra"))
    rotas.certificar(n, passos, inst, outra)
    if outra != bom:
        _recusa(rotas.verificar, n, passos, [inst], semente, outra)


def test_cancelamento_e_faixas():
    try:
        rotas.executar(1500, 10, [1], crypto.H(b"c"), continuar=lambda ops: False)
    except rotas.Cancelado:
        pass
    else:
        raise AssertionError("continuar=False devia cancelar")
    for tamanho, passos, parametros in ((3, 1, [0]), (2001, 1, [0]), (10, 0, [0]), (10, 10_001, [0]), (10, 1, []), (2000, 10_000, [0])):
        try:
            rotas.validar(tamanho, passos, parametros)
        except ValueError:
            continue
        raise AssertionError(f"aceitou {tamanho}, {passos}, {parametros}")


if __name__ == "__main__":
    for nome, f in list(globals().items()):
        if nome.startswith("test_"):
            f()
            print("ok", nome)
