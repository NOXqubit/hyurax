# AURON-SPEC-01

**Estado:** rascunho de trabalho. Congela quando o documento oficial do projeto
definir a tokenomics. Tudo marcado `PROVISÓRIO` depende desse documento.

**Escopo:** regras de consenso da rede Auron. O que está aqui, dois nós
precisam calcular igual, bit a bit, ou a rede racha.

**Implementação de referência:** `reference/auron/`, em Python, sem dependência
externa em nada que seja consenso. Ela é o oráculo. O nó de produção em Rust
precisa reproduzir cada valor definido aqui, byte a byte, contra os vetores em
`vectors/`.

---

## 1. Unidades

1 AUR = 100 000 000 unidades internas (`AUR_UNIT`, 8 casas).

Dinheiro é **sempre** inteiro sem sinal. Todo valor serializado cabe em `u64`.

`MAX_SUPPLY` = 21 000 000 AUR = 2 100 000 000 000 000 unidades.

### Regras do parser de valor

O parser é parte do consenso na prática: se Python e Rust discordarem sobre o
que é um valor válido, discordam sobre transações. As regras abaixo existem
para que os dois recusem exatamente as mesmas entradas.

1. **Ponto flutuante é proibido.** Não só desaconselhado: a função recusa o
   tipo. `to_units(0.1)` era aceito no protótipo, e era por ali que a poeira
   de arredondamento entrava no dinheiro.
2. **Negativo é recusado**, tanto em texto quanto em inteiro. Saldo, valor e
   taxa são `u64`; um valor negativo não tem representação na codificação.
   A checagem fica na saída comum das duas rotas, senão o caminho de inteiro
   passa por baixo do parser de texto.
3. **Só dígitos ASCII 0-9.** `str.isdigit()` do Python devolve verdadeiro para
   dígito de largura completa (`１`), algarismo indo-arábico oriental (`١`) e
   dezenas de outros, e `int()` converte todos. `str::parse::<u64>()` do Rust
   só aceita ASCII. Sem esta regra, `１` valeria 1 AUR num lado e erro no
   outro. Também fecha uma porta de falsificação visual: `１.5` e `1.5` são
   idênticos na tela.
4. **Mais de 8 casas decimais é recusado, não truncado.** Truncar é perder
   dinheiro em silêncio.
5. `+` inicial é aceito. `.5` e `7.` são aceitos. Espaço nas pontas é
   ignorado. Espaço no meio, vírgula, notação científica e prefixo hexadecimal
   são recusados.

A formatação (`to_aur_str`) aceita inteiro negativo de propósito, porque é
função de exibição e às vezes é preciso mostrar a diferença entre dois saldos.
Isso não cria valor monetário negativo no protocolo.

O arquivo `vectors/units.json` lista os casos aceitos **e** os recusados. O
Rust precisa reproduzir os dois lados.

## 2. AACL — Auron Adaptive Cryptographic Layer

Cada primitiva tem identificador e versão, e o protocolo carrega o
identificador junto com o dado. Trocar de algoritmo é registrar um novo
identificador, nunca reescrever o protocolo.

| Identificador | Uso | Código binário | Estado |
|---|---|---|---|
| `HASH-SHA512-V1` | hash geral, ids, Merkle | — | ativo |
| `SIG-ED25519-V1` | assinatura de transação | `1` | ativo |
| `SIG-BP512-V1` | Brainpool P-512 | — | legado, **não utilizável** |

O PoW usa Argon2id e é definido na seção 9. Ele não é intercambiável pelo
mesmo mecanismo: trocar o PoW é um hard fork, não uma troca de identificador.

**Por que Ed25519 no lugar de Brainpool P-512.** Verificação em microssegundos
contra dezenas de milissegundos. Determinístico por construção (RFC 8032), sem
nonce aleatório que possa vazar a chave. Suporte Rust de primeira linha. Como a
mainnet ainda não existe, a troca não custa migração.

Assinaturas com `S >= L` (ordem do grupo) são **rejeitadas**. Sem essa regra a
assinatura é maleável e o `txid` deixa de identificar a transação.

### Regra completa de verificação de `SIG-ED25519-V1`

