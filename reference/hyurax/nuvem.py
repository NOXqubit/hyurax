# ✝ Atos 2:44 — “Todos os que criam estavam juntos e tinham tudo em comum.”
"""Nuvem Hyurax: mercado de máquinas, armazenamento distribuído e livro de
contas, entre os próprios nós (sem servidor central).

Especificação de referência de `crates/hyurax-nuvem` (ver docs/NUVEM.md).
Fora do consenso: nada aqui muda bloco, transação ou cadeia. Créditos de
computação NÃO são dinheiro nem HYX.

1. Mensagens de rede de tipo `TIPO_NUVEM` (0x4E56, "NV"), dentro da conexão
   cifrada (Noise XX: quem manda já provou a identidade). Corpo:
   `u8 VERSAO || u8 subtipo || campos`, big-endian, sem sobra.

     1 ANUNCIO   anúncio de máquina assinado (ver 2)
     2 GUARDAR   arquivo[32] || u8 indice || u32 tamanho || hash[64]
                 "vou mandar este fragmento; guarde-o para mim"
     3 PARTE     arquivo[32] || u8 indice || u32 deslocamento || var_bytes(dados)
                 um pedaço do fragmento (até PARTE_MAX bytes), nos dois sentidos
     4 GUARDADO  arquivo[32] || u8 indice || u8 ok || str motivo
     5 DESAFIO   arquivo[32] || u8 indice || nonce[32]
     6 PROVA     arquivo[32] || u8 indice || nonce[32] || h[64]
                 h = H(DOMINIO_PROVA || nonce || fragmento): só quem guarda o
                 fragmento inteiro responde certo
     7 BUSCAR    arquivo[32] || u8 indice
     8 ENTREGA   arquivo[32] || u8 indice || u8 tem || u32 tamanho || hash[64]
                 resposta ao BUSCAR; com tem=1, as PARTEs vêm em seguida
     9 APAGAR    arquivo[32] || u8 indice
    10 RECIBO    recibo de aluguel assinado (ver 4)

   Só o dono (a identidade Noise que mandou GUARDAR) busca, desafia ou apaga
   um fragmento.

2. Anúncio de máquina (mercado). Assinado pela chave do worker (Ed25519):

     worker[32] || identidade[32] || u8 tipo || u64 instante_ms || u32 validade_s
     || u16 linhas || u32 ram_mib || str gpu || u32 vram_mib || u64 disco_mib
     || u64 preco_credito_mili || u64 preco_gb_mes_mili || u64 preco_venda_centavos
     || str descricao || u64 creditos_hora || assinatura[64]

   assinatura = Ed25519(worker, H(DOMINIO_ANUNCIO || tudo antes dela)).
   tipo: 1 capacidade (aluga CPU/GPU/armazenamento), 2 máquina inteira,
   3 venda (só listagem; a venda acontece fora da rede). `creditos_hora` é a
   medida do benchmark do próprio dono: ESTIMADO, não conferido.

3. Armazenamento: o arquivo é cifrado (ChaCha20-Poly1305, chave aleatória por
   arquivo, que só o dono guarda) e o texto cifrado é dividido em `k`
   fragmentos de dados mais `m` de paridade (Reed–Solomon sistemático sobre
   GF(2^8), polinômio 0x11D, matriz de Cauchy). Quaisquer `k` dos `k + m`
   fragmentos reconstroem o arquivo. Quem guarda um fragmento nunca vê o
   conteúdo em claro.

     linha de paridade r (0 ≤ r < m), coluna j (0 ≤ j < k):
       C[r][j] = 1 / ((k + r) XOR j)            (em GF(2^8))
     fragmento k + r, byte p = XOR_j  C[r][j] · dado_j[p]

   Tamanho de cada fragmento: L = max(1, ceil(n / k)); o fim é completado
   com zeros. 1 ≤ k ≤ 32, 0 ≤ m ≤ 32.

4. Recibo de aluguel. O cliente assina quanto trabalho verificado o
   fornecedor fez para um JOB, e quanto isso vale no preço anunciado:

     cliente[32] || fornecedor[32] || job[64] || u64 creditos_mili
     || u64 preco_credito_mili || u64 total_mili || u64 instante_ms || assinatura[64]

   total_mili = creditos_mili · preco_credito_mili / 1000 (inteiro, para baixo).
   assinatura = Ed25519(cliente, H(DOMINIO_RECIBO || tudo antes dela)).

5. Livro de contas (local, só acréscimo, encadeado por hash):

     hash = H(DOMINIO_LIVRO || anterior[64] || u64 seq || u64 instante
              || str tipo || u32 conta || job[64] || contraparte[32] || u64 valor_mili
              || str nota)
"""

