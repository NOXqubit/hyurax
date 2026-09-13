# AURON-SPEC-01

**Estado:** rascunho de trabalho. Congela quando o documento oficial do projeto
definir a tokenomics. Tudo marcado `PROVISÓRIO` depende desse documento.

**Revisão de 12/09/2026 — `consensus_version = 2`.** Consenso híbrido (seção
9A): todo bloco exige prova de trabalho útil; cabeçalho passa a 222 bytes;
política criptográfica com nível real por primitiva (seção 2). Como a rede
ainda não existe e o documento não está congelado, a mudança entra aqui em vez
de abrir `AURON-SPEC-02`.

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
6. **No máximo 64 caracteres.** O maior valor representável tem 20 dígitos
   inteiros e 8 decimais, então 64 dá folga de sobra. O limite é conferido
   antes de qualquer conversão. Sem ele, uma entrada com milhões de dígitos
   fazia o `int()` do Python levantar `ValueError` (acima de 4300 dígitos ele
   recusa converter), que não é o erro de valor que quem chama trata, e ainda
   gastava CPU à toa. Corrigido em 12/09/2026, na revisão de ataque.

A formatação (`to_aur_str`) aceita inteiro negativo de propósito, porque é
função de exibição e às vezes é preciso mostrar a diferença entre dois saldos.
Isso não cria valor monetário negativo no protocolo.

O arquivo `vectors/units.json` lista os casos aceitos **e** os recusados. O
Rust precisa reproduzir os dois lados.

## 2. AACL — Auron Adaptive Cryptographic Layer

Cada primitiva tem identificador e versão, e o protocolo carrega o
identificador junto com o dado. Trocar de algoritmo é registrar um novo
identificador, nunca reescrever o protocolo.

| Identificador | Uso | Código binário | Clássico | Quântico | Estado |
|---|---|---|---|---|---|
| `HASH-SHA512-V1` | hash geral, ids, Merkle | — | 256 bits (colisão) | ~256 bits (pré-imagem)¹ | ativo |
| `SIG-ED25519-V1` | assinatura de transação | `1` | ~128 bits | nenhum | ativo |
| `SIG-BP512-V1` | Brainpool P-512 | — | ~256 bits | nenhum | legado, **não utilizável** |
| `SIG-MLDSA65-V1` | ML-DSA-65 (FIPS 204), par híbrido | — | ~192 bits | ~192 bits | **planejado, sem código** |
| `AEAD-XCHACHA20POLY1305-V1` | cifragem de dados fora da cadeia | — | 256 bits | ~128 bits | **planejado, sem código** |

¹ Como está em `crypto.PRIMITIVAS`: colisão sob Grover/BHT fica abaixo disso;
o número é a referência de pré-imagem.

A tabela é a mesma de `reference/auron/crypto.py` (`PRIMITIVAS`, política
versão 1), e `politica_valida()` recusa: primitiva ativa abaixo de
128 bits clássicos, campo faltando, e **qualquer nível declarado igual ou
acima de 1024 bits**.

### Meta de arquitetura "classe 1024 bits"

É **meta interna de projeto**, experimental, e não um nível atingido. Nenhuma
primitiva real entrega 1024 bits de segurança, e o projeto não inventa uma
("SHA-1024" não existe). Em comunicação pública valem os números da tabela.
Segurança criptográfica nunca é um campo genérico de tamanho fixo (o protótipo
tinha "8 bits"): é declarada por primitiva, com o algoritmo de verdade.

### Modo híbrido pós-quântico (planejado)

Assinatura híbrida = Ed25519 **e** ML-DSA-65 sobre a mesma mensagem, ambas
obrigatórias; quebrar uma não basta. Entra por novo código de algoritmo na
transação e nova `crypto_policy_version`. Tamanhos: chave pública 1952 bytes,
assinatura 3309 bytes. Nada disso está implementado, e a biblioteca precisa ser
auditada e em Rust puro antes de entrar.

