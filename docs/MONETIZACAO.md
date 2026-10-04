# Monetização: planos, preços e como a cobrança funciona

Escrito em 04/10/2026, a partir da seção 3 do documento "Arquitetura Mestre:
Monetização + Cloud Design 2.0" e das regras de [ECONOMIA.md](ECONOMIA.md).

> **Regra que não muda:** o projeto vende **serviço**, em **reais**, por uma
> empresa com CNPJ. Não vende HYX, não faz pré-venda, não promete
> valorização nem rendimento. Créditos de computação são unidade de serviço
> (como "minutos" de um plano de celular): não são HYX, não são dinheiro, não
> se transferem entre pessoas e não se trocam de volta por reais.

## A tese

Demanda real por computação e armazenamento → capacidade distribuída →
serviço medido → receita → remuneração de quem fornece a capacidade →
margem da plataforma. O HYX não é a fonte da demanda; a utilidade nasce do
serviço.

O que já existe no programa e sustenta isso:

| Peça | Onde | Estado |
|---|---|---|
| Medição de consumo (créditos) | `hyurax-ultrax/src/job.rs` | funciona |
| Orçamento por JOB e por conta | `ciencia`, `contas` | funciona |
| Recibo assinado de aluguel | `hyurax-nuvem/src/recibo.rs` | funciona |
| Livro de contas com divisão fornecedor / plataforma / reserva (80/15/5) | `hyurax-nuvem/src/livro.rs` | funciona |
| Faturas por mês (em créditos) | tela Faturamento | funciona |
| API para clientes | `/api/v1/externa/` | funciona |
| Planos e ativação por voucher assinado | `hyurax-nuvem/src/plano.rs`, tela Planos | **nesta etapa** |
| Cobrança em reais (checkout, nota fiscal) | — | depende do dono (ver Fase M1) |

## Os planos

Os preços são a proposta inicial. Todos ficam num lugar só
(`hyurax-nuvem/src/plano.rs`, o catálogo) e podem mudar antes da venda abrir.

| | **Comunidade** | **Pro** | **Equipe** | **Empresa** |
|---|---|---|---|---|
| Preço | **grátis** | **R$ 29/mês** ou R$ 290/ano | **R$ 149/mês** ou R$ 1.490/ano | **a partir de R$ 990/mês**, contrato |
| Para quem | quem quer testar, minerar, guardar arquivos com quem conhece | pesquisador, estudante, criador | laboratório, startup, time pequeno | empresa com exigência de contrato |
| Programa completo (nó, carteira, mineração, ULTRAX, mercado, nuvem P2P) | ✔ | ✔ | ✔ | ✔ |
| Créditos por mês na **capacidade gerenciada** | — | 150 mil (≈ 20 a 40 h de CPU) | 1 milhão (≈ 140 a 280 h de CPU) | sob medida |
| Armazenamento **gerenciado** (cifrado, 3 guardiões) | — | 50 GB | 500 GB | sob medida |
| Comissão da plataforma quando **você** aluga sua máquina no mercado | 15% | 10% | 8% | negociada |
| Chaves da API externa | as do seu nó | as do seu nó | +10 contas gerenciadas | ilimitadas |
| Prioridade na fila da capacidade gerenciada | — | ✔ | ✔ alta | dedicada |
| Relatórios (JSON, CSV, PDF) | ✔ | ✔ | ✔ + histórico de 1 ano | ✔ + auditoria |
| Suporte | comunidade | e-mail, 2 dias úteis | e-mail, 1 dia útil | canal dedicado + SLA |
| Nota fiscal | — | ✔ | ✔ | ✔ |

**Avulsos** (para qualquer plano):

| Item | Preço proposto |
|---|---|
| Pacote de créditos | R$ 5 por 100 mil · R$ 45 por 1 milhão · R$ 400 por 10 milhões |
| Armazenamento gerenciado extra | R$ 0,20 por GB por mês |
| Comissão padrão do mercado de máquinas | 15% da plataforma + 5% de reserva; 80% para o dono da máquina |

### De onde vêm os números

- **1 crédito = 1.000 milicréditos = 1 bilhão de operações medidas**, o que
  dá de meio a um segundo de um núcleo atual, conforme o motor. 1 hora de
  CPU ≈ 3.600 a 7.200 créditos; por isso "150 mil créditos ≈ 20 a 40 h".
- **Créditos avulsos** a R$ 5 por 100 mil saem a R$ 0,18 a R$ 0,36 por
  hora de CPU: abaixo das nuvens grandes (perto de R$ 0,40 por vCPU-hora), com
  margem bruta de 15 a 30% sobre o custo de uma VPS pequena, que é a faixa
  do documento mestre.
- **Armazenamento** a R$ 0,20 por GB-mês, com três guardiões e código
  Reed-Solomon, fica perto do preço de nuvem comum, com a vantagem de o
  operador nunca ver o conteúdo (cifrado antes de sair).
- **A divisão do mercado** (80/15/5) é a que o livro de contas já faz.

