# Auron

Blockchain com prova de trabalho, em desenvolvimento.

**Estado: pré-testnet. Não existe mainnet. Não existe token com valor.**
Qualquer pessoa que disser o contrário está mentindo.

## O que existe hoje

| Parte | Estado |
|---|---|
| Especificação `AURON-SPEC-01` | escrita, congelável quando a tokenomics fechar |
| Implementação de referência (Python) | completa, 121 testes |
| Vetores de validação cruzada | 17 arquivos |
| Nó de produção (Rust) | núcleo migrado e igual ao gabarito: `auron-types`, `auron-crypto`, `auron-codec`, `auron-pow`, `auron-usefulpow`, `auron-tx`, `auron-block`, `auron-consensus`, `auron-state`, `auron-chain`, `auron-store` (73 testes). Falta: rede P2P e mempool |
| Nó local e minerador no celular | `auron-no` (carteira de teste, mineração de blocos inteiros numa cadeia gravada no disco, saldo) e `auron-minerar` (medição); rodam no Termux. Ver [`docs/MINERAR-NO-CELULAR.md`](docs/MINERAR-NO-CELULAR.md) |
| Rede P2P | não começada |
| Testnet pública | não começada |
| Mainnet | não existe |
| Auron Flux (stablecoins e pagamentos) | arquitetura registrada em [`docs/AURON-FLUX.md`](docs/AURON-FLUX.md); estado RED, nada implementado |
| Auron Direct, Resonance e Transport (pagamentos P2P, offline e mesh) | arquitetura registrada em [`docs/AURON-DIRECT-RESONANCE.md`](docs/AURON-DIRECT-RESONANCE.md); estado RED, nada implementado |

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

## Licença

O código do Auron é distribuído sob a licença MIT ou a Apache-2.0, à escolha
de quem usa. Os textos estão em [LICENSE-MIT](LICENSE-MIT) e
[LICENSE-APACHE](LICENSE-APACHE).
