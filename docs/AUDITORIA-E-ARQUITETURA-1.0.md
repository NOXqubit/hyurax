# Hyurax / ULTRAX 1.0: auditoria e arquitetura

**27/09/2026.** Levantamento feito no código do branch
`claude/aurora-project-setup-d7eb67` (commit `5829591`). Nenhum código foi
alterado nesta etapa.

**Rótulos usados neste documento:**

| Rótulo | Significado |
|---|---|
| **REAL** | Medido ou calculado de verdade, agora. |
| **DERIVADO** | Conta exata sobre um dado real. |
| **ESTIMADO** | Fórmula com um parâmetro que ninguém mediu. |
| **SIMULADO** | Cálculo de verdade sobre uma entrada inventada. |
| **MOCK** | Valor de enfeite. |
| **PENDENTE** | Não existe ainda. |

---

## Parte 1 — O WebGL 2 hoje

O programa cria **dois** contextos WebGL 2, com funções que não têm nada a
ver uma com a outra. O resto dos gráficos é canvas 2D.

| | Onde | Função |
|---|---|---|
| **A** | `painel/gpu.js` e `painel/gpu-trabalhador.js` | **Computação.** Um canvas invisível de 1×1 serve só para a GPU calcular. Não desenha nada na tela. |
| **B** | `painel/ciencia.js:464-710` | **Desenho.** A "Visão 3D" da seção Ciência. |

### As oito perguntas

**1. O que o WebGL 2 renderiza?**

- **A:** nada visível. O shader de fragmento (`gpu.js:24-39`) calcula
  `C = A·B` em inteiros de 32 bits: cada pixel de uma textura `R32UI` é uma
  entrada de C. O resultado é lido de volta com `readPixels`.
- **B:** cubos. Cada cubo é uma unidade de JOB, ou um grupo delas quando
  passa de 4.096 (`estadosDasUnidades`, `ciencia.js:573`).
  - A altura é a fração de operações feitas; a cor é o estado da unidade.
  - Um cubo branco representa este nó, e cubos cinza em círculo representam
    os pares.
  - Pontos que viajam do nó até um cubo marcam unidades despachadas.

**2. De onde vêm os dados?**

- **A:** as matrizes vêm do núcleo em Rust
  (`/api/ultrax/gpu/entrada`, `ultrax.rs:1681`). O resultado volta por
  `/api/ultrax/gpu/resultado`, e a CPU confere por Freivalds antes de
  creditar (`ultrax.rs:1705` → `concluir`).
- **B:** três fontes, lidas por consulta periódica, não por aviso:
  - `/api/ciencia/job/<id>` a cada 1,5 s: faixas feitas, faixas com falha e
    unidades em voo;
  - `/api/ciencia/eventos` a cada 1 s: eventos de progresso, verificação e
    despacho;
  - `/api/estado` a cada 1 s: o número de pares.
- Na reprodução, os dados são os horários gravados em `unidades.jsonl`.

**3. Os dados vêm do workload real?**

- **A:** sim, mas de um workload de **laboratório**.
  - A tarefa da GPU é sempre uma matriz com entrada sorteada
    (`Instancia::PorWorker`, com entropia, `ultrax.rs:1629`).
  - A GPU **nunca** calcula uma unidade de JOB científico, nem a prova de
    trabalho útil do bloco.
  - A conta é real e conferida, mas a entrada é sintética.
- **B:** sim, no nível de **controle**: quantas operações de cada unidade o
  motor já contou (pelo `continuar(ops)`), e se a unidade está na fila,
  rodando, conferindo, feita ou com falha.
  - **Não** no nível do **conteúdo**: a cena não mostra nenhum dado do
    cálculo (a molécula, a matriz, a população, a rota).
  - Os motores nem expõem esse conteúdo durante a execução: a única saída
    intermediária deles é um contador de operações.

**4. A cena representa uma computação que existe, ou é animação?**

- Representa uma computação que existe, mas só como um **quadro de
  progresso em 3D**. É um gráfico de barras com câmera, não uma
  visualização do workload.
- Uma triagem de moléculas e uma simulação genética aparecem iguais na
  tela: cubos subindo.
- É exatamente o problema que você apontou: não é falso, mas não mostra o
  trabalho.

**5. Há sincronização entre o cálculo e a cena?**

