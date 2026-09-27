# Assinatura digital do programa para Windows

Pesquisa de 27/09/2026.

## O problema

Sem assinatura, o Windows mostra "O Windows protegeu o computador" ao abrir o
instalador. Quem baixa tem de clicar em *Mais informações* e depois em
*Executar assim mesmo*.

## Três coisas que não dá para contornar

1. **O certificado é pessoal.** A autoridade certificadora confere a
   identidade de uma pessoa ou de uma empresa, com documento e, em geral,
   selfie ou vídeo. Isso só o dono do projeto pode fazer; ninguém faz por ele.
2. **Desde junho de 2023, a chave fica em hardware seguro:** num cartão ou
   token, ou num HSM na nuvem.
   - Não existe mais certificado em arquivo `.pfx` para colar num segredo do
     GitHub.
   - Na integração contínua, a assinatura passa por um serviço na nuvem.
3. **Assinar não some com o aviso na hora.** O SmartScreen olha a reputação
   do certificado e do arquivo, que cresce com os downloads. Desde 2024, nem
   o certificado EV dá reputação imediata. O ganho imediato é outro: o aviso
   passa a mostrar o **nome de quem publicou**, em vez de "editor
   desconhecido", e o arquivo não pode ser alterado sem quebrar a
   assinatura.

## As opções para uma pessoa no Brasil

| Opção | Custo | Serve para o Hyurax? |
|---|---|---|
| **Certum "Open Source Code Signing"**, na nuvem (SimplySign) | cerca de US$ 50 a 70 no primeiro ano; renovação por volta de € 29 | **Sim, é a recomendada.** É feita para desenvolvedor de software livre pessoa física e funciona no GitHub Actions |
| **SignPath Foundation** | grátis para software livre | **Depois.** Pede "reputação verificável" do projeto, uma política de assinatura publicada no site e aprovação manual a cada versão. Um minerador pode ser lido como "programa potencialmente indesejado" |
| **Azure Artifact Signing** (Microsoft) | US$ 9,99 por mês | **Não.** Pessoa física só nos EUA e no Canadá, e empresa só nos EUA, Canadá, União Europeia ou Reino Unido |
| Certificado comum (OV) de outra autoridade | US$ 200 a 400 por ano, mais o token | Funciona, mas custa mais que o Certum |
| Certificado feito por nós mesmos | grátis | **Não.** O Windows não confia nele, e o aviso fica igual |

**Atenção:** o certificado Certum de software livre mostra o **nome civil**
do dono, no formato "Open Source Developer, Fulano de Tal". Qualquer pessoa
que abrir as propriedades do arquivo vê esse nome.

## O passo a passo (Certum)

**O que só o dono do projeto pode fazer:**

1. Comprar o "Open Source Code Signing in the Cloud" na loja da Certum.
2. Fazer a verificação de identidade:
   - com documento com foto e vídeo, pelo serviço que a Certum indicar;
   - mandando um comprovante de que o projeto é de código aberto (o link do
     repositório e a licença MIT ou Apache-2.0).
3. Ativar o SimplySign. Na ativação, a Certum mostra um **segredo TOTP**, o
   mesmo tipo de código do Google Authenticator. Guarde esse segredo.
4. No GitHub, em *Settings → Environments*, criar um ambiente
   `assinatura`, com aprovação obrigatória. Nele, criar os segredos:
   - `CERTUM_USUARIO`: o e-mail da conta;
   - `CERTUM_TOTP`: o segredo TOTP.

**Depois disso, o projeto:**

- liga a assinatura no `lancamento.yml`, num trabalho separado que só
  recebe os arquivos já compilados. Nenhuma ferramenta de compilação chega
  perto do segredo;
- assina o `Hyurax.exe` **antes** de montar o instalador, porque o
  instalador leva o `.exe` dentro, e depois assina o próprio instalador;
- testa numa etiqueta de ensaio antes da versão de verdade.

**Por que esperar o certificado para ligar:**

- as duas ações prontas que usam o SimplySign no GitHub Actions têm poucos
  dias de vida e quase nenhum uso. Entregar a elas o segredo TOTP, que
  permite assinar em nome do dono, pede conferência do código e fixação
  por commit;
- sem a conta de verdade, isso não se testa.

## Fontes

- [Azure Artifact Signing: perguntas frequentes](https://learn.microsoft.com/en-us/azure/artifact-signing/faq)
- [Azure Artifact Signing: preço](https://azure.microsoft.com/en-us/pricing/details/artifact-signing/)
- [Disponibilidade fora dos EUA e do Canadá (issue #81)](https://github.com/Azure/artifact-signing-action/issues/81)
- [SignPath Foundation: termos](https://signpath.org/terms)
- [Certum Open Source Code Signing na nuvem](https://certum.store/open-source-code-signing-on-simplysign.html)
- [Relato de um desenvolvedor com o certificado Certum de software livre (2025)](https://piers.rocks/2025/10/30/certum-open-source-code-sign.html)
- [Ação `certum-cloud-code-sign`](https://github.com/jay0lee/certum-cloud-code-sign)
