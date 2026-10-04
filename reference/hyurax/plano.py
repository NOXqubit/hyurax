# ✝ Lucas 14:28 — “Qual de vós, querendo edificar uma torre, não se assenta primeiro a fazer as contas dos gastos?”
"""Planos do Hyurax: o catálogo e o voucher de plano assinado.

Especificação de referência de `crates/hyurax-nuvem/src/plano.rs` (ver
docs/MONETIZACAO.md). Fora do consenso: nada aqui muda bloco, transação ou
cadeia. Créditos de computação NÃO são HYX nem dinheiro; o preço do plano é
em reais e é cobrado fora do programa, por uma empresa.

1. O catálogo: identificador, nome, preço mensal e anual em centavos de real,
   créditos por mês, armazenamento gerenciado em GiB e comissão da plataforma
   no mercado de máquinas, em pontos-base (1500 = 15%).

2. O voucher (o plano ativo de um nó), assinado pela chave de planos do
   projeto. Sem dado pessoal: o beneficiário é a chave pública do worker.

     u8 VERSAO || u8 plano || beneficiario[32] || u64 inicio_ms || u64 fim_ms
     || u64 creditos_mes || u32 armazenamento_gib || u16 comissao_bp
     || u32 serie || assinatura[64]

   assinatura = Ed25519(emissor, H(DOMINIO_PLANO || tudo antes dela)).
   Os campos de benefício vêm escritos no voucher (não só o número do plano):
   um voucher emitido com um preço antigo continua valendo o que vendeu.

   Regras: plano do catálogo e pago (não o gratuito); fim > inicio; fim -
   inicio de no máximo 400 dias; comissão de no máximo 10000 bp. O texto é
   `hyurax-plano:` seguido dos bytes em hexadecimal.

3. Vale para um nó quando: a assinatura confere com a chave de planos
   embutida, o beneficiário é o worker do nó e o instante está em
   [inicio, fim).
"""

from hyurax import codec, crypto, identidade

DOMINIO_PLANO = identidade.rotulo("PLANO-v1")
VERSAO = 1
PREFIXO = "hyurax-plano:"
DURACAO_MAX_MS = 400 * 86_400_000
TAMANHO = 1 + 1 + 32 + 8 + 8 + 8 + 4 + 2 + 4 + 64

COMUNIDADE, PRO, EQUIPE, EMPRESA = 0, 1, 2, 3

# id: (nome, centavos/mês, centavos/ano, créditos/mês, GiB gerenciados, comissão bp)
CATALOGO = {
    COMUNIDADE: ("Comunidade", 0, 0, 0, 0, 1500),
    PRO: ("Pro", 2_900, 29_000, 150_000, 50, 1000),
    EQUIPE: ("Equipe", 14_900, 149_000, 1_000_000, 500, 800),
    EMPRESA: ("Empresa", 99_000, 990_000, 10_000_000, 5_000, 500),
}


def corpo(v: dict) -> bytes:
    if v["plano"] not in CATALOGO or v["plano"] == COMUNIDADE:
        raise ValueError("plano desconhecido")
    if len(v["beneficiario"]) != 32:
        raise ValueError("beneficiário com tamanho errado")
    if v["fim_ms"] <= v["inicio_ms"]:
        raise ValueError("o voucher acaba antes de começar")
    if v["fim_ms"] - v["inicio_ms"] > DURACAO_MAX_MS:
        raise ValueError("voucher mais longo que 400 dias")
    if v["comissao_bp"] > 10_000:
        raise ValueError("comissão acima de 100%")
    return (
        codec.enc_u8(VERSAO) + codec.enc_u8(v["plano"]) + v["beneficiario"]
        + codec.enc_u64(v["inicio_ms"]) + codec.enc_u64(v["fim_ms"])
        + codec.enc_u64(v["creditos_mes"]) + codec.enc_u32(v["armazenamento_gib"])
        + codec.enc_u16(v["comissao_bp"]) + codec.enc_u32(v["serie"])
    )


def emitir(segredo_emissor: bytes, plano: int, beneficiario: bytes, inicio_ms: int, fim_ms: int, serie: int) -> bytes:
    """Voucher com os benefícios do catálogo de hoje."""
    _, _, _, creditos, gib, comissao = CATALOGO.get(plano, (None, 0, 0, 0, 0, 0))
    v = {
        "plano": plano, "beneficiario": beneficiario, "inicio_ms": inicio_ms, "fim_ms": fim_ms,
        "creditos_mes": creditos, "armazenamento_gib": gib, "comissao_bp": comissao, "serie": serie,
    }
    c = corpo(v)
    return c + crypto.ed25519_sign(segredo_emissor, crypto.H(DOMINIO_PLANO + c))


def ler(dados: bytes) -> dict:
    r = codec.Reader(dados)
    if r.u8() != VERSAO:
        raise ValueError("versão de voucher desconhecida")
    v = {
        "plano": r.u8(), "beneficiario": r.take(32), "inicio_ms": r.u64(), "fim_ms": r.u64(),
        "creditos_mes": r.u64(), "armazenamento_gib": r.u32(), "comissao_bp": r.u16(), "serie": r.u32(),
    }
    v["assinatura"] = r.take(64)
    r.finish()
    corpo(v)  # as mesmas regras de quem emite
    return v


def ler_texto(texto: str) -> dict:
    t = texto.strip()
    if not t.startswith(PREFIXO):
        raise ValueError("isto não é um voucher de plano do Hyurax")
    h = t[len(PREFIXO):]
    if len(h) != 2 * TAMANHO or any(c not in "0123456789abcdefABCDEF" for c in h):
        raise ValueError("voucher com tamanho errado ou caractere inválido")
    return ler(bytes.fromhex(h))


def texto(dados: bytes) -> str:
    return PREFIXO + dados.hex()


def assinatura_confere(v: dict, emissor: bytes) -> bool:
    return crypto.ed25519_verify(emissor, crypto.H(DOMINIO_PLANO + corpo(v)), v["assinatura"])


def vale(v: dict, emissor: bytes, worker: bytes, agora_ms: int) -> bool:
    return assinatura_confere(v, emissor) and v["beneficiario"] == worker and v["inicio_ms"] <= agora_ms < v["fim_ms"]