Uma assinatura `R || S` de 64 bytes sobre a mensagem `M`, com a chave pública
`A` de 32 bytes, é válida se e só se:

1. `A` e `R` estão em encoding canônico: `y < p`, e bit de sinal zero quando
   `x = 0`. Qualquer outro encoding é recusado, mesmo que represente um ponto
   da curva.
2. `S < L`.
3. `[S]B = R + [k]A`, sem multiplicar pelo cofator, com
   `k = SHA-512(R || A || M) mod L` calculado sobre os bytes recebidos.

Chave ou assinatura de tamanho errado é recusada, e nunca vira erro: toda
entrada inválida dá o mesmo resultado, "assinatura inválida".

**Ponto de ordem pequena em `A` ou em `R` não é recusado.** Decisão consciente,
de 10/09/2026, que preserva o comportamento que a implementação de referência já
tinha. A consequência, dita na cara: uma chave de ordem pequena não tem segredo
correspondente, e qualquer um produz assinatura válida para ela. O que for
enviado ao endereço derivado de uma dessas chaves pode ser gasto por qualquer
um; o da identidade, por exemplo, é `892d1bf7e0f6107736c32cdc55930f95bf9611f3`.
Isso não afeta endereço de chave legítima, porque a geração de chave nunca
produz ponto de ordem pequena.

Esta seção explicita a regra que a referência já seguia; não muda consenso.
`vectors/crypto_ed25519_verify.json` trava a regra nos dois sentidos: um nó que
recuse ponto de ordem pequena, ou que aceite encoding não-canônico, falha nos
vetores.

## 3. Codificação canônica

Binária, posicional, de largura fixa. JSON foi descartado: escapamento de
Unicode, formatação de número e ordenação de chave variam entre linguagens, e
reproduzir o `json.dumps` do Python em Rust é uma fonte permanente de
divergência.

- Inteiros: big-endian, largura fixa e explícita (`u8`, `u16`, `u32`, `u64`).
- Bytes variáveis: prefixo `u32` big-endian de tamanho, depois o conteúdo.
- Bytes fixos (hash, endereço): sem prefixo.
- String: bytes variáveis contendo UTF-8.
- Lista: `u32` de contagem, depois os itens.
- Struct: concatenação dos campos na ordem declarada. Sem nomes, sem
  separadores, sem campo opcional implícito.

**Regra de unicidade.** Toda estrutura tem exatamente uma representação em
bytes. O decodificador rejeita sobra de bytes no fim. Um bloco cuja
recodificação não devolve os bytes originais é inválido.

## 4. Endereços

```
endereço = SHA-512(ascii(sig_algo_id) || pubkey)[0..20]
```

20 bytes. O identificador do algoritmo entra no hash de propósito: a mesma
chave sob algoritmos diferentes precisa dar endereços diferentes, senão trocar
de algoritmo cria colisão de identidade.

## 5. Transações

Dois tipos, distinguidos pelo primeiro byte (`kind`).

### 5.1 Coinbase (`kind = 0`)

```
u8   kind = 0
u16  version = 1
u64  height
[20] recipient
u64  amount
var  extra_nonce      (máx. 64 bytes)
```

Criada pela cadeia. **Não tem campo de remetente nem de assinatura.** No
protótipo, coinbase era uma transferência com `sender = "COINBASE"`, protegida
por checagens espalhadas; esquecer uma checagem em qualquer caminho virava
impressão de dinheiro. Aqui não existe como forjar: a estrutura não tem os
campos.

`height` precisa bater com a altura do bloco. Isso também torna cada coinbase
única, mesmo com mesmo destinatário e mesmo valor.

### 5.2 Transferência (`kind = 1`, versão 2)

```
u8   kind = 1
u16  version = 2
u8   sig_code                 (1 = SIG-ED25519-V1)
[20] sender
u64  fee                      (sempre em AUR)
u64  nonce                    (nonce de conta, sequencial)
u32  quantidade de saídas     (de 1 a 16)
     cada saída:
       [20] recipient
       [32] asset_id
       u64  amount
var  public_key
var  signature                (não entra no payload assinado)
```

Mensagem assinada:

```
"AURON-TX-v2" || network_magic || <tudo acima menos signature>
```

A versão 1 tinha uma saída só, com o ativo implícito. Ela não existe mais: a
arquitetura do projeto exige que a transação nasça preparada para mais de um
ativo (`docs/AURON-DIRECT-RESONANCE.md`, §19; decisão de 11/09/2026). O
domínio da assinatura mudou junto com o formato, de modo que uma assinatura
feita para a versão 1 nunca vale como assinatura de uma transferência
versão 2. A coinbase não mudou e continua na versão 1.

**Ativos.** O `asset_id` tem 32 bytes. O AUR, ativo nativo, é o identificador
todo zero; ativos futuros vão ter um identificador derivado do hash da própria
emissão. Até existir uma regra de emissão, o consenso aceita só o AUR.

O `network_magic` entra de propósito: sem ele, transação assinada na testnet
vale na mainnet.

`txid` = SHA-512(codificação completa, assinatura inclusive).

Regras estruturais e criptográficas, conferidas antes de qualquer regra de
saldo, nesta ordem:

1. De 1 a 16 saídas. O decodificador recusa a contagem antes de ler qualquer
   saída, então uma contagem absurda não custa leitura nenhuma.
2. `fee` e cada `amount` dentro de `u64`, e cada `amount > 0`.
3. Todo `asset_id` é de um ativo conhecido. Hoje, só o AUR.
4. Nenhuma saída para o próprio `sender`.
5. Saídas em ordem estritamente crescente de `(recipient, asset_id)`. Isso
   proíbe saída repetida e deixa um único jeito de escrever o mesmo
   pagamento, como pede a regra de unicidade da seção 3.
6. Para cada ativo, a soma das saídas (mais a taxa, no AUR) não estoura
   `u64`.
7. `public_key` e `signature` com o tamanho do algoritmo declarado.
8. `endereço(public_key) == sender`.
9. Assinatura válida sobre o payload acima.

A regra de saldo (seção 13) é conferida por ativo: o saldo do `sender` em cada
ativo precisa cobrir a soma das saídas naquele ativo, mais a taxa no AUR.
Todos os ativos são conferidos antes de qualquer débito.

## 6. Árvore de Merkle

RFC 6962 (Certificate Transparency), com SHA-512.

```
MTH({})     = H("")
MTH({d})    = H(0x00 || d)
MTH(D[n])   = H(0x01 || MTH(D[0:k]) || MTH(D[k:n]))     k = maior potência de 2 < n
```

Duas propriedades que o protótipo não tinha:

- Prefixo `0x00` em folha e `0x01` em nó interno impede confundir o hash de uma
  folha com o de uma subárvore.
- Em contagem ímpar o último elemento é **promovido**, nunca duplicado.
  Duplicar é o bug do Bitcoin CVE-2012-2459, em que duas listas diferentes
  produzem a mesma raiz.

**As folhas são a codificação completa da transação, assinatura inclusive.**
No protótipo a impressão digital deixava chave e assinatura de fora, então
trocar a assinatura não mudava o cabeçalho.

**A prova de inclusão não amarra o total de folhas.** A verificação recebe o
total como parâmetro, e árvores de totais diferentes podem ter a mesma forma
no caminho de uma folha: a prova de uma árvore de 3 folhas passa quando
conferida com total 4. Quem confere uma prova precisa obter o total de uma
fonte autenticada. `vectors/codec_edge.json` trava esse comportamento. Nota
de 11/09/2026; não muda consenso.

## 7. Cabeçalho de bloco

```
u16  version = 1
u64  height
[64] prev_hash        (SHA-512 do cabeçalho anterior)
[64] merkle_root
u64  timestamp        (segundos Unix)
u32  bits             (alvo em forma compacta)
u64  nonce
```

Tamanho fixo: 158 bytes.

Dois hashes diferentes sobre o mesmo cabeçalho, de propósito:

```
block_hash = SHA-512(cabeçalho)      identidade, ligação entre blocos, barato
pow_hash   = Argon2id(cabeçalho)     prova de trabalho, caro
```

