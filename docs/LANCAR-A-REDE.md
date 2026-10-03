# Lançar a rede

Este é o passo a passo para sair de "roda na minha máquina" para "roda na
internet, com gente de fora". Cada passo tem o comando exato e diz quem pode
fazer: **você** (tem conta, senha ou máquina) ou **qualquer um**.

> O que se lança agora é a **rede de teste**. O HYX dela não tem valor, não há
> venda, pré-venda nem promessa de lucro. A rede principal tem critérios
> próprios, no fim deste documento, e nenhum deles está cumprido ainda.

## O que já está pronto

Ensaiado nesta máquina, com processos separados conversando por socket — que é
o que vai acontecer na internet, só que ali com IP de verdade:

| O que | Resultado |
|---|---|
| Um nó sobe e escuta | `Escutando na porta 18790` |
| Outro nó acha o primeiro e sincroniza | altura 0 → 3, sozinho |
| Bloco minerado num nó aparece no outro | `altura 3 · 0 no mempool` |
| Transferência assinada num terceiro processo | `Enviada para 1 par(es)` |
| A transferência entra num bloco minerado por outra máquina | bloco 4 |
| O destinatário aparece com o dinheiro no primeiro nó | `Saldo gastável: 12.50000000 HYX` |

Falta o que só a internet dá: IP de fora, porta aberta e gente.

## A gênese da rede de teste

Todo nó que abrir na rede de teste começa exatamente nisto:

```text
rede:   hyurax-testnet
altura: 0
ponta:  339a70718105e5be912966c59a481deceb71bc8d7b0d88db00caea8637319a3b1
        0568f8778b4eded095dfd97672c157503fcd28fa73bd84e9f901a10971e154e
```

Confira com `hyurax-no estado --rede testnet --pasta pasta-nova`. Se a sua
ponta na altura 0 for outra, **você não está na mesma rede** — quase sempre é
versão antiga do programa.

## Passo 1 — publicar o código (você)

O site e o programa apontam para `NOXqubit/hyurax`.

1. No GitHub, confira em *Settings → Repository name* que o repositório se
   chama `hyurax`.
2. Mande o ramo de trabalho e junte na `main` (pelo site do GitHub, ou
   `git checkout main && git merge <ramo>`).
3. *Settings → Pages* → branch `gh-pages`, pasta `/`.

Depois disso, três endereços passam a existir — e três coisas do programa
dependem deles: o botão de baixar do site, a lista de sementes e o link do
código dentro do LEIA-ME.

## Passo 2 — publicar os programas (você)

O GitHub monta os pacotes sozinho quando uma etiqueta `v*` chega:

```bash
git tag v1.0.0
```

```bash
git push origin v1.0.0
```

Em *Actions* aparece o `lancamento`. No fim, em *Releases*, ficam os pacotes com
a soma SHA-256 de cada um: Windows com janela, Windows/Linux/macOS de terminal
e o de ARM que serve para o celular no Termux.

Confira **antes de divulgar**: baixe o instalador do Windows, confira a soma, instale e abra o
programa. A versão que ele mostra tem que ser a mesma da etiqueta
(`1.0.0`), com "rede de teste" na barra de cima.

### Passo 2b — assinar a atualização (você, a partir da 1.0.1)

Os programas instalados só aceitam versão nova cujo manifesto foi assinado
pela chave de lançamento (o segredo fica em `D:\chaves-hyurax\lancamento.chave`,
fora de qualquer repositório; a chave pública está em
`rede/chave-de-lancamento.pub`). O GitHub não tem o segredo, então este passo
é feito aqui, depois que a Release sai:

1. Baixe da Release o `hyurax-instalador-windows-x86_64.exe`.
2. Assine:

```bash
hyurax-no lancamento assinar --chave D:\chaves-hyurax\lancamento.chave --instalador hyurax-instalador-windows-x86_64.exe --versao 1.0.1 --notas "o que mudou"
```

3. Confira e publique o `atualizacao.txt` na mesma Release:

```bash
hyurax-no lancamento conferir --arquivo atualizacao.txt --instalador hyurax-instalador-windows-x86_64.exe
```

```bash
gh release upload v1.0.1 atualizacao.txt
```

Os programas abertos acham a versão nova em até 12 horas (ou na hora, em
Ajustes → Atualização → Buscar agora), conferem a assinatura, baixam o
instalador, conferem tamanho e SHA-512 e só então o abrem.

**Guarde uma cópia do segredo fora deste computador** (pendrive guardado,
por exemplo). Perder o segredo obriga a publicar uma versão nova com outra
chave pública, que os programas antigos não aceitam sozinhos.

## Passo 3 — o primeiro nó sempre ligado (você)

Uma rede precisa de pelo menos um endereço fixo para os novos baterem. Três
jeitos, do mais barato para o mais confortável:

