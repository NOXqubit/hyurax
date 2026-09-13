# Auron — implementação de referência (Python)

**Isto não é o nó de produção.** O nó de produção será em Rust. Este pacote
existe para três coisas:

1. Especificar sem ambiguidade o que o protocolo faz.
2. Gerar vetores de teste que o Rust precisa reproduzir byte a byte.
3. Servir de oráculo em testes cruzados Python ↔ Rust.

Por isso prioriza clareza e determinismo sobre velocidade, e **não tem
dependência externa na regra de consenso**. A única biblioteca de terceiros é
o numpy. Desde o consenso híbrido (`usefulpow.py`) ela também roda na
validação de bloco, mas só como calculadora de multiplicação **inteira** exata
(`int64`, sem ponto flutuante, com faixa conferida antes para não estourar): a
geração das matrizes e o desafio são SHA-512, e o resultado é travado por
`vectors/usefulpow.json`. No Rust (ainda não migrado) a mesma conta será laço simples.

## Rodar

```bash
python tests/run_all_tests.py
```

121 testes. Entre 2 e 4 minutos nesta máquina, conforme a memória livre. Não
precisa instalar nada além do numpy.

## Achados próprios, depois do protótipo

Dois bugs que não vieram do protótipo: apareceram no código que eu mesmo
escrevi, e foram pegos ao escrever o teste que faltava para `units.py`.

| Achado | Por que importava | Teste |
|---|---|---|
| `to_units("１")` devolvia 1 AUR | `str.isdigit()` do Python aceita dígito Unicode de largura completa e `int()` converte. O Rust recusa. Os dois lados discordariam sobre o que é valor válido, e a mesma string na tela viraria valores diferentes | `test_00::test_entrada_malformada_recusada` |
| `to_units("-1.5")` era aceito | A especificação diz que dinheiro é `u64`. Um valor negativo não tem como ser serializado, então o vetor de teste continha um caso que o Rust nunca poderia reproduzir | `test_00::test_negativo_recusado_nos_dois_caminhos` |

`units.py` era o único módulo sem arquivo de teste próprio. Foi exatamente
onde os dois se esconderam.

## Estado

Fase 0 concluída. Os doze bugs levantados na leitura do protótipo estão
corrigidos, cada um com teste de regressão que falharia na versão antiga.

| # | Bug do protótipo | Corrigido em | Teste de regressão |
|---|---|---|---|
| 1 | Freivalds sorteava vetores sem semente; nós honestos podiam discordar | `utrax.py` | `test_06::test_freivalds_is_deterministic` |
| 2 | Mochila aceitava qualquer solução viável; 40 bytes de zeros passavam | `utrax.py` | `test_06::test_knapsack_rejects_all_zeros` |
| 3 | Dificuldade e tamanho do problema eram o mesmo número | `consensus.py` | `test_04::test_verification_cost_independent_of_difficulty` |
| 4 | Não existia ajuste de dificuldade | `consensus.py` | `test_04::test_retarget_reacts_to_fast_and_slow_blocks` |
| 5 | Nó aceitava bloco de peer sem verificar prova de trabalho | `chain.py` | `test_05::test_forged_block_without_pow_rejected` |
| 6 | Cabeçalho não cobria as assinaturas | `block.py` | `test_05::test_merkle_root_covers_signatures` |
| 7 | Duas definições incompatíveis de gênese | `chain.py` | `test_05::test_genesis_is_single_and_deterministic` |
| 8 | Teste do marketplace passava por sorte | `utrax.py` | `test_06::test_knapsack_rejects_all_zeros` |
| 9 | Sem Merkle, sem teto de emissão, sem persistência | `codec.py`, `consensus.py`, `store.py` | `test_02`, `test_04::test_emission_halves_and_respects_cap`, `test_07` |
| 10 | Recompensa mobile amarrada ao topo exato da cadeia | mecanismo **removido** | ver abaixo |
| 11 | Marketplace: o verificador fazia o trabalho; escrow burlava o estado | `utrax.py` | `test_06::test_invalid_submission_does_not_lock_task` |
| 12 | Brainpool P-512 em Python puro, dezenas de ms por verificação | `crypto.py` | `test_01::test_rfc8032_vectors` |

### Sobre o bug 10

Não foi corrigido, foi **removido**. O protótipo tinha registro de dispositivo e
reivindicação de recompensa por época para celular. O desenho era inviável na
raiz: a prova era amarrada ao hash exato da ponta, então cada bloco novo
invalidava toda prova em andamento e o celular sempre perdia a corrida.

Com PoW convencional, celular e desktop rodam o **mesmo** minerador, que é
exatamente o que você definiu no escopo (Termux e CMD, mesmo binário). O
Argon2id com pressão de memória já nivela bastante a disputa entre um telefone e
um desktop. Menos código, menos superfície de ataque, mesmo resultado.

## Validação contra padrões externos

Nada aqui é "confia em mim". As três primitivas críticas batem com vetores
oficiais publicados:

| Primitiva | Padrão | Vetores |
|---|---|---|
| Ed25519 | RFC 8032 §7.1 | 4 vetores, incluindo mensagem de 1023 bytes |
| Argon2id | RFC 9106 §5 | as 3 variantes (d, i, id) |
| Merkle | RFC 6962 | provas de inclusão em 1..17 folhas |

## Módulos

| Arquivo | O que é | Consenso? |
|---|---|---|
| `units.py` | dinheiro inteiro, teto de supply | sim |
| `crypto.py` | AACL: SHA-512, Ed25519, endereços | sim |
| `codec.py` | codificação canônica binária, Merkle | sim |
| `argon2.py` | Argon2id RFC 9106, Python puro | sim |
| `consensus.py` | alvo, PoW, retarget LWMA, emissão | sim |
| `tx.py` | coinbase e transferência | sim |
| `state.py` | saldos, nonces, maturação, desfazer | sim |
| `block.py` | cabeçalho e bloco | sim |
| `chain.py` | gênese, validação, reorg, escolha de ponta | sim |
| `store.py` | persistência com revalidação | sim |
| `utrax.py` | trabalho útil, marketplace | **não** |

## O que substituiu o quê

O dump original tinha **três cópias divergentes** do mesmo core, que já haviam
começado a discordar entre si:

- `auron_complete_now/core/auron_core.py`
- `auron_complete_now/core/auron_reference.py`
- `auron_complete_now/core/{units,wallet,pow_core,blockchain,...}.py`

As três foram descartadas em favor deste pacote. `auron_reference.py` era a mais
correta das três e serviu de ponto de partida.

Os arquivos originais **não foram copiados para o disco**: eles chegaram pelo
chat e nunca existiram em `D:\auron`. Se quiser preservar a proveniência,
coloque-os em `reference/archive/` — a pasta já está criada e não é lida por
nada.

Observação: o dump original **não rodava nesta máquina**. Ele importa `ecdsa`,
que não está instalado. A suíte de testes do protótipo nunca poderia ter
passado aqui.

## Fusion

`fusion_core.py` continua congelado, sem manutenção nesta janela, como
combinado. Dois defeitos ficaram anotados para quando ele for retomado:

- `_transition` tem código morto: a condição externa já garante
  `d.state != to`, então o `if` interno nunca é falso.
- `verify_fusion` pode devolver `gates` vazio em silêncio, por causa do
  desempacotamento condicional da tupla.

## Próximo passo

Fase 1: workspace Rust, e o harness que compara cada vetor de `vectors/` entre
Python e Rust. Gere os vetores com:

```bash
python tools/gen_vectors.py
```
