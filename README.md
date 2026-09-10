# Auron

Blockchain com prova de trabalho, em desenvolvimento.

**Estado: pré-testnet. Não existe mainnet. Não existe token com valor.**
Qualquer pessoa que disser o contrário está mentindo.

## O que existe hoje

| Parte | Estado |
|---|---|
| Especificação `AURON-SPEC-01` | escrita, congelável quando a tokenomics fechar |
| Implementação de referência (Python) | completa, 77 testes |
| Vetores de validação cruzada | 11 arquivos |
| Nó de produção (Rust) | começado: `auron-types` |
| Rede P2P | não começada |
| Testnet pública | não começada |
| Mainnet | não existe |

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
. "D:\Nova pasta\auron\scripts\env.ps1"
```

## Decisões de projeto

**PoW Argon2id**, padrão RFC 9106. Memory-hard, sem ponto flutuante, roda em
CPU no CMD do Windows e no Termux do celular. Verificar custa exatamente uma
avaliação, sempre, independente da dificuldade.

**Assinatura Ed25519**, RFC 8032. Verificação em microssegundos, determinística
por construção, sem nonce aleatório que possa vazar a chave.

**Trabalho útil fora do consenso.** A segurança da cadeia vem do PoW
convencional. O Utrax é camada econômica de tarefas verificáveis. A cadeia não
precisa do Utrax para sobreviver, e é justamente isso que permite ao Utrax
evoluir sem colocar a moeda em risco.

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
