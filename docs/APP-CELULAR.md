# Aplicativo do celular (carteira)

Código em [`mobile/`](../mobile). Expo SDK 57 (React Native 0.86) e
TypeScript. O celular é **só a carteira**: ele não minera nem guarda a cadeia.

> Rede de **teste**: o HYX não tem valor.

## Como funciona

```
  celular                                   nó Hyurax (PC ou semente)
  ───────                                   ─────────────────────────
  chave no aparelho (Keystore)
  monta e ASSINA a transação   ── bytes ──▶  /api/v1/leve/transacao
                                             a validação do nó confere
  saldo, nonce, histórico      ◀── JSON ──   /api/v1/leve/conta, /historico
```

- **A chave nunca sai do aparelho.** O nó recebe a transação pronta e a
  confere como confere qualquer outra (assinatura, nonce, saldo, magic da
  rede).
- **Onde a chave fica:** no SecureStore (no Android, cifrado pelo
  Keystore). Ela é guardada sem exigir biometria no armazenamento, porque
  trocar a digital invalidaria a chave e o saldo ficaria preso. Quem
  protege o uso é a confirmação do aparelho (digital, rosto ou PIN), pedida
  antes de cada envio, antes de mostrar a cópia de segurança e antes de
  tirar a carteira.
- **Cópia de segurança:** o arquivo `HYURAX-CARTEIRA-v2`, o mesmo do
  programa do PC, cifrado com a senha (Argon2id 64 MiB + ChaCha20-Poly1305).
  Uma carteira criada no celular abre no PC, e uma do PC abre no celular.

## Conferido contra os vetores

O núcleo do app (`mobile/src/nucleo`) não é uma "versão parecida": ele é
conferido byte a byte contra os mesmos arquivos do Python e do Rust
(`npm run testar`, também no GitHub a cada push).

| O quê | Contra |
|---|---|
| Endereço Bech32m, recusas e mensagens | `vectors/enderecos.json` |
| Chave pública e endereço | `vectors/crypto_ed25519.json` |
| Transferência: mensagem assinada, assinatura, bytes e txid | `vectors/transactions.json` |
| Arquivo de carteira (abrir e escrever igual) | `mobile/teste/dados/carteira-v2.txt`, gerado e conferido pelo Rust |

E de ponta a ponta, em 04/10/2026: um nó regtest local, o app importando a
carteira de gabarito, envio de 12,5 HYX assinado no app, aceito pelo nó e
minerado no bloco 70.

## A API das carteiras leves (no nó)

Desligada por padrão. No PC: **Ajustes → Carteiras de celular**. Na linha
de comando: `hyurax-no painel --carteiras`. Ligada, o nó aceita conexões de
fora só nestas rotas (além do que já estiver ligado).

| Método | Rota | O que faz |
|---|---|---|
| GET | `/api/v1/leve/info` | rede, prefixo, magic, altura, maturidade, taxa sugerida |
| GET | `/api/v1/leve/conta/ENDERECO` | saldo gastável e imaturo (em unidades) e o próximo nonce |
| GET | `/api/v1/leve/historico/ENDERECO` | os últimos 50 movimentos |
| POST | `/api/v1/leve/transacao` | corpo: a transação em hexadecimal; devolve o txid |

Sem chave de acesso: é o que a cadeia já mostra a todos, e a transação
chega assinada. Limite de 60 pedidos por minuto por IP. As respostas levam
`Access-Control-Allow-Origin: *` (pedidos simples, sem cookie). A recusa
diz o motivo: nonce já usado, nonce adiantado ou saldo insuficiente.

**Risco:** a conexão é HTTP, sem TLS (o projeto não usa biblioteca de TLS
em C). Na rede de casa, tudo bem. Pela internet, a privacidade de qual
endereço é seu fica exposta a quem estiver no caminho, embora ninguém
consiga gastar nem alterar a transação sem a chave.

## Instalar no celular (Android)

O GitHub monta o APK sozinho (`.github/workflows/app-celular.yml`), sem
Android Studio e sem conta na Expo. As versões ficam nas Releases com a
etiqueta `app-v…`, ao lado da soma SHA-256.

1. Baixe o `.apk` da Release mais nova `app-v…` no celular.
2. Permita "instalar apps desconhecidos" para o navegador, quando ele pedir.
3. No PC: Ajustes → Carteiras de celular. No app: Ajustes → endereço do PC
   (o IP dele no Wi-Fi, por exemplo `192.168.0.10`).

O APK sai assinado com uma chave de depuração gerada a cada montagem. Para
atualizar, desinstale a versão anterior (guarde antes a cópia de segurança).
A chave de lançamento do app entra como segredo do repositório quando ele
for para a Play Store.

## Desenvolver

```bash
npm ci
```

```bash
npm run testar
```

```bash
npm run tipos
```

Ver as telas no navegador (só para desenvolvimento; ali a carteira fica em
`sessionStorage`, nunca use com saldo de verdade):

```bash
npx expo start --web
```

## O que falta

- iOS: o código é o mesmo, mas montar para iPhone exige conta da Apple.
- Assinatura de lançamento e Play Store.
- Vários nós ao mesmo tempo (hoje é um nó escolhido pela pessoa); entra
  quando houver sementes públicas.
- Notificação de recebimento.
