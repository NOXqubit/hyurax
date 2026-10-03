# Malha Hyurax — os nós são a rede

Pedido do autor em 03/10/2026: *"os nós são a própria rede"*. Nenhum
servidor central para os nós se acharem, e, mais adiante, internet passando
pelos próprios nós, sem depender de operadora.

Este documento separa o que dá para construir agora do que depende de
física, de equipamento e de lei. Ele se apoia no [Éter](HYURAX-ETER.md) (o
meio de transporte como peça trocável) e no modo offline do
[Direct/Resonance](HYURAX-DIRECT-RESONANCE.md).

---

## Parte 1 — Os nós se encontram sozinhos

### O problema, sem enfeite

Dois computadores atrás de CGNAT (a maioria das casas no Brasil) conseguem
*chamar* outros, mas não conseguem *ser chamados*. Se **todos** os nós
estiverem assim, nenhum fala com nenhum pela internet: não existe software
que resolva isso sozinho. O que a tecnologia pode fazer é:

1. **Usar qualquer nó alcançável como ponte**, sem escolher nenhum fixo e sem
   dar poder a ele.
2. **Achar caminhos que não passam pela operadora**: rede local, Wi-Fi
   direto, Bluetooth, rádio (o Éter).
3. **Nunca depender de uma fonte só** para conhecer os outros nós.

### O que já existe

| Peça | Estado |
|---|---|
| Livro de endereços: cada nó guarda os pares que já viu e os reusa ao abrir | REAL |
| Troca de endereços entre pares (`GETADDRS`/`ADDRS`) | REAL |
| Lista de sementes publicada (arquivo no GitHub, embutida e local), sem nenhuma ser obrigatória | REAL |
| Diversidade de pares: teto por IP e por faixa /24 (contra cercar um nó com identidades falsas) | REAL |
| IPv6: endereços e conexões | REAL (muitas operadoras dão IPv6 público; com ele, o nó é alcançável sem abrir porta) |
| Éter: Wi-Fi local, som e Bluetooth serial | núcleo testado; falta ligar ao nó |

### O que construir, em ordem

| Etapa | O que | Por que | Como provar |
|---|---|---|---|
| **M1. Descoberta na rede local** | O nó anuncia a sua porta por difusão UDP no Wi-Fi/cabo de casa e ouve os anúncios dos outros | Numa lan house, escola ou casa, os nós se acham **sem internet e sem semente** | dois PCs no mesmo Wi-Fi, sem lista de sementes, conectam |
| **M2. Porta aberta sozinha** | O nó pede ao roteador para abrir a porta (UPnP/NAT-PMP/PCP) e usa IPv6 quando houver; se ficou alcançável, se oferece aos outros como ponto de entrada | Cada usuário com internet "boa" vira semente sem saber, e a rede ganha muitas portas de entrada em vez de uma | o nó diz "alcançável de fora: sim/não", e outro nó de fora conecta |
| **M3. Ponte (relay) por qualquer nó** | Um nó alcançável aceita repassar, com limite, a conexão entre dois nós que não se alcançam. O conteúdo continua cifrado de ponta a ponta (Noise): a ponte não lê nem altera | Dois nós atrás de CGNAT falam através de qualquer terceiro, que não precisa ser confiável | dois nós atrás de NAT trocam blocos via um terceiro; adulterar na ponte derruba a conexão |
| **M4. Furo de NAT** | Com a ajuda de um terceiro, os dois nós abrem caminho direto entre si (*hole punching*) e dispensam a ponte | Menos carga nas pontes, mais velocidade | o tráfego deixa de passar pelo terceiro depois do encontro |
| **M5. Éter ligado ao nó** | Blocos e transações atravessam Bluetooth, Wi-Fi direto, rádio e pendrive | Rede que continua de pé sem operadora entre vizinhos | uma transação sai de um celular sem internet e chega à cadeia por outro nó |

**Limite honesto.** A M4 (furo de NAT) funciona bem sobre UDP e mal sobre
TCP. A rede hoje é TCP; a M4 pede um transporte UDP cifrado, que é trabalho
grande, e não funciona com todo tipo de NAT (o "simétrico" fura pouco).
Por isso a M3 (ponte) vem antes: ela funciona sempre.

**Segurança.** Mais portas de entrada deixam a rede *mais* difícil de
derrubar do que uma semente fixa: não há um alvo só. As defesas que já
existem continuam valendo (cifra com identidade, banimento, tetos por IP e
por faixa). A ponte ganha os seus próprios tetos, para não virar amplificador
de ataque.

