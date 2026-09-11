# Auron — Arquitetura Integrada de Comunicação, Pagamentos P2P e Offline Mesh

> Documento de arquitetura do projeto, registrado em 11/09/2026. O texto a
> partir de "Arquitetura original" é o do autor, sem alteração de conteúdo. A
> seção "Notas do projeto" foi acrescentada para registrar o estado real, como
> o documento se encaixa no que já existe, as decisões tomadas e os pontos de
> atenção.

## Notas do projeto (11/09/2026)

### Estado de cada parte

| Estado | O que está nele hoje |
|---|---|
| **GREEN** — implementado e comprovado no Rust, contra os vetores do gabarito | Codificação canônica e árvore de Merkle (`auron-codec`). Assinatura Ed25519, endereço, SHA-512 e XOF (`auron-crypto`). Dinheiro inteiro (`auron-types`). |
| **YELLOW** — testado só na referência em Python | Transação com nonce por conta e assinatura presa à rede. Estado, cadeia, prova de trabalho Argon2id. UTRAX (o "UsefulPoW" dos §49 e §51, fase 10), fora do consenso. |
| **RED** — hipótese | Tudo o que é próprio deste documento: Identity, Packet, ATAP, Bluetooth, Wi-Fi, Direct, Payment Intent, vouchers offline, Resonance, mesh, canais, pagamentos de máquinas. |

### Como o documento se encaixa no que já existe

- A ordem obrigatória do §64 é a mesma que o projeto segue. Os itens 1
  (`auron-codec`) e 2 (`auron-crypto`) estão prontos.
- **Decisão de 11/09/2026:** a transação nasce com vários ativos, como pede o
  §19. O gabarito em Python muda primeiro, e só depois a transação vai para
  o Rust:
  - identificador de ativo de 32 bytes, com o AUR reservado como tudo zero;
  - saldo guardado por conta e por ativo;
  - taxa sempre em AUR;
  - nonce por conta;
  - o consenso aceita só AUR até existir uma regra de emissão de ativos.
- **O modelo continua sendo de contas**, porque o próprio documento descreve a
  carteira com `nonce`, `asset_balances` e `offline_budget`.
- **Regras dos §58 a §60 que já estão no código e testadas:**
  - serialização canônica, com prefixo de tamanho em todo campo variável;
  - separação de domínio nas assinaturas (a transação assina
    `"AURON-TX-v1" || magic da rede || corpo`);
  - versão explícita em cada objeto.
- **O `packet_id` do §7** precisa ser o hash da codificação canônica do
  `auron-codec`, com prefixo de tamanho, e nunca de campos simplesmente
  concatenados. Foi exatamente a falta desse prefixo que permitiu o ataque à
  rede Liquid em 06/09/2026 (ver `docs/AURON-FLUX.md`).
- **Nomes:** para não confundir com a "Fase 1" do núcleo e com as fases F0 a
  F10 do Flux, as fases do §51 são chamadas aqui de **D1 a D10**.

### Pontos de atenção

1. **QUIC contra a regra do projeto.** As bibliotecas de QUIC em Rust
   dependem de código em C (`ring` ou `aws-lc-rs`), e o projeto proíbe isso
   de propósito, para compilar em celular e em qualquer computador. O
   transporte pela internet fica em TCP com cifra Noise. O QUIC entra quando
   houver uma opção em Rust puro.
2. **Quem fica com o prejuízo num gasto duplo offline.** O §27 está certo:
   sem comunicação com o consenso, ninguém garante que o saldo foi gasto uma
   vez só. Detectar o conflito não basta: quando a mesma pessoa gasta em
   dobro, alguém fica sem receber. O mecanismo mais usado é uma **caução**:
   quem quer pagar offline trava antes um valor na blockchain, e esse valor
   compensa as vítimas se houver fraude. É regra de consenso e precisa ser
   decidida antes da fase D3.
3. **Destino de uso único no modelo de contas.** Receber cada pagamento num
   endereço novo espalha o saldo em muitas contas pequenas, e juntar tudo
   depois custa várias transações. A privacidade do §16 fica para uma camada
   por cima: endereços derivados e agregação.
4. **Bluetooth exige celular e aplicativo.** Pagamento por Bluetooth de verdade
   pede um app Android, que é um projeto à parte. A fase D3 testa toda a lógica
   do voucher sem Bluetooth, e isso roda na máquina de desenvolvimento.