Separar importa: a identidade é calculada milhões de vezes ao sincronizar e
precisa ser barata; a prova precisa ser cara. Uma função só obrigaria a
escolher entre índice lento e prova barata.

Bloco = cabeçalho seguido da lista de transações.

## 8. Alvo em forma compacta

4 bytes. Byte alto é o expoente, três bytes baixos são a mantissa. O bit
`0x00800000` (sinal) é sempre zero.

```
size = bits >> 24
mantissa = bits & 0x007FFFFF
target = mantissa >> (8 * (3 - size))     se size <= 3
target = mantissa << (8 * (size - 3))     se size > 3
```

**Forma canônica obrigatória.** Reempacotar o alvo decodificado tem que
devolver exatamente os mesmos `bits`. Sem isso, dois `bits` diferentes
representam o mesmo alvo e o cabeçalho perde representação única.

Todo alvo que vai para um cabeçalho passa por `normalize_target`, que arredonda
para baixo, isto é, nunca deixa o alvo mais fácil que o pretendido.

## 9. Prova de trabalho

```
pow_hash = Argon2id(
    password = cabeçalho serializado (158 bytes),
    salt     = "AURON-POW-v1\0\0\0\0"   (16 bytes, fixo e público),
    m        = pow_memory_kib,
    t        = pow_time_cost,
    p        = pow_lanes,
    tag      = 32 bytes,
    version  = 0x13
)
```

Válido quando `int_be(pow_hash) <= target`.

Argon2id conforme RFC 9106, validado contra os vetores oficiais da seção 5.

**Os três custos são grandezas separadas.** Era o bug 3 do protótipo, onde um
único `difficulty` alimentava ao mesmo tempo a probabilidade e o tamanho do
problema, tornando o custo super-exponencial e o retarget impossível.

| Grandeza | O que é | Onde vive | Muda a cada bloco? |
|---|---|---|---|
| `CONSENSUS_DIFFICULTY` | `target` | campo `bits` do cabeçalho | sim, pelo retarget |
| `WORK_SIZE` | parâmetros do Argon2id | `ChainParams`, por rede | não |
| `VERIFICATION_COST` | uma avaliação Argon2id | constante | não |

`pow_hash` não conhece o alvo. Verificar custa o mesmo com alvo fácil ou
difícil, e é isso que torna o custo de validação previsível.

Comparação é numérica contra o alvo, não contagem de bits zero. Contar zeros só
permitiria dificuldade em potências de 2.

## 10. Retarget — LWMA-1

A cada bloco, sobre uma janela de `lwma_window` blocos.

```
T = target_spacing
k = window * (window + 1) / 2 * T

para i em 1..window:
    st = timestamp[i] - timestamp[i-1]
    st = clamp(st, -6T, +6T)
    weighted += st * i

weighted = max(weighted, k / 3)
candidate = (média dos alvos da janela) * weighted / k
candidate = clamp(candidate, alvo_anterior / 2, alvo_anterior * 2)
next_target = normalize_target(min(candidate, max_target))
```

**Por que não o algoritmo do Bitcoin.** Janela de 2016 blocos é perigosa numa
rede pequena: um minerador grande chega, minera rápido, vai embora, e a cadeia
trava por semanas até o próximo ajuste. LWMA reage em blocos.

Três defesas contra timestamp mentiroso: o limite de `±6T` por intervalo, o
piso `k/3` no denominador (sem ele, timestamps colados fazem o alvo explodir), e
a trava de fator 2 por bloco nos dois sentidos.

Tempo de solução negativo é tolerado e limitado, não rejeitado: timestamps não
são monotônicos por regra de rede, só o median-time-past é.

## 11. Timestamps

- `median_time_past` = mediana dos últimos `median_time_span` (11) timestamps.
- `timestamp > median_time_past` — obrigatório.
- `timestamp <= agora + max_future_drift` (120 s) — obrigatório.

A mediana é o que impede manipular o retarget mentindo no relógio: seria
preciso controlar a maioria da janela, não só o próprio bloco.

## 12. Emissão — `PROVISÓRIO`

```
block_reward(h) = initial_reward >> (h / halving_interval)      0 após 64 halvings
```

