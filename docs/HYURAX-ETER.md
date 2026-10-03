# Éter — o meio é peça trocável

> **Éter** é o nome da tecnologia de comunicação entre nós do Hyurax: a camada
> que trata Wi-Fi, Bluetooth, rádio, pendrive e satélite como maneiras
> diferentes de atravessar a mesma coisa. Na especificação, o formato se chama
> `HYURAX-ETER-v1`; o crate é `hyurax-eter`.
>
> *"O nó não fala Wi-Fi nem Bluetooth. O nó fala Hyurax — e o Éter escolhe o caminho."*

> Estado em 19/09/2026: **implementado e testado** o núcleo (`crates/hyurax-eter`)
> e os meios **Wi-Fi** (UDP na rede local), **som** (modem FSK para FM, cabo e
> ar) e **fio serial** (Bluetooth SPP e rádios USB), além de pasta e memória. Um
> objeto se divide entre eles pela velocidade de cada um. Falta o teste em
> aparelho real: este PC não tem microfone, e o Termux não abre Bluetooth.
> LoRa continua **projetado**. Nada disto entra no consenso.

## Os três meios juntos (19/09/2026)

```text
eter enviar  foto.jpg  --wifi --som saida --bluetooth COM5
eter receber recebidos --wifi --som gravacoes --bluetooth COM5
```

| Meio | Módulo | Como atravessa | Vazão | Estado |
|---|---|---|---|---|
| Wi-Fi | `wifi.rs` | UDP em difusão na rede local; serve o roteador do celular, sem internet | centenas de KB/s | testado no PC |
| Som / FM | `som.rs` | Bytes viram tons (FSK, Bell 202). Alto-falante → microfone, ou transmissor FM → rádio | 109 B/s (perfil `fm`), 27 B/s (perfil `ar`) | testado em simulação de canal real |
| Bluetooth | `serial.rs` | Porta serial do Bluetooth (SPP): `COM5`, `/dev/rfcomm0`. Também rádios LoRa por USB | ~20 KB/s | testado com fio simulado |

**Como um objeto se divide.** Cada pedaço vai pelo meio que terminaria de
entregá-lo primeiro, contando a fila que já está nele (`Meio::vazao`). Um meio
dez vezes mais rápido leva dez vezes mais; o lento nunca vira gargalo. O aviso
(manifesto) vai por todos. Se um meio cai, o receptor sabe exatamente o que
falta (`Faltando`) e o reenvio vai pelos que sobraram.

**A moldura dos fios** (`enquadramento.rs`). Som e serial são rios de bytes com
ruído, sem começo nem fim de pacote. Cada quadro vai em `E7 3C 5A C3 ·
tamanho(2) · quadro · CRC-32`. O CRC só separa o que o fio estragou; quem
garante o conteúdo continua sendo a prova de Merkle.

**O modem de som** resiste, nos testes, a gravação em outra taxa (48 kHz
tocado, 44,1 kHz gravado), relógio 300 ppm fora, volume a 20% e ruído de um
terço do sinal. Cada byte ressincroniza o relógio (moldura de UART).

**O que não é possível sem hardware ou app próprio, dito com clareza:**
- Wi-Fi de placa para placa sem roteador (Wi-Fi Direct) não é exposto a
  programa comum no Windows nem no Termux. O roteador do celular cumpre o papel.
- Bluetooth no celular exige um aplicativo Android; o Termux não tem acesso.
- Transmitir em FM exige um transmissor. No Brasil, só vale o de baixíssima
  potência homologado pela Anatel (radiação restrita, como os de carro); fora
  disso é transmissão sem licença, que é crime. Receber exige um rádio FM, que
  muitos celulares têm. O modem já é o mesmo. Pelo ar e por cabo não há
  licença nenhuma envolvida.

Tocar e gravar no Windows, sem instalar nada: `scripts/eter-tocar.ps1` e
`scripts/eter-gravar.ps1` (este também faz o teste de eco com `-Tocar`).

## O problema

Hoje um nó Hyurax só conversa por TCP. Isso amarra a rede à internet: se o
roteador cai, se não há sinal, se os dois lados estão em casas diferentes atrás
de NAT, a rede para. Mas o protocolo do Hyurax nunca precisou de TCP — ele
precisa que **bytes cheguem**, de qualquer jeito, em qualquer ordem, a qualquer
hora.

## A ideia

O nó não fala "Wi-Fi" nem "Bluetooth". Ele entrega bytes a um `Meio`. O mesmo
objeto pode sair **pedaço a pedaço por meios diferentes, ao mesmo tempo**.