### Regras ainda sem código (valem para quando existir)

- **CryptoProvider:** todo uso de primitiva passa por uma interface que recebe
  o identificador; nenhum módulo chama algoritmo pelo nome.
- **Separação de domínio:** toda mensagem assinada ou hash de compromisso leva
  um prefixo próprio (`AURON-TX-v2`, `AURON-UPOW-TASK-v1`, ...). Hoje já vale e
  é testado em `test_11`.
- **Hierarquia de chaves:** chave de identidade separada da chave de carteira;
  chave de dispositivo derivada, revogável.
- **Data Chain:** dado pessoal bruto nunca vai para a cadeia. Na cadeia, só
  compromisso (hash com sal) e referência; o dado cifrado fica fora.

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

O limite de 64 bytes do `extra_nonce` vale **na leitura e na escrita**. Uma
coinbase com `extra_nonce` maior é recusada ao ser decodificada. Antes da
revisão de 12/09/2026, a leitura aceitava e o erro só aparecia depois, ao
recodificar para calcular o `txid` — fora de qualquer caminho preparado para
tratá-lo. Todo erro de transação dentro de um bloco é motivo de rejeição do
bloco, e precisa chegar a quem valida como erro de cadeia.

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
u16  version = 2
u64  height
[64] prev_hash        (SHA-512 do cabeçalho anterior)
[64] merkle_root
[64] useful_root      (compromisso da prova de trabalho útil, seção 9A)
u64  timestamp        (segundos Unix)
u32  bits             (alvo em forma compacta)
u64  nonce
```

Tamanho fixo: 222 bytes. A versão 1 (158 bytes, sem `useful_root`) é recusada.

Dois hashes diferentes sobre o mesmo cabeçalho, de propósito:

```
block_hash = SHA-512(cabeçalho)      identidade, ligação entre blocos, barato
pow_hash   = Argon2id(cabeçalho)     prova de trabalho, caro
```

Separar importa: a identidade é calculada milhões de vezes ao sincronizar e
precisa ser barata; a prova precisa ser cara. Uma função só obrigaria a
escolher entre índice lento e prova barata.

```
bloco = cabeçalho || var_bytes(prova de trabalho útil) || lista de transações
```

A prova vai com prefixo de tamanho. Bloco sem prova ainda decodifica (campo
vazio) para a validação recusar com o motivo certo.

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
    password = cabeçalho serializado (222 bytes, useful_root incluído),
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
| `TARGET_BLOCK_INTERVAL` | tempo **desejado** entre blocos | `target_spacing` | não; é só o alvo do retarget |
| `CONSENSUS_DIFFICULTY` | `target` | campo `bits` do cabeçalho | sim, pelo retarget |
| `WORK_SIZE` | parâmetros do Argon2id | `ChainParams`, por rede | não |
| `VERIFICATION_COST` | uma avaliação Argon2id | constante | não |
| `USEFUL_WORK_SIZE` | lado `n` das matrizes do trabalho útil | regra da seção 9A | sim, segue o trabalho validado |
| `USEFUL_VERIFICATION_COST` | Freivalds, O(n² · rodadas) | `useful_rounds` | acompanha `n` |
| `CRYPTO_SECURITY_LEVEL` | nível real de cada primitiva | tabela da seção 2 | não; muda por versão de política |

O intervalo de bloco não define dificuldade, nem tamanho de trabalho, nem custo
de verificação. Nenhuma dessas grandezas é derivada de outra por acidente.

`pow_hash` não conhece o alvo. Verificar custa o mesmo com alvo fácil ou
difícil, e é isso que torna o custo de validação previsível.

Comparação é numérica contra o alvo, não contagem de bits zero. Contar zeros só
permitiria dificuldade em potências de 2.

## 9A. Trabalho útil no consenso — UsefulPoW híbrido

Desde `consensus_version = 2`, **todo bloco** precisa trazer uma prova
verificável de trabalho útil. O Argon2id da seção 9 continua, como camada
complementar de segurança; um não substitui o outro. Implementação:
`reference/auron/usefulpow.py`. Vetores: `vectors/usefulpow.json`.

### Família 1 — `MATRIX-FREIVALDS-V1`

**Tarefa.** `C = A · B`, com `A` e `B` matrizes `n×n` de inteiros em
`[0, 999]`, geradas por SHA-512 em modo contador (mesmo gerador da seção 18).

**Instância, derivada do estado anterior do consenso:**

```
semente = SHA-512("AURON-UPOW-TASK-v1" || magic || u64 altura
                  || prev_hash || endereço do minerador)
