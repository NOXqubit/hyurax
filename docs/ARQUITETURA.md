# Hyurax / Ultrax 1.0: arquitetura

O programa é a versão **1.0**. A rede é de **teste** (TESTNET): o HYX não tem
valor, e o programa não aceita outra rede além de `testnet` e `regtest`
(`hyurax_nucleo::config::rede_do_nome` recusa qualquer outro nome).

Este documento descreve o que existe no código. O plano que levou a ele, com a
auditoria do WebGL 2 e do programa 0.x, está em
[`historico/AUDITORIA-1.0.md`](historico/AUDITORIA-1.0.md).

## Princípios

1. **Nada chega à tela sem origem.** Cada número leva um rótulo: REAL,
   DERIVADO, ESTIMADO, SIMULADO, AJUSTE, MOCK ou PENDENTE. A tela mostra o
   rótulo ao lado do número. Limite configurado nunca aparece como uso; estimativa
   nunca aparece como medição.
2. **O motor diz o que está fazendo.** Cada motor de cálculo entrega amostras
   do próprio estado enquanto calcula. A cena 3D só desenha essas amostras.
3. **Observar não muda o resultado.** O resultado e o hash são os mesmos com o
   observador ligado ou desligado (teste `hyurax-ultrax/tests/observador.rs`,
   motor a motor, e os vetores do gabarito Python continuam valendo).
4. **A tela é cliente.** O núcleo funciona sem ela: `hyurax-no` roda o mesmo
   núcleo no terminal.
5. **Python especifica, Rust executa, vetores provam.**

## As peças

```
┌──────────────── Hyurax.exe (hyurax-app: janela tao + wry/WebView2) ────────────────┐
│  hyurax-interface (HTML, CSS, módulos JS, embutidos no .exe)                        │
│    estado único ◄── /api/v1/fluxo (SSE)    comandos ──► POST /api/v1/* (só local)   │
│    telas: Visão geral · ULTRAX · Ciência · Carteira · Cadeia · Rede · Registro · Ajustes │
│    cena 3D (WebGL 2): motor de cena + um visualizador por tipo de trabalho          │
│    backend de GPU (WebGL 2, cálculo) num worker próprio                             │
└───────────────────────────────────────▲─────────────────────────────────────────────┘
                                        │ HTTP em 127.0.0.1:8800, Host e Origin conferidos
┌────────────────────────────── hyurax-nucleo (Rust) ─────────────────────────────────┐
│ api         rotas v1, estado em JSON, fluxo SSE (no máximo 8 fluxos)                │
│ barramento  eventos em ordem (registro, tarefa, ciencia, amostra, bloco)            │
│ servico     liga e desliga tudo; trava a pasta de dados contra uma segunda cópia    │
│ cadeia · carteira · mineracao · ultrax · ciencia · metricas · maquinas · ajustes    │
│ pastas      configuração e dados do usuário, separados do programa                  │
│ instalacao  a pasta do programa e o manifesto do instalador                         │
└─────────────────────────────────────────────────────────────────────────────────────┘
   protocolo: types, crypto, codec, tx, block, consensus, state, chain, store, pow, usefulpow
   rede: wire, net (Noise XX), eter    ·    trabalho útil: hyurax-ultrax (motores + observador)
```

| Crate | Papel |
|---|---|
| `hyurax-nucleo` | tudo o que o programa faz, sem tela |
| `hyurax-interface` | a tela: arquivos estáticos embutidos (`ARQUIVOS`), com teste de que todo arquivo pedido está embutido |
| `hyurax-app` | o programa com janela (`Hyurax.exe`); só compila o código de janela no Windows |
| `hyurax-no` | o programa de terminal: carteira, minerar, enviar, `painel` (núcleo + interface no navegador), nó, ULTRAX, ciência |
| `hyurax-instalador` | o instalador para Windows, que leva o programa dentro |
| `hyurax-ultrax` | os oito motores de trabalho útil e o observador |

## Pastas do usuário

O programa nunca grava ao lado de si mesmo. No Windows:

| Pasta | O que guarda |
|---|---|
| `%LOCALAPPDATA%\Programs\Hyurax` | o programa: `Hyurax.exe`, `WebView2Loader.dll`, termos, LEIA-ME e o manifesto `instalacao.txt` |
| `%APPDATA%\Hyurax` | configuração: `carteira.txt` (cifrada), `ajustes.txt`, `termos.txt` (aceite), `no.chave`, segundo fator, sementes |
| `%LOCALAPPDATA%\Hyurax` | dados: a cadeia (`<rede>.cadeia`), `ultrax/`, `ciencia/`, `registros/hyurax.log`, `navegador/` (perfil do WebView2) |

