# Rode um nó do Hyurax em 5 minutos

> **Hyurax / Ultrax 1.0, rede de TESTE.** O programa é a versão 1.0; a rede
> é de teste e o HYX não tem valor. Não há venda, pré-venda nem promessa de
> lucro. Rodar um nó hoje é testar,
> medir e ajudar a achar falhas.

Um nó guarda a cadeia, confere cada bloco e cada transação sozinho e conversa
com outros nós por uma **conexão cifrada** (Noise XX). A carteira fica
**cifrada com a sua senha**.

## 1. Baixar

> **Quer só o programa com janela no Windows, sem terminal?** Baixe o
> `hyurax-instalador-windows-x86_64.exe`, leia e aceite os termos e clique em
> *Instalar*. Não pede administrador. Este guia é para quem quer o nó de
> terminal.

Na página de [Releases do GitHub](https://github.com/NOXqubit/hyurax/releases),
baixe o pacote do seu sistema:

| Pacote | Para |
|---|---|
| `windows-x86_64.zip` | Windows 10 ou 11 |
| `linux-x86_64.tar.gz` | PC com Linux |
| `linux-arm64-e-celular.tar.gz` | Android pelo Termux, e Linux em ARM |
| `macos-arm64.tar.gz` | Mac com chip Apple |

**Confira a soma SHA-256** antes de rodar. Ela vem num arquivo `.sha256` ao
lado do pacote. No Windows:

```powershell
Get-FileHash .\hyurax-*-windows-x86_64.zip -Algorithm SHA256
```

No Linux, no Mac ou no Termux:

```bash
sha256sum -c hyurax-*.tar.gz.sha256
```

Depois, descompacte e entre na pasta. Nos exemplos abaixo, o programa aparece
como `./hyurax-no`; no Windows é `.\hyurax-no.exe`.

Prefere compilar? Com Rust 1.98 ou mais novo:
`cargo build --release -p hyurax-no -p hyurax-pow` (os programas ficam em
`.target/release/`).

**Celular (Termux):** depois de descompactar, rode `chmod +x hyurax-no`.
Se o sistema recusar o programa, compile no próprio aparelho, como está em
[MINERAR-NO-CELULAR.md](https://github.com/NOXqubit/hyurax/blob/main/docs/MINERAR-NO-CELULAR.md).

## 2. Criar a carteira

```bash
./hyurax-no carteira nova --arquivo carteira.txt
```

O programa pede uma senha (mínimo de 10 caracteres), que não aparece na tela
enquanto você digita, e mostra o seu **endereço**.

- Sem a senha, o arquivo não gasta nada.
- **Sem o arquivo e a senha juntos, o saldo fica perdido.** Guarde uma cópia do
  arquivo e não esqueça a senha. Ninguém consegue recuperar por você.

Para ver o endereço de novo: `./hyurax-no carteira ver --arquivo carteira.txt`.

O endereço da rede de teste começa com `thyx1` e tem um **dígito verificador**
(formato Bech32m, o mesmo do Bitcoin moderno): um erro de digitação é
recusado, em vez de mandar HYX para um endereço que ninguém controla. A rede
principal, quando existir, usa `hyx1`, e um endereço de uma rede é recusado na
outra. O formato antigo, com 40 dígitos hexadecimais, continua aceito, mas
sem essa proteção.

## 3. Colocar o nó no ar

```bash
./hyurax-no no --rede testnet --pasta dados --porta 8790
```

- `--pasta dados` é onde ficam a cadeia e a **identidade do nó**
  (`dados/no.chave`). A identidade não é carteira e não guarda saldo.
- `--porta 8790` deixa outros nós conectarem em você. Para receber conexões da
  internet, libere essa porta no roteador (redirecionamento de porta). Sem
  isso, o nó ainda funciona, mas só conecta para fora.

Na rede de teste, **o nó procura a rede sozinho**: sem `--semente`, ele lê a
lista publicada no repositório do projeto
([`rede/sementes-testnet.txt`](../rede/sementes-testnet.txt)) e bate nos
endereços dela. Para escolher você mesmo, use `--semente IP:PORTA` (várias,
separadas por vírgula) ou escreva um endereço por linha em
`PASTA/sementes.txt`. Depois do primeiro par, o nó descobre os outros sozinho.
`--sem-sementes-padrao` faz o nó não buscar lista nenhuma.

> **A lista publicada ainda está vazia.** Enquanto o primeiro semente não
> responde de fora, teste com dois aparelhos seus na mesma rede local, ou com
> um amigo: um roda `no --porta 8790` e o outro usa
> `--semente IP_DO_PRIMEIRO:8790`.

## 4. Minerar

### Com o painel (recomendado no PC)

```bash
./hyurax-no painel --arquivo carteira.txt --semente IP:PORTA
```

Abra `http://127.0.0.1:8800` no navegador. É o mesmo painel do programa com
janela, com oito seções:
- **Visão geral:** o trabalho em foco (trabalho → como → recurso → resultado →
  verificação → impacto na rede), a cena 3D desenhada das amostras reais do
  cálculo, as métricas desta máquina e o registro recente;
- **ULTRAX:** liga o worker de trabalho útil, com linhas de CPU, limite por
  linha, teto de memória e GPU; tarefas executando, placar e histórico;
- **Computação científica:** JOBs, relatórios e o ULTRA BENCHMARK;
- **Carteira, Cadeia e mineração, Rede, Registro e Ajustes.**

Cada número diz de onde veio: REAL (medido), DERIVADO, ESTIMADO, SIMULADO
(a carga LAB, gerada nesta máquina), AJUSTE (escolha sua) ou PENDENTE.

Nada disso precisa de internet: o painel vem dentro do programa.

- `--painel-porta 9000` muda a porta.
- `--painel-rede` deixa o celular no mesmo Wi-Fi **ver** o painel, em
  `http://IP_DO_PC:8800`. Os comandos continuam só no próprio PC.

### Só no terminal

```bash
./hyurax-no minerar --rede testnet --pasta dados --endereco SEU_ENDERECO --blocos 0 --semente IP:PORTA
```

- `--blocos 0` minera sem parar; `Ctrl+C` interrompe, e o que já foi minerado
  fica salvo.
- Ele sincroniza com a rede **antes** de minerar, para não trabalhar num ramo
  que vai ser descartado.
- A recompensa espera 20 blocos na testnet para poder ser gasta.
- No celular: `--linhas 1` e `--pausa-ms 200` esquentam menos.

## 5. Enviar HYX de teste

```bash
./hyurax-no enviar --rede testnet --pasta dados --arquivo carteira.txt --para ENDERECO_DESTINO --valor 1.5 --semente IP:PORTA
```

O programa sincroniza, confere o saldo, pede a senha, assina e manda para a
rede. A transação entra no próximo bloco que um minerador achar. Use ponto no
valor (`1.5`), com até 8 casas.

## 6. Ver saldo e altura

```bash
./hyurax-no estado --rede testnet --pasta dados --endereco SEU_ENDERECO
```

## Problemas comuns

| O que aparece | O que fazer |
|---|---|
| `senha errada, ou arquivo de carteira alterado` | Confira a senha. Se estiver certa, o arquivo foi mexido: use a sua cópia |
| `saldo gastável insuficiente` | A recompensa de mineração ainda não liberou, ou o nó não sincronizou: confira com `estado` |
| `não alcancei a rede em 90 s` | A semente está fora do ar ou a porta está bloqueada |
| `aperto de mão recusado` | O outro nó é de outra rede (`--rede`) ou de uma versão antiga |
| Carteira antiga, criada antes de 13/09/2026 | Proteja com `./hyurax-no carteira cifrar --arquivo carteira.txt` |

## Achou uma falha?

Tentar quebrar o Hyurax é bem-vindo, desde que seja em nós seus ou na testnet do
projeto. Veja como relatar em [SECURITY.md](https://github.com/NOXqubit/hyurax/blob/main/SECURITY.md).
