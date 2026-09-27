# Hyurax

Blockchain com prova de trabalho, em desenvolvimento.

**Estado: pré-testnet. Não existe mainnet. Não existe token com valor.**
Qualquer pessoa que disser o contrário está mentindo.

> **O projeto se chamava Auron até 19/09/2026.** O nome mudou porque já havia
> outros projetos com ele (inclusive o Apache Auron e o ticker AUR na bolsa),
> e ninguém deveria confundir um com o outro. A tecnologia é a mesma. As
> versões até `v0.1.0-teste.3` saíram com o nome antigo e falam uma rede
> diferente (outra gênese): para testar, use a partir de `v0.2.0-teste.1`.

> **O ticker mudou de HYUR para HYX em 26/09/2026.** Só o nome na tela: a
> rede, a gênese e as carteiras continuam as mesmas.

## O que existe hoje

| Parte | Estado |
|---|---|
| Especificação `HYURAX-SPEC-01` | escrita, congelável quando a tokenomics fechar |
| Implementação de referência (Python) | completa, 177 testes |
| Vetores de validação cruzada | 25 arquivos |
| Nó de produção (Rust) | núcleo migrado e igual ao gabarito: `hyurax-types`, `hyurax-crypto`, `hyurax-codec`, `hyurax-pow`, `hyurax-usefulpow`, `hyurax-tx`, `hyurax-block`, `hyurax-consensus`, `hyurax-state`, `hyurax-chain`, `hyurax-store`, `hyurax-wire`, `hyurax-net` (80 testes) |
| Rede entre nós | conexão TCP, aperto de mão, sincronização, propagação de blocos e transações, mempool, descoberta de pares (um nó novo acha a rede a partir de uma semente) e retomada automática — tudo em `hyurax-net`, mais reorganização profunda entre pares (a cadeia com mais trabalho vence, mesmo bifurcando fundo). **Conexão cifrada** com Noise XX e identidade de nó (protocolo versão 2) |
| Nó local e minerador no celular | `hyurax-no` (carteira **com senha**, envio de HYX de teste, nó em rede com `--porta`/`--semente`, mineração de blocos inteiros, saldo) e `hyurax-minerar` (medição); rodam no Termux. Ver [`docs/MINERAR-NO-CELULAR.md`](docs/MINERAR-NO-CELULAR.md) |
| Programas prontos | **instalador para Windows** (termos de uso, sem administrador, desinstala pelo Windows) e o programa com janela; pacotes de terminal para Windows, Linux, celular (Termux) e Mac, montados pelo GitHub a cada versão; guia em [`docs/RODAR-UM-NO.md`](docs/RODAR-UM-NO.md) |
| ULTRAX (trabalho útil) | worker no programa, modo LAB: matrizes, otimização, simulação e treino de IA sobre 2.048 moléculas reais, na CPU e na GPU (inclusive a integrada), cada resultado conferido antes de contar; especificado em Python e igual no Rust por vetor. Ver [`docs/ULTRAX.md`](docs/ULTRAX.md). O Work Score mede contribuição: não é dinheiro nem HYX |
| Computação científica (JOBs) | JOB dividido em unidades, cada uma conferida antes de contar, com checkpoint, retomada e relatório em JSON, CSV e PDF. Motores de verdade: genética de populações, melhoramento de plantas, rotas (logística), triagem de 8.289 moléculas reais, IA, matriz, mochila e difusão. Nível 3 de verificação manda a mesma unidade para workers **de outros nós**, com compromisso antes da revelação e maioria, e a conferência daqui recusa conluio. Visão 3D alimentada só por eventos reais, e ULTRA BENCHMARK com base, atual, alvo e ganho medidos. Créditos de computação separados do HYX, sem liquidação. Ensaiado com vários processos numa máquina; entre máquinas, fica para a testnet. Ver [`docs/COMPUTACAO-CIENTIFICA.md`](docs/COMPUTACAO-CIENTIFICA.md) |
| Termos de uso | [`docs/TERMOS-DE-USO.md`](docs/TERMOS-DE-USO.md), versão 2, aceitos no instalador e na primeira abertura; revisados pelo projeto contra o CDC, a LGPD, o Marco Civil, o ECA, o Código Penal e a Lei 14.478 (lista no começo do arquivo), **sem assinatura de advogado** |
| Assinatura digital do programa | não tem ainda; opções, custos e passo a passo em [`docs/ASSINATURA-DIGITAL.md`](docs/ASSINATURA-DIGITAL.md) |
| Testnet pública | ensaiada com três processos separados (bloco e transferência atravessando a rede); passo a passo do lançamento em [`docs/LANCAR-A-REDE.md`](docs/LANCAR-A-REDE.md); lista de sementes publicada em [`rede/sementes-testnet.txt`](rede/sementes-testnet.txt), que o nó busca sozinho; kit do nó semente em [`docs/NO-SEMENTE.md`](docs/NO-SEMENTE.md); **nenhum semente no ar ainda**; roteiro de 12 semanas em [`docs/ROTEIRO-LANCAMENTO.md`](docs/ROTEIRO-LANCAMENTO.md) |
| Mainnet | não existe |
| HYX Index (cesta de 7 referências: ouro, Bitcoin, prata, cobre, platina, paládio e petróleo) | medida informativa, fora do consenso, sem lastro e sem resgate; documento em [`docs/HYX-INDEX.md`](docs/HYX-INDEX.md); estado RED, nada calculado ainda |
| Hyurax Flux (stablecoins e pagamentos) | arquitetura registrada em [`docs/HYURAX-FLUX.md`](docs/HYURAX-FLUX.md); estado RED, nada implementado |
| Éter (transporte por qualquer meio) | núcleo pronto e testado em `hyurax-eter`: objeto fatiado em fragmentos que se provam sozinhos, espalhados por vários meios ao mesmo tempo; meios de hoje: pasta de arquivos e memória. Bluetooth, LoRa e rádio projetados, não implementados. Ver [`docs/HYURAX-ETER.md`](docs/HYURAX-ETER.md) |
| Hyurax Direct, Resonance e Transport (pagamentos P2P, offline e mesh) | arquitetura registrada em [`docs/HYURAX-DIRECT-RESONANCE.md`](docs/HYURAX-DIRECT-RESONANCE.md); estado RED, nada implementado |

