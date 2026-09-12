# Auron — o que é, qual é o foco e o que nós não prometemos

Documento de apresentação do projeto, escrito em 11/09/2026.

---

## 1. Em uma frase

O Auron é uma criptomoeda em desenvolvimento que quer usar o poder de cálculo
dos computadores da rede para resolver problemas reais — em ciência,
medicina, agricultura e inteligência artificial — e não para especulação.

---

## 2. O que é o Auron

O Auron é um projeto de pesquisa e desenvolvimento, chamado de **Auron
Technology**, que está construindo:

- **uma moeda digital com rede própria**, com regras abertas e escritas,
  que qualquer pessoa pode conferir;
- **um mercado de trabalho computacional**, o UTRAX: quem precisa de cálculo
  paga, e quem tem computador executa e recebe.

**Dito com clareza:**

- O projeto ainda não tem empresa constituída.
- A rede pública ainda não existe.
- A moeda não tem valor de mercado.

Tudo o que existe hoje é tecnologia em construção e em teste. A seção 7 diz
exatamente o que está pronto.

---

## 3. O foco: poder de cálculo que serve para alguma coisa

### O problema

Em muitas criptomoedas, os computadores da rede fazem bilhões de cálculos por
segundo só para provar que trabalharam. Esse trabalho protege a rede, mas o
resultado dele não serve para mais nada. É muita capacidade de cálculo jogada
fora.

### A proposta do Auron

Usar essa mesma capacidade de cálculo para resolver problemas que precisam
dela de verdade. Alguns exemplos do que se quer atender:

- **Melhoramento genético e cruzamento de espécies.** Escolher, entre milhões
  de combinações possíveis, os cruzamentos de plantas ou animais com mais
  chance de dar as características desejadas: resistência a pragas, a seca,
  mais produtividade. É um problema de otimização gigante, feito para
  computadores.
- **Desenvolvimento de novos remédios.** Simular como milhares de moléculas
  candidatas se encaixam numa proteína do corpo, para descobrir quais valem a
  pena testar em laboratório. Isso economiza anos de pesquisa e muito
  dinheiro.
- **Inteligência artificial.** Treinar e rodar modelos de IA é, no fundo,
  multiplicar matrizes enormes, e esse é justamente um dos tipos de cálculo
  que o Auron já sabe distribuir e conferir.
- **Simulação e otimização em geral:** logística, clima, engenharia, qualquer
  área em que falta poder de cálculo.

### Como funciona (o UTRAX)

1. Quem precisa de cálculo publica uma tarefa e **deposita o pagamento
   antes**. O valor fica travado, e ninguém trabalha de graça.
2. Quem tem computador executa a tarefa e entrega o resultado.
3. **A rede confere o resultado antes de pagar.** O ponto central do projeto é
   que conferir é muito mais barato que calcular. Por exemplo, dá para provar
   que uma multiplicação de matrizes gigante está certa sem refazê-la, com um
   método matemático chamado verificação de Freivalds.
4. Resultado certo: quem trabalhou recebe. Resultado errado: não recebe, e a
   tarefa volta a ficar disponível.

### O que isso não é, tecnicamente

Para ninguém exagerar o que a tecnologia faz:

- **A segurança da moeda continua vindo de uma mineração comum** (Argon2id),
  testada e previsível. O trabalho útil fica num sistema separado, por
  enquanto fora das regras que decidem quais blocos valem. Motivo: se um
  tipo de tarefa tiver um defeito, isso não pode travar ou quebrar a moeda.
  Juntar as duas coisas com segurança é um problema em aberto na pesquisa, e
  o projeto trata isso como pesquisa, não como promessa.
- **Hoje o protótipo do UTRAX tem três tipos de tarefa:** multiplicação de
  matrizes, otimização de escolha (o "problema da mochila") e simulação de
  difusão. Aplicações como remédios e genética são o objetivo. Elas dependem
  de parcerias com quem tem os dados e os modelos científicos: laboratórios,
  universidades e empresas de pesquisa.

---

## 4. O que o Auron NÃO é

Esta seção é tão importante quanto as outras.

- **Não é promessa de enriquecimento.** Ninguém vai "ficar rico da noite para
  o dia" com o Auron. Não existe estimativa de preço, nem previsão de
  valorização, nem "a moeda vai valer X".
- **Não é investimento, e não há venda antecipada.** O projeto não vende moeda,
  não faz pré-venda e não aceita dinheiro em troca de moeda futura.
- **Não é pirâmide nem esquema de indicação.** Ninguém ganha trazendo outras
  pessoas.
- **Não é para enganar ninguém.** Quem prometer lucro, rendimento ou
  valorização em nome do Auron está mentindo, e não fala pelo projeto.

**Por que levamos isso tão a sério:**

- **Confiança.** Uma moeda só funciona se as pessoas confiam nela. Promessa
  falsa destrói essa confiança para sempre.
- **A lei.** No Brasil, oferecer um ativo digital com promessa de lucro pode
  ser tratado como oferta de investimento, fiscalizada pela CVM. E desde
  fevereiro de 2026, empresas que intermediam ou guardam ativos virtuais
  precisam de autorização do Banco Central. O Auron quer crescer dentro da
  lei, não correndo dela.

---

## 5. Por que precisamos das pessoas

Uma criptomoeda não é confiável porque o criador diz que é. Ela é confiável
quando **muitas pessoas independentes conferem** e chegam à mesma conclusão.
Por isso o Auron precisa de gente:

- **Quem roda nós da rede.** A rede só é descentralizada se muitos
  computadores, de muitas pessoas, rodarem o programa e conferirem as regras.