No Linux, as pastas do XDG. `--pasta P` (no `hyurax-no`) põe tudo em `P`.
Ao abrir a 1.0 pela primeira vez sobre dados da 0.x, `pastas::migrar_da_0x`
arquiva o que mudou de formato em `arquivo-0.x` e marca a pasta com
`versao-dos-dados.txt`. A pasta de dados é travada (`em-uso.trava`): uma
segunda cópia do programa só abre outra janela para o núcleo que já roda.

## O caminho do dado

```
WORKLOAD REAL → MOTOR (CPU ou GPU) → AMOSTRAS REAIS → VERIFICAÇÃO → EVENTOS → WEBGL 2
```

1. **Trabalho.** Um JOB pedido (unidade de um JOB científico, REAL) ou, sem
   JOB, a carga **LAB**: conta de verdade sobre entrada gerada nesta máquina
   (SIMULADO na tela). Tarefas-desafio, com resposta conhecida, testam o
   próprio worker.
2. **Cálculo.** Linhas de CPU com limitador por linha (fatia de tempo) e teto
   de memória; ou a GPU pelo WebGL 2 (só matriz), num worker da janela.
3. **Amostras.** Cada motor tem `executar_observado(…, &mut dyn Observador)`.
   O núcleo limita a 1 amostra a cada 150 ms por linha (`ObservadorDaLinha`) e
   publica no barramento como `amostra` = `{contexto, amostra}`. O contexto diz
   tarefa, JOB e unidade (ou LAB), linha, recurso (CPU/GPU) e a especificação.
   A GPU manda, a cada faixa lida de volta, uma linha real de C.
4. **Verificação.** Recomputação, prova do próprio motor ou Freivalds (a CPU
   confere o que a GPU calculou). Os passos viram eventos `tarefa`:
   `WORK STARTED`, `RESULT SUBMITTED`, `VERIFICATION PASSED/FAILED`,
   `SETTLEMENT COMPLETED`…
5. **Tela.** O fluxo SSE leva o estado (no máximo uma vez por segundo) e os
   eventos. A cena 3D guarda o estado de cada tarefa que mandou amostra e
   desenha a que está em foco.

### Os oito motores e o que a cena mostra

| Motor (tipo) | Amostra | O que se vê |
|---|---|---|
| Matriz (`matrix`) | uma linha real de C (até 128 colunas) | fileiras de C, altura = valor entre o menor e o maior visto; cor do recurso (CPU ou GPU) |
| Mochila (`knapsack`) | a linha da tabela de PD depois de cada item | a tabela do ótimo crescendo item a item |
| Difusão (`diffusion`) | a grade real, reduzida | a superfície da grade, passo a passo |
| IA (`ai-training`) | passo, perda do lote e os pesos da saída | os pesos (claros positivos, escuros negativos) e a curva da perda |
| Genética (`population-genetics`) | contagem do alelo por locus | a frequência de cada locus ao longo das gerações |
| Melhoramento (`crop-breeding`) | (valor genético, fenótipo, selecionada) | nuvem de plantas por geração, selecionadas em destaque |
| Rotas (`routing`) | a rota depois de cada passada do 2-opt | as cidades reais da instância e as rotas empilhadas |
| Triagem (`molecular-screening`) | molécula, log S previsto, nota, aprovada | a trilha das moléculas triadas e a molécula em avaliação (grafo do SMILES real; **conformação 3D: PENDENTE**) |

O que falta na amostra aparece como falta na legenda, nunca preenchido.
Domínios sem motor (materiais, energia, meio ambiente, regeneração) não
desenham nada.

A Visão geral mostra a **linha de leitura** da tarefa em foco: trabalho →
como → recurso → resultado → verificação → impacto na rede. O impacto é dito
como é: a carga LAB não muda nada na cadeia; um JOB gera créditos de
computação (não HYX); nenhum dos dois muda a recompensa do bloco.

### Backends de cálculo

Não há um `trait Backend` artificial: a CPU são as linhas do worker, e a GPU
é o canal `/api/v1/gpu/*` (pegar, entrada, progresso, resultado, cancelar).
`Ultrax::gpu_pegar` entrega primeiro unidades de JOB de matriz
(`Agendador::proxima_do_tipo`) e só depois carga LAB, se o LAB estiver ligado.
A GPU calcula; a CPU confere por Freivalds antes de creditar.

A **prova útil do bloco fica na CPU**: as matrizes do bloco têm lado de 32 a
256 (48³ ≈ 110 mil operações, menos de 1 ms); mandar para a GPU custaria mais
na ida e volta do que a conta.

PENDENTE: GPU nos outros motores (difusão é a próxima candidata); backend
nativo (Vulkan/DX12), que traria dependência grande e o compilador MSVC.

