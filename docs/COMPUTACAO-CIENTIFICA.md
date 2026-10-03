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
| GPU (integrada inclusive) com limitador, conferida por Freivalds; calcula unidades de JOB de matriz | sim | `hyurax-interface/ui/js/gpu/`, `/api/v1/gpu/*` |
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
| Molecular | **Triagem** de catálogo de moléculas reais: filtros de regra (Lipinski e afins, pelos descritores do catálogo) e solubilidade prevista pelo modelo de referência (erro de 1,233 log S nas 6.241 moléculas que o treino nunca viu, contra 2,262 de quem prevê a média) | implementado (`triagem.py`, `triagem.rs`, `vectors/triagem.json`; 8.289 moléculas) |
| Genética / biologia | **Genética de populações** Wright-Fisher: deriva, seleção, dominância, mutação; réplicas independentes | implementado (`genetica.py`, `genetica.rs`, `vectors/genetica.json`) |
| Agricultura | **Melhoramento de culturas**: QTL aditivos, ambiente (água, nitrogênio, solo) pela lei do mínimo com interação G×E, seleção truncada e cruzamento | implementado (`melhoramento.py`, `melhoramento.rs`, `vectors/melhoramento.json`) |
| Logística | **Rotas** (caixeiro-viajante): 2-opt a partir de partidas diferentes; a verificação confere a rota e o custo em O(n) | implementado (`rotas.py`, `rotas.rs`, `vectors/rotas.json`) |
| Economia / alocação | Mochila 0/1 (ótimo exato) | existe |
| IA / ML | Treino da rede de solubilidade; inferência em lote dentro da triagem | existe |
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
- `jobs/<JOB_ID>/checkpoint.bin` (e `checkpoint.anterior`): intervalos,
  agregador, tentativas e consumo (os créditos do JOB), com SHA-512 de
  integridade;
- `jobs/<JOB_ID>/eventos.jsonl`: só-acréscimo, é o que permite a
  reprodução;
- `jobs/<JOB_ID>/unidades.jsonl`: uma linha por unidade conferida, com os
  hashes e o tempo; é dela que sai o "reproduzir" da tela;
- `jobs/<JOB_ID>/relatorio.{json,csv,pdf}`;
- `benchmarks.jsonl`.

A reputação por WORKER_ID fica **na memória** do nó que pediu, enquanto ele
roda: é o que este nó viu, não consenso de rede, e recomeça do zero ao
reabrir. Guardar em disco fica para depois da testnet.

Usuários, créditos e transações **na cadeia** continuam sendo da cadeia. O
programa não tem cadastro de usuário: o pedido de "users" vira "quem enviou o
JOB", pela chave.

### 2.8 Eventos, telemetria e a visão 3D

Na 1.0 o caminho é o descrito em [`ARQUITETURA.md`](ARQUITETURA.md):

1. **motor:** cada motor tem `executar_observado`, que entrega amostras do
   próprio estado (uma linha de C, a grade da difusão, a rota do 2-opt…)
   sem mudar o resultado;
2. **eventos:** cada mudança de unidade vira uma linha com `job_id`,
   `work_unit_id`, `node_id`, `workload_type`, `operation`, `timestamp`,
   `progress`, `input_hash`, `result_hash`, `execution_time_ms`, `cpu_usage`,
   `ram_bytes`, `throughput` e `verification_status`;
3. **fluxo:** a tela recebe tudo pelo SSE `/api/v1/fluxo` (eventos `ciencia`,
   `tarefa` e `amostra`); `/api/v1/ciencia/eventos?desde=N` continua para
   quem lê por consulta;
4. **cena 3D:** WebGL 2 próprio, sem biblioteca, um visualizador por tipo de
   trabalho, alimentado só pelas amostras;
5. **tela:** a lista de JOBs, o detalhe (mapa das unidades, agregado, figura
   do resultado) e os eventos.

A velocidade é a do cálculo: sem amostra nova, a cena fica parada.

**Medidas da máquina:** o uso da CPU e da GPU (3D e cálculo) e a memória da
GPU vêm dos contadores do Windows, para a máquina inteira e para o programa
(REAL). O WebGL não informa temperatura, clock nem energia: a temperatura fica
PENDENTE, e a energia é ESTIMADA a partir do uso medido e dos watts que o dono
informa.

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
  reais. `crates/hyurax-nucleo/tests/tres_nos.rs` sobe três núcleos
  completos (identidade, cadeia e pasta próprias) ligados pela rede P2P de
  verdade em 127.0.0.1, roda um JOB de nível 3 e derruba um nó no meio de
  outro. Entre máquinas diferentes (latência, perda, NAT), fica para a
  testnet pública.
