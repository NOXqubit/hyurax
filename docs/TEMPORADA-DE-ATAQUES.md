# Temporada de ataques

As semanas 7 a 9 do [roteiro](ROTEIRO-LANCAMENTO.md) são para quebrar o
Hyurax de propósito, **antes** da mainnet, quando uma falha ainda custa
pouco. Este guia diz o que pode ser atacado, como montar o laboratório, o que
já está coberto e como relatar. A regra para a mainnet é a mesma do roteiro:
**toda falha grave relatada ganha um teste que a trava**.

Nada aqui vale dinheiro. O HYX da rede de teste não tem valor, e não há
recompensa em dinheiro nem em HYX por falha encontrada: quem acha uma falha
ganha crédito público no [placar](#placar), se quiser.

## Regras

**Pode:**

- atacar **nós seus**: no seu computador, numa rede de teste local
  (`--rede regtest`) ou na rede de teste pública (`--rede testnet`) com nós
  seus;
- mandar à semente pública (`wss://hyurax-semente.onrender.com/p2p`)
  **mensagens de protocolo** malformadas, fora de ordem, repetidas ou
  mentirosas: é exatamente o que ela precisa aguentar;
- minerar, reorganizar, gastar duas vezes e mentir na rede de teste pública.

**Não pode:**

- inundar a semente pública com volume (DDoS). Ela roda num plano grátis, a
  enxurrada derruba a rede para todo mundo e viola as regras do provedor. Para
  medir carga, suba a sua própria semente (`hyurax-no no --rede regtest
  --pasta minha-semente --ws-porta 8080`) e ataque essa;
- atacar computadores de outras pessoas, inclusive nós de quem também está
  testando, por fora do protocolo (varredura, invasão, engenharia social);
- publicar o passo a passo de uma falha grave antes da correção sair.

A plataforma HYURAX Compute ainda não está hospedada, então fica fora desta
temporada. Quem quiser atacá-la sobe a própria cópia (`hyurax-plataforma
servir --teste`) e relata do mesmo jeito.

## O que conta

| Gravidade | Exemplos |
|---|---|
| **Grave** | um nó aceita bloco ou transação inválida; emissão acima da regra; gasto duplo confirmado; reorganização que a regra não permite; derrubar ou travar um nó remotamente só com mensagens; ler ou alterar o tráfego cifrado (Noise XX); o gabarito Python e o nó Rust discordarem num caso; a conferência do ULTRAX aceitar um resultado falso |
| **Média** | gastar memória ou processador de um nó muito além do que o atacante gasta; atrasar a sincronização de propósito; fazer um nó honesto ser banido; vazar a identidade do nó ou a chave da carteira por um caminho inesperado |
| **Leve** | mensagem de erro que engana, limite mal documentado, comportamento estranho sem efeito na segurança |

## O laboratório

Com o programa da [Release](https://github.com/NOXqubit/hyurax/releases/latest)
ou compilado do código:

```bash
# dois nós numa rede de teste local, no mesmo computador
hyurax-no no --rede regtest --pasta lab-a --porta 18790
hyurax-no no --rede regtest --pasta lab-b --porta 18791 --semente 127.0.0.1:18790
```

O formato de cada mensagem está nos vetores (`vectors/wire.json`,
`vectors/malha.json`, `vectors/rede_ultrax.json`, `vectors/nuvem.json`) e no
gabarito Python (`reference/`). Os vetores de bytes são o jeito mais rápido de
montar uma mensagem forjada.

Os ataques que o projeto já roda, para começar de onde eles param:

```bash
# os cenários de rede (sobem nós de verdade em 127.0.0.1)
cargo test -p hyurax-net -- --ignored --test-threads=1
# fuzzing de todo decodificador que recebe bytes de fora, e dos 8 motores
HYX_FUZZ=200000 HYX_FUZZ_SEMENTE=7 cargo test -p hyurax-nucleo --test fuzz -- --nocapture
```

Troque a semente do fuzzing (`HYX_FUZZ_SEMENTE`) para explorar outras
mutações; a mesma semente reproduz exatamente a mesma rodada.

## O que já está coberto

Cada linha tem um teste que falha se a defesa sumir. Procure o que **não**
está aqui.

**Rede** (`crates/hyurax-net/tests/integracao.rs`)
- quadro gigante, lixo puro e aperto de mão gotejado: caem no prazo, sem
  estourar a memória;
- bloco forjado sem prova, órfão forjado e transação impagável: recusados, e
  quem mandou é derrubado;
- gasto duplo e nonce com buraco no mempool;
- cadeia com mais trabalho vence mesmo bifurcando fundo; ramo curto não desvia;
- intermediário só vê bytes cifrados; se alterar algo, a conexão cai;
- identidade provada na cifra; uma conexão por nó; identidade maliciosa
  banida; teto de entradas (por IP e por faixa de endereços), com a vaga
  liberada quando o par cai;
- ponte sem reserva recusa o circuito; pacote do Éter carrega blocos sem rede;
- enxurrada de conexões lentas na porta WebSocket (a da semente): acima do
  teto, a conexão fecha na hora, e a porta continua atendendo.

**Decodificadores** (`crates/hyurax-nucleo/tests/fuzz.rs`): o quadro do fio, o
bloco, o cabeçalho, a transação, a prova útil, as mensagens da malha (com
prefixo, anúncio de vizinho e pacote), a mensagem e o registro de prova do
ULTRAX, a mensagem da nuvem, o voucher de plano, a especificação de JOB e o
quadro do Éter. Nada cai nem demora, e o que é aceito tem uma codificação só.

**Conferência dos 8 motores** (o mesmo arquivo): resultado adulterado não
derruba a conferência nem demora. Nos motores de resultado único (matriz,
difusão, IA, genética, melhoramento, triagem), nenhum resultado diferente do
verdadeiro passa.

**Computação entre nós** (`crates/hyurax-nucleo/src/ciencia/rede.rs`):
maioria honesta vence o adulterador; o conluio de dois é pego pela conferência
local; quem troca o resultado entre o compromisso e a revelação é marcado.

## Como relatar

- **Falha grave:** pelo relato privado do GitHub, em
  [Security → Report a vulnerability](https://github.com/NOXqubit/hyurax/security/advisories/new).
  Se esse botão não aparecer, abra uma issue com o título "Pedido de canal
  privado" **sem nenhum detalhe da falha**, e a equipe responde por ali com um
  caminho privado.
- **Média e leve:** uma [issue](https://github.com/NOXqubit/hyurax/issues)
  normal.

Mande o passo a passo para reproduzir, a versão (`hyurax-no versao`) e, se
der, um teste que falha. Um teste que falha vale mais que qualquer descrição.

## Placar

| Quando | Quem | O que achou | Gravidade | Correção |
|---|---|---|---|---|
| 22/10/2026 | rodada interna | a porta WebSocket abria uma linha para cada conexão antes do upgrade, sem teto: uma enxurrada de conexões lentas esgotava as linhas do sistema, e a falha ao abrir uma linha nova derrubava o laço que aceita conexões (a semente ficava de pé, mas surda) | média | 1.5.3: teto de 128 atendimentos antes do upgrade, e criação de linha que não derruba o laço; teste `websocket_aguenta_enxurrada_de_conexoes_lentas` |
| 22/10/2026 | rodada interna | quadros de controle do WebSocket (ping) com até 4 MiB eram aceitos e ecoados; bits reservados e tipos desconhecidos eram ignorados | leve | 1.5.3: as regras da RFC 6455 (controle com até 125 bytes e inteiro; o resto derruba a conexão) |

## A rodada interna (22/10/2026)

Antes de chamar gente de fora, o projeto atacou a si mesmo:

- **Fuzzing:** 15 decodificadores × 200 mil mutações × 3 sementes, mais de
  9 milhões de entradas, e os 8 motores × 20 mil resultados adulterados × 2
  sementes. Nenhum pânico, nenhuma demora, nenhuma codificação dupla, nenhum
  resultado falso aceito.
- **Porta WebSocket da semente:** as duas falhas do placar, corrigidas com
  teste. O teste da enxurrada falha no código antigo (as conexões ficavam
  presas 10 s cada, sem teto) e passa no novo.
- **Dependências:** a conferência de falhas conhecidas (`cargo audit`) roda em
  todo envio e em todo lançamento; os avisos de outubro no TLS foram
  corrigidos nas versões 1.5.1 e 1.5.2.

O que a rodada interna **não** cobre, e por isso a temporada existe: ataques
que dependem de tempo e de várias máquinas (eclipse, sincronização com
atraso), economia de mineração (egoísta, retenção de bloco) e qualquer coisa
que o projeto não pensou em testar.
