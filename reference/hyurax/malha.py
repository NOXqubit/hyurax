# ✝ Eclesiastes 4:12 — “O cordão de três dobras não se quebra tão depressa.”
"""Malha: os nós se encontram e se alcançam sem servidor central.

Especificação de referência de `crates/hyurax-net/src/malha.rs` (ver
docs/HYURAX-MALHA.md). Fora do consenso: nada aqui muda bloco, transação ou
cadeia. Três peças:

1. Mensagens de rede de tipo `TIPO_MALHA` (0x4D41, "MA"), dentro da conexão
   cifrada. Corpo: `u8 VERSAO || u8 subtipo || campos`, big-endian, sem sobra.

     1 PEDIR_ALCANCE  u16 porta || token[16]
                      "tente me alcançar no IP que você vê de mim, nesta porta,
                      e me mande este token por lá"
     2 ALCANCE        endereço || u8 tentou
                      o IP e a porta que quem conferiu viu, e se conseguiu
                      abrir a conexão
     3 RESERVAR       token[16]
                      "guarde uma vaga de ponte para mim; vou abrir um socket
                      com este token"
     4 RESERVA        token[16] || u8 aceita
     5 PONTES         u8 n (até 32) || n × (alvo[32] || endereço)
                      "o nó `alvo` é alcançável pela ponte `endereço`"

   endereço = u8 família (4 ou 6) || ip (4 ou 16 bytes) || u16 porta

2. Prefixos de uma conexão TCP nova, ANTES da cifra. O primeiro quadro do
   Noise começa com o tamanho (0x00 0x20); estes começam com "HX", então o
   nó distingue os dois sem ambiguidade.

     "HXV1" || token[16]   verificação de alcance (quem confere -> quem pediu)
     "HXR1" || token[16]   socket de reserva (nó atrás de NAT -> ponte)
     "HXC1" || alvo[32]    pedido de circuito (quem disca -> ponte)

   A ponte responde ao "HXC1" com um byte: 1 (circuito aberto) ou 0 (sem
   reserva para esse alvo). No socket de reserva, a ponte manda 1 byte (1)
   quando um circuito começa. Daí em diante a ponte só copia bytes: os dois
   nós fazem o aperto Noise XX de ponta a ponta, e a ponte não lê nem altera.

3. Anúncio na rede local, por UDP (difusão na porta `PORTA_VIZINHOS`):

     "HXD1" || magic[4] || u16 porta de escuta || identidade[32]

   Quem ouve usa o IP de origem do datagrama (nunca um IP escrito dentro
   dele) com a porta anunciada, e só de endereço de rede local.

4. Pacote do Éter (um arquivo que atravessa pendrive, Bluetooth, som):

     "HXP1" || magic[4] || u32 n (até 4096) || n × var_bytes(quadro)

   Cada quadro é um quadro de rede inteiro (seção 21: transação ou bloco),
   conferido como se tivesse chegado por um par. O pacote não confia em
   nada: um quadro inválido derruba só ele.
"""

from __future__ import annotations

from . import codec

TIPO_MALHA = 0x4D41
VERSAO = 1
PORTA_VIZINHOS = 8792
MAX_PONTES = 32

PEDIR_ALCANCE, ALCANCE, RESERVAR, RESERVA, PONTES = range(1, 6)

PREFIXO_VERIFICAR = b"HXV1"
PREFIXO_RESERVA = b"HXR1"
PREFIXO_CIRCUITO = b"HXC1"
PREFIXO_VIZINHO = b"HXD1"
PREFIXO_PACOTE = b"HXP1"
MAX_QUADROS_NO_PACOTE = 4096


def enc_endereco(familia: int, ip: bytes, porta: int) -> bytes:
    if (familia, len(ip)) not in ((4, 4), (6, 16)):
        raise ValueError("endereço com família ou tamanho errados")
    return codec.enc_u8(familia) + ip + codec.enc_u16(porta)


def ler_endereco(r: codec.Reader) -> tuple[int, bytes, int]:
    familia = r.u8()
    if familia == 4:
        ip = r.take(4)
    elif familia == 6:
        ip = r.take(16)
    else:
        raise ValueError("família de endereço desconhecida")
    return familia, ip, r.u16()


def _cab(subtipo: int) -> bytes:
    return codec.enc_u8(VERSAO) + codec.enc_u8(subtipo)


def pedir_alcance(porta: int, token: bytes) -> bytes:
    assert len(token) == 16
    return _cab(PEDIR_ALCANCE) + codec.enc_u16(porta) + token


