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

- Temperatura da CPU e da GPU; energia medida (hoje é estimada).
- GPU nos motores além da matriz; backend nativo (Vulkan/DX12).
- Conformação 3D otimizada das moléculas (RDKit/ETKDG ou campo de força). Desde
  a 1.2.0, a tela Moléculas mostra um 3D aproximado (ver abaixo).
- Furo de NAT direto (transporte UDP); hoje quem não tem porta aberta passa
  por ponte.
- Cobrança em dinheiro no mercado: os preços são em créditos de computação.
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

## 1.2.0 (04/10/2026)

- **Marca nova:** o H com a travessa em subida, em âmbar
  (`design/marca/`, gerado por `scripts/gerar-marca.py`). O instalador grava
  `Hyurax.ico` ao lado do programa e aponta para ele os atalhos (Área de
  Trabalho e Menu Iniciar) e a entrada em "Aplicativos". Antes os atalhos
  ficavam sem ícone. A janela usa a mesma marca.
- **Tela Moléculas:** 8.289 moléculas reais da AqSolDB em 3D. A geometria é
  calculada no próprio programa, sem biblioteca:
  - hidrogênios completados pela valência;
  - comprimento de ligação pelos raios covalentes;
  - ângulo pela hibridização;
  - anéis aromáticos e conjugados planos;
  - distâncias resolvidas em 4D e achatadas para 3D.

  Em 20 moléculas do catálogo, o desvio médio das ligações ficou entre 0,00 e
  0,08 Å. Os comandos da tela são:
  - **Vibrar:** dinâmica clássica nas mesmas molas, com temperatura ajustável;
  - **Bastões** e **Esferas** (raios de van der Waals);
  - busca por nome ou fórmula.

  Ao lado de cada molécula aparecem a solubilidade medida em laboratório e a
  prevista pela IA do ULTRAX, com o erro típico do modelo (medido em
  moléculas que ele não viu no treino). Não é cálculo quântico: o rótulo da
  tela diz isso.
- **Bytes da rede:** contados no socket, nos dois sentidos, e mostrados no
  Painel ao vivo (REAL).
- **API:** `GET /api/v1/ciencia/moleculas?busca=` (até 24 resultados). A
  rota `GET /api/v1/ciencia/molecula/<i>` agora traz também `previsto_mili`,
  `erro_tipico_mili` e `total`.

## 1.3.2 (04/10/2026): correção do núcleo travando

- **O defeito:** na 1.3.1, assim que o programa se conectava a outro nó (a
  semente pública, pela primeira vez), o núcleo parava inteiro: o painel
  mostrava "o núcleo não está respondendo" e a mineração parava junto.
- **A causa:** para montar o estado da tela, o núcleo pega a trava da cadeia.
  Com ela ainda pega, chamava uma função que pega a mesma trava de novo para
  dizer se o nó está "sincronizado". A trava não é reentrante, então a linha
  esperava por si mesma para sempre, e tudo que precisa da cadeia esperava
  atrás. Sem nenhum par conectado esse caminho não era percorrido; por isso
  o defeito só apareceu com a semente pública no ar.
- **A correção:** o estado compara o trabalho dos pares com a cadeia que já
  tem na mão, sem pegar a trava de novo (`Rede::trabalho_dos_pares`). Uma
  varredura no núcleo não achou outro ponto com o mesmo padrão.
- **O teste que trava o defeito:** `estado_responde_com_par_conectado`
  (`crates/hyurax-nucleo/tests/api_local.rs`). Com o código antigo, ele
  congela como o programa congelou; com a correção, passa. Num ensaio na
  rede de teste real, ligado à semente pelo WebSocket, o estado respondeu
  em 15 ms durante 3 minutos.

## 1.4.0 (04/10/2026): planos

- **Planos** ([MONETIZACAO.md](MONETIZACAO.md)): Comunidade (grátis), Pro
  (R$ 29/mês), Equipe (R$ 149/mês) e Empresa (a partir de R$ 990/mês),
  com o que entra em cada um e de onde vêm os números.
- **Voucher de plano assinado:** especificado em `reference/hyurax/plano.py`
  e conferido pelo Python, pelo Rust (`hyurax-nuvem/src/plano.rs`) e pelos
  vetores (`vectors/plano.json`). O programa confere sozinho, sem conta nem
  senha; o beneficiário é a chave do worker, não uma pessoa.
- **Efeito real já hoje:** com plano ativo, a comissão da plataforma no livro
  de contas cai (15% → 10% no Pro, 8% no Equipe).
- **Tela Planos** no programa e **seção de preços** no site (4 idiomas),
  marcadas "a venda ainda não abriu".
- **`hyurax-no planos`**: catálogo, chave de planos (fora do repositório),
  emitir e conferir voucher.

## 1.5.0 (16/10/2026): HYURAX Compute, a plataforma V1

O mercado de computação do documento "HYURAX V1 — Compute Marketplace", de
ponta a ponta. O mapa seção por seção está em
[V1-PLATAFORMA.md](V1-PLATAFORMA.md); como hospedar e operar, em
[OPERAR-A-PLATAFORMA.md](OPERAR-A-PLATAFORMA.md).

