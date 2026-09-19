# Auron

Blockchain com prova de trabalho, em desenvolvimento.

**Estado: pré-testnet. Não existe mainnet. Não existe token com valor.**
Qualquer pessoa que disser o contrário está mentindo.

## O que existe hoje

| Parte | Estado |
|---|---|
| Especificação `AURON-SPEC-01` | escrita, congelável quando a tokenomics fechar |
| Implementação de referência (Python) | completa, 122 testes |
| Vetores de validação cruzada | 18 arquivos |
| Nó de produção (Rust) | núcleo migrado e igual ao gabarito: `auron-types`, `auron-crypto`, `auron-codec`, `auron-pow`, `auron-usefulpow`, `auron-tx`, `auron-block`, `auron-consensus`, `auron-state`, `auron-chain`, `auron-store`, `auron-wire`, `auron-net` (80 testes) |
| Rede entre nós | conexão TCP, aperto de mão, sincronização, propagação de blocos e transações, mempool, descoberta de pares (um nó novo acha a rede a partir de uma semente) e retomada automática — tudo em `auron-net`, mais reorganização profunda entre pares (a cadeia com mais trabalho vence, mesmo bifurcando fundo). **Conexão cifrada** com Noise XX e identidade de nó (protocolo versão 2) |
| Nó local e minerador no celular | `auron-no` (carteira **com senha**, envio de AUR de teste, nó em rede com `--porta`/`--semente`, mineração de blocos inteiros, saldo) e `auron-minerar` (medição); rodam no Termux. Ver [`docs/MINERAR-NO-CELULAR.md`](docs/MINERAR-NO-CELULAR.md) |
| Programas prontos | Windows, Linux, celular (Termux) e Mac, montados pelo GitHub a cada versão; guia em [`docs/RODAR-UM-NO.md`](docs/RODAR-UM-NO.md) |
| Testnet pública | kit do nó semente pronto ([`docs/NO-SEMENTE.md`](docs/NO-SEMENTE.md)), com explorador de blocos; nenhum semente no ar ainda; roteiro de 12 semanas em [`docs/ROTEIRO-LANCAMENTO.md`](docs/ROTEIRO-LANCAMENTO.md) |
| Mainnet | não existe |
| Auron Flux (stablecoins e pagamentos) | arquitetura registrada em [`docs/AURON-FLUX.md`](docs/AURON-FLUX.md); estado RED, nada implementado |
| Éter (transporte por qualquer meio) | núcleo pronto e testado em `auron-eter`: objeto fatiado em fragmentos que se provam sozinhos, espalhados por vários meios ao mesmo tempo; meios de hoje: pasta de arquivos e memória. Bluetooth, LoRa e rádio projetados, não implementados. Ver [`docs/AURON-ETER.md`](docs/AURON-ETER.md) |
| Auron Direct, Resonance e Transport (pagamentos P2P, offline e mesh) | arquitetura registrada em [`docs/AURON-DIRECT-RESONANCE.md`](docs/AURON-DIRECT-RESONANCE.md); estado RED, nada implementado |

## Rodar um nó

Programas prontos e o passo a passo em [`docs/RODAR-UM-NO.md`](docs/RODAR-UM-NO.md):
baixar, criar carteira com senha, colocar o nó no ar, minerar e enviar AUR de
teste.

## Como está organizado

```
spec/          AURON-SPEC-01: as regras de consenso
reference/     implementação Python. Não é o nó; é o oráculo
vectors/       o contrato entre Python e Rust
crates/        o nó de produção, em Rust
scripts/       preparo de ambiente
```

O Python **especifica**. O Rust **executa**. Os vetores **provam** que os dois
concordam. Um módulo só é considerado migrado quando cada vetor bate byte a
byte, não quando compila.

## Rodar

Referência Python, sem instalar nada além do numpy:

```bash
python reference/tests/run_all_tests.py
```

Nó Rust:

```bash
cargo test --workspace
```

Os testes de integração da rede sobem nós de verdade em sockets e disputam CPU
com os testes de mineração, então ficam marcados como `ignored` e rodam sob
demanda, isolados (aperto de mão, sincronização, propagação, ataques e
auto-reanimação):

```bash
cargo test -p auron-net -- --ignored --test-threads=1
```

Terminal que não achar `cargo` ou `git`:

```bash
. "D:\auron\scripts\env.ps1"
```

## Máquina nova

Tudo o que o projeto precisa vive em `.toolchain/`, no mesmo disco do código.
Nada é instalado no `C:`. Reinstalar o Windows não derruba o ambiente: o que se
perde são as variáveis do perfil do usuário, não as ferramentas.

