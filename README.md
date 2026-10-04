# Hyurax / Ultrax

Blockchain com prova de trabalho e trabalho computacional útil e conferível.

**Programa: versão 1.0. Rede: TESTNET.** A rede pública é de teste, o HYX de
teste não tem valor e não existe mainnet. Não há venda, pré-venda nem promessa
de lucro. Qualquer pessoa que disser o contrário está mentindo.

## O programa 1.0

Um programa só, com janela, que junta:

- **o nó** da rede de teste (sincroniza, propaga e confere blocos e transações,
  com conexão cifrada Noise XX);
- **a carteira**, cifrada com senha (Argon2id e ChaCha20-Poly1305), com segundo
  fator opcional (código de 6 dígitos);
- **a mineração** (Argon2id + prova de trabalho útil por Freivalds em todo bloco);
- **o ULTRAX**, o worker de trabalho útil: oito motores reais (matrizes, mochila,
  difusão, treino de IA sobre moléculas reais, genética de populações,
  melhoramento de plantas, rotas e triagem de moléculas), na CPU e na GPU pelo
  WebGL 2, cada resultado conferido antes de contar;
- **a computação científica**: JOBs divididos em unidades conferidas, com
  relatório em JSON, CSV e PDF, e verificação entre nós;
- **o painel**: estado do nó, carteira, mineração, tarefas, métricas da máquina
  e a **visualização 3D (WebGL 2) desenhada a partir das amostras reais** que os
  motores entregam enquanto calculam.

Todo número na tela diz de onde veio: **REAL** (medido agora), **DERIVADO**
(conta exata sobre um valor medido), **ESTIMADO** (depende de um parâmetro não
medido), **SIMULADO** (conta de verdade sobre entrada gerada nesta máquina, a
carga LAB), **AJUSTE** (escolha do dono) ou **PENDENTE** (ainda não medido). O
Work Score e os créditos de computação medem contribuição: **não são dinheiro
nem HYX**, e o trabalho do ULTRAX ainda não muda a recompensa do bloco (isso é
mudança de consenso, PENDENTE na SPEC-02).

Instalar: o **instalador para Windows** (sem administrador; termos de uso;
desinstala pelo Windows sem tocar na carteira) ou os pacotes de terminal
(`hyurax-no`) para Windows, Linux, celular (Termux) e Mac. Guia em
[`docs/RODAR-UM-NO.md`](docs/RODAR-UM-NO.md); notas desta versão em
[`docs/NOTAS-1.0.md`](docs/NOTAS-1.0.md); arquitetura em
[`docs/ARQUITETURA.md`](docs/ARQUITETURA.md).

Onde ficam as coisas (Windows): programa em `%LOCALAPPDATA%\Programs\Hyurax`;
configuração e **carteira** em `%APPDATA%\Hyurax`; cadeia, ULTRAX, JOBs e
registros em `%LOCALAPPDATA%\Hyurax`. No Linux, as pastas do XDG.

## O que existe hoje

