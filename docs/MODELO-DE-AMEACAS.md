# Hyurax / Ultrax 1.0: modelo de ameaças do programa de PC

Fase 3 do roteiro do [Documento Mestre](DOCUMENTO-MESTRE.md) (§12). Escrito
contra o código de 03/10/2026; cada defesa aponta para onde está e como foi
testada. O que não está defendido aparece como **risco residual**, sem
enfeite.

## O que proteger

| Ativo | Onde mora | Por que importa |
|---|---|---|
| Segredo da carteira | `%APPDATA%\Hyurax\carteira.txt`, cifrado (Argon2id + ChaCha20-Poly1305) | gasta o saldo (de teste, hoje; de verdade, um dia) |
| Segredo do segundo fator | `%APPDATA%\Hyurax\seguranca.txt` | gera os códigos de 6 dígitos |
| Identidade do nó | `%APPDATA%\Hyurax\no.chave` | quem a tem fala na rede como este nó e assina registros de prova como este worker |
| Chave de lançamento | fora de qualquer repositório (`D:\chaves-hyurax`) | quem a tem publica "atualização" que todo programa instalado aceita |
| Cadeia local | `%LOCALAPPDATA%\Hyurax\<rede>.cadeia` | blocos minerados e saldo visto |
| JOBs, relatórios e evidências | `%LOCALAPPDATA%\Hyurax\ciencia\` | o que o cliente recebe como resultado |
| CPU, memória e energia do dono | o computador | limites escolhidos pelo dono |
| Privacidade | endereço, saldo, IP, registros | o que o painel mostra |

## Quem ataca, e o que existe contra cada um

### A1 — uma página aberta no navegador do dono

Quer ler o painel ou mandar comando para `127.0.0.1:8800`.

- **Comando** exige a chave de sessão do painel (32 bytes por abertura, só na
  pasta de configuração do usuário, entregue à janela pelo `#chave=`), `Host`
  local e `Origin` local (`api/mod.rs`). Teste: `tests/api_local.rs`.
- **DNS rebinding:** todo pedido precisa de `Host` que seja `localhost` ou IP
  escrito por extenso; domínio é recusado, mesmo com a chave. Teste: idem.
- **Leitura** completa (`/estado`, `/fluxo`, `/ciencia`) também exige a chave.

### A2 — outro programa, ou outra conta do Windows, no mesmo computador

- Outra **conta** do Windows não lê `%APPDATA%` do dono, então não tem a chave
  de sessão: não manda comando nem lê o estado. Teste: `tests/api_local.rs`.
- O **código de 6 dígitos** tem espera crescente depois de 5 erros, não vale
  duas vezes, e cada erro vai para o registro (`carteira/seguranca.rs`).
  Teste: `codigo_nao_vale_duas_vezes_e_erros_seguidos_fazem_esperar`.
- **Risco residual:** um programa rodando **na conta do dono** lê os mesmos
  arquivos que o Hyurax (a chave de sessão, a carteira cifrada, o segundo
  fator, a identidade do nó). Contra ele valem a senha da carteira (Argon2id)
  e nada mais. Cifra em repouso de `no.chave` e `seguranca.txt` (DPAPI) e
  zeroização dos segredos em memória: **PENDENTE**.

### A3 — um aparelho na mesma rede local

- Com "ver no celular" **desligado**, a conexão de fora é recusada antes de
  qualquer byte ser lido (`api/mod.rs`, laço de `abrir`).
- Ligado, o aparelho **só lê** (nunca manda comando), com teto de conexões e
  de fluxos por aparelho, e vagas de fluxo reservadas para a janela local.
- O resumo das outras máquinas do dono (`maquinas.rs`) vai com um desafio
  aleatório e volta assinado (Ed25519) pela chave de worker de quem
  respondeu, sobre o desafio e os bytes exatos do corpo. A chave fica fixada
  na primeira resposta (`maquinas-conhecidas.txt`): um aparelho que tome o IP
  de uma delas, ou repita uma resposta velha, aparece como erro. Testes:
  `assinatura_do_resumo_e_chave_fixada`, `tests/api_local.rs`.
