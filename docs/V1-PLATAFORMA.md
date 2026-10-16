# HYURAX Compute (V1): a plataforma do mercado de computação

Escrito em 16/10/2026 a partir do documento **"HYURAX V1 — Arquitetura
Completa — Compute Marketplace"** do autor. Este arquivo diz, seção por seção,
o que foi construído, onde está no código, como foi testado e o que ainda
falta (e por quê).

> **Regra que não muda (seção 20 do documento):** a V1 cobra em real ou
> dólar, por saldo pré-pago. Não há token, HYX, blockchain nem mineração no
> caminho do dinheiro. O saldo é crédito de uso: não é criptomoeda,
> investimento nem promessa de valorização.

## Em uma página

```text
 cliente ──► página web ──► API /api/v1 ──► contas · cobrança · mercado · tarefas
                                                   │
                                              agendador ──► registro de nós
                                                   │
                                     agente (programa Hyurax do fornecedor)
                                                   │
                              motor do ULTRAX, isolado por construção
                                                   │
                         resultado ──► conferência ──► cobrança ──► repasse
```

| Peça | Onde | Estado |
|---|---|---|
| Servidor central (API, painéis, agendador, cobrança, administração) | `crates/hyurax-plataforma` (binário `hyurax-plataforma`) | **pronto e testado** |
| Regras de dinheiro e assinatura (preço, liquidação, reputação, pedido do agente) | `reference/hyurax/plataforma.py` → `vectors/plataforma.json` → `regras.rs` | **pronto**, Python e Rust iguais por vetor |
| Agente do fornecedor | `crates/hyurax-nucleo/src/plataforma.rs`, tela **Fornecer** do programa e `hyurax-no agente` | **pronto e testado com o servidor de verdade** |
| Painel do cliente, do fornecedor e do administrador | `crates/hyurax-plataforma/web` | **pronto** (claro, escuro e celular) |
| Hospedagem pública | — | **depende do dono**: precisa de servidor com disco que não se apaga (ver `OPERAR-A-PLATAFORMA.md`) |
| Pagamento por provedor integrado (Asaas, Mercado Pago) | — | **depende do dono** (CNPJ com a atividade certa, conta PJ, contrato com o provedor) |

### Como foi provado

- **A definição de pronto inteira (seção 63)**, por HTTP de verdade, com dois
  fornecedores simulados que executam os motores do ULTRAX de verdade e
  assinam como o programa: `crates/hyurax-plataforma/tests/fluxo_completo.rs`.
- **Segurança básica**: dados de outro cliente, papéis, CSRF, cabeçalhos,
  repetição de pedido do agente, janela de tempo, força bruta, chave de API
  com escopo e limite, corpo grande, caminho estranho, e a conferência de que
  o código dos motores não usa arquivo, rede, processo nem ambiente:
  `tests/seguranca.rs`.
- **Recuperação**: o servidor cai no meio do trabalho e volta igual; a
  unidade que estava com o agente é entregue depois da volta; a escrita
  cortada no meio é desfeita só na última linha: `tests/recuperacao.rs`.
- **Fornecedor desonesto**: matrizes erradas e bem assinadas são recusadas
  por Freivalds no servidor, o nó vai para a quarentena e ninguém paga por
  resultado errado.
- **O agente real** (o programa `hyurax-no`, com o worker do ULTRAX) contra o
  servidor rodando nesta máquina: vínculo pelo código, pulso com a
  telemetria real (Intel Atom x5-Z8350, 4 núcleos, 3,4 GB), tarefas de rotas
  e de genética de populações calculadas, entregues, conferidas e cobradas.

## Seção por seção

Legenda: **pronto** (feito e testado), **parcial** (feito com limite dito),
**depende do dono**, **depois da V1** (o próprio documento deixa para depois).

### 1 a 4. Produto, fluxo, princípio e arquitetura — pronto

O fluxo da seção 2 é o caminho do código, sem atalho. O princípio da seção 3
(o cliente nunca entra no computador do fornecedor) é garantido **por
construção**: só rodam os oito motores que vêm no programa; o pedido do cliente
é uma lista de parâmetros, nunca código ou comando. O teste
`motores_nao_tocam_arquivo_rede_nem_processo` confere que o código dos motores
não usa `std::fs`, `std::net`, `std::process`, `std::env` nem sockets.

