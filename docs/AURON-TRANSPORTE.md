# Auron Transporte — o meio é peça trocável

> Estado em 18/09/2026: **implementado e testado** o núcleo (`crates/auron-transporte`),
> com dois meios reais: pasta de arquivos e memória. Bluetooth, LoRa e rádio
> estão **projetados, não implementados**. Nada disto entra no consenso.

## O problema

Hoje um nó Auron só conversa por TCP. Isso amarra a rede à internet: se o
roteador cai, se não há sinal, se os dois lados estão em casas diferentes atrás
de NAT, a rede para. Mas o protocolo do Auron nunca precisou de TCP — ele
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
5 MB levavam **337 segundos**. Entrou `merkle_raiz_e_caminhos` no `auron-codec`,
que calcula raiz e todos os caminhos numa passada: os mesmos testes levam
**2,8 segundos**. Há teste travando que a função nova dá exatamente o mesmo
resultado da antiga, folha a folha, de 0 a 33 folhas.

## O que já dá para fazer hoje

- Cortar qualquer objeto até 1 GiB em fragmentos verificáveis.
- Espalhar por vários meios ao mesmo tempo, em rodízio.
- Montar do outro lado, com pedaços fora de ordem, repetidos e adulterados no meio.
- Transportar por **pasta**: um programa grava e vai embora; outro lê depois.

## O que falta para o teste da foto entre duas casas

1. **Adaptador TCP** deste transporte (o `auron-net` já tem a conexão cifrada;
   falta plugar os dois).
2. **Mensagem nova no `auron-wire`** para fragmentos viajarem entre nós — fora
   do consenso, como manda o §18 do documento mestre: a blockchain **não** é
   banco de arquivos.
3. **Encontro entre casas**: nó semente público e furo de NAT.
4. **App Android**, para Bluetooth de verdade (o Termux não o expõe).

## O que isto não é

- Não é armazenamento: o ar transporta, não guarda (§19 do documento mestre).
- Não é consenso: nenhum bloco depende desta camada.
- Não é anonimato: repartir por meios dificulta escuta passiva de um meio só,
  mas não é uma promessa de privacidade.