- **Reputação local.** Gravada em disco desde 03/10/2026 (ver
  [REPUTACAO.md](REPUTACAO.md)), mas cada nó tem a sua.
- **Bytes pela rede não medidos.** A tela conta mensagens, não bytes.
- **Medidas de uma rodada só.** Nesta máquina, o mesmo motor varia de
  0,7× a 1,3× entre duas rodadas sem mudança de código. Ganho abaixo
  disso é ruído.
- **Escala:** a arquitetura descreve bilhões de unidades sem materializá-las,
  e o teste de estresse mede o agendador com milhões. Calcular um bilhão de
  unidades de verdade num Atom não cabe no tempo, e o documento não vai dizer
  que coube.
- **GPU:** só WebGL2, só matriz. Os outros motores são CPU.
- **PDF:** texto simples, sem gráficos.

## 5. Estado em 27/09/2026 (medido)

Máquina: Intel Atom, 4 núcleos lógicos, 3,4 GB de RAM, projeto num HD
externo USB. Programa em release.

**Etapas:** A, B, C, D e E prontas, com testes. F: revisão, medições e este
documento.

**Testes:**

- Python: 177;
- Rust: `hyurax-ultrax` 85, `hyurax-no` 80, contra os vetores;
- clippy sem avisos.

**Verificação entre nós.** Testes com workers simulados, usando chaves de
verdade:

| Caso | O que acontece |
|---|---|
| 3 honestos | CONSENSUS 3/3 |
| 1 adultera o resultado | CONSENSUS 2/3; ele fica com 1 divergência |
| 2 combinam o mesmo resultado falso | é maioria, mas a conferência daqui recusa; a unidade volta para a fila, os dois levam recusa, e a cópia honesta não é punida |
| Revela algo diferente do compromisso | falha e recusa |

**Ensaio com 3 processos** (dois workers aceitando trabalho e um nó
pedindo). JOB de genética com 30 unidades, nível 3, redundância 3:

- 30/30 unidades em CONSENSUS 3/3, em cerca de 15 s depois que os nós se
  acharam (a descoberta leva uns 10 s);
- 124 mensagens recebidas e 120 enviadas pelo nó que pediu.

**ULTRA BENCHMARK.** A base é a primeira medida em release; o atual é
depois da troca da espera fixa por sinal na fila:

| Métrica | Base | Atual | Ganho |
|---|---|---|---|
| Unidades verificadas por segundo, ponta a ponta (JOB de 120 unidades, matriz 48) | 5,0 | 77,3 | **15,6×** |
| Agendador: unidades marcadas por segundo (5 milhões fora de ordem) | 2,33 mi | 2,36 mi | 1,01× |
| Agendador: memória de pico por milhão de unidades | 56,6 kB | 56,6 kB | 1,00× |
| Derivação de unidades por segundo (JOB de 2^40) | 253 mil | 255 mil | 1,01× |
| Matriz na CPU, todas as linhas | 497 mi ops/s | 668 mi ops/s | 1,35× (dentro do ruído) |

**O gargalo ponta a ponta:**

- **Causa:** o gerador repunha a fila uma vez por segundo, com no máximo
  `linhas + 1` tarefas, e a linha sem tarefa dormia 200 ms. Com unidade
  curta, a máquina ficava parada esperando o relógio.
- **Correção:** a fila acorda a linha, e cada tarefa pega ou terminada
  acorda o gerador.
- **O que não mudou:** a conta dos motores. Nenhum motor ficou 10× mais
  rápido, e a tabela mostra isso.

**Resultados científicos** dos JOBs rodados. Cada unidade foi refeita bit a
bit por `hyurax-no ciencia refazer`, e a auditoria está limpa.

- **Genética:** a frequência de A vai de 0,10 a cerca de 0,21–0,23 em 200
  gerações, com s = 0,01. É coerente com a conta determinística (≈ 0,23).
- **Melhoramento:** a variância genética cai com a seleção, como se
  espera.
- **Rotas:** melhor rota de 115.237.
- **Triagem:** 8.289 moléculas, 5.621 passam no filtro.
  - A de nota mais alta, nitreto de silício, é inorgânica, e o modelo erra
    feio nela: previsão de +2,96 contra −5,67 medido. O modelo foi treinado
    em moléculas orgânicas, e isso fica escrito no relatório.
  - Marcar o que está fora do domínio do modelo é a melhoria seguinte.

