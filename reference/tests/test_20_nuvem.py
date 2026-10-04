# ✝ Atos 2:44 — “Todos os que criam estavam juntos e tinham tudo em comum.”
"""Nuvem: Reed–Solomon, anúncio de máquina, recibo, mensagens e livro."""

import itertools
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyurax import codec, crypto, nuvem  # noqa: E402

SEGREDO = bytes(range(32))
OUTRO = bytes(range(1, 33))


def _recusada(f, dados: bytes) -> bool:
    try:
        f(dados)
    except (ValueError, codec.CodecError):
        return True
    return False


def test_gf_inverso_e_produto():
    for a in range(1, 256):
        assert nuvem.gf_mul(a, nuvem.gf_inv(a)) == 1
    assert nuvem.gf_mul(0, 7) == 0
    assert nuvem.gf_mul(2, 0x80) == 0x1D  # x^8 = x^4 + x^3 + x^2 + 1


def test_qualquer_k_fragmentos_reconstroem():
    dados = bytes((i * 37 + 11) % 256 for i in range(1000))
    for k, m in [(1, 0), (1, 2), (2, 1), (3, 2), (4, 4)]:
        frags = nuvem.codificar(dados, k, m)
        assert len(frags) == k + m
        assert all(len(f) == nuvem.tamanho_do_fragmento(len(dados), k) for f in frags)
        assert b"".join(frags[:k])[: len(dados)] == dados, "sistemático: os k primeiros são os dados"
        for escolhidos in itertools.combinations(range(k + m), k):
            assert nuvem.reconstruir({i: frags[i] for i in escolhidos}, k, m, len(dados)) == dados


def test_poucos_fragmentos_ou_adulterado_nao_reconstroem_igual():
    dados = b"conteudo cifrado de verdade" * 3
    frags = nuvem.codificar(dados, 3, 2)
    assert _recusada(lambda _: nuvem.reconstruir({0: frags[0], 4: frags[4]}, 3, 2, len(dados)), b"")
    torto = bytearray(frags[4])
    torto[0] ^= 1
    assert nuvem.reconstruir({0: frags[0], 1: frags[1], 4: bytes(torto)}, 3, 2, len(dados)) != dados
    # o hash de cada fragmento (no manifesto do dono) é quem pega a adulteração
    assert crypto.H(bytes(torto)) != crypto.H(frags[4])


def test_arquivo_vazio_e_limites():
    assert nuvem.codificar(b"", 2, 1) == [b"\x00", b"\x00", b"\x00"]
    assert nuvem.reconstruir({1: b"\x00", 2: b"\x00"}, 2, 1, 0) == b""
    assert _recusada(lambda _: nuvem.codificar(b"x", 0, 1), b"")
    assert _recusada(lambda _: nuvem.codificar(b"x", 33, 0), b"")
    assert _recusada(lambda _: nuvem.codificar(b"x", 2, 33), b"")


def _anuncio(**muda):
    a = {
        "identidade": bytes([9]) * 32, "tipo": 1, "instante_ms": 1_790_000_000_000, "validade_s": 3600,
        "linhas": 4, "ram_mib": 8192, "gpu": "Intel HD 400", "vram_mib": 512, "disco_mib": 10_240,
        "preco_credito_mili": 1200, "preco_gb_mes_mili": 50_000, "preco_venda_centavos": 0,
        "descricao": "PC de casa, ligado à noite", "creditos_hora": 3_600,
    }
    a.update(muda)
    return a


