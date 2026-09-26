# HYX Index

Documento do autor, registrado em 26/09/2026. As notas do projeto vêm antes; o
texto do autor vem depois, fiel.

## Notas do projeto (26/09/2026)

### Estado

**RED: nada implementado.** Nenhum preço é buscado, nenhum valor do índice é
calculado ou publicado ainda.

### O que muda junto com este documento

O ticker da moeda passa de **HYUR** para **HYX**. É troca só de exibição: as
marcas de rede já eram `HYXM`, `HYXT` e `HYXR`, e nenhum rótulo que entra em
hash, assinatura ou gênese mudou. A rede de teste continua a mesma, e o arquivo
da cadeia em disco continua com a marca `HYURXDB1`, para não invalidar cadeias
já salvas.

O ticker HYX já aparece em alguns tokens pequenos no Ethereum (HyperX,
Hydrogen exchange), sem movimento relevante. Ticker não tem dono nem registro
central; o risco é só de confusão em buscas.

### O índice fica fora do consenso, sempre

Um nó não consegue conferir sozinho o preço do ouro. Se o valor do índice
entrasse numa regra de bloco, dois nós que lessem fontes diferentes, ou a
mesma fonte em momentos diferentes, discordariam, e a rede se partiria.
Por isso o índice é **informação calculada fora da cadeia**, como o mercado do
painel: nenhum bloco, transação ou recompensa depende dele.

### O que precisa ser decidido antes do primeiro valor

| Item | Proposta | Decide |
|---|---|---|
| Moeda de cálculo | Dólar, porque é a cotação comum dos sete | autor |
| Pesos | Iguais (1/7 cada) na largada, até existir regra pública de ajuste | autor |
| Base | Valor 1000 numa data fixa, publicada | autor |
| Frequência | Um valor por dia, no fechamento, porque boa parte das fontes gratuitas é diária | projeto |
| Rebalanceamento | Datas fixas e publicadas com antecedência; nunca no meio do mês por decisão avulsa | autor |
| Registro | Cada valor publicado guarda o preço de cada ativo, a fonte e a hora | projeto |

### O problema mais difícil: fonte de preço

"Fonte identificável" e "gratuita com direito de republicar" raramente são a
mesma fonte. Os preços de referência mais usados dos metais e do cobre são
licenciados pelos donos, e republicar o valor pode exigir contrato. Antes de
escolher cada fonte, é preciso conferir a licença dela. O que se sabe hoje,
a confirmar item a item:

- **Petróleo:** o governo dos EUA (EIA) publica diariamente o preço à vista do
  Brent e do WTI, como dado público.
- **Bitcoin:** há muitas fontes, e a média de várias corretoras reduz a
  dependência de uma só.
- **Ouro, prata, platina, paládio e cobre:** os preços de referência mais
  conhecidos têm licença. Falta achar fonte com direito de uso, ou pagar por
  ela.

Se um ativo ficar sem fonte num dia, o índice daquele dia sai marcado como
incompleto, em vez de inventar um preço.

### Jurídico

Não é aconselhamento jurídico. Como está escrito pelo autor (medida
informativa, sem lastro, sem resgate e sem promessa), o índice é informação.
Onde o risco reaparece é na **forma de mostrar**. O app não pode sugerir que o
HYX acompanha o índice, nem usar frases como "protegido por ouro" ou "economia
garantida". Comparar os dois lado a lado pede o aviso do próprio documento:
o valor do HYX não é o valor do índice.

A versão anterior da ideia tinha urânio e diamante. Saíram, e foi bom: o
comércio de minério nuclear é monopólio da União no Brasil (Constituição,
art. 177), e diamante não tem preço padrão.

### Primeira entrega possível

Um cálculo do índice no programa do PC e no app, a partir de fontes
conferidas, com histórico salvo e metodologia na tela. Vem depois da tela
Trabalho e do polimento do programa do PC, na ordem combinada.

---

## Texto do autor

O HYX Index é o motor econômico de referência do ecossistema HYX.

Ele não é o token, não representa propriedade sobre os ativos que compõem sua
cesta e não significa que cada HYX esteja lastreado ou seja resgatável por
esses ativos.

O token da rede é o HYX.

O HYX é um criptoativo próprio do protocolo, enquanto o HYX Index é um índice
econômico independente utilizado para acompanhar e medir o comportamento de uma
cesta diversificada de ativos e mercados de referência.

### Composição inicial do HYX Index

O índice será composto por sete referências:

1. Ouro
2. Bitcoin
3. Prata
4. Cobre
5. Platina
6. Paládio
7. Petróleo

A composição, os pesos e a metodologia poderão ser atualizados de acordo com
regras públicas e previamente definidas.

### Função do HYX Index

O objetivo do índice é fornecer uma referência econômica objetiva para o
ecossistema HYX.

O sistema poderá utilizar o índice para:

- acompanhar condições econômicas globais;
- comparar o comportamento do HYX com diferentes classes de ativos;
- gerar métricas econômicas dentro do aplicativo;
- auxiliar análises e modelos econômicos do protocolo;
- criar indicadores históricos;
- estudar relações entre o desempenho da rede e mercados externos.

O índice será calculado a partir de fontes de preços identificáveis, com
metodologia documentada, pesos definidos e registro histórico dos valores
utilizados.

### HYX ≠ HYX Index

**HYX:** é o token/ativo digital nativo do protocolo.

**HYX Index:** é um indicador econômico que mede uma cesta de referências
externas.

O valor do HYX não será automaticamente igual ao valor do HYX Index.

Da mesma forma, uma variação positiva ou negativa do índice não constitui
promessa de valorização ou proteção do HYX.

### Princípio fundamental

O HYX Index existe para medir e fornecer informação econômica, e não para
prometer que determinados ativos estão armazenados para garantir cada unidade
de HYX.

Em outras palavras:

> HYX é o ativo.
> HYX Index é o motor de referência econômica.

O índice transforma informações de diferentes mercados em uma única referência
mensurável que pode ser utilizada pelo ecossistema HYX.