- **`hyurax-plataforma`**, o servidor central (vem nos pacotes de terminal):
  contas com Argon2id, sessão e chave de API (HYX_API_KEY) com escopo e
  limite; registro de nós com pulso e telemetria; agendador que escolhe o
  nó mais adequado, não o mais forte; tarefas com estimativa e reserva de
  saldo; conferência (Freivalds no servidor, concordância entre nós de donos
  diferentes por amostragem de reputação, quarentena); cobrança em real ou
  dólar num livro de partida dobrada; retenção e saque dos fornecedores;
  antifraude; administração com kill switch em quatro níveis; diário com
  hash encadeado que é ao mesmo tempo banco, sistema de eventos e registro
  de auditoria.
- **Painéis web** do cliente, do fornecedor e do administrador, no Cloud
  Design do programa (claro, escuro e celular).
- **Tela Fornecer** no programa e **`hyurax-no agente`**: este computador
  como fornecedor. Vincula com o código do painel, empresta os núcleos e a
  memória que o dono escolher, pausa na hora. As unidades rodam no mesmo
  worker do ULTRAX, com os mesmos limites e a mesma prova assinada.
- **Regras de dinheiro especificadas em Python** (`reference/hyurax/plataforma.py`):
  preço por hora de núcleo de referência, liquidação que fecha ao centavo,
  reputação e assinatura do agente, conferidas pelo Rust em
  `vectors/plataforma.json`.
- **Testes:** a definição de pronto inteira por HTTP (`fluxo_completo.rs`),
  segurança básica (`seguranca.rs`), queda e volta (`recuperacao.rs`), e o
  agente real contra o servidor real nesta máquina.
- **O que depende do dono:** servidor com disco persistente para hospedar,
  e a empresa (CNPJ com a atividade certa, conta PJ, provedor de pagamento,
  termos revisados) para abrir a venda. Até lá, o pagamento é Pix manual
  conferido pela equipe, e só abre quando a chave Pix da empresa estiver na
  configuração.

## 1.5.1 (17/10/2026): TLS corrigido

A 1.5.0 não chegou a ser publicada: a conferência de falhas conhecidas
(`cargo audit`) parou o lançamento, porque em 14/10 saíram quatro avisos para
o `rustls` 0.23.45 (RUSTSEC-2026-0346 a 0349). A 1.5.1 é a 1.5.0 com o
`rustls` 0.23.46, a versão corrigida, sem dependência nova.

- **Servidor da plataforma:** o pedido recusado antes de o corpo ser lido
  (grande demais, lento demais) agora fecha com calma. Antes, o sistema podia
  cortar a conexão com RST e apagar a resposta: o navegador mostrava "conexão
  redefinida" em vez do aviso de tamanho.

## 1.5.2 (22/10/2026): raízes de certificado atualizadas

Mais dois avisos para o TLS, de 19 e 20/10: as raízes embutidas confiavam em
intermediários da emSign já desconfiados (RUSTSEC-2026-0381, `webpki-roots`
1.0.9 → 1.0.10), e a conferência da cadeia podia aceitar intermediário
desconfiado (RUSTSEC-2026-0382, `rustls-webpki` 0.103.15 → 0.103.16). Nenhuma
dependência nova. Novo teste `https_de_verdade` (só com
`HYX_TESTE_INTERNET=1`): o cliente do agente fala HTTPS com a semente no Render.
Também: o teste `decisora_decide_fora_da_thread_do_par` lia os eventos antes
de a decisora emiti-los (ela fecha o JOB dentro da trava e emite depois) e
falhava às vezes em máquina carregada; agora espera o `JOB_COMPLETED`.

## 1.5.3 (22/10/2026): temporada de ataques, rodada interna

Inclui tudo da 1.5.2 (raízes de certificado e conferência de cadeia).
O guia da temporada de ataques (regras, laboratório, o que já está coberto,
como relatar e o placar) está em [TEMPORADA-DE-ATAQUES.md](TEMPORADA-DE-ATAQUES.md).
Antes de chamar gente de fora, o projeto atacou a si mesmo:

- **Porta WebSocket** (a da semente pública): cada conexão ganhava uma linha
  antes do upgrade, sem teto, e uma falha ao abrir linha nova derrubava o laço
  que aceita conexões. Agora são no máximo 128 atendimentos antes do upgrade,
  e a falha fecha só aquela conexão. Os quadros seguem a RFC 6455: controle
  com até 125 bytes e inteiro, e bits reservados ou tipos desconhecidos
  derrubam a conexão. O mesmo cuidado na criação de linha vale para a porta
  TCP do nó.
- **Fuzzing** (`crates/hyurax-nucleo/tests/fuzz.rs`): os 15 decodificadores
  que recebem bytes de fora e a conferência dos 8 motores, com mutações
  determinísticas. Nada cai nem demora, o que é aceito tem uma codificação
  só, e nenhum resultado falso passa nos motores de resultado único. O CI
  roda 100 mil mutações por alvo, com semente nova a cada execução.
- **SECURITY.md** atualizado para a rede de teste pública.