from __future__ import annotations

from . import codec, crypto, identidade

TIPO_NUVEM = 0x4E56
VERSAO = 1

(ANUNCIO, GUARDAR, PARTE, GUARDADO, DESAFIO, PROVA, BUSCAR, ENTREGA, APAGAR,
 RECIBO) = range(1, 11)

DOMINIO_ANUNCIO = identidade.rotulo("NUVEM-ANUNCIO-v1")
DOMINIO_PROVA = identidade.rotulo("NUVEM-PROVA-v1")
DOMINIO_RECIBO = identidade.rotulo("NUVEM-RECIBO-v1")
DOMINIO_LIVRO = identidade.rotulo("NUVEM-LIVRO-v1")

TIPOS_DE_ANUNCIO = (1, 2, 3)
VALIDADE_MAX_S = 86_400
GPU_MAX = 64
DESCRICAO_MAX = 280
MOTIVO_MAX = 200
PARTE_MAX = 256 * 1024
FRAGMENTO_MAX = 64 * 1024 * 1024
K_MAX = 32
M_MAX = 32

# --------------------------------------------------------------------------
# GF(2^8)
# --------------------------------------------------------------------------

_EXP = [0] * 512
_LOG = [0] * 256


def _tabelas() -> None:
    x = 1
    for i in range(255):
        _EXP[i] = x
        _LOG[x] = i
        x <<= 1
        if x & 0x100:
            x ^= 0x11D
    for i in range(255, 512):
        _EXP[i] = _EXP[i - 255]


_tabelas()


def gf_mul(a: int, b: int) -> int:
    if a == 0 or b == 0:
        return 0
    return _EXP[_LOG[a] + _LOG[b]]


def gf_inv(a: int) -> int:
    if a == 0:
        raise ZeroDivisionError("zero não tem inverso")
    return _EXP[255 - _LOG[a]]


def _conferir_km(k: int, m: int) -> None:
    if not (1 <= k <= K_MAX and 0 <= m <= M_MAX):
        raise ValueError("k ou m fora da faixa")


def cauchy(k: int, m: int) -> list[list[int]]:
    _conferir_km(k, m)
    return [[gf_inv((k + r) ^ j) for j in range(k)] for r in range(m)]