- **Risco residual:** a primeira resposta é aceita sem prova de quem é (a
  chave é fixada nela); a lista vem por HTTP, então quem escuta a rede local
  lê os números.

### A4 — um par malicioso na internet (porta 8790)

- **Canal** cifrado e autenticado (Noise XX); o par prova a identidade.
  Testes: `intermediario_so_ve_bytes_cifrados`, `intermediario_que_altera_derruba_a_conexao`.
- **Recursos:** teto de conexões de entrada (total e por IP) antes de abrir a
  thread; prazo absoluto no aperto de mão e em cada mensagem (gotejar não
  segura a thread); fila de saída com teto de mensagens e de bytes; toda
  escrita com prazo; livro de endereços com teto; discagem com prazo e espera
  crescente (`hyurax-net/src/servidor.rs`). Testes: `aperto_gotejado_cai_no_prazo`,
  `quadro_gigante_nao_estoura_a_memoria`, `lixo_puro_nao_vira_par`.
- **Consenso:** todo bloco e toda transação são validados contra a spec;
  órfão forjado e bloco sem prova derrubam o par, e quem mostrou malícia fica
  banido uma hora (identidade e IP). Testes: `orfao_forjado_derruba_quem_mandou`,
  `identidade_que_mostrou_malicia_fica_banida`, `bloco_forjado_sem_prova_e_recusado`.
- A prova de trabalho (Argon2id) de um bloco novo é conferida **fora** da
  trava do nó, na thread do par que mandou; o recibo (`conferir_pow`) só vale
  para aquele cabeçalho, e o resto da validação continua dentro da trava.
  Bloco que já tenho não custa conta nenhuma. Testes: `pow_fora_da_trava.rs`.
- Teto de conexões de entrada por faixa de endereços públicos (IPv4 /24,
  IPv6 /48), além do teto por IP. Teste: `faixa_agrupa_vizinhos_publicos_e_ignora_a_rede_local`.
- Rotação da identidade: `hyurax-no identidade girar` (com o programa
  fechado) troca a identidade e guarda a antiga ao lado. Teste:
  `girar_troca_a_identidade_e_guarda_a_antiga`.
- **Risco residual:** Sybil com IPs espalhados passa pelos tetos (encarecem,
  não impedem; a reputação local cobra trabalho verificado de cada
  identidade, ver [REPUTACAO.md](REPUTACAO.md)); não existe revogação
  anunciada na rede: quem copiou uma identidade antiga ainda fala como ela.
  **PENDENTE.**

### A4b — a malha (pontes, alcance, rede local, pacotes)

Superfície nova de `docs/HYURAX-MALHA.md`:

- **Ponte.** Copia bytes que são Noise de ponta a ponta: não lê, não altera
  (alterar derruba a cifra) e não se passa pelo alvo (quem disca exige a
  identidade do alvo no aperto de mão). Reserva só com token dito dentro da
  cifra: ninguém sequestra a vaga de outro. Tetos: 64 reservas (2 por nó),
  16 circuitos (4 por alvo), 256 MiB e 5 min de silêncio por circuito.
- **Verificação de alcance.** Quem confere só disca para o IP que já vê do
  par, uma vez a cada 5 minutos por IP: não serve para mandar o nó atacar
  outro endereço.
- **Rede local.** Anúncio aceito só de endereço local; o IP usado é o de
  origem do datagrama, nunca um escrito dentro dele; um anúncio por segundo
  por IP; o pior que um anúncio falso faz é uma discagem que a cifra recusa.
- **Roteador.** O nó só fala HTTP com endereço da rede local (o roteador
  nunca está na internet); a porta aberta é a do próprio nó e é devolvida ao
  fechar (ou vence em 1 h).
- **Pacote do Éter.** Cada quadro é conferido como se viesse de um par
  (prova de trabalho, validação completa); quadro ruim é recusado sozinho,
  sem banir ninguém.
