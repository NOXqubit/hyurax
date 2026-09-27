# ✝ Neemias 4:6 — “Assim edificamos o muro, porque o povo tinha ânimo para trabalhar.”
"""JOB do ULTRAX: um pedido de computação científica dividido em unidades.

Especificação de referência de `crates/hyurax-ultrax/src/job.rs`. Fora do
consenso: nada aqui muda bloco ou transação.

Um JOB diz qual motor roda, com qual especificação-modelo, em quantas
unidades, com que nível de verificação e redundância, até quando e com que
orçamento. O `JOB_ID` é o hash da codificação canônica.

A unidade `i` não é guardada em lugar nenhum: a especificação dela sai do
modelo (na triagem, a faixa de moléculas anda com `i`) e a semente é
`H(DOMINIO_UNIDADE || JOB_ID || u64 i)`. Um JOB de um bilhão de unidades se
descreve no mesmo tamanho que um de dez.

O progresso se guarda em memória proporcional aos buracos, não ao total: o
conjunto de intervalos `[a, b)` concluídos. O resumo dos resultados é
aditivo (soma módulo 2^512 de um hash por unidade), então não depende da
ordem em que as unidades chegaram. Ele serve para dois nós conferirem que
consolidaram o mesmo conjunto; não é prova criptográfica forte contra quem
escolhe os resultados de propósito, e o relatório guarda o hash de cada
unidade para isso.

Créditos de computação, versão 1: 1 crédito = 10^9 operações executadas e
conferidas. Tempo, memória e rede são registrados, mas não precificados na
v1: tempo depende da máquina, e contar tempo premiaria a máquina lenta.
Crédito não é HYX: nada converte um no outro.
"""

from __future__ import annotations

from dataclasses import dataclass, field

from . import codec, crypto, identidade

DOMINIO_JOB = identidade.rotulo("ULTRAX-JOB-v1")
DOMINIO_UNIDADE = identidade.rotulo("ULTRAX-UNIDADE-v1")
DOMINIO_RESUMO = identidade.rotulo("ULTRAX-RESUMO-v1")

VERSAO = 1
DESCRICAO_MAX = 2000
UNIDADES_MAX = 1 << 40
REDUNDANCIA_MAX = 7
OPERACOES_POR_MILICREDITO = 1_000_000
MODULO_RESUMO = 1 << 512

# Domínio de pesquisa -> tipos de trabalho (códigos) que atendem. Vazio =
# domínio declarado, sem motor ainda: o JOB é recusado.
DOMINIOS = {
    1: ("molecular", (8,)),
    2: ("genetica", (5,)),
    3: ("agricultura", (6,)),
    4: ("logistica", (7,)),
    5: ("economia", (2,)),
    6: ("ia", (4,)),
    7: ("matematica", (1, 3)),
    8: ("materiais", ()),
    9: ("energia", ()),
    10: ("meio-ambiente", ()),
    11: ("regeneracao", ()),
}

# Parâmetros extras por tipo (os antigos não levam).
PARAMETROS_POR_TIPO = {1: 0, 2: 0, 3: 0, 4: 0}


def enc_especificacao(tipo: int, tamanho: int, passos: int, parametros: list[int]) -> bytes:
    """`u8 tipo || u32 tamanho || u32 passos`, e nos tipos científicos
    `|| u8 n || n × u32`."""
    saida = codec.enc_u8(tipo) + codec.enc_u32(tamanho) + codec.enc_u32(passos)
    if tipo >= 5:
        saida += codec.enc_u8(len(parametros)) + b"".join(codec.enc_u32(p) for p in parametros)
    elif parametros:
        raise ValueError("tipo antigo não leva parâmetros")
    return saida


@dataclass(frozen=True)
class Job:
    dominio: int
    tipo: int
    tamanho: int
    passos: int
    parametros: tuple[int, ...]
    unidades: int
    nivel: int
    redundancia: int
    prazo_s: int
    orcamento_milicreditos: int
    descricao: str

    def validar(self) -> None:
        if self.dominio not in DOMINIOS:
            raise ValueError("domínio desconhecido")
        nome, motores = DOMINIOS[self.dominio]
        if not motores:
            raise ValueError(f"domínio {nome} sem motor")
        if self.tipo not in motores:
            raise ValueError("motor não atende o domínio")
        if not 1 <= self.unidades <= UNIDADES_MAX:
            raise ValueError("unidades fora da faixa")
        if not 1 <= self.nivel <= 5:
            raise ValueError("nível fora da faixa")
        if not 1 <= self.redundancia <= REDUNDANCIA_MAX:
            raise ValueError("redundância fora da faixa")
        if self.nivel >= 3 and self.redundancia < 2:
            raise ValueError("concordância exige redundância 2 ou mais")
        texto = self.descricao.encode("utf-8")
        if len(texto) > DESCRICAO_MAX:
            raise ValueError("descrição longa demais")
        if any(ord(c) < 0x20 and c not in "\n\t" for c in self.descricao):
            raise ValueError("descrição com caractere de controle")

    def codificar(self) -> bytes:
        return (
            codec.enc_u8(VERSAO)
            + codec.enc_u8(self.dominio)
            + enc_especificacao(self.tipo, self.tamanho, self.passos, list(self.parametros))
            + codec.enc_u64(self.unidades)
            + codec.enc_u8(self.nivel)
            + codec.enc_u8(self.redundancia)
            + codec.enc_u64(self.prazo_s)
            + codec.enc_u64(self.orcamento_milicreditos)
            + codec.enc_str(self.descricao)
        )

    def id(self) -> bytes:
        return crypto.H(DOMINIO_JOB + self.codificar())


def semente_da_unidade(job_id: bytes, indice: int) -> bytes:
    return crypto.H(DOMINIO_UNIDADE + job_id + codec.enc_u64(indice))


def folha_do_resumo(indice: int, hash_do_resultado: bytes) -> int:
    return int.from_bytes(crypto.H(DOMINIO_RESUMO + codec.enc_u64(indice) + hash_do_resultado), "big")


def resumo(pares: list[tuple[int, bytes]]) -> bytes:
    """Soma módulo 2^512 das folhas: mesma saída em qualquer ordem."""
    total = 0
    for indice, hash_do_resultado in pares:
        total = (total + folha_do_resumo(indice, hash_do_resultado)) % MODULO_RESUMO
    return total.to_bytes(64, "big")


@dataclass
class Intervalos:
    """Intervalos `[a, b)` ordenados, disjuntos e não encostados."""

    faixas: list[tuple[int, int]] = field(default_factory=list)

    def inserir(self, a: int, b: int) -> None:
        if a >= b:
            return
        novas = []
        for x, y in self.faixas:
            if y < a or x > b:
                novas.append((x, y))
            else:
                a, b = min(a, x), max(b, y)
        novas.append((a, b))
        self.faixas = sorted(novas)

    def contem(self, i: int) -> bool:
        return any(x <= i < y for x, y in self.faixas)

    def concluidas(self) -> int:
        return sum(y - x for x, y in self.faixas)

    def primeira_faltante(self, desde: int, total: int) -> int | None:
        i = desde
        for x, y in self.faixas:
            if y <= i:
                continue
            if x > i:
                break
            i = y
        return i if i < total else None

    def codificar(self) -> bytes:
        return codec.enc_u32(len(self.faixas)) + b"".join(codec.enc_u64(x) + codec.enc_u64(y) for x, y in self.faixas)


def milicreditos(operacoes: int, operacoes_verificacao: int) -> int:
    return (operacoes + operacoes_verificacao) // OPERACOES_POR_MILICREDITO