### Relação com os outros documentos

- O **Flux** (`docs/AURON-FLUX.md`) é o item 15 da ordem do §64: vem por
  último.
- O **UTRAX** existe na referência em Python, fora do consenso (§18 da
  AURON-SPEC-01), e é o que os §49 e §51 chamam de UsefulPoW.

---

# Arquitetura original

**AURON — ARQUITETURA INTEGRADA DE COMUNICAÇÃO, PAGAMENTOS P2P E OFFLINE MESH**

- Versão: 1.0
- Status: Architecture / Engineering Specification
- Projeto: Auron
- Componentes principais: Auron Core, Auron Direct, Auron Resonance, Auron
  Transport
- Objetivo: criar uma infraestrutura descentralizada capaz de transportar
  comunicação e transferências de valor por múltiplos meios, incluindo
  operação sem Internet através de Bluetooth/Wi-Fi local e posterior
  sincronização com a blockchain.

## 1. VISÃO

O Auron não deve tratar P2P como simplesmente:

```text
Carteira A → Carteira B
```

O objetivo é transformar o P2P em um protocolo de interação direta entre
entidades, capaz de transportar:

- pagamentos;
- Payment Intents;
- recibos;
- contratos;
- mensagens;
- arquivos;
- estados de pagamento;
- vouchers offline;
- informações de sincronização;
- tarefas computacionais.

A arquitetura deve ser transport-agnostic.

A aplicação não deve depender de saber se os dados estão viajando por:

```text
Internet
Bluetooth
Wi-Fi Direct
Wi-Fi local
outro transporte futuro
```

O protocolo cria um pacote Auron e a camada de transporte escolhe o mecanismo
disponível.

## 2. PRINCÍPIO FUNDAMENTAL

```text
                 AURON
                   │
        ┌──────────┴──────────┐
        │                     │
     RESONANCE              DIRECT
   comunicação               valor
        │                     │
        └──────────┬──────────┘
                   │
              AURON CORE
                   │
        blockchain / consenso
```

**Auron Resonance.** Responsável pela comunicação descentralizada.

**Auron Direct.** Responsável por pagamentos, contratos de pagamento e
transferência de valor.

**Auron Core.** Responsável pelo consenso, blockchain, estado e regras
monetárias.

**Auron Transport.** Responsável por transportar dados entre dispositivos.

## 3. OBJETIVOS

### 3.1 Objetivos obrigatórios

1. Comunicação P2P.
2. Pagamentos P2P.
3. Bluetooth offline.
4. Wi-Fi local/Direct quando disponível.
5. Store-and-forward.
6. Criptografia ponta a ponta.
7. Identidade criptográfica.
8. Payment Intent.
9. Offline Voucher.
10. Limite de gasto offline.
11. Detecção de conflitos.
12. Sincronização posterior.
13. QR para bootstrap/pagamento.
14. NFC como transporte/opção futura.
15. Multi-asset desde a definição do protocolo.
16. Compatibilidade com mobile e desktop.
17. Nenhum servidor central obrigatório.

## 4. ARQUITETURA DE CAMADAS

```text
┌─────────────────────────────────────────┐
│           APPLICATION LAYER             │
│ Wallet / Chat / Merchant / AI / Apps    │
├─────────────────────────────────────────┤
│             AURON DIRECT                │
│ Payment / Contract / Voucher / Escrow   │
├─────────────────────────────────────────┤
│           AURON RESONANCE               │
│ Message / File / Group / Sync / Relay   │
├─────────────────────────────────────────┤
│             AURON PACKET                │
│ Unified protocol envelope               │
├─────────────────────────────────────────┤
│             AURON ATAP                  │
│ Transport abstraction                   │
├─────────────────────────────────────────┤
│ Bluetooth │ Wi-Fi │ TCP/QUIC │ Future   │
├─────────────────────────────────────────┤
│                DEVICE                   │
└─────────────────────────────────────────┘
```

## 5. AURON IDENTITY

A identidade deve ser separada da carteira financeira.

```text
AuronIdentity
├── identity_id
├── communication_public_key
├── device_keys[]
├── capability_descriptor
└── identity_metadata
```

A carteira:

```text
AuronWallet
├── wallet_id
├── account_keys
├── asset_balances
├── nonce
└── offline_budget
```

Uma pessoa pode ter múltiplas carteiras sem precisar criar uma nova
identidade de comunicação.