- **Risco residual:** a ponte vê quem fala com quem e quanto (não o quê);
  abrir porta no roteador expõe o nó à internet como qualquer servidor (as
  defesas de A4 valem); o comando de terminal parado à força não devolve a
  porta antes de ela vencer (1 h).

### A5 — workers maliciosos na computação entre nós (nível 3)

- Só especificação viaja, nunca código; faixas conferidas antes de executar;
  memória, prazo e fila com teto por par (`ciencia/rede.rs`, `hyurax-ultrax/src/rede.rs`).
- Compromisso antes da revelação; maioria; o resultado da maioria é refeito
  aqui antes de entrar no JOB, numa thread própria (a decisora), sem segurar
  a leitura do par. Teste: `decisora_decide_fora_da_thread_do_par`. Testes: `conluio_de_dois_e_pego_pela_conferencia_local`,
  `worker_que_adultera_perde_na_maioria_e_leva_divergencia`.
- Oferta só vale com horário próximo e uma chave por par; compromisso só com o
  worker da oferta daquele par; falha sem assinatura não pesa no worker
  alegado; operações declaradas acima do teto recusam a entrega; semente que
  não é a da unidade é recusada. Testes: `oferta_velha_ou_repetida_por_outro_par_nao_entra`,
  `pedido_com_semente_que_nao_e_da_unidade_e_recusado`.
- O compromisso é assinado pelo worker para a unidade do pedido (protocolo
  ULTRAX v2): quem revela outro resultado deixa duas assinaturas suas que não
  fecham, e a falha pesa nele. Testes: `revelacao_que_nao_bate_com_o_compromisso_falha`,
  `compromisso_assinado_igual_ao_gabarito`.
- A reputação é gravada em disco (`ciencia/reputacao.txt`), com Gold Score e
  nó verificado ([REPUTACAO.md](REPUTACAO.md)).
- **Risco residual:** nada disso foi ensaiado entre
  máquinas diferentes. **PENDENTE.**

### A6 — atualização falsa ou download adulterado

- O programa só oferece versão nova cujo manifesto foi assinado pela chave de
  lançamento (Ed25519, chave pública embutida), com endereço só das Releases
  do projeto, e confere tamanho e SHA-512 do instalador baixado antes de
  abrir (`atualizacao.rs`). Testes: `manifesto_assinado_confere_e_adulterado_nao`,
  `instalador_so_das_releases_do_projeto`, `arquivo_baixado_e_conferido_por_tamanho_e_sha512`.
- O instalador apaga só o que o próprio manifesto de instalação lista; os
  caminhos chegam ao PowerShell por variável de ambiente, nunca no texto do
  script. Teste: `scripts/testar-instalacao.ps1` (30 conferências).
- **Risco residual:** o próprio `.exe` não tem assinatura Authenticode (o
  Windows avisa ao abrir); quem baixa o primeiro instalador confia no GitHub
  e na soma publicada. Certificado: **PENDENTE** (do dono do projeto).

### A7 — quem leva o computador ou copia os arquivos

- A carteira é cifrada com senha de no mínimo 10 caracteres (Argon2id de
  64 MiB por tentativa).
- **Risco residual:** quem copia a pasta leva o segredo do segundo fator e a
  identidade do nó em texto (o fator vale contra quem senta no computador e
  sabe a senha, não contra quem copia arquivos — dito na tela e nos termos).

## Integridade dos dados

- Carteira, segundo fator, identidade, chave de sessão, checkpoint dos JOBs e
  cadeia são gravados com `sync_all` antes da troca de nome: uma queda de
  energia deixa o arquivo antigo ou o novo inteiro (`arquivos.rs`,
  `hyurax-store`).
- Checkpoint de JOB com SHA-512 e cópia anterior; o programa grava todos ao
  fechar.
- Registro do programa com teto de tamanho e rotação.

## O que fica de fora do programa de PC

Contas, backend, marketplace, pagamentos, aplicativo móvel e painel web
(Documento Mestre §14, §16, §17) ainda não existem; quando existirem, entram
neste documento com as ameaças próprias.
