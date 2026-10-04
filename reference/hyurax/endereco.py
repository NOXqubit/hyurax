# ✝ Provérbios 22:28 — “Não removas os limites antigos que fizeram teus pais.”
"""Endereço como as pessoas veem: Bech32m (BIP-350), com dígito verificador.

Especificação de referência de `crates/hyurax-nucleo/src/carteira/endereco.rs`
e de `mobile/src/nucleo/endereco.ts`. Fora do consenso: por dentro o endereço
continua sendo os 20 bytes da seção 4 da especificação.

- O texto é `prefixo || "1" || dados(5 bits) || verificador(6)`, sobre os 20
  bytes do endereço, sem byte de versão.
- O prefixo diz a rede: `hyx` (principal), `thyx` (teste), `rhyx` (local).
  Um endereço de uma rede é recusado em outra.
- Maiúsculas valem, desde que o texto inteiro esteja numa caixa só.
- O hexadecimal de 40 dígitos também é aceito na entrada (sem verificador).
"""

ALFABETO = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
BECH32M = 0x2BC830A3
GERADOR = (0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233DD, 0x2A1462B3)
TAMANHO = 20

PREFIXOS = {"mainnet": "hyx", "testnet": "thyx", "regtest": "rhyx"}


def prefixo(nome_da_rede: str) -> str:
    nome = nome_da_rede.removeprefix("hyurax-")
    return PREFIXOS.get(nome, "rhyx")


def _polimodo(valores: list[int]) -> int:
    c = 1
    for v in valores:
        topo = c >> 25
        c = ((c & 0x1FFFFFF) << 5) ^ v
        for i, g in enumerate(GERADOR):
            if (topo >> i) & 1:
                c ^= g
    return c


def _expandir(hrp: str) -> list[int]:
    return [ord(x) >> 5 for x in hrp] + [0] + [ord(x) & 31 for x in hrp]


def _reagrupar(dados: list[int], de: int, para: int, completar: bool) -> list[int]:
    acumulado, bits, saida = 0, 0, []
    maximo = (1 << para) - 1
    for v in dados:
        if v >> de:
            raise ValueError("valor fora da faixa")
        acumulado = (acumulado << de) | v
        bits += de
        while bits >= para:
            bits -= para
            saida.append((acumulado >> bits) & maximo)
    if completar:
        if bits:
            saida.append((acumulado << (para - bits)) & maximo)
    elif bits >= de or (acumulado << (para - bits)) & maximo:
        raise ValueError("endereço com bits que sobram")
    return saida


def codificar(hrp: str, carga: bytes) -> str:
    dados = _reagrupar(list(carga), 8, 5, True)
    v = _polimodo(_expandir(hrp) + dados + [0] * 6) ^ BECH32M
    verificador = [(v >> (5 * (5 - i))) & 31 for i in range(6)]
    return hrp + "1" + "".join(ALFABETO[d] for d in dados + verificador)


def ler_bech32m(texto: str) -> tuple[str, list[int]]:
    t = texto.strip()
    if len(t) > 90 or len(t) < 8:
        raise ValueError("endereço com tamanho errado")
    if any(c.islower() for c in t) and any(c.isupper() for c in t):
        raise ValueError("endereço mistura maiúsculas e minúsculas")
    t = t.lower()
    sep = t.rfind("1")
    if sep < 0:
        raise ValueError("endereço sem o separador 1")
    hrp, resto = t[:sep], t[sep + 1:]
    if not hrp or len(resto) < 6:
        raise ValueError("endereço incompleto")
    dados = []
    for c in resto:
        if c not in ALFABETO:
            raise ValueError(f"caractere que não existe em endereço: {c}")
        dados.append(ALFABETO.index(c))
    if _polimodo(_expandir(hrp) + dados) != BECH32M:
        raise ValueError("o dígito verificador não confere: tem erro de digitação no endereço")
    return hrp, dados[:-6]


def mostrar(endereco: bytes, nome_da_rede: str) -> str:
    if len(endereco) != TAMANHO:
        raise ValueError("endereço com tamanho errado")
    return codificar(prefixo(nome_da_rede), endereco)


def ler(texto: str, nome_da_rede: str) -> bytes:
    t = texto.strip()
    if len(t) == 2 * TAMANHO and all(c in "0123456789abcdefABCDEF" for c in t):
        return bytes.fromhex(t)
    hrp, dados = ler_bech32m(t)
    esperado = prefixo(nome_da_rede)
    if hrp != esperado:
        de_qual = {"hyx": "da rede principal", "thyx": "da rede de teste", "rhyx": "da rede local"}.get(hrp, "de outra coisa, não do Hyurax")
        raise ValueError(f"este endereço é {de_qual}; aqui a rede espera um que comece com {esperado}1")
    b = bytes(_reagrupar(dados, 5, 8, False))
    if len(b) != TAMANHO:
        raise ValueError("endereço com tamanho errado")
    return b
