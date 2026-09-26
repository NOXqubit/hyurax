# ULTRAX — motor de trabalho útil

Escrito em 26/09/2026, a partir do pedido do autor de reestruturar a
"mineração" como um motor de trabalho útil, mensurável e verificável.

> Não prove que seu computador estava ligado. Prove que ele realizou um
> trabalho verificável.

**Nome.** ULTRAX é o nome do produto. Os rótulos de protocolo que já existem
(`HYURAX-UTRAX-INSTANCE-v1` e outros) continuam como estão: entram em hash, e
trocá-los mudaria vetores e a instância do trabalho útil do consenso.

## 1. O que existe hoje (análise de 26/09/2026)

| Peça | Onde | Estado | O que aproveitar |
|---|---|---|---|
| Argon2id do consenso | `hyurax-pow` | Rust, testado | Continua: é a segurança da cadeia |
| Trabalho útil do consenso (`MATRIX-FREIVALDS-V1`) | `hyurax-usefulpow`, spec §9A | Rust, igual ao gabarito | Gerador de matrizes e Freivalds |
| Mercado Utrax: matriz, mochila, difusão, escrow | `reference/hyurax/utrax.py`, spec §18 | **só Python** | Os três trabalhos e as regras (executor entrega resultado, submissão inválida não trava tarefa) |
| Vetores do Utrax | `vectors/utrax.json` | prontos | Conferir a tradução byte a byte |
| Minerador com limite de CPU | `hyurax-no/src/painel.rs` | Rust | Linhas, pausa proporcional, medição por tentativa |
| Identidade de nó (Ed25519) | `hyurax-net`, `PASTA/no.chave` | Rust | É o `WORKER_ID` natural |
| Rede cifrada entre nós | `hyurax-net`, spec §21 | Rust | Transporte do modo TESTNET |
| Painel com janela | `hyurax-no/painel/` | Rust + JS | Vira o painel do ULTRAX; a estação 3D sai |

### O achado que define o plano

**Hoje, quase todo o esforço da máquina vai para o Argon2id.** Cada tentativa
usa 32 MiB e é feita milhares de vezes por bloco. O trabalho útil do consenso
é **uma** multiplicação de matrizes por candidato a bloco: na testnet, 48×48,
cerca de 110 mil multiplicações, que um celular faz em milissegundos. O
trabalho útil do consenso é real e verificável, mas **é pequeno** perto do
Argon2id.

Chamar a mineração atual de "trabalho útil" seria exatamente o que o pedido
proíbe: trocar o nome sem mudar o que a máquina faz. Por isso a arquitetura
abaixo **separa** as duas coisas:

- **Mineração** (Argon2id + trabalho útil do consenso): protege a cadeia e é o
  que rende HYX pela regra de emissão. Continua existindo e continua com esse
  nome.
- **ULTRAX** (trabalho útil fora do consenso): tarefas de verdade, executadas,
  verificadas e medidas. Rende **Work Score**, não HYX. Não existe regra que
  converta um no outro.

A máquina divide os recursos entre as duas, pelos limites que o usuário
escolher.

## 2. Princípios que não mudam

1. **Nada aparece na tela sem estar sendo calculado.** Toda tarefa mostrada
   foi executada por esta máquina, e o resultado tem hash e verificação.
2. **O modo está sempre visível:** LAB, TESTNET ou REAL. Não existe REAL
   enquanto não houver cliente externo de verdade.
3. **Benchmark não é cliente. Simulação não é receita. Work Score não é
   dinheiro.** Tarefa gerada localmente leva o selo "TESTNET WORKLOAD".
4. **Resultado de worker não é verdade até ser verificado.** O método de
   verificação faz parte da tarefa, não é decidido depois.
5. **Nenhum código de fora roda na máquina do usuário.** Só rodam os tipos de
   trabalho compilados no programa, com parâmetros validados. Código externo
   só com sandbox (Etapa 6).
6. **Cada mecanismo tem função.** Assinatura existe porque atribui resultado a
   um worker; redundância existe porque há workers independentes. Três
   threads da mesma máquina não são redundância, e não vão ser mostradas como
   se fossem.