```

- `prev_hash` só existe depois do bloco anterior: ninguém resolve antes.
- O endereço é o `recipient` da coinbase: copiar a prova de outro minerador
  não serve.
- `n` sai da regra abaixo, nunca de escolha do minerador: não há tarefa fácil
  para escolher.

**Tamanho exigido (`USEFUL_WORK_SIZE`).** Multiplicar matrizes custa `n³`. Para
o trabalho útil crescer na mesma razão que o trabalho de consenso, `n` cresce
com a raiz cúbica, em inteiro:

```
delta = max(0, bits_de_trabalho(alvo) - bits_de_trabalho(max_target))
        onde bits_de_trabalho(t) = 256 - bit_length(t)
n = useful_size_base << (delta / 3)
n = n * (1000, 1260, 1587)[delta % 3] / 1000
n = clamp(n, useful_size_min, useful_size_max)
```

O alvo reflete o trabalho que a rede de fato validou, porque o retarget o
ajusta pelo ritmo observado. Rede mais forte, mais trabalho útil por bloco.
Mudar `target_spacing` não muda `n` para o mesmo alvo.

| Rede | mín | base | máx |
|---|---|---|---|
| mainnet / testnet | 32 | 48 | 256 |
| regtest | 4 | 6 | 16 |

Na mainnet: `delta` 0 → n 48; 3 → 96; 6 → 192; 9 ou mais → 256.

**Prova.**

```
u8   family  = 1
u16  version = 1
u32  n
var  result  = C, n·n entradas u32 big-endian, linha a linha
useful_root  = SHA-512("AURON-UPOW-PROOF-v1" || prova codificada)
```

Como o Argon2id é calculado sobre o cabeçalho que contém `useful_root`, trocar
a prova depois de achar o nonce invalida o Argon2id.

**Conferência, nesta ordem** (a primeira falha recusa o bloco):

1. prova presente;
2. `useful_root` do cabeçalho igual ao compromisso da prova;
3. `family == 1` e `version == 1`;
4. `n` **igual** ao exigido. Menor é trabalho insuficiente; maior também é
   recusado, para a regra ser uma só e o tamanho do bloco previsível;
5. `result` com exatamente `n·n·4` bytes;
6. cada entrada em `[0, n · 998²]` (faixa de um produto honesto; fecha o
   estouro de inteiro, como na seção 18);
7. Freivalds, `useful_rounds = 4` rodadas, vetores de 20 bits tirados de
   `XOF("AURON-UPOW-CHALLENGE-v1", SHA-512(semente || result))`. Erro de aceitar
   resultado falso `<= 2^-80`.

Custo: fazer é `n³`; conferir é `4 · 3 · n²`. Para n = 256, cerca de
16,7 milhões de operações contra 786 mil.

**O que esta prova demonstra.** Que o minerador calculou um produto de matrizes
do tamanho exigido para aquele bloco, o mesmo tipo de conta que sustenta
treino e inferência de IA.

**O que ela NÃO demonstra.** Utilidade externa. A instância é gerada pela rede,
não trazida por um cliente, porque nenhum bloco pode depender de alguém de fora
aparecer com uma tarefa na hora certa. Problemas reais de terceiros continuam no
mercado UTRAX da seção 18.

**Limite conhecido, dito com todas as letras.** A prova viaja inteira no bloco:
n = 48 ocupa 9,2 KB; n = 256 ocupa 262 KB, um quarto de `max_block_bytes`. Na
mainnet o teto de `n` chega com apenas 9 bits de trabalho acima do mínimo; a
partir daí o trabalho útil para de acompanhar a rede. A família 2 (várias
instâncias por bloco, ou compromisso de Merkle sobre `C` com abertura por
amostragem) é **planejada**, e entra por nova `task_rules_version`.

### Versões de regra

`ChainParams` declara, em todas as redes: `consensus_version = 2`,
`task_rules_version = 1`, `verification_rules_version = 1`,
`reward_schedule_version = 1`, `crypto_policy_version = 1`. Toda mudança de
regra muda um desses números.

### Fraude

Prova inválida faz o bloco ser recusado; o minerador perde o custo do trabalho
e a recompensa daquele bloco. **Confisco automático de fundos não é regra
padrão.** Não existe, nesta versão, stake ou saldo que o consenso tome de volta.

## 10. Retarget — LWMA-1

A cada bloco, sobre uma janela de `lwma_window` blocos.

```
T = target_spacing
k = window * (window + 1) / 2 * T

