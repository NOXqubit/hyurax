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
| Identidade de nó (X25519, a chave da cifra) | `hyurax-net`, `PASTA/no.chave` | Rust | O `WORKER_ID` é derivado dela (ver o registro de prova) |
| Rede cifrada entre nós | `hyurax-net`, spec §21 | Rust | Transporte do modo TESTNET |
| Painel com janela | `hyurax-no/painel/` | Rust + JS | Vira o painel do ULTRAX; a estação 3D saiu na Etapa 3 |

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

Até o veredito, a tarefa pode ser cancelada ou expirar. Cancelar durante a
conferência deixa o resultado **sem julgamento**, o que não é o mesmo que
recusado. Recusada volta para a fila uma vez; recusada de novo é abandonada
(erro que se repete).

Só essas transições são aceitas; qualquer outra é erro, com teste. Cada
transição vira um evento com horário, gravado no histórico
(`PASTA/ultrax/historico.txt`). O histórico só recebe linhas novas; passando
de 4 MiB, o arquivo vira `historico.txt.1` e o `.1` anterior se perde: ele
guarda as tarefas recentes (alguns milhares), não todas desde sempre.

A telemetria segue a ordem da especificação, com uma troca registrada aqui:
**RESOURCE ALLOCATED vem antes de WORK STARTED**, porque a memória é reservada
antes de o trabalho começar. Tarefa que não cabe nem começa.

### Registro de prova

| Campo | Conteúdo |
|---|---|
| `TASK_ID` | SHA-512 da especificação codificada: tipo, parâmetros, método de verificação, forma da instância, origem, prioridade, horários |
| `INPUT_HASH` | Tarefa gerada: SHA-512 de especificação e semente, porque a entrada é função determinística das duas (evita montar 8 MB só para o hash). Tarefa com dados de fora, quando existir: hash dos dados |
| `WORK_TYPE` / `PARAMETERS` | O que foi pedido |
| `VERIFICATION_METHOD` | Freivalds, recomputação, resultado esperado ou redundância |
| `RESULT_HASH` | SHA-512 do resultado, na codificação canônica |
| `WORKER_ID` | Chave pública Ed25519 do worker, derivada por hash com domínio próprio do segredo de `no.chave`. A chave de `no.chave` é X25519, da cifra; usar o mesmo segredo nos dois algoritmos não se faz. Na Etapa 5 o nó prova o vínculo entre as duas pelo canal cifrado |
| `TIMESTAMP` | Início e fim da execução |
| `STATUS` | Estado atual do ciclo. Fica **fora** da assinatura do registro, porque muda depois da entrega |
| assinatura | Ed25519 do worker sobre os campos acima (menos `STATUS`), para o resultado ser atribuível |
| veredito | Segunda assinatura, sobre o hash do registro e o estado final. Trocar `REJECTED` por `SETTLED` no arquivo quebra essa assinatura |

`hyurax-no ultrax auditar` confere cada linha do histórico:

- refaz o `TASK_ID`;
- exige o worker deste nó e as duas assinaturas;
- confere as operações contra o modelo de custo;
- confere se a tarefa marcada como desafio é mesmo um desafio do gabarito;
- procura tarefa liquidada duas vezes;
- com o histórico inteiro, compara o placar;
- refaz do zero uma amostra de tarefas liquidadas.

**O limite dela, dito na própria saída:** quem tem o `no.chave` assina
qualquer coisa. A auditoria prova que o histórico é coerente, não que o dono
é honesto. Isso só se resolve com verificação por outros nós (Etapa 5).

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
| **LAB** | Geradas nesta máquina | Pela própria máquina. Freivalds usa outra conta, então pega também defeito que se repete na multiplicação. A recomputação roda o mesmo código de novo e só pega erro passageiro. O erro que se repete, quem pega são as tarefas-desafio | Etapa 2 |
| **TESTNET** | Trocadas entre nós da rede de teste | Por outros nós; redundância entre workers diferentes | Etapa 5 |
| **REAL** | De clientes externos | Paga pelo cliente (spec §18) | Só com cliente de verdade |

## 4. Tipos de trabalho

| Tipo | Primeiro trabalho | Verificação | Etapa |
|---|---|---|---|
| Matemático | Multiplicação de matrizes | Freivalds, O(n²), erro ≤ 2⁻⁸⁰ | 1 |
| Matemático | Difusão em ponto fixo | Recomputação | 1 |
| Otimização | Mochila 0/1, exigindo o ótimo | Recomputação da programação dinâmica | 1 |
| IA | Rede neural pequena, em inteiros de ponto fixo, prevendo solubilidade de moléculas reais (AqSolDB, domínio público CC0) | Recomputação da inferência; Freivalds nas camadas | 4 |
| Verificação | Tarefas-desafio: casos de `vectors/utrax.json`, com a resposta calculada pelo gabarito em Python, fora desta máquina. Uma a cada 10 tarefas | Resultado esperado | 2 |
| Verificação | Reconferir provas de trabalho útil dos blocos já guardados | Resultado esperado: a cadeia já foi aceita | depois |
| Rede | Latência e disponibilidade medidas contra os pares reais | Medida direta, sem prova forte (dito na tela) | 5, porque precisa de pares de verdade |
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
| 2 | Worker LAB no `hyurax-no`: gerador de tarefas, fila, execução com limite de CPU e memória, prazo e cancelamento, tarefas-desafio, histórico em disco, telemetria com modo DEBUG, auditoria. **Pronta em 26/09/2026** | O programa executa e verifica tarefas sozinho, respeitando os limites, e o histórico sobrevive a reiniciar |
| 3 | Painel ULTRAX no lugar da estação 3D: trabalho ativo, histórico, verificação, contribuição, nó e o selo do modo. **Pronta em 26/09/2026** | Um usuário responde às 7 perguntas do critério de sucesso olhando a tela |
| 4 | Trabalho de IA: rede pequena em ponto fixo sobre a AqSolDB, com a molécula desenhada na tela. **Pronta em 26/09/2026** | Inferência verificada por recomputação; a curva de erro na tela sai do cálculo real |
| 5 | TESTNET: mensagens de tarefa e resultado entre nós, redundância com workers independentes, reputação por `WORKER_ID` | Três nós separados executam a mesma tarefa, e um resultado adulterado é detectado pela maioria |
| 6 | GPU (depois de medir se compensa), sandbox para trabalho externo, estrutura de JOB/cliente | Só com hardware para testar e com decisão sobre o equilíbrio CPU × GPU |

