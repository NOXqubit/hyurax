# Auditoria do programa de PC contra o Documento Mestre

Fase 1 do roteiro do [Documento Mestre](DOCUMENTO-MESTRE.md), feita em 02 e
03/10/2026, seguida das correções das Fases 2 (núcleo PC) e 3 (cibersegurança)
que cabiam no programa de PC.

## Como foi feita

1. **Leitura do código por área**, contra as seções do documento: execução e
   recuperação (§3, §5, §15), verificação (§6), rede e identidade (§12),
   segurança local (§3, §12, §17), telemetria (§7), blockchain (§13),
   relatórios (§18), operação e interface (§15, fase 4), economia (§8–§11,
   §14) e dependências e testes. Quatro áreas foram lidas por auditores
   automáticos em paralelo; as outras seis, e a conferência de todos os
   defeitos, foram feitas à mão.
2. **Nenhum defeito foi corrigido sem antes ser conferido no código** (o
   caminho até a falha seguido de verdade). Os 40 defeitos alegados se
   confirmaram; a gravidade de alguns foi revista.
3. **Medidas desta máquina** (Atom x5-Z8350, 3,4 GB): `cargo test --workspace`
   (338 testes, 0 falhas no fim), os 18 cenários de rede com sockets
   (`--ignored`, todos passam), `clippy -D warnings` limpo, 177 testes do
   gabarito Python, instalação limpa (30 conferências) e teste de execução
   longa (`scripts/teste-longo.ps1`).

Legenda do estado: **REAL** (implementado e exercitado por teste ou uso),
**PARCIAL**, **SIMULADO** (roda de verdade sobre dado gerado, ou só numa
máquina), **AUSENTE**, **FORA DO PC** (backend, web, mobile, economia).

---

## §3 e §15 — núcleo de PC

| Funcionalidade | Estado | O que funciona e como foi testado | O que falta (risco) | Como comprovar |
|---|---|---|---|---|
| Registro e identidade do dispositivo | REAL | `no.chave` (X25519) gerada uma vez; o nó prova a identidade no Noise XX. Teste: `identidade_do_no_e_provada_na_cifra` | rotação e revogação (§12); cifra em repouso | trocar `no.chave` e ver a identidade nova na tela |
| Autenticação e comunicação segura | REAL | Noise XX entre nós; API local com chave de sessão, Host e Origin. Testes: `intermediario_*`, `tests/api_local.rs` | — | `cargo test -p hyurax-nucleo --test api_local` |
| Recebimento e validação de tarefas | REAL | só especificação viaja; faixas, memória, prazo e semente conferidos antes de executar. Testes: `pedido_com_especificacao_invalida_e_recusado_na_leitura`, `pedido_com_semente_que_nao_e_da_unidade_e_recusado` | processo separado com teto de memória do sistema (Job Object) | — |
| Gerenciamento de work units | REAL | ciclo de vida fechado em tabela; JOB pausado e sem orçamento fecham; cancelar para as unidades. Testes: `tarefa.rs`, `job_pausado_fecha_*`, `job_cancelado_nao_quer_mais_*` | — | — |
| Execução por CPU/GPU | PARCIAL | 8 motores na CPU; matriz na GPU (WebGL 2) conferida por Freivalds. Testes: `job_na_cpu_pelas_linhas_do_worker_ate_concluir`, `gpu_honesta_e_creditada_e_adulterada_e_recusada`; GPU real usada na janela | GPU nos outros motores; backend nativo | — |
| Checkpoints e recuperação | PARCIAL | checkpoint do JOB com SHA-512 e cópia anterior, gravado com `sync_all` e também ao fechar; linha que cai é reiniciada e devolve a unidade. Testes: `retoma_do_checkpoint_*`, `encerrar_grava_o_checkpoint_*`, `linha_que_cai_solta_a_memoria_e_a_tarefa` | checkpoint dentro de uma unidade longa (ela recomeça do zero) | — |
| Envio e recebimento de resultados | SIMULADO | compromisso, revelação e maioria entre nós; testado com mensagens injetadas, sem socket (`ciencia/rede.rs`) | ensaio entre máquinas | dois PCs na mesma rede, JOB de nível 3 |
| Verificação e evidências | REAL | registro de prova assinado, veredito assinado, auditoria do histórico (agora com INPUT_HASH), `consenso.jsonl` com as assinaturas dos votos. Testes: `prova.rs`, `auditoria_pega_*` | âncora externa de tempo | `hyurax-no ultrax auditar` |
| Telemetria real de hardware | REAL | CPU, RAM, GPU 3D/cálculo e memória da GPU pelos contadores do Windows, com origem na tela | temperatura (PENDENTE), bytes da rede | — |
| Logs, falhas e reconexão | REAL | registro com teto e rotação; falha de código registrada; reconexão com espera crescente | — | — |
| Atualizações seguras e assinadas | REAL | manifesto assinado (Ed25519) pela chave de lançamento; instalador conferido por tamanho e SHA-512 antes de abrir. Testes: `atualizacao.rs`; assinatura e conferência pela linha de comando | busca pela internet não testada sem uma Release publicada; Authenticode (certificado) | publicar 1.0.1 assinada e ver a 1.0.0 oferecer a atualização |
| Controle de recursos e preferências | REAL | linhas, limite por linha, teto de memória (unidade que não cabe é recusada na submissão), GPU com fatia; tudo em `ajustes.txt`. Testes: `limite_de_cpu_*`, `teto_de_memoria_*`, `unidade_maior_que_o_teto_*` | teto de memória do sistema operacional | — |
| Isolamento de tarefas remotas | PARCIAL | nenhum código de fora roda; limites por par | processo separado | — |
| UI sem lógica crítica (§15) | REAL | o núcleo roda sem a tela (`hyurax-no painel`, `no`); a tela só consome a API | — | — |
| Testar tarefas longas (§15) | REAL | `scripts/teste-longo.ps1`: memória, handles e threads medidos a cada 30 s, auditoria no fim. 03/10, 30 min, 2 linhas, build debug: memória privada 7,8 → 7,5 MiB, 99 handles e 6 threads do começo ao fim, 382 registros, 381 assinaturas e vereditos válidos, 20 de 20 refeitas iguais, placar conferido | rodar horas, não minutos, antes de cada lançamento | ver o CSV do teste |

