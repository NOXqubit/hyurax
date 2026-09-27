# Hyurax — Rede de computação científica verificável

Documento de desenho da expansão do ULTRAX pedida em 27/09/2026 ("HYURAX —
ULTRA CODE MASTER IMPLEMENTATION"). Ele continua o [`ULTRAX.md`](ULTRAX.md):
a Etapa 5 (tarefas entre nós) e a Etapa 6 (JOB e cliente) daquele plano são o
esqueleto do que foi pedido aqui.

**Regra que manda em tudo:** nada inventado na tela. Cada número, cada peça
da visão 3D e cada linha de relatório sai de um cálculo que aconteceu, com
identificador e hash. O que ainda não existe aparece como **"não
implementado"**, ou como **EXPERIMENTAL** com o limite escrito, e nunca como
dado.

Backup antes da mudança: branch `backup/antes-da-computacao-cientifica`
(commit `2e16660`). Os dados de quem já usa o programa são migrados com cópia
antes (ver "Armazenamento").

---

## 1. Auditoria: o que já existe e é reaproveitado

| Pedido | Já existe | Onde |
|---|---|---|
| Worker CPU com limite de CPU, RAM, prazo e cancelar | sim | `hyurax-no/src/ultrax.rs` |
| GPU (integrada inclusive) com limitador, conferida por Freivalds | sim | `painel/gpu.js`, `gpu-trabalhador.js`, `/api/ultrax/gpu/*` |
| Tipos de trabalho determinísticos, em inteiros | matriz, mochila, difusão, IA | `hyurax-ultrax/src/trabalho.rs`, `ia.rs` |
| Ciclo de vida com transições permitidas | sim | `tarefa.rs` |
| Registro de prova assinado (TASK_ID, INPUT_HASH, RESULT_HASH, WORKER_ID) | sim | `prova.rs` |
| Verificação: Freivalds, recomputação, resultado esperado | sim | `trabalho.rs` |
| Validador por maioria e por compromisso | sim, **sem rede** | `validador.rs` |
| Reputação e Work Score | sim | `reputacao.rs`, `pontuacao.rs` |
| Histórico assinado, auditoria, telemetria | sim | `ultrax.rs` (`historico.txt`, `auditar`) |
| Rede P2P cifrada, pares, mensagens tipadas que o nó antigo ignora | sim | `hyurax-net`, `hyurax-wire` (`Message::Desconhecida`) |
| Carteira, cadeia, mineração | sim, **não muda** | `hyurax-no`, núcleo |
| Moléculas reais e modelo de solubilidade | 2.048 da AqSolDB, rede 10→16→1 | `dados/moleculas.tsv`, `ia.rs` |
| Banco de dados | não há SQL; arquivos só-acréscimo | `hyurax-store`, `PASTA/ultrax/` |
| Job, unidades de trabalho, checkpoint, relatório, créditos, eventos, 3D | **não existe** | este documento |

**Conclusão:** não se refaz nada. Os pedaços novos são quatro:

1. os motores científicos, como tipos de trabalho novos do mesmo `trabalho.rs`;
2. a camada de JOB, que divide, consolida, guarda progresso e gera relatório;
3. o transporte de unidades entre nós, a Etapa 5;
4. a tela: o fluxo de eventos, a visão 3D, o painel técnico e o marketplace.

## 2. Decisões

### 2.1 Motores só compilados no programa (a sandbox)

Nenhum nó executa código que veio de fora. Um JOB escolhe um motor
compilado e passa **parâmetros validados**: faixas conferidas, custo e
memória calculados antes de rodar.

Por isso não existe código remoto para prender numa sandbox: a sandbox é o
próprio desenho. Os limites que já existem valem para cada unidade:

- linhas de CPU e fração de uso;
- teto de RAM reservado antes de começar;
- prazo;
- botão de parar;
- limitador da GPU.

Trabalho com código de fora (WASM, contêiner) é a Etapa 6 e continua **não
implementado**.

### 2.2 Domínios

| Domínio | Motor | Estado |
|---|---|---|
| Molecular | **Triagem** de catálogo de moléculas reais: filtros de regra (Lipinski e afins, pelos descritores do catálogo) e solubilidade prevista pelo modelo de referência | a implementar (Etapa A) |
| Genética / biologia | **Genética de populações** Wright-Fisher: deriva, seleção, dominância, mutação; réplicas independentes | a implementar (Etapa A) |
| Agricultura | **Melhoramento de culturas**: QTL aditivos, ambiente (água, nitrogênio, solo) pela lei do mínimo com interação G×E, seleção truncada e cruzamento | implementado (`melhoramento.py`, `melhoramento.rs`, `vectors/melhoramento.json`) |
| Logística | **Rotas** (caixeiro-viajante): 2-opt a partir de partidas diferentes; a verificação confere a rota e o custo em O(n) | a implementar (Etapa A) |
| Economia / alocação | Mochila 0/1 (ótimo exato) | existe |
| IA / ML | Treino da rede de solubilidade; inferência em lote dentro da triagem | existe / Etapa A |
| Matemática / física | Matrizes (CPU e GPU), difusão de calor | existe |
| Materiais, energia, meio ambiente | — | **não implementado**: o JOB é recusado com "domínio sem motor" |
| Regeneração de tecidos | — | **não implementado**: não há modelo biológico no programa. O caminho de hipóteses, simulação, ranking e relatório é o do JOB, e vale para qualquer motor que um dia exista |

O que cada motor **não** é fica escrito no relatório:

- A **triagem** usa descritores calculados de moléculas reais do catálogo e
  um modelo de erro típico de ±1,25 log S. Ela não calcula afinidade com
  alvo, estabilidade nem toxicidade, porque não há modelo para isso no
  programa. Pedido dessas propriedades é **recusado**, e não estimado.
- **Genética e melhoramento** são modelos estatísticos com os parâmetros de
  quem pediu. Não preveem safra real nem o comportamento de organismo real.
  Não há protocolo de laboratório: é simulação e análise de dados.
- **Nenhum** resultado é "cura", "descoberta" ou "verdade". O rótulo é sempre
  "resultado computacional, que precisa de validação científica".

### 2.3 Parâmetros na especificação

A `Especificacao` ganha até `PARAMETROS_MAX` = 12 inteiros `u32`, com sentido
definido por tipo.

- Os tipos antigos continuam sem parâmetros e com a **mesma codificação**:
  `u8 tipo || u32 tamanho || u32 passos`. TASK_ID, histórico e vetores não
  mudam.
- Os tipos novos acrescentam `u8 n || n × u32`.

O parâmetro de cada motor está no módulo dele e no gabarito em Python.

| Código | Tipo | `tamanho` | `passos` | parâmetros |
|---|---|---|---|---|
| 5 | Genética | indivíduos diploides N | gerações | loci, mutação (por 10⁶), seleção (+10⁴, deslocada), dominância (%), frequência inicial (por 10⁴) |
| 6 | Melhoramento | indivíduos N | gerações | QTL, arquitetura (sorteia os efeitos, igual em todo o JOB), % selecionada, ruído ambiental (% do desvio genético), água, nitrogênio, solo, G×E |
| 7 | Rotas | cidades n | teto de passadas do 2-opt | instância |
| 8 | Triagem | moléculas da faixa | 0 | catálogo, início da faixa, limites dos filtros |

### 2.4 Regra do projeto mantida

"Python especifica, Rust executa, vetores provam."

- Cada motor novo nasce em `reference/hyurax/<motor>.py`, com testes em
  `reference/tests/` e vetores em `vectors/<motor>.json`.
- O Rust bate byte a byte.
- Tudo em **aritmética inteira**: dois nós honestos chegam no mesmo resultado
  bit a bit, e é isso que torna possíveis a recomputação e a concordância
  entre nós. Ponto flutuante só na hora de **mostrar** (relatório e tela),
  nunca no que é conferido.

### 2.5 JOB e unidades de trabalho, em escala

Um JOB tem:

- domínio e motor;
- especificação-modelo;
- número de unidades (`u64`);
- nível de verificação;
- prazo;
- orçamento em créditos;
- descrição.

O `JOB_ID` é o hash da codificação canônica do JOB.

A unidade `i` **não é guardada**: ela se deriva na hora.

- A especificação vem do modelo, com o que muda por unidade. Na triagem, por
  exemplo, o início da faixa é `i × tamanho`.
- A semente é `H(dominio || JOB_ID || u64 i)`.

Um JOB de um bilhão de unidades custa o mesmo que um de dez para ser
descrito. O progresso é guardado em três peças:

- **o conjunto de intervalos concluídos:** `[a, b)` fundidos, memória
  proporcional aos buracos, não ao total;
- **o agregador do motor, com memória constante:** melhores K da triagem,
  somas e somas de quadrados das réplicas, melhor rota. A codificação
  canônica tem hash, e o resultado consolidado não depende da ordem em que
  as unidades chegaram;
- **as tentativas pendentes:** repetição com limite, e divergência reenviada
  a outro nó.

Checkpoint em disco a cada lote e no fechar. Ao reabrir, o JOB continua de
onde parou.

### 2.6 Verificação em cinco níveis

| Nível | O que é | Como |
|---|---|---|
| 1 | Integridade | RESULT_HASH confere com os bytes; registro assinado confere com o WORKER_ID |
| 2 | Reexecução | Recomputação, ou Freivalds, pelo nó que pediu, antes de aceitar |
| 3 | Concordância | A mesma unidade em N workers **de nós diferentes**, maioria por RESULT_HASH (`validador::por_maioria`). Três linhas da mesma máquina **não** contam |
| 4 | Verificação independente | Algoritmo diferente (Freivalds em vez de refazer; CPU conferindo GPU), ou um nó verificador que não produziu o resultado |
| 5 | Reprodutibilidade | O relatório leva versão do programa, motor, parâmetros, sementes, hashes de entrada e resultado, e o comando que refaz a unidade |

Consenso computacional quer dizer "os nós chegaram ao mesmo número". Não
quer dizer que o modelo está certo sobre o mundo, e o relatório diz isso.

### 2.7 Armazenamento: arquivos versionados, sem SQL

Não entra banco SQL.

- O SQLite exige compilador C, e o núcleo é Rust puro.
- O projeto já guarda tudo em arquivos só-acréscimo.

A pasta `PASTA/ciencia/` tem um `VERSAO` de esquema e migrações numeradas.
Antes de migrar, a versão anterior é copiada para `PASTA/ciencia/backup-vN/`.

Conteúdo da pasta:

- `jobs/<JOB_ID>/job.bin`: a especificação;
- `jobs/<JOB_ID>/checkpoint.bin`: intervalos, agregador e tentativas;
- `jobs/<JOB_ID>/eventos.jsonl`: só-acréscimo, é o que permite a
  reprodução;
- `jobs/<JOB_ID>/relatorio.{json,csv,pdf}`;
- `creditos.jsonl`;
- `benchmarks.jsonl`;
- `nos.txt`: reputação por WORKER_ID.

Usuários, créditos e transações **na cadeia** continuam sendo da cadeia. O
programa não tem cadastro de usuário: o pedido de "users" vira "quem enviou o
JOB", pela chave.

### 2.8 Eventos, telemetria e a visão 3D

O caminho:

1. **motor:** cada pedaço executado chama `continuar(ops)`, como já é hoje;
2. **evento:** cada mudança vira uma linha com `job_id`, `work_unit_id`,
   `node_id`, `workload_type`, `operation`, `timestamp`, `progress`,
   `input_hash`, `result_hash`, `execution_time_ms`, `cpu_*`, `gpu_*`,
   `ram_*`, `throughput` e `verification`;
3. **fluxo:** a tela lê por `/api/ciencia/eventos?desde=N`;
4. **visão 3D:** WebGL2 próprio, sem biblioteca;
5. **tela.**

O que a visão 3D mostra, por tipo:

| Tipo | Representação |
|---|---|
| Matriz | Os blocos de linhas realmente feitos, na ordem em que ficaram prontos (CPU ou GPU) |
| Triagem | As moléculas da faixa em execução, desenhadas do SMILES real, e a nota que cada uma recebeu |
| Genética | As frequências alélicas de cada geração já calculada |
| Rotas | A rota atual e o custo, quando a unidade entrega |
| Nós | Este nó e os pares **conectados de verdade**, com os bytes medidos |

A velocidade é a dos eventos: se o cálculo está lento, a tela está lenta.
"Inspecionar cálculo" mostra os campos do evento, não uma descrição.

**O que o navegador embutido não entrega, e fica escrito na tela:**

- **Uso da GPU:** o WebGL não informa o uso da placa inteira, nem a
  temperatura, o clock, a energia ou a VRAM livre.
  - O que é medido é o **ciclo de trabalho da GPU pelo ULTRAX**: tempo de
    conta dividido pelo tempo total.
  - A VRAM mostrada é a **alocada pelo ULTRAX**, os bytes das texturas.
  - O resto aparece como "indisponível pelo WebGL".
- **Uso de CPU:** o do ULTRAX, medido pelo próprio limitador. Não é o da
  máquina inteira.

### 2.9 Créditos de computação, separados do HYX

A contabilidade é por unidade conferida:

- operações;
- tempo de CPU e de GPU;
- memória reservada × tempo;
- bytes pela rede;
- verificação.

A conversão em **créditos de computação** segue uma fórmula publicada,
versão 1, e fica em `creditos.jsonl`.

A liquidação é uma interface (`Liquidacao`) cuja única implementação hoje é
**"não liquida"**. Crédito não vira HYX, pela mesma regra do Work Score:
recompensa só com regra econômica publicada. As três camadas (computação,
contabilidade e token) ficam em módulos separados.

### 2.10 ULTRA BENCHMARK

"10×" não é enfeite. A suíte mede:

- vazão de unidades verificadas por segundo;
- unidades concorrentes;
- capacidade do agendador (unidades despachadas por segundo, e memória por
  milhão de unidades);
- vazão de verificação;
- vazão de GPU e de CPU por motor;
- nós simultâneos.

Cada linha tem **BASE**, **ATUAL**, **ALVO** e **GANHO REAL**. A base é a
medida registrada da versão anterior. Se o ganho for 2×, aparece 2×.

## 3. Etapas

Cada etapa termina com testes, clippy limpo e commit.

| Etapa | Entrega |
|---|---|
| A | Motores: genética, melhoramento, rotas e triagem. Python, vetores, Rust e testes, com o esqueleto comum pronto antes, para os quatro não se atropelarem |
| B | JOB: especificação, `JOB_ID`, derivação de unidades, intervalos, agregadores, checkpoint, créditos v1, relatório (JSON, CSV e PDF) e interfaces de domínio (inclusive os sem motor) |
| C | Agendador no nó: fila de JOBs, validação, estimativa, despacho para CPU e GPU, repetição, retomada, cancelamento, eventos, migração da pasta, benchmark e API |
| D | Rede (Etapa 5 do ULTRAX): mensagens de oferta, unidade e resultado entre nós, redundância com workers de nós diferentes e reputação por WORKER_ID. Teste com três processos |
| E | Tela: fluxo de eventos, visão 3D, "inspecionar cálculo", painel técnico, marketplace local, reprodução de JOB passado |
| F | Revisão adversarial, testes de falha e de escala, medições e documento final |

## 4. Limites que já se sabem

- **Uma máquina, até agora.** A redundância entre nós só é real com nós
  reais. O teste da Etapa D usa três processos com identidades separadas
  numa máquina. Entre máquinas diferentes, fica para a testnet pública.
- **Escala:** a arquitetura descreve bilhões de unidades sem materializá-las,
  e o teste de estresse mede o agendador com milhões. Calcular um bilhão de
  unidades de verdade num Atom não cabe no tempo, e o documento não vai dizer
  que coube.
- **GPU:** só WebGL2, só matriz. Os outros motores são CPU.
- **PDF:** texto simples, sem gráficos.