## Rodar um nó

Programas prontos e o passo a passo em [`docs/RODAR-UM-NO.md`](docs/RODAR-UM-NO.md):
baixar, criar carteira com senha, colocar o nó no ar, minerar e enviar HYX de
teste.

## Como está organizado

```
spec/          HYURAX-SPEC-01: as regras de consenso
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
cargo test -p hyurax-net -- --ignored --test-threads=1
```

Terminal que não achar `cargo` ou `git`:

```bash
. "D:\hyurax\scripts\env.ps1"
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
$tc = "D:\hyurax\.toolchain"
[Environment]::SetEnvironmentVariable('RUSTUP_HOME',   "$tc\rustup",    'User')
[Environment]::SetEnvironmentVariable('CARGO_HOME',    "$tc\cargo",     'User')
[Environment]::SetEnvironmentVariable('PIP_CACHE_DIR', "$tc\pip-cache", 'User')
```

E no `PATH` do usuário: `.toolchain\cargo\bin`, `.toolchain\git\cmd`,
`.toolchain\python`, `.toolchain\python\Scripts`.

Windows reinstalado troca o SID do usuário, e o git passa a recusar o
repositório por dono diferente. Uma vez:

```bash
git config --global --add safe.directory D:/hyurax
```

Memória virtual, uma vez, como administrador. A mudança só vale depois de
reiniciar:

```powershell
& "D:\hyurax\scripts\setup-pagefile.ps1"
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

Tentar quebrar o Hyurax é bem-vindo: veja [SECURITY.md](SECURITY.md) para saber
como relatar. Ataque só nós seus ou a testnet do projeto.

## Doação

O Hyurax é feito por um desenvolvedor independente. Se quiser ajudar a manter o
trabalho, o endereço é da **rede Bitcoin** (envie só bitcoin):

```
bc1qkp7d90t9tnmuv2rwq742pwc8pnet28a59zdzt7
```

Depois de colar, confira o começo e o fim: `bc1qkp7d … zdzt7`.

**Doação é voluntária e não dá direito a nada:** nem token, nem participação,
nem retorno.

## Licença

O código do Hyurax é distribuído sob a licença MIT ou a Apache-2.0, à escolha
de quem usa. Os textos estão em [LICENSE-MIT](LICENSE-MIT) e
[LICENSE-APACHE](LICENSE-APACHE).