| Onde | Custo | O problema |
|---|---|---|
| **O seu PC**, com a porta 8790 aberta no roteador | zero | precisa ficar ligado; o IP de casa muda de vez em quando |
| **O PC do laboratório** (Athlon), quando voltar a dar vídeo | zero | mesmo problema do IP |
| **Máquina grátis na nuvem** (Oracle, Google) | zero em dinheiro, mas pede cartão para confirmar identidade | a conta é sua; ver `docs/NO-SEMENTE.md` |

Para o começo, o seu PC **serve**. A rede de teste não precisa de 24 horas por
dia: precisa de alguém acessível enquanto outra pessoa está tentando entrar.

No Windows, deixe o nó rodando junto com a máquina:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\no-sempre-ligado.ps1
```

O script cria uma tarefa que sobe o nó no logon, diz qual porta abrir no
roteador e mostra o seu endereço de fora.

## Passo 4 — conferir de fora (você, com o celular)

Um nó que só responde dentro de casa não serve de semente. O teste honesto é de
outra internet, e o 4G do celular é de graça:

1. No celular, **desligue o Wi-Fi**.
2. No Termux (ver `docs/MINERAR-NO-CELULAR.md`):

```bash
./hyurax-no no --rede testnet --pasta teste --semente SEU_IP_DE_FORA:8790
```

Se aparecer `1 par(es)`, está no ar. Se não aparecer, é quase sempre uma destas
três: a porta não foi redirecionada no roteador, o Firewall do Windows está
barrando, ou o seu provedor não dá IP público (CGNAT) — nesse caso, só com
máquina na nuvem.

## Passo 5 — publicar o endereço (qualquer um)

O endereço que respondeu entra numa linha do arquivo
[`rede/sementes-testnet.txt`](../rede/sementes-testnet.txt):

```text
203.0.113.7:8790   # nó do João, casa, liga de manhã
```

É esse arquivo que todo nó novo busca quando não tem semente nenhuma
configurada. Por isso ninguém precisa esperar versão nova do programa para a
rede ser encontrada, e não há servidor para pagar.

Onde o programa procura, nesta ordem, parando no primeiro que der resultado:

1. `PASTA/sementes.txt`, na máquina de quem roda;
2. a lista embutida no programa (`SEMENTES_TESTNET`);
3. o arquivo publicado, buscado com `curl`.

`--sem-sementes-padrao` desliga 2 e 3. Estar na lista **não é selo de
confiança**: é só um endereço para bater. Quem atender ainda passa pelo aperto
de mão cifrado, fala a mesma rede e prova cada bloco que mandar.

## Passo 6 — o site conta a verdade (você)

- salve o cartão de compartilhamento: abra `site/tools/cartao.html`, clique em
  **Baixar o PNG** e salve em `site/assets/og.png`;
- troque `SEU-DOMINIO` em `site/index.html` e em `site/sitemap.xml` pelo
  endereço real (o do GitHub Pages serve até existir domínio);
- confira que o botão de baixar chega mesmo no zip da Release.

## Passo 7 — chamar gente (você)

O convite é **rode, teste, ataque, contribua** — nunca "invista", "vai
valorizar" ou "ganhe dinheiro". As regras de divulgação e as ideias de vídeo
estão em `docs/ROTEIRO-LANCAMENTO.md`.

O que pedir a quem chegar:

1. rodar um nó e dizer se sincronizou;
2. minerar um bloco e dizer quanto tempo levou;
3. mandar HYX de teste para outra pessoa;
4. tentar quebrar, e contar como (`SECURITY.md`).

## Não lançar sem isto

- [ ] a soma SHA-256 do pacote confere com a publicada;
- [ ] o programa baixado abre e mostra a versão da etiqueta;
- [ ] a gênese da rede de teste bate com a deste documento;
- [ ] pelo menos um nó respondeu de **fora** da sua rede;
- [ ] a página e o LEIA-ME dizem, sem rodeio, que é rede de teste e que o HYX
      não tem valor;
- [ ] `SECURITY.md` diz para onde mandar falha encontrada.

## O que ainda falta para a rede principal

Continua valendo a lista de `docs/ROTEIRO-LANCAMENTO.md`, e ela **não está
cumprida**. O que falta hoje, do lado técnico:

| Falta | Por quê |
|---|---|
| Endereço com dígito verificador | um caractere trocado hoje manda o dinheiro para o nada, e é mudança de formato: tem que sair antes de existir valor |
| Rede de teste rodando 4 semanas sem falha grave aberta | é o prazo que faz a falha barata aparecer |
| Nós de pelo menos 5 pessoas diferentes | uma rede com um dono só não é uma rede |
| Parâmetros de consenso congelados e spec com hash publicado | depois disso, mudar quebra todo mundo |

Enquanto isso não fechar, o programa continua dizendo em toda tela que é rede de
teste — e isso não é modéstia, é o que é verdade.