def tamanho_do_fragmento(n: int, k: int) -> int:
    return max(1, -(-n // k))


def codificar(dados: bytes, k: int, m: int) -> list[bytes]:
    """Os k + m fragmentos de `dados`."""
    _conferir_km(k, m)
    largura = tamanho_do_fragmento(len(dados), k)
    cheio = dados + bytes(largura * k - len(dados))
    blocos = [cheio[j * largura:(j + 1) * largura] for j in range(k)]
    c = cauchy(k, m)
    paridade = []
    for r in range(m):
        saida = bytearray(largura)
        for j in range(k):
            coef = c[r][j]
            bloco = blocos[j]
            for p in range(largura):
                saida[p] ^= gf_mul(coef, bloco[p])
        paridade.append(bytes(saida))
    return blocos + paridade


def _linha(indice: int, k: int, c: list[list[int]]) -> list[int]:
    if indice < k:
        return [1 if j == indice else 0 for j in range(k)]
    return list(c[indice - k])


def _inverter(a: list[list[int]]) -> list[list[int]]:
    n = len(a)
    a = [linha[:] + [1 if i == j else 0 for j in range(n)] for i, linha in enumerate(a)]
    for col in range(n):
        piv = next((r for r in range(col, n) if a[r][col]), None)
        if piv is None:
            raise ValueError("matriz singular")
        a[col], a[piv] = a[piv], a[col]
        inv = gf_inv(a[col][col])
        a[col] = [gf_mul(v, inv) for v in a[col]]
        for r in range(n):
            if r != col and a[r][col]:
                f = a[r][col]
                a[r] = [v ^ gf_mul(f, w) for v, w in zip(a[r], a[col])]
    return [linha[n:] for linha in a]


def reconstruir(fragmentos: dict[int, bytes], k: int, m: int, tamanho: int) -> bytes:
    """Os dados originais a partir de quaisquer k fragmentos (índice →
    bytes). Usa os k de menor índice."""
    _conferir_km(k, m)
    largura = tamanho_do_fragmento(tamanho, k)
    usados = sorted(i for i in fragmentos if 0 <= i < k + m)[:k]
    if len(usados) < k:
        raise ValueError("fragmentos insuficientes")
    if any(len(fragmentos[i]) != largura for i in usados):
        raise ValueError("fragmento de tamanho errado")
    c = cauchy(k, m)
    inv = _inverter([_linha(i, k, c) for i in usados])
    saida = bytearray()
    for j in range(k):
        bloco = bytearray(largura)
        for t, i in enumerate(usados):
            coef = inv[j][t]
            if coef:
                frag = fragmentos[i]
                for p in range(largura):
                    bloco[p] ^= gf_mul(coef, frag[p])
        saida += bloco
    return bytes(saida[:tamanho])


def prova(nonce: bytes, fragmento: bytes) -> bytes:
    assert len(nonce) == 32
    return crypto.H(DOMINIO_PROVA + nonce + fragmento)


# --------------------------------------------------------------------------
# Anúncio de máquina
# --------------------------------------------------------------------------

def _str_limitada(texto: str, maximo: int) -> bytes:
    b = texto.encode("utf-8")
    if len(b) > maximo:
        raise ValueError("texto longo demais")
    return codec.enc_str(texto)


def corpo_do_anuncio(a: dict) -> bytes:
    if a["tipo"] not in TIPOS_DE_ANUNCIO:
        raise ValueError("tipo de anúncio desconhecido")
    if not 1 <= a["validade_s"] <= VALIDADE_MAX_S:
        raise ValueError("validade fora da faixa")
    assert len(a["worker"]) == 32 and len(a["identidade"]) == 32
    return (
        a["worker"] + a["identidade"] + codec.enc_u8(a["tipo"]) + codec.enc_u64(a["instante_ms"])
        + codec.enc_u32(a["validade_s"]) + codec.enc_u16(a["linhas"]) + codec.enc_u32(a["ram_mib"])
        + _str_limitada(a["gpu"], GPU_MAX) + codec.enc_u32(a["vram_mib"]) + codec.enc_u64(a["disco_mib"])
        + codec.enc_u64(a["preco_credito_mili"]) + codec.enc_u64(a["preco_gb_mes_mili"])
        + codec.enc_u64(a["preco_venda_centavos"]) + _str_limitada(a["descricao"], DESCRICAO_MAX)
        + codec.enc_u64(a["creditos_hora"])
    )


def anuncio(segredo: bytes, a: dict) -> bytes:
    """O anúncio assinado (sem o cabeçalho de mensagem). `a` sem `worker`:
    ele sai do segredo."""
    a = dict(a, worker=crypto.ed25519_public_key(segredo))
    corpo = corpo_do_anuncio(a)
    return corpo + crypto.ed25519_sign(segredo, crypto.H(DOMINIO_ANUNCIO + corpo))


def ler_anuncio(r: codec.Reader) -> dict:
    a = {
        "worker": r.take(32), "identidade": r.take(32), "tipo": r.u8(), "instante_ms": r.u64(),
        "validade_s": r.u32(), "linhas": r.u16(), "ram_mib": r.u32(), "gpu": r.string(),
        "vram_mib": r.u32(), "disco_mib": r.u64(), "preco_credito_mili": r.u64(),
        "preco_gb_mes_mili": r.u64(), "preco_venda_centavos": r.u64(), "descricao": r.string(),
        "creditos_hora": r.u64(),
    }
    a["assinatura"] = r.take(64)
    if a["tipo"] not in TIPOS_DE_ANUNCIO:
        raise ValueError("tipo de anúncio desconhecido")
    if not 1 <= a["validade_s"] <= VALIDADE_MAX_S:
        raise ValueError("validade fora da faixa")
    if len(a["gpu"].encode()) > GPU_MAX or len(a["descricao"].encode()) > DESCRICAO_MAX:
        raise ValueError("texto longo demais")
    return a


def anuncio_confere(a: dict) -> bool:
    return crypto.ed25519_verify(a["worker"], crypto.H(DOMINIO_ANUNCIO + corpo_do_anuncio(a)), a["assinatura"])


# --------------------------------------------------------------------------
# Recibo de aluguel
# --------------------------------------------------------------------------

def total_do_recibo(creditos_mili: int, preco_credito_mili: int) -> int:
    return creditos_mili * preco_credito_mili // 1000


def corpo_do_recibo(rc: dict) -> bytes:
    assert len(rc["cliente"]) == 32 and len(rc["fornecedor"]) == 32 and len(rc["job"]) == 64
    if rc["total_mili"] != total_do_recibo(rc["creditos_mili"], rc["preco_credito_mili"]):
        raise ValueError("total do recibo não bate com créditos e preço")
    return (
        rc["cliente"] + rc["fornecedor"] + rc["job"] + codec.enc_u64(rc["creditos_mili"])
        + codec.enc_u64(rc["preco_credito_mili"]) + codec.enc_u64(rc["total_mili"])
        + codec.enc_u64(rc["instante_ms"])
    )


def recibo(segredo: bytes, fornecedor: bytes, job: bytes, creditos_mili: int, preco_credito_mili: int,
           instante_ms: int) -> bytes:
    rc = {
        "cliente": crypto.ed25519_public_key(segredo), "fornecedor": fornecedor, "job": job,
        "creditos_mili": creditos_mili, "preco_credito_mili": preco_credito_mili,
        "total_mili": total_do_recibo(creditos_mili, preco_credito_mili), "instante_ms": instante_ms,
    }
    corpo = corpo_do_recibo(rc)
    return corpo + crypto.ed25519_sign(segredo, crypto.H(DOMINIO_RECIBO + corpo))


def ler_recibo(r: codec.Reader) -> dict:
    rc = {
        "cliente": r.take(32), "fornecedor": r.take(32), "job": r.take(64), "creditos_mili": r.u64(),
        "preco_credito_mili": r.u64(), "total_mili": r.u64(), "instante_ms": r.u64(),
    }
    rc["assinatura"] = r.take(64)
    if rc["total_mili"] != total_do_recibo(rc["creditos_mili"], rc["preco_credito_mili"]):
        raise ValueError("total do recibo não bate com créditos e preço")
    return rc


def recibo_confere(rc: dict) -> bool:
    return crypto.ed25519_verify(rc["cliente"], crypto.H(DOMINIO_RECIBO + corpo_do_recibo(rc)), rc["assinatura"])


# --------------------------------------------------------------------------
# Mensagens
# --------------------------------------------------------------------------

def _cab(subtipo: int) -> bytes:
    return codec.enc_u8(VERSAO) + codec.enc_u8(subtipo)


def _alvo(arquivo: bytes, indice: int) -> bytes:
    assert len(arquivo) == 32
    if not 0 <= indice < K_MAX + M_MAX:
        raise ValueError("índice de fragmento fora da faixa")
    return arquivo + codec.enc_u8(indice)


def msg_anuncio(anuncio_assinado: bytes) -> bytes:
    return _cab(ANUNCIO) + anuncio_assinado


def guardar(arquivo: bytes, indice: int, tamanho: int, hash_: bytes) -> bytes:
    if not 1 <= tamanho <= FRAGMENTO_MAX:
        raise ValueError("fragmento grande demais")
    return _cab(GUARDAR) + _alvo(arquivo, indice) + codec.enc_u32(tamanho) + hash_


def parte(arquivo: bytes, indice: int, deslocamento: int, dados: bytes) -> bytes:
    if not 1 <= len(dados) <= PARTE_MAX:
        raise ValueError("parte de tamanho inválido")
    return _cab(PARTE) + _alvo(arquivo, indice) + codec.enc_u32(deslocamento) + codec.enc_bytes(dados)


def guardado(arquivo: bytes, indice: int, ok: bool, motivo: str) -> bytes:
    return _cab(GUARDADO) + _alvo(arquivo, indice) + codec.enc_u8(1 if ok else 0) + _str_limitada(motivo, MOTIVO_MAX)


def desafio(arquivo: bytes, indice: int, nonce: bytes) -> bytes:
    assert len(nonce) == 32
    return _cab(DESAFIO) + _alvo(arquivo, indice) + nonce


def msg_prova(arquivo: bytes, indice: int, nonce: bytes, h: bytes) -> bytes:
    assert len(nonce) == 32 and len(h) == 64
    return _cab(PROVA) + _alvo(arquivo, indice) + nonce + h


def buscar(arquivo: bytes, indice: int) -> bytes:
    return _cab(BUSCAR) + _alvo(arquivo, indice)


def entrega(arquivo: bytes, indice: int, tem: bool, tamanho: int, hash_: bytes) -> bytes:
    if tamanho > FRAGMENTO_MAX:
        raise ValueError("fragmento grande demais")
    return _cab(ENTREGA) + _alvo(arquivo, indice) + codec.enc_u8(1 if tem else 0) + codec.enc_u32(tamanho) + hash_


def apagar(arquivo: bytes, indice: int) -> bytes:
    return _cab(APAGAR) + _alvo(arquivo, indice)


def msg_recibo(recibo_assinado: bytes) -> bytes:
    return _cab(RECIBO) + recibo_assinado


def _bool(r: codec.Reader) -> bool:
    v = r.u8()
    if v not in (0, 1):
        raise ValueError("marca inválida")
    return v == 1


def _ler_alvo(r: codec.Reader) -> tuple[bytes, int]:
    arquivo = r.take(32)
    indice = r.u8()
    if indice >= K_MAX + M_MAX:
        raise ValueError("índice de fragmento fora da faixa")
    return arquivo, indice


def ler(dados: bytes) -> dict:
    """Decodifica uma mensagem da nuvem; levanta ValueError (ou
    codec.CodecError) se não for exatamente uma mensagem válida."""
    r = codec.Reader(dados)
    if r.u8() != VERSAO:
        raise ValueError("versão de mensagem da nuvem desconhecida")
    s = r.u8()
    if s == ANUNCIO:
        m = {"subtipo": "anuncio", "anuncio": ler_anuncio(r)}
    elif s == GUARDAR:
        arquivo, indice = _ler_alvo(r)
        tamanho = r.u32()
        if not 1 <= tamanho <= FRAGMENTO_MAX:
            raise ValueError("fragmento grande demais")
        m = {"subtipo": "guardar", "arquivo": arquivo, "indice": indice, "tamanho": tamanho, "hash": r.take(64)}
    elif s == PARTE:
        arquivo, indice = _ler_alvo(r)
        deslocamento = r.u32()
        corpo = r.var_bytes()
        if not 1 <= len(corpo) <= PARTE_MAX:
            raise ValueError("parte de tamanho inválido")
        m = {"subtipo": "parte", "arquivo": arquivo, "indice": indice, "deslocamento": deslocamento, "dados": corpo}
    elif s == GUARDADO:
        arquivo, indice = _ler_alvo(r)
        ok = _bool(r)
        motivo = r.string()
        if len(motivo.encode()) > MOTIVO_MAX:
            raise ValueError("texto longo demais")
        m = {"subtipo": "guardado", "arquivo": arquivo, "indice": indice, "ok": ok, "motivo": motivo}
    elif s == DESAFIO:
        arquivo, indice = _ler_alvo(r)
        m = {"subtipo": "desafio", "arquivo": arquivo, "indice": indice, "nonce": r.take(32)}
    elif s == PROVA:
        arquivo, indice = _ler_alvo(r)
        m = {"subtipo": "prova", "arquivo": arquivo, "indice": indice, "nonce": r.take(32), "h": r.take(64)}
    elif s == BUSCAR:
        arquivo, indice = _ler_alvo(r)
        m = {"subtipo": "buscar", "arquivo": arquivo, "indice": indice}
    elif s == ENTREGA:
        arquivo, indice = _ler_alvo(r)
        tem = _bool(r)
        tamanho = r.u32()
        if tamanho > FRAGMENTO_MAX:
            raise ValueError("fragmento grande demais")
        m = {"subtipo": "entrega", "arquivo": arquivo, "indice": indice, "tem": tem, "tamanho": tamanho, "hash": r.take(64)}
    elif s == APAGAR:
        arquivo, indice = _ler_alvo(r)
        m = {"subtipo": "apagar", "arquivo": arquivo, "indice": indice}
    elif s == RECIBO:
        m = {"subtipo": "recibo", "recibo": ler_recibo(r)}
    else:
        raise ValueError("subtipo de mensagem da nuvem desconhecido")
    r.finish()
    return m


# --------------------------------------------------------------------------
# Livro de contas
# --------------------------------------------------------------------------

TIPOS_DO_LIVRO = ("consumo", "provedor", "plataforma", "reserva", "credito", "recibo")
NOTA_MAX = 200


def hash_da_linha(anterior: bytes, seq: int, instante: int, tipo: str, conta: int, job: bytes,
                  contraparte: bytes, valor_mili: int, nota: str) -> bytes:
    assert len(anterior) == 64 and len(job) == 64 and len(contraparte) == 32
    if tipo not in TIPOS_DO_LIVRO:
        raise ValueError("tipo de lançamento desconhecido")
    return crypto.H(
        DOMINIO_LIVRO + anterior + codec.enc_u64(seq) + codec.enc_u64(instante) + codec.enc_str(tipo)
        + codec.enc_u32(conta) + job + contraparte + codec.enc_u64(valor_mili) + _str_limitada(nota, NOTA_MAX)
    )


def repartir(total_mili: int, provedor_pct: int, plataforma_pct: int) -> tuple[int, int, int]:
    """Parte de um consumo: (provedor, plataforma, reserva). A reserva fica
    com o resto, então as três somam sempre o total."""
    if provedor_pct + plataforma_pct > 100 or provedor_pct < 0 or plataforma_pct < 0:
        raise ValueError("percentuais inválidos")
    provedor = total_mili * provedor_pct // 100
    plataforma = total_mili * plataforma_pct // 100
    return provedor, plataforma, total_mili - provedor - plataforma