### 5. Frontend — pronto

- **Painel do cliente:** saldo, adicionar saldo, criar tarefa (motor,
  tamanho, unidades, núcleos em paralelo, cópias, GPU, prazo), estimativa de
  preço ao vivo, acompanhamento com progresso de cada unidade, histórico,
  resultado agregado e por unidade, relatório completo, consumo, fatura do
  mês, extrato e chaves de API.
- **Painel do fornecedor:** registrar computador (código de vínculo),
  instruções do agente, estado, CPU, GPU, RAM, disco, temperatura (quando o
  sistema informa), uso, unidades executadas, ganhos, disponibilidade,
  limites, pausar e remover.
- **Painel do administrador:** seção 37 inteira (abaixo).

### 6 e 7. Contas e usuários — pronto, com dois limites

Cadastro, entrada, sessão (cookie HttpOnly, SameSite=Strict, Secure no HTTPS;
7 dias no máximo, 24 h parada), papéis CUSTOMER, PROVIDER e ADMIN (um usuário
pode ser cliente e fornecedor), país e moeda, estado de verificação.
Senha só em hash Argon2id (m = 19 MiB, t = 2, p = 1), com regra de senha e
trava por força bruta.

- **Recuperação de conta:** por oito códigos de recuperação mostrados no
  cadastro (cada um vale uma vez), ou pelo administrador, que gera códigos
  novos para entregar por um canal conferido. **Não há envio de e-mail**: o
  servidor não tem SMTP. Ligar um provedor de e-mail é passo de operação.
- **MFA:** a seção 6 deixa para depois. O programa já tem código de segundo
  fator (TOTP) na carteira; trazer para a plataforma é a primeira coisa da
  Fase 2.

### 8. Registro de nós — pronto

Todos os campos da seção (dono, estado, CPU, núcleos, RAM, GPU, VRAM, disco,
sistema, versão do agente, região, rede, último pulso, reputação,
disponibilidade) e os estados OFFLINE, ONLINE (IDLE ou BUSY), PAUSED,
QUARANTINED, MAINTENANCE e BANNED, mais UNSTABLE (seção 32) e REMOVED.

### 9. Agente — pronto

Autentica com a chave do worker (cada pedido assinado, janela de 5 minutos,
assinatura repetida recusada), recebe e valida unidades (a especificação é
decodificada e conferida pelo motor), reserva memória, limita CPU (linhas e
fatia do ULTRAX), executa, coleta telemetria, entrega resultado com registro
de prova assinado, libera a memória, recebe ordem de parar (kill switch) e
avisa quando precisa atualizar. Não executa nada fora dos motores.

O agente não refaz a conta para conferir os motores que a plataforma confere
por concordância (isso só dobraria o custo do fornecedor); a matriz continua
conferida aqui também, porque Freivalds é barato e pega erro de GPU.

### 10 e 11. Segurança do nó e HYURAX Node OS — parcial / depois da V1

O isolamento da V1 é o mais forte possível para o que ela roda: **não há
código de fora**. Memória reservada antes de cada unidade, linhas limitadas,
prazo, sem disco e sem rede dentro do motor. O que a seção 10 descreve para
código arbitrário (VM, contêiner, política de rede) só passa a ser necessário
quando a plataforma aceitar runtime de fora (Python, renderização), e é o
trabalho do **HYURAX Node OS** (seção 11), que o próprio documento põe no
futuro.

### 12. Agendador — pronto

Tarefa criada → validação → requisitos → nós disponíveis → ordem → escolha →
despacho (`servico/agendador.rs`). Considera motor, GPU, memória, núcleos em
paralelo, região, carga atual, reputação e estabilidade do pulso, e **não
escolhe o mais forte**: um nó bem mais adequado recebe primeiro por até 10 s.
Justiça entre clientes (quem foi atendido há mais tempo vem primeiro), nunca
entrega a tarefa ao nó do próprio cliente, e as cópias de uma unidade vão para
donos diferentes. **Latência** não entra (para trabalho em lote não faz
diferença; entra no Game Cloud, seção 53).

### 13 e 14. Tarefas e ciclo de vida — pronto

