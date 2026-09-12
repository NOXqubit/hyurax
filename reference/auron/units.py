# ✝ Gênesis 41:35-36 — “Ajuntem toda a comida destes bons anos que vêm; e esta comida será para provimento da terra, para os sete anos de fome.”
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

# Tamanho máximo do texto aceito por `to_units`.
#
# O maior valor representável tem 20 dígitos inteiros e 8 decimais. 64
# caracteres dão folga de sobra para sinal, ponto e zeros à esquerda.
#
# Sem este limite, uma string com milhões de dígitos fazia o `int()` do Python
# levantar ValueError (acima de 4300 dígitos ele recusa a conversão), que não é
# AmountError e escapava de quem só tratava erro de valor; e, no caminho sem
# esse limite, gastava CPU à toa. O Rust recusa por não caber em u64. Cortar
# cedo, pelo tamanho, faz os dois recusarem a mesma coisa pelo mesmo motivo.
MAX_AMOUNT_TEXT = 64

if MAX_SUPPLY > MAX_AMOUNT:
    raise AssertionError("MAX_SUPPLY não cabe em u64")


class AmountError(ValueError):
    """Valor monetário inválido."""


_DIGITOS_ASCII = frozenset("0123456789")


def _apenas_digitos_ascii(texto: str) -> bool:
    """Só 0-9 de verdade. Nada de dígito Unicode exótico.

    `str.isdigit()` do Python devolve True para dígito de largura completa
    ('１'), algarismo indo-arábico oriental ('١') e vários outros, e `int()`
    converte todos eles. O Rust não faz isso: `str::parse::<u64>()` só aceita
    ASCII.

    Sem esta checagem, '１' viraria 1 AUR no Python e erro no Rust. Duas
    implementações discordando sobre o que é um valor válido é exatamente o
    tipo de divergência que racha uma rede. Também fecha uma porta de
    falsificação visual: '１.5' e '1.5' são indistinguíveis na tela.
    """
    return all(c in _DIGITOS_ASCII for c in texto)


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

    # Toda a entrada precisa ser ASCII, e só espaço ASCII é aparado.
    #
    # `str.strip()` do Python remove espaço em branco Unicode, incluindo o
    # espaço inseparável U+00A0. O `trim()` do Rust também, mas as duas listas
    # não são idênticas em toda versão. Em vez de tentar casar duas tabelas
    # Unicode, o protocolo simplesmente não aceita nada fora do ASCII.
    if not aur.isascii():
        raise AmountError(
            f"valor deve conter apenas caracteres ASCII: {aur!r}"
        )
    if len(aur) > MAX_AMOUNT_TEXT:
        raise AmountError(
            f"texto de valor tem {len(aur)} caracteres, máximo é {MAX_AMOUNT_TEXT}"
        )
    s = aur.strip(" \t\n\r\f\v")
    if not s:
        raise AmountError("valor vazio")

    # Dinheiro no protocolo é SEMPRE sem sinal: saldo, valor e taxa são todos
    # u64 na codificação. Aceitar negativo aqui criaria um valor que a
    # especificação não sabe serializar, e que o Rust não teria como
    # reproduzir. Rejeitar é o que mantém as duas implementações iguais.
    if s[0] == "-":
        raise AmountError(
            f"valor monetário não pode ser negativo: {aur!r} "
            "(saldo, valor e taxa são sem sinal no protocolo)"
        )
    if s[0] == "+":
        s = s[1:]

    if "." in s:
        whole, frac = s.split(".", 1)
    else:
        whole, frac = s, ""

    if "." in frac:
        raise AmountError("mais de um separador decimal")
    if whole and not _apenas_digitos_ascii(whole):
        raise AmountError(f"parte inteira inválida: {whole!r}")
    if frac and not _apenas_digitos_ascii(frac):
        raise AmountError(f"parte fracionária inválida: {frac!r}")
    if not whole and not frac:
        raise AmountError("valor sem dígitos")
    if len(frac) > AUR_DECIMALS:
        raise AmountError(
            f"mais de {AUR_DECIMALS} casas decimais: {aur!r} (truncar seria perder dinheiro)"
        )

    frac = frac.ljust(AUR_DECIMALS, "0")
    return _checked(int(whole or "0") * AUR_UNIT + int(frac))


def to_aur_str(units: int) -> str:
    """Formata unidades internas como string decimal com 8 casas, sem float.

    Aceita inteiro negativo de propósito, porque isto é função de exibição e
    às vezes é preciso mostrar uma diferença entre dois saldos. Isso NÃO
    significa que existe valor monetário negativo no protocolo: `to_units`
    recusa negativo, e todo campo serializado é u64.
    """
    if isinstance(units, bool) or not isinstance(units, int):
        raise AmountError("unidades devem ser int")
    sign = "-" if units < 0 else ""
    whole, frac = divmod(abs(units), AUR_UNIT)
    return f"{sign}{whole}.{frac:0{AUR_DECIMALS}d}"


def _checked(units: int) -> int:
    """Porta única de saída de `to_units`. Fecha o caminho de string e o de int.

    Sem a checagem de negativo aqui, `to_units(-5)` passaria direto pelo ramo
    de inteiro, sem nunca tocar no parser de texto.
    """
    if units < 0:
        raise AmountError(
            f"valor monetário não pode ser negativo: {units} "
            "(saldo, valor e taxa são sem sinal no protocolo)"
        )
    if units > MAX_AMOUNT:
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
