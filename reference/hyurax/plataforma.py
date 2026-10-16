# ✝ Lucas 16:10 — “Quem é fiel no mínimo, também é fiel no muito.”
"""HYURAX Compute (V1): as regras da plataforma que mexem com dinheiro e
assinatura. Especificação de referência de `crates/hyurax-plataforma`
(ver docs/V1-PLATAFORMA.md).

Fora do consenso: nada aqui muda bloco, transação ou cadeia. A V1 cobra em
moeda corrente (BRL ou USD), por saldo pré-pago; não há token, HYX nem
blockchain no caminho do dinheiro.

1. O pedido do agente. O programa do fornecedor (o agente) fala com a
   plataforma por HTTPS e assina cada pedido com a chave do worker (Ed25519,
   a mesma que assina o registro de prova do ULTRAX):

     m = H(DOMINIO_PEDIDO || str metodo || str caminho || u64 tempo_ms || H(corpo))
     assinatura = Ed25519(worker, m)

   Vale quando o método é GET ou POST, o caminho começa por /api/v1/agente/
   (ASCII visível, até 512 bytes), |agora - tempo| <= 5 minutos e a
   assinatura confere. Repetição dentro da janela é barrada pelo servidor
   (memória das assinaturas vistas), fora desta especificação.

2. O JOB da tarefa. O JOB_ID do ULTRAX é o hash do pedido: dois clientes
   que pedem a mesma coisa teriam as mesmas sementes, e um nó poderia
   devolver de cor o que calculou na outra. A plataforma prende cada tarefa a
   um JOB próprio:

     job = H(DOMINIO_JOB || JOB_ID[64] || u64 tarefa || nonce[32])
     semente da unidade i = H(DOMINIO_UNIDADE_ULTRAX || job || u64 i)

3. O preço. Cobra-se trabalho conferido, medido em operações do modelo de
   custo de cada motor, não o tempo que o fornecedor diz ter gastado (tempo
   se infla; operação conferida, não). Os preços são por hora de um núcleo
   de referência, em micro-unidades da moeda (1 real = 1.000.000):

     CPU: ceil(ops * (cpu_nucleo_hora * 2^30 + memoria * ram_gib_hora)
                / (ops_por_s_cpu * 3600 * 2^30))
     GPU: ceil(ops * gpu_hora / (ops_por_s_gpu * 3600))

   Tetos: ops <= 2^50, memória <= 2^36 bytes, preço <= 2^40, 1 <= ops_por_s
   <= 2^40. A estimativa (o que se reserva do saldo) é o custo da unidade
   pelo teto de operações, vezes unidades e cópias, até 2^100.

4. A liquidação, uma vez por tarefa, quando ela termina:

     cobranca = ceil(soma das execuções pagas pelo cliente / 10.000) centavos
     fornecedores do cliente = floor(cobranca * fornecedor_bp / 10.000)
     conferência = floor(ceil(soma das conferências / 10.000) * fornecedor_bp / 10.000)
     plataforma = cobranca - fornecedores do cliente - conferência

   As conferências por amostragem são custo da plataforma (o cliente não
   paga a dúvida dela). Cada parte é repartida entre os nós pelo maior
   resto (Hamilton), proporcional às micro-unidades de cada um; empate vai
   para o nó de número menor. A parte da plataforma pode sair negativa:
   conferir um nó de reputação baixa custa mais do que a margem.

5. A reputação, de 0 a 100:

     pontuação = floor(100 * (v + 7) / (v + 10 + 2 f + 25 i))

   v = entregas conferidas, f = falhas (prazo vencido ou abandono), i =
   resultados incorretos confirmados. Nó novo começa em 70 (normal). A faixa
   decide quanto se confere por amostragem e quando o nó vai para a
   quarentena (pontuação abaixo de 50 ou 3 incorretos). Nas 10 primeiras
   entregas conferidas, tudo é conferido por outro nó (período de prova).
"""

