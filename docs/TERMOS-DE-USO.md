<!--
Termos de uso do programa Hyurax, versão 2, de 27/09/2026.
(Versão 1, de 26/09/2026: sem a computação científica.)
O instalador e o programa mostram este texto e guardam o aceite com a versão.
Mudar o texto de forma relevante = subir a versão (TERMOS_VERSAO no código),
para o programa pedir o aceite de novo.

Este texto foi escrito pelo projeto, com cuidado, mas não é aconselhamento
jurídico e ainda não foi revisado por advogado. Antes da rede principal, ele
precisa dessa revisão.
-->

# Termos de uso e isenção de responsabilidade

**Versão 2 · 27 de setembro de 2026**

Leia com atenção. Ao instalar ou usar o Hyurax, você declara que leu,
entendeu e concorda com estes termos. Se não concorda, não instale nem use.

## 1. O que é o Hyurax

O Hyurax é um **software livre e experimental**, distribuído sob a licença MIT
ou a Apache-2.0, à escolha de quem usa. Ele liga o seu computador a uma
**rede de teste**.

- O HYX desta rede **não tem valor econômico**. Ele existe para testar o
  software.
- **Não há venda, pré-venda, investimento nem promessa de lucro** de qualquer
  tipo. Ninguém está autorizado a oferecer HYX como investimento em nome do
  projeto.
- A rede principal ainda não existe. Quando e se existir, isso será anunciado
  publicamente, com termos próprios.

## 2. Sem garantia

O software é entregue **"no estado em que se encontra"**, sem garantia de
nenhum tipo, expressa ou implícita: nem de funcionar, nem de estar disponível,
nem de não ter erros, nem de servir para um fim específico. Por ser
experimental, ele pode ter falhas, perder dados ou parar de funcionar.

## 3. Limitação de responsabilidade

Na máxima extensão permitida pela lei, os autores e colaboradores do Hyurax
**não respondem por perdas ou danos** de qualquer natureza que venham do uso ou
da impossibilidade de uso do software. Isso inclui perda de dados, perda de
acesso à carteira, dano ou desgaste de equipamento, gasto de energia e lucros
que você esperasse ter.

## 4. Sua carteira é só sua

- A chave da carteira é criada e guardada **no seu computador**, cifrada com a
  sua senha. O projeto não recebe, não guarda e não tem como ver a sua chave
  nem a sua senha.
- **Ninguém consegue recuperar** uma carteira perdida ou uma senha esquecida.
  Guardar uma cópia do arquivo da carteira e lembrar a senha é
  responsabilidade sua.
- Uma transação assinada e enviada não pode ser desfeita.

## 5. O uso do seu computador

A mineração e o ULTRAX usam o processador (CPU), a placa de vídeo ou a GPU
integrada, a memória e a energia do seu computador. Isso esquenta o
equipamento, aumenta a conta de luz e pode contribuir para o desgaste das
peças. O programa mostra estimativas e tem limitadores de uso, mas **você
decide** quanto usar e responde por essa decisão. Em notebook, fique de olho
na temperatura e na bateria.

## 6. O ULTRAX, a computação científica e o Work Score

O ULTRAX executa trabalho computacional útil e verificável. Ele trabalha de
três jeitos:

- **Modo LAB:** as tarefas são geradas no seu próprio computador. É
  trabalho de teste, e **ninguém de fora encomendou nem paga por ele**.
- **JOBs científicos seus:** você pede o cálculo (genética, plantas, rotas,
  moléculas e outros), e o seu computador o faz.
- **Calcular para outros nós:** fica **desligado** até você ligar a chave
  "Calcular unidades que outros nós pedirem". Ligada, o seu computador
  calcula pedaços de JOBs de outras pessoas, dentro dos limites de CPU,
  memória e tempo que você escolheu, e pode desligar quando quiser.
  **Ninguém paga por esse trabalho.**

Sobre os resultados, os pontos e os créditos:

- Os resultados são **simulações e modelos**. Não são laudo, diagnóstico,
  recomendação médica, agronômica, de engenharia ou de investimento, e
  **não substituem** estudo, ensaio de laboratório ou profissional
  habilitado. Um modelo pode errar, e o relatório diz onde ele sabidamente
  erra.
- O **Work Score** e os **créditos de computação** medem contribuição
  computacional. **Não são dinheiro, não são HYX e não dão direito a
  pagamento, recompensa ou participação** de nenhum tipo.
- A reputação de cada worker é uma medida **local**: é o que o seu nó viu,
  não uma nota da rede.

## 7. Conexões que o programa faz

Para funcionar, o programa:

- conversa com **outros nós da rede Hyurax**, por conexão cifrada. Os nós com
  que você se conecta veem o seu endereço IP, como em qualquer rede par a par;
- se você ligar "calcular para outros nós", **recebe** deles a descrição do
  cálculo e **devolve** o resultado. Ao pedir um JOB com verificação entre
  nós, faz o contrário: manda a descrição e recebe os resultados. Cada
  resultado vai assinado com uma chave derivada da identidade do seu nó,
  que os outros nós veem. Ela não leva o seu nome nem a sua carteira;
- baixa a **lista pública de nós semente** do repositório do projeto no
  GitHub;
- consulta a **api.coingecko.com** para mostrar preços de criptomoedas, **só**
  se você ligar o painel "Mercado" nos ajustes;
- se você ligar "ver no celular", deixa o painel visível para aparelhos da sua
  rede local, só para leitura.

O programa **não envia dados de uso, não tem rastreamento e não mostra
anúncios**.

## 8. Privacidade (LGPD)

- Os dados do programa (cadeia, carteira cifrada, histórico do ULTRAX, JOBs
  científicos e relatórios, ajustes) ficam na pasta `%APPDATA%\Hyurax` do seu computador. O projeto não
  recebe esses dados.
- As **transações na cadeia são públicas** por natureza: qualquer nó vê os
  endereços e valores. Não coloque informação pessoal em lugar nenhum da
  cadeia.
- Para apagar tudo, feche o programa e apague a pasta `%APPDATA%\Hyurax`.
  **Isso apaga também a carteira**: guarde uma cópia antes, se quiser.

## 9. Uso de acordo com a lei

Você é responsável por usar o Hyurax de acordo com as leis do seu país,
inclusive as tributárias e as que tratam de criptoativos. É proibido usar o
programa para qualquer atividade ilícita.

## 10. Segurança

Tentar achar falhas no Hyurax é bem-vindo, desde que seja **em nós seus** ou
na rede de teste do projeto. Falhas se relatam pelo caminho descrito no
arquivo `SECURITY.md` do repositório. Atacar redes, computadores ou nós de
terceiros é proibido.

## 11. Doações

Doações ao projeto são voluntárias e **não dão direito a nada**: nem token,
nem participação, nem retorno.

## 12. Mudanças nestes termos

Estes termos podem mudar. Quando a mudança for relevante, a versão sobe, e o
programa pede o seu aceite de novo antes de continuar.
