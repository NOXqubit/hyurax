# Reputação, Gold Score e nós verificados

Documento Mestre §11. Implementado em
`crates/hyurax-nucleo/src/ciencia/reputacao.rs`; o contador por parecer é
`crates/hyurax-ultrax/src/reputacao.rs`.

## De onde vêm os números

Só do que **este** nó conferiu. Cada entrega de um worker de outro nó passa
pela maioria e pela conferência local antes de virar parecer:

| Parecer | Quando | Conta como |
|---|---|---|
| Aceito | a entrega bate com a maioria e a conferência daqui aceitou | verificada |
| Recusado | a conferência daqui provou o erro, ou o worker assinou duas coisas que não fecham (compromisso × revelação), ou declarou operações acima do teto | recusada (com evidência) |
| Divergente | diferente da maioria, sem prova de erro | divergente (não pesa contra) |

Ninguém declara a própria reputação, e nenhum número vem de outro nó. A
reputação fica em `%LOCALAPPDATA%\Hyurax\ciencia\reputacao.txt` (um worker
por linha, com a primeira e a última data), gravada a cada mudança. O
teto é de 4096 workers; acima disso sai quem tem menos entregas.

## As fórmulas

```text
nota       = (verificadas + 1) / (verificadas + recusadas + 2)   em milésimos
volume     = min(verificadas, 200) / 200
Gold Score = nota × volume                                        0 a 1000
```

Worker novo tem nota 500 (sem histórico) e Gold Score 0: Gold mede
confiança acumulada, e sem volume não há confiança.

## Nó verificado

As quatro regras, todas medidas aqui:

1. visto pela primeira vez há pelo menos **90 dias** (a referência do
   documento);
2. pelo menos **100 unidades verificadas** por este nó;
3. no máximo **1%** de recusas entre as julgadas;
4. alguma entrega nos últimos **30 dias**.

Tempo de cadastro sozinho não basta (regra 2), e volume sem tempo também
não (regra 1). A tela mostra o que falta para cada worker.

## Limites, sem enfeite

- **Visão local.** Cada nó tem a sua opinião sobre cada worker. Não há
  reputação global nem troca de reputação entre nós (seria fácil de forjar).
- **Sybil.** Cada unidade verificada custa cálculo de verdade, refeito aqui,
  então fabricar mil identidades verificadas custa mil vezes o trabalho. Isso
  encarece, não impede (§12). Uma identidade com reputação boa pode ser
  vendida ou roubada (quem tem o `no.chave` dela).
- **Sem consequência econômica.** Gold Score e "verificado" ordenam a
  escolha de workers (melhores primeiro) e aparecem na tela. Não pagam nada
  e não dão direito a nada.

## Como comprovar

```bash
cargo test -p hyurax-nucleo --lib reputacao
```

e, com a computação entre nós ligada, a tabela de workers em Ciência → Rede.
