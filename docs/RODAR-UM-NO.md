# Rode um nó do Auron em 5 minutos

> **Rede de TESTE.** A rede pública ainda não existe e o AUR não tem valor.
> Não há venda, pré-venda nem promessa de lucro. Rodar um nó hoje é testar,
> medir e ajudar a achar falhas.

Um nó guarda a cadeia, confere cada bloco e cada transação sozinho e conversa
com outros nós por uma **conexão cifrada** (Noise XX). A carteira fica
**cifrada com a sua senha**.

## 1. Baixar

Na página de [Releases do GitHub](https://github.com/NOXqubit/auron/releases),
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
Get-FileHash .\auron-*-windows-x86_64.zip -Algorithm SHA256
```

No Linux, no Mac ou no Termux:

```bash
sha256sum -c auron-*.tar.gz.sha256
```

Depois, descompacte e entre na pasta. Nos exemplos abaixo, o programa aparece
como `./auron-no`; no Windows é `.\auron-no.exe`.

Prefere compilar? Com Rust 1.98 ou mais novo:
`cargo build --release -p auron-no -p auron-pow` (os programas ficam em
`.target/release/`).

**Celular (Termux):** depois de descompactar, rode `chmod +x auron-no auron-minerar`.
Se o sistema recusar o programa, compile no próprio aparelho, como está em
[MINERAR-NO-CELULAR.md](https://github.com/NOXqubit/auron/blob/main/docs/MINERAR-NO-CELULAR.md).

## 2. Criar a carteira

```bash
./auron-no carteira nova --arquivo carteira.txt
```

O programa pede uma senha (mínimo de 10 caracteres), que não aparece na tela
enquanto você digita, e mostra o seu **endereço**.

- Sem a senha, o arquivo não gasta nada.
- **Sem o arquivo e a senha juntos, o saldo fica perdido.** Guarde uma cópia do
  arquivo e não esqueça a senha. Ninguém consegue recuperar por você.

Para ver o endereço de novo: `./auron-no carteira ver --arquivo carteira.txt`.

## 3. Colocar o nó no ar

```bash
./auron-no no --rede testnet --pasta dados --porta 8790
```

- `--pasta dados` é onde ficam a cadeia e a **identidade do nó**
  (`dados/no.chave`). A identidade não é carteira e não guarda saldo.
- `--porta 8790` deixa outros nós conectarem em você. Para receber conexões da
  internet, libere essa porta no roteador (redirecionamento de porta). Sem
  isso, o nó ainda funciona, mas só conecta para fora.

Para entrar numa rede que já existe, adicione `--semente IP:PORTA` de um nó
conhecido (várias, separadas por vírgula). O nó descobre os outros sozinho.

> **Nós semente oficiais ainda não existem.** Enquanto a testnet pública não
> sobe, teste com dois aparelhos seus na mesma rede local, ou com um amigo:
> um roda `no --porta 8790` e o outro usa `--semente IP_DO_PRIMEIRO:8790`.

## 4. Minerar

```bash
./auron-no minerar --rede testnet --pasta dados --endereco SEU_ENDERECO --blocos 0 --semente IP:PORTA
```

- `--blocos 0` minera sem parar; `Ctrl+C` interrompe, e o que já foi minerado
  fica salvo.
- Ele sincroniza com a rede **antes** de minerar, para não trabalhar num ramo
  que vai ser descartado.
- A recompensa espera 20 blocos na testnet para poder ser gasta.
- No celular: `--linhas 1` e `--pausa-ms 200` esquentam menos.

## 5. Enviar AUR de teste

```bash
./auron-no enviar --rede testnet --pasta dados --arquivo carteira.txt --para ENDERECO_DESTINO --valor 1.5 --semente IP:PORTA
```

O programa sincroniza, confere o saldo, pede a senha, assina e manda para a
rede. A transação entra no próximo bloco que um minerador achar. Use ponto no
valor (`1.5`), com até 8 casas.

## 6. Ver saldo e altura

```bash
./auron-no estado --rede testnet --pasta dados --endereco SEU_ENDERECO
```

## Problemas comuns

| O que aparece | O que fazer |
|---|---|
| `senha errada, ou arquivo de carteira alterado` | Confira a senha. Se estiver certa, o arquivo foi mexido: use a sua cópia |
| `saldo gastável insuficiente` | A recompensa de mineração ainda não liberou, ou o nó não sincronizou: confira com `estado` |
| `não alcancei a rede em 90 s` | A semente está fora do ar ou a porta está bloqueada |
| `aperto de mão recusado` | O outro nó é de outra rede (`--rede`) ou de uma versão antiga |
| Carteira antiga, criada antes de 13/09/2026 | Proteja com `./auron-no carteira cifrar --arquivo carteira.txt` |

## Achou uma falha?

Tentar quebrar o Auron é bem-vindo, desde que seja em nós seus ou na testnet do
projeto. Veja como relatar em [SECURITY.md](https://github.com/NOXqubit/auron/blob/main/SECURITY.md).