```text
             ┌──────────── objeto (bloco, transação, arquivo) ───────────┐
             │                                                            │
        Manifesto                    Fragmentos                     Faltando
   (quem é, quantos pedaços,   (pedaço + prova de Merkle)     (o que ainda não veio)
        raiz de Merkle)
             │                            │                              │
     ┌───────┴────────┐          ┌────────┴────────┐            ┌────────┴───────┐
     ▼                ▼          ▼                 ▼            ▼                ▼
   rádio           Wi-Fi       Wi-Fi          Bluetooth       qualquer um dos meios
 (cabe o aviso)   (cabe tudo)  (metade)        (metade)
```

## As três garantias

1. **Cada fragmento se prova sozinho.** Ele carrega o caminho de Merkle até a
   raiz que veio no manifesto. Um pedaço que chegou pelo Bluetooth é conferido
   sem depender dos que vieram pelo Wi-Fi, e lixo morre na chegada.
2. **A identidade é o conteúdo.** O `id` do objeto é o SHA-512 dele inteiro. A
   montagem só termina se o remontado tiver aquele hash: mentir na raiz ou no
   total não adianta, porque a conta final não fecha.
3. **A ordem e a hora não importam.** Fragmentos chegam fora de ordem,
   repetidos, por caminhos diferentes e com horas de diferença.

## Cada meio faz o que aguenta

O tamanho do pedaço sai do **maior** meio disponível, para não desperdiçar
banda de quem tem banda. Depois:

| Meio | Quadro | Papel |
|---|---|---|
| Rádio (LoRa), projetado | ~220 B | Leva o **aviso**: "existe o objeto X, com N pedaços, raiz R". Não leva dado |
| Bluetooth, projetado | ~512 B a 20 KB | Descoberta, controle e pedaços de objetos pequenos |
| Wi-Fi / TCP | 16 KB+ | Carrega o peso |
| Pasta de arquivos | 16 KB+ | Pendrive, cartão, pasta sincronizada: atravessa **o tempo** |

Um meio pequeno demais para o dado **não é erro**: ele fica com o aviso, e quem
ouviu só o rádio já sabe o que existe e o que falta. Erro é quando *nenhum*
meio comporta um fragmento.

## O limite honesto da prova por fragmento

O caminho de Merkle cresce com o número de pedaços, e o número de pedaços
cresce quando o pedaço encolhe. Em meio minúsculo isso se morde: um quadro de
400 bytes não carrega pedaço útil de um arquivo de 5 MB — a prova comeria o
quadro inteiro. A função `pedaco_para_o_meio` calcula isso e **recusa com
mensagem clara** em vez de inventar.

Consequência de projeto, não de implementação: **rádio é para aviso; dado pesado
pede meio largo.** Se um dia for preciso mandar arquivo grande por meio
minúsculo, o caminho é verificação em duas camadas (peças verificáveis,
subdivididas em quadros sem prova individual), como o BitTorrent v2 faz. Não
está implementado.

## Desempenho

Tirar a prova de cada pedaço com `merkle_path` refazia a árvore a cada chamada:
5 MB levavam **337 segundos**. Entrou `merkle_raiz_e_caminhos` no `hyurax-codec`,
que calcula raiz e todos os caminhos numa passada: os mesmos testes levam
**2,8 segundos**. Há teste travando que a função nova dá exatamente o mesmo
resultado da antiga, folha a folha, de 0 a 33 folhas.

## O que já dá para fazer hoje

- Cortar qualquer objeto até 1 GiB em fragmentos verificáveis.
- Espalhar por vários meios ao mesmo tempo, em rodízio.
- Montar do outro lado, com pedaços fora de ordem, repetidos e adulterados no meio.
- Transportar por **pasta**: um programa grava e vai embora; outro lê depois.

## O que falta para o teste da foto entre duas casas

1. **Adaptador TCP** deste transporte (o `hyurax-net` já tem a conexão cifrada;
   falta plugar os dois).
2. **Mensagem nova no `hyurax-wire`** para fragmentos viajarem entre nós — fora
   do consenso, como manda o §18 do documento mestre: a blockchain **não** é
   banco de arquivos.
3. **Encontro entre casas**: feito sem semente fixa pela malha
   ([HYURAX-MALHA.md](HYURAX-MALHA.md)): porta aberta sozinha por UPnP,
   pontes entre nós sem porta aberta, vizinhos na rede local. Blocos e
   transações já atravessam o Éter como **pacote** (`hyurax-no pacote`,
   pasta `eter/entrada`). Furo de NAT direto: PENDENTE.
4. **App Android**, para Bluetooth de verdade (o Termux não o expõe).

## O que isto não é

- Não é armazenamento: o ar transporta, não guarda (§19 do documento mestre).
- Não é consenso: nenhum bloco depende desta camada.
- Não é anonimato: repartir por meios dificulta escuta passiva de um meio só,
  mas não é uma promessa de privacidade.