| Ferramenta | Versão conferida | Onde |
|---|---|---|
| Rust, alvo GNU | 1.98.1 | `.toolchain/rustup`, `.toolchain/cargo` |
| Git portátil | 2.55.0 | `.toolchain/git` |
| Python embeddable | 3.13.15 | `.toolchain/python` |
| numpy | 2.5.3 | `.toolchain/python/Lib/site-packages` |

Se as variáveis do perfil se perderem:

```powershell
$tc = "D:\auron\.toolchain"
[Environment]::SetEnvironmentVariable('RUSTUP_HOME',   "$tc\rustup",    'User')
[Environment]::SetEnvironmentVariable('CARGO_HOME',    "$tc\cargo",     'User')
[Environment]::SetEnvironmentVariable('PIP_CACHE_DIR', "$tc\pip-cache", 'User')
```

E no `PATH` do usuário: `.toolchain\cargo\bin`, `.toolchain\git\cmd`,
`.toolchain\python`, `.toolchain\python\Scripts`.

Windows reinstalado troca o SID do usuário, e o git passa a recusar o
repositório por dono diferente. Uma vez:

```bash
git config --global --add safe.directory D:/auron
```

Memória virtual, uma vez, como administrador. A mudança só vale depois de
reiniciar:

```powershell
& "D:\auron\scripts\setup-pagefile.ps1"
```

## Decisões de projeto

**PoW Argon2id**, padrão RFC 9106. Memory-hard, sem ponto flutuante, roda em
CPU no CMD do Windows e no Termux do celular. Verificar custa exatamente uma
avaliação, sempre, independente da dificuldade.

**Assinatura Ed25519**, RFC 8032. Verificação em microssegundos, determinística
por construção, sem nonce aleatório que possa vazar a chave.

**Consenso híbrido: trabalho útil em todo bloco.** Cada bloco prova que o
minerador multiplicou matrizes do tamanho exigido (a mesma conta que sustenta
IA), com instância derivada do bloco anterior e do minerador, conferida por
Freivalds. O tamanho cresce com o trabalho validado da rede, separado do
intervalo de bloco. O Argon2id continua como camada complementar. Tarefas de
clientes de verdade ficam no mercado Utrax, fora do consenso: nenhum bloco
depende de alguém publicar tarefa. Limite atual, na spec (§9A): a prova viaja
no bloco, então o tamanho tem teto (n = 256).

**Criptografia com números reais.** Ed25519 dá ~128 bits clássicos; SHA-512,
256. "Classe 1024 bits" é meta interna de arquitetura, não nível atingido, e o
código recusa qualquer primitiva que declare isso. Assinatura híbrida com
ML-DSA-65 está planejada, sem código ainda.

**Só dependência Rust pura.** Nada que precise de compilador C. Sem `ring`,
sem `aws-lc-rs`. É o que permite compilar o mesmo código num PC Linux velho e
num celular via Termux, sem preparar toolchain em lugar nenhum.

**Criptografia com identificador e versão.** A camada AACL registra cada
algoritmo com nome e versão no protocolo. Trocar de algoritmo é registrar um
identificador novo, não reescrever o protocolo.

## Validação contra padrões externos

Nada aqui é "confia em mim". As primitivas críticas batem com vetores oficiais
publicados:

| Primitiva | Padrão |
|---|---|
| Ed25519 | RFC 8032, seção 7.1 |
| Argon2id, Argon2i, Argon2d | RFC 9106, seção 5 |
| Árvore de Merkle | RFC 6962 |

## Honestidade

Este repositório declara o estado real da rede. Enquanto não houver mainnet,
nenhum material do projeto vai dizer que há. Enquanto o Utrax estiver fora do
consenso, nenhum material vai apresentá-lo como lastro econômico da moeda.

## Contribuir e atacar

Tentar quebrar o Auron é bem-vindo: veja [SECURITY.md](SECURITY.md) para saber
como relatar. Ataque só nós seus ou a testnet do projeto.

## Doação

O Auron é feito por um desenvolvedor independente. Se quiser ajudar a manter o
trabalho, o endereço é da **rede Bitcoin** (envie só bitcoin):

```
bc1qkp7d90t9tnmuv2rwq742pwc8pnet28a59zdzt7
```

Depois de colar, confira o começo e o fim: `bc1qkp7d … zdzt7`.

**Doação é voluntária e não dá direito a nada:** nem token, nem participação,
nem retorno.

## Licença

O código do Auron é distribuído sob a licença MIT ou a Apache-2.0, à escolha
de quem usa. Os textos estão em [LICENSE-MIT](LICENSE-MIT) e
[LICENSE-APACHE](LICENSE-APACHE).
