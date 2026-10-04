# Economia do Hyurax / Ultrax: HYX, créditos e o que ainda não existe

Fase 9 do [Documento Mestre](DOCUMENTO-MESTRE.md) (§8, §9, §10, §14). Escrito
contra o código de 03/10/2026. Os números vêm de `hyurax-consensus`,
`hyurax-types` e `hyurax-ultrax/src/job.rs`; nada aqui é promessa.

> **Não há dinheiro de verdade em lugar nenhum do projeto.** A rede é de
> teste. HYX de teste não vale nada, não é vendido, não tem pré-venda e não
> tem liquidez. Este texto não é aconselhamento jurídico nem financeiro.

## Para que o HYX é necessário, função por função

O §8 pede que o projeto explique por que o HYX é necessário em cada função
e que não crie demanda artificial por ele. A resposta, uma a uma:

| Função | Usa HYX hoje? | É necessário? | Por quê |
|---|---|---|---|
| Taxa de transação | sim | **sim** | Sem custo por transação, qualquer um enche os blocos de lixo. A taxa é paga no ativo da própria cadeia porque é o único que todo nó sabe conferir sem confiar em ninguém. |
| Recompensa de quem minera | sim | **sim** | A emissão nova paga quem gasta energia no Argon2id que protege o registro de blocos. É o mecanismo de consenso, não um produto. |
| Pagamento por computação científica | **não** | **ainda não se sabe** | A contabilidade usa **créditos de computação** (milicréditos), que não são HYX. Créditos resolvem a conta interna de um JOB sem token nenhum. HYX só seria justificável para liquidar entre partes que não confiam umas nas outras e não têm um intermediário comum; enquanto isso não for demonstrado, a liquidação fica desligada (`Liquidado::NaoLiquidado`). |
| Recompensa de quem calcula unidades científicas | **não** | **não por enquanto** | O trabalho científico não muda a recompensa do bloco (§13: ciência e consenso separados). Integrar os dois é mudança de consenso, que exigiria uma SPEC-02 publicada. |
| Marketplace de capacidade | **não** | **não por enquanto** | A troca de unidades entre nós existe, sem preço. Um preço em HYX amarraria o custo de energia do operador à cotação de um token sem mercado. |
| HYX Index | **não** | **não** | Índice de acompanhamento, sem lastro, custódia nem resgate ([HYX-INDEX.md](HYX-INDEX.md)). |
| Créditos, assinaturas, planos | **não** | **não** | Cobrança de serviço é em moeda corrente, por empresa com CNPJ; ver "Cobrança" abaixo. |

Conclusão honesta: hoje o HYX é necessário **só** para o que toda cadeia
precisa (taxa e recompensa de consenso). Todo o resto funciona, e funciona
melhor, sem token. Isso continua assim até alguém demonstrar, com o
problema real na mão, uma função em que o HYX resolva o que créditos não
resolvem.

## Emissão (tokenomics)

| Parâmetro | Valor | Onde |
|---|---|---|
| Teto de emissão | 21.000.000 HYX | `hyurax-types::MAX_SUPPLY` |
| Casas decimais | 8 | `HYX_UNIT` |
| Recompensa inicial | 50 HYX por bloco | `ParametrosRede::MAINNET.initial_reward` |
| Intervalo alvo | 120 s | `target_spacing` |
| Halving | a cada 210.000 blocos (PROVISÓRIO) | `halving_interval` |
| Maturidade da recompensa | 100 blocos (teste: 20) | `coinbase_maturity` |
| Pré-mineração | **nenhuma**: a gênese emite 1 unidade mínima (0,00000001 HYX) para um endereço de zeros, que ninguém gasta | `make_genesis` |

**Risco que precisa ser decidido antes de qualquer rede principal:** com
blocos de 120 s, 210.000 blocos são uns **292 dias**. Metade de tudo o que
existirá sai no primeiro ano, e 7/8 em menos de três. É uma emissão muito
concentrada no começo: quem chegar cedo recebe quase tudo. O valor está
marcado PROVISÓRIO no código; mudar é mudança de consenso, com novos
vetores e nova SPEC.

## Créditos de computação (o que existe)

- `milicreditos = (operações executadas + operações de conferência) / 1.000.000`
  (`hyurax-ultrax/src/job.rs`). Tempo, memória e rede ficam de fora da v1:
  contar tempo premiaria a máquina lenta.
- Cada JOB tem orçamento em milicréditos; acabado o orçamento, o JOB para
  em "sem orçamento" e fecha.
- O consumo de cada unidade fica no registro do JOB (`unidades.jsonl`), e o
  relatório soma. É a **contabilidade e o histórico de operações** do §8.
- Worker remoto não declara tempo: só conta o que foi conferido aqui
  (`ms_calculo` 0 para remotos).

## O que não existe, e o que cada coisa exige

| Item do documento | Estado | O que falta para existir |
|---|---|---|
| Cobrança (por computação, assinatura, plano) | PARCIAL (04/10/2026) | Planos, preços e voucher assinado prontos ([MONETIZACAO.md](MONETIZACAO.md), Fase M0). Falta a Fase M1: conta PJ, provedor de pagamento, contador e termos comerciais. O software nunca toca no dinheiro do cliente. |
| Preços e estimativas de custo | PARCIAL | A estimativa em créditos existe antes de submeter (operações previstas pelo modelo de custo); preço em moeda depende da cobrança. |
| Liquidação em HYX | AUSENTE de propósito | Demonstrar a necessidade (tabela acima); SPEC-02; estrutura jurídica. |
| Liquidez, listagem, venda | AUSENTE de propósito | Nada disso entra antes de rede principal, auditoria externa e parecer jurídico. |
| Disputas do marketplace | PARCIAL | Disputa técnica existe (maioria, conferência local, evidência em `consenso.jsonl`); disputa comercial depende de contrato. |

## Regulação (Brasil), sem aconselhamento jurídico

- A Lei 14.478/2022 dá ao Banco Central a autorização e a supervisão das
  prestadoras de serviços de ativos virtuais, e à CVM os tokens que forem
  valor mobiliário.
- Oferecer um token com promessa de valorização ou de rendimento pelo
  esforço de terceiros pode configurar oferta de valor mobiliário. Por isso
  o projeto não promete rentabilidade a operadores nem valorização a quem
  tem HYX (§10, §22), e os termos de uso dizem isso.
- Stablecoin, câmbio e remessa têm regras próprias do Banco Central; nenhuma
  função do Hyurax toca nisso.
- Qualquer fase com dinheiro real exige estrutura jurídica e autorização, ou
  parceiro autorizado. Até lá: só rede de teste.

## Como comprovar

- Emissão: `cargo test -p hyurax-consensus` (vetores do gabarito Python,
  `cumulative_emission` nunca passa do teto).
- Créditos: `cargo test -p hyurax-ultrax` (consumo e orçamento) e o relatório
  de qualquer JOB concluído.
- Nenhuma promessa de rendimento: busca por "rentab", "lucro", "valoriza" em
  `README.md`, `site/`, `docs/` e na interface.