- **Parcial, e com atraso.** O motor atualiza um contador atômico. O núcleo
  lê esse contador e gera um evento de progresso só quando o valor muda. A
  tela pergunta a cada 1 s. Entre o motor e o cubo passam de 1 a 2
  segundos.
- **Não há vínculo com a prova do bloco (UsefulPoW):** a cena não sabe que
  blocos existem.
- **Não há vínculo com a computação da GPU:** as tarefas da GPU são do
  modo LAB e não entram na cena, que só mostra unidades de JOB.

**6. O que é real e o que não é?**

| Valor | Rótulo | Detalhe |
|---|---|---|
| Estado de cada unidade (fila, rodando, conferindo, feita, falha) | REAL | eventos do núcleo |
| Altura do cubo (progresso) | REAL | operações contadas pelo motor ÷ total |
| Reprodução de um JOB passado | REAL | horários gravados |
| Número de cubos de pares | REAL | conexões abertas |
| **Posição** dos pares no círculo | enfeite de layout | não significa nada |
| Ponto viajando do nó ao cubo | efeito sobre evento real | o despacho aconteceu; a viagem de 700 ms é desenho, e o despacho foi **local**, não pela rede |
| Uso de GPU "x% (limite)" | REAL como ajuste | a tela diz "limite" |
| Ritmo da GPU (M ops/s) | REAL | medido nas tarefas da GPU |
| **CPU "medido na última unidade: x%"** (`ciencia.js:317`) | **FALSO** | o valor é o **limite configurado** (`ciencia.rs:704`), não uma medida. **Defeito a corrigir.** |
| Energia em watts | ESTIMADO | 12 W por núcleo **por padrão** (`painel.rs:114`), vezes as linhas; a tela escreve "(estimativa)" |
| Moléculas triadas | DERIVADO exato | operações ÷ 183. A tela repete a constante do motor (`ciencia.js:21`), o que é acoplamento frágil |
| RAM | REAL, mas é a **reservada pelo ULTRAX**, não a da máquina | |
| Temperatura, VRAM | não medidos | a tela diz "não medida" |

**7. O WebGL 2 só desenha, ou também calcula?**

Os dois, em contextos separados:

- **A** calcula (a GPU faz a multiplicação de matrizes do LAB);
- **B** só desenha.
- Um não alimenta o outro: **o resultado da GPU nunca é desenhado.**

**8. Há algo fingido só para demonstrar o conceito?**

- **Fingido:** não achei animação sem evento nem número sorteado. **A única
  exceção é o rótulo "medido" da CPU**, que é falso.
- **Três coisas, porém, parecem mais do que são:**
  1. **A "Visão 3D" parece mostrar a ciência, mas mostra progresso.**
  2. **"GPU no ULTRAX" parece a GPU ajudando a ciência, mas ela só calcula
     matrizes de laboratório.** A ciência roda 100% na CPU.
  3. **O "trabalho útil" do bloco é uma matriz aleatória derivada do bloco
     anterior** (`hyurax-chain/src/lib.rs:458`). É uma prova de trabalho
     que se confere barato, mas o resultado não serve para ninguém. A
     ciência de verdade (JOBs) roda fora do consenso e não muda nada nos
     blocos.

### O papel do WebGL 2 na 1.0

**O WebGL 2 fica**, nas duas funções, com papéis formais e ligados um ao
outro.

- **Como renderizador (B):** é adequado.
  - Roda na janela que o programa já tem (WebView2) e funciona na GPU
    integrada desta máquina, que é o pior caso.
  - Instanciamento e texturas inteiras bastam para as cenas da 1.0.
  - O que muda é **o que ele recebe**: amostras do estado real de cada
    motor, e não só o progresso.
- **Como computação (A):** vira um **backend formal** do motor de
  computação, atrás da mesma interface que a CPU.
  - Passa a calcular trabalho de verdade: unidades de JOB de matriz e a
    prova útil do bloco, que também é `C = A·B`.
  - O resultado dele alimenta a cena, com a verificação mostrada em cima.

O fluxo da 1.0:

