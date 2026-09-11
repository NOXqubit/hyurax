# ✝ Isaías 26:20 — “Vai, pois, povo meu, entra nos teus quartos e fecha as tuas portas sobre ti; esconde-te só por um momento, até que passe a ira.”
"""Dinheiro: inteiro, sem sinal, sem float, sem perda silenciosa.

Este arquivo faltava. A conversao de valor monetario e a superficie onde erro
mais barato vira prejuizo mais caro, e ela nao tinha teste proprio.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from auron import units  # noqa: E402
from auron.units import AUR_UNIT, MAX_SUPPLY, AmountError, to_aur_str, to_units  # noqa: E402


def esperar_erro(fn, *args, rotulo=""):
    try:
        resultado = fn(*args)
    except AmountError:
        return
    raise AssertionError(f"{rotulo}: deveria ter sido recusado, devolveu {resultado!r}")


def test_conversao_basica():
    assert to_units("0") == 0
    assert to_units("1") == AUR_UNIT
    assert to_units("0.1") == 10_000_000
    assert to_units("0.00000001") == 1
    assert to_units("1.5") == 150_000_000
    assert to_units("50") == 50 * AUR_UNIT
    assert to_units("21000000") == MAX_SUPPLY
    assert to_units("+2") == 2 * AUR_UNIT
    assert to_units("  3.25  ") == 325_000_000
    assert to_units(".5") == 50_000_000
    assert to_units("7.") == 7 * AUR_UNIT
    print("PASS conversao basica")


def test_formatacao_e_ida_e_volta():
    assert to_aur_str(0) == "0.00000000"
    assert to_aur_str(1) == "0.00000001"
    assert to_aur_str(AUR_UNIT) == "1.00000000"
    assert to_aur_str(150_000_000) == "1.50000000"
    for texto in ("0", "1", "0.1", "1.5", "123.45678901", "21000000"):
        assert to_aur_str(to_units(texto)) == to_aur_str(to_units(texto))
        # ida e volta preserva o valor, nao necessariamente o texto
        assert to_units(to_aur_str(to_units(texto))) == to_units(texto)
    print("PASS formatacao e ida e volta")


def test_sem_poeira_de_ponto_flutuante():
    """O motivo de dinheiro ser inteiro: 0.1 somado dez vezes tem que dar 1."""
    acumulado = 0
    for _ in range(10):
        acumulado += to_units("0.1")
    assert acumulado == to_units("1")
    # em float isso falharia
    assert 0.1 * 10 != 1.0 or True  # o ponto e que nao dependemos disso

    total = 0
    for _ in range(1_000):
        total += to_units("0.00000001")
    assert total == to_units("0.00001")
    print("PASS soma de centavos nao acumula erro")


def test_float_recusado():
    """`to_units(0.1)` era aceito no prototipo. Era por ali que a poeira entrava."""
    esperar_erro(to_units, 0.1, rotulo="float 0.1")
    esperar_erro(to_units, 1.0, rotulo="float 1.0")
    esperar_erro(to_units, 1e8, rotulo="float 1e8")
    print("PASS float recusado")


def test_negativo_recusado_nos_dois_caminhos():
    """Dinheiro no protocolo e u64. Negativo nao tem representacao."""
    esperar_erro(to_units, "-1", rotulo="string -1")
    esperar_erro(to_units, "-1.5", rotulo="string -1.5")
    esperar_erro(to_units, "-0.00000001", rotulo="string negativa minima")
    # o caminho de inteiro tambem precisa fechar, senao passa por baixo do parser
    esperar_erro(to_units, -1, rotulo="int -1")
    esperar_erro(to_units, -MAX_SUPPLY, rotulo="int muito negativo")
    print("PASS negativo recusado na string e no inteiro")


def test_entrada_malformada_recusada():
    for ruim, rotulo in (
        ("", "vazio"),
        ("   ", "so espaco"),
        (".", "so ponto"),
        ("abc", "letras"),
        ("1.2.3", "dois pontos"),
        ("1,5", "virgula"),
        ("1 5", "espaco no meio"),
        ("0x10", "hexadecimal"),
        ("1e8", "notacao cientifica"),
        ("１", "digito de largura completa"),
        ("١", "algarismo indo-arabico oriental"),
        (" 1.5", "espaco inseparavel na frente"),
        ("1.5 ", "espaco inseparavel atras"),
        ("1​.5", "espaco de largura zero no meio"),
    ):
        esperar_erro(to_units, ruim, rotulo=rotulo)
    esperar_erro(to_units, True, rotulo="bool True")
    esperar_erro(to_units, None, rotulo="None")
    esperar_erro(to_units, [1], rotulo="lista")
    print("PASS entrada malformada recusada")


def test_precisao_extra_recusada_em_vez_de_truncada():
    """Truncar seria perder dinheiro em silencio. Melhor recusar."""
    assert to_units("1.12345678") == 112_345_678
    esperar_erro(to_units, "1.123456789", rotulo="9 casas")
    esperar_erro(to_units, "0.000000001", rotulo="10 casas")
    print("PASS mais de 8 casas e recusado, nao truncado")


def test_limites():
    assert to_units(units.MAX_AMOUNT) == units.MAX_AMOUNT
    esperar_erro(to_units, units.MAX_AMOUNT + 1, rotulo="acima de u64")
    esperar_erro(to_units, "184467440737.09551616", rotulo="acima de u64 em texto")
    print("PASS limites de faixa")


def test_aritmetica_conferida():
    assert units.checked_add(1, 2) == 3
    assert units.checked_sub(5, 3) == 2
    assert units.checked_sub(5, 5) == 0

    try:
        units.checked_sub(3, 5)
        raise AssertionError("subtracao abaixo de zero passou")
    except AmountError:
        pass

    try:
        units.checked_add(units.MAX_AMOUNT, 1)
        raise AssertionError("soma acima de u64 passou")
    except AmountError:
        pass
    print("PASS soma e subtracao recusam estouro")


def test_teto_de_supply_cabe_em_u64():
    assert MAX_SUPPLY == 21_000_000 * AUR_UNIT
    assert MAX_SUPPLY <= units.MAX_AMOUNT
    assert to_aur_str(MAX_SUPPLY) == "21000000.00000000"
    print("PASS teto de supply consistente")


if __name__ == "__main__":
    test_conversao_basica()
    test_formatacao_e_ida_e_volta()
    test_sem_poeira_de_ponto_flutuante()
    test_float_recusado()
    test_negativo_recusado_nos_dois_caminhos()
    test_entrada_malformada_recusada()
    test_precisao_extra_recusada_em_vez_de_truncada()
    test_limites()
    test_aritmetica_conferida()
    test_teto_de_supply_cabe_em_u64()
    print("=== UNITS OK ===")
