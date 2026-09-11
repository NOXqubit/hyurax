# Auron Flux — Arquitetura Mestre

> Documento de visão do projeto, registrado em 10/09/2026. O texto a partir de
> "Arquitetura original" é o do autor, sem alteração de conteúdo. A seção
> "Notas do projeto" foi acrescentada para registrar o estado real de cada
> parte, as decisões tomadas e os riscos conhecidos.

## Notas do projeto (10/09/2026)

### Estado de cada parte

Pela regra do §88 deste próprio documento: nada vira "tecnologia comprovada"
sem teste.

| Estado | O que significa | O que está nele hoje |
|---|---|---|
| **GREEN** | Implementado e comprovado no Rust, contra os vetores do gabarito em Python | Dinheiro inteiro (`auron-types`). Assinatura Ed25519, endereço, SHA-512 e XOF (`auron-crypto`). |
| **YELLOW** | Testado só na referência em Python | Codificação canônica e árvore de Merkle com prova de inclusão. Transação com nonce por conta e assinatura presa à rede. Cadeia e prova de trabalho Argon2id. Escrow com contabilidade que fecha (UTRAX), que é o molde do invariante de supply do §25. |
| **RED** | Hipótese | Tudo o que é próprio do Flux: stablecoins, padrão ASS, mint e burn, multisig, reserva, oráculo, Auron Pay e `AURONPAY://`, Market, POS, crédito, Synapse, MCP, multi-rail, tecnologias de privacidade e Fusion. |

### Nomes das fases

O projeto já chama de "Fase 1" a migração do núcleo da blockchain para Rust.
Para não confundir, as fases do roadmap deste documento (§87) são chamadas
aqui de **F0 a F10**.

### Decisões

- **AUR-BRL com 8 casas decimais**, como o AUR. O lado bom: reaproveita a
  regra de dinheiro que já existe e já está validada nos dois lados
  (`units.py` e `auron-types`, fixos em 8 casas). O custo: fração abaixo de um
  centavo não tem correspondente numa reserva em reais. Antes de qualquer
  resgate, a especificação do Flux precisa definir o arredondamento (resgate
  sempre para baixo) e o destino da fração que sobra. O teto por conta é o do
  `u64`: cerca de R$ 184,4 bilhões.
- **O núcleo da blockchain vem antes do Flux:** primeiro a codificação,
  depois a transação.
- **Produto para gerar receita no curto prazo:** software que nunca guarda
  dinheiro nem ativo do cliente, ligado a instituições autorizadas. Ver
  "Riscos jurídicos".

### O que falta na cadeia para o Flux existir nela

Medido no código em 10/09/2026:

- A transação só conhece o AUR. Existem dois tipos, coinbase e transferência
  (`reference/auron/tx.py`), e o decodificador recusa qualquer outro.
- A transferência não tem campo de ativo nem de memo. Hoje não dá nem para
  ancorar um pedido de pagamento dentro de uma transação.
- Não existe multisig (uma chave e uma assinatura por transação), nem burn: o
  total emitido só cresce, e o invariante de supply é uma desigualdade
  (circulante menor ou igual ao teto), não a igualdade que o §25 exige.
- Stablecoin dentro da cadeia é mudança de consenso. Pela §20 da
  AURON-SPEC-01, isso significa publicar a AURON-SPEC-02.
- Um simulador fora do consenso não depende de nada disso, assim como o UTRAX
  (§18 da especificação).

### Riscos jurídicos

Isto não é aconselhamento jurídico. São fatos pesquisados em 10/09/2026, com
as fontes no fim desta seção.

- A Lei 14.478/2022 dá ao Banco Central a autorização e a supervisão das
  prestadoras de serviços de ativos virtuais (PSAV). À CVM cabem os tokens que
  forem valor mobiliário.
- As Resoluções BCB 519, 520 e 521, de 2025, estão em vigor desde 02/02/2026.
  Intermediar, custodiar ou negociar ativo virtual exige autorização, com
  capital mínimo de R$ 10,8 milhões a R$ 37,2 milhões, conforme as atividades.
- Quem já operava tem até 30/10/2026 para pedir autorização. Depois dessa
  data, bancos e instituições de pagamento não podem atender prestadora sem
  autorização.
- Compra, venda e troca de ativo referenciado em moeda fiduciária passaram a
  ser tratadas como operação de câmbio. Transferência para carteira
  autocustodiada também.
- A IN BCB 701/2026 exige segregação, prova de reservas e controle de emissão
  e resgate.
- A Resolução BCB 561/2026 proíbe usar stablecoin para liquidar pagamento
  internacional (eFX) a partir de 01/10/2026. Isso atinge direto o uso de
  AUR-USD e AUR-EUR em remessa.
- Quem só fornece software, sem nunca guardar dinheiro nem ativo do cliente,
  fica fora da regra de PSAV, segundo duas fontes independentes. Pode cair em
  outras regras se mexer com pagamento.
- **Conclusão:** simulador e rede de teste sem dinheiro real não dependem de
  nada disso. Qualquer fase com dinheiro de verdade (F5 em diante) exige
  estrutura jurídica e autorização, ou um parceiro autorizado.

### O caso DePix, que o §2 usa como referência

- O DePix é emitido pela empresa Eulen na rede Liquid. Os próprios
  revendedores declaram que ele não é regulado pelo Banco Central. O
  repositório público do projeto foi arquivado em abril de 2026.