## 3. Arquitetura

```
             ┌──────────── TASK ENGINE ────────────┐
 demanda ──► │ tipo, parâmetros, requisitos,       │
 (LAB hoje)  │ verificação, prazo, prioridade, ID  │
             └────────────────┬────────────────────┘
                              ▼
 RESOURCE MANAGER ◄──── WORKER ENGINE ────► PROOF ENGINE
 CPU, RAM, GPU,         reserva, executa,    TASK_ID, INPUT_HASH,
 prazo, cancelar        progresso, resultado RESULT_HASH, WORKER_ID,
                              │              assinatura, STATUS
                              ▼
                     VERIFICATION ENGINE
                     Freivalds, recomputação,
                     resultado esperado, hash
                              │
                              ▼
                      VALIDATOR ENGINE ────► REPUTATION ENGINE
                      compara workers,       enviadas, verificadas,
                      divergência, suspeita  recusadas, disputadas
                              │
                              ▼
                     SETTLEMENT ENGINE
                     Work Score (medida); recompensa
                     só com regra econômica publicada
```

**Onde fica cada peça:**

- **Crate novo `hyurax-ultrax`**: biblioteca pura, sem rede e sem disco.
  - tipos de trabalho;
  - tarefa e ciclo de vida;
  - registro de prova;
  - verificação;
  - validador;
  - reputação;
  - Work Score.

  É a parte crítica, então é a que tem testes e vetores.
- **`hyurax-no`**: o worker de verdade.
  - gerador de tarefas LAB;
  - fila;
  - execução com limites;
  - histórico em disco;
  - telemetria;
  - painel.
- **`hyurax-net`**: mensagens de tarefa e resultado entre nós (Etapa 5).

### Ciclo de vida

```
CREATED → QUEUED → ASSIGNED → EXECUTING → SUBMITTED → VERIFYING → VERIFIED → SETTLED
                                   │                        └────→ REJECTED
                                   └→ CANCELLED / EXPIRED
```

Só essas transições são aceitas; qualquer outra é erro, com teste. Cada
transição vira um evento com horário, gravado num registro só de acréscimo.

### Registro de prova

| Campo | Conteúdo |
|---|---|
| `TASK_ID` | SHA-512 da especificação codificada: tipo, parâmetros, semente, verificação |
| `INPUT_HASH` | SHA-512 da entrada (as matrizes, os itens da mochila, a grade, o lote de dados) |
| `WORK_TYPE` / `PARAMETERS` | O que foi pedido |
| `VERIFICATION_METHOD` | Freivalds, recomputação, resultado esperado ou redundância |
| `RESULT_HASH` | SHA-512 do resultado, na codificação canônica |
| `WORKER_ID` | Chave pública da identidade do nó (`no.chave`) |
| `TIMESTAMP` | Início e fim da execução |
| `STATUS` | Estado atual do ciclo |
| assinatura | Ed25519 do worker sobre os campos acima, para o resultado ser atribuível |

### Work Score, versão 1

Uma medida de contribuição, **não** de valor:

- cada tipo de trabalho declara um **modelo de custo em operações inteiras**;
  - matriz n×n: `n³` multiplicações com soma;
  - mochila: `n · (C+1)` células;
  - difusão: `g² · passos` células;
  - IA: multiplicações com soma de cada camada;
- **só tarefa VERIFICADA soma pontos.** Recusada soma zero;
- 1 ponto = 1 milhão de operações.

Os outros fatores do pedido (qualidade, disponibilidade, custo de
verificação, dificuldade) entram quando houver medição que os sustente, cada
um com regra publicada e versão (`work_score_version`). Um fator somado sem
medição é chute, e chute vestido de métrica é o que o princípio 3 proíbe.

### Modos

| Modo | Tarefas | Verificação | Quando |
|---|---|---|---|
| **LAB** | Geradas nesta máquina | Pela própria máquina, com método matemático ou recomputação; mais tarefas-desafio de resposta conhecida | Etapa 2 |
| **TESTNET** | Trocadas entre nós da rede de teste | Por outros nós; redundância entre workers diferentes | Etapa 5 |
| **REAL** | De clientes externos | Paga pelo cliente (spec §18) | Só com cliente de verdade |

