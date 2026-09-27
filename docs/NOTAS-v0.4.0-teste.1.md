# Hyurax v0.4.0-teste.1

**Rede de TESTE.** A rede pública não existe e o HYX não tem valor. Não há
venda, pré-venda nem promessa de lucro. O Work Score e os créditos de
computação medem contribuição: não são dinheiro e não são HYX.

Esta versão fala a mesma rede que a `v0.3.0-teste.1`: nada muda na cadeia, no
bloco nem na transação, e quem já tem a carteira continua com ela.

As mensagens novas, da computação entre nós, usam um tipo que as versões
anteriores ignoram. Por isso, nós novos e antigos convivem na mesma rede.

## O que há de novo

### Computação científica (seção "Ciência" do programa)

- **JOBs de verdade:** você descreve o cálculo, e o programa o divide em
  unidades, calcula, confere cada unidade antes de contar e junta tudo num
  relatório em JSON, CSV e PDF.
  - Tem checkpoint e retoma de onde parou se o programa fechar.
  - Cada unidade se refaz bit a bit com `hyurax-no ciencia refazer`.
- **Motores:**
  - genética de populações (Wright-Fisher);
  - melhoramento de plantas, com água, nitrogênio e solo;
  - rotas de entrega (logística);
  - triagem de 8.289 moléculas reais da base AqSolDB;
  - treino de IA, matriz, mochila e difusão.
  - Materiais, energia, meio ambiente e regeneração existem só como
    interface: ainda não têm motor, e a tela diz isso.
- **Visão 3D ao vivo**, alimentada só por eventos reais: nada se mexe sem
  cálculo por trás.
  - "Inspecionar cálculo" mostra os campos do evento.
  - "Reproduzir" repassa um JOB antigo no tempo em que ele aconteceu.
- **ULTRA BENCHMARK**, com base, atual, alvo e ganho medidos nesta máquina.
- **Créditos de computação**, separados do HYX e sem liquidação: crédito
  não vira dinheiro.

### Calcular para outros nós (vem desligado)

- A chave "Calcular unidades que outros nós pedirem" deixa o seu computador
  calcular pedaços de JOBs de outras pessoas.
  - Respeita os seus limites de CPU, memória e tempo.
  - Desliga quando você quiser.
  - Ninguém paga por esse trabalho.
- **JOB com verificação entre nós (nível 3):**
  - a mesma unidade vai para workers de outros nós, e uma cópia é calculada
    aqui;
  - cada worker publica um compromisso antes de revelar o resultado, e
    decide a maioria;
  - o resultado vencedor ainda é conferido aqui antes de contar;
  - se dois workers combinarem o mesmo resultado falso, a conferência recusa
    e eles levam a recusa na reputação.

### Termos de uso, versão 2

- Cobrem os JOBs científicos e o cálculo para outros nós.
- Os resultados são simulações e modelos: não são laudo nem recomendação
  profissional.
- Os créditos não são dinheiro.
- O programa pede o aceite de novo a quem aceitou a versão 1.
- O site traz os mesmos termos e a política de privacidade atualizada.
- Ainda não foram revisados por advogado. Isso é obrigatório antes da rede
  principal.

### Desempenho

- Unidade curta ficava esperando o relógio, porque a fila do ULTRAX era
  reposta uma vez por segundo. Agora a fila acorda o trabalho na hora.
- Medido num Atom: de 5 para 77 unidades conferidas por segundo, do começo
  ao fim (15,6×). A conta dos motores não mudou.

## Conferir antes de rodar

Cada arquivo da Release tem a soma SHA-256 ao lado. O programa ainda não tem
assinatura digital, então o Windows pode mostrar "O Windows protegeu o
computador". Para abrir, clique em *Mais informações* e depois em *Executar
assim mesmo*. Faça isso só depois de conferir a soma.

## O que não foi medido

- A computação entre nós foi ensaiada com três processos **numa máquina
  só**: 30 de 30 unidades em consenso 3/3. Ainda não foi feita entre
  computadores diferentes.
- A reputação dos workers fica só na memória e recomeça ao reabrir o
  programa.
- A rede conta mensagens, não bytes.
- Na triagem, o modelo foi treinado em moléculas orgânicas e erra feio nas
  inorgânicas. O relatório mostra isso.
- O instalador continua testado só neste Windows 10.
