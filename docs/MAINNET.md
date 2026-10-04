# Preparação da mainnet

Escrito em 04/10/2026. O roteiro está em [ROTEIRO-LANCAMENTO.md](ROTEIRO-LANCAMENTO.md),
com prazo de **13/12/2026**. Este documento diz o que falta, medido no código
e na rede, e o que só o autor pode decidir.

> A regra continua a mesma: a mainnet **só sai com os critérios cumpridos**.
> Mudar a data é aceitável; lançar com falha conhecida não é. Enquanto ela
> não sai, a linha de comando recusa `mainnet` de propósito, e o HYX não tem
> valor, não é vendido e não tem promessa de ganho.

## Onde estamos (04/10/2026)

| Critério do roteiro | Estado | Como foi medido |
|---|---|---|
| Testnet pública rodando 4 semanas sem falha grave | **não começou** | `rede/sementes-testnet.txt` não tem nenhuma semente pública; não há nó respondendo de fora |
| Nenhuma falha de consenso sem correção e teste | cumprido até aqui | nenhuma falha aberta; os ataques conhecidos têm teste em `crates/hyurax-net/tests` |
| Cifra da conexão e carteira com senha em uso | cumprido | Noise XX desde o protocolo v2; carteira `HYURAX-CARTEIRA-v2` (Argon2id + ChaCha20-Poly1305), no PC e no celular |
| Pelo menos 5 nós de pessoas diferentes | **0 de 5** | depende da testnet pública |
| Python e Rust concordando em todos os vetores | cumprido | 29 arquivos em `vectors/`, conferidos pelos dois lados (e pelo app do celular nos de endereço, transação e carteira) |
| Spec congelada, com o hash publicado | **não** | recompensa e halving ainda marcados PROVISÓRIO (`consensus.py:150`, `crates/hyurax-consensus/src/lib.rs:62`) |
| Aviso público de que HYX não é investimento | cumprido | README, termos de uso v3, programa e app |

**O que trava tudo é a semente pública.** Sem ela não há testnet pública, e
sem as 4 semanas de testnet pública a mainnet não sai. Contando de hoje, a
data mais cedo possível para a mainnet fica por volta de **meados de
novembro**, e só se a semente subir já e aparecerem nós de fora. O 13/12
continua possível, mas está em risco.

## Decisões que só o autor toma

1. **Onde roda a semente pública.** A Oracle pede cartão. O PC de casa
   expõe o IP de casa (e o anonimato). Outras saídas: uma VPS barata paga
   sem cartão (Pix ou boleto), ou um amigo de confiança com porta aberta. O
   kit já está pronto (`deploy/instalar-semente.sh`, `docs/NO-SEMENTE.md`).
2. **A emissão.** Hoje: 50 HYX por bloco, halving a cada 210.000 blocos,
   bloco a cada 2 minutos (oferta total ≈ 21 milhões). Precisa ser
   confirmada, ou trocada, **antes** de congelar a spec. Mudar depois da
   gênese é bifurcação.
3. **Premine: nenhum (recomendado).** Qualquer reserva para o autor, para
   o projeto ou para venda vira, na prática, oferta de token. Com a regra do
   projeto (sem venda, sem promessa), o mais limpo é começar do bloco 1 como
   qualquer minerador.
4. **A mensagem da gênese** (o texto que fica para sempre no primeiro bloco).
5. **A data**, depois das 4 semanas de testnet pública.

## O que é trabalho técnico (e está aqui para ser feito)

| Item | Estado |
|---|---|
| Parâmetros da mainnet no código (`ParametrosRede::MAINNET`, magic `HYXM`, endereços `hyx1…`) | existem, iguais no Python e no Rust |
| Endereço com a rede no prefixo; endereço de uma rede recusado na outra | feito, com vetor (`vectors/enderecos.json`) |
| Assinatura presa à rede (o magic entra na mensagem assinada) | feito desde a SPEC-01: transação de teste nunca vale na principal |
| Carteira do celular falando com nós (`/api/v1/leve/`) | feito em 04/10/2026; ver [APP-CELULAR.md](APP-CELULAR.md) |
| Gênese da mainnet | **falta**: gerar com o mesmo código da testnet (`vectors/genesis.json`) quando a mensagem e a data estiverem decididas |
| Congelar a spec | **falta**: tirar o PROVISÓRIO, gerar os vetores finais, publicar o SHA-512 da spec e do `MANIFEST.json` |
| Textos que dizem "rede de teste" | trocar por rede quando a mainnet existir (programa, app, cabeçalho do arquivo de carteira). O selo de teste do app já depende da rede do nó |
| Liberar `mainnet` na linha de comando | **por último**, num commit próprio, depois de tudo acima |
| Assinatura digital do programa (Windows) | depende do certificado (`docs/ASSINATURA-DIGITAL.md`) |
| Doação | o endereço é de corretora; antes da mainnet, trocar por uma carteira em que só o autor tenha a chave |
| Revisão de segurança de fora | recomendada antes do lançamento: chamar a temporada de ataques na testnet pública (semanas 7 a 9 do roteiro) |

## O que fica de fora da mainnet, de propósito

- **Dinheiro no mercado de máquinas.** Os preços seguem em créditos de
  computação, que não são HYX nem dinheiro.
- **Trabalho científico na recompensa do bloco.** Precisa da SPEC-02, que
  ainda não existe.
- **Stablecoins (Flux).** É mudança de consenso e assunto regulado; ver as
  notas em [HYURAX-FLUX.md](HYURAX-FLUX.md).

## Riscos jurídicos (não é aconselhamento jurídico)

- No Brasil, quem **presta serviço** com ativo virtual (troca, custódia,
  intermediação) precisa de autorização do Banco Central (Lei 14.478/2022 e
  as regras de PSAV em vigor desde 02/02/2026). Software aberto que cada um
  roda no próprio computador, sem custódia e sem venda, é outra coisa, mas
  o limite é fino. Antes de qualquer serviço que toque em HYX de terceiros,
  consultar um advogado.
- Oferecer HYX com promessa de valorização pode ser oferta de valor
  mobiliário (CVM). A regra do projeto já proíbe isso: nada de venda,
  pré-venda, "invista" ou "vai valorizar".
- O anonimato do autor não protege de responsabilidade se algo acima for
  feito. Ele protege a pessoa, não a atividade.

## Ordem proposta

1. Semente pública no ar (decisão 1).
2. Anunciar a testnet pública e o app do celular; buscar 5 nós de fora.
3. Temporada de ataques; corrigir com teste o que aparecer.
4. Decisões 2 a 4; congelar a spec; gerar a gênese.
5. Quatro semanas de testnet estável depois do congelamento.
6. Decisão 5 e lançamento: liberar `mainnet` na linha de comando, publicar
   o programa e o app com os textos da rede principal.