- **Quem testa.** Antes de qualquer lançamento, a rede de teste precisa rodar
  por meses, com pessoas usando, tentando quebrar e relatando problemas.
- **Quem revisa e audita.** Programadores e especialistas em segurança
  procurando falhas que os criadores não viram. Olhar de fora é essencial.
- **Quem tem problemas reais para resolver.** Pesquisadores, laboratórios e
  empresas que precisam de poder de cálculo e podem testar o UTRAX com
  tarefas de verdade.
- **Quem dá opinião sincera**, inclusive dizendo o que está errado.

**O que pedimos:** tempo, computador e olhar crítico. **O que não fazemos:**
vender. Quem quiser pode fazer uma doação voluntária, que não dá direito a
moeda, retorno, prioridade ou qualquer contrapartida. O projeto publica o que
recebeu e como usou.

---

## 6. Como o projeto garante que é sério

Não pedimos que ninguém acredite na nossa palavra. O método de trabalho foi
feito para que tudo possa ser conferido:

- **Regras escritas.** Existe uma especificação técnica (AURON-SPEC-01) que
  diz exatamente o que é válido na rede.
- **Duas implementações que precisam concordar.** Primeiro, cada regra é
  escrita num programa de referência, simples de ler. Depois, ela é
  traduzida para o programa de produção, e as duas versões precisam dar
  **exatamente o mesmo resultado, byte a byte**, em milhares de casos de
  teste. Se discordarem em um único caso, o teste falha.
- **Testes de ataque, não só do caminho feliz.** Os testes tentam gastar o
  mesmo dinheiro duas vezes, forjar assinatura, inflar a recompensa, adulterar
  bloco, e confirmam que a rede recusa.
- **Revisão de segurança.** Em 11/09/2026 foi feita uma revisão de ataque com
  revisores independentes. Ela encontrou 6 falhas, nenhuma que permitisse
  criar dinheiro ou gastar duas vezes, e as correções estão em andamento.
  Encontrar falhas agora, antes do lançamento, é exatamente o objetivo.
- **Estado honesto.** Cada parte do projeto é marcada como **comprovada**,
  **em teste** ou **ideia**, e nunca se apresenta ideia como tecnologia
  pronta.
- **Decisões arriscadas por escrito.** Quando uma escolha técnica tem risco, o
  risco fica registrado na especificação, com o motivo da escolha.

---

## 7. Onde estamos hoje (11/09/2026)

| Parte | Situação |
|---|---|
| Especificação técnica | escrita; ainda é rascunho, porque a tokenomics não está definida |
| Programa de referência (Python) | completo, 119 testes passando |
| Programa de produção (Rust) | começado: dinheiro, criptografia e codificação prontos, 40 testes |
| Transação com vários ativos e vários destinatários | pronta no programa de referência |
| UTRAX (trabalho útil) | protótipo com três tipos de tarefa |
| Revisão de segurança | feita; 6 falhas encontradas, correções em andamento |
| Rede entre os computadores (P2P) | não começada |
| Rede de teste pública | não existe |
| Rede principal e moeda com valor | **não existem** |

---

## 8. Atualizações futuras

São planos, na ordem em que vão ser feitos, **sem data prometida**. Cada etapa
só avança depois que a anterior estiver testada.

1. **Terminar o programa de produção**: transação, bloco, mineração, regras
   de consenso e armazenamento.
2. **Rede entre os computadores**: comunicação cifrada entre os nós.
3. **Rede de teste pública**, rodando por meses, com vários computadores.
4. **Auditoria de segurança externa**, feita por especialistas independentes.
5. **Lançamento da rede principal**, só depois de tudo isso.

Depois do lançamento, a arquitetura já prevê:

- **Auron Direct:** pagamentos entre pessoas, inclusive **sem internet**, por
  Bluetooth, com acerto posterior na rede.
- **Auron Resonance:** mensagens cifradas entre pessoas, que chegam ao destino
  mesmo quando a internet cai, passando de aparelho em aparelho.
- **Mercado de computação ampliado:** mais tipos de tarefa, incluindo pesquisa
  científica e inteligência artificial, e pagamentos entre máquinas.
- **Auron Flux:** moedas estáveis ligadas ao real e a outras moedas. **Só com
  estrutura jurídica e autorização do Banco Central.**
- **Ferramentas para lojistas:** cobrança por Pix e caixa para pequeno
  comércio. Ideia pausada, para o futuro.

---

## 9. Guia para quem vai falar em nome do Auron

**Pode dizer:**

- que o Auron é uma criptomoeda em desenvolvimento, focada em usar poder de
  cálculo para resolver problemas reais;
- que o trabalho útil é conferido antes de ser pago, e dar os exemplos de
  genética, remédios e IA **como objetivos**;
- que o projeto está em fase de construção e testes, e dizer o estado real
  (seção 7);
- que o projeto precisa de testadores, revisores, pesquisadores e pessoas
  para rodar nós;
- que as regras são escritas e o método de teste permite conferir tudo.

**Nunca diga:**

- que alguém vai lucrar, ganhar dinheiro fácil ou ficar rico;
- qualquer previsão de preço ou de valorização;
- que existe venda, pré-venda, "entrada antecipada" ou oportunidade de
  investimento;
- que a rede já funciona publicamente ou que a moeda já tem valor;
- que o Auron já está curando doenças ou revolucionando a genética **hoje**:
  isso é o objetivo, não o presente;
- qualquer coisa que você não tenha certeza de que é verdade. Na dúvida,
  diga "não sei, vou confirmar".

> **Regra de ouro do projeto:** simples para quem usa, extremamente rigoroso
> por baixo, e honesto sempre.