## 6. CHAVES

Hierarquia recomendada:

```text
Master Identity
      │
      ├── Communication Key
      │
      ├── Wallet Key
      │
      ├── Device Key
      │
      └── Offline Spending Key
```

A `Offline Spending Key` deve possuir permissões limitadas.

Nunca entregar a chave privada principal para Bluetooth.

## 7. AURON PACKET

Toda comunicação entre peers deve utilizar um envelope comum.

```text
AuronPacket
{
    version,
    packet_type,
    packet_id,
    sender_id,
    recipient_id,
    session_id,
    timestamp,
    expiration,
    priority,
    payload_hash,
    flags,
    signature,
    encrypted_payload
}
```

`packet_id` deve permitir deduplicação.

Recomendação:

```text
packet_id = SHA-512(canonical_packet_header || payload)
```

O formato exato deve ser definido pelo `auron-codec`.

## 8. AURON TRANSPORT — ATAP

Auron Transport Abstraction Protocol

A aplicação nunca chama Bluetooth diretamente.

```text
Application
    ↓
Auron Transport API
    ↓
ATAP
    ↓
Available Transport
```

Transportes:

```text
Bluetooth LE
Bluetooth Classic
Wi-Fi Direct
Wi-Fi LAN
QUIC/TCP
Future transports
```

Interface conceitual:

```rust
trait Transport {
    fn discover(&self) -> Result<Vec<Peer>>;
    fn connect(&self, peer: PeerId) -> Result<Session>;
    fn send(&self, packet: AuronPacket) -> Result<()>;
    fn receive(&self) -> Result<AuronPacket>;
    fn close(&self);
}
```

## 9. BLUETOOTH OFFLINE

Bluetooth é o transporte primário para o primeiro protótipo mobile.

Fluxo:

```text
Device A
   │
 BLE Discovery
   ↓
Device B
   │
Handshake
   ↓
Encrypted Session
   ↓
Auron Packet
```

O Bluetooth não é consenso.

Ele somente transporta mensagens/provas.

## 10. BLUETOOTH DISCOVERY

Beacon mínimo:

```text
AURON
protocol_version
capability_flags
ephemeral_public_key
```

Não transmitir saldo, chave privada ou informações financeiras desnecessárias
durante discovery.

## 11. HANDSHAKE

```text
A                         B
│                         │
│──── HELLO ─────────────►│
│◄─── HELLO ──────────────│
│                         │
│──── Challenge ─────────►│
│◄─── Signed Response ────│
│                         │
│──── Session Confirm ───►│
│◄─── Session Confirm ────│
│                         │
└──── ENCRYPTED ──────────┘
```

A sessão deve utilizar chaves efêmeras.

Objetivos:

- autenticação;
- forward secrecy;
- replay protection;
- associação de mensagens à sessão.

## 12. SESSION

```text
Session
├── session_id
├── local_ephemeral_key
├── remote_ephemeral_key
├── sequence_number
├── receive_window
├── expiration
└── capabilities
```

Cada mensagem recebe sequência:

```text
sequence = 0
sequence = 1
sequence = 2
...
```

Mensagens antigas/repetidas devem ser rejeitadas conforme as regras da sessão.

## 13. AURON DIRECT

Auron Direct é o protocolo de valor.

Ele não começa pela transação.

Começa por:

```text
PAYMENT INTENT
```

## 14. PAYMENT INTENT

Estrutura:

```text
PaymentIntent
{
    intent_id,
    sender,
    recipient,
    asset_id,
    amount,
    nonce,
    max_fee,
    expiration,
    purpose,
    policy_hash,
    session_id,
    signature
}
```

Estados:

```text
CREATED
   ↓
OFFERED
   ↓
ACCEPTED
   ↓
AUTHORIZED
   ↓
LOCKED
   ↓
TRANSFERRED
   ↓
SETTLED
   ↓
FINAL
```

Estados alternativos:

```text
REJECTED
EXPIRED
CANCELLED
CONFLICTED
DISPUTED
```

## 15. PAYMENT IDENTITY

Auron deve permitir uma identidade de pagamento legível.

Exemplo conceitual:

```text
bob.aur
```

Resolver:

```text
Payment Identity
       ↓
current receiving key
       ↓
one-time destination
```

A resolução deve ser descentralizada quando implementada.

