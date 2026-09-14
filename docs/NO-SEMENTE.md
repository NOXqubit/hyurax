# Colocar um nó semente da testnet no ar

Um **nó semente** é um nó sempre ligado, com endereço fixo, onde os nós novos
batem primeiro para encontrar a rede. Depois eles descobrem os outros pares
sozinhos.

> Rede de **teste**: o AUR não tem valor. Não há venda, pré-venda nem promessa
> de lucro.

## Por que não dá para usar o GitHub

O GitHub Pages hospeda sites, mas não roda programa. O GitHub Actions roda
programa por no máximo 6 horas, não aceita conexão de fora, e os termos do
GitHub proíbem usá-lo como servidor permanente. Um semente precisa de uma
máquina ligada 24 horas por dia, com uma porta aberta.

## Onde rodar de graça

| Opção | O que dá | Observação |
|---|---|---|
| **Oracle Cloud "Always Free"** | Máquina ARM de até 4 núcleos e 24 GB, sem prazo | É a mais folgada. Pede cartão para confirmar a identidade, sem cobrança no plano gratuito |
| **Google Cloud e2-micro** | 2 vCPU compartilhadas, 1 GB, em algumas regiões dos EUA | Pede cartão. O tráfego de saída tem limite mensal |
| **Um computador em casa** | Sem custo novo | Precisa ficar ligado e ter a porta 8790 redirecionada no roteador |

Os limites e as regras mudam: confira no site do provedor antes de criar a
conta. **A conta é sua**, e o cadastro você mesmo faz.

## Passo a passo (qualquer Linux com systemd)

1. Crie a máquina com **Ubuntu 22.04 ou 24.04**.
2. No painel do provedor, **libere a entrada TCP** nas portas **8790** (nó) e
   **8080** (explorador). Na Oracle isso fica em *Virtual Cloud Network →
   Security List → Ingress Rules*.
3. Entre pela SSH e rode:

```bash
curl -fsSLO https://raw.githubusercontent.com/NOXqubit/auron/main/deploy/instalar-semente.sh
```

```bash
less instalar-semente.sh
```

```bash
sudo bash instalar-semente.sh
```

O script baixa o `auron-no` da Release, **confere a soma SHA-256**, cria um
usuário sem login, liga dois serviços que voltam sozinhos se a máquina
reiniciar, e mostra o endereço final.

4. Teste de outro computador:

```bash
auron-no no --rede testnet --pasta teste --semente IP_DO_SERVIDOR:8790
```

5. Abra o explorador: `http://IP_DO_SERVIDOR:8080/`.

## Depois que ele responder

Mande o endereço `IP:8790`. Ele entra na lista `SEMENTES_TESTNET` em
`crates/auron-no/src/main.rs`, e a próxima versão do programa já conecta
sozinha, sem ninguém precisar digitar `--semente`.

Só entra na lista um semente que já respondeu de verdade: endereço morto no
programa atrapalha quem está começando.

## Manutenção

| Tarefa | Comando |
|---|---|
| Ver o registro ao vivo | `journalctl -u auron-semente -f` |
| Reiniciar | `sudo systemctl restart auron-semente` |
| Atualizar a versão | `sudo AURON_VERSAO=vX.Y.Z bash instalar-semente.sh` |
| Ver a identidade do nó | `sudo -u auron grep publica /var/lib/auron/testnet/no.chave` |

## O que o semente não faz

- Não guarda carteira nem minera.
- Não recebe comando pela rede: só conversa o protocolo do Auron, cifrado.
- O explorador é só leitura: um arquivo JSON e uma página estática.