| Parte | Estado |
|---|---|
| Especificação `HYURAX-SPEC-01` | escrita, congelável quando a tokenomics fechar |
| Implementação de referência (Python) | completa, 177 testes |
| Vetores de validação cruzada | 29 arquivos, conferidos pelo Python, pelo Rust e (endereço, transação e carteira) pelo app do celular |
| Nó de produção (Rust) | `hyurax-types`, `hyurax-crypto`, `hyurax-codec`, `hyurax-pow`, `hyurax-usefulpow`, `hyurax-tx`, `hyurax-block`, `hyurax-consensus`, `hyurax-state`, `hyurax-chain`, `hyurax-store`, `hyurax-wire`, `hyurax-net`, todos iguais ao gabarito por vetor |
| Programa 1.0 | `hyurax-nucleo` (o núcleo, sem tela), `hyurax-interface` (a tela), `hyurax-app` (a janela), `hyurax-no` (terminal) e `hyurax-instalador`. Ver [`docs/ARQUITETURA.md`](docs/ARQUITETURA.md) |
| ULTRAX (trabalho útil) | oito motores, especificados em Python e iguais no Rust por vetor; CPU com limitador por linha e GPU (WebGL 2) conferida pela CPU. Ver [`docs/ULTRAX.md`](docs/ULTRAX.md) |
| Computação científica (JOBs) | unidades conferidas, checkpoint, retomada, relatório, verificação entre nós com compromisso assinado antes da revelação. Três núcleos completos pela rede P2P numa máquina (`tests/tres_nos.rs`); entre máquinas, fica para a testnet. Ver [`docs/COMPUTACAO-CIENTIFICA.md`](docs/COMPUTACAO-CIENTIFICA.md) e [`docs/REPUTACAO.md`](docs/REPUTACAO.md) |
| API externa (clientes) | contas com chave de acesso mandam JOBs a um nó, com limite de créditos de computação (não é dinheiro). Ver [`docs/API-EXTERNA.md`](docs/API-EXTERNA.md) |
| Segurança e operação | [`docs/MODELO-DE-AMEACAS.md`](docs/MODELO-DE-AMEACAS.md), [`docs/OPERACAO.md`](docs/OPERACAO.md) (saúde, cópias, incidentes) e a auditoria contra o Documento Mestre em [`docs/AUDITORIA-DOCUMENTO-MESTRE.md`](docs/AUDITORIA-DOCUMENTO-MESTRE.md) |
| Economia | para que o HYX é necessário (e para que não é) em [`docs/ECONOMIA.md`](docs/ECONOMIA.md); nada de venda, pré-venda ou promessa de valorização |
| Termos de uso | [`docs/TERMOS-DE-USO.md`](docs/TERMOS-DE-USO.md), versão 3, aceitos no instalador e na primeira abertura; revisados pelo projeto (lista de leis no começo do arquivo), **sem assinatura de advogado** |
| Assinatura digital do programa | não tem ainda; opções em [`docs/ASSINATURA-DIGITAL.md`](docs/ASSINATURA-DIGITAL.md) |
| Testnet pública | ensaiada com três processos; lançamento em [`docs/LANCAR-A-REDE.md`](docs/LANCAR-A-REDE.md); sementes em [`rede/sementes-testnet.txt`](rede/sementes-testnet.txt); kit do nó semente em [`docs/NO-SEMENTE.md`](docs/NO-SEMENTE.md); semente pública no ar desde 04/10/2026: `wss://hyurax-semente.onrender.com/p2p` (Render, grátis; saúde em https://hyurax-semente.onrender.com/saude) |
| Aplicativo do celular | carteira Android (Expo), chave só no aparelho, APK montado pelo GitHub. Ver [`docs/APP-CELULAR.md`](docs/APP-CELULAR.md) |
| Mainnet | não existe; o que falta e o que o autor decide em [`docs/MAINNET.md`](docs/MAINNET.md) |
| HYX Index, Hyurax Flux, Direct e Resonance | arquiteturas registradas em `docs/`; estado RED, nada implementado |
| Éter (transporte por qualquer meio) | núcleo em `hyurax-eter` (pasta de arquivos e memória); Bluetooth, LoRa e rádio projetados, não implementados. Ver [`docs/HYURAX-ETER.md`](docs/HYURAX-ETER.md) |

## Como está organizado

```
spec/          HYURAX-SPEC-01: as regras de consenso
reference/     implementação Python. Não é o nó; é o oráculo
vectors/       o contrato entre Python e Rust
crates/        o nó e o programa, em Rust
docs/          documentação (arquitetura, guias, termos, notas)
rede/          lista pública de sementes da testnet
scripts/       empacotamento, teste de instalação e preparo de ambiente
site/          o site do projeto
```

O Python **especifica**. O Rust **executa**. Os vetores **provam** que os dois
concordam. Um módulo só é considerado migrado quando cada vetor bate byte a
byte, não quando compila.

## Rodar

Referência Python, sem instalar nada além do numpy:

```bash
python reference/tests/run_all_tests.py
```

Rust (testes e clippy):

```bash
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
```

Os testes de integração da rede sobem nós de verdade em sockets e disputam CPU
com os testes de mineração, então ficam marcados como `ignored` e rodam sob
demanda, isolados (aperto de mão, sincronização, propagação, ataques e
auto-reanimação):

```bash
cargo test -p hyurax-net -- --ignored --test-threads=1
```

O programa sem janela, com a interface no navegador (http://127.0.0.1:8800):

```bash
cargo run -p hyurax-no -- painel --rede regtest --pasta ./teste
```

Programa e instalador para Windows, e o teste da instalação limpa (pasta
isolada, sem tocar na instalação de verdade):

```powershell
powershell -ExecutionPolicy Bypass -File scripts\empacotar-windows.ps1
powershell -ExecutionPolicy Bypass -File scripts\testar-instalacao.ps1 -Instalador dist\hyurax-instalador-windows-x86_64.exe
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
intervalo de bloco. O Argon2id continua como camada complementar. Os JOBs
do ULTRAX ficam fora do consenso: nenhum bloco depende de alguém publicar
tarefa. Limite atual, na spec (§9A): a prova viaja
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
nenhum material do projeto vai dizer que há. Enquanto o ULTRAX estiver fora do
consenso, nenhum material vai apresentá-lo como lastro econômico da moeda.
Enquanto um número for estimado, simulado ou pendente, a tela diz isso.

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
