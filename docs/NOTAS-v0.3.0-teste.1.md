# Hyurax v0.3.0-teste.1

**Rede de TESTE.** A rede pública não existe e o HYX não tem valor. Não há
venda, pré-venda nem promessa de lucro. O Work Score do ULTRAX mede
contribuição: não é dinheiro e não é HYX.

Esta versão fala a mesma rede que a `v0.2.0-teste.1`: nada muda na cadeia, no
bloco nem na transação. Quem já tem a carteira continua com ela.

## O que há de novo

### Instalador para Windows

- `hyurax-instalador-windows-x86_64.exe`: mostra os termos de uso e só instala
  depois do "li e aceito".
- Instala para o usuário atual, **sem pedir administrador**, em
  `%LOCALAPPDATA%\Programs\Hyurax`. Cria o atalho no Menu Iniciar e, se você
  quiser, na Área de Trabalho.
- Aparece em *Configurações → Aplicativos*, de onde se desinstala. A
  desinstalação tira o programa, os atalhos e o registro, e **não apaga** a
  pasta de dados (`%APPDATA%\Hyurax`), onde está a carteira.
- Para instalar por script: `--silencioso --aceito-os-termos`, com
  `--area-de-trabalho` e `--abrir` opcionais.
- O zip de sempre (`hyurax-windows-x86_64.zip`) continua, para quem prefere
  não instalar.

### Termos de uso

- Texto em [`docs/TERMOS-DE-USO.md`](TERMOS-DE-USO.md): software experimental,
  sem garantia, e cada pessoa responde pelo uso que faz.
- O instalador e o programa pedem o aceite. Quem abre pelo zip vê os termos
  na primeira abertura.
- Ainda não foi revisado por advogado. Isso é obrigatório antes da rede
  principal.

### ULTRAX: trabalho útil de verdade

- **IA sobre moléculas reais.** Uma rede neural pequena aprende a
  solubilidade em água de 2.048 moléculas da base AqSolDB (CC0).
  - Tudo em aritmética inteira. A especificação é o Python, e o Rust dá o
    mesmo resultado byte a byte.
  - A tela desenha a molécula a partir do SMILES e mostra o valor medido em
    laboratório ao lado do previsto.
  - Erro típico do melhor modelo: ±1,25 em log S, nas moléculas que ele
    nunca viu no treino.
  - Prever solubilidade é uma etapa de triagem na pesquisa de remédios. Isto
    não descobre remédio, e a tela diz isso.
- **A GPU também trabalha, inclusive a integrada.** As contas de matriz rodam
  no WebGL2 da própria janela, e a CPU confere cada resultado (Freivalds)
  antes de contar.
  - Continua trabalhando com a janela minimizada.
  - Tem limitador próprio: 25%, 50%, 75% ou 100%.
  - Num Atom com vídeo Intel HD, a GPU fez 426 milhões de operações por
    segundo, contra 250 milhões de um núcleo da CPU.
- Limitador de CPU (de 10% a 100%) e teto de memória separados, tarefas-desafio com resposta
  conhecida, histórico assinado e auditável, reputação e Work Score.

### Programa

- **Redesign completo:** navegação por seção (Visão geral, ULTRAX,
  IA · Moléculas, Mineração, Carteira, Rede), as fontes da marca dentro do programa e o
  layout de celular.
- **Endereço com dígito verificador** (Bech32m, começa com `thyx1` na rede de
  teste).
  - Um caractere errado é recusado antes de enviar, e não vira HYX perdido.
  - Endereço em hexadecimal continua aceito.
- **Painel mais rápido:** cada resposta esperava 300 ms à toa, e agora não
  espera mais.

### Site

- Novo desenho, com os capítulos ULTRAX e IA (moléculas reais da base).
- **Nenhum cookie e nenhum terceiro.** As fontes agora são servidas pelo
  próprio site, sem Google Fonts. Política de segurança de conteúdo em todas
  as páginas.
- Páginas de [privacidade e cookies](../site/privacidade.html) (LGPD) e de
  [termos de uso](../site/termos.html), e o `/.well-known/security.txt`.

## Conferir antes de rodar

Cada arquivo da Release tem a soma SHA-256 ao lado. O programa ainda não tem
assinatura digital, então o Windows pode mostrar "O Windows protegeu o
computador". Para abrir, clique em *Mais informações* e depois em *Executar
assim mesmo*. Faça isso só depois de conferir a soma.

## O que não foi medido

- O instalador foi testado neste Windows 10, numa pasta de teste. Ainda não
  foi testado em Windows 11 nem numa máquina sem nada instalado.
- A GPU foi medida numa Intel HD. Placas AMD e NVIDIA ainda não foram
  testadas.