Valores atuais, sujeitos ao documento oficial:

- `initial_reward` = 50 AUR
- `halving_interval` = 210 000 blocos
- Emissão total: exatamente 21 000 000 AUR, dentro de `MAX_SUPPLY`.

O protótipo pagava 50 AUR para sempre, sem halving e sem teto.

A coinbase pode pagar no máximo `block_reward(altura) + soma das taxas do
bloco`. Só a parte de subsídio conta como emissão nova; a parte de taxa é
dinheiro que já existia. O estado recusa qualquer bloco que faria a emissão
passar de `MAX_SUPPLY`.

## 13. Estado

Modelo de contas: saldo por endereço e por ativo; nonce por endereço. Nenhum saldo pode existir em ativo sem regra de emissão.

- Nonce começa em 0 e é estritamente sequencial. Nonce fora de ordem é
  rejeitado, o que fecha replay.
- Transferência debita `amount + fee` do remetente e credita `amount` ao
  destinatário. A taxa vai para a coinbase.
- **Maturação de coinbase.** A recompensa fica retida por `coinbase_maturity`
  blocos antes de virar saldo gastável. Sem isso, um reorg deixa sem lastro
  quem aceitou pagamento financiado por aquela recompensa.
- Transação duplicada dentro do mesmo bloco é rejeitada.
- Aplicação é atômica: ou o bloco inteiro entra, ou nada entra.

`apply_block` devolve um registro de desfazer. Sem ele não existe reorg
correto, e sem reorg correto não existe rede P2P.

Invariantes conferidas depois de cada bloco: nenhum saldo negativo, emissão
total dentro do teto, moeda em circulação (saldos livres mais coinbase imatura)
dentro do teto.

## 14. Validação de bloco, na ordem

Do mais barato para o mais caro. Um bloco malformado é recusado sem gastar um
Argon2id — o contrário é vetor de negação de serviço.

1. `version == 1`
2. `height == ponta.height + 1`
3. `prev_hash == hash da ponta`
4. `bits` igual ao esperado pelo retarget
5. `bits` decodifica para alvo canônico
6. `timestamp > median_time_past`
7. `timestamp <= agora + max_future_drift`
8. Tamanho do bloco dentro de `max_block_bytes`
9. Primeira transação é coinbase; nenhuma outra é
10. `merkle_root` bate com as transações
11. Codificação é canônica
12. **Prova de trabalho bate o alvo**
13. Estado aceita o bloco inteiro
14. Invariantes continuam valendo

Existe **um único** caminho de entrada, `accept_block`. Não há função que
aplique um bloco sem validar. No protótipo, `broadcast_block` conferia altura e
hash anterior e chamava `_commit` direto, sem verificar prova de trabalho
nenhuma: qualquer bloco forjado entrava na cadeia dos vizinhos.

## 15. Escolha de ponta

Vence a cadeia com maior **trabalho acumulado**, onde o trabalho de um bloco é
`2^256 / (target + 1)`.

Não é altura. Comparar altura deixa uma cadeia longa de blocos fáceis ganhar de
uma curta de blocos difíceis, que é exatamente o que um atacante quer.

Empate em trabalho desempata pelo `block_hash` menor. É arbitrário, mas
determinístico: dois nós honestos precisam escolher a mesma ponta sem
conversar.

## 16. Gênese

Uma só por rede, derivada dos parâmetros, nunca lida de arquivo.

- `height = 0`, `prev_hash = 64 bytes zero`
- `timestamp = 1788912000` (2026-09-09T00:00:00Z), congelado
- `bits = target_to_compact(max_target)`, `nonce = 0`
- Uma coinbase de 1 unidade para o endereço nulo, com `extra_nonce`
  identificando a rede. Ninguém tem a chave do endereço nulo: nenhuma conta
  começa com dinheiro.

O protótipo tinha duas gêneses incompatíveis. `auron_core` começava com a lista
vazia e o primeiro bloco minerado ficava na altura 0; `auron_reference` criava a
gênese na altura 0 e o primeiro minerado ia para a altura 1.

## 17. Parâmetros por rede