## 16. ONE-TIME DESTINATION

Não reutilizar endereços de recebimento quando não for necessário.

Fluxo:

```text
Bob Identity
    ↓
derive receiving destination
    ↓
Alice
    ↓
payment
```

Próximo pagamento:

```text
Bob Identity
    ↓
different destination
```

Isso melhora privacidade e análise de histórico.

## 17. ATOMIC PAYMENT

Para operações que exigem condição:

```text
Payment
+
Condition
+
Proof
```

Fluxo:

```text
Alice
  │
  │ lock value
  ↓
Protocol
  │
  │ condition satisfied
  ↓
Bob
```

O sistema deve impedir que uma parte consiga finalizar sua obrigação enquanto
a outra permanece permanentemente sem proteção.

## 18. PAYMENT CHANNEL

Para múltiplos pagamentos entre duas entidades:

```text
Alice ═════════════ Bob
          Channel
```

Estado inicial:

```text
Alice: 100 AUR
Bob:     0 AUR
```

Atualizações off-chain:

```text
State 1:
Alice 90 / Bob 10

State 2:
Alice 80 / Bob 20

State 3:
Alice 75 / Bob 25
```

Somente o estado final precisa ser liquidado on-chain.

O mecanismo deve possuir:

- sequence;
- revocation/invalid-state protection;
- timeout;
- settlement;
- dispute procedure.

## 19. MULTI-ASSET

O protocolo de transação deve nascer preparado para:

```text
asset_id
amount
```

em vez de assumir implicitamente apenas AUR.

Exemplo:

```text
asset_id = AUR
amount = 10.00000000
```

Futuro:

```text
asset_id = AUR-BRL
amount = ...
```

O suporte real a assets adicionais depende das regras do Auron Core.

Não implementar Flux diretamente em uma transação de asset único.

## 20. OFFLINE MODE

O modo offline utiliza:

```text
Auron Direct
      ↓
Offline Controller
      ↓
Offline Voucher
      ↓
Bluetooth
```

Não existe alegação de finalidade global instantânea offline.

Existe: autorização criptográfica + orçamento offline + posterior settlement.

## 21. OFFLINE BUDGET

Carteira:

```text
Total Balance:
100 AUR

Offline Budget:
20 AUR
```

Políticas:

```text
offline_total_limit
offline_single_tx_limit
offline_expiration
offline_max_depth
```

Exemplo:

```text
offline_total_limit = 20 AUR
offline_single_tx = 5 AUR
offline_expiration = 24h
```

## 22. OFFLINE VOUCHER

```text
OfflineVoucher
{
    version,
    voucher_id,
    parent_state_hash,
    asset_id,
    amount,
    issuer,
    recipient,
    offline_counter,
    expiration,
    policy_hash,
    session_id,
    signature
}
```

O voucher é um objeto criptográfico verificável.

Ele não deve ser interpretado como uma nova moeda independente.

É uma autorização de transferência/claim que precisa ser reconciliada com o
estado on-chain.

## 23. OFFLINE TRANSFER

```text
Alice
  │
  │ Payment Intent
  ↓
Offline Controller
  │
  │ create voucher
  ↓
Bluetooth
  │
  ↓
Bob
  │
  │ validate
  ↓
Accepted Offline
```

Bob armazena:

```text
voucher
+
receipt
+
sender proof
```

## 24. OFFLINE RECEIPT

Bob assina:

```text
OfflineReceipt
{
    voucher_id,
    received_amount,
    recipient,
    timestamp,
    signature
}
```

Alice recebe:

```text
✓ Recipient acknowledged
```

## 25. OFFLINE COUNTER

Cada carteira possui:

```text
offline_counter
```

Exemplo:

```text
18492
```

Pagamento:

```text
18493
```

Próximo:

```text
18494
```

O estado deve ser monotônico dentro do domínio definido pelo protocolo.

## 26. OFFLINE CONFLICT

Se a mesma origem tentar criar estados conflitantes:

```text
State X
 ├── Voucher A
 └── Voucher B
```

a rede registra:

```text
CONFLICT
```

Quando reconectada:

```text
Offline State
      ↓
Conflict Detector
      ↓
Core Validation
      ↓
Settlement / Rejection
```

O tratamento exato deve ser especificado pelo consenso.

## 27. LIMITAÇÃO FUNDAMENTAL DO OFFLINE