- Em 06/09/2026 a Liquid foi atacada. Alguém criou cerca de 4 mil L-BTC sem
  lastro e sacou 95% das reservas em 23 minutos. A causa foi o cache de
  verificação de range proofs, cuja chave era calculada sobre campos de
  tamanho variável **sem prefixo de tamanho**: dava para mover bytes de um
  campo para o outro e cair na mesma chave. O atacante devolveu 3.400 BTC.
- O DePix não foi o alvo, mas ficou travado enquanto a rede esteve parada. A
  rede voltou em 10/09/2026, com a versão 23.3.4 do Elements.
- **Lição direta para o Auron:** nenhum resumo criptográfico pode ser tirado
  de campos variáveis sem prefixo de tamanho. A codificação canônica do Auron
  já obriga isso, e o `auron-codec` trava a regra em vetores.

### Conflito a resolver

O §4.1 lista "investimento" e "possibilidade de valorização" para o AUR, e o
README diz que não existe token com valor. Oferecer AUR com promessa de
valorização pode configurar oferta de valor mobiliário. Enquanto não houver
rede principal nem estrutura jurídica, nada de venda ou pré-venda.

### Relação com a especificação

A §19 da AURON-SPEC-01 diz que o Fusion está congelado. Este documento reabre
o Fusion como terceira categoria (F10). A especificação não muda agora.

### Fontes