```
WORKLOAD REAL (unidade de JOB | prova do bloco | LAB rotulado)
      │
      ▼
MOTOR DE COMPUTAÇÃO ── escolhe o backend: CPU | GPU (WebGL 2) | ...
      │   └── o motor emite AMOSTRAS do estado real (Observador)
      ▼
RESULTADO ──► VERIFICAÇÃO (Freivalds | recomputação | maioria entre nós)
      │                 │
      ▼                 ▼
BARRAMENTO DE EVENTOS (sequência, origem, rótulo REAL/DERIVADO/ESTIMADO)
      │
      ▼  fluxo contínuo (SSE), não consulta a cada segundo
VISUALIZADOR do tipo de trabalho ──► WebGL 2 (só desenha)
```

---

## Parte 2 — Auditoria do resto

### Reaproveitar sem reescrever (REAL, testado contra o gabarito Python)

- **Núcleo da cadeia:**
  - `hyurax-types`, `hyurax-crypto`, `hyurax-codec`;
  - `hyurax-tx`, `hyurax-block`, `hyurax-consensus`, `hyurax-state`;
  - `hyurax-chain`, `hyurax-store`;
  - `hyurax-pow` (Argon2id) e `hyurax-usefulpow`.
  - Todos conferidos por vetor.
- **Rede:** `hyurax-wire` e `hyurax-net`.
  - Inclui TCP, Noise XX, identidade de nó, descoberta de pares,
    sincronização e reorganização.
  - Inclui também as mensagens do ULTRAX.
- **Motores:** os oito tipos de `hyurax-ultrax` (matriz, mochila, difusão,
  IA, genética, melhoramento, rotas, triagem).
  - Resultados bit a bit iguais aos do Python.
  - Mais o JOB, o agregador, a prova, a reputação e o validador.
- **Carteira:** `carteira.rs`, `envio.rs`, `endereco.rs`, `senha.rs`,
  `totp.rs` e `seguranca.rs`: carteira cifrada, envio com nonce, endereço
  Bech32m e segundo fator.
- **Computação científica:** `ciencia.rs` e `ciencia_rede.rs`, com JOBs,
  checkpoint, consenso entre nós e conferência local.
- **Outros:** o instalador e os termos, a janela (tao e wry), e o `hyurax-eter`.

### Reescrever ou reorganizar

**`painel.rs` (2.169 linhas) mistura sete coisas:**

1. o servidor HTTP escrito à mão;
2. os ajustes;
3. o **laço de mineração** (`minerador`, `uma_rodada`, linhas 805-925);
4. a criação e a importação da carteira;
5. o envio;
6. a busca de preços;
7. o JSON da tela, montado com `format!`.

A mineração e a carteira são serviços do nó; não podem morar no servidor da
tela.

**`ultrax.rs` (2.890 linhas)** junta o agendador, as linhas de CPU, o
limitador, o gerador do LAB, o **protocolo da GPU por HTTP**, o placar e a
auditoria. Vira três partes:

- o **motor de computação**, com backends;
- o **agendador**;
- o **LAB**.

**A tela:**

- `painel.js` (1.884 linhas) e `ciencia.js` (1.005 linhas) consultam o
  servidor a cada 1 a 4 segundos e montam o estado à mão.
- Na 1.0, recebem um fluxo contínuo e guardam o estado num lugar só.
- A cena 3D sai de `ciencia.js` para um módulo próprio, com um visualizador
  por tipo de trabalho.

**O JSON** é montado à mão em dezenas de lugares. Passa para um módulo só,
com escape testado e esquema versionado (`/api/v1`).

### Protótipos e remendos

| Onde | O que é | Rótulo hoje |
|---|---|---|
| Visão 3D | quadro de progresso, não do workload | REAL, mas genérico |
| GPU | só matrizes do LAB; o protocolo passa pela API da tela; se a página recarrega, a conta cai | REAL, mas desligado da ciência |
| Modo LAB | carga gerada aqui para manter o worker ocupado | SIMULADO (conta real sobre entrada sorteada), rotulado "TESTNET WORKLOAD" |
| Prova útil do bloco | matriz aleatória | REAL como prova de trabalho; não é útil |
| Energia e custo | 12 W por núcleo, fixo | ESTIMADO |
| Uso de CPU "medido" | é o limite | **FALSO** (defeito) |
| CPU, RAM e GPU da máquina | não medidos; só o que o ULTRAX reservou e o limite | PENDENTE |
| Temperatura | não medida | PENDENTE |
| Reputação dos workers | só na memória | REAL, mas se perde ao fechar |
| Bytes pela rede | não medidos | PENDENTE |
| Nós semente | nenhum no ar | PENDENTE (infraestrutura) |
| Painel "Mercado" (CoinGecko) | preço de outras moedas, alheio ao produto | REAL, mas fora do escopo |

