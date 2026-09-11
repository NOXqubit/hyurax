# Selos

O código do Auron carrega selos do autor. Cada arquivo de código (`.rs`, `.py`,
`.ps1`) começa com uma linha de comentário que contém uma mensagem cifrada.

O conteúdo das mensagens é do autor, e só quem tiver a senha dele consegue
lê-lo. Este documento existe para que qualquer pessoa, inclusive quem for
auditar o projeto, saiba exatamente o que essas linhas são e o que elas não
são.

## O que os selos não são

- **Não são código.** São comentários: o compilador do Rust e o interpretador
  do Python os ignoram. Eles não entram no programa compilado do nó, não mudam
  nenhuma regra de consenso e não mudam nenhum vetor de teste.
- **Não são porta dos fundos.** Nenhuma linha do nó lê os selos. Só a
  ferramenta `scripts/selar.py` os lê, e ela só roda quando alguém a executa
  à mão.

Para conferir: apagar a primeira linha de cada arquivo e rodar os testes dá
exatamente o mesmo resultado.

## Formato

```text
// AURON-SELO-v1 <nonce> <cifra> <etiqueta>
```

Em Python e PowerShell, o comentário começa com `#`. Os três campos estão em
hexadecimal.

- **Chave:** Argon2id (RFC 9106) sobre a senha, com o sal `AURON-SELO-v1|sal`,
  3 passadas, 16 MiB de memória, 1 faixa e 64 bytes de saída. A derivação é
  lenta de propósito, para a senha resistir a quem tentar adivinhá-la em
  massa.
- **Fluxo:** o XOF da AURON-SPEC-01 (SHA-512 em modo contador), com a chave
  como semente e `AURON-SELO-v1|fluxo|` seguido do nonce como domínio.
- **Cifra:** a mensagem em UTF-8, com XOR contra o fluxo.
- **Etiqueta:** os primeiros 16 bytes de
  `SHA-512("AURON-SELO-v1|etiqueta|" || chave || nonce || mensagem)`. Ela
  depende da chave, então sem a senha ninguém consegue testar textos
  candidatos contra ela.

Verificador da senha: ainda não gravado.

O verificador são os primeiros 16 bytes de
`SHA-512("AURON-SELO-v1|verificador|" || chave)`. Ele só serve para a
ferramenta recusar uma senha diferente da usada nos selos que já existem; não
revela a senha nem a chave.

## Como conferir, quando a senha for revelada

```text
python scripts/selar.py --ler
```

A ferramenta pede a senha, mostra a mensagem de cada arquivo e confere a
etiqueta. Uma etiqueta que confere prova que o texto é o que o autor selou.