# a janela vira uma sequência não decrescente antes da conta
mono[0] = timestamp[0]
para i em 1..window:
    mono[i] = max(timestamp[i], mono[i-1])

para i em 1..window:
    st = mono[i] - mono[i-1]          # nunca negativo
    st = min(st, 6T)
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

**Timestamps não decrescentes no cálculo.** Timestamps não são monotônicos por
regra de rede, só o median-time-past é. Mas aceitar tempo de solução negativo
na conta abre um ataque de parada: um minerador sem maioria publica blocos com
horário para trás, cada um válido sozinho, e derruba a soma ponderada. Soma
menor significa alvo menor, ou seja **dificuldade maior para a rede inteira**.
Medido na regra anterior, intercalar horários antigos batia no limite de
metade do alvo por bloco; repetido, trava a cadeia.

Por isso a janela é tornada não decrescente antes da conta: horário para trás
vira tempo zero. O pior que o atacante consegue é não contribuir com tempo, e
a manipulação no outro sentido fica em poucos por cento. Corrigido em
12/09/2026, na revisão de ataque; os casos `horario_alternado`,
`horario_para_tras` e `horarios_iguais` em `vectors/targets.json` travam a
regra.

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

A coinbase paga `amount`, com `0 <= amount <= block_reward(altura) + soma das
taxas do bloco`. **Zero é válido:** o minerador pode abrir mão da recompensa, e
isso só reduz a emissão. A regra está escrita nos dois limites de propósito: o
gabarito recusava o zero enquanto esta seção falava apenas em máximo, e um nó
em Rust seguindo o texto aceitaria um bloco que o gabarito recusa. Dois
programas discordando sobre o mesmo bloco racha a rede. Alinhado em
12/09/2026, na revisão de ataque.

Só a parte de subsídio conta como emissão nova; a parte de taxa é dinheiro que
já existia. O estado recusa qualquer bloco que faria a emissão passar de
`MAX_SUPPLY`.

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

1. `version == 2`
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
12. **Prova de trabalho útil confere** (seção 9A): O(n²), mais cara que o que
    vem antes e bem mais barata que o Argon2id
13. **Prova de trabalho Argon2id bate o alvo**
14. Estado aceita o bloco inteiro
15. Invariantes continuam valendo

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
- Prova de trabalho útil como qualquer bloco (altura 0, `prev_hash` zero,
  minerador = endereço nulo, alvo `max_target`), e `useful_root` dela. O formato
  é um só, sem exceção para o primeiro bloco.
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

## 18. Utrax — mercado de trabalho útil

Duas coisas diferentes, que não se confundem:

- **Dentro do consenso:** a família `MATRIX-FREIVALDS-V1` da seção 9A. Instância
  gerada pela rede, exigida em todo bloco.
