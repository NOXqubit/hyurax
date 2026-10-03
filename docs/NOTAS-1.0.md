# Hyurax / Ultrax 1.0

**O programa é a versão 1.0. A rede é de TESTE.** O HYX de teste não tem
valor. Não há venda, pré-venda nem promessa de lucro. O Work Score e os
créditos de computação medem contribuição: não são dinheiro e não são HYX.

## Compatibilidade

- **A cadeia é a mesma** da `v0.4.0-teste.1`: bloco, transação, consenso e
  prova útil não mudaram. Nós 0.4 e 1.0 mineram e sincronizam juntos.
- **A carteira continua valendo.** Instalar a 1.0 por cima atualiza o
  programa e não toca na carteira.
- **O cálculo científico entre nós só fala com nós 1.0.** As etiquetas
  internas do ULTRAX passaram de `UTRAX` para `ULTRAX`, e as assinaturas de
  resultado de um nó 0.4 não conferem num 1.0 (nem o contrário). Isso não
  mexe na cadeia.
- **Pastas novas.** A configuração e a carteira continuam em
  `%APPDATA%\Hyurax`; a cadeia, o ULTRAX, os JOBs e os registros passam para
  `%LOCALAPPDATA%\Hyurax`. Na primeira abertura, o histórico do ULTRAX e dos
  JOBs da 0.x vai para `arquivo-0.x` (o formato mudou), e a cadeia é movida
  para a pasta nova.
- **Termos de uso, versão 3.** Saiu o CoinGecko (o painel de preços foi
  removido) e entraram as duas pastas. O programa pede o aceite de novo.

## O que há de novo

- **Um núcleo só** (`hyurax-nucleo`), sem tela: nó, carteira, mineração,
  ULTRAX, ciência, métricas e a API local v1 com fluxo de eventos (SSE). A
  janela, o terminal (`hyurax-no painel`) e as outras máquinas do dono leem o
  mesmo núcleo.
- **Painel novo**, com oito seções: Visão geral, ULTRAX, Computação
  científica, Carteira, Cadeia e mineração, Rede, Registro e Ajustes.
- **Cada número diz de onde veio:** REAL, DERIVADO, ESTIMADO, SIMULADO,
  AJUSTE ou PENDENTE. O limite de CPU aparece como ajuste, ao lado do uso
  medido, e nunca no lugar dele.
- **Métricas reais da máquina:** CPU da máquina e do programa, RAM, uso da
  GPU (3D e cálculo) e memória da GPU, pelos contadores do Windows (ou
  `/proc` no Linux). Temperatura: PENDENTE. Energia e custo: ESTIMADO, com a
  fórmula na tela.
- **Visualização 3D feita do cálculo de verdade.** Os oito motores entregam
  amostras do próprio estado enquanto calculam, e a cena desenha só isso: a
  matriz C linha a linha (na cor da GPU quando foi ela que calculou), a
  superfície da difusão, a tabela da mochila, os pesos e a perda da IA, as
  frequências da genética, a nuvem de plantas do melhoramento, as rotas do
  2-opt sobre as cidades reais e a triagem de moléculas. Observar não muda o
  resultado (testado motor a motor).
- **Linha de leitura** da tarefa em foco: trabalho → como → recurso →
  resultado → verificação → impacto na rede, dizendo o que é teste (LAB) e o
  que é pedido de verdade (JOB).
- **A GPU calcula unidades de JOB** de matriz, não só a carga de teste, e a
  CPU confere cada resultado por Freivalds antes de contar.
- **Instalador 1.0:** atualiza no lugar, grava um manifesto do que instalou
  (`instalacao.txt`) e a desinstalação apaga só o que está nele.
- **Montagem reprodutível** (`scripts/empacotar-windows.ps1`) e teste de
  instalação limpa numa pasta isolada (`scripts/testar-instalacao.ps1`), que o
  lançamento roda antes de publicar.

## O que saiu

- O nome antigo do projeto, em código, textos e domínios.
- O minerador legado, o painel de preços (CoinGecko) e a personalização de
  painéis.
- O painel 0.x inteiro (`painel.js`, `ciencia.js` e o estilo), substituído
  pela interface em módulos.
- A ideia de "mainnet" no programa: a linha de comando aceita só `testnet` e
  `regtest`.

## O que ainda não existe (PENDENTE)

- Temperatura da CPU e da GPU; energia medida; bytes da rede.
- GPU nos motores além da matriz; backend nativo (Vulkan/DX12).
- Coordenadas 3D reais das moléculas (a cena mostra o grafo plano do SMILES).
- Cena da rede (pares e unidades atravessando) e do bloco.
- Trabalho científico valendo na recompensa do bloco: mudança de consenso,
  especificação nova (SPEC-02).
- Nós semente públicos da testnet e assinatura digital do programa.

## Depois da 1.0: o Documento Mestre (02 e 03/10/2026)

O roadmap de 10 fases do [Documento Mestre](DOCUMENTO-MESTRE.md) foi seguido
em ordem; o registro de cada fase está em
[AUDITORIA-DOCUMENTO-MESTRE.md](AUDITORIA-DOCUMENTO-MESTRE.md). O que mudou
para quem usa:

- **Segurança:** a janela fala com o núcleo por uma chave de sessão; o
  código de 6 dígitos tem espera depois de erros e não vale duas vezes; a
  carteira antiga (segredo em texto) não envia mais; atualizações só com
  manifesto assinado pela chave de lançamento.
- **Computação entre nós (protocolo ULTRAX v2):** o compromisso é assinado
  pelo worker; um nó que cai não segura mais as unidades por 2 minutos;
  a reputação dos workers é gravada, com Gold Score e "nó verificado".
- **Novos comandos:** `hyurax-no identidade ver|girar`,
  `hyurax-no contas ...` (API externa), `hyurax-no copia criar|conferir|restaurar`.
- **Tela:** API externa e contas em Ajustes; aviso quando o núcleo para de
  responder e quando um JOB termina; "pular para o conteúdo" e contraste
  maior no texto secundário.
- **Operação:** `/api/v1/saude` para monitoramento e o manual
  [OPERACAO.md](OPERACAO.md).