Sem comunicação com o consenso global, nenhum protocolo puramente de software
consegue garantir que um saldo global foi gasto uma única vez em todos os
lugares simultaneamente.

Portanto, o Auron offline deve depender de:

```text
limited offline budget
+
cryptographic state
+
expiration
+
hardware-backed protection quando disponível
+
reconciliation
+
conflict detection
```

Isso é requisito de segurança, não detalhe opcional.

## 28. OFFLINE VOUCHER CHAIN

Opcional/fase experimental.

```text
Voucher #1
   ↓
Voucher #2
   ↓
Voucher #3
```

Cada voucher contém:

```text
parent_state_hash
```

Isso cria uma cadeia de dependência.

Limitar:

```text
max_offline_depth
```

para impedir crescimento ilimitado.

## 29. BLUETOOTH RELAY

Um dispositivo pode transportar um pacote recebido.

```text
A
 ↓ Bluetooth
B
 ↓ Bluetooth
C
 ↓ Internet
Network
```

B e C não precisam descriptografar o conteúdo.

Eles transportam:

```text
packet_id
routing metadata
encrypted payload
expiration
```

## 30. STORE AND FORWARD

Cada peer possui uma fila:

```text
OutboundStore
├── P0 emergency
├── P1 priority
├── P2 normal
├── P3 bulk
└── P4 background
```

Quando aparece conectividade:

```text
Queue
 ↓
Peer discovery
 ↓
Forward
 ↓
ACK
 ↓
Delete/retain according to policy
```

## 31. MESSAGE PRIORITY

```text
P0 = critical
P1 = high
P2 = normal
P3 = bulk
P4 = background
```

Mensagens expiradas são descartadas.

## 32. AURON RESONANCE

Sistema de comunicação:

```text
Auron Resonance
├── messaging
├── group messaging
├── file transfer
├── synchronization
├── relay
└── voice/data future
```

O primeiro MVP deve começar por:

```text
text message
+
signed packet
+
Bluetooth
+
store-and-forward
```

## 33. END-TO-END ENCRYPTION

Mensagem:

```text
plaintext
   ↓
encryption
   ↓
ciphertext
   ↓
Auron Packet
   ↓
relay
```

Relay:

```text
✓ pode transportar
✗ não deve conseguir ler
```

O protocolo deve prever rotação de chaves e recuperação segura de sessão.

## 34. GROUP COMMUNICATION

Grupo:

```text
A ─── B
│ ╲   │
│  ╲  │
C ─── D
```

Não depender de servidor único.

Cada grupo possui:

```text
group_id
group_state
membership
key_epoch
message_sequence
```

Mudança de membro deve provocar rotação apropriada de chaves.

## 35. FILE TRANSFER

Arquivos não devem ser enviados como um único pacote.

```text
File
 ↓
Hash
 ↓
Chunking
 ↓
Chunks
 ↓
Auron Packets
```

Exemplo:

```text
FILE
├── chunk 0
├── chunk 1
├── chunk 2
├── ...
└── chunk N
```

Cada chunk:

```text
chunk_id
file_id
index
size
hash
payload
```

## 36. FUTURO: ERASURE CODING

Para redes instáveis:

```text
Original
 ↓
Erasure Coding
 ↓
N fragments
 ↓
M fragments sufficient
```

Isso permite recuperar o arquivo mesmo com perda de alguns fragments.

Deve ser fase posterior.

## 37. AURON ADAPTIVE MESH ROUTING

Cada peer mantém métricas:

```text
latency
reliability
bandwidth
availability
packet_loss
battery_class
connection_history
```

Não existe "melhor peer" absoluto.

A escolha depende do tipo de pacote.

## 38. ROUTING SCORE

Conceitualmente:

```text
score =
    reliability_weight
  + connectivity_weight
  + latency_weight
  + diversity_weight
  + delivery_probability
```

Os pesos devem ser ajustáveis por versão do protocolo.

Nunca permitir que reputação isoladamente determine autoridade.

## 39. PEER DIVERSITY

O nó deve manter diversidade de conexões.

Evitar que todos os peers pertençam ao mesmo:

```text
network cluster
software implementation
routing domain
```

quando essas informações forem disponíveis de forma apropriada.

Objetivo:

```text
N1
├── Peer A
├── Peer B
├── Peer C
└── Peer D
```

não serem simplesmente quatro identidades controladas pelo mesmo atacante.