- **Fora do consenso (esta seção):** o mercado UTRAX, onde alguém publica uma
  tarefa de verdade e paga por ela. Nenhum bloco depende de haver tarefa
  publicada.

Consequência de projeto: quem paga a verificação é quem publicou a tarefa.
Verificação cara vira decisão de negócio do publicador, não vetor de negação de
serviço contra a rede.

Cada tarefa declara `WorkSpec` com `WORK_SIZE` e `VERIFICATION_COST`.

**Matriz.** Executa `C = A·B`. Verificação por Freivalds em O(n²), com desafio
derivado de `H(semente || tamanho || resultado)`. Vetores em `[0, 2^20)`,
4 rodadas, erro `<= 2^-80`. O desafio depender do próprio resultado alegado é o
que impede grinding contra vetores fixos. O protótipo sorteava sem semente, e
dois nós honestos podiam discordar.

**Faixa obrigatória do resultado.** Antes de qualquer conta, cada entrada de
`C` precisa estar em `[0, n * (MATRIX_ENTRY_MAX - 1)^2]`, que é a faixa de um
produto honesto. Sem essa checagem, um executor entrega entradas perto de
`2^63`: a multiplicação `C·r` estoura o inteiro de 64 bits, dá a volta em
silêncio, e um resultado errado pode coincidir com o certo no que sobra. A
checagem custa O(n²), a mesma ordem da verificação. Corrigido em 12/09/2026,
na revisão de ataque.

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

- **P2P.** Começou: a rede entre nós está na seção 21, e a auto-reanimação na
  seção 22. A referência em Python continua de nó único; a rede é só no Rust.
- **Mempool e política de taxa.** Junto com a rede (seção 21).
- **Mineração mobile como mecanismo separado.** Removida. O protótipo tinha
  registro de dispositivo e reivindicação de recompensa por época, e o desenho
  era inviável: a prova era amarrada ao hash exato da ponta, então cada bloco
  novo invalidava toda prova em andamento e o celular sempre perdia a corrida.
  Com um minerador só (trabalho útil da seção 9A mais Argon2id), celular e
  desktop rodam o **mesmo** programa, e o Argon2id com pressão de memória já nivela bastante
  a disputa. Menos código,
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

## 21. Rede entre nós — `AURON-WIRE-v1`

A referência em Python é de nó único. A rede é código só de Rust, começando em
12/09/2026. Só o **formato das mensagens** e as **regras de decisão** são
consenso de rede — dois nós precisam concordar bit a bit sobre o que é uma
mensagem válida. O transporte (sockets, threads, tempo real) fica fora do
consenso e pode mudar sem quebrar a rede.

### 21.1 Enquadramento

Toda mensagem na rede é um quadro:

```
[4] magic da rede            recusa cru quem fala com a rede errada
u16 versão do protocolo = 1
u16 tipo
u32 tamanho do corpo         <= MAX_FRAME_BODY (2 MiB)
[N] corpo                    codificação canônica da seção 3
```

O `tamanho` vem antes do corpo e é conferido **antes** de alocar qualquer
coisa: um quadro que anuncia 4 GiB é recusado sem reservar 4 GiB. `MAX_FRAME_BODY`
é 2 MiB, o suficiente para o maior bloco (1 MiB) com folga, e um teto que fecha
a exaustão de memória por quadro gigante. Corpo maior que o teto derruba a
conexão, não o nó.

### 21.2 Tipos de mensagem

| Tipo | Nome | Corpo | Para quê |
|---|---|---|---|
| 1 | `HELLO` | versão, magic, altura da ponta, trabalho acumulado, nonce aleatório, porta de escuta | abre a conversa |
| 2 | `HELLO_ACK` | o mesmo, mais o eco do nonce recebido | fecha o aperto de mão |

