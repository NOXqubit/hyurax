# Minerar Argon2id no celular

O minerador `auron-minerar` (crate `crates/auron-pow`) é Rust puro, sem nenhum
código em C. Por isso ele compila direto no celular, pelo Termux, sem preparar
nada além do próprio Rust.

## O que já funciona e o que ainda não

| Parte | Estado |
|---|---|
| Argon2id com os parâmetros de cada rede (32 MiB na mainnet) | pronto, igual ao gabarito byte a byte |
| Medir a velocidade do aparelho (`medir`) | pronto |
| Achar o nonce de um cabeçalho montado pelo nó (`cabecalho`) | pronto; acha o mesmo nonce que o Python |
| Várias linhas de execução, pausa contra aquecimento | pronto |
| Prova de trabalho útil (seção 9A) em Rust | pronto (`crates/auron-usefulpow`), igual ao gabarito; o `medir` mostra quanto custa no aparelho |
| Montar o cabeçalho com a prova útil | **ainda não**; depende do bloco e da cadeia em Rust |
| Receber blocos da rede e minerar sozinho | **ainda não**; depende do nó e da rede P2P |

Ou seja: o celular já faz a parte pesada do Argon2id e já dá para medir quanto
ele aguenta. Minerar "de verdade", ligado numa rede, vem quando o nó em Rust
existir.

## Por que o celular aguenta

- **Memória:** cada linha usa 32 MiB. Com 2 linhas são 64 MiB, pouco para um
  celular com 3 GB ou mais.
- **Mesma regra para todos:** o celular roda exatamente o mesmo programa do
  computador. Os 32 MiB por tentativa fazem a memória pesar tanto quanto o
  processador, e isso tira parte da vantagem de máquinas especializadas.
- **Trabalho útil:** o produto de matrizes do bloco vai até 256 × 256, cerca de
  17 milhões de contas, e é feito uma vez por bloco, não por tentativa.

Referência medida num PC com Atom (processador fraco, de 2012):

| Medida | Resultado |
|---|---|
| Argon2id, mainnet | 5,7 a 6 tentativas por segundo por linha |
| Trabalho útil 256 × 256, fazer | 0,08 s |
| Trabalho útil 256 × 256, conferir | 0,04 s |
 Celulares recentes costumam ser mais rápidos
por núcleo, mas isso é estimativa: o número que vale é o do `medir` no seu
aparelho.

## Passo a passo no Termux

1. Instale o **Termux pelo F-Droid** (a versão da Play Store está parada).
2. No Termux:

```bash
pkg update
```

```bash
pkg install rust git
```

3. Baixe o código do Auron (o arquivo `.zip` da seção "Engenharia aberta" do
   site, ou pelo Git) e entre na pasta do projeto.
4. Compile só o minerador (a primeira vez demora alguns minutos):

```bash
cargo build --release -p auron-pow
```

5. Meça o aparelho:

```bash
./target/release/auron-minerar medir --linhas 2 --segundos 30
```

O Rust precisa ser 1.98 ou mais novo (`rustc --version`). Se o Termux tiver um
mais antigo, rode `pkg upgrade`.

## Opções

| Opção | O que faz | Padrão |
|---|---|---|
| `--rede mainnet\|testnet\|regtest` | parâmetros do Argon2id | `mainnet` |
| `--linhas N` | quantos núcleos usar; cada um gasta 32 MiB | metade dos núcleos |
| `--segundos S` | duração do `medir` | 20 |
| `--pausa-ms P` | descanso depois de cada tentativa, para não esquentar | 0 |
| `--nonce-inicial N` | de onde começa a busca | 0 |

## Cuidados com o aparelho

- **Use metade dos núcleos** (é o padrão). O celular continua usável e esquenta
  menos.
- **Minere na tomada.** Minerar gasta bateria rápido.
- **Esquentou?** Aumente `--pausa-ms` (por exemplo 200) ou diminua `--linhas`.
  Celular quente reduz a velocidade sozinho, então forçar não compensa.
- **O Android pode fechar o Termux em segundo plano.** Deixe a tela do Termux
  aberta, ou use `termux-wake-lock`.

**Lembrete do projeto:** a rede pública não existe e o AUR não tem valor.
Minerar hoje é teste e medição, não ganho.