## 40. ANTI-SYBIL

Combinar:

```text
cryptographic identity
+
protocol cost
+
Proof/Work where appropriate
+
behavior
+
reputation
+
peer diversity
```

Nenhum componente sozinho deve representar "verdade".

## 41. REPUTATION

Reputação multidimensional:

```text
PeerReputation
├── availability
├── delivery
├── protocol_behavior
├── verification
└── work_reliability
```

Com decay temporal.

Não usar reputação como substituto do consenso.

## 42. QUARANTINE

Estados:

```text
NORMAL
   ↓
SUSPICIOUS
   ↓
QUARANTINE
   ↓
REVALIDATION
   ↓
NORMAL / PENALIZED
```

Motivos:

```text
invalid packet
replay
spam
invalid proof
protocol abuse
malformed data
```

## 43. QR

QR pode transportar:

```text
PaymentIntent
Peer bootstrap
Identity reference
Connection information
Invoice
```

Não colocar segredo privado no QR.

## 44. NFC

NFC será um transporte de proximidade.

Fluxo:

```text
Phone A
   ║ NFC
Phone B
```

Pode iniciar:

```text
session
payment intent
identity exchange
```

O restante pode migrar para Bluetooth.

## 45. BLUETOOTH + NFC

Fluxo ideal:

```text
NFC tap
   ↓
exchange bootstrap
   ↓
Bluetooth session
   ↓
encrypted communication
```

NFC serve para aproximação/autenticação inicial.

Bluetooth transporta o restante.

## 46. AI / MACHINE PAYMENTS

A arquitetura deve permitir que um agente ou máquina tenha identidade e
política próprias.

```text
AI Agent
   ↓
Payment Intent
   ↓
Policy Engine
   ↓
Auron Direct
   ↓
Settlement
```

Exemplo:

```text
maximum_per_transaction = 1 AUR
maximum_daily = 10 AUR
approved_services = [...]
```

## 47. POLICY ENGINE

Toda autorização automática deve passar por política.

```text
Policy
├── asset
├── recipient
├── amount_limit
├── daily_limit
├── expiration
├── purpose
└── required_confirmation
```

Exemplo:

```text
if amount <= 1 AUR
and recipient in approved_list
then automatic_authorization
```

## 48. PAYMENT NEGOTIATION

Futuro:

```text
REQUEST
   ↓
QUOTE
   ↓
NEGOTIATE
   ↓
ACCEPT
   ↓
LOCK
   ↓
SERVICE
   ↓
SETTLE
```

Isso permite mercados P2P e máquinas negociando serviços.

## 49. P2P RESOURCE MARKET

A mesma arquitetura pode anunciar:

```text
CPU
GPU
storage
bandwidth
compute
```

Fluxo:

```text
Requester
   ↓
Task Request
   ↓
Peer Discovery
   ↓
Quote
   ↓
Payment Intent
   ↓
Work
   ↓
Verification
   ↓
Settlement
```

Integração futura com UsefulPoW.

## 50. DIRETÓRIO DE CÓDIGO

Estrutura inicial recomendada:

```text
auron/
│
├── crates/
│
│   ├── auron-core/
│   │   ├── blockchain/
│   │   ├── consensus/
│   │   ├── state/
│   │   └── assets/
│   │
│   ├── auron-codec/
│   │
│   ├── auron-crypto/
│   │
│   ├── auron-direct/
│   │   ├── payment_intent/
│   │   ├── payment_state/
│   │   ├── settlement/
│   │   ├── channels/
│   │   ├── atomic/
│   │   └── policy/
│   │
│   ├── auron-offline/
│   │   ├── voucher/
│   │   ├── budget/
│   │   ├── counter/
│   │   ├── conflict/
│   │   └── redemption/
│   │
│   ├── auron-resonance/
│   │   ├── message/
│   │   ├── group/
│   │   ├── file/
│   │   ├── relay/
│   │   └── sync/
│   │
│   ├── auron-transport/
│   │   ├── bluetooth/
│   │   ├── wifi/
│   │   ├── quic/
│   │   └── tcp/
│   │
│   ├── auron-identity/
│   │
│   └── auron-wallet/
│
├── apps/
│   ├── auron-node/
│   ├── auron-wallet/
│   └── auron-mobile/
│
└── tests/
    ├── vectors/
    ├── codec/
    ├── payment/
    ├── offline/
    ├── bluetooth/
    ├── mesh/
    └── adversarial/
```