A **porta de escuta** no `HELLO` diz onde este nó aceita conexões (`0` = só
disca). É o que a descoberta usa para anunciar um par: a porta da conexão de
saída é efêmera e não serve para reconectar.
| 3 | `GET_HEADERS` | locator (lista de hashes) e quantidade pedida | sincronizar por cabeçalhos primeiro |
| 4 | `HEADERS` | lista de cabeçalhos (até 2000) | resposta |
| 5 | `GET_BLOCKS` | lista de hashes de bloco | pedir os blocos inteiros |
| 6 | `BLOCK` | um bloco codificado (seção 7) | resposta, ou anúncio |
| 7 | `GET_ADDRS` | vazio | pedir endereços de outros nós |
| 8 | `ADDRS` | lista de endereços de rede (até 1000) | descoberta de pares |
| 9 | `TX` | uma transação (seção 5) | espalhar transação para o mempool |
| 10 | `PING` / 11 `PONG` | nonce | vivo? |

Tipo desconhecido é ignorado (não derruba a conexão): é o que deixa versões
futuras acrescentarem mensagens sem quebrar as antigas. Versão de protocolo
diferente no `HELLO` encerra a conexão com educação.

### 21.3 Aperto de mão

1. Quem conecta manda `HELLO` com magic, versão, sua ponta e um nonce aleatório.
2. Quem recebe confere magic e versão. Erra qualquer um dos dois, fecha.
3. Responde `HELLO_ACK` ecoando o nonce, com a própria ponta.
4. O nonce ecoado prova que o outro lado respondeu a esta conexão, não a um
   um quadro gravado e repetido.

O aperto de mão **não** cifra ainda. A cifra da conexão (Noise sobre TCP, sem
QUIC porque QUIC puxa C) entra numa versão seguinte do protocolo, e o número de
versão sobe quando entrar. Enquanto não cifra, nada de segredo viaja: só cadeia
pública e transações assinadas, que já são públicas por natureza. Ainda assim,
sem cifra um intermediário vê o tráfego e pode censurar seletivamente, então o
uso hoje é **rede local ou de confiança**, não a internet aberta.

O que falta decidir antes de a cifra entrar, medido em 13/09/2026:

- **Qual padrão Noise.** `XX` autentica os dois lados sem conhecimento prévio;
  `IK` exige saber a chave do par antes de falar. `XX` casa melhor com
  descoberta aberta de pares.
- **Identidade de nó.** A cifra traz uma chave estática por nó, que é uma
  identidade persistente. Isso é bom (autentica o par) e tem custo de
  privacidade (o nó passa a ser reconhecível entre conexões). Precisa de decisão
  explícita, e é separada da chave da carteira (seção 2, hierarquia de chaves).
- **Gerador aleatório.** Nesta máquina o `getrandom` não compila no alvo
  Windows GNU, então o nó fornece o próprio gerador, semeado pela entropia do
  sistema operacional. A biblioteca Noise em Rust puro compila sem `getrandom`.

### 21.4 Sincronização, do jeito seguro

Primeiro cabeçalhos, depois blocos:

1. `GET_HEADERS` com o **locator**: os hashes da própria cadeia, da ponta para
   trás em passos que dobram, terminando na gênese. Quem responde procura o
   primeiro hash que conhece e manda o que veio depois dele. É assim que dois
   nós acham o ancestral comum **mesmo depois de uma bifurcação** — mandar só a
   própria ponta não resolve, porque num ramo divergente o outro lado não a
   conhece. Nenhum hash conhecido: responde desde a gênese.
2. Recebe `HEADERS`, valida cada cabeçalho barato (encadeamento, dificuldade
   esperada, Argon2id) **sem** baixar o corpo. Cabeçalho que não encadeia, ou
   com trabalho de menos, derruba a conexão.
3. Só então `GET_BLOCKS` para os corpos que faltam, e cada bloco entra pelo
   caminho único `accept_block` da seção 14.

Baixar cabeçalho antes de corpo é o que impede um nó mentiroso de fazer o outro
gastar banda e memória com uma cadeia longa e falsa: o cabeçalho já revela que
não tem trabalho, e custa 222 bytes descobrir.