### Medido na Etapa 2 (Atom x5-Z8350, uma linha, versão otimizada)

| Tarefa | Operações | Cálculo |
|---|---|---|
| Matriz 1024 × 1024 | 1,07 bilhão | 4,0 a 4,6 s |
| Difusão 256 × 256, 1024 passos | 67 milhões | 1,8 a 2,5 s |
| Mochila 256 itens | 1,6 milhão | 0,02 s |

- **Limite de CPU em 50%:** a matriz levou 0,66 s de cálculo em 1,35 s de
  relógio.
- **12 tarefas seguidas:** as 12 foram liquidadas, e a tarefa-desafio passou.
- **Auditoria:** conferiu os 13 registros (as 12 tarefas e uma que ficou na
  fila e foi cancelada no fim). Refez do zero 4 tarefas sorteadas, com o mesmo
  resultado.
- **A mochila é pequena de propósito.** O gabarito limita a 256 itens, porque a
  escolha cabe numa máscara de 32 bytes. Ela passa do limite só numa versão
  nova do formato.

### A tela (Etapa 3), pergunta por pergunta

| Pergunta | Onde a tela responde |
|---|---|
| O que meu computador está fazendo? | Cartão da tarefa ativa: categoria (MATHEMATICAL WORK, OPTIMIZATION WORK), trabalho e tamanho |
| Qual tarefa recebeu? | Número da tarefa, `TASK_ID` e `INPUT_HASH` no cartão |
| Quanto trabalho executou? | Progresso em unidades reais: linhas de C, itens da mochila, passos da difusão, rodadas de Freivalds; e as operações |
| O resultado foi verificado? | Ciclo de vida no cartão (o estado atual invertido), método e quem confere; histórico com o estado final |
| Como a contribuição foi medida? | Work Score com a regra escrita embaixo: 1 ponto = 1 milhão de operações verificadas |
| Qual a reputação do nó? | Nota de 0 a 1000 com a fórmula, e o `WORKER_ID` |
| Em qual ambiente estou? | Selo `LAB · TESTNET WORKLOAD` fixo no cabeçalho do painel |

O progresso vem da contagem do worker, lida a cada segundo; nada anda
sozinho na tela. GPU aparece como "não usada" e temperatura como "não
medida", porque o programa não mede nenhuma das duas. A mineração ganhou
painel próprio, e o botão ULTRAX fica ao lado do MINERAR: são duas coisas
diferentes, e podem rodar juntas.

### O trabalho de IA (Etapa 4)

- **A base:** 2.048 moléculas da AqSolDB (CC0), filtradas e preparadas por
  `reference/tools/preparar_moleculas.py`, e embutidas no programa
  (300 KB).
- **O modelo:** rede de 10 descritores → 16 neurônios → 1 saída, em inteiros
  Q12. A especificação é `reference/hyurax/ia.py`; o Rust
  (`hyurax-ultrax/src/ia.rs`) bate byte a byte com `vectors/ia.json`.
- **A tarefa:** um treino de `passos` passos com lotes de 32. A taxa de
  aprendizado cai em três degraus (1/64, 1/128, 1/256). Com taxa fixa, o treino
  longo não assentava: o erro de validação ia de 0,31 para 2,1. O resultado é
  a conta dos pesos finais mais o erro de validação, e a conferência é por
  recomputação.
- **O que sai:** o melhor modelo desta máquina erra, em média, cerca de ±1,2
  em log S nas 410 moléculas que ficaram fora do treino. Modelos simples
  publicados sobre essa base ficam na mesma faixa. A tela mostra uma dessas
  410 a cada 8 segundos, com a estrutura desenhada a partir do SMILES, a
  solubilidade medida e a prevista.
- **O que não é:** prever solubilidade é uma etapa de triagem na pesquisa de
  remédios. Não é descoberta de remédio, e a tela diz isso.

### Limites conhecidos, para resolver antes da Etapa 5

- **A nota de reputação não pesa divergência nem tamanho.** Um worker que
  diverge sempre da maioria mantém a nota, e mil tarefas pequenas diluem uma
  recusa grande. No LAB isso não importa, porque ninguém escolhe worker pela
  nota. Na TESTNET importa, e a regra ganha versão nova.
- **O placar (`placar.txt`) não é assinado.** A auditoria o compara com o
  histórico enquanto o histórico está inteiro.

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