### Acoplamentos desnecessários

1. Mineração e carteira dentro do servidor da tela.
2. A GPU agenda o próprio trabalho pela API da tela, e o núcleo depende de a
   página estar aberta e viva.
3. A tela repete constantes dos motores (as 183 operações por molécula) e
   calcula coisas que o núcleo deveria entregar prontas.
4. Os eventos da ciência (em `ciencia.rs`) e os do ULTRAX (`marcar`, em
   `ultrax.rs`) são dois sistemas de evento diferentes, com formatos
   diferentes.
5. O progresso é lido de contadores atômicos por varredura, não emitido pelo
   motor.

### Como os dados andam hoje

- **Motor → cena:** o motor chama `continuar(ops)` → um contador atômico →
  o núcleo varre as unidades ativas e gera o evento de progresso → a tela
  pergunta a cada 1 s → o cubo muda de altura. **Nenhum dado do cálculo
  chega à tela.**
- **Resultado → verificação:**
  1. `executar` devolve os bytes do resultado;
  2. `verificar` (Freivalds, recomputação ou resultado esperado) confere;
  3. o `RegistroDeProva` é assinado pela chave do worker;
  4. a unidade entra no JOB: intervalos, agregador, resumo aditivo e
     créditos;
  5. no nível 3, há maioria entre nós, conferida aqui por recomputação.
- **Resultado → rede:**
  - a prova útil do bloco entra no cabeçalho (`useful_root`) e é conferida
    por todos os nós;
  - o trabalho científico **não entra na cadeia**. Ele vira reputação,
    créditos e Work Score, que não são HYX.

---

## Parte 3 — Arquitetura da 1.0

### Princípios

1. **Nada chega à tela sem origem.** Cada número leva o rótulo REAL,
   DERIVADO, ESTIMADO, SIMULADO ou PENDENTE, e a tela mostra o rótulo.
2. **O motor diz o que está fazendo.** Cada motor emite amostras do próprio
   estado. A visualização só desenha amostras.
3. **Observar não muda o resultado.** Os vetores e os testes de
   bit a bit continuam valendo com o observador ligado ou desligado.
4. **A tela é cliente.** O núcleo funciona sem ela: nó, mineração, JOBs, rede
   e carteira.
5. **"1.0" é a versão do programa, não da rede.** A rede continua sendo de
   **teste**, e o HYX continua sem valor. A tela diz isso.

### Camadas

```
┌──────────────────────── Hyurax.exe (janela: tao + wry/WebView2) ───────────────────────┐
│  Interface (JS)                                                                          │
│   estado único ◄── fluxo /api/v1/fluxo (SSE)   comandos ──► /api/v1/* (POST, Origin local) │
│   telas: Visão geral · ULTRAX · Ciência · Carteira · Cadeia · Rede · Registro            │
│   cena 3D (WebGL 2): motor de cena + um VISUALIZADOR por tipo de trabalho               │
│   backend GPU (WebGL 2) num worker próprio, falando com o núcleo por canal binário      │
└──────────────────────────────────────────▲───────────────────────────────────────────────┘
                                           │ HTTP local (127.0.0.1), Host e Origin conferidos
┌──────────────────────────────────── núcleo (Rust) ───────────────────────────────────────┐
│ api        rotas v1, JSON com esquema e escape testado, SSE                              │
│ barramento eventos tipados + amostras; sequência; rótulo de origem; histórico em disco   │
│ servicos   nó (cadeia + mempool), mineração, carteira, ciência (JOBs), rede, ajustes     │
│ computacao motor de computação: agendador → backends {CPU, GPU-WebGL2, ...}; limites     │
│ metricas   CPU / RAM / GPU da máquina pelos contadores do Windows; energia estimada à parte │
└──────────────────────────────────────────────────────────────────────────────────────────┘
   crates reaproveitados: chain, consensus, net (Noise), usefulpow, ultrax (motores), eter …
```

### O contrato novo dos motores (em `hyurax-ultrax`)

Hoje, `executar(esp, semente, continuar)`. Na 1.0 entra um observador:

```rust
pub trait Observador {
    /// Quer uma amostra agora? (limita a frequência sem o motor saber de relógio)
    fn quer(&mut self) -> bool;
    fn amostra(&mut self, a: Amostra);
}

pub enum Amostra {
    Matriz   { lado: u32, linhas_feitas: u32, bloco: Vec<i64> /* recorte real de C */ },
    Difusao  { lado: u32, passo: u32, grade: Vec<i64> /* grade real, reduzida */ },
    Ia       { passo: u32, erro: u64, pesos: Vec<i32> /* camada real */ },
    Genetica { geracao: u32, frequencias: Vec<u32> /* por locus */ },
    Melhoramento { geracao: u32, pontos: Vec<(i32, i32)> /* (valor genético, fenótipo) */, selecionados: u32 },
    Rotas    { passada: u32, custo: u64, rota: Vec<u16> },
    Triagem  { molecula: u32, nota: i64, passou: bool },
    Mochila  { item: u32, linha: Vec<i64> /* linha da tabela real */ },
}
```

- As amostras são **dados do cálculo**, recortados quando grandes demais.
  Quando há recorte, ele é dito na amostra.
- A interface de hoje (`continuar`) continua existindo: os motores ficam
  com uma assinatura só, e o observador é opcional.
- **Teste obrigatório:** o resultado e o hash são iguais com e sem
  observador, para cada motor e contra os vetores.

### Motor de computação e backends

```rust
pub trait Backend {
    fn recurso(&self) -> Recurso;               // CPU (núcleos) | GPU integrada | GPU dedicada, nome, memória
    fn aceita(&self, tipo: TipoDeTrabalho) -> bool;
    fn executar(&self, unidade, limites, &mut dyn Observador) -> Result<Execucao, _>;
}
```

- **CPU:** as linhas e o limitador que já existem, movidos para cá. É
  REAL.
- **GPU (WebGL 2):** o código de hoje, com três mudanças:
  - um canal próprio, sem passar pela API da tela;
  - calcula **unidades de JOB de matriz** e a **prova útil do bloco**;
  - classifica a placa como integrada ou dedicada, pelo nome do adaptador
    e pela memória.
  - Continua conferida por Freivalds na CPU.
- **GPU nos outros motores:** PENDENTE, motor por motor. Difusão (uma grade
  de inteiros) é a próxima candidata natural. Genética e rotas são
  sequenciais e não ganham com GPU.
- **Backend nativo (wgpu, Vulkan, DX12):** PENDENTE. Precisa de dependência
  nova e do compilador MSVC (ver decisões).

### A cena 3D por tipo de trabalho (motor de cena em WebGL 2)

| Workload | O que se vê | Dado que alimenta |
|---|---|---|
| Matriz | A e B como planos, e C como relevo que se preenche faixa a faixa, na cor do recurso que calculou (CPU ou GPU); depois, o "selo" de Freivalds | recorte real de C, faixas feitas, backend e resultado da verificação |
| Difusão | superfície 3D da grade, passo a passo | a grade real, reduzida |
| Genética | superfície: geração × locus × frequência do alelo | as frequências reais de cada geração |
| Melhoramento | nuvem de plantas (valor genético × fenótipo × geração), com as selecionadas destacadas | os indivíduos reais, por amostra |
| Rotas | cidades no plano, a rota atual como linha que se reorganiza a cada passada do 2-opt, e o custo | a rota e o custo reais |
| Triagem (molecular) | a molécula em avaliação, com a nota prevista e o valor medido em laboratório; a fila de moléculas como trilha | molécula, nota e filtro reais. **Estrutura 3D: PENDENTE** até gerar coordenadas (ver decisões); até lá, estrutura 2D posta no espaço, e a tela diz isso |
| IA | camadas da rede com os pesos reais, e a curva do erro | os pesos e o erro reais |
| Mochila | itens como barras (peso × valor), com os escolhidos marcados | tabela e escolha reais |
| Materiais, energia, meio ambiente, regeneração | **nada é desenhado**: "sem motor" | PENDENTE |
| Rede | este nó e os pares **conectados**, com as unidades que atravessaram cada ligação | ofertas, pedidos e resultados reais |
| Bloco | a prova útil do bloco (matriz) e a busca Argon2id, com o bloco achado | cabeçalho e prova reais |

Em toda cena, a mesma faixa de leitura, que responde às seis perguntas:

**qual trabalho → como está sendo processado → em que recurso → resultado →
como foi verificado → impacto na rede.**

