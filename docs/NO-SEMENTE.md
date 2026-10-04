# Colocar um nó semente da testnet no ar

Um **nó semente** é um nó sempre ligado, com endereço fixo, onde os nós novos
batem primeiro para encontrar a rede. Depois eles descobrem os outros pares
sozinhos.

> Rede de **teste**: o HYX não tem valor. Não há venda, pré-venda nem promessa
> de lucro.

## Por que não dá para usar o GitHub

O GitHub Pages hospeda sites, mas não roda programa. O GitHub Actions roda
programa por no máximo 6 horas, não aceita conexão de fora, e os termos do
GitHub proíbem usá-lo como servidor permanente. Um semente precisa de uma
máquina ligada 24 horas por dia, com uma porta aberta.

## Onde rodar de graça

| Opção | O que dá | Observação |
|---|---|---|
| **Render (plano grátis)** | Contêiner sempre no ar (750 h por mês), só HTTPS | **Sem cartão**, entra com a conta do GitHub. Como só passa HTTPS, o nó usa WebSocket (`wss://…/p2p`); ver a seção abaixo |
| **Oracle Cloud "Always Free"** | Máquina ARM de até 4 núcleos e 24 GB, sem prazo | É a mais folgada. Pede cartão para confirmar a identidade, sem cobrança no plano gratuito |
| **Google Cloud e2-micro** | 2 vCPU compartilhadas, 1 GB, em algumas regiões dos EUA | Pede cartão. O tráfego de saída tem limite mensal |
| **Um computador em casa** | Sem custo novo | Precisa ficar ligado e ter a porta 8790 redirecionada no roteador |

Os limites e as regras mudam: confira no site do provedor antes de criar a
conta. **A conta é sua**, e o cadastro você mesmo faz.

## No Render (grátis, sem cartão)

O Render só deixa entrar HTTPS, e não uma porta TCP qualquer. Desde a 1.3.0, o
nó também escuta **WebSocket** (`--ws-porta`), e as sementes podem ser escritas
como `wss://NOME.onrender.com/p2p`. Por dentro do WebSocket passa a mesma
conexão de sempre, com o aperto de mão Noise XX e a validação de cada bloco.

1. No Render: **New → Web Service → Public Git Repository**, com o endereço
   `https://github.com/NOXqubit/hyurax`. Não precisa ligar o GitHub ao
   Render, porque o repositório é público.
2. Linguagem **Docker**; **Dockerfile Path** `deploy/render/Dockerfile`;
   **Docker Build Context** `deploy/render`; plano **Free**.
3. **Health Check Path**: `/saude`.
4. Depois de subir, `https://NOME.onrender.com/saude` mostra a altura e os
   pares. Coloque `wss://NOME.onrender.com/p2p` em `rede/sementes-testnet.txt`.

O que o plano grátis impõe:

- **Dorme depois de 15 minutos sem tráfego.** A conexão dos nós já mantém o
  serviço acordado, e o workflow `semente-acordada.yml` dá um toque a cada
  10 minutos. Quem chega com ele dormindo espera cerca de um minuto (o nó
  espera até 90 segundos pela resposta).
- **O disco some a cada reinício:** a cadeia é baixada de novo dos outros
  nós, e a identidade do nó muda. Para uma semente, que é só porta de
  entrada, tudo bem.
- **750 horas por mês**: dá um serviço ligado o mês inteiro.

O TLS do `wss://` usa o rustls com criptografia em Rust puro
(rustls-rustcrypto, ainda em versão alfa). Isso foi aceito porque o TLS ali é
só o cano: quem prova a identidade e cifra o tráfego entre os nós é o Noise.

## Passo a passo (qualquer Linux com systemd)

1. Crie a máquina com **Ubuntu 22.04 ou 24.04**.
2. No painel do provedor, **libere a entrada TCP** nas portas **8790** (nó) e
   **8080** (explorador). Na Oracle isso fica em *Virtual Cloud Network →
   Security List → Ingress Rules*.
3. Entre pela SSH e rode:

```bash
curl -fsSLO https://raw.githubusercontent.com/NOXqubit/hyurax/main/deploy/instalar-semente.sh
```

```bash
less instalar-semente.sh
```

```bash
sudo bash instalar-semente.sh
```

O script baixa o `hyurax-no` da Release, **confere a soma SHA-256**, cria um
usuário sem login, liga dois serviços que voltam sozinhos se a máquina
reiniciar, e mostra o endereço final.

4. Teste de outro computador:

```bash
hyurax-no no --rede testnet --pasta teste --semente IP_DO_SERVIDOR:8790
```

5. Abra o explorador: `http://IP_DO_SERVIDOR:8080/`.

## No Windows, com o seu próprio PC

Para começar sem conta em nuvem nenhuma:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\no-sempre-ligado.ps1
```

O script cria uma tarefa que sobe o nó a cada logon, abre a porta no Firewall
do Windows (rodando como administrador), mostra o seu endereço de fora e diz o
que redirecionar no roteador. Para desfazer tudo: o mesmo comando com
`-Remover`.

## Depois que ele responder

Confira **de fora** — pelo 4G do celular, com o Wi-Fi desligado. Se aparecer
`1 par(es)`, o endereço `IP:8790` entra numa linha de
[`rede/sementes-testnet.txt`](../rede/sementes-testnet.txt).

Todo nó que abre sem semente configurada busca esse arquivo sozinho (com o
`curl` do sistema), então **não precisa de versão nova do programa** para a
rede ser encontrada. A ordem de procura é: `PASTA/sementes.txt` na máquina de
quem roda, depois a lista embutida `SEMENTES_TESTNET` em
`crates/hyurax-no/src/lib.rs`, depois o arquivo publicado.
`--sem-sementes-padrao` desliga as duas últimas.

Só entra na lista um semente que já respondeu de verdade: endereço morto
atrapalha justamente quem está começando. O passo a passo completo do
lançamento está em [`LANCAR-A-REDE.md`](LANCAR-A-REDE.md).

## Manutenção

| Tarefa | Comando |
|---|---|
| Ver o registro ao vivo | `journalctl -u hyurax-semente -f` |
| Reiniciar | `sudo systemctl restart hyurax-semente` |
| Atualizar a versão | `sudo HYURAX_VERSAO=vX.Y.Z bash instalar-semente.sh` |
| Ver a identidade do nó | `sudo -u hyurax grep publica /var/lib/hyurax/testnet/no.chave` |

## O que o semente não faz

- Não guarda carteira nem minera.
- Não recebe comando pela rede: só conversa o protocolo do Hyurax, cifrado.
- O explorador é só leitura: um arquivo JSON e uma página estática.