Todos os campos e os estados CREATED, QUEUED, SCHEDULED, DISPATCHED, RUNNING,
VERIFYING, COMPLETED, FAILED, CANCELLED, TIMEOUT e REJECTED. O ciclo da seção
14 é exatamente o do código: criar → validar → reservar → fila → agendador →
despacho → execução → telemetria → resultado → conferência → concluir →
cobrar → pagar.

### 15 e 16. Formato e política de workload — pronto

O pedido é JSON (`tipo`, `tamanho`, `passos`, `parametros`, `unidades`,
`paralelo`, `gpu`, `prazo_s`, `copias`, `descricao`, `regiao`) e cada tarefa
declara a política da seção 16 (CPU, RAM, GPU, armazenamento, rede, prazo,
runtime). Tetos de unidades, memória e operações por unidade, prazo, núcleos
em paralelo e cópias ficam na configuração do administrador.

**Limite desta versão:** os runtimes são os oito motores do ULTRAX
(matrizes, mochila, difusão, IA de solubilidade, genética de populações,
melhoramento de culturas, rotas, triagem de moléculas). Dados enviados pelo
cliente (por exemplo, as cidades das suas entregas) e runtimes de fora
(Python, renderização, compilação) são a Fase 2: cada um precisa de
especificação no gabarito Python e de isolamento à altura.

### 17. Armazenamento — pronto

Entrada: nasce da especificação e da semente (nada a guardar). Execução: só
na memória do fornecedor, liberada no fim. Saída: guardada no servidor,
conferida pelo hash do diário a cada download, apagada 30 dias depois do fim
da tarefa. Um cliente nunca lê arquivo de outro.

### 18 e 19. Conferência e reputação — pronto

Sempre: registro de prova assinado, entrada, hash, tamanho e operações pelo
modelo de custo de cada motor. Matriz: Freivalds no servidor. Rotas e
triagem: conferência estrutural no servidor. Os outros: concordância entre
nós de donos diferentes, **por amostragem que depende da reputação** (10% a
100%; tudo nas 10 primeiras entregas conferidas de um nó novo). Cópias só
concordam quando resultado e operações batem; discordância chama um terceiro
nó, e a minoria perde reputação. Pontuação de 0 a 100 (nó novo = 70) e
quarentena abaixo de 50 ou com 3 resultados errados. Regra em
`reference/hyurax/plataforma.py`, igual no Rust por vetor.

Quando não há outro fornecedor online, a unidade fecha só pela integridade
(e isso aparece no painel como "só integridade"): não faz sentido segurar a
tarefa do cliente esperando um conferente que não existe.

### 20 a 23. Cobrança, créditos, reserva e repasse — pronto

Real e dólar, cada conta na sua moeda. Livro em **partida dobrada**: cada
lançamento soma zero, e o painel do administrador mostra se o livro fecha. A
reserva sai do saldo na criação pelo teto de operações; no fim, cobra-se só
o conferido, e o resto volta. Divisão configurável no servidor (padrão 70% do
fornecedor, 30% da plataforma, seção 23), nunca no frontend; as conferências
por amostragem são custo da plataforma, não do cliente.

### 24 e 25. Mercado e preços — pronto

Preço fixo por categoria (a própria seção 25 deixa o dinâmico para depois):
CPU R$ 0,50/h por 4 núcleos, RAM R$ 0,10 por GB-hora, GPU R$ 3,00/h, e a
tabela em dólar. **Cobra-se trabalho conferido**, em horas de um núcleo de
referência (1 bilhão de operações por segundo) pelas operações do modelo de
custo de cada motor, e não o tempo que o fornecedor diz ter gastado: máquina
lenta não cobra mais por isso, e ninguém infla a conta com relógio.

Tarefa minúscula (menos de um centavo) cobra um centavo; nesse caso a parte
do fornecedor arredonda para zero.

### 26 e 27. Pagamento e saque — parcial (depende do dono)

- **Entrada:** pedido de pagamento com código (`HYX-P-000123`), Pix manual na
  chave da empresa (configurada pelo administrador) e confirmação no painel
  com a referência do banco e a taxa. Em modo de teste, o pagamento confirma
  na hora e todas as telas avisam. O registro guarda transação, valor,
  estado, usuário, referência externa e taxa.