from hyurax import codec, crypto, identidade, job

DOMINIO_PEDIDO = identidade.rotulo("PLATAFORMA-PEDIDO-v1")
DOMINIO_JOB = identidade.rotulo("PLATAFORMA-JOB-v1")
JANELA_MS = 5 * 60_000
PREFIXO_DO_AGENTE = "/api/v1/agente/"
CAMINHO_MAX = 512

MICRO_POR_CENTAVO = 10_000
GIB = 1 << 30
OPS_MAX = 1 << 50
MEMORIA_MAX = 1 << 36
PRECO_MAX = 1 << 40
OPS_POR_S_MAX = 1 << 40
ESTIMATIVA_MAX = 1 << 100
BP = 10_000


# ---------------------------------------------------------------------------
# 1. pedido do agente
# ---------------------------------------------------------------------------

def mensagem_do_pedido(metodo: str, caminho: str, tempo_ms: int, corpo: bytes) -> bytes:
    return crypto.H(DOMINIO_PEDIDO + codec.enc_str(metodo) + codec.enc_str(caminho) + codec.enc_u64(tempo_ms) + crypto.H(corpo))


def assinar_pedido(segredo: bytes, metodo: str, caminho: str, tempo_ms: int, corpo: bytes) -> bytes:
    return crypto.ed25519_sign(segredo, mensagem_do_pedido(metodo, caminho, tempo_ms, corpo))


def conferir_pedido(chave: bytes, metodo: str, caminho: str, tempo_ms: int, corpo: bytes, assinatura: bytes, agora_ms: int) -> str | None:
    """None quando vale; senão o motivo, com as palavras exatas."""
    if metodo not in ("GET", "POST"):
        return "método não aceito"
    if not caminho.startswith(PREFIXO_DO_AGENTE) or len(caminho) > CAMINHO_MAX or not all(0x21 <= ord(c) <= 0x7E for c in caminho):
        return "caminho fora da área do agente"
    if abs(agora_ms - tempo_ms) > JANELA_MS:
        return "pedido fora da janela de 5 minutos"
    if len(chave) != 32 or len(assinatura) != 64:
        return "assinatura do nó não confere"
    if not crypto.ed25519_verify(chave, mensagem_do_pedido(metodo, caminho, tempo_ms, corpo), assinatura):
        return "assinatura do nó não confere"
    return None


# ---------------------------------------------------------------------------
# 2. JOB da tarefa
# ---------------------------------------------------------------------------

def job_da_tarefa(job_id: bytes, tarefa: int, nonce: bytes) -> bytes:
    if len(job_id) != 64 or len(nonce) != 32:
        raise ValueError("tamanho errado")
    return crypto.H(DOMINIO_JOB + job_id + codec.enc_u64(tarefa) + nonce)


def semente(job_plataforma: bytes, indice: int) -> bytes:
    return job.semente_da_unidade(job_plataforma, indice)


# ---------------------------------------------------------------------------
# 3. preço
# ---------------------------------------------------------------------------

CAMPOS_DA_TABELA = ("cpu_nucleo_hora", "ram_gib_hora", "gpu_hora", "ops_por_s_cpu", "ops_por_s_gpu")


def validar_tabela(t: dict) -> None:
    for c in ("cpu_nucleo_hora", "ram_gib_hora", "gpu_hora"):
        if not 0 <= t[c] <= PRECO_MAX:
            raise ValueError("preço acima do teto")
    for c in ("ops_por_s_cpu", "ops_por_s_gpu"):
        if not 1 <= t[c] <= OPS_POR_S_MAX:
            raise ValueError("ritmo de referência fora da faixa")