### O que "capacidade gerenciada" quer dizer

É a parte que custa dinheiro ao projeto e, por isso, é a parte paga:
máquinas que o próprio projeto mantém (ou contrata) sempre ligadas, com
prazo e redundância garantidos. Tudo o que é P2P entre pessoas, como a
nuvem entre conhecidos, o mercado e a mineração, continua grátis no
programa, e o código continua aberto. Ninguém precisa pagar para usar a rede.

## Como a cobrança funciona

```
 cliente paga em reais (Pix/cartão, provedor de pagamento da empresa)
        │
        ▼
 o provedor avisa o serviço central (webhook)          ← Fase M2
        │
        ▼
 o serviço central emite um VOUCHER DE PLANO assinado  ← emissão manual na M1
        │   (plano, para qual nó, de quando a quando, créditos, armazenamento,
        │    comissão; assinado pela chave de planos do projeto)
        ▼
 o programa do cliente confere a assinatura e ativa o plano
        │
        ▼
 a capacidade gerenciada confere o mesmo voucher antes de atender
```

**Por que um voucher assinado, e não uma conta num servidor:** o programa
confere sozinho, sem perguntar a ninguém (o mesmo modelo da atualização
assinada). Não há senha, cadastro nem dado pessoal no voucher: só a chave
pública do nó (o "worker") que recebe o plano. A chave que assina os
vouchers fica fora do repositório, como a chave de lançamento.

**O software nunca toca no dinheiro do cliente.** O pagamento passa
inteiro pelo provedor de pagamento da empresa. O programa só vê o voucher.

## Fases

| Fase | O que entra | Quem faz | Estado |
|---|---|---|---|
| **M0** | Catálogo de planos, voucher assinado (especificado em Python, igual no Rust por vetor), ativação no programa, tela Planos, preços no site marcados "em breve", este documento | o projeto | **feita nesta etapa** |
| **M1** | Primeira venda: link de pagamento do provedor, voucher emitido à mão com `hyurax-no planos emitir`, capacidade gerenciada inicial (1 ou 2 servidores pagos) | o dono + o projeto | precisa dos itens da lista abaixo |
| **M2** | Serviço central: checkout, webhook do provedor, emissão automática do voucher, nota fiscal pela integração do provedor | o projeto | depois da M1 validada |
| **M3** | Repasse em reais a quem aluga máquina (split de pagamento do provedor) | o projeto, com parecer jurídico | depois da M2 |
| **M4** | Empresa: contrato, SLA, rede privada, auditoria | o dono | sob demanda |

### Antes da primeira venda (Fase M1): só o dono pode fazer

1. **Atividade do CNPJ.** Conferir com o contador se as atividades (CNAE)
   cobrem o serviço: por exemplo 6311-9/00 (processamento de dados,
   hospedagem) e 6203-1/00 (software não customizável). MEI não cobre
   desenvolvimento de software.
2. **Conta PJ e provedor de pagamento** (Asaas, Mercado Pago, Pagar.me etc.):
   Pix e cartão, com nota fiscal de serviço integrada.
3. **Contador:** regime tributário e emissão de NFS-e.
4. **Termos de serviço e política de reembolso** revisados por advogado. Na
   venda pela internet vale o direito de arrependimento de 7 dias (CDC,
   art. 49).
5. **Capacidade gerenciada:** pelo menos um servidor pago e sempre ligado.
   O plano grátis do Render dorme e não aguenta compromisso de prazo.
6. **Anonimato:** ver abaixo.

### Anonimato e venda

Vender pela internet obriga a mostrar na página o **CNPJ e o endereço** da
empresa (Decreto 7.962/2013), e o cadastro público do CNPJ mostra quem são
os sócios. Ou seja, **abrir a venda encerra o anonimato do projeto** naquela
ponta. O número do CNPJ não foi posto em nenhum arquivo público: entra na
página de preços e nos termos só quando o dono decidir abrir a venda.

### O que fica de fora, de propósito

- Pagar ou cobrar em HYX; converter créditos em HYX ou em reais.
- Rendimento, "ganhe dinheiro minerando", qualquer promessa de retorno.
- Repasse em reais a operadores antes do parecer jurídico (Fase M3):
  intermediar pagamento entre terceiros pode exigir autorização do Banco
  Central, e o split do provedor existe justamente para isso.
- Datasets, modelos de IA e renderização (linhas do documento mestre): ficam
  para quando a capacidade gerenciada estiver vendendo.

## Metas honestas para os 6 primeiros meses de venda

Sem previsão de faturamento: o cenário de R$ 19,9 milhões por mês do
documento mestre supõe 20 mil clientes pagantes e é ilustração, não meta.
O que vale medir no começo:

| Indicador | Meta |
|---|---|
| Clientes Pro pagantes | 20 |
| Clientes Equipe | 3 |
| Custo da capacidade gerenciada coberto pela receita | sim, a partir do 3º mês |
| Reembolsos | menos de 5% |
| Incidentes com perda de arquivo | zero |