- **Saída:** ganho → retenção (7 dias, configurável) → liberação (sem conta
  bloqueada nem ganho segurado) → saque pedido com chave Pix → aprovado →
  pago pela empresa, à mão, com a referência.
- **Falta:** o provedor integrado (aviso do provedor conferido antes de
  creditar, nota fiscal) e o repasse automático. Os dois dependem da empresa
  (`docs/MONETIZACAO.md`, Fase M1) e o repasse a terceiros, de parecer
  jurídico (Fase M3).

### 28. Antifraude — pronto (regras), parcial (análise)

Muitas contas do mesmo IP, senha errada em sequência, criação excessiva de
tarefas, pagamentos pendentes demais, resultado falso, nó do próprio cliente,
mesmo hardware em contas diferentes, pedido de agente repetido, estorno
(bloqueia a conta negativa e segura os ganhos das tarefas dela) e a varredura
de padrões a cada 10 minutos (fornecedor que só ganha de um cliente do mesmo
IP). Tudo vira alerta no painel do administrador. Modelo estatístico de
fraude é para quando houver volume.

### 29 e 30. API e chaves — pronto

Tudo em `/api/v1/` (auth, conta, chaves, saldo, extrato, faturas, pagamentos,
tarefas, resultados, nós, ganhos, saques, mercado, catálogo, agente,
administração). A **HYX_API_KEY** tem escopos (`tarefas:ler`,
`tarefas:criar`, `saldo:ler`), limite por minuto, revogação e rotação; só o
hash fica guardado, e a chave nunca aparece em log nem no diário.

### 31 e 32. Telemetria e pulso — pronto

CPU, GPU, RAM, VRAM, temperatura, potência (estimada pelos watts por núcleo
do dono), rede, progresso de cada unidade, tempo de execução, versão e saúde.
Sem dado pessoal (nada de nome de máquina, usuário ou IP local). Pulso a cada
5 s; instável depois de 20 s, offline depois de 60 s; nó offline sai da
escolha. **Temperatura:** o Windows não informa sem permissão de
administrador, e aí o painel diz "o computador não informa este número" em
vez de inventar.

### 33. Monitoramento — pronto

`/saude` para o provedor de hospedagem; no painel: pedidos, erros, latência
(p50 e p95), fila, nós com falha, última cópia do diário e o selo da corrente.

### 34 e 35. Banco e fila — feito de outro jeito, de propósito

O documento recomenda banco relacional. A V1 usa um **diário só de
acréscimo**, com cada evento numa linha e hash encadeado, e o estado é o
diário relido em ordem (`diario.rs`, `estado.rs`). Por quê:

- é o **registro de auditoria** (seção 42) e o **sistema de eventos** (seção
  36) ao mesmo tempo, à prova de adulteração (linha mudada ou apagada no meio
  não abre);
- **recuperação** (seção 44) é reler: o teste derruba o servidor e confere
  que o estado volta byte a byte;
- não puxa banco de dados com código em C, que o projeto não aceita.

Fila e cache ficam na memória do processo. O limite é escala: um servidor só.
Quando o volume pedir, o diário vira a fonte para popular um banco
relacional (seção 54).

### 36. Eventos — pronto

Todos os da seção (USER_CREATED, NODE_ONLINE, NODE_OFFLINE, TASK_CREATED,
TASK_ASSIGNED, TASK_STARTED, TASK_COMPLETED, TASK_FAILED, RESULT_VERIFIED,
PAYMENT_COMPLETED, PAYOUT_CREATED, SECURITY_ALERT) e outros 30.

### 37 e 38. Administração e kill switch — pronto

Bloquear e desbloquear usuário, quarentena, manutenção, banimento e liberação
de nó, cancelar tarefa, registro de auditoria com filtro, pagamentos,
saques, tarefas, preços, comissão, retenção, limites, Pix, motores, versão
mínima do agente, alertas, livro, cópia do diário e ajuste manual de saldo
(com motivo). Kill switch nos quatro níveis: **KILL NODE**, **KILL TASK**,
**DISABLE WORKLOAD TYPE** e **EMERGENCY COMPUTE STOP** (nada é despachado e
tudo que roda é cancelado em todos os nós no próximo pulso).

### 39 a 42. Confiança zero, rede, segredos e auditoria — pronto