| Parâmetro | mainnet | testnet | regtest |
|---|---|---|---|
| magic | `AURM` | `AURT` | `AURR` |
| Argon2id memória | 32 MiB | 32 MiB | 32 KiB |
| Argon2id t / p | 1 / 1 | 1 / 1 | 1 / 1 |
| `target_spacing` | 120 s | 120 s | 120 s |
| `lwma_window` | 60 | 60 | 30 |
| `coinbase_maturity` | 100 | 20 | 2 |
| `max_block_bytes` | 1 000 000 | 1 000 000 | 1 000 000 |
| `max_future_drift` | 120 s | 120 s | 120 s |
| `median_time_span` | 11 | 11 | 11 |

A memória menor no regtest existe porque a implementação Python leva dezenas de
segundos em 32 MiB. O algoritmo é o mesmo; só o `WORK_SIZE` muda. É exatamente
por isso que `WORK_SIZE` precisava estar separado da dificuldade.

## 18. Utrax — trabalho útil, **fora do consenso**

A segurança da cadeia vem do PoW da seção 9. O Utrax é camada econômica de
tarefas verificáveis. **A cadeia não precisa do Utrax para sobreviver.**

Consequência de projeto: quem paga a verificação é quem publicou a tarefa.
Verificação cara vira decisão de negócio do publicador, não vetor de negação de
serviço contra a rede.

Cada tarefa declara `WorkSpec` com `WORK_SIZE` e `VERIFICATION_COST`.

**Matriz.** Executa `C = A·B`. Verificação por Freivalds em O(n²), com desafio
derivado de `H(semente || tamanho || resultado)`. Vetores em `[0, 2^20)`,
4 rodadas, erro `<= 2^-80`. O desafio depender do próprio resultado alegado é o
que impede grinding contra vetores fixos. O protótipo sorteava sem semente, e
dois nós honestos podiam discordar.

**Mochila.** A especificação exige o **ótimo**, e a verificação recomputa a
programação dinâmica. Custo O(n·C), igual à execução. O protótipo conferia só
viabilidade e valor declarado, então 40 bytes de zeros passavam como trabalho
válido.

**Difusão.** Aritmética inteira de ponto fixo (escala 2^20), divisão truncando
na direção do zero. Sem ponto flutuante em lugar nenhum. Sem prova curta:
verificação reexecuta.

Geração de instância por SHA-512 em modo contador. **Não usa o gerador do
numpy**: o fluxo de bits do numpy é dependência de versão, e nada que dois nós
precisem calcular igual pode depender de versão de biblioteca.

**Marketplace.** A semente da instância inclui o endereço do executor, senão o
segundo copia a resposta do primeiro. O executor entrega o **resultado**; no
protótipo entregava só um nonce e o verificador executava o trabalho. Submissão
inválida não trava a tarefa. O escrow é explícito e a contabilidade fecha:
saldos livres mais retido é sempre igual ao total.

## 19. Ausente de propósito

Não está nesta especificação, e não é esquecimento:

- **P2P.** Fica na Fase 2, no Rust. A referência é de nó único.
- **Mempool e política de taxa.** Fase 2.
- **Mineração mobile como mecanismo separado.** Removida. O protótipo tinha
  registro de dispositivo e reivindicação de recompensa por época, e o desenho
  era inviável: a prova era amarrada ao hash exato da ponta, então cada bloco
  novo invalidava toda prova em andamento e o celular sempre perdia a corrida.
  Com PoW convencional, celular e desktop rodam o **mesmo** minerador, e o
  Argon2id com pressão de memória já nivela bastante a disputa. Menos código,
  menos superfície, mesmo resultado.
- **Fusion.** Congelado no protótipo, sem manutenção nesta janela.
- **Air Storage, Recovery, Space Relay, ponte Bitcoin.** Roadmap.

## 20. Controle de mudança

Esta especificação é congelada por versão. Mudar regra de consenso é publicar
`AURON-SPEC-02`, não editar esta.

Todo módulo migrado para Rust segue:

```
Referência Python -> vetor de teste -> implementação Rust -> comparação byte a byte
```

Um módulo só é considerado migrado quando os resultados forem comprovadamente
iguais onde a especificação exigir igualdade.