def alcance(familia: int, ip: bytes, porta: int, tentou: bool) -> bytes:
    return _cab(ALCANCE) + enc_endereco(familia, ip, porta) + codec.enc_u8(1 if tentou else 0)


def reservar(token: bytes) -> bytes:
    assert len(token) == 16
    return _cab(RESERVAR) + token


def reserva(token: bytes, aceita: bool) -> bytes:
    assert len(token) == 16
    return _cab(RESERVA) + token + codec.enc_u8(1 if aceita else 0)


def pontes(lista: list[tuple[bytes, int, bytes, int]]) -> bytes:
    if len(lista) > MAX_PONTES:
        raise ValueError("pontes demais")
    corpo = _cab(PONTES) + codec.enc_u8(len(lista))
    for alvo, familia, ip, porta in lista:
        assert len(alvo) == 32
        corpo += alvo + enc_endereco(familia, ip, porta)
    return corpo


def _bool(r: codec.Reader) -> bool:
    v = r.u8()
    if v not in (0, 1):
        raise ValueError("marca inválida")
    return v == 1


def ler(dados: bytes) -> dict:
    """Decodifica uma mensagem da malha; levanta ValueError (ou
    codec.CodecError) se não for exatamente uma mensagem válida."""
    r = codec.Reader(dados)
    if r.u8() != VERSAO:
        raise ValueError("versão de mensagem da malha desconhecida")
    subtipo = r.u8()
    if subtipo == PEDIR_ALCANCE:
        m = {"subtipo": "pedir_alcance", "porta": r.u16(), "token": r.take(16)}
    elif subtipo == ALCANCE:
        familia, ip, porta = ler_endereco(r)
        m = {"subtipo": "alcance", "familia": familia, "ip": ip, "porta": porta, "tentou": _bool(r)}
    elif subtipo == RESERVAR:
        m = {"subtipo": "reservar", "token": r.take(16)}
    elif subtipo == RESERVA:
        m = {"subtipo": "reserva", "token": r.take(16), "aceita": _bool(r)}
    elif subtipo == PONTES:
        n = r.u8()
        if n > MAX_PONTES:
            raise ValueError("pontes demais")
        lista = []
        for _ in range(n):
            alvo = r.take(32)
            lista.append((alvo, *ler_endereco(r)))
        m = {"subtipo": "pontes", "lista": lista}
    else:
        raise ValueError("subtipo de mensagem da malha desconhecido")
    r.finish()
    return m


def anuncio_vizinho(magic: bytes, porta: int, identidade: bytes) -> bytes:
    assert len(magic) == 4 and len(identidade) == 32
    return PREFIXO_VIZINHO + magic + codec.enc_u16(porta) + identidade


def ler_anuncio_vizinho(dados: bytes) -> dict:
    r = codec.Reader(dados)
    if r.take(4) != PREFIXO_VIZINHO:
        raise ValueError("não é anúncio de vizinho")
    m = {"magic": r.take(4), "porta": r.u16(), "identidade": r.take(32)}
    r.finish()
    if m["porta"] == 0:
        raise ValueError("porta zero")
    return m


def prefixo(dados: bytes) -> dict:
    """Lê o prefixo de uma conexão nova (os primeiros bytes, antes da cifra)."""
    cab = dados[:4]
    resto = dados[4:]
    if cab == PREFIXO_VERIFICAR and len(resto) == 16:
        return {"tipo": "verificar", "token": resto}
    if cab == PREFIXO_RESERVA and len(resto) == 16:
        return {"tipo": "reserva", "token": resto}
    if cab == PREFIXO_CIRCUITO and len(resto) == 32:
        return {"tipo": "circuito", "alvo": resto}
    raise ValueError("prefixo desconhecido")


def pacote(magic: bytes, quadros: list[bytes]) -> bytes:
    assert len(magic) == 4
    if len(quadros) > MAX_QUADROS_NO_PACOTE:
        raise ValueError("quadros demais no pacote")
    return PREFIXO_PACOTE + magic + codec.enc_u32(len(quadros)) + b"".join(codec.enc_bytes(q) for q in quadros)


def ler_pacote(dados: bytes) -> dict:
    r = codec.Reader(dados)
    if r.take(4) != PREFIXO_PACOTE:
        raise ValueError("não é pacote do Éter")
    magic = r.take(4)
    n = r.u32()
    if n > MAX_QUADROS_NO_PACOTE:
        raise ValueError("quadros demais no pacote")
    quadros = [r.var_bytes() for _ in range(n)]
    r.finish()
    return {"magic": magic, "quadros": quadros}
