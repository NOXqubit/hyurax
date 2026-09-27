# ✝ Eclesiastes 4:9 — “Melhor é serem dois do que um, porque têm melhor paga do seu trabalho.”
"""Mensagens do ULTRAX entre nós (Etapa D de docs/COMPUTACAO-CIENTIFICA.md).

Especificação de referência de `crates/hyurax-ultrax/src/rede.rs`. Fora do
consenso: nenhuma destas mensagens muda bloco, transação ou cadeia.

Transporte: o corpo de uma mensagem de rede de tipo `TIPO_ULTRAX` (0x5558,
"UX"). Nós que não conhecem o tipo decodificam como "desconhecida" e
ignoram; nada quebra para quem ainda não tem o ULTRAX de rede.

Corpo: `u8 VERSAO || u8 subtipo || campos`, big-endian, sem sobra:

  1 OFERTA       worker[32] || u16 tipos || u8 linhas || u32 memória MiB ||
                 u64 instante ms || assinatura[64]
                 A assinatura é do worker (a chave do registro de prova), sobre
                 H(DOMINIO_OFERTA || worker || tipos || linhas || memória ||
                 instante): uma oferta não se forja em nome de outro worker.
                 `tipos` tem o bit (código − 1) de cada tipo de trabalho aceito.
  2 PEDIDO       u64 pedido || job[64] || u64 índice || especificação ||
                 semente[64] || u64 prazo ms || u8 com_compromisso
  3 RECUSA       u64 pedido || string motivo (até 500 bytes)
  4 COMPROMISSO  u64 pedido || worker[32] || compromisso[64]
                 compromisso = H(DOMINIO_COMPROMISSO || RESULT_HASH || worker),
                 o mesmo de `validador.compromisso`: quem copia o resultado de
                 outro não sabe o compromisso antes da revelação.
  5 REVELAR      u64 pedido
  6 RESULTADO    u64 pedido || var_bytes registro de prova ||
                 assinatura[64] || var_bytes resultado (até RESULTADO_MAX)
  7 CANCELAR     u64 pedido

`pedido` é um número de quem pede, único enquanto a unidade está em voo; o
worker só ecoa. Quem recebe confere tudo de novo (a especificação é
decodificada e validada, os tamanhos têm teto) e nunca executa código que
veio de fora: só os motores compilados no programa.
"""

from __future__ import annotations

from . import codec, crypto, identidade, job

TIPO_ULTRAX = 0x5558
VERSAO = 1
DOMINIO_OFERTA = identidade.rotulo("UTRAX-OFERTA-v1")
DOMINIO_COMPROMISSO = identidade.rotulo("UTRAX-COMPROMISSO-v1")
RESULTADO_MAX = 1 << 20
MOTIVO_MAX = 500
REGISTRO_MAX = 4096

OFERTA, PEDIDO, RECUSA, COMPROMISSO, REVELAR, RESULTADO, CANCELAR = range(1, 8)


def mensagem_da_oferta(worker: bytes, tipos: int, linhas: int, memoria_mib: int, instante_ms: int) -> bytes:
    return crypto.H(
        DOMINIO_OFERTA + worker + codec.enc_u16(tipos) + codec.enc_u8(linhas)
        + codec.enc_u32(memoria_mib) + codec.enc_u64(instante_ms)
    )


def oferta(segredo: bytes, tipos: int, linhas: int, memoria_mib: int, instante_ms: int) -> bytes:
    worker = crypto.ed25519_public_key(segredo)
    assinatura = crypto.ed25519_sign(segredo, mensagem_da_oferta(worker, tipos, linhas, memoria_mib, instante_ms))
    return (
        codec.enc_u8(VERSAO) + codec.enc_u8(OFERTA) + worker + codec.enc_u16(tipos) + codec.enc_u8(linhas)
        + codec.enc_u32(memoria_mib) + codec.enc_u64(instante_ms) + assinatura
    )


def pedido(numero: int, job_id: bytes, indice: int, especificacao: bytes, semente: bytes, prazo_ms: int,
           com_compromisso: bool) -> bytes:
    return (
        codec.enc_u8(VERSAO) + codec.enc_u8(PEDIDO) + codec.enc_u64(numero) + job_id + codec.enc_u64(indice)
        + especificacao + semente + codec.enc_u64(prazo_ms) + codec.enc_u8(1 if com_compromisso else 0)
    )