### 21.5 Escolha de cadeia e reorg

A regra é a da seção 15: vence o **maior trabalho acumulado**, não a maior
altura. Ao receber uma cadeia concorrente com mais trabalho, o nó desfaz seus
blocos até o ancestral comum (o `Undo` da seção 13) e aplica a nova. Se a nova
falhar na validação no meio, o nó **volta para a cadeia que tinha**: uma
reorganização que não completa não pode deixar o nó pior do que antes.

Como isso acontece na prática: um bloco que não encadeia na ponta atual é
**guardado** (órfão), não recusado — pode ser um ramo concorrente ainda
chegando. Quando os blocos guardados formam uma sequência contígua a partir de
um ancestral da cadeia ativa, o nó compara:

```
vence o ramo se   trabalho(até o ancestral) + trabalho(ramo)  >  trabalho(minha ponta)
```

Só então desfaz e troca. Os blocos desfeitos viram órfãos, porque aquele ramo
pode voltar a vencer se crescer. O número de órfãos tem **teto**: ao estourar, o
nó descarta todos e recomeça a sincronizar — perder um ramo em construção é
barato, ficar sem memória não é. É defesa contra um par que mande órfãos sem
parar.

### 21.6 Defesas de rede (as regras, não o transporte)

- **Teto por quadro** (`MAX_FRAME_BODY`) e por lista (2000 cabeçalhos, 1000
  endereços, hashes pedidos): nenhuma mensagem faz o nó alocar sem limite.
- **Cabeçalho antes de corpo**: sincronização mentirosa custa barato de recusar.
- **Trabalho, não altura**: cadeia longa e fácil não engana.
- **Sem confiar em endereço anunciado**: `ADDRS` é dica, testada antes de virar
  par; endereço não vira conexão automática.
- **Ban por comportamento, não por identidade**: um par que manda quadro
  inválido, cabeçalho que não encadeia ou bloco que não valida perde pontos e é
  desconectado. É a defesa contra eclipse e envenenamento — descrita como regra
  aqui, medida nos ataques da seção 23.

## 22. Auto-reanimação

**Propriedade central, pedida pelo autor:** enquanto **um único nó** guardar a
cadeia inteira, ele ressemeia a rede toda a partir do zero. A cadeia é o próprio
backup. Não é escudo mágico contra derrubada; é resiliência por redundância, e
quanto mais nós guardam a cadeia, mais impossível apagá-la.

O que torna isso verdade, e não promessa:

1. **A gênese é derivada dos parâmetros, nunca lida de arquivo** (seção 16).
   Um nó zerado recria a gênese idêntica sozinho.
2. **A cadeia se auto-verifica** (seção 21.4 mais o `accept_block` da seção 14).
   Um nó recebe blocos de qualquer par, valida cada um do zero, e chega ao mesmo
   estado. Não precisa confiar em quem mandou.
3. **A gênese carrega uma semente de descoberta.** `extra_nonce` da coinbase da
   gênese pode listar endereços de arranque (ou um nome estável), para um nó
   recém-nascido achar o primeiro par sem coordenador central.
4. **Retomada automática.** Um nó que perdeu todos os pares tenta de novo, em
   intervalos crescentes, os endereços que já conheceu e os da semente. Rede que
   volta é rede que se reconecta sozinha.

O que a auto-reanimação **não** faz, dito com todas as letras: não recupera o
que ninguém guardou. Se, ao mesmo tempo, todo nó do planeta perder a cadeia, ela
acaba — como qualquer dado sem cópia. A defesa é ter muitas cópias, e o desenho
empurra para isso: cada nó completo é uma cópia viva que pode reanimar as
outras.

**Dado pessoal em claro nunca circula.** O que os nós carregam e ressemeiam é a
cadeia pública e compromissos cifrados (seção 2, Data Chain). A reanimação
reconstrói o livro-razão, não expõe dado de ninguém.