Cliente, nó, carga e rede são tratados como não confiáveis: sessão ou chave
em todo pedido, assinatura em todo pedido do agente, conferência em todo
resultado. CSRF barrado por cabeçalho próprio e origem; CSP estrita nas
páginas. O motor não tem rede (seção 40). Nenhum segredo entra no diário (o
teste procura a senha, o token, a chave de API e o código de vínculo no
arquivo).

### 43 e 44. Cópia e recuperação — pronto

Cópia conferida do diário a cada 6 horas (as 28 mais novas ficam), download
pelo administrador, `hyurax-plataforma copia` e `conferir` na linha de
comando. Queda no meio da escrita corta só a última linha incompleta. Unidade
cujo prazo venceu volta para a fila sozinha (até 4 tentativas).

### 45 e 46. CI/CD e atualização do agente — parcial

Testes, clippy, vetores e auditoria de dependências rodam a cada envio; o
lançamento compila e publica. **Falta um ambiente de homologação (staging)**,
que depende do mesmo servidor da hospedagem. O agente se atualiza com o
programa (atualização assinada, conferida antes de instalar); a plataforma
recusa trabalho a agente abaixo da versão mínima. Volta automática para a
versão anterior não existe ainda.

### 47 e 48. Workloads da V1 e o que fica fora — pronto

CPU Compute (sete motores), GPU Compute (matrizes pelo WebGL 2 do programa
com janela) e Rendering fica para quando houver runtime seguro, como o
documento pede. Fora da V1, como manda a seção 48: token, blockchain,
mineração e staking não entram na plataforma (continuam no programa, na rede
de teste, que é outro produto); Game Cloud, Commerce AI e app de celular
completo também ficam de fora.

### 49 a 53. Roteiro — Fase 1 pronta

A Fase 1 (MVP) está inteira no código. As Fases 2 a 5 começam depois do
primeiro processamento pago.

### 54. Escala — como o documento pede

Começa simples: um binário com frontend, backend, diário, fila e os agentes.
As peças estão em módulos separados (`servico/contas.rs`, `nos.rs`,
`agendador.rs`, `tarefas.rs`, `financeiro.rs`, `admin.rs`, `fraude.rs`) para
virarem serviços quando o volume justificar.

### 55 a 58. Métricas, metas e economia — pronto (as métricas)

O painel do administrador mostra nós ativos, horas de computação, tarefas e
taxa de sucesso; GMV, receita, take rate, margem bruta, gasto médio, ganhos
dos fornecedores e caixa; disponibilidade, latência, falha de execução e
falha de verificação. Atendimento ao cliente (seção 55, "support tickets")
ainda não tem canal integrado.

### 59 a 61. Experiência e UX — pronto

O cliente pensa em "quanto computação eu preciso e quanto custa": a
estimativa aparece antes de criar, com tempo, capacidade online e avisos. O
fornecedor: cadastrar → instalar o Hyurax → colar o código → escolher limites
→ ligar → receber → sacar.

### 62 a 66. Arquitetura final, pronto, regras — pronto

A regra 64 (simplificar sem destruir) foi seguida: a plataforma reaproveita
os motores do ULTRAX, o worker com limites e prova assinada, a verificação e
a reputação que já existiam; nada útil foi apagado. A regra 65 (o que não
ajuda a comprar, fornecer, proteger, cobrar ou executar fica fora) decidiu o
que entrou.

## O que só o dono pode fazer para a primeira venda

1. **Servidor com disco que não se apaga.** O plano grátis do Render apaga o
   disco a cada reinício, e o diário é o banco. Um servidor pequeno (Render
   com disco, uma VPS) resolve. O guia está em
   [`OPERAR-A-PLATAFORMA.md`](OPERAR-A-PLATAFORMA.md).
2. **A empresa:** atividade do CNPJ (processamento de dados), conta PJ,
   contador e termos com advogado (`docs/MONETIZACAO.md`, Fase M1).
3. **A chave Pix da empresa** no painel (Configuração). Sem ela, o botão de
   pagamento avisa que a venda ainda não abriu.
4. **Anonimato:** vender pela internet obriga a mostrar CNPJ e endereço na
   página. Abrir a venda encerra o anonimato do projeto nessa ponta.