## §4 e §6 — verificação computacional

| Funcionalidade | Estado | O que funciona e como foi testado | O que falta | Como comprovar |
|---|---|---|---|---|
| Integridade (hashes, assinaturas) | REAL | INPUT_HASH, RESULT_HASH, registro assinado, raiz de Merkle das unidades no relatório | assinatura dos arquivos do JOB | — |
| Reexecução | REAL | cada unidade é conferida antes de contar, pelo método adequado ao motor (Freivalds, ótimo exato, recomputação) | — | — |
| Múltiplos nós | SIMULADO | maioria + conferência local da maioria; conluio pego. Testado em processo | ensaio entre máquinas; compromisso assinado pelo worker | — |
| Verificação independente | PARCIAL | a CPU confere a GPU por outro algoritmo | nó verificador dedicado; segunda implementação | — |
| Reprodutibilidade | PARCIAL | semente por unidade, `hyurax-no ciencia refazer`, versão do programa por unidade no registro e no relatório | versão por motor; hash dos dados embutidos | — |
| Cálculo × validação científica | REAL | aviso no relatório, incertezas por motor (o erro da IA é o medido no JOB, não um número fixo), termos de uso | — | — |

## §5 — distribuição e checkpoints

| Funcionalidade | Estado | Notas |
|---|---|---|
| Divisão em unidades | REAL | semente derivada por índice; intervalos de progresso |
| Filas e agendamento | REAL | no máximo 8 em voo por JOB; JOB tem prioridade sobre o LAB |
| Seleção de nós compatíveis | SIMULADO | tipo, memória, ocupação, frescor da oferta, reputação local, par indisponível fica de fora 5 min |
| Balanceamento | PARCIAL | entre linhas e GPU; entre nós, só por ocupação |
| Persistência e retomada | REAL | checkpoint, gravação ao fechar |
| Reexecução seletiva | PARCIAL | `ciencia refazer` de uma unidade; reabrir unidades com falha pela tela: AUSENTE |
| Controle de duplicidade | REAL | resultado repetido não entra; CSV com uma linha por unidade |
| Nós desconectados | PARCIAL | prazo devolve a unidade; faltar voto não conta como falha |

## §7 — telemetria e visualização 3D

| Item | Estado | Onde |
|---|---|---|
| ID do job, unidade e nó | REAL | ULTRAX, Ciência, histórico |
| Tipo e estado; progresso real | REAL | ULTRAX (operações feitas / total) |
| CPU, GPU, VRAM, RAM | REAL | Visão geral; a memória "reservada" é ESTIMADO (modelo de custo), e a tela diz isso |
| Temperatura e energia | PENDENTE / ESTIMADO | sem leitura confiável sem administrador; energia pela fórmula na tela |
| Throughput | PARCIAL | ritmo da GPU e da mineração; por tarefa de CPU, no histórico de eventos |
| Checkpoints e hashes | PARCIAL | hashes na inspeção da unidade e no histórico; checkpoint não aparece na tela |
| Tentativas, falhas, reexecuções | REAL | placar e JOB |
| Resultado da verificação | REAL | linha de leitura, histórico, inspeção |
| Histórico e reprodução temporal | REAL | Ciência → Histórico e inspeção: reprodução pelos eventos gravados, na ordem e no ritmo em que aconteceram |
| Inspeção individual | REAL (computações) / AUSENTE (nós) | registro completo de cada unidade; página por nó: AUSENTE |
| Visualização 3D de eventos reais | REAL | a cena desenha só amostras dos motores |