def test_anuncio_assinado_vai_e_volta_e_adulterado_nao_confere():
    corpo = nuvem.anuncio(SEGREDO, _anuncio())
    m = nuvem.ler(nuvem.msg_anuncio(corpo))
    a = m["anuncio"]
    assert a["worker"] == crypto.ed25519_public_key(SEGREDO)
    assert nuvem.anuncio_confere(a)
    assert not nuvem.anuncio_confere(dict(a, preco_credito_mili=1))
    assert _recusada(nuvem.ler, nuvem.msg_anuncio(nuvem.anuncio(SEGREDO, _anuncio())) + b"\x00")
    assert _recusada(lambda _: nuvem.anuncio(SEGREDO, _anuncio(tipo=4)), b"")
    assert _recusada(lambda _: nuvem.anuncio(SEGREDO, _anuncio(validade_s=0)), b"")
    assert _recusada(lambda _: nuvem.anuncio(SEGREDO, _anuncio(descricao="x" * 281)), b"")


def test_recibo_confere_e_total_e_amarrado():
    job = crypto.H(b"job")
    fornecedor = crypto.ed25519_public_key(OUTRO)
    corpo = nuvem.recibo(SEGREDO, fornecedor, job, 2_500, 1_200, 1_790_000_000_000)
    rc = nuvem.ler(nuvem.msg_recibo(corpo))["recibo"]
    assert rc["total_mili"] == 3_000
    assert nuvem.recibo_confere(rc)
    assert not nuvem.recibo_confere(dict(rc, fornecedor=crypto.ed25519_public_key(SEGREDO)))
    torto = bytearray(nuvem.msg_recibo(corpo))
    torto[2 + 32 + 32 + 64 + 16 + 7] ^= 1  # total
    assert _recusada(nuvem.ler, bytes(torto))


def test_mensagens_de_armazenamento_vao_e_voltam():
    arq = bytes([5]) * 32
    h = crypto.H(b"frag")
    n = bytes([6]) * 32
    assert nuvem.ler(nuvem.guardar(arq, 3, 100, h))["hash"] == h
    assert nuvem.ler(nuvem.parte(arq, 3, 64, b"abc"))["dados"] == b"abc"
    assert nuvem.ler(nuvem.guardado(arq, 3, False, "sem espaço"))["motivo"] == "sem espaço"
    assert nuvem.ler(nuvem.desafio(arq, 3, n))["nonce"] == n
    assert nuvem.ler(nuvem.msg_prova(arq, 3, n, h))["h"] == h
    assert nuvem.ler(nuvem.buscar(arq, 63))["indice"] == 63
    assert nuvem.ler(nuvem.entrega(arq, 0, False, 0, bytes(64)))["tem"] is False
    assert nuvem.ler(nuvem.apagar(arq, 1))["subtipo"] == "apagar"
    assert _recusada(nuvem.ler, nuvem.buscar(arq, 1)[:-1] + bytes([64]))
    assert _recusada(nuvem.ler, bytes([1, 3]) + arq + bytes([0]) + bytes(4) + codec.enc_bytes(b""))
    assert _recusada(nuvem.ler, bytes([1, 11]))
    assert _recusada(nuvem.ler, bytes([2, 7]) + arq + bytes([0]))


def test_prova_depende_do_fragmento_inteiro():
    f = bytes(range(200))
    n = bytes(32)
    assert nuvem.prova(n, f) != nuvem.prova(n, f[:-1])
    assert nuvem.prova(n, f) != nuvem.prova(bytes([1]) + n[1:], f)


def test_livro_encadeia_e_repartir_soma_o_total():
    zero = bytes(64)
    h1 = nuvem.hash_da_linha(zero, 1, 10, "consumo", 1, bytes(64), bytes(32), 1000, "JOB")
    h2 = nuvem.hash_da_linha(h1, 2, 10, "provedor", 1, bytes(64), bytes(32), 800, "")
    assert h2 != nuvem.hash_da_linha(zero, 2, 10, "provedor", 1, bytes(64), bytes(32), 800, "")
    assert _recusada(lambda _: nuvem.hash_da_linha(zero, 1, 1, "saque", 0, bytes(64), bytes(32), 1, ""), b"")
    for total in (0, 1, 7, 999, 10**12):
        p, pl, r = nuvem.repartir(total, 80, 15)
        assert p + pl + r == total and r >= 0
    assert _recusada(lambda _: nuvem.repartir(1, 90, 20), b"")