def recusa(numero: int, motivo: str) -> bytes:
    texto = motivo.encode("utf-8")[:MOTIVO_MAX].decode("utf-8", "ignore")
    return codec.enc_u8(VERSAO) + codec.enc_u8(RECUSA) + codec.enc_u64(numero) + codec.enc_str(texto)


def compromisso_de(resultado_hash: bytes, worker: bytes) -> bytes:
    return crypto.H(DOMINIO_COMPROMISSO + resultado_hash + worker)


def compromisso(numero: int, worker: bytes, valor: bytes) -> bytes:
    return codec.enc_u8(VERSAO) + codec.enc_u8(COMPROMISSO) + codec.enc_u64(numero) + worker + valor


def revelar(numero: int) -> bytes:
    return codec.enc_u8(VERSAO) + codec.enc_u8(REVELAR) + codec.enc_u64(numero)


def resultado(numero: int, registro: bytes, assinatura: bytes, dados: bytes) -> bytes:
    if len(dados) > RESULTADO_MAX or len(registro) > REGISTRO_MAX:
        raise ValueError("resultado ou registro grande demais")
    return (
        codec.enc_u8(VERSAO) + codec.enc_u8(RESULTADO) + codec.enc_u64(numero) + codec.enc_bytes(registro)
        + assinatura + codec.enc_bytes(dados)
    )


def cancelar(numero: int) -> bytes:
    return codec.enc_u8(VERSAO) + codec.enc_u8(CANCELAR) + codec.enc_u64(numero)


def ler(dados: bytes) -> dict:
    """Decodifica uma mensagem; levanta ValueError (ou codec.CodecError) se
    não for exatamente uma mensagem válida."""
    r = codec.Reader(dados)
    if r.u8() != VERSAO:
        raise ValueError("versão desconhecida")
    subtipo = r.u8()
    if subtipo == OFERTA:
        m = {"subtipo": "oferta", "worker": r.fixed(32), "tipos": r.u16(), "linhas": r.u8(), "memoria_mib": r.u32(),
             "instante_ms": r.u64(), "assinatura": r.fixed(64)}
        msg = mensagem_da_oferta(m["worker"], m["tipos"], m["linhas"], m["memoria_mib"], m["instante_ms"])
        m["assinatura_confere"] = crypto.ed25519_verify(m["worker"], msg, m["assinatura"])
    elif subtipo == PEDIDO:
        m = {"subtipo": "pedido", "pedido": r.u64(), "job": r.fixed(64), "indice": r.u64()}
        tipo, tamanho, passos = r.u8(), r.u32(), r.u32()
        parametros = []
        if tipo >= 5:
            n = r.u8()
            if n > 12:
                raise ValueError("parâmetros demais")
            parametros = [r.u32() for _ in range(n)]
        m.update({"tipo": tipo, "tamanho": tamanho, "passos": passos, "parametros": parametros,
                  "semente": r.fixed(64), "prazo_ms": r.u64()})
        marca = r.u8()
        if marca not in (0, 1):
            raise ValueError("marca de compromisso inválida")
        m["com_compromisso"] = marca == 1
    elif subtipo == RECUSA:
        m = {"subtipo": "recusa", "pedido": r.u64(), "motivo": r.string()}
        if len(m["motivo"].encode("utf-8")) > MOTIVO_MAX:
            raise ValueError("motivo longo demais")
    elif subtipo == COMPROMISSO:
        m = {"subtipo": "compromisso", "pedido": r.u64(), "worker": r.fixed(32), "compromisso": r.fixed(64)}
    elif subtipo == REVELAR:
        m = {"subtipo": "revelar", "pedido": r.u64()}
    elif subtipo == RESULTADO:
        m = {"subtipo": "resultado", "pedido": r.u64(), "registro": r.var_bytes(), "assinatura": r.fixed(64),
             "resultado": r.var_bytes()}
        if len(m["registro"]) > REGISTRO_MAX or len(m["resultado"]) > RESULTADO_MAX:
            raise ValueError("resultado ou registro grande demais")
    elif subtipo == CANCELAR:
        m = {"subtipo": "cancelar", "pedido": r.u64()}
    else:
        raise ValueError("subtipo desconhecido")
    r.finish()
    return m


def especificacao(tipo: int, tamanho: int, passos: int, parametros) -> bytes:
    return job.enc_especificacao(tipo, tamanho, passos, list(parametros))