## Métricas da máquina

| Métrica | Origem | Como |
|---|---|---|
| CPU da máquina e deste programa | REAL | Windows: contadores de desempenho pelo .NET (`PerformanceCounter`, nomes em inglês, que valem em qualquer idioma do Windows), num PowerShell auxiliar sem janela que sai sozinho quando o programa fecha; Linux: `/proc` |
| RAM livre, deste programa | REAL | idem (`Memory`, `Process` / `Working Set - Private`) |
| RAM usada | DERIVADO | total menos livre |
| GPU 3D, GPU cálculo, memória dedicada e compartilhada | REAL | `GPU Engine` (`engtype_3D`, `engtype_Compute`) e `GPU Adapter Memory`; todos os programas juntos |
| Tipo da GPU (integrada/dedicada) | DERIVADO | pelo nome do adaptador |
| Temperatura | PENDENTE | sem leitura confiável sem administrador |
| Energia e custo por mês | ESTIMADO | CPU medida do programa × núcleos × watts por núcleo **informados pelo dono** × preço do kWh informado |
| Bytes da rede | PENDENTE | não medidos na 1.0 |
| Limite de CPU por linha, teto de memória, fatia da GPU | AJUSTE | escolha do dono, mostrada ao lado do uso medido |

Sem o PowerShell (ou com os contadores desligados), as métricas ficam
PENDENTE e a tela diz o motivo (`metricas.problema`).

## A API local (v1)

Leitura (GET): `/api/v1/estado`, `/resumo` (público, para as outras máquinas
do dono), `/fluxo` (SSE), `/termos`, `/ciencia`, `/ciencia/benchmarks`,
`/ciencia/eventos?desde=N`, `/ciencia/molecula/I`, `/ciencia/rotas/inst/n`,
`/ciencia/job/ID[/historico|/relatorio.json|.csv|.pdf]`, `/gpu/entrada/N`.

Comando (POST de formulário): `/mineracao`, `/ultrax`, `/carteira/nova`,
`/carteira/importar`, `/carteira/conferir`, `/carteira/enviar`,
`/seguranca/comecar|confirmar|mudar`, `/destravar`, `/ciencia/rede`,
`/ciencia/benchmark`, `/ciencia/estimar`, `/ciencia/submeter`,
`/ciencia/job/ID/pausar|retomar|cancelar`, `/ajustes`, `/sementes`,
`/maquinas`, `/termos/aceitar`, `/abrir-pasta`, `/gpu/pegar`,
`/gpu/progresso/N`, `/gpu/cancelar/N`, `/gpu/resultado/N` (binário).

Segurança: comando só com Host e Origin locais; com "ver no celular" ligado,
a rede local só lê; programa trancado (segundo fator) recusa tudo menos
`/destravar`.

## Decisões registradas

- **Tag de consenso congelada.** As etiquetas de domínio do ULTRAX passaram a
  `HYURAX-ULTRAX-*`, menos a que entra no consenso da prova útil, que continua
  com o nome antigo: trocá-la mudaria o hash de blocos já validados.
- **Sem mainnet no código.** A CLI só aceita `testnet` e `regtest`.
- **Mercado (CoinGecko) e painéis personalizáveis saíram.** Não eram parte do
  produto e traziam dado de terceiro para a tela.
- **Sem dependência nova.** A 1.0 usa só o que já estava no `Cargo.lock`
  (o `serde_json` passou a ser usado pelo núcleo).
- **Regras de código:** `unsafe` proibido; `clippy` nega `unwrap`, `expect`,
  `panic` e indexação direta nos crates do programa.

## O que é o quê na 1.0

| Parte | Estado |
|---|---|
| Nó, cadeia, consenso, Argon2id, prova útil, carteira, envio, segundo fator | REAL |
| Mineração (ritmo medido; limite é AJUSTE) | REAL |
| Motores do ULTRAX e verificação | REAL (cálculo e conferência de verdade) |
| Carga LAB | SIMULADO (entrada gerada aqui; a conta é real) |
| JOBs científicos e relatórios | REAL (pedido do dono); verificação entre máquinas ainda não ensaiada fora de uma máquina |
| Work Score e créditos de computação | DERIVADO (medem contribuição; não são dinheiro nem HYX) |
| Cena 3D | REAL (só amostras dos motores) |
| Coordenadas 3D de moléculas | PENDENTE |
| Cena da rede (pares e unidades atravessando) e do bloco | PENDENTE |
| Trabalho científico na recompensa do bloco | PENDENTE (mudança de consenso, SPEC-02) |
| Nós semente públicos e assinatura digital do programa | PENDENTE |