def _teto(a: int, b: int) -> int:
    return -(-a // b)


def custo_da_execucao(t: dict, operacoes: int, memoria: int, gpu: bool) -> int:
    """Micro-unidades da moeda por uma execução."""
    validar_tabela(t)
    if operacoes > OPS_MAX:
        raise ValueError("operações acima do teto")
    if memoria > MEMORIA_MAX:
        raise ValueError("memória acima do teto")
    if gpu:
        return _teto(operacoes * t["gpu_hora"], t["ops_por_s_gpu"] * 3600)
    return _teto(operacoes * (t["cpu_nucleo_hora"] * GIB + memoria * t["ram_gib_hora"]), t["ops_por_s_cpu"] * 3600 * GIB)


def estimativa(t: dict, operacoes_maximas: int, memoria: int, unidades: int, copias: int, gpu: bool) -> int:
    total = custo_da_execucao(t, operacoes_maximas, memoria, gpu) * unidades * copias
    if total > ESTIMATIVA_MAX:
        raise ValueError("estimativa acima do teto")
    return total


def centavos(micro: int) -> int:
    return _teto(micro, MICRO_POR_CENTAVO)


# ---------------------------------------------------------------------------
# 4. liquidação
# ---------------------------------------------------------------------------

def repartir(total: int, pesos: dict[int, int]) -> dict[int, int]:
    """Maior resto: cada nó leva floor(total * peso / soma), e o que sobra vai
    de 1 em 1 para os maiores restos; empate, nó de número menor."""
    soma = sum(pesos.values())
    if soma == 0 or total == 0:
        return {no: 0 for no in pesos}
    partes = {no: total * p // soma for no, p in pesos.items()}
    sobra = total - sum(partes.values())
    ordem = sorted(pesos, key=lambda no: (-(total * pesos[no] % soma), no))
    for no in ordem[:sobra]:
        partes[no] += 1
    return partes


def liquidar(execucoes: list[tuple[int, int, str]], fornecedor_bp: int) -> dict:
    """`execucoes`: (nó, micro, pagador), pagador "cliente" ou "plataforma"."""
    if not 0 <= fornecedor_bp <= BP:
        raise ValueError("parte do fornecedor fora da faixa")
    do_cliente: dict[int, int] = {}
    da_plataforma: dict[int, int] = {}
    for no, micro, pagador in execucoes:
        alvo = do_cliente if pagador == "cliente" else da_plataforma
        alvo[no] = alvo.get(no, 0) + micro
    cobranca = centavos(sum(do_cliente.values()))
    pool_cliente = cobranca * fornecedor_bp // BP
    pool_conferencia = centavos(sum(da_plataforma.values())) * fornecedor_bp // BP
    partes: dict[int, int] = {}
    for pool, pesos in ((pool_cliente, do_cliente), (pool_conferencia, da_plataforma)):
        for no, v in repartir(pool, pesos).items():
            partes[no] = partes.get(no, 0) + v
    return {
        "cobranca": cobranca,
        "fornecedores": dict(sorted(partes.items())),
        "conferencia": pool_conferencia,
        "plataforma": cobranca - pool_cliente - pool_conferencia,
    }


# ---------------------------------------------------------------------------
# 5. reputação
# ---------------------------------------------------------------------------

def pontuacao(verificadas: int, falhas: int, incorretas: int) -> int:
    return max(0, min(100, 100 * (verificadas + 7) // (verificadas + 10 + 2 * falhas + 25 * incorretas)))


def faixa(p: int) -> str:
    if p >= 100:
        return "excelente"
    if p >= 90:
        return "confiavel"
    if p >= 70:
        return "normal"
    if p >= 50:
        return "atencao"
    return "risco"


PERIODO_DE_PROVA = 10


def amostragem_bp(p: int, verificadas: int) -> int:
    """Chance, em pontos-base, de uma entrega ser conferida por outro nó."""
    if verificadas < PERIODO_DE_PROVA:
        return 10_000
    if p >= 90:
        return 1_000
    if p >= 70:
        return 2_500
    if p >= 50:
        return 5_000
    return 10_000


def quarentena(p: int, incorretas: int) -> bool:
    return p < 50 or incorretas >= 3