## §12 — segurança e identidade

Ver o [modelo de ameaças](MODELO-DE-AMEACAS.md), que lista cada defesa com o
teste que a prova e o risco que sobra.

| Item | Estado |
|---|---|
| Identidade criptográfica; comunicação autenticada | REAL |
| Autenticação e autorização (API local) | REAL (chave de sessão) |
| Rotação e revogação de credenciais | AUSENTE |
| Proteção de chaves privadas | PARCIAL (carteira cifrada; `no.chave` e `seguranca.txt` em texto, só o dono lê) |
| Proteção contra replay | REAL (Noise; oferta com frescor; código de 6 dígitos não vale duas vezes) |
| Rate limiting | PARCIAL (tetos de conexão, fila, livro e código; balde por par: AUSENTE) |
| Detecção de Sybil | AUSENTE (banimento por identidade e IP; nenhum mecanismo isolado resolve, como o documento diz) |
| Integridade de executáveis; atualizações assinadas | PARCIAL (atualização assinada REAL; Authenticode PENDENTE) |
| Isolamento de tarefas | PARCIAL |
| Logs e resposta a incidentes | PARCIAL (registro; procedimento de incidente: `SECURITY.md` só diz como relatar) |

## §13 — blockchain e carteira

Componentes validados byte a byte contra o gabarito Python (vetores) e
testados no Rust: carteira e chaves, transações (nonce por conta, magic da
rede), estado contábil, consenso, emissão e maturidade, registro de blocos,
prova útil por Freivalds. A cadeia passou a ser gravada com `sync_all`. A
recompensa do bloco **não** depende do trabalho científico (separação pedida
pelo §13); integrar os dois é mudança de consenso, PENDENTE (SPEC-02). Nada
disso está pronto para produção: a rede é de **teste**.

## §17 e §18 — instalador e relatórios

- **Instalador:** sem administrador, manifesto do que instalou, desinstalação
  que só apaga o listado, atualização no lugar; testado pela instalação limpa
  (30 conferências). Distribuído pelas Releases do GitHub com SHA-256 e, para
  atualizações, manifesto assinado. Authenticode: PENDENTE.
- **Relatórios:** JSON, CSV e PDF com metadados, parâmetros, motor, nível de
  verificação atingido (lido do registro por unidade), versões do programa
  que calcularam, evidências (resumo aditivo, raiz de Merkle, `consenso.jsonl`),
  métricas, incertezas e o comando para refazer. Dados brutos de cada
  resultado: AUSENTE no relatório (só os hashes).

## §8 a §11, §14, §16, §17 (cliente) — fora do programa de PC

| Item | Estado |
|---|---|
| Créditos de computação, orçamento por JOB, Work Score | REAL, separados do HYX, sem liquidação |
| Reputação local de workers | PARCIAL (só na memória) |
| Nós verificados (3 meses), Gold Score | AUSENTE |
| Marketplace, preços, pagamentos, disputas | FORA DO PC (a troca de unidades entre nós existe, sem preço) |
| Contas, backend, API externa, painel web, aplicativo móvel | FORA DO PC |
| HYX Index | só documento, sem lastro (`docs/HYX-INDEX.md`) |
| Promessa de rentabilidade ou valorização | nenhuma encontrada em README, site, docs, termos e tela |

O §20 do documento cita "registro de dispositivos, P2P simulado, marketplace
de tarefas, execução prolongada". Conferido: a identidade do nó existe (não
há cadastro central); o P2P é **real** (Noise, sockets, testado entre
processos), não simulado; o "marketplace de tarefas" é a troca de unidades
entre nós, sem preço; execução prolongada passou a ter teste
(`teste-longo.ps1`).

## Dependências e testes

- Todo crate proíbe `unsafe`; `clippy` nega `unwrap`, `expect`, `panic` e
  indexação direta. Nenhuma dependência nova nesta auditoria.
- Não há `cargo-audit` nem `cargo-deny` no CI: **PENDENTE** (adicionar exige
  baixar a ferramenta).
- Sem teste automático: a janela (`hyurax-app`) e o JavaScript da interface
  (conferidos à mão no navegador, com dados reais); o instalador é testado
  pelo `testar-instalacao.ps1`.
- O CI de testes roda só no Linux; o Windows é testado no lançamento
  (montagem e instalação limpa).

---

## Defeitos

