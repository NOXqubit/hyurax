# Marcos do projeto

Cada marco registra o que foi provado, como, e o que **não** foi medido.
Estado possível: `PROVADO EXPERIMENTALMENTE`, `PROVADO EM PRODUÇÃO` ou `PLANEJADO`.

---

## Marco 1 — Laço nó móvel (MOBILE NODE LOOP)

**Estado:** PROVADO EXPERIMENTALMENTE, em 18/09/2026.

### O que foi provado

Duas máquinas de arquiteturas diferentes produziram blocos, trocaram-nos pela
rede cifrada e cada uma validou sozinha o que a outra produziu.

```text
PC (Windows, x86_64)                      Celular (Android, arm64, Termux)
        │                                              │
        │  minera bloco 1 e bloco 2 ─────────────────► │  valida e aceita (altura 2)
        │                                              │
        │ ◄───────────────────────── minera bloco 3    │
        │  valida e aceita (altura 3)                  │
```

- Programas: os binários publicados da versão `v0.1.0-teste.3`, baixados do
  GitHub com a soma SHA-256 conferida. Não é compilação de laboratório.
- Conexão: protocolo v2, cifrada com Noise XX, cada nó provando a própria
  identidade.
- Cada bloco carrega prova de trabalho Argon2id **e** trabalho útil
  (multiplicação de matrizes 48×48, conferida por Freivalds).
- Ao fim, os dois lados tinham a mesma cadeia: altura 3, trabalho total 1024.

### Por onde passou o tráfego

**Pela rede local, não pelo cabo.** O PC estava ligado por Ethernet e o
celular por Wi-Fi, no mesmo roteador. O cabo USB só serviu para instalar os
programas e digitar comandos no celular (adb); nenhum bloco passou por ele.

Isso quer dizer que a etapa "Wi-Fi em rede local" do caminho abaixo **já está
feita**.

### Números medidos

| Medida | PC | Celular |
|---|---|---|
| Tempo para minerar um bloco | 26,7 s e 30,8 s | 39,4 s |
| Linhas de mineração | 2 | 4 |
| Memória do Argon2 | 64 MiB | 128 MiB |
| Blocos produzidos | 2 | 1 |
| Blocos recebidos e aceitos pelo outro lado | 2 de 2 | 1 de 1 |
| Falhas | 0 | 0 |

### Números **não** medidos

Latência de rede, vazão, uso de CPU, uso de RAM do processo, consumo de
energia e taxa de falha sob carga. Três blocos provam que o laço fecha; não
servem de benchmark. Estes números entram no Marco 2.

### O caminho a partir daqui

| Etapa | Estado |
|---|---|
| Rede local, Wi-Fi + Ethernet | **feito** (este marco) |
| Sem internet: celular como ponto de acesso | planejado |
| Éter com pasta de arquivos (atravessa o tempo) | feito em teste, sem nós reais ainda |
| Bluetooth, via app Android próprio | planejado |
| Internet, entre casas diferentes (semente + furo de NAT) | planejado |
| Rede distribuída pública | planejado |

Cada etapa daqui para a frente precisa registrar: latência, vazão, taxa de
sucesso, CPU, RAM, energia, tarefas processadas e taxa de falhas.