## 4. Tipos de trabalho

| Tipo | Primeiro trabalho | Verificação | Etapa |
|---|---|---|---|
| Matemático | Multiplicação de matrizes | Freivalds, O(n²), erro ≤ 2⁻⁸⁰ | 1 |
| Matemático | Difusão em ponto fixo | Recomputação | 1 |
| Otimização | Mochila 0/1, exigindo o ótimo | Recomputação da programação dinâmica | 1 |
| IA | Rede neural pequena, em inteiros de ponto fixo, prevendo solubilidade de moléculas reais (AqSolDB, domínio público CC0) | Recomputação da inferência; Freivalds nas camadas | 4 |
| Verificação | Reconferir provas de trabalho útil dos blocos já guardados | Resultado esperado: a cadeia já foi aceita | 2 |
| Rede | Latência e disponibilidade medidas contra os pares reais | Medida direta, sem prova forte (dito na tela) | 2 |
| Matemático | Transformada NTT | Avaliação em ponto aleatório (Schwartz-Zippel) | depois |

A IA usa **inteiros**, não ponto flutuante, pelo mesmo motivo da difusão:
dois nós precisam chegar no mesmo resultado bit a bit para a redundância
funcionar. Na tela, a IA aparece como **"AI WORK — LAB"** até existir alguém
que tenha encomendado o treino.

## 5. Plano de migração, por etapas

Cada etapa termina com testes, clippy limpo e commit próprio. A seguinte só
começa com a anterior verde.

| Etapa | Entrega | Pronta quando |
|---|---|---|
| 0 | Este documento | O autor leu |
| 1 | Crate `hyurax-ultrax`: tipos de trabalho (matriz, mochila, difusão), especificação e `TASK_ID`, ciclo de vida, registro de prova, verificação, validador por maioria, reputação, Work Score v1 | Os três trabalhos batem byte a byte com `vectors/utrax.json`; transição inválida recusada; resultado adulterado recusado; divergência 2×1 detectada sem punir quem não tem evidência contra si |
| 2 | Worker LAB no `hyurax-no`: gerador de tarefas, fila, execução com limite de CPU e memória, prazo e cancelamento, tarefas-desafio, histórico em disco, telemetria com modo DEBUG | O programa executa e verifica tarefas sozinho, respeitando os limites, e o histórico sobrevive a reiniciar |
| 3 | Painel ULTRAX no lugar da estação 3D: trabalho ativo, histórico, verificação, contribuição, nó e o selo do modo | Um usuário responde às 7 perguntas do critério de sucesso olhando a tela |
| 4 | Trabalho de IA: rede pequena em ponto fixo sobre a AqSolDB, com a molécula desenhada na tela | Inferência verificada por recomputação; a curva de erro na tela sai do cálculo real |
| 5 | TESTNET: mensagens de tarefa e resultado entre nós, redundância com workers independentes, reputação por `WORKER_ID` | Três nós separados executam a mesma tarefa, e um resultado adulterado é detectado pela maioria |
| 6 | GPU (depois de medir se compensa), sandbox para trabalho externo, estrutura de JOB/cliente | Só com hardware para testar e com decisão sobre o equilíbrio CPU × GPU |

### GPU, dito com todas as letras

A máquina de desenvolvimento tem uma Intel HD Graphics integrada (Atom
x5-Z8350). Usar GPU em Rust puro exige uma biblioteca grande (`wgpu`), cara de
compilar aqui e que talvez nem funcione nesse chip. Por isso a GPU fica para a
Etapa 6. Até lá, o painel mostra a GPU como **"não usada"**, e não inventa um
percentual.

## 6. Critério de sucesso

O usuário olha o ULTRAX e responde:

1. o que o computador está fazendo;
2. qual tarefa recebeu;
3. quanto trabalho executou;
4. se o resultado foi verificado;
5. como a contribuição foi medida;
6. qual a reputação do nó;
7. em qual ambiente está: LAB, TESTNET ou REAL.