- Ataque à Liquid:
  [CertiK](https://www.certik.com/blog/liquid-network-incident-analysis),
  [crypto.news](https://crypto.news/liquid-network-320-million-drain-cache-bug-unbacked-bitcoin/),
  [SpendNode](https://www.spendnode.io/blog/liquid-network-resumes-block-production-emergency-patch-september-2026/),
  [SpaceMoney](https://www.spacemoney.com.br/investimentos/criptomoedas/depix-stablecoin-liquid-parada),
  [TechCripto](https://techcripto.com/noticias/depix-hack-liquid-network/09/2026/)
- DePix:
  [documentação da Eulen](https://docs.eulen.app/),
  [repositório DePix](https://github.com/eulen-repo/DePix),
  [Área Bitcoin](https://blog.areabitcoin.com.br/depix/)
- Regulação:
  [Conjur, 01/07/2026](https://www.conjur.com.br/2026-jul-01/o-cerco-do-banco-central-as-stablecoins/),
  [Conjur, 09/06/2026](https://conjur.com.br/2026-jun-09/inovacao-mas-com-controle-bc-restringe-uso-de-stablecoins-em-pagamentos-internacionais/),
  [Transfeera](https://transfeera.com/blog/psav-regulacao-banco-central/),
  [NDM Advogados](https://ndmadvogados.com.br/artigo/psavs-resolucoes-bcb-no-519-520-e-521/),
  [Azify](https://azify.com/blog/o-que-e-psav-no-brasil-em-2026),
  [ConduitPay](https://conduitpay.com/blog/crypto-regulation-news-brazil),
  [Monitor do Mercado](https://monitordomercado.com.br/noticias/421139-opiniao-a-batalha-de-enquadramento-legal-das-stablecoins-no-brasil/)

---

# Arquitetura original

**AURON TECHNOLOGY**

**AURON FLUX**

Arquitetura Mestre do Sistema de Stablecoins, Pagamentos, Liquidação e Mercado

- Versão: 1.0
- Status: Arquitetura / Engenharia
- Classificação: Projeto de P&D
- Ecossistema: Auron Technology

## 1. VISÃO

O Auron Flux será a infraestrutura financeira do ecossistema Auron.

Seu objetivo é criar uma camada capaz de representar digitalmente diferentes
moedas fiduciárias e conectá-las ao universo de ativos digitais, blockchain,
pagamentos e comércio.

O sistema deverá permitir:

- stablecoins 1:1;
- pagamentos digitais;
- conversão entre moedas;
- pagamentos cripto;
- pagamentos fiduciários através de parceiros;
- comércio digital;
- comércio físico;
- checkout;
- QR;
- liquidação;
- emissão;
- resgate;
- custódia e autocustódia;
- prova de reservas;
- auditoria;
- integração com empresas;
- futura infraestrutura de crédito.

A ideia fundamental é:

Uma infraestrutura financeira única capaz de transportar valor entre diferentes
sistemas monetários sem exigir que o usuário compreenda a complexidade
tecnológica existente por baixo.

## 2. REFERÊNCIA: MODELO DEPIX

O DePix demonstra uma arquitetura prática de representação do real em
blockchain.

O conceito fundamental é:

```text
BRL
 │
 │ 1:1
 ▼
DePix
 │
 ▼
Blockchain
 │
 ▼
Wallet
 │
 ▼
Pagamento / Transferência
```

No caminho contrário:

```text
DePix
 │
 ▼
Burn / Resgate
 │
 ▼
BRL
 │
 ▼
PIX
```

O modelo atual utiliza a Liquid Network como infraestrutura do ativo e depende
de uma estrutura de emissão/resgate.

O Auron Flux deverá aproveitar o princípio, mas não ficar limitado a uma única
moeda ou a uma única função.

## 3. O QUE O AURON DEVE FAZER DIFERENTE

O Auron não deve simplesmente criar:

"um DePix dentro da blockchain Auron."

Isso seria pequeno demais para a visão do projeto.

O objetivo é criar uma:

AURON DIGITAL CURRENCY INFRASTRUCTURE

capaz de suportar:

```text
BRL
USD
EUR
GBP
JPY
outras moedas
        │
        ▼
Auron Stable Standard
        │
        ▼
Auron Flux
        │
        ▼
Auron Blockchain
        │
        ├── Wallet
        ├── Pay
        ├── Market
        ├── Exchange
        ├── Credit
        └── Commerce
```

## 4. FAMÍLIA DE ATIVOS AURON

O ecossistema deverá separar claramente três categorias.

### 4.1 AUR

AUR é o ativo nativo do ecossistema Auron.

Características:

- preço de mercado próprio;
- oferta definida pelo protocolo;
- possibilidade de valorização;
- possibilidade de desvalorização;
- uso dentro da economia Auron;
- investimento;
- reserva;
- pagamento;
- taxas;
- serviços;
- incentivos;
- economia da rede.

AUR NÃO é uma stablecoin.

## 5. AURON STABLE

A segunda categoria será formada pelas stablecoins.

Exemplos:

```text
AUR-BRL
AUR-USD
AUR-EUR
AUR-GBP
AUR-JPY
```

Cada ativo possui uma referência monetária.

Exemplo:

```text
1 AUR-BRL ≈ R$1
1 AUR-USD ≈ US$1
1 AUR-EUR ≈ €1
```

A paridade deve ser definida por um mecanismo econômico verificável.

Não basta escrever:

```text
price = 1
```

no código.

A arquitetura precisa sustentar economicamente essa relação.

## 6. TERCEIRA CATEGORIA — FUSION

A terceira categoria será o ativo/protocolo Fusion.

Ele não deverá ser simplesmente outra stablecoin.

O Fusion deverá possuir finalidade econômica própria.

Possíveis funções:

- participação em mecanismos econômicos especiais;
- infraestrutura experimental;
- utilidade;
- incentivos;
- governança futura;
- mecanismos de liquidez;
- aplicações tecnológicas;
- integração com tecnologias experimentais Auron.

A definição econômica do Fusion deverá ser feita separadamente.

## 7. PRINCÍPIO FUNDAMENTAL

As três categorias não devem competir.

```text
                 AURON
                   │
       ┌───────────┼───────────┐
       │           │           │
       ▼           ▼           ▼
      AUR       AUR-STABLE   FUSION
       │           │           │
       │           │           │
   economia     estabilidade   função
   nativa       monetária      especial
```

Isso evita criar três moedas fazendo exatamente a mesma coisa.

## 8. AURON STABLE STANDARD

Criar um padrão interno:

ASS — Auron Stable Standard

Esse padrão define como qualquer stablecoin Auron deverá funcionar.

Cada stablecoin deverá possuir:

```text
asset_id
symbol
reference_currency
target_price
issuer_model
collateral_model
reserve_policy
mint_policy
burn_policy
redemption_policy
oracle_policy
risk_parameters
audit_policy
compliance_policy
```

## 9. MODELOS DE LASTRO

O Auron Flux deverá ser projetado para suportar mais de um modelo.

### MODELO A — FIAT-BACKED

Exemplo:

```text
R$1 reservado
     ↓
1 AUR-BRL emitido
```

Esse modelo é conceitualmente semelhante ao modelo DePix.

Vantagens:

- simples de entender;
- paridade direta;
- baixa complexidade financeira;
- fácil integração comercial.

Desvantagens:

- depende de instituições financeiras;
- depende de reservas;
- depende do emissor/custodiante;
- existe risco jurídico/regulatório;
- existe risco bancário.

## 10. MODELO B — CRYPTO-COLLATERALIZED

Outra possibilidade:

```text
Cripto colateralizada
       ↓
protocolo
       ↓
AUR-BRL
```

Nesse modelo, a garantia pode existir on-chain.

Vantagens:

- maior verificabilidade on-chain;
- menor dependência de conta bancária;
- maior potencial de descentralização.

Desvantagens:

- maior complexidade;
- risco de volatilidade;
- necessidade de sobrecolateralização;
- necessidade de liquidação;
- necessidade de oráculos;
- risco de depeg.

## 11. MODELO C — HÍBRIDO

O Auron poderá futuramente utilizar:

```text
Fiat
 +
Cripto
 +
Reserva
 +
Hedge
 +
Liquidity Pool
```

para formar uma arquitetura híbrida.

Isso não deverá ser implementado inicialmente sem modelagem econômica e testes.

## 12. PRINCÍPIO DE NÃO CONFUNDIR DESCENTRALIZAÇÃO

Uma stablecoin lastreada em BRL mantido em banco não é completamente
descentralizada.

Mesmo que:

- blockchain seja descentralizada;
- wallet seja não custodial;
- transações sejam P2P;

a reserva fiduciária ainda depende de uma instituição.

Portanto:

```text
Blockchain descentralizada
≠
stablecoin completamente descentralizada
```

O Auron deverá documentar exatamente qual camada é descentralizada e qual
camada depende de confiança institucional.

## 13. AURON FLUX

O Flux será a camada de transporte financeiro.

Arquitetura:

```text
                    AURON FLUX
                        │
        ┌───────────────┼────────────────┐
        │               │                │
        ▼               ▼                ▼
     AURON PAY     AURON MARKET      AURON WALLET
        │               │                │
        └───────────────┼────────────────┘
                        │
                        ▼
                 AURON SETTLEMENT
                        │
                        ▼
                 AURON BLOCKCHAIN
```

## 14. AURON PAY

Auron Pay será a camada de pagamento.

Funções:

- pagamento;
- recebimento;
- QR;
- invoices;
- checkout;
- merchant API;
- webhooks;
- confirmação;
- refund;
- conversão;
- recorrência quando juridicamente e tecnicamente apropriada.

## 15. PAYMENT REQUEST

Criar um protocolo próprio:

```text
AURONPAY://
```

Exemplo conceitual:

```text
payment_id
merchant_id
destination
asset
amount
currency
expiration
nonce
network
metadata_hash
signature
```

O QR não deve conter informações pessoais desnecessárias.

## 16. PAGAMENTO

Fluxo:

```text
CLIENTE
   │
   ▼
QR / PAYMENT REQUEST
   │
   ▼
AURON WALLET
   │
   ▼
CONFIRMAÇÃO
   │
   ▼
ASSINATURA
   │
   ▼
AURON FLUX
   │
   ▼
BLOCKCHAIN
   │
   ▼
CONFIRMAÇÃO
   │
   ▼
MERCHANT
```

## 17. PAGAMENTO COM AUR

Exemplo:

Produto:

R$100

Cliente possui AUR.

O sistema pode:

```text
AUR
 ↓
cotação
 ↓
valor equivalente
 ↓
Auron Pay
 ↓
merchant
```

O comerciante poderá escolher:

```text
Receber AUR
```

ou:

```text
Receber AUR-BRL
```

ou outro ativo permitido.

## 18. PAGAMENTO COM AUR-BRL

O comerciante define:

```text
Preço = 100 AUR-BRL
```

O cliente paga:

```text
100 AUR-BRL
```

Como o ativo tem como objetivo acompanhar o BRL:

```text
100 AUR-BRL ≈ R$100
```

O comerciante não precisa assumir diretamente a volatilidade do AUR.

## 19. CONVERSÃO AUTOMÁTICA

Futuramente:

```text
Cliente
 possui AUR

       ↓

Merchant aceita AUR-BRL

       ↓

Auron Flux
       ↓
conversão
       ↓
AUR-BRL

       ↓

Merchant
```

Isso cria uma camada de abstração financeira.

## 20. AURON MARKET

O Auron Market deverá utilizar IDs únicos para produtos.

Exemplo:

```text
product_id:
AURON-MKT-00000001
```

Cada produto poderá possuir:

```text
product_id
merchant_id
name
description
price
currency
inventory
category
metadata_hash
status
```

## 21. CHECKOUT

```text
Produto
 ↓
Product ID
 ↓
Carrinho
 ↓
Checkout
 ↓
Preço
 ↓
Moeda
 ↓
Auron Pay
 ↓
Pagamento
 ↓
Liquidação
 ↓
Pedido confirmado
```

## 22. LIQUIDAÇÃO

A liquidação deve ser separada da interface.

```text
USER EXPERIENCE
       │
       ▼
AURON PAY
       │
       ▼
AURON FLUX
       │
       ▼
SETTLEMENT ENGINE
       │
       ▼
BLOCKCHAIN
```

Isso permite trocar componentes sem destruir todo o sistema.

## 23. MINT

O nascimento de uma stablecoin deverá seguir regras rígidas.

Exemplo:

```text
Reserva verificada
       ↓
Mint authorization
       ↓
Mint
       ↓
AUR-BRL
```

Nunca:

```text
API
 ↓
mint(1000000000)
```

sem mecanismo de autorização.

## 24. BURN

O caminho inverso:

```text
AUR-BRL
   ↓
redeem request
   ↓
verificação
   ↓
burn / lock
   ↓
resgate
   ↓
BRL
```

## 25. SUPPLY INVARIANT

Um princípio fundamental:

```text
TOTAL_STABLE_ISSUED
-
TOTAL_STABLE_BURNED
=
TOTAL_STABLE_CIRCULATING
```

E, dependendo do modelo:

```text
RESERVE_VALUE
≈
CIRCULATING_SUPPLY
```

para um modelo fiat-backed 1:1.

Qualquer discrepância deve gerar alerta.

## 26. PROOF OF RESERVES

Criar:

Auron Reserve Transparency Engine

Responsável por publicar/verificar:

- supply;
- emissão;
- burn;
- reservas;
- timestamp;
- auditoria;
- discrepâncias;
- cobertura.

Quando tecnicamente possível:

- provas criptográficas;
- attestations;
- auditoria independente;
- segregação de reservas;
- relatórios periódicos.

## 27. RESERVA SEGREGADA

A reserva de uma stablecoin não deve ser misturada arbitrariamente com:

- capital operacional;
- tesouraria;
- dinheiro do desenvolvedor;
- fundos do AUR;
- fundos do Fusion.

Separar contabilmente e operacionalmente.

## 28. AURON RESERVE VAULT

Criar uma camada lógica:

```text
AURON RESERVE VAULT
```

com:

```text
reserve_id
asset
currency
amount
custodian
timestamp
attestation
status
```

## 29. MULTI-SIGNATURE

Operações críticas devem utilizar múltiplas autorizações.

Exemplo:

```text
Mint
 ↓
Policy Engine
 ↓
Risk Check
 ↓
Multi-signature
 ↓
Execution
```

Nenhuma chave isolada deveria ter capacidade ilimitada.

## 30. HOT / COLD RESERVE

Separar:

```text
HOT RESERVE
```

para liquidez operacional.

e

```text
COLD RESERVE
```

para maior proteção.

Limites rígidos deverão existir entre elas.

## 31. AURON ORACLE

Para conversões e ativos não 1:1, criar:

Auron Oracle Layer

Fontes múltiplas.

```text
Exchange A
Exchange B
Exchange C
Market Data
      ↓
Oracle Aggregator
      ↓
Median
      ↓
Validation
      ↓
Auron Flux
```

Nunca depender de uma única cotação sem proteção.

## 32. ORACLE SECURITY

Implementar:

- medianização;
- timestamp;
- heartbeat;
- limites;
- circuit breaker;
- detecção de preço anormal;
- fallback;
- múltiplas fontes;
- proteção contra manipulação.

## 33. PRIVACIDADE

A arquitetura deverá evitar colocar dados pessoais diretamente na blockchain.

Separar:

```text
IDENTIDADE
TRANSAÇÃO
COMÉRCIO
LIQUIDAÇÃO
```

A blockchain deve registrar somente aquilo que precisa ser verificável.

Dados pessoais devem permanecer em sistemas adequados.

## 34. CONFIDENTIALIDADE

O Auron deverá pesquisar e, quando apropriado, implementar tecnologias de
privacidade como:

- confidential transactions;
- zero-knowledge proofs;
- commitments;
- stealth addressing;
- selective disclosure.

A tecnologia específica dependerá do desenho final da Auron Blockchain.

## 35. AURON ID

Criar uma identidade financeira modular.

```text
Auron ID
    │
    ├── Wallet
    ├── Merchant
    ├── Device
    ├── Authorization
    └── Compliance
```

Auron ID NÃO deve significar exposição pública da identidade do usuário.

## 36. COMPLIANCE POR CAMADAS

A arquitetura deverá permitir diferentes níveis de acesso.

Exemplo:

```text
Anonymous public blockchain data
          │
          ▼
Wallet
          │
          ▼
Verified user
          │
          ▼
Merchant
          │
          ▼
Institution
```

Cada função possui permissões diferentes.

## 37. CRIPTOCRÉDITO

Futuro módulo:

Auron Credit

O crédito não deve ser confundido com stablecoin.

```text
Saldo insuficiente
        ↓
Credit Engine
        ↓
Eligibility
        ↓
Limit
        ↓
Approval
        ↓
Merchant Settlement
        ↓
Debt
```

A dívida é um estado financeiro diferente do saldo monetário.

## 38. AURON CREDIT ENGINE

Pode considerar:

- histórico;
- capacidade;
- comportamento;
- limite;
- risco;
- fraude;
- inadimplência;
- garantias;
- tempo de relacionamento.

O motor deverá ser auditável.

## 39. PROTEÇÃO CONTRA CRÉDITO INFINITO

Nunca permitir:

```text
AI
 ↓
credit(∞)
```

Existirão:

```text
maximum_credit_limit
daily_limit
transaction_limit
risk_limit
collateral_limit
```

## 40. AURON SYNAPSE

O Synapse será a inteligência do ecossistema.

Pode atuar em:

- fraude;
- risco;
- previsão;
- roteamento;
- liquidez;
- análise;
- detecção de anomalias;
- otimização.

Mas:

Synapse não será o dono do dinheiro.

A IA pode:

```text
ANALYZE
PREDICT
RECOMMEND
DETECT
SIMULATE
```

A execução financeira crítica deve exigir:

```text
POLICY
+
AUTHORIZATION
+
SIGNATURE
```

## 41. MCP

MCP pode conectar o Synapse às ferramentas externas.

Exemplos:

```text
Synapse
   │
   ▼
MCP
   ├── GitHub
   ├── Design
   ├── Figma
   ├── Browser
   ├── Database
   ├── Cloud
   ├── Monitoring
   └── Testing
```

MCP não deve receber automaticamente autoridade para:

- mint;
- transferir fundos;
- alterar reservas;
- alterar consenso.

## 42. SEGURANÇA DE AGENTES

Separar:

```text
READ
ANALYZE
SIMULATE
PROPOSE
```

de:

```text
AUTHORIZE
SIGN
EXECUTE
```

Um agente comprometido não pode controlar sozinho o sistema financeiro.

## 43. THREAT MODEL

O Auron Flux deverá ser testado contra:

1. Double-spend
2. Replay attack
3. Fake payment
4. QR substitution
5. QR forgery
6. Signature forgery
7. Nonce reuse
8. Unauthorized mint
9. Unauthorized burn
10. Reserve manipulation
11. Oracle manipulation
12. Price manipulation
13. Sybil
14. API abuse
15. Rate-limit bypass
16. Race condition
17. Integer overflow
18. Precision attack
19. Transaction duplication
20. Merchant impersonation
21. Wallet compromise
22. Key theft
23. Privilege escalation
24. Blockchain reorganization
25. Network partition
26. Node failure
27. Database inconsistency
28. Refund abuse
29. Credit abuse
30. Insider attack
31. AI prompt injection
32. MCP tool abuse
33. Supply mismatch
34. Reserve mismatch
35. Depeg attack
36. Liquidity attack

## 44. AURON SECURITY LAB

Criar ambiente separado:

```text
AURON SECURITY LAB
```

Com:

- testnet;
- wallets falsas;
- merchants fictícios;
- stablecoins de teste;
- nodes controlados;
- fuzzing;
- load testing;
- fault injection;
- adversarial testing;
- simulation;
- monitoring.

## 45. FUZZING

Testar:

```text
payment_id
amount
nonce
asset
address
signature
timestamp
expiration
metadata
```

com:

- valores inválidos;
- valores extremos;
- dados corrompidos;
- inputs gigantes;
- entradas vazias;
- caracteres inesperados;
- operações concorrentes.

## 46. TESTE DE DOUBLE-SPEND

Executar:

```text
Wallet A
 │
 ├── Transaction 1 → Merchant A
 │
 └── Transaction 2 → Merchant B
```

com o mesmo saldo.

O sistema deve aceitar apenas a transação válida conforme as regras de
consenso.

## 47. TESTE DE REPLAY

Capturar uma transação válida e tentar executá-la novamente.

Esperado:

```text
REJECTED
```

devido a:

- nonce;
- transaction ID;
- state;
- replay protection.

## 48. TESTE DE MINT

Tentativas:

```text
unauthorized_mint()
fake_reserve()
invalid_signature()
reused_signature()
overflow_mint()
```

Todas devem falhar.

## 49. TESTE DE RESERVA

Simular:

```text
Supply = 1.000.000
Reserve = 999.999
```

O sistema deve detectar a inconsistência.

## 50. TESTE DE DEPEG

Simular:

```text
AUR-BRL = R$0,98
```

e:

```text
AUR-BRL = R$1,05
```

O sistema deverá:

- detectar;
- alertar;
- identificar causa;
- avaliar liquidez;
- ativar mecanismos definidos;
- impedir decisões automáticas perigosas.

## 51. CIRCUIT BREAKER

Em caso de comportamento anormal:

```text
NORMAL
 ↓
ANOMALY
 ↓
WARNING
 ↓
RESTRICTED
 ↓
PAUSED
```

Operações críticas podem ser temporariamente suspensas.

## 52. KILL SWITCH

Existirá mecanismo de emergência para:

- interromper emissão;
- interromper integração;
- congelar novas operações administrativas;
- impedir dano sistêmico.

O kill switch deve possuir:

- controle de acesso;
- multisig;
- auditoria;
- procedimento de recuperação.

Não deve permitir arbitrariamente confiscar fundos dos usuários.

## 53. IDEMPOTÊNCIA

Todo pagamento possuirá:

```text
payment_id
```

Se a mesma solicitação chegar duas vezes:

```text
request
request
```

o sistema não deve gerar:

```text
payment
payment
```

mas:

```text
same payment state
```

## 54. ESTADOS DO PAGAMENTO

```text
CREATED
   ↓
AUTHORIZED
   ↓
SUBMITTED
   ↓
PENDING
   ↓
CONFIRMED
   ↓
SETTLED
```

Estados alternativos:

```text
FAILED
REJECTED
EXPIRED
CANCELLED
REFUNDED
DISPUTED
```

## 55. EVENT SOURCING

Eventos importantes deverão ser registrados:

```text
PaymentCreated
PaymentAuthorized
PaymentSubmitted
PaymentConfirmed
PaymentSettled
PaymentRefunded
MintAuthorized
MintExecuted
BurnExecuted
ReserveUpdated
RiskAlert
SecurityIncident
```

Isso facilita auditoria e recuperação.

## 56. BLOCKCHAIN NÃO DEVE SER O BANCO DE DADOS DO MARKET

O Market poderá utilizar:

- PostgreSQL;
- cache;
- indexadores;
- armazenamento distribuído;
- object storage.

Blockchain:

```text
ownership
payments
settlement
mint
burn
financial state
```

Off-chain:

```text
images
search
catalog
descriptions
analytics
inventory cache
```

## 57. AURON SETTLEMENT ENGINE

Camada responsável por finalizar operações.

```text
Payment
 ↓
Validation
 ↓
Risk
 ↓
Routing
 ↓
Settlement
 ↓
Blockchain
 ↓
Confirmation
```

## 58. MULTI-RAIL

O Flux deverá futuramente conseguir trabalhar com múltiplos trilhos.

```text
AURON
Liquid
Bitcoin
Lightning
EVM
outras redes
sistemas fiduciários
```

O usuário não precisa necessariamente saber em qual trilho o pagamento está
sendo liquidado.

## 59. OMNI-ASSET

Uma mesma carteira poderá visualizar:

```text
AUR
AUR-BRL
AUR-USD
AUR-EUR
FUSION
BTC
outros ativos autorizados
```

## 60. SMART ROUTING

O Auron Flux poderá escolher o melhor caminho com base em:

- custo;
- velocidade;
- liquidez;
- segurança;
- disponibilidade;
- privacidade;
- finalidade.

Exemplo:

```text
Pagamento
   ↓
Flux Router
   ├── Auron Chain
   ├── Lightning
   ├── outra rede
   └── settlement partner
```

## 61. MERCHANT TERMINAL

Criar:

Auron POS

Pode funcionar em:

- celular;
- tablet;
- computador;
- terminal dedicado.

Funções:

- gerar cobrança;
- QR;
- receber;
- confirmar;
- refund;
- histórico;
- fechamento.

## 62. AURON POS + MARKET

Fluxo:

```text
Produto
 ↓
POS
 ↓
Product ID
 ↓
Preço
 ↓
QR
 ↓
Cliente
 ↓
Wallet
 ↓
Auron Flux
 ↓
Merchant
```

## 63. PAGAMENTO SEM ENTENDER CRIPTO

Objetivo UX:

O usuário não precisa saber:

- blockchain;
- gas;
- UTXO;
- hash;
- consenso;
- oracle;
- settlement.

Ele vê:

```text
Pagar R$25,00
```

e confirma.

A infraestrutura resolve o resto.

## 64. TRANSAÇÃO FINANCEIRA

Internamente:

```text
User
 ↓
Wallet
 ↓
Authentication
 ↓
Authorization
 ↓
Risk
 ↓
Routing
 ↓
Settlement
 ↓
Blockchain
 ↓
Merchant
```

## 65. TAXAS

O Flux deverá possuir uma camada explícita de fees.

```text
network_fee
service_fee
conversion_fee
merchant_fee
settlement_fee
```

Nunca esconder custos.

## 66. AUDITORIA

Registrar:

- payment ID;
- transaction ID;
- wallet;
- merchant;
- timestamp;
- asset;
- amount;
- status;
- fee;
- network;
- settlement;
- security events.

## 67. PRIVACIDADE DO MERCHANT

O sistema não deverá expor desnecessariamente:

- CPF;
- CNPJ;
- endereço;
- dados bancários;
- dados internos;
- informações de clientes.

## 68. DISPUTAS

Criar:

Auron Dispute Engine

Estados:

```text
OPEN
INVESTIGATING
EVIDENCE
RESOLUTION
CLOSED
```

Importante:

Uma disputa comercial não deve permitir alteração arbitrária de uma transação
blockchain já finalizada.

A solução pode ocorrer por:

- refund;
- escrow;
- arbitragem;
- crédito comercial;
- compensação.

## 69. ESCROW

Futuro módulo:

```text
Buyer
  ↓
Escrow
  ↓
Merchant
```

O dinheiro fica bloqueado até determinadas condições serem cumpridas.

## 70. AURON CREDIT + MARKET

Futuro:

```text
Produto R$1.000
       ↓
Saldo = R$200
       ↓
Crédito = R$800
       ↓
Merchant recebe conforme política
       ↓
Cliente possui dívida
```

A arquitetura deve separar o saldo líquido da obrigação de crédito.

## 71. RESILIÊNCIA

O sistema deverá sobreviver a:

- Node offline;
- API offline;
- banco offline;
- congestionamento;
- falha de rede;
- perda de conexão;
- falha de provedor;
- ataque.

Usar:

- retries;
- timeout;
- queues;
- circuit breakers;
- failover;
- idempotência;
- recovery.

## 72. SEGURANÇA DE CHAVES

Chaves críticas:

- emissão;
- burn;
- treasury;
- governance;
- admin.

Devem utilizar mecanismos apropriados como:

- HSM;
- multisig;
- cold storage;
- key rotation;
- access policy.

## 73. SEPARAÇÃO DE PODERES

Nenhuma entidade deverá concentrar todas as funções.

Separar:

```text
Issuer
Reserve
Validator
Treasury
Developer
Auditor
Risk
Governance
```

Quanto mais crítico o componente, maior deve ser a segregação.

## 74. PRINCÍPIO ZERO TRUST

Nenhum Node deve ser confiável automaticamente.

Sempre verificar:

- identidade;
- assinatura;
- permissão;
- integridade;
- estado;
- reputação;
- contexto.

## 75. AURON NODE

Nodes podem participar de:

- blockchain;
- pagamentos;
- liquidação;
- indexação;
- compute;
- armazenamento;
- serviços.

Cada Node possui identidade criptográfica.

## 76. AURON SYNAPSE + SECURITY

Synapse poderá observar:

```text
volume
frequência
geografia aproximada
padrões
falhas
fraude
latência
liquidez
```

e identificar:

```text
normal
suspeito
anômalo
crítico
```

Mas recomendações críticas devem passar por políticas determinísticas.

## 77. MCP SECURITY

MCP deverá possuir:

```text
Tool
 ↓
Permission
 ↓
Scope
 ↓
Policy
 ↓
Authorization
 ↓
Execution
```

Um MCP comprometido não poderá automaticamente acessar todos os sistemas.

## 78. ARQUITETURA COMPLETA

```text
                         AURON SYNAPSE
                               │
                               ▼
                         AURON MCP LAYER
                               │
                 ┌─────────────┼─────────────┐
                 │             │             │
              GitHub        Testing        Cloud
                 │
                 ▼
        ┌────────────────────────────┐
        │        AURON FLUX          │
        │                            │
        │ Payment                    │
        │ Settlement                 │
        │ Routing                    │
        │ Conversion                 │
        │ Risk                       │
        │ Oracle                     │
        │ Credit                     │
        └──────────────┬─────────────┘
                       │
          ┌────────────┼────────────┐
          │            │            │
          ▼            ▼            ▼
      AURON PAY    AURON MARKET   AURON WALLET
          │            │            │
          └────────────┼────────────┘
                       │
                       ▼
                 AURON BLOCKCHAIN
                       │
          ┌────────────┼────────────┐
          │            │            │
          ▼            ▼            ▼
         AUR       AUR-STABLE     FUSION
                      │
             ┌────────┼────────┐
             ▼        ▼        ▼
           BRL       USD      EUR
```

## 79. PRIMEIRO PROTÓTIPO

Antes de dinheiro real:

Criar:

```text
Alice
Bob
Merchant
Issuer
Reserve
Blockchain
```

Simular:

```text
1 BRL
↓
1 AUR-BRL
```

Depois:

```text
AUR-BRL
↓
payment
↓
merchant
↓
burn
↓
BRL redemption simulation
```

## 80. TESTES

1. Unit tests
2. Integration tests
3. Property-based tests
4. Fuzzing
5. Load tests
6. Adversarial tests
7. External security audit
8. Somente depois: Mainnet

## 81. TESTE DE CARGA

Simular:

```text
10 TPS
100 TPS
1.000 TPS
10.000 TPS
100.000 TPS
```

medindo:

- latência;
- throughput;
- erro;
- confirmação;
- CPU;
- RAM;
- bandwidth;
- recovery.

Os valores reais não devem ser prometidos antes dos benchmarks.

## 82. TESTE DE FALHA

Simular:

```text
Node offline
Validator offline
Oracle offline
Database offline
Network partition
Wallet offline
API timeout
Blockchain congestion
```

O sistema deve ter comportamento definido para cada caso.

## 83. TESTE DE CONSISTÊNCIA

Verificar:

```text
Blockchain balance
=
Index balance
=
Wallet balance
=
Settlement balance
```

quando aplicável.

Qualquer divergência deve gerar incidente.

## 84. TESTE DE RESERVA

Automatizar:

```text
circulating_supply
vs
reserve_value
```

e gerar:

```text
coverage_ratio
```

Exemplo:

```text
Coverage = Reserve / Supply
```

Para uma stablecoin fiat-backed 1:1, a política deverá buscar cobertura
integral, observadas as regras e estrutura jurídica escolhidas.

## 85. DASHBOARD

Criar:

Auron Flux Observatory

Exibir:

```text
AUR supply
Stable supply
Mint
Burn
Reserve
Coverage
Transactions
Volume
Liquidity
Failures
Security alerts
Nodes
Latency
```

## 86. TRANSPARÊNCIA

Usuário deverá conseguir verificar:

```text
Quanto existe?
Quanto foi emitido?
Quanto foi queimado?
Qual é a política?
Qual é o ativo?
Qual é o emissor?
Como ocorre o resgate?
Qual é o risco?
```

Auron não deve vender "confiança cega".

## 87. ROADMAP

**FASE 0 — ESPECIFICAÇÃO.** Documentação. Threat model. Economia. Modelo
jurídico.

**FASE 1 — SIMULADOR.** Stablecoin fictícia. Mint. Burn. Reserve. Payments.

**FASE 2 — TESTNET.** AUR-BRL de teste. Wallet. QR. Payment API.

**FASE 3 — MARKET.** Products. Checkout. Merchant. Auron POS.

**FASE 4 — SECURITY LAB.** Ataques. Fuzzing. Load. Fault injection.

**FASE 5 — RESERVE ENGINE.** Proof of reserves. Audit. Multisig. HSM.

**FASE 6 — MAINNET EXPERIMENTAL.** Somente após validação.

**FASE 7 — MULTI-CURRENCY.** AUR-USD. AUR-EUR. AUR-GBP. etc.

**FASE 8 — SMART ROUTING.** Conversão automática. Liquidity routing.
Multi-rail.

**FASE 9 — CREDIT.** Auron Credit.

**FASE 10 — FUSION.** Integração econômica do Fusion.

## 88. PRINCÍPIO DE ENGENHARIA

O projeto deverá sempre distinguir:

- **GREEN:** implementado e comprovado.
- **YELLOW:** protótipo ou engenharia experimental.
- **RED:** hipótese ou pesquisa.

Nunca transformar:

```text
IDEIA
```

em:

```text
TECNOLOGIA COMPROVADA
```

sem testes.

## 89. PRINCÍPIO ECONÔMICO

AUR:

```text
valor de mercado
```

Stable:

```text
valor de referência
```

Fusion:

```text
função econômica própria
```

Não misturar as políticas monetárias.

## 90. PRINCÍPIO FINAL

O objetivo do Auron Flux não é criar apenas uma moeda.

É criar uma:

INFRAESTRUTURA FINANCEIRA DIGITAL

na qual:

```text
AUR
AUR-BRL
AUR-USD
AUR-EUR
FUSION
```

possam coexistir.

O usuário poderá:

```text
guardar
transferir
pagar
receber
investir
comprar
vender
converter
```

sem precisar compreender toda a infraestrutura existente por baixo.

## 91. VISÃO FINAL

```text
                    AURON TECHNOLOGY
                           │
                           ▼
                    AURON SYNAPSE
                           │
                           ▼
                       AURON FLUX
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
    AURON PAY         AURON MARKET       AURON WALLET
        │                  │                  │
        └──────────────────┼──────────────────┘
                           │
                           ▼
                    AURON BLOCKCHAIN
                           │
             ┌─────────────┼─────────────┐
             │             │             │
             ▼             ▼             ▼
            AUR       AURON STABLE     FUSION
                          │
              ┌───────────┼───────────┐
              │           │           │
             BRL         USD         EUR
              │           │           │
              └───────────┼───────────┘
                          │
                          ▼
                  GLOBAL DIGITAL MARKET
```

A visão final é transformar o Auron em uma infraestrutura na qual valor possa
circular entre moedas fiduciárias, ativos digitais, comércio e blockchain
através de uma camada única de liquidação.

O Auron Flux deverá ser construído para que o usuário enxergue simplicidade
enquanto a infraestrutura executa segurança, roteamento, liquidação,
auditoria, conversão e controle de risco por baixo.

Regra máxima:

**Simples para o usuário.**

**Extremamente rigoroso por baixo.**

FIM DA ARQUITETURA MESTRE.