Todos confirmados no código antes da correção. Commits: `fcbfed2` (segurança
local e execução), `c159b31` (rede, ciência entre nós, relatório), `7e28b7c`
(atualização segura).

| # | Defeito | Gravidade | Situação |
|---|---|---|---|
| 1 | Unidade maior que o teto de memória girava sem fim | alta | corrigido |
| 2 | JOB pausado ou sem orçamento nunca concluía | média | corrigido |
| 3 | Cancelar o JOB não parava as unidades | média | corrigido |
| 4 | Fechar não gravava o checkpoint | média | corrigido |
| 5 | Mapa de pedidos entre nós crescia sem limite | média | corrigido |
| 6 | Benchmark deixava threads vivas | baixa | corrigido |
| 7 | Queda duplicava linha no CSV | baixa | corrigido |
| 8 | Operações declaradas por worker remoto inflavam créditos | alta | corrigido |
| 9 | Oferta sem frescor nem vínculo ao par | alta | corrigido (compromisso assinado: PENDENTE) |
| 10 | Conferência remota na thread do par, com trava presa | média | parcial: fora da trava; ainda na thread do par |
| 11 | Pedido com semente falsa sujava a auditoria | média | corrigido |
| 12 | Assinaturas dos votos remotos descartadas | média | corrigido (`consenso.jsonl`) |
| 13 | Relatório nunca mostrava nível 3 | média | corrigido |
| 14 | Relatório gravava a versão de quem gera, não de quem calculou | média | corrigido |
| 15 | Erro da IA fixo no relatório | média | corrigido |
| 16 | Auditoria não conferia o INPUT_HASH | baixa | corrigido |
| 17 | Fila de saída P2P sem limite e escrita sem prazo | alta | corrigido |
| 18 | Sem teto de conexões; aperto gotejado | alta | corrigido |
| 19 | Livro de endereços sem teto; discagem de tudo | alta | corrigido |
| 20 | Compromisso aceitava qualquer worker | alta | corrigido |
| 21 | Replay de oferta alheia | média | corrigido |
| 22 | Recusa ou sumiço derrubava a unidade sem pesar no par | alta | corrigido |
| 23 | Pedidos de pares diferentes misturados | média | corrigido (pedido repetido recusado) |
| 24 | Semente por nome rediscada a cada 2 s | média | corrigido |
| 25 | Resumo das outras máquinas sem autenticação | média | PENDENTE |
| 26 | `no.chave` em texto; permissão só depois | média | parcial: 0600 desde a criação e `sync_all`; cifra em repouso PENDENTE |
| 27 | Sem banimento; órfão forjado custa Argon2id com a trava | alta | parcial: banimento feito; conferência fora da trava PENDENTE |
| 28 | "Sincronizado" falso para sempre | média | corrigido |
| 29 | HEADERS não validado | baixa | corrigido (encadeamento) |
| 30 | API lia o pedido inteiro antes de recusar a rede local | alta | corrigido |
| 31 | GET sem conferir Host (DNS rebinding) | média | corrigido |
| 32 | Fluxo de eventos ignorava o cadeado | média | corrigido |
| 33 | Código de 6 dígitos sem limite e reaproveitável | média | corrigido |
| 34 | API local sem token | média | corrigido (chave de sessão) |
| 35 | Carteira antiga enviava com qualquer senha | média | corrigido |
| 36 | Fluxos sem vaga para a janela local | média | corrigido |
| 37 | Segredos gravados sem `fsync` | média | corrigido (e a cadeia também) |
| 38 | `seguranca.txt` com permissão padrão | baixa | corrigido |
| 39 | PowerShell montado com o caminho no texto do script | baixa | corrigido |
| 40 | Segredos não zerados da memória | baixa | PENDENTE |

Achados desta revisão, além dos 40: a cadeia sem `fsync` (corrigido); a
memória reservada rotulada como REAL (corrigido para ESTIMADO); reprodução
temporal e inspeção ausentes na interface 1.0 (implementado); atualização
assinada ausente (implementado); linha do worker sem supervisão (corrigido);
nenhum teste de JOB pelas threads do worker (criado).

## O que fica para depois do PC, na ordem do roadmap

1. Compromisso assinado pelo worker, conferência remota fora da thread do par,
   reputação gravada em disco, ensaio entre máquinas (fase 6, rede).
2. Cifra em repouso de `no.chave` e `seguranca.txt`, zeroização, rotação da
   identidade, assinatura do resumo das máquinas (fase 3, restante).
3. Authenticode do `.exe` (depende do certificado do dono).
4. `cargo-audit` no CI.
5. Backend, contas, marketplace, mobile e economia (fases 5, 7, 8 e 9), só
   depois do núcleo de PC estável, como o documento manda.