## 51. IMPLEMENTAÇÃO EM FASES

**FASE 1 — CORE.** Implementar:

```text
auron-codec
auron-crypto
transaction
multi-asset foundation
```

Testes:

```text
byte-for-byte vectors
serialization
signatures
hashes
nonce
replay
```

**FASE 2 — AURON DIRECT.** Implementar:

```text
PaymentIntent
PaymentState
Receipt
Policy
Settlement
```

Testar:

```text
Alice → Bob
Alice → múltiplos outputs
expiration
replay
invalid signature
invalid nonce
```

**FASE 3 — OFFLINE.** Implementar:

```text
OfflineBudget
OfflineCounter
OfflineVoucher
OfflineReceipt
ConflictDetector
Redemption
```

Primeiro sem Bluetooth:

```text
Wallet A
 ↓
Voucher
 ↓
Wallet B
```

Isso permite testar toda a lógica antes de lidar com hardware.

**FASE 4 — BLUETOOTH.** Implementar:

```text
BLE discovery
handshake
session
encrypted packets
ACK
retry
timeout
```

Teste:

```text
A ↔ B
```

**FASE 5 — OFFLINE PAYMENT REAL.** Integrar:

```text
Bluetooth
+
OfflineVoucher
+
Receipt
+
Core reconciliation
```

Teste:

```text
A → B
offline

B → network
online

voucher → settlement
```

**FASE 6 — RESONANCE.** Implementar:

```text
text messages
store-and-forward
relay
priority
expiration
deduplication
```

Teste:

```text
A → B → C → D
```

**FASE 7 — MESH.** Adicionar:

```text
adaptive routing
peer diversity
reputation
quarantine
multi-hop
```

Criar simulador com:

```text
100 peers
1.000 peers
10.000 virtual peers
```

**FASE 8 — FILES.** Adicionar:

```text
chunking
hash
resume
erasure coding
```

**FASE 9 — CHANNELS / ATOMIC.** Adicionar:

```text
payment channels
atomic exchange
escrow
conditional payments
```

**FASE 10 — AI / RESOURCE ECONOMY.** Adicionar:

```text
machine identities
payment policies
service negotiation
compute marketplace
UsefulPoW integration
```

## 52. TESTES ADVERSARIAIS

Obrigatório testar:

```text
replay attack
double-spend attempt
fake voucher
modified voucher
expired voucher
wrong recipient
wrong asset
wrong amount
nonce collision
packet duplication
packet reordering
malicious relay
malicious peer
Sybil peers
Eclipse-like topology
Bluetooth interruption
battery loss
device crash
network partition
```

## 53. TESTE DE PARTIÇÃO

Simular:

```text
NETWORK A          NETWORK B

A ─ B ─ C          D ─ E ─ F
```

Sem comunicação.

Depois:

```text
C ═══════════ D
```

A rede deve sincronizar os estados válidos.

## 54. TESTE DE OFFLINE DOUBLE-SPEND

Simular:

```text
Alice
 │
 ├── Voucher 1 → Bob
 │
 └── Voucher 2 → Carol
```

Depois reconectar:

```text
Alice
 ↓
Core
 ↓
Conflict Detector
```

O teste deve provar que o sistema detecta o conflito, e que as regras de
liquidação não permitem simplesmente aceitar estados incompatíveis como se
ambos fossem definitivamente válidos.

## 55. PRINCÍPIO DE SEGURANÇA

Nunca confiar em:

```text
Bluetooth
IP
peer
reputation
device
relay
offline voucher
```

como autoridade individual.

A autoridade monetária final continua sendo:

```text
AURON CORE CONSENSUS
```

## 56. PRINCÍPIO DE PRIVACIDADE

Separar:

```text
communication identity
payment identity
device identity
network address
```

sempre que possível.

Não registrar na blockchain dados que não precisam ser on-chain.

## 57. PRINCÍPIO DE RESILIÊNCIA

O sistema deve degradar progressivamente:

```text
Internet
   ↓
Wi-Fi
   ↓
Bluetooth
   ↓
Store-and-forward
   ↓
Local operation
   ↓
Reconnection
   ↓
Synchronization
```

A perda de um transporte não deve destruir o estado local.

## 58. PRINCÍPIO DE INTEROPERABILIDADE

Todos os componentes devem utilizar:

```text
canonical serialization
versioned protocol
explicit message types
cryptographic domain separation
```

Nenhum módulo deve depender de estruturas internas de outro módulo.

## 59. DOMÍNIOS CRIPTOGRÁFICOS

Assinaturas devem separar contextos.

Exemplo conceitual:

```text
AURON_PAYMENT
AURON_VOUCHER
AURON_MESSAGE
AURON_RECEIPT
AURON_HANDSHAKE
AURON_STATE
```

Nunca assinar simplesmente:

```text
hash(payload)
```

sem indicar o domínio.

## 60. VERSIONAMENTO

Todo objeto persistente:

```text
version
```

Todo protocolo:

```text
protocol_version
```

Todo voucher:

```text
voucher_version
```

Mudanças incompatíveis devem produzir nova versão explícita.

## 61. MVP REALISTA

A primeira versão codável não deve tentar fazer tudo.

**MVP-1**

```text
Auron Packet
Identity
Crypto
BLE
Handshake
Encrypted Message
```

**MVP-2**

```text
PaymentIntent
OfflineVoucher
OfflineBudget
Receipt
```

**MVP-3**

```text
Bluetooth Offline Payment
Reconnection
Settlement
```

**MVP-4**

```text
Store-and-forward
Relay
Mesh
```

**MVP-5**

```text
Payment Channels
Atomic
Resource Marketplace
AI payments
```

## 62. CRITÉRIO DE SUCESSO

O projeto não deve ser considerado pronto porque:

```text
"conseguiu mandar 10 AUR"
```

O teste de sucesso é:

```text
DEVICE A
   │
   │ sem Internet
   │
 Bluetooth
   ↓
DEVICE B
   │
   │ aceita voucher
   ↓
DEVICE B
   │
   │ dias/horas depois
   ↓
Internet
   │
   ↓
Auron Core
   │
   ↓
Validation
   │
   ↓
Settlement
```

E simultaneamente:

```text
Auron Resonance

DEVICE A
   ↓ Bluetooth
DEVICE B
   ↓ Bluetooth
DEVICE C
   ↓ Internet
NETWORK
   ↓
DEVICE D
```

A mensagem deve sobreviver à interrupção de infraestrutura e chegar ao
destinatário quando existir caminho disponível.

## 63. VISÃO FINAL

A arquitetura completa:

```text
                           AURON
                             │
       ┌─────────────────────┼─────────────────────┐
       │                     │                     │
       ↓                     ↓                     ↓
 AURON RESONANCE       AURON DIRECT          AURON CORE
 comunicação              valor              blockchain
       │                     │                     │
       │                     │                     │
       └──────────────┬──────┴─────────────────────┘
                      ↓
                 AURON PACKET
                      ↓
                  ATAP LAYER
                      ↓
       ┌──────────────┼──────────────┐
       ↓              ↓              ↓
   Bluetooth       Wi-Fi          Internet
       │              │              │
       └──────────────┼──────────────┘
                      ↓
                 AURON MESH
                      ↓
             STORE / CARRY / FORWARD
```

O conceito fundamental é:

O Auron não deve depender de um único caminho para transportar valor ou
informação.

Online:

```text
Internet → P2P → Blockchain
```

Offline:

```text
Bluetooth → Voucher → Reconnection → Blockchain
```

Comunicação:

```text
Bluetooth → Relay → Mesh → Internet → Destination
```

Futuro:

```text
Payment
+
Communication
+
Compute
+
Storage
+
Machine-to-Machine Economy
```

## 64. ORDEM OBRIGATÓRIA PARA OS AGENTES

Não implementar Flux antes do core.

Ordem:

```text
1. auron-codec
2. auron-crypto
3. transaction
4. multi-asset foundation
5. Auron Direct
6. Offline Voucher
7. Bluetooth Transport
8. Offline Settlement
9. Auron Resonance
10. Store-and-forward
11. Mesh
12. Channels / Atomic
13. Storage
14. AI / Resource Economy
15. Flux integration
```

Cada etapa deve possuir:

```text
implementation
unit tests
integration tests
adversarial tests
serialization vectors
documentation
```

Regra: não considerar uma funcionalidade "segura" apenas porque o fluxo feliz
funciona. Toda funcionalidade monetária offline precisa ser testada contra
replay, duplicação, conflito, adulteração, interrupção e tentativa de
double-spend.
