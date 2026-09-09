"""AURON — unidades monetárias.

Regra travada: dinheiro é SEMPRE inteiro. Nunca float, em lugar nenhum.

1 AUR = 100_000_000 unidades internas.

Esta é a única definição de dinheiro do protocolo. O `auron_core.py` e o
`auron_reference.py` do protótipo antigo tinham cópias divergentes desta
lógica; as duas foram descartadas em favor deste módulo.
"""

from __future__ import annotations

AUR_DECIMALS = 8
AUR_UNIT = 10**AUR_DECIMALS  # 100_000_000

# Teto absoluto de emissão. Nenhuma regra do protocolo pode ultrapassar isto.
MAX_SUPPLY = 21_000_000 * AUR_UNIT

# Limite de sanidade para qualquer valor serializado (cabe em u64).
MAX_AMOUNT = 2**64 - 1

if MAX_SUPPLY > MAX_AMOUNT:
    raise AssertionError("MAX_SUPPLY não cabe em u64")


class AmountError(ValueError):
    """Valor monetário inválido."""


def to_units(aur: str | int) -> int:
    """Converte '1.5' (AUR) ou um inteiro (já em unidades) para unidades internas.

    Diferente do protótipo, float é recusado explicitamente em vez de aceito
    silenciosamente. `to_units(0.1)` era aceito antes e é justamente o caminho
    pelo qual poeira de ponto flutuante entrava no dinheiro.
    """
    if isinstance(aur, bool):
        raise AmountError("bool não é valor monetário")
    if isinstance(aur, int):
        return _checked(aur)
    if isinstance(aur, float):
        raise AmountError(
            "float é proibido em valores monetários; passe string, ex: '0.1'"
        )
    if not isinstance(aur, str):
        raise AmountError(f"tipo inválido para valor: {type(aur).__name__}")

    s = aur.strip()
    if not s:
        raise AmountError("valor vazio")

    sign = 1
    if s[0] in "+-":
        if s[0] == "-":
            sign = -1
        s = s[1:]

    if "." in s:
        whole, frac = s.split(".", 1)
    else:
        whole, frac = s, ""

    if "." in frac:
        raise AmountError("mais de um separador decimal")
    if whole and not whole.isdigit():
        raise AmountError(f"parte inteira inválida: {whole!r}")
    if frac and not frac.isdigit():
        raise AmountError(f"parte fracionária inválida: {frac!r}")
    if not whole and not frac:
        raise AmountError("valor sem dígitos")
    if len(frac) > AUR_DECIMALS:
        raise AmountError(
            f"mais de {AUR_DECIMALS} casas decimais: {aur!r} (truncar seria perder dinheiro)"
        )

    frac = frac.ljust(AUR_DECIMALS, "0")
    return _checked(sign * (int(whole or "0") * AUR_UNIT + int(frac)))


def to_aur_str(units: int) -> str:
    """Formata unidades internas como string decimal com 8 casas, sem float."""
    if isinstance(units, bool) or not isinstance(units, int):
        raise AmountError("unidades devem ser int")
    sign = "-" if units < 0 else ""
    whole, frac = divmod(abs(units), AUR_UNIT)
    return f"{sign}{whole}.{frac:0{AUR_DECIMALS}d}"


def _checked(units: int) -> int:
    if abs(units) > MAX_AMOUNT:
        raise AmountError(f"valor fora da faixa representável: {units}")
    return units


def checked_add(a: int, b: int) -> int:
    """Soma que estoura em vez de dar a volta silenciosamente."""
    r = a + b
    if r < 0 or r > MAX_AMOUNT:
        raise AmountError(f"overflow na soma: {a} + {b}")
    return r


def checked_sub(a: int, b: int) -> int:
    """Subtração que recusa resultado negativo (saldo não fica devendo)."""
    r = a - b
    if r < 0:
        raise AmountError(f"underflow na subtração: {a} - {b}")
    return r