### Métricas da máquina

| Métrica | 1.0 | Como |
|---|---|---|
| CPU da máquina e do processo | REAL | contadores de desempenho do Windows (PDH), lidos pelo `typeperf`, sem dependência nova e sem `unsafe` |
| RAM da máquina | REAL | pelo mesmo caminho |
| GPU (uso por motor 3D/compute, memória dedicada e compartilhada) | REAL no Windows 10 1709 ou mais novo | contadores "GPU Engine" e "GPU Adapter Memory" |
| Temperatura | PENDENTE | sem API confiável sem administrador; a tela diz "indisponível", com o motivo |
| Energia | ESTIMADO | TDP informado pelo dono × uso medido, com a fórmula na tela. Com bateria, a taxa de descarga do Windows é REAL |
| Custo | ESTIMADO | energia × preço do kWh informado |

### Cadeia, carteira e rede na 1.0

- **Carteira:** saldo, endereço, nonce, histórico, envio, recebimento e
  estado da sincronização. Tudo isso já é REAL; ganha tela própria e passa
  a ser serviço do núcleo.
- **Cadeia:**
  - altura, dificuldade, recompensa, coinbase, validação e sincronização
    são REAIS;
  - a mineração sai da tela e vira serviço;
  - a prova útil pode ser calculada na GPU.
- **Rede P2P:**
  - conexão cifrada, descoberta, tarefas e resultados entre nós são REAIS;
  - nós semente: PENDENTE (infraestrutura);
  - bytes medidos: passam a REAL;
  - reputação: passa a ser gravada em disco.
- **Impacto da ciência na rede:** na 1.0 ele é mostrado como é:
  - unidades conferidas por outros nós;
  - reputação;
  - créditos, que não são HYX.
  - **Ligar a ciência à recompensa do bloco é PENDENTE.** É mudança de
    consenso: pede uma especificação nova (SPEC-02), vetores e análise de
    ataque. Não entra na 1.0 como remendo.

---

## Parte 4 — Decisões que dependem de você

| # | Decisão | Recomendação | Custo |
|---|---|---|---|
| 1 | GPU nativa (wgpu) na 1.0? | **Não na 1.0.** WebGL 2 como backend formal agora; o nativo fica como próximo backend | wgpu traz dependências grandes e exige o compilador MSVC. Nesta máquina (Atom, alvo GNU) o `windows-link` novo já quebrou o build uma vez |
| 2 | Métricas pelo `typeperf` (processo externo) em vez de chamar a API do Windows direto | **Sim.** É real, não precisa de `unsafe` nem de biblioteca nova | uma leitura a cada 1 a 2 s |
| 3 | Moléculas em 3D de verdade | **Sim, gerando coordenadas 3D uma vez**, no Python (RDKit, ETKDG), gravadas no catálogo com a origem | instalar o RDKit na pasta `.toolchain` do projeto (~30 MB); o catálogo cresce uns 5 MB |
| 4 | Nome da versão | **"Hyurax 1.0 · rede de TESTE"** | nenhum |

---

## Parte 5 — Ordem de implementação

Cada fase termina com testes, clippy limpo, commit e uma captura de tela do
que mudou.

1. **Núcleo separado da tela:**
   - mineração, carteira e ajustes viram serviços;
   - um barramento de eventos só;
   - `/api/v1` com fluxo SSE;
   - o comportamento não muda, e a tela atual continua funcionando.
2. **Observador nos oito motores:** as amostras de cada um, mais o teste de
   que observar não muda o resultado.
3. **Métricas reais da máquina,** e a correção do rótulo falso da CPU.
4. **Motor de computação com backends:** a CPU e a GPU (WebGL 2) formais; a
   GPU passa a calcular unidades de JOB de matriz e a prova do bloco.
5. **Motor de cena e visualizadores:** um por tipo de trabalho, nesta ordem:
   matriz, genética, rotas, difusão, melhoramento, IA, mochila, triagem,
   rede e bloco.
6. **Telas da 1.0:** visão geral com a linha "trabalho → recurso →
   resultado → verificação → impacto", mais Carteira, Cadeia, Rede e
   Registro.
7. **Testes de ponta a ponta, instalador 1.0 e documentação.**

As fases 1 e 2 não mudam nada que você veja, mas são elas que tornam as
outras honestas.