---

## Parte 2 — Internet pelos nós

Aqui há duas ideias diferentes, que é importante não misturar.

### 2a. A rede carregar os próprios dados — viável

Os nós já carregam a cadeia, as transações e os JOBs. Dá para carregar
também mensagens, arquivos e páginas próprias da rede, endereçados pelo
conteúdo (o endereço é o hash: quem recebe confere que é o arquivo certo).
Entre vizinhos ligados por Wi-Fi ou rádio, isso funciona **sem operadora
nenhuma**. É a "internet própria" da rede: o que os usuários publicam nela
circula por ela.

Regra que continua valendo: a blockchain não é banco de arquivos. Os dados
viajam e ficam guardados fora do consenso (ver "O que isto não é" no [Éter](HYURAX-ETER.md)).

### 2b. Dar acesso à internet do mundo (Google, YouTube) — tem limite físico

Os nós não "contêm a internet". A internet do mundo está nos servidores do
mundo. Para chegar a eles, **algum** nó da malha precisa ter uma saída: uma
operadora, uma fibra, um satélite. O que a malha faz é **dividir essa
saída** com os vizinhos, como as redes comunitárias já fazem:

| Rede comunitária | O que provou |
|---|---|
| Guifi.net (Espanha) | dezenas de milhares de nós, a maior do mundo |
| Freifunk (Alemanha) | roteadores de voluntários formando malha nas cidades |
| NYC Mesh (EUA) | antenas em telhados, saída compartilhada |

Lição delas: o difícil não é o software. É o **enlace físico** (cada nó
precisa alcançar outro pelo ar) e a **manutenção**.

**O alcance real de cada meio:**

| Meio | Alcance | Velocidade | Serve para |
|---|---|---|---|
| Wi-Fi de PC ou celular | dezenas de metros | alta | dentro de casa, lan house |
| Wi-Fi com antena no telhado (5 GHz, ponto a ponto) | 1 a 30 km com visada | dezenas a centenas de Mbit/s | ligar bairros e cidades |
| LoRa | quilômetros | poucos kbit/s | mensagens, transações e pagamentos, nunca vídeo |
| Bluetooth | metros | baixa | troca entre aparelhos próximos |

Então "internet sem operadora" exige **equipamento**: roteadores com antena
nos telhados, alinhados entre si. O computador do usuário sozinho não leva
internet longe.

### O modelo de negócio, e a lei

Ideias que fazem sentido:

1. **Kit Hyurax de malha**: roteador com antena, já com o nó instalado,
   para formar a rede entre casas, escolas e comércios.
2. **Quem tem saída de internet e compartilha recebe pelo tráfego** que
   passou por ele, medido como os créditos de computação já são. Isso não
   cria demanda artificial por HYX ([ECONOMIA.md](ECONOMIA.md)).
3. **Rede local de serviços**: pagamentos, mensagens e arquivos que
   funcionam mesmo quando a operadora cai.

Antes de vender qualquer coisa disso, sem aconselhamento jurídico:

- **Vender acesso à internet** no Brasil é Serviço de Comunicação Multimídia,
  regulado pela Anatel. Pequenos provedores têm regime simplificado, mas
  ainda há cadastro e obrigações. Precisa de CNPJ e de parecer de
  especialista.
- **Equipamento de rádio** precisa ser homologado pela Anatel e respeitar o
  limite de potência das faixas livres (2,4 GHz, 5 GHz, 915 MHz).
- **Responsabilidade pelo tráfego**: quem compartilha a saída de internet
  responde pelo que passa nela. A malha precisa de regras de uso e de
  registro mínimo.

---

## Ordem proposta

1. **Agora, sem nenhum servidor:** M1 (descoberta na rede local). Ela
   deixa a rede funcionar no teste da lan house sem semente nenhuma.
2. **Depois:** M2 e M3, para a internet. Cada usuário com porta aberta ou
   IPv6 vira entrada, e os outros passam pelas pontes.
3. **Em seguida:** M5 (Éter no nó) e a 2a (dados da própria rede).
4. **Por último, com CNPJ e parecer:** M4, o kit de malha e o
   compartilhamento de saída de internet (2b).

## O que isto não é

- Não é internet de graça sem equipamento: o ar tem alcance, e a saída para
  o mundo tem dono.
- Não é anonimato: cifrar o conteúdo não esconde quem fala com quem.
- Não é promessa de prazo nem de renda.
