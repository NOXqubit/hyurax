# Direção de arte do site Auron

Estudo de 13/09/2026: três direções (A cinematográfica, B instrumento vivo, C luxo arquitetônico), três jurados (placar A 121,5 · B 121,5 · C 116; dois jurados escolheram A). Vence A, com enxertos de B e C.

Partes 1 a 3 vêm da Direção A; a partir da 4, da síntese final (o começo da síntese se perdeu no limite de uso).

# Direção A: Lançamento cinematográfico

## 1. Tese e os primeiros 3 segundos

**Tese.** A página se comporta como o filme de lançamento de uma máquina. Ela tem um único objeto de metal, o "A" da Auron, e a luz quente aparece só onde existe algo real e conferido. Por isso, rolar a página é ver essa luz acender, se confirmar e depois se apagar, até o ponto em que o projeto ainda é só ideia.

**A regra que sustenta tudo:** a luz quente (`--luz`) quer dizer "isto existe e foi conferido". Ela não é decoração. Quando uma coisa é recusada, a luz apaga e aparece um corte. Quando uma coisa ainda é ideia, ela vira contorno sem luz.

### O momento "puta que pariu", quadro a quadro

Os primeiros 1,3 s não dependem do Three.js. O arquivo `vendor/three.module.min.js` pesa 691 KB e demora para ser interpretado no Atom. Então essa parte é feita só com HTML, CSS e SVG e aparece no primeiro desenho da tela. Assim some a tela vazia de 1,5 a 2,25 s que existe hoje.

| Tempo | O que a pessoa vê | Como é feito |
|---|---|---|
| 0 ms | Preto `#040405`. No canto superior esquerdo, o símbolo A com 20 px e "AURON" em Michroma 12 px. Mais nada. | HTML estático com os textos em pt-BR já no HTML. O i18n só troca o texto depois. |
| 0–450 ms | Um fio de luz quente de 1,5 px atravessa a tela da esquerda para a direita. É a **lâmina do logo**, a curva `M8 90 C30 75 51 64 75 57`, ampliada para 120vw. Atrás dela há um rastro de 8 px com 12% de opacidade. | `stroke-dashoffset` num `<path>` do próprio símbolo, sem `filter`. |
| 450–900 ms | Por onde a lâmina passou, surge o **A metálico gigante**: 78svh de altura, na metade direita, cortado uns 8% pela borda direita. Ele parece iluminado de raspão pela lâmina. | `clip-path` animado num único elemento com o `#simbolo-a` e o gradiente `#metal`. |
| 600–1300 ms | O título sobe de dentro de máscaras, linha por linha, com 120 ms entre elas, alinhado à esquerda nas colunas 1–7: **"Todo bloco / prova um / cálculo útil."** Usa Archivo wdth 125, peso 780. O fio da lâmina termina embaixo de "útil." e fica ali. | `transform: translateY` dentro de linhas com `overflow: hidden`. É animação só de composição, sem blur e sem animar `filter`. |
| 1300–2000 ms | **O golpe.** O A que parecia um logo plano **gira 22° e mostra que é uma peça sólida, usinada**, com espessura, chanfro e reflexo de metal. Um filete de luz corre pela aresta do vértice até os pés, uma vez só. Uma poeira quente e esparsa se condensa em volta. | O canvas 3D aparece por baixo do SVG com o A **na mesma pose e no mesmo enquadramento** (câmera com FOV 18, como uma teleobjetiva, casada com a caixa do SVG). O SVG some em 200 ms e a peça gira. A pessoa não vê emenda. |
| 2000–2600 ms | No canto inferior direito, em mono 14 px, aparece de uma vez, sem efeito de digitação: `sha512(cabeçalho v2 · 222 bytes) = 9c1e…4a07 · 0,38 ms neste aparelho`. No alto à esquerda, em mono 12 px prata: `REDE DE TESTE · SEM VENDA · SEM PREÇO`. | `engine.mineBlock()` calcula de verdade um cabeçalho de exemplo e `performance.now()` mede o tempo. O rótulo diz "cabeçalho de exemplo". |
| 2600–3000 ms | Silêncio. O A fica **parado**, sem girar à toa. Embaixo, uma linha vertical de 1 px com a palavra "role" em mono 12 px. | Nada mais anima. A calma aqui é intencional. |

**Por que causa impacto:** a pessoa acha que está vendo um logo, e em um segundo ele vira um objeto físico, numa página preta, com letras enormes. Logo depois vem um número calculado *agora, no aparelho dela*. Não há promessa, só uma prova.

**Variações da abertura:**

- **LOW (Atom):** os primeiros 1,3 s são iguais. A troca para o 3D acontece quando o módulo terminar de carregar, na mesma pose, então não há salto. Se o governador (seção 5.4) decidir que nem o LOW sustenta, o SVG fica, e o filete de luz passa a ser uma faixa de gradiente animada só com `transform`, dentro de um contêiner com `clip-path: url(#forma-a)` fixo.
- **Celular:** o A ocupa a metade de cima, cortado pela borda direita. O título fica embaixo, alinhado à esquerda, em `--t-monumento` do celular (52 px).
- **`prefers-reduced-motion`:** o último quadro aparece direto, com o A estático (SVG ou um único quadro 3D) e o texto completo.

---

## 2. Sistema de tokens

### 2.1 Paleta (8 valores, um acento)

| Token | Hex | Papel | Regra |
|---|---|---|---|
| `--vazio` | `#040405` | Fundo absoluto | O preto do filme |
| `--grafite` | `#111214` | Placas e superfícies raras | No máximo 6 placas na página inteira |
| `--aco` | `#24262A` | Filetes, estados apagados, contornos de "ideia" | **Nunca em texto** (contraste baixo) |
| `--chumbo` | `#7C8088` | Texto terciário e rótulos | 5,2:1 sobre `--vazio` e 4,7:1 sobre `--grafite` (passa no AA) |
| `--prata` | `#C4C7CC` | Metal, texto secundário forte, dados | Cor padrão dos números |
| `--osso` | `#EDEBE6` | Texto principal | — |
| `--luz` | `#F2D9A8` | **Acento único:** "existe e foi conferido" | Selo implementado, CONTA REAL, aceito, foco |
| `--luz-nucleo` | `#FFF4E0` | A mesma luz no ponto de saturação | Só no miolo de brilhos do 3D e no fio da lâmina. Não é outra cor. |

**Removidos:**
- `--st-pesquisa` (azul), `--st-aceito` (verde) e `--st-recusado` (salmão);
- no 3D, `RECUSADO 0xff5e4d`, `FRIO 0x9cc4ff`, `ACEITO 0x6fe3a5` e `OURO 0xf2b865`;
- os mais de 15 `rgba(243,226,196,.x)` escritos à mão. No lugar deles entra `color-mix(in srgb, var(--luz) 12%, transparent)`, ou um token `--luz-12`.

**Estado por forma e luz, sem cor nova.** Os glifos têm 12 px e sempre vêm acompanhados de texto:

| Estado | Glifo | Cor |
|---|---|---|
| Implementado | ● disco cheio | `--luz` |
| Em desenvolvimento | ◐ meio disco | `--luz` |
| Pesquisa | ◌ anel tracejado | `--prata` |
| Planejado | ○ anel vazio | `--chumbo` |
| Recusado / erro / quarentena | **luz apagada** (o elemento cai para `--aco`) e **corte da lâmina**: um filete de 1 px a **−26,2°**, o ângulo real da lâmina do logo (de (8,90) a (75,57)) | `--prata` no corte |

### 2.2 Tipografia (Google Fonts)

```
https://fonts.googleapis.com/css2?family=Archivo:wdth,wght@62..125,250..800&family=IBM+Plex+Mono:wght@400;500&family=Michroma&display=swap
```

Quando o idioma é `ja`, entra também `Noto+Sans+JP:wght@400;700`, carregada pelo `i18n.js`.

| Função | Família | Eixos e pesos | Uso |
|---|---|---|---|
| **Display** | Archivo variável | wdth **125** / wght **780** para a ênfase; wdth **62** / wght **250** para o contraponto | Monumentos e títulos. A ênfase vem do **contraste de largura e peso, não de cor**: o `<em>` quente dos 18 títulos deixa de existir. |
| **Texto** | Archivo | wdth 100 / wght 400 e 500 | Corpo e lead |
| **Dados** | IBM Plex Mono | 400 e 500 (algarismos tabulares nativos) | Hashes, campos da spec, nomes de testes, unidades |
| **Claquete** | Michroma | 400 | **Só** na claquete de capítulo (`04 — CADEIA`) e no "TECHNOLOGY" do wordmark |

**Escala.** São 7 passos e nada fica abaixo de 12 px. Isso substitui os cerca de 25 tamanhos soltos de hoje.

| Token | Valor | 1440 px | 375 px | Entrelinha | Espaçamento entre letras |
|---|---|---|---|---|---|
| `--t-monumento` | `clamp(3.25rem, 11.5vw, 13rem)` | 165 | 52 | .86 | −.035em |
| `--t-titulo` | `clamp(2.25rem, 6vw, 6rem)` | 86 | 36 | .95 | −.02em |
| `--t-frase` | `clamp(1.5rem, 3vw, 2.75rem)` | 43 | 24 | 1.12 | −.01em |
| `--t-lead` | `clamp(1.125rem, 1.5vw, 1.375rem)` | 22 | 18 | 1.4 | 0 |
| `--t-corpo` | `1.0625rem` | 17 | 17 | 1.55 | 0 |
| `--t-dado` | `.875rem` mono | 14 | 14 | 1.4 | 0 |
| `--t-rotulo` | `.75rem` mono em caixa alta | 12 | 12 | 1.3 | +.14em |

**Regras de texto:**
- Há só três valores de espaçamento entre letras. Não existe mais .02, .06, .24, .34 nem .62.
- Palavras em `--t-monumento` têm **no máximo 12 caracteres nos 4 idiomas**. A regra fica documentada em `locales/glossary.js`.
- `font-size` usa `min(var(--t-monumento), calc(100cqi / var(--chars)))` com container query, para "INFRAESTRUTURA" em es não estourar.
- Em ja, o monumento vale × .72, sem eixo de largura.

### 2.3 Grade

- **Colunas:** 12 no desktop, 8 a partir de 40rem e 4 no celular.
- **Margem:** `--margem: clamp(20px, 5vw, 80px)`.
- **Calha:** `--calha: clamp(16px, 2vw, 32px)`.
- **Largura máxima:** 105rem (1680 px). Os palcos 3D sangram a tela inteira.
- **Breakpoints:** só **3**: 40rem, 64rem e 96rem. Hoje são 7.
- **Composição:**
  - O texto se ancora nas colunas 1–5 ou 8–12.
  - O objeto ocupa o lado oposto e **invade a coluna do texto em 1 ou 2 colunas**. A sobreposição é o que dá profundidade.
  - Só **dois** momentos são centralizados: o último quadro de `#fim` e a claquete de ato.

### 2.4 Espaçamento (base 4 px)

`--e-1` 4 · `--e-2` 8 · `--e-3` 12 · `--e-4` 16 · `--e-5` 24 · `--e-6` 32 · `--e-7` 48 · `--e-8` 64 · `--e-9` 96 · `--e-10` 144 · `--e-11` 216 px

- **Silêncio entre atos:** um bloco de **70svh de preto**, só com a claquete de ato no centro.
- **Entre capítulos do mesmo ato:** `--e-10`.

### 2.5 Raios (3 valores)

| Token | Valor | Uso |
|---|---|---|
| `--r-0` | 0 | Palcos, telas, réguas. Arestas secas de peça usinada. |
| `--r-1` | 3 px | Placas, campos de texto, chips de dado |
| `--r-pilula` | 999 px | **Só** a chave liga/desliga e o botão principal único |

**Chanfro de marca:** as placas recebem `clip-path: polygon(0 0, calc(100% - 12px) 0, 100% 12px, 100% 100%, 0 100%)`, um canto cortado a 45° que ecoa o triângulo do A. É barato e não usa sombra.

### 2.6 Materiais

| Material | CSS | 3D |
|---|---|---|
| **Metal escovado** (corpo do A) | SVG `#metal` como está | HIGH: `MeshStandardMaterial` com metalness 1, roughness .28, ambiente PMREM procedural de estúdio (luz de topo quente e rebatedor **prata neutro**, sem o frio azulado atual). MEDIUM, LOW e celular: `MeshMatcapMaterial` com matcap de 256² gerada em canvas. |
| **Fio de luz** (aresta da lâmina) | `stroke` `--luz`, 1,5 px | Faixa emissiva injetada com `onBeforeCompile`: `--luz-nucleo` × 3 em HDR, na layer de bloom |
| **Placa gravada** | `--grafite`, `box-shadow: inset 0 1px 0 color-mix(in srgb, var(--prata) 10%, transparent)`, chanfro, **sem `backdrop-filter`** | — |
| **Projeção** (ato IV, "ideia") | Filete de 1 px `--aco`/`--prata`, tracejado quando é pesquisa | `EdgesGeometry` do A com `LineBasicMaterial` prata a 35% |
| **Vazio** | `--vazio` | Quad de fundo opaco com vinheta desenhada no próprio shader. **A div `.veu` sai do DOM.** No HIGH, grão de amplitude .035 na composição. |

### 2.7 Movimento

A unidade de tempo vem da trilha própria: **62 BPM, 1 batida = 968 ms**.

| Token | Valor | Uso |
|---|---|---|
| `--d-1` | 120 ms | Troca de estado |
| `--d-2` | 240 ms | Microinteração |
| `--d-3` | 484 ms | Meia batida: entradas |
| `--d-4` | 968 ms | Uma batida: fusões de forma |
| `--d-5` | 1936 ms | Duas batidas: movimentos de câmera autônomos |
| `--curva-filme` | `cubic-bezier(.16,1,.3,1)` | Entradas |
| `--curva-maquina` | `cubic-bezier(.65,0,.35,1)` | Deslocamentos mecânicos, a lâmina |

---

## 3. Layout e ritmo da página

### 3.1 Nova ordem: quatro atos e epílogo

O mundo 3D passa a ser dirigido por **planos**. Cada seção declara `data-plano="<id>"` e uma altura de rolagem (`--rolagem`). O palco é `position: sticky` com 100svh, e a rolagem nativa gera um progresso `p ∈ [0,1]`. Não há scroll-jacking nem biblioteca de rolagem suave.

| # | id | Ato | Título (pt-BR) | Rolagem | Luz do ato |
|---|---|---|---|---|---|
| 00 | `#topo` | Abertura | Todo bloco prova um cálculo útil. | 160svh | cheia |
| 01 | `#visao` | I · O problema | Trilhões de cálculos. Jogados fora. | 220svh | fria, só no fim acende |
| 02 | `#nucleo` | índice vivo | Uma regra. Nenhum dono. | 120svh | cheia |
| — | *silêncio* | claquete `ATO II · O QUE JÁ FUNCIONA` | — | 70svh | — |
| 03 | `#utrax` | II | Pegue a mentira sem refazer a conta. | 300svh | exposição 1,0 |
| 04 | `#cadeia` | II | Mexa num bloco antigo. | 260svh | 1,0 |
| 05 | `#nos` | II | Uma rede que se levanta sozinha. | 260svh | 1,0 |
| 06 | `#seguranca` | II | Atacamos o próprio código. | 200svh | 1,0 |
| 07 | `#aur` | II | Regras, não preço. | 160svh | 0,8, esfriando |
| — | *silêncio* | `ATO III · EM CONSTRUÇÃO` | — | 70svh | — |
| 08 | `#mercado` **(novo)** | III | Um mercado de cálculo que não paga erro. | 120svh | 0,6 |
| 09 | `#rodar` **(novo)** | III | Rode um nó hoje. | 110svh | 0,5, o 3D dorme |
| — | *silêncio* | `ATO IV · AINDA É IDEIA` | — | 70svh | — |
| 10 | `#direct` | IV | Valor que viaja como dado. | 120svh | 0,25, projeção |
| 11 | `#resonance` | IV (absorve `#radio`) | Mensagens que encontram um caminho. | 140svh | 0,25 |
| 12 | `#fragmentacao` | IV (absorve `#armazenamento`) | Um arquivo vira 256 pedaços. | 160svh | 0,25 |
| 13 | `#inteligencia` | IV | Máquinas que contratam máquinas. | 100svh | 0,2 |
| — | *silêncio* | `EPÍLOGO` | — | 70svh | — |
| 14 | `#escala` | Epílogo | Você está aqui. | 260svh | acende só o que existe |
| 15 | `#onde` | Epílogo (absorve `#caminho`) | Onde estamos, sem enfeite. | 140svh | mapa |
| 16 | `#aberto` | Epílogo | Construído em público. | 110svh | — |
| 17 | `#apoio` | Epílogo | Apoio sem contrapartida. | 100svh | — |
| 18 | `#fim` | Final | Não confie. Confira. | 140svh | cheia, depois apaga |

**O que sai de cena:**
- Os ids `#radio`, `#armazenamento` e `#caminho` deixam de ser seções. `#caminho` continua como âncora alternativa dentro de `#onde`, para não quebrar links antigos.
- A narrativa passa de 21 para 19 blocos.
- Menu principal: **Visão · O que funciona · Em construção · Ideias · Onde estamos · Código**. Sai "AUR" e sai o botão duplicado "Explorar".
- Na barra fica um único botão secundário: **"Assistir ao filme · 3 min · voz sintética"**, que abre `#palco-edit`.

### 3.2 Componentes globais (o que muda de verdade)

| Hoje | Agora |
|---|---|
| Cerca de 21 `.painel` de vidro com `backdrop-filter` | **Palco aberto sobre o preto**, com filetes de 1 px, como uma ficha técnica. No máximo **6 placas** na página, só onde há um objeto manipulável (QR, campo de texto, chave). Zero `backdrop-filter` no site. |
| `.olho` Michroma + `h2` com `<em>` quente, 15 vezes | **Claquete**: Michroma 12 px `04 — CADEIA`, com um contador de ato `II · 2/5` à direita. Depois vem **um monumento ou um título, nunca os dois**. Cada capítulo escolhe **uma** escala: monumento em 5 capítulos (topo, visão, utrax, aur, fim) e título nos demais. |
| Seis fileiras de chips `.etapas` com "→" | **Régua de máquina**: um filete horizontal com marcas de escala e rótulos mono 12 px embaixo. A etapa ativa é um **segmento de luz que corre pelo filete**. É um componente só, usado em no máximo 4 lugares (`#cadeia`, `#mercado`, `#direct`, `#fragmentacao`). No celular vira vertical. |
| `.selo-sim` "SIMULATION" piscando 11 vezes (código de "AO VIVO") | **Dois rótulos fixos, sem pulso:** `CONTA REAL · neste aparelho · 0,38 ms` (ponto `--luz`) e `[dados de exemplo]` (colchetes `--chumbo`). A pessoa vê exatamente o que é cálculo de verdade e o que é cenário. |
| Mosaicos de 1 px (`.fatos-aur`, `.mecanismos`, `.onde`) | **Tabela de especificação**: linhas com filete e três colunas (afirmação · prova em mono · link). Nada de cartões iguais. |
| 3 cartões `.arquivo` e 2 `.id-cartao` lado a lado | Linhas de lista e a **árvore `├─ └─`**, que já é boa, em largura cheia |
| Rodapé com "Qualidade 3D: HIGH" | Sai. O HUD de depuração (fps, `renderer.info.render.calls`, triângulos, perfil) só aparece com `?debug=1`. |
| Barra de vidro com blur | Barra sólida `color-mix(--vazio 92%)` sem blur. Contém: símbolo A, **régua de progresso do filme** (1 px com 19 marcas, parte percorrida em `--luz`), idioma, som (com texto "Som: desligado", não só ícone) e menu. |

### 3.3 Tratamento de cada seção

#### `#topo`: Abertura (160svh)
- **Composição:** descrita na seção 1. O A ocupa as colunas 6–12 e invade o título. A frase fica colada à esquerda.
- **Saída (p 0,6 → 1):** a câmera faz um dolly **para dentro da aresta da lâmina** até ela preencher a tela como uma linha horizontal. Essa mesma linha é a linha de corte de `#visao`. É um **corte por semelhança**: o fim de um plano tem a mesma forma do início do próximo.
- **Removido:** wordmark AURON grande, "TECHNOLOGY", dois CTAs e a faixa `.verdade` ilegível. A faixa vira a linha de status em mono 12 px `--prata`, que passa no AA.

#### `#visao`: O desperdício (220svh)
Substitui a visão abstrata, os 6 pilares e os lemas em inglês.

- **p 0–0,35:** monumento à esquerda, "Trilhões de cálculos.", em wdth 62, peso 250. Ao fundo, uma **chuva de hashes SHA-512 reais**: `crypto.subtle.digest` sobre nonces crescentes, com ritmo limitado. No 3D, são glifos hexadecimais em quads instanciados com um atlas de 16 caracteres em `CanvasTexture`, na cor `--chumbo`, caindo devagar.
- **p 0,35–0,55:** "Jogados fora." em wdth 125, peso 780. A chuva esfria para `--aco`. Um contador mono **verdadeiro** mostra `hashes calculados nesta página: 18 402 · úteis: 0`. A própria página demonstra o desperdício.
- **p 0,55–0,8:** a lâmina varre a tela na horizontal. Abaixo da linha, os glifos se encaixam numa **matriz 8×8 de números**. Aparece a frase "O Auron exige que a conta sirva."
- **p 0,8–1:** a matriz se condensa **na letra A**. Esse A é a matriz A de `A · B = C`, e isso só é explicado em `#utrax`.
- **Celular:** 400 glifos e 160svh.

#### `#nucleo`: Índice vivo (120svh)
- **A imagem:** o A visto de cima, com teleobjetiva e FOV 20, parece um mapa gravado. Os **três vértices e a lâmina são as quatro partes**: vértice superior = CONSENSO, pé esquerdo = CADEIA, pé direito = NÓS, lâmina = PROTOCOLO.
- **Os botões:** são `<button>` do DOM posicionados com `Vector3.project()`, então continuam acessíveis por teclado. Cada um mostra o glifo de estado e o número do capítulo para onde leva ("→ 04").
- **Sai:** o organograma SVG.
- **Corrigir:** alinhar os estados com `data.js`, porque a cadeia está implementada.

#### `#utrax`: Pegue a mentira (300svh, a peça central)
Só a parte de consenso. O mercado vai para `#mercado`.

- **p 0:** o A se desdobra numa **placa 8×8**, a matriz A. Ao lado ficam as placas B e C. São `InstancedMesh` de cubos com alturas de 0 a 9 vindas de `engine.matriz()`, com o glifo "A" gravado na primeira placa.
- **p 0,1:** o texto: "A · B = C. Alguém diz que fez a conta."
- **p 0,3:** o vetor `r` aparece como uma fileira de 8 contas de luz. Os bytes do SHA-512 de C, de onde `r` é tirado, ficam visíveis em mono.
- **p 0,45–0,6:** um **feixe quente atravessa B e depois A**, e as colunas acendem. Em paralelo, outro feixe atravessa C.
- **p 0,75:** as duas colunas de 8 números **deslizam lado a lado e se encaixam**. Os números iguais acendem.
- **p 0,85, o registro de custo honesto:** "Em 8×8, conferir custa 576 multiplicações e refazer custa 512. Aqui conferir sai mais caro." Logo depois, uma mudança de escala física: um grão ao lado de uma montanha de pontos, com "1000×1000: 9 milhões contra 1 bilhão."
- **Interação livre ao fim do plano:**
  - **Chave "Fazer o executor mentir".** No desktop, a pessoa **clica no cubo de C que quer adulterar**. No celular, escolhe numa lista de linha e coluna.
  - O cubo sobe uma unidade e a conferência roda de novo. Na linha adulterada **os feixes não se encaixam, a luz daquela linha apaga e o corte a −26,2° atravessa a linha**.
  - O veredito, em mono: `recusado · linha 5 não confere · pagamento: 0`.
  - A probabilidade de uma mentira escapar é mostrada **a partir do domínio real de `r` usado no `engine.js`**. Nada de número decorado.
- **Rótulos:** `CONTA REAL · Freivalds · 0,2 ms` e `[matrizes de exemplo]`.

#### `#cadeia`: Um bloco nasce (260svh + interação)
- **O bloco:** deixa de ser cartão e vira uma **régua de 222 unidades**, uma placa fina e comprida. Ela é dividida em segmentos proporcionais aos campos do §7: `versão 2 · altura 8 · anterior 64 · merkle 64 · useful_root 64 · horário 8 · bits 4 · nonce 8` (2+8+64+64+64+8+4+8 = 222). Os nomes ficam gravados em mono.
- **A corrente:** o segmento `anterior` de cada bloco é **literalmente um feixe de luz** que sai do hash do bloco anterior.
- **Enquanto a pessoa rola:** a câmera percorre a corrente ao longo de uma `CatmullRomCurve3`. Um bloco novo nasce na ponta direita **a cada 8 batidas (7,74 s)** e passa pelas checagens na régua de máquina: `assinatura · nonce · saldo · rede · trabalho útil · Argon2id`. O item trabalho útil leva o rótulo "simulado neste navegador · real no nó".
- **Interação "Mexa num bloco antigo":**
  - A pessoa escolhe um bloco de 0 a 6 e edita o valor de uma transação num campo mono.
  - O SHA-512 é **recalculado de verdade**: os bytes de `merkle` trocam na régua e o hash muda.
  - O feixe até o próximo bloco **desencaixa**. Os blocos seguintes **perdem a luz em cascata**, 120 ms cada, e ganham o corte da lâmina.
  - O botão "Restaurar" faz tudo voltar e acender de novo.
- **Hover (desktop):** passar sobre um segmento mostra os bytes daquele campo.
- **Celular:** a régua fica na vertical.

#### `#nos`: A rede real (260svh)
Hoje o capítulo leva o selo "Planejado". Passa a ser **implementado · rede de teste local**.

- **p 0–0,1:** o A se desfaz em nós, usando o sistema de pontos existente no modo `rede`.
- **p 0,1–0,5, reprodução de um teste real (`um_no_sobrevivente_ressemeia_a_rede`):**
  - Os nós apagam para `--aco` até sobrar **um aceso**. As conexões renascem a partir dele.
  - Selo: `reprodução de um teste real · integracao.rs · commit a244716`.
  - A sequência vem de um arquivo gravado a partir do teste (seção 8). Enquanto esse traço não existir, o rótulo diz `[ilustração do teste]`.
- **p 0,55–0,8:** a câmera entra numa conexão, e a linha vira **um tubo de bytes cifrados rolando**.
  - Texto: "Quem está no meio vê isto." Literal: `Noise_XX_25519_ChaChaPoly_BLAKE2s` com o prólogo `AURON-WIRE-v2`.
  - Os bytes são AES-GCM real do navegador, com a nota "o nó usa ChaChaPoly".
  - Interação **"Trocar um byte"**: a lâmina corta o tubo, e aparece `conexão derrubada · intermediario_que_altera_derruba_a_conexao`.
- **p 0,85–1:** dois nós acesos lado a lado, `PC · windows-x86_64` e `celular · linux-arm64 (Termux)`, com a frase "minerando juntos" (commit c9ff264).
- **Ficha do nó:** altura, trabalho acumulado, pares e "conexão: cifrada". **Sai a "Reputação: ATIVA".**

#### `#seguranca`: Atacamos o próprio código (200svh)
- **O A vira corpo de prova** sob luz rasante. A lâmina desfere **um golpe por cenário de teste** (o número recontado; hoje são 17 funções em `integracao.rs`). Cada golpe é desviado e deixa um risco fino de luz na superfície.
- **Log sincronizado:** à esquerda, cada golpe acende uma linha de terminal em mono: `ok  bloco_forjado_sem_prova_e_recusado`. É uma lista de nomes reais, não animação inventada.
- **Parede de provas:** tabela de especificação com três colunas (afirmação | teste em mono | `crates/auron-net/tests/integracao.rs` com link "conferir"). Entram Conexão cifrada, Carteira com senha e Ataques com nós reais.
- **Revisão de ataque:** uma linha do tempo em régua, `11/09/2026 · 6 falhas encontradas → 12/09/2026 · 6 corrigidas, 1 teste cada`, com selo implementado.
- **Sybil (planejado):** vira uma linha da tabela com ○ e "abrir a simulação", carregada sob demanda.

#### `#aur`: Regras, não preço (160svh)
- **Sai:** a moeda girando, o texto circular e a curva de emissão com o ponto de luz subindo.
- **Entra:** o A **fatiado em estratos horizontais**, com `clippingPlanes` do core ou fatias instanciadas. Cada estrato tem **metade da espessura do anterior**.
- **A câmera desce** conforme `p` cresce: o movimento é para baixo e a luz vai esfriando. É o anti-hype.
- **Rótulos das fatias** (todos com `PROVISÓRIO`, vindos de `auron-consensus`): `altura 0–209 999 · 50 AUR/bloco`, `210 000–419 999 · 25`, `… · 12,5`.
- **Monumento:** "Sem preço." em wdth 62.
- **Ficha técnica em linhas:** teto 21 000 000 · 8 casas · bloco 120 s · halving a cada 210 000 blocos (≈ 292 dias a 120 s) · maturidade 100 blocos · mineração **trabalho útil + Argon2id 32 MiB**.
- **"O AUR não é":** marcadores de traço `--aco`, sem o × salmão.

#### `#mercado` (novo): Em construção (120svh)
- **O que vem de `#utrax`:** os 5 candidatos e as 7 etapas.
- **3D a meia luz:** 5 As pequenos instanciados. O escolhido acende, calcula, e **o pagamento só acende depois que a conferência passa**, com a referência "a mesma conferência do capítulo 03".
- **Régua de máquina:** `TAREFA · MERCADO · ESCOLHA · CÁLCULO · CONFERÊNCIA · PROVA · PAGAMENTO`.
- **Selo:** ◐ em desenvolvimento. O ◐ só aparece aqui, então o selo único deixa de esconder a parte pronta.

#### `#rodar` (novo): Rode um nó hoje (110svh)
- **O 3D dorme:** a renderização pausa. É o silêncio antes do ato IV.
- **Colunas 1–7:** a **gravação** de uma sessão real (`auron-no` iniciando, sincronizando e minerando), em texto estático completo com carimbos de tempo. **Sem efeito de digitação.** Selo `gravação de DD/09/2026 · commit xxxxxxx`.
- **Colunas 8–12, os 4 alvos em lista:**
  - `linux-x86_64`, `linux-arm64 (celular/Termux)`, `windows-x86_64`, `macos-arm64`;
  - tamanho e SHA-256 da Release, **só se a Release estiver publicada**; senão, "compile a partir do código", com link para `docs/RODAR-UM-NO.md`.
- **Carteira:** `AURON-CARTEIRA-v2 · Argon2id 64 MiB · 3 passadas · ChaCha20-Poly1305 · senha de 10+ caracteres`.

#### Ato IV: a linguagem de "ideia"

Nesses quatro capítulos o A vira **projeção**: só arestas prata a 35%, sem metal e **sem `--luz`**. A exceção é o rótulo `CONTA REAL` quando a conta é real: "a conta é de verdade, a rede é ideia".

Cada demonstração começa como um **quadro parado com o botão "Abrir a simulação"**, e o módulo JS só é importado nesse clique. Isso tira 4 módulos da carga inicial.

- **`#direct`:**
  - Dois contornos de A, Alice e Bob. A cápsula é um ponto prata que viaja.
  - Alternador `COM INTERNET / SEM INTERNET`.
  - **Orçamento sem internet** como régua de 20 marcas que se consome até 16.
  - "Tentar gastar duas vezes" provoca o corte.
  - Frase fixa: "Sem internet, o gasto tem limite e é conferido quando a rede volta."
- **`#resonance`:**
  - O contorno do A se espalha em 12 "cidades".
  - "Derrubar um nó da rota" dispara Dijkstra real, com o rótulo "algoritmo real, malha de exemplo".
  - O pacote se abre em duas visões lado a lado, a do destinatário e a de quem repassa. O conteúdo passa a ser **AES-GCM real**, em vez de bytes aleatórios.
  - Identidade e carteira usam a árvore `├─`, sem caixas.
  - O rádio vira **uma anotação** presa por um filete ao transporte: "Pesquisa: mandar pedaços por caminhos sem fio diferentes e juntar no destino."
- **`#fragmentacao`:**
  - O tabuleiro 16×16 vira um plano instanciado inclinado em perspectiva.
  - A pessoa digita o texto, que é cifrado com AES-GCM 256 real (4096 bytes = 256 × 16), e as casas se enchem.
  - As casas cujo centro cai dentro da silhueta do A ficam um pouco mais claras: **o A desenhado nos seus próprios bytes cifrados**, com a nota "o desenho é só luz, os bytes são os do seu texto".
  - "Corromper" troca um bit, a casa leva o corte, e a reconstrução é recusada **pela própria etiqueta do GCM**, com a mensagem mostrada.
  - O armazenamento vira uma frase e uma régua com 6 marcas: `ARQUIVO · CIFRA · CORTE · ESPALHA · CONFERE · RECONSTRÓI`.
  - **Celular:** o tabuleiro cabe em 100% da largura (casa = `calc((100vw − 2×margem)/16)`) e o `min-width: 25rem` sai.
- **`#inteligencia`:**
  - As 4 abas viram 4 frases curtas em sequência.
  - **Saem as barras oscilando e a razão "1,23×"**, que liam como cotação.
  - 3D mínimo: o contorno do A duplicado como esquema de agente. Selo ○ planejado.

#### `#escala`: Você está aqui (260svh)
A rolagem afasta a câmera por degraus verdadeiros:

1. **Um nó no seu computador:** o A aceso.
2. **Dois nós, PC e celular:** dois pontos acesos e o rótulo "existe · nos testes".
3. **Rede de teste pública:** um anel tracejado `--aco`, com **`você está aqui → próximo passo`**.
4. **Auditoria externa:** contorno fraco.
5. **Rede principal:** quase invisível.

A câmera **para no degrau 2/3**. O resto da rolagem é vazio escuro com a frase "Daqui para frente, ainda não existe." Sai "GLOBAL INFRASTRUCTURE".

#### `#onde`: Onde estamos (140svh, com o antigo `#caminho`)
- **O mapa:** o A deitado, visto de cima. Os **6 ramos** saem dele como sulcos gravados.
- **O comprimento aceso** de cada ramo é proporcional a itens implementados ÷ total, calculado de `data.js`. Sem datas.
- **A tabela abaixo** traz os glifos de estado, e as colunas atualizadas passam para "Implementado": Noise XX, carteira com senha e programas para 4 sistemas.
- **Sai** a data "dezembro de 2026".

#### `#aberto`: Construído em público (110svh)
- **Números recontados** em ficha: `testes Python · testes Rust · cenários de rede com nós reais · vetores em vectors/`. Os números estáticos, **sem contagem rolando**, porque rolagem de número lembra cotação.
- **Os 14 crates em mono:** `auron-block · auron-chain · auron-codec · auron-consensus · auron-crypto · auron-net · auron-no · auron-pow · auron-state · auron-store · auron-tx · auron-types · auron-usefulpow · auron-wire`.
- **Downloads em linhas:** zip com commit, documento e roteiro, mais o GitHub.

#### `#apoio` (100svh)
- **Placa de grafite:** uma das 6, com chanfro e o QR.
- **Endereço:** `bc1` em mono `--t-dado`, **quebrado em grupos de 4 caracteres** para conferir a olho, com o botão "Copiar".
- **"Sem contrapartida":** em `--t-corpo`, não em microtexto.

#### `#fim` (140svh)
- O A volta inteiro, com luz cheia, por 2 batidas.
- A luz **apaga** e sobra só a silhueta em aresta.
- Aparece, centralizado (um dos 2 centros permitidos), **"Não confie. Confira."** em monumento wdth 125, com o subtítulo "Código aberto. Rede de teste. Nenhuma promessa." e o link "Conferir o código".
- **Rodapé:** mínimo, sem o selo de qualidade.

---


## 4. Direção de movimento (síntese)

### 4.1 Mecânica da rolagem

Um único rolador de penda um único `requestAnimationFrame`. Nesse quadro:
  1. descobre o plano ou a pose ativa pela linha de 50% da tela, usando só `scrollY` e os números guardados, sem `getBoundingClientRect` por quadro;
  2. calcula `p`;
  3. chama `mundo.definirPlano(id, p)` ou `mundo.definirPose(id, p)`;
  4. atualiza **no máximo 6** elementos do DOM, só com `transform` e `opacity`: passo ativo, leitura, régua de progresso e código na barra. Nenhuma variável CSS herdada por uma subárvore grande.

```js
// main.js
const trechos = [...document.querySelectorAll("[data-plano],[data-pose]")].map((el) => ({
  el, id: el.dataset.plano || el.dataset.pose, plano: !!el.dataset.plano, topo: 0, altura: 0 }));
function medir() { for (const t of trechos) { t.topo = t.el.getBoundingClientRect().top + scrollY; t.altura = t.el.offsetHeight; } }
let pedido = false;
addEventListener("scroll", () => { if (!pedido) { pedido = true; requestAnimationFrame(rolar); } }, { passive: true });
function rolar() {
  pedido = false;
  const meio = scrollY + innerHeight * 0.5;
  const t = trechos.find((x) => meio >= x.topo && meio < x.topo + x.altura) || trechos[0];
  const p = Math.min(1, Math.max(0, (scrollY - t.topo) / Math.max(1, t.altura - innerHeight)));
  t.plano ? mundo.definirPlano(t.id, p) : mundo.definirPose(t.id, p);
  capitulos.aoRolar(t.id, p);        // cada capítulo ativo mexe em até 6 elementos
}
```

- **Capítulos sob demanda:** um `IntersectionObserver` com `rootMargin: "100% 0px"` importa `sections/<id>.js` uma única vez. Os módulos do ato IV só carregam no `toggle` do `<details>`.

### 4.2 Passagens

| Tipo | Quando | Como |
|---|---|---|
| **Fusão** | Entre capítulos do mesmo ato | 1 batida (968 ms). Morph na GPU com atraso por nó proporcional à projeção na direção da lâmina, de modo que a troca varre a cena a 26,2°. |
| **Corte seco** | Na claquete de cada ato | `#mundo.corte { opacity: 0 }`: 121 ms sem transição na ida, 242 ms na volta. A forma nova é montada nesse intervalo, sem morph visível. A trilha recua 6 dB por 2 batidas. |
| **Corte por semelhança** | `#topo` → `#visao` (aresta da lâmina → linha horizontal); `#visao` → `#nucleo` (placa 6×6 → A); `#aur` → claquete do ato III (último estrato, fino como linha → filete da claquete) | A pose final de um é a pose inicial do outro |

### 4.3 Entradas

- **Títulos:** as linhas sobem de máscaras (`translateY(105%)` → 0) em 484 ms, com `--curva-filme` e 80 ms entre linhas. O disparo é por `IntersectionObserver` (`rootMargin: "0px 0px -12% 0px"`), uma vez só.
- **Blocos `.revela`:** partem de `opacity: .35` e `translateY(12px)` e chegam em 484 ms. Nunca partem de opacidade 0.
- **Leituras MEDIDO:** o hex aparece em blocos de 16 caracteres, 60 ms cada, na ordem em que foi calculado. Nunca há embaralhamento falso de dígitos.
- **Nunca animar:** `filter`, `backdrop-filter`, `clip-path`, `mask`, `box-shadow`, `width`, `height`, `top` ou `left`. `text-shadow` também não, com uma exceção: a transição de cor do "=" da abertura.

### 4.4 Microinterações

| Elemento | Comportamento |
|---|---|
| Botão principal | No hover, o fundo vai de `--osso` a `--prata` em 121 ms; ao pressionar, `translateY(1px)`. |
| Link | O sublinhado cresce da esquerda (`scaleX`) em 242 ms. |
| Copiar hash ou endereço | Varredura `steps(n)` de `--chumbo` para `--osso` em 484 ms, com o rótulo "copiado". |
| Recusa | O corte se desenha em 242 ms (`--curva-maquina`); a luz do elemento cai em 484 ms; `audio.abafar()`. |
| Aceite | A luz acende em 484 ms. Sem confete e sem escala saltando. |
| Hover num dado | O mesmo campo acende em todas as vistas: régua, ficha e 3D (`mundo.pulso("destaque", …)`). |
| Chave | Alavanca em 242 ms, com o estado escrito ao lado. |
| Foco | `outline: 2px solid var(--osso)` com `outline-offset: 3px`, sem animação. |

### 4.5 Cursor

- **Sempre nativo.** Sem cursor próprio e sem botão magnético.
- **Só em desktop HIGH ou MEDIUM, e só no `#topo` e no `#fim`:**
  - a posição do mouse vira o uniform `uLampada`: o reflexo do matcap acompanha o cursor, até ±8°, como uma lâmpada de inspeção sobre a peça;
  - a câmera ganha parallax de ±3 px;
  - a leitura é feita no rAF do mundo, não no `mousemove`.

### 4.6 Som

A trilha própria de `audio.js` (62 BPM, lá menor, sintetizada) continua **desligada por padrão**. Liga só com toque, e o estado aparece escrito.

- **Relógio (`js/relogio.js`):**
  - `batida()` conta batidas desde o carregamento;
  - usa `audio.tempo()` quando a trilha toca e `performance.now()` a cada 968 ms quando não toca.
- **O que segue a batida (nunca a rolagem):**
  - o bloco novo da cadeia, a cada 8 batidas;
  - os passos do mercado, em meias batidas.
- **Cuidado com `aoBatida`:** ele chega até 3,3 s antes da batida. O consumidor agenda com `setTimeout((t − audio.tempo()) × 1000)`.
- **Funções novas em `audio.js`:**
  - `filtroAto(freq)`: `BiquadFilter` passa-baixa entre `mestre` e `compressor`, com rampa de 2 batidas. Ato II: 18 kHz. Ato III: 8 kHz. Ato IV: 2,4 kHz. Epílogo: 12 kHz.
  - `abafar()`: 400 Hz por 1 batida na recusa, na conexão cortada e no bloco quebrado.
  - `recuar(db)`: −6 dB na claquete de ato; −12 dB em `#apoio`.
- **Sem acréscimos:** nenhum som de interface e nenhum arquivo de áudio além da narração. A narração continua abaixando a trilha.

### 4.7 Versão reduzida

Vale para `@media (prefers-reduced-motion: reduce)` e para `html.calmo` (escolha em Ajustes).

- **Página:**
  - `.plano` perde a altura extra e `.palco` deixa de ser `sticky`; cada palco mostra o estado final dos passos;
  - a abertura mostra direto o quadro final (1.7).
- **Mundo:**
  - sem laço de animação;
  - a cada troca de plano ou pose, `desenharParado()` desenha um único quadro da pose `still` do roteiro;
  - troca instantânea, sem fusão.
- **Demonstrações:**
  - tudo passo a passo, com "Próximo passo";
  - nada roda sozinho, e o bloco novo da cadeia só nasce por botão;
  - a cascata de blocos apaga de uma vez;
  - o espectrograma é estático (32 linhas);
  - `#escala` vira 5 botões.
- **Correção de defeito:** `objeto()` passa a chamar `desenharParado()` no modo calmo e força `entrada = 1`. Hoje o vídeo, nesse modo, mostra o fundo sem o objeto.
- **Critério de aceite:** 1 s depois do carregamento,
  - `document.getAnimations().length === 0`, sem contar transições de hover;
  - `__auron.mundo.rodando === false`.

---

## 5. Motor 3D

### 5.1 Arquivos e API

```
site/js/world/
  mundo.js       fachada: cria 3D ou 2D e troca a quente de 3D para 2D sem os capítulos saberem
  motor.js       renderer, laço, teto de FPS, governador, dormir/acordar, recorte, compor()
  pontos.js      Points e LineSegments com morph na GPU e tetos de tamanho e de cobertura
  formas.js      11 formas com índices embaralhados (FORMAS_VERSAO = 2)
  heroi.js       o A: extrusão, material, estratos, contorno, alinhamento ao DOM, golpe
  roteiro.js     6 planos com quadros-chave e 13 poses (só dados)
  props/placas.js · props/corrente.js · props/estratos.js
  pos.js         só HIGH: bloom e composição
  estudio.js     objetos do vídeo, movidos de world.js (linhas 497–1141), sem mudar a lógica
  world.js       monta as peças e exporta criarMundo()
  fallback2d.js  poses em SVG
```

**Formas (11):** `poeira`, `chuva`, `placa6`, `logo`, `nucleo`, `cadeia`, `rede`, `grade`, `malha`, `global`, `andaime`.

| Método | Faz |
|---|---|
| `nivel` | `"HIGH"`, `"MEDIUM"`, `"LOW"` ou `"2D"` |
| `definirPlano(id, p)` | Interpola os quadros-chave de um dos 6 planos |
| `definirPose(id, p)` | Fusão para a pose de um capítulo; `p` opcional (usado no deslize do `#aur`) |
| `encaixar(id, elemento)` | Registra a âncora DOM que o 3D segue naquele plano ou pose |
| `corte()` | Corte seco entre atos |
| `golpe({ porTempo })` / `golpe({ p })` | Giro de 22° do A no `#topo`; ignora se não couber |
| `pulso(nome, dados)` | `"utrax-recusa"`, `"cadeia-quebra"`, `"seguranca-risco"`, `"destaque"` |
| `foco(indices)` | `#nos`: acende os nós do teste |
| `recorte(x, y, w, h)` | Tesoura: no celular, desenha só a metade de cima |
| `dormir(motivo)` / `acordar(motivo)` | Para ou retoma o rAF; os motivos se acumulam num `Set` |
| `aquecerEstudio()` | PMREM e `compileAsync` do vídeo (HIGH e MEDIUM) |
| `trocarPara2D(motivo)` | Libera o renderer e passa a fachada para o 2D |
| `relatorio()` | Só com `?debug=1`: estatísticas por plano |
| Do vídeo, mantidos | `somenteObjetos`, `objeto`, `hudEscala`, `introducao`, `fps`, e `definirZoom(v)` como apelido de `definirPlano("escala", v)` |

O `fallback2d.js` implementa todos esses métodos: vazios, ou trocando a pose em SVG.

### 5.2 Chão firme, antes de avaliar qualquer visual

1. **Detecção (`quality.js`):**
   - `getContext("webgl2", { failIfMajorPerformanceCaveat: true })`. Se voltar nulo, é 2D, **sem importar** `three.module.min.js`;
   - perfis, tetos de megapixel e a regra do celular estão em 5.7;
   - a escolha de `Ajustes` tem prioridade; `auron-imagem=auto-2d` expira em 7 dias.
2. **Teto de FPS por agenda.** Hoje MEDIUM e LOW rodam a 30 e 20 fps reais em telas de 60 Hz.
   ```js
   const intervalo = 1000 / perfil.fps;
   function quadro(agora) {
     if (dormindo) return;                       // acordar() chama requestAnimationFrame de novo
     requestAnimationFrame(quadro);
     if (agora - ultimo < intervalo - 2) return; // 2 ms de tolerância ao vsync
     ultimo = agora - ultimo > intervalo * 2 ? agora : ultimo + intervalo;
     passo(agora);
   }
   ```
3. **Governador** com histerese e que volta a subir (5.9).
4. **Índices embaralhados.** Qualquer prefixo passa a ser uma amostra uniforme, então `setDrawRange` nunca mais corta hemisférios nem metade do 16×16:
   ```js
   function permutacao(n, semente) {
     const p = Uint16Array.from({ length: n }, (_, i) => i), r = mulberry32(semente);
     for (let i = n - 1; i > 0; i--) { const j = Math.floor(r() * (i + 1)); [p[i], p[j]] = [p[j], p[i]]; }
     return p;
   }
   // cada forma é gerada em ordem natural e depois reordenada: destino[i*3+k] = forma[perm[i]*3+k]
   ```
5. **Vizinhos pré-calculados:**
   - `site/tools/gerar-vizinhos.html` importa `world/formas.js`, calcula 3 vizinhos da forma `rede` para N = 240, 360, 700 e 1500, e oferece `site/dados/vizinhos-v2-{N}.bin` (`Uint16Array`, N × 3; cerca de 17 KB no total);
   - o motor busca o arquivo com `fetch` (`connect-src 'self'`);
   - se falhar ou o tamanho não bater, calcula em fatias de 8 ms com `requestIdleCallback` e mostra a rede sem linhas até terminar.
6. **Rotação:** antes de voltar a 0, normalizar `rotation.y` para o intervalo (−π, π]. Hoje ela "desenrola" várias voltas.
7. **Carregamento paralelo:** `main.js` dispara `import("./world/world.js")` **antes** de esperar o i18n.
8. **Nada alocado por quadro:** saem `set([...])`, `slice` e closures de `forEach`; entram buffers reutilizados e envio parcial com `addUpdateRange`.
9. **Dormir:**
   - motivos: aba oculta; `#rodar`, `#onde`, `#aberto` e `#apoio` na tela; uma camada DOM opaca cobrindo 98% ou mais da tela (medido por `IntersectionObserver`);
   - no LOW, depois que a forma converge e sem pacotes, o mundo cai para 10 fps.
10. **Tamanho do canvas:** `#mundo { height: 100lvh }` e `ResizeObserver`. Com ponteiro grosso, ignora mudanças de altura menores que 120 px, então não realoca o framebuffer quando a barra de endereço aparece ou some.
11. **Vídeo e modo calmo:** `objeto()` corrigido (4.7). Objetos inativos do HUD saem do grafo (`hud.remove`) em vez de `visible = false`, para `updateMatrixWorld` não percorrê-los.
12. **Contagem de desenho:** `renderer.info.autoReset = false`, com `reset()` a cada quadro, para o HUD contar certo com pós.
13. **Cores do 3D:**
    - `RECUSADO` passa a `0x25272C`, com brilho 0 e as metades se afastando 2 unidades;
    - `FRIO` passa a prata;
    - `ACEITO` passa a luz;
    - `OURO` passa a `0xF3E2C4`.

### 5.3 O herói

**Geometria**
- `Shape` a partir dos dois paths do `logo.svg`, com Y invertido (`y' = 100 − y`) e coordenadas de 0 a 100.
- **A geometria não é transladada:** o centro fica num `Object3D` pivô em (−50, −51, 0).
- Extrusão: corpo com profundidade 9; lâmina com 10,5, saltando 0,75 de cada lado, como peça encaixada e sem z-fighting.
- Bevel com `bevelThickness .8`, `bevelSize .6` e `bevelSegments` 3, 2 ou 1; `curveSegments` 24, 12 ou 8 (HIGH, MEDIUM, LOW).
- Corpo e lâmina são unidos à mão num único `BufferGeometry`, com o atributo `aCaixa` (vec4 da caixa de cada path, em coordenadas invertidas): corpo `(5, 8, 95, 94)`, lâmina `(8, 10, 78.5, 43)`. Resultado: 1 draw call.

**Material**
- `MeshMatcapMaterial` com `toneMapped: false`.
- **Matcap gerada em `<canvas>`** (256² no HIGH, 128² nos demais), com `colorSpace = SRGBColorSpace`:
  - fundo radial de `#9EA2A9` a `#16171A`;
  - mancha quente em (0,32; 0,28), raio 0,22, `#FFF3DF` a 90%;
  - aro prata em (0,78; 0,62), raio 0,3, `#D6D8DC` a 60%;
  - faixa de estúdio horizontal em y 0,36, altura 0,04, `#FDFDFB` a 70%. É ela que corre pelo chanfro quando a peça gira.
  - A variante **fria** (AUR) não tem a mancha quente; a **fosca** (estratos) tem o contraste pela metade.
- O rosto do A reproduz exatamente o `linearGradient` do SVG (`x1=0 y1=0 x2=1 y2=1`, por caixa de cada path). Com o A a 0°, 3D e SVG ficam iguais:

```js
const RAMPA = `
vec3 srgbLinear(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c)); }
vec3 rampaMetal(float t) {
  vec3 c0 = vec3(.969,.969,.961), c1 = vec3(.706,.722,.745), c2 = vec3(.992,.992,.984), c3 = vec3(.478,.494,.525), c4 = vec3(.839,.847,.863);
  vec3 c = t < .34 ? mix(c0, c1, t / .34) : t < .52 ? mix(c1, c2, (t - .34) / .18) : t < .76 ? mix(c2, c3, (t - .52) / .24) : mix(c3, c4, (t - .76) / .24);
  return srgbLinear(c);
}`;
material.onBeforeCompile = (s) => {
  Object.assign(s.uniforms, uniformsDoHeroi); // uVarredura, uForcaVarredura, uExposicao, uQuente, uLampada
  s.vertexShader = s.vertexShader
    .replace("#include <common>", "#include <common>\nattribute vec4 aCaixa; varying vec2 vB; varying float vFrente;")
    .replace("#include <begin_vertex>", "#include <begin_vertex>\nvB = (position.xy - aCaixa.xy) / (aCaixa.zw - aCaixa.xy);\nvFrente = step(.9, abs(normal.z));");
  s.fragmentShader = s.fragmentShader
    .replace("#include <common>", "#include <common>\nuniform float uVarredura, uForcaVarredura, uExposicao; uniform vec3 uQuente; varying vec2 vB; varying float vFrente;" + RAMPA)
    .replace("#include <opaque_fragment>",
      "outgoingLight = mix(outgoingLight, rampaMetal((vB.x + 1. - vB.y) * .5), vFrente);\n" +
      "outgoingLight += uQuente * smoothstep(.05, 0., abs(dot(vB, vec2(.897, .442)) - uVarredura)) * uForcaVarredura;\n" +
      "outgoingLight *= uExposicao;\n#include <opaque_fragment>");
};
```

**Alinhamento ao DOM** (`PerspectiveCamera` com FOV 18° em (0, 0, 100), olhando para a origem):

```js
export function alinhar(heroi, elemento, camera, alturaCanvasCss) {
  const r = elemento.getBoundingClientRect();
  const s = (2 * camera.position.z * Math.tan(THREE.MathUtils.degToRad(camera.fov / 2))) / alturaCanvasCss; // mundo por px CSS
  const escala = (r.height / 100) * s;                              // a caixa do <svg> tem 100 unidades
  heroi.scale.setScalar(escala);
  heroi.position.set((r.left + r.width / 2 - innerWidth / 2) * s, -(r.top + r.height / 2 - alturaCanvasCss / 2) * s, -5.25 * escala);
}
```

Usa a altura CSS do canvas (100lvh), não `innerHeight`. Roda na entrada do plano, em `document.fonts.ready` e no `resize`.

**Golpe (máquina de estados)**
1. `svg` → `pronto`: mundo carregado, sem modo calmo, perfil diferente de 2D. O A 3D é desenhado alinhado **sob** o SVG.
2. `pronto` → golpe por tempo: t ≥ 3,2 s, perfil HIGH ou MEDIUM, `scrollY < 4`.
   - `uVarredura` vai de −0,2 a 1,2 em 484 ms;
   - aos 242 ms o SVG vai a `opacity: 0` em 121 ms;
   - `rotation.y` vai de 0 a −22° em 968 ms, com `--curva-filme`;
   - no HIGH, a força do bloom sobe de 0 a 0,55 no mesmo intervalo, para o brilho não denunciar a troca.
3. `pronto` → golpe pela rolagem, em qualquer perfil 3D: `rotation.y = −22° × smoothstep(0,04; 0,3; p)`. É reversível. A troca SVG → 3D acontece em p = 0,04, sob a varredura.
4. Depois do golpe por tempo, a rotação fica em −22° até p 0,55. De 0,55 a 1 vem o mergulho (3.4, capítulo 00).

**Estratos (`props/estratos.js`)**
- 7 faixas com altura 86/7 cada, de y_svg 6 a 92. A profundidade da faixa k é 9 / 2ᵏ.
- **Corpo:** interseção analítica, porque o corpo é poligonal. Acima de y_svg = 42 é um trapézio entre as bordas externas; abaixo, dois trapézios (as pernas) entre bordas externa e interna. Faixas que cruzam 42 são divididas.
- **Lâmina:** recorte Sutherland–Hodgman do contorno `shape.extractPoints(8)` contra a faixa. A faixa é convexa e o resultado é uma tira conexa.
- Cada peça vira `ExtrudeGeometry` sem bevel. Todas são unidas à mão num único `BufferGeometry`, com matcap fosca e fria.

**Contornos**
- Ato IV e `#fim`: `new EdgesGeometry(geometriaDoA, 20)` com `LineBasicMaterial` prata a 35% e `toneMapped: false`.
- `#mercado`: as arestas copiadas 5 vezes num único `BufferGeometry`, gerando 1 `LineSegments`.

### 5.4 Pontos e linhas

- **Atributos:** `aDe`, `aPara` (vec3), `aAtraso`, `aTam`, `aSemente`, `aBrilho` (atualizado só quando muda), `aPapel`.
- **Uniforms:** `uMorph`, `uTempo`, `uTamMax`, `uEscala`, `uCobertura`, `uExposicao`, `uQuente`, `uHalo`, `uQueda`, `uFoco`, `uOnda`, `uRevela`.
- **Morph compartilhado** por `Points` e `LineSegments`; as linhas usam os mesmos atributos pelo índice:

```glsl
vec3 posicaoMorph() {
  float k = clamp((uMorph - aAtraso * .45) / .55, 0., 1.);
  k = k * k * (3. - 2. * k);
  return mix(aDe, aPara, k) + vec3(-.442, .897, 0.) * sin(k * 3.14159265) * uArco;
}
// Points: tamanho com teto e ponto invisível sem custo de preenchimento
gl_PointSize = min(aTam * uEscala * uCobertura * cintila * (1. + 1.1 * aBrilho) / -mv.z, uTamMax);
if (vVis < .01) gl_PointSize = 0.;
```

- **Troca de forma:**
  - `uMorph` vai de 0 a 1 em 968 ms;
  - `aAtraso` = projeção de `aPara` em (0,897; 0,442), normalizada de 0 a 1, mais ±0,06 de `aSemente`;
  - se a troca for interrompida, a CPU avalia a mesma função para N ≤ 1500 (cerca de 0,2 ms) e copia o resultado para `aDe`;
  - acaba o envio de `position` a cada quadro.
- **Pacotes:**
  - cada pacote carrega `de`, `para` e `atraso` das duas pontas da aresta, mais `aFase` e `aVel`;
  - no shader, `t = fract(aFase + uTempo * aVel)`;
  - quando `t` dá a volta, só aquele pacote troca de aresta, com `addUpdateRange` do trecho.
- **Destaque sob o cursor** (só `#nos`, desktop HIGH ou MEDIUM): a CPU tira uma cópia das posições 10 vezes por segundo.
- **Cobertura:** na entrada de cada plano ou pose, com a câmera do quadro-chave:
  ```js
  let soma = 0;
  for (let i = 0; i < N; i++) { const d = Math.min(tamMax, aTam[i] * escala / profundidade(i)); soma += 0.785 * d * d; }
  uCobertura.value = Math.min(1, Math.sqrt(teto / (soma / (larguraPx * alturaPx))));
  ```
- **Luz nos pontos:**
  - `uHalo = 0` no ato IV (só o miolo, 1,5 px);
  - `uQuente` recebe `--luz` ou, no `#aur`, `--prata`;
  - `uExposicao` substitui o `.veu` com 50% de preto.
- **Vinheta:**
  - no HIGH, dentro da composição;
  - nos demais perfis, uma camada CSS estática `.vinheta` (z 1) com gradiente radial, que não anima.
  - Saem `.veu` e `.mundo-ao-fundo`.

### 5.5 Cenas por capítulo

Draw calls incluem pontos, linhas e pacotes quando visíveis (formato H / M / L).

| Capítulo | Herói (A) | Pontos | Âncora e quadros-chave | Técnica | Draw calls |
|---|---|---|---|---|---|
| 00 `#topo` (plano) | Aço aceso. Golpe de 22°. De p 0,55 a 1, mergulho: rola 26,2° e FOV vai de 18° a 32°. | `poeira`: 40% revelado, halo quente fraco | `svg.a-grande` | Extrusão + matcap + degradê + varredura | 4 / 4 / 3 |
| 01 `#visao` (plano) | Oculto até p 0,8; depois se forma só com pontos | `chuva` → `placa6` (p 0,45) → `logo` (p 0,8), exposição 0,4 | `.espectrograma` | Morph na GPU, `uQueda` | 2 / 2 / 2 |
| 02 `#nucleo` (pose) | Deitado (X −68°), FOV 22° | `nucleo` a 30% | `.nucleo-a`; âncoras projetadas | `Vector3.project` na convergência | 3 / 3 / 2 |
| corte seco (ato II) | | | | | |
| 03 `#utrax` (plano) | Desdobra na placa A; placas B e C; feixes | `poeira` a 15% | `.grade-a`, `.grade-b`, `.grade-c`; quadros-chave em p 0, 0,12, 0,45, 0,65, 0,8 | `InstancedMesh` (108 cubos; quadrados no LOW), quads instanciados | 4 / 4 / 3 |
| 04 `#cadeia` (plano) | 7 réguas e 6 pinos | `cadeia` a 30% | `.corrente`; câmera numa `CatmullRomCurve3` de p 0 a 1 | `InstancedMesh` (56 segmentos), cilindros de 8 lados | 4 / 4 / 3 |
| 05 `#nos` (plano) | Vira pó de p 0 a 0,1 | `rede` com `aPapel`: 4 nós acesos, o resto a 15% | `.rede-teste`; passos em 0,1 / 0,25 / 0,4 / 0,55 / 0,7 / 0,85 | Morph, `foco()`, linhas | 4 / 4 / 3 |
| 06 `#seguranca` (pose) | Corpo de prova sob luz rasante; um risco por grupo | `rede` a 40% | `.corpo-de-prova` | Matcap com luz direcional; `LineSegments` (sem riscos no LOW) | 4 / 3 / 3 |
| 07 `#aur` (pose com deslize) | 7 estratos, aço frio, sem luz quente | Exposição 0 | `.estratos`; a câmera desce com p | Geometria de estratos unida | 2 / 2 / 2 |
| corte seco (ato III) | | | | | |
| 08 `#mercado` (pose) | 5 As em andaime, 1 revestido pela metade | `andaime` a 45% | `.candidatos` | Arestas unidas + 1 malha | 3 / 3 / 3 |
| 09 `#rodar` | **Dorme** | — | — | — | 0 |
| corte seco (ato IV) | | | | | |
| 10–13 (poses) | Contorno do A (duplicado em 10 e 13) | `malha`, `grade` ou `rede` em contorno a 25%, `uHalo` 0 | `.desenho-*` | `EdgesGeometry`, pontos sem halo | 3 / 3 / 2 |
| corte seco (epílogo) | | | | | |
| 14 `#escala` (plano) | — | `global` em contorno; 1 e depois 2 nós acesos | `.degraus`; para em p 0,5 | Círculos da esfera em `LineSegments` | 3 / 3 / 3 |
| 15–17 | **Dorme** | — | — | — | 0 |
| 18 `#fim` (pose) | Aço aceso por 2 batidas → exposição 0 → só a aresta | `poeira` a 20% | `.fim-a` | Matcap + `EdgesGeometry` | 4 / 3 / 3 |

### 5.6 Pós-processamento (só HIGH)

1. Cena → `rtCena`: `WebGLRenderTarget` com `samples: 2`, RGBA8 e depth. O contexto é criado sem `antialias` e com `depth: false`.
2. `rtCena` → `rtBrilho` a 1/4: limiar de luminância 0,82.
3. Desfoque horizontal, 9 amostras, para `rtA` (1/4).
4. Desfoque vertical, 9 amostras, de volta para `rtBrilho`.
5. **Composição na tela:** cena + brilho × força (0,55, ou a rampa do golpe), vinheta 0,28 e grão 0,03 com semente `quadro % 4`.
   - Duas variantes de `ShaderMaterial`: `composicaoPagina` (só `#include <colorspace_fragment>`) e `composicaoVideo` (`#include <tonemapping_fragment>` e `#include <colorspace_fragment>`, ativa com `somenteObjetos(true)`).
   - No r170, o tone mapping não se aplica a render target, então os objetos PBR do vídeo precisam dele na passada final.

- **Tipo do brilho:** `rtBrilho` e `rtA` usam `HalfFloatType` só se existir `EXT_color_buffer_half_float` ou `EXT_color_buffer_float`; senão, `UnsignedByteType`. Testar com `?debug=semfloat`.
- **Função única `compor()`:** substitui os três `renderer.render` atuais (quadro normal, vídeo e parado) e **termina sempre na tela**, para o `drawImage` do vídeo, logo depois dela, nunca pegar quadro preto.

### 5.7 Orçamento por perfil

| Métrica | HIGH | MEDIUM | LOW | Celular | 2D |
|---|---|---|---|---|---|
| Quem entra | WebGL2 sem ressalva, mais de 4 núcleos, 8 GB ou mais, sem integrada antiga, ponteiro fino | WebGL2 sem ressalva, 4 núcleos ou mais, 4 GB ou mais, sem integrada antiga | WebGL2 sem ressalva nos demais casos (inclui "Intel HD Graphics" sem número, Atom x5) | Ponteiro grosso e largura < 760 px: MEDIUM se a GPU não casar com `mali-[34]\|adreno \(tm\) [1-4]\d\d` e houver 6 GB ou mais; senão LOW | Sem WebGL2, WebGL2 só por software, "Sem 3D", ou queda automática |
| Pontos (N) | 1 500 | 700 | 240 | 360 / 240 | — |
| Pacotes | 90 | 40 | 8 | 12 / 8 | — |
| Segmentos da rede | ≈ 2 980 | ≈ 1 360 | ≈ 470 | ≈ 700 / 470 | — |
| Triângulos por quadro | ≤ 12 000 | ≤ 5 000 | ≤ 1 800 | ≤ 1 800 | — |
| A: `curveSegments` / `bevelSegments` | 24 / 3 | 12 / 2 | 8 / 1 | 8 / 1 | SVG |
| Draw calls | ≤ 10 + 4 de pós | ≤ 8 | ≤ 6 | ≤ 6 | — |
| dpr máximo | 1,5 | 1,25 | 1 | 1,5 / 1 | 1,5 |
| Escala do backing store | 1 | 0,9 | 0,8 | 0,75 | — |
| Teto de megapixels | 4,5 MP | 1,8 MP | 1,0 MP | 0,9 / 0,6 MP | — |
| FPS alvo | 60 | 30 | 30 (piso 20) | 30 | Só nas trocas de pose (opacidade CSS) |
| `gl_PointSize` máximo | 28 px | 18 px | 12 px | 14 / 12 px | — |
| Cobertura máxima (Σ área ÷ tela) | 1,2 | 0,6 | 0,35 | 0,45 / 0,35 | — |
| Antialias | RT com MSAA 2× | MSAA do contexto | Não | Não | — |
| Pós | Bloom a 1/4, vinheta, grão | Não | Não | Não | — |
| Matcap | 256² | 128² | 128² | 128² | — |
| Memória de GPU estimada | ≤ 140 MB | ≤ 70 MB | ≤ 30 MB | ≤ 40 MB | — |
| Tempo de GPU por quadro (`?debug=gpu`, com `gl.finish()`) | ≤ 10 ms | ≤ 18 ms | ≤ 25 ms | ≤ 25 ms | — |
| CPU do mundo por quadro | ≤ 3 ms | ≤ 2 ms | ≤ 2 ms | ≤ 2 ms | ≤ 1 ms |
| Montagem antes do 1º quadro | ≤ 80 ms | ≤ 50 ms | ≤ 40 ms | ≤ 40 ms | ≤ 10 ms |
| Compilação de shaders | `compileAsync` em ocioso, depois da abertura | Idem | Idem | Idem | — |

Escala efetiva = `min(escala do perfil, √(teto de MP ÷ (largura × altura × dpr²)))`.

Conta de memória: HIGH a 4,5 MP ≈ 72 MB de MSAA + 18 de resolve + 7 de brilho + 18 da tela ≈ 115 MB. MEDIUM a 1,8 MP com MSAA 4× ≈ 65 MB. LOW a 1,0 MP ≈ 8 MB.

### 5.8 O que cai em cada perfil

- **MEDIUM perde:** pós, grão, lâmpada de inspeção fora do desktop, matcap de 256² e poeira no palco do vídeo. O estúdio do vídeo continua com PMREM.
- **LOW perde, além disso:**
  - golpe por tempo (vira golpe pela rolagem);
  - cubos (viram quadrados);
  - riscos da segurança e destaque sob o cursor;
  - pacotes acima de 8;
  - estúdio do vídeo com PMREM (vira matcap);
  - laço a 30 fps depois que a forma converge (cai para 10 fps).
- **Celular perde, além disso:**
  - hover e retícula;
  - o palco 3D fica na metade de cima nos capítulos com demonstração (`recorte(0, 0, 1, .5)`), com a demo DOM opaca embaixo;
  - o mundo dorme quando o palco sai da tela.
- **2D perde todo o WebGL** e ganha poses em SVG por ato (5.10). A abertura e todas as demonstrações, que são DOM, continuam inteiras.

### 5.9 Governador

- **Janela:** 60 quadros desenhados.
- **Ignora:**
  - os 90 primeiros quadros;
  - a abertura (até 5 s);
  - 1,5 s depois de cada troca de plano ou pose;
  - aba oculta;
  - a abertura do vídeo.
- **Rebaixa um degrau** se a mediana do intervalo passar de 1,5 × o intervalo-alvo em duas janelas seguidas.
- **Sobe um degrau** se o p95 ficar abaixo de 1,15 × o alvo por 8 s **e** o último rebaixamento tiver mais de 20 s.
- **Degraus, em ordem:**
  1. pós desligado (HIGH);
  2. escala −0,1 por passo, até 0,7;
  3. `uTamMax × 0,75`;
  4. N pela metade (`setDrawRange` sobre índices embaralhados);
  5. pacotes em 0;
  6. linhas em 50%;
  7. só contorno;
  8. 2D.
- **Queda para 2D:** no último degrau, com p95 acima de 50 ms por 3 s, chama `trocarPara2D("automatico")` e grava `auron-imagem=auto-2d`. O rodapé mostra "Imagem: leve (escolhida automaticamente)", com a opção de tentar "Plena".
- **Registro:** cada decisão vai para `__auron.relatorio()` com hora, plano e motivo.

### 5.10 2D e vídeo

**2D (`fallback2d.js`)**
- **Estrutura:**
  - `<div class="mundo-2d">` fixo com vinheta CSS estática;
  - poses em `<svg>` criadas sob demanda (nada no HTML para quem tem 3D);
  - cada pose é um elemento separado, trocado por `opacity` em 242 ms, com `will-change` só durante a troca.
- **Poses:**
  - A a 12% à direita (ato II);
  - planta do A com 4 marcas (`#nucleo`);
  - 7 estratos (`#aur`);
  - A em andaime com grade de 24 px recortada (ato III);
  - A tracejado com hachura e cotas (ato IV);
  - círculo com meridianos e 2 pontos (`#escala`);
  - silhueta e depois contorno (`#fim`).
- **Em repouso:** zero rAF. A rede antiga de partículas em 2D sai.

**Vídeo (`#palco-edit`, `sections/edit.js`, `world/estudio.js`)**
- **Materiais do estúdio por perfil:**
  - HIGH e MEDIUM mantêm PBR e PMREM;
  - no LOW, `cromo` e `grafite` viram `MeshMatcapMaterial`, e `luz` vira `MeshBasicMaterial`.
- **Cores:** o material `ouro` vira `quente` (`#F3E2C4`), sem ouro perto do AUR. `RECUSADO`, `FRIO` e `ACEITO` seguem 5.2.
- **Pré-aquecimento por intenção:** `pointerenter`, `focus` ou `touchstart` no botão da apresentação chama `mundo.aquecerEstudio()` (PMREM + `compileAsync`). O travamento de cerca de 1,85 s sai do meio do vídeo.
- **Textos:** selos por `t()` (`fonte.ilustrado`, `estado.pesquisa`, `estado.planejado`) e lema final por `t("fim.lema")`.
- **Gravação:** a ordem `compor()` → `drawImage(mundoCanvas)` fica no mesmo rAF.

---

## 6. O que preservar do site atual

1. **Demonstrações que calculam de verdade:** SHA-512 do cabeçalho de 222 bytes, Merkle RFC 6962, AES-GCM 256 com os 256 fragmentos e o "corromper", Freivalds com o executor desonesto (agora com a regra do consenso), Dijkstra e `packet_id` em SHA-512. Tudo continua em `simulation/engine.js`, só estendido.
2. **Sistema de idiomas:**
   - 4 idiomas, `data-i18n`, `data-i18n-html` e `data-i18n-attr`;
   - sanitizador de `i18n.js` (`EM`, `STRONG`, `B`, `SPAN`, `BR`, sem atributos);
   - `?lang=` e `localStorage`;
   - `glossary.js` e `pronuncia.js`;
   - `tools/teste_idiomas.html`.
3. **Apresentação narrada:**
   - 10 cenas, legenda no padrão de TV, cortes na batida;
   - voz sintética declarada (ElevenLabs, registrada no `site/README.md`);
   - ordem de recaída mp3 → wav → voz do navegador → só legenda;
   - gravação `.webm` e `tools/teste_video.html`.
4. **Doação:** QR (`vendor/qrcode.min.js`), endereço em `data.js`, botão de copiar, textos de cuidado e "sem contrapartida".
5. **Honestidade:** o conteúdo de "Onde estamos", o aviso da rede, a lista do que o AUR não é e a regra "estado real de cada item" em `data.js`. A forma muda; o compromisso não.
6. **Marca em SVG inline:** `#simbolo-a`, `#palavra`, `#metal` e `#metal-h` nos `<defs>`, reusados com `<use>`. `logo.svg`, `favicon.svg` e as peças de `assets/brand/`.
7. **Conceito do mundo único** de partículas que muda de forma por capítulo, e o zoom guiado pela rolagem de `#escala`.
8. **Engenharia de leveza:**
   - perfis de qualidade, versão 2D e `prefers-reduced-motion` no CSS e no JS;
   - `.revela` sem partir de opacidade 0;
   - `?debug=1`.
9. **Acessibilidade:** `aria-live`, `role`, `.so-leitor`, foco visível, botões de teclado nas âncoras e `lang` nos trechos.
10. **Arquitetura:**
    - sem build, módulos por capítulo, `vendor/` com Three r170;
    - CSP estrita, sem script inline e sem `eval`;
    - `tools/build_downloads.py` com `downloads/manifesto.js` (zip com commit) e `tools/servidor_dev.py`.
11. **Trilha original** de `audio.js`: 62 BPM, lá menor, sintetizada, desligada por padrão.
12. **Detalhes com personalidade:** a árvore `├─ └─` da identidade e da carteira, a legenda de transportes por estilo de traço e o trilho de blocos (que vira a corrente).

---

## 7. Plano de construção, em ordem

**Regras gerais:**
- As Etapas 0 e 1 vão para o ar sozinhas: são verdade e desempenho, sem mudar o visual.
- Da 2 à 5, o trabalho acontece no ramo `site-v2` e só substitui o site publicado quando a Etapa 5 passar nos critérios.
- Python: `D:\auron\.toolchain\python\python`.
- Servidor: `python site/tools/servidor_dev.py 8766`.
- O autor só avalia o visual depois da Etapa 1, com o HUD confirmando o perfil.

### Etapa 0: verdade antes do visual

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/tools/extrair_dados.py` (novo) | Lê `vectors/genesis.json` (testnet), `vectors/usefulpow.json` (3 provas e 6 recusas, com a magic `AURR` e os alvos completados para 64 dígitos) e `vectors/emission.json`; importa `reference/auron/consensus.py` (MAINNET e TESTNET) e **para com erro se a importação falhar**. Conta `#[test]` em `crates/`, `def test_` em `reference/tests/`, testes de `integracao.rs` (e quantos têm `#[ignore]`) e arquivos de `vectors/`. Grava `site/dados/genese-teste.json`, `trabalho-util.json`, `emissao.json`, `contagem.json` e `provas.json` (nome e linha de cada teste), todos com data e commit. `--release TAG` grava `release.json`, só depois de a Release ser conferida no GitHub. | ~200 |
| `site/tools/gravar_testes.py` (novo) | Roda `cargo test -p auron-net --locked -- --ignored --test-threads=1` e grava `site/dados/gravacao-rede.json` (comando, data, commit, sistema, duração, linhas). | ~80 |
| `site/locales/pt-BR.js`, `en.js`, `es.js`, `ja.js` | Correções factuais nas chaves atuais: Noise XX, carteira e programas passam a Implementado; sai "Reputação: ATIVA"; checagens com trabalho útil; sai a data de dezembro; números novos; frase da Liquid; "cálculo útil" sai de `meta` e `hero`. | ~+40 / −30 cada |
| `site/js/data.js` | Estados corrigidos (`redep2p` implementado; itens novos `cifra`, `carteira`, `programas`); sai `CORES`. | ~+12 / −4 |
| `site/tools/servidor_dev.py` | Envia o `Content-Security-Policy` lido de `site/_headers` em tudo, **menos `/tools/`**, onde as páginas de ferramenta usam script inline. | ~+25 |

**Pronto quando:**
- `extrair_dados.py` imprime `rust=102 python=122 rede=17 (16 com sockets) vetores=18`, e os JSON validam com `json.load`;
- `teste_idiomas.html` (Edge headless, comando no arquivo) não mostra nenhum problema;
- o site atual, servido com CSP, abre nos 4 idiomas sem nenhuma violação no console.

### Etapa 1: chão firme do motor

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/quality.js` | Reescrito: WebGL2 com `failIfMajorPerformanceCaveat`, perfis e tetos de 5.7, regra do celular, escolha de Ajustes, `auto-2d` com validade. | 39 → ~110 |
| `site/js/world/world.js` | Itens 2 a 13 de 5.2 (ainda num arquivo só). | ~+160 / −70 |
| `site/js/world/mundo.js` (novo) | Fachada com troca a quente para o 2D. | ~70 |
| `site/js/world/fallback2d.js` | Métodos vazios para toda a API de 5.1. | ~+25 |
| `site/js/main.js` | Import do mundo em paralelo; capítulos sob demanda; sai `#qualidade`. | ~+60 / −25 |
| `site/js/depurar.js` (novo) | HUD com `?debug=1` (perfil, escala, dpr, mediana e p95, draw calls, triângulos, pontos, cobertura, memória estimada, plano e p, rebaixamentos, contagem de `setSize`); `__auron.relatorio()`; `?debug=gpu` com `gl.finish()`. | ~150 |
| `site/tools/gerar-vizinhos.html` + `.js` (novos) e `site/dados/vizinhos-v2-{240,360,700,1500}.bin` | Pré-cálculo dos vizinhos. | ~90 + ~17 KB |

**Pronto quando** (Atom x5, perfil LOW, 1440×900, `?debug=1`):
- depois de 3 minutos rolando e parado, o HUD mostra 240 nós e zero rebaixamentos;
- a esfera de `#escala` e o 16×16 aparecem completos;
- mediana do intervalo de 33 ± 1,5 ms em `rede`;
- no painel Performance, nenhuma tarefa acima de 100 ms depois do primeiro paint, exceto a avaliação do Three (anotar o valor no relatório);
- no Edge com `--disable-gpu --disable-gpu-compositing`, o site cai no 2D e a aba Network **não** baixa `three.module.min.js`;
- no celular emulado, o HUD conta `setSize` = 1 depois de rolar a página inteira.

### Etapa 2: sistema visual

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/index.html` (`<head>`) | Novo link de fontes; `og:` e `twitter:` em pt-BR; JSON-LD em pt-BR; `<script src="js/cedo.js">` antes dos CSS; `styles/sistema.css` e `styles/capitulos.css`. | ~30 |
| `site/js/cedo.js` (novo, clássico) | Classes `js`, `calmo`, `leve`, `sem3d`, `idioma-pendente` (com teto de 2,5 s) e `abertura-vista` (`sessionStorage`). Mesma lógica de idioma de `i18n.js`. Tudo em `try/catch`. | ~40 |
| `site/styles/sistema.css` (novo) | `:root` de 2.11; reset; tipografia; grade; barra; claquetes; placa com chanfro; botões; estado, proveniência e corte; régua de máquina; tabela de especificação; bancada; rodapé e Ajustes; `.leve`; `.calmo`. | ~650 |
| `site/assets/brand/a-mascara.svg` (novo) | Máscara do A (os 2 paths em branco). | ~1 KB |
| `site/tools/vitrine.html` + `.js` (novos) | Página com todos os tokens e componentes, nos 4 idiomas. | ~200 |

**Pronto quando:**
- a vitrine abre a 375 e a 1440 px sem estouro;
- `git grep -n "backdrop-filter" site/styles` não retorna nada;
- `git grep -n "@media" site/styles` só mostra 40rem, 64rem, 96rem e as preferências do usuário;
- `tools/teste_contraste.html`, rodado na vitrine, não mostra nenhum par abaixo de 4,5:1 (texto menor que 24 px) ou 3:1 (a partir de 24 px).

### Etapa 3: estrutura em atos e textos

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/index.html` | Reescrito na ordem de 3.1: `data-ato`, `data-plano`, `data-pose`, textos pt-BR estáticos, ids antigos como âncoras, `<defs>` com os `clipPath` da fresta e os glifos de estado, `#mercado` e `#rodar` novos, zero `style=""`. | 635 → ~780 |
| `site/styles/capitulos.css` (novo) | Layout de todos os capítulos, alturas de plano, recolhidos do ato IV. `styles/main.css` é removido. | ~850 / −508 |
| `site/locales/*.js` (4) | Chaves novas: `placa.*`, `fonte.*`, `ato.*`, `capitulo.<id>.codigo`, `abertura.*`, `visao.*`, `utrax.*`, `mercado.*`, `rodar.*`, `aur.*`, `escala.*`, `onde.*`, `fim.*`, `ajustes.*`, `barra.*`, `provas.<teste>`. Saem pilares, lemas, barras da IA e todo o inglês solto. Monumentos em spans de linha, com até 12 caracteres. | ~+220 / −160 cada |
| `site/locales/glossary.js` | Sai "Build the infrastructure."; entram `MATRIX-FREIVALDS-V1`, `AURON-UPOW-PROOF-v1`, `AURON-WIRE-v2`, `AURON-CARTEIRA-v2`, `Noise_XX_25519_ChaChaPoly_BLAKE2s`, `LWMA-1` e a regra dos 12 caracteres. | ~+14 / −2 |
| `site/js/i18n.js` | Carrega Noto Sans JP para `ja`; remove `idioma-pendente` depois de `aplicar()`. | ~+30 |
| `site/js/main.js` | Barra de status e progresso, menu na ordem nova, mecânica de 4.1. | ~+90 / −40 |
| `site/js/ajustes.js` (novo) | Imagem, Movimento e Som, com persistência e aplicação imediata. | ~110 |
| `site/tools/teste_estouro.html` + `.js` (novos) | Carrega o site em `<iframe>` a 375, 768, 1440 e 1920 px nos 4 idiomas e acusa `scrollWidth > clientWidth + 1` em `.equacao`, `.monumento`, `h1`, `h2` e `.placa-obra`, além de rolagem horizontal da página. | ~120 |
| `site/tools/teste_contraste.html` + `.js` (novos) | Calcula o contraste de cada elemento de texto contra o primeiro fundo opaco ancestral, ou `--vazio`. | ~130 |

**Pronto quando:**
- **Sem JavaScript:** `msedge --headless=new --blink-settings=scriptEnabled=false --dump-dom http://127.0.0.1:8766/` mostra todos os `h1` e `h2` com texto em pt-BR.
- **Idiomas:** `teste_idiomas.html` sem problemas; `teste_estouro.html` com "0 estouros em 16 combinações".
- **Limpeza:**
  - `git grep -n 'style="' site/index.html` não retorna nada;
  - `git grep -n -i -E "SIMULATION|RESEARCH|FUTURE|BUILD THE|WHERE WE ARE|ZOOM OUT|The Vision" site/index.html site/js site/locales/pt-BR.js` não retorna nada.
- **Rolagem:**
  - `scrollHeight / innerHeight` ≤ 24 a 1440×900 e ≤ 32 a 390×844;
  - `document.documentElement.scrollWidth ≤ innerWidth` a 375 px.

### Etapa 4: abertura e a prova oficial

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/prova/trabalho-util.js` (novo) | Port da §9A: `xof` (bloco i = `SHA-512(domínio ‖ semente ‖ u64be(i))`), `inteirosDaSemente`, `sementeDaTarefa`, `gerarMatrizes` (domínio `AURON-UTRAX-INSTANCE-v1`), `ladoExigido`, `decodificarProva`, `compromisso` (`AURON-UPOW-PROOF-v1`) e `conferir()`. A ordem das checagens e as mensagens são **idênticas** às do Rust. Aritmética em `Number` para n ≤ 92, porque n² · 999² · (2²⁰ − 1) < 2⁵³; acima disso, `BigInt`. Devolve `{ ok, motivo, rodada, r, esq, dir, ms }`. | ~190 |
| `site/js/abertura.js` (novo) | Confere a prova da altura 1, acende o "=", preenche a leitura, dispara o golpe por tempo, soma aos totais. | ~110 |
| `site/js/totais.js` (novo) | `somar("hashes" \| "provas" \| "recusas", n)` e `ler()`. | ~45 |
| `site/styles/capitulos.css` | Keyframes da abertura (`capa`, `laje-sobe`, `laje-desce`, `derrame`, `varre`, `sobe`) e as variantes `.leve`, `.calmo`, `.abertura-vista` e `.idioma-pendente`. | ~+150 |
| `site/tools/teste_trabalho_util.html` + `.js` (novos) | Roda as 3 provas (`verifies: true`) e as 6 recusas; confere `semente` e `compromisso` contra o vetor. | ~90 |

**Pronto quando:**
- **Prova:** `teste_trabalho_util.html` mostra "9 de 9", com as seis mensagens de recusa idênticas a `vectors/usefulpow.json`.
- **Abertura no Atom x5, cache vazio:**
  - placa de obra legível no primeiro paint;
  - equação completa em até 1,6 s;
  - "=" aceso em até 2,2 s.
- **Custo:** com "Paint flashing" ligado, nenhuma repintura maior que a faixa da fresta entre 0 e 1,8 s; a aba Animations mostra só `transform` e `opacity`.
- **Variações:**
  - com movimento reduzido, o quadro final aparece direto;
  - na segunda visita da aba, a abertura termina em até 400 ms;
  - sem HTTPS (IP de rede local), aparece a frase de recaída.

### Etapa 5: herói 3D, roteiro e morph na GPU

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/world/motor.js` | Sai de `world.js`: renderer, laço, governador, dormir, recorte, `compor()`. | ~320 |
| `site/js/world/pontos.js` | Morph na GPU, pacotes no shader, tetos de tamanho e cobertura. | ~240 |
| `site/js/world/formas.js` | As 7 formas atuais embaralhadas, mais `poeira`, `chuva`, `placa6` e `andaime`. | ~300 |
| `site/js/world/heroi.js` | Tudo de 5.3. | ~400 |
| `site/js/world/roteiro.js` | Quadros-chave dos 6 planos e as 13 poses, cada um com o quadro `still`. | ~280 |
| `site/js/world/estudio.js` | Movido de `world.js`, com os materiais por perfil de 5.10. | ~650 (movido) |
| `site/js/world/world.js` | Passa a só compor as peças. | ~120 |
| `site/js/depurar.js` | `?debug=emenda`: desenha o SVG do A num canvas 2D (imagem `data:`, permitida por `img-src`) e compara com `readPixels` do quadro 3D na mesma pose, sem pós, dentro da máscara do A. | ~+60 |

**Pronto quando:**
- **Emenda:** diferença média ≤ 8/255 por canal a 1440×900 e a 390×844.
- **Golpe:** sem salto de posição maior que 1 px (medido no HUD).
- **FPS no LOW** (`__auron.relatorio()` depois de rolar tudo em cerca de 2 minutos), em cada plano: mediana ≤ 34,5 ms, p95 ≤ 50 ms, zero rebaixamentos, nenhum quadro acima de 100 ms fora das trocas de ato (no máximo 2 por troca).
- **Rotação:** voltar do `#nos` ao `#topo` depois de 5 minutos não desenrola nenhuma volta.

### Etapa 6: ato I e ato II

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/sections/visao.js` (novo) | Espectrograma, contadores, produtos conferidos, orçamento de 1,5 ms. | ~160 |
| `site/js/sections/nucleo.js` | Âncoras projetadas, escada dos atos, versão 2D. | 28 → ~110 |
| `site/js/sections/utrax.js` | Prova oficial, sequência por p, vernier, régua de custo, mentir e valor impossível, "demonstra / não demonstra". A parte de mercado sai. | 80 → ~280 |
| `site/js/world/props/placas.js` (novo) | Placas A, B e C e os feixes. | ~200 |
| `site/js/sections/cadeia.js` | Gênese real, régua de 222 bytes, adulteração, pino, cascata, relógio de batidas. | 130 → ~300 |
| `site/js/world/props/corrente.js` (novo) | Réguas, pinos, curva da câmera. | ~170 |
| `site/js/simulation/engine.js` | `mineBlock` aceita `prev_hash` real; `camposDoCabecalho(bytes)`; `trocarByte`. A Freivalds antiga (3 rodadas, r de 1 a 97) é substituída por `prova/trabalho-util.js`. | ~+60 / −15 |
| `site/js/sections/nos.js` | Passos do teste, gravação, cifra, ficha sem reputação, diagrama 2D. | 87 → ~230 |
| `site/js/sections/seguranca.js` | Tabela de `provas.json`, linha do tempo da revisão, Sybil em `<details>`. | 80 → ~170 |
| `site/js/sections/aur.js` (novo) | Ficha, gráfico em degraus, rótulos dos estratos. | ~120 |
| `site/js/world/props/estratos.js` (novo) | Geometria dos estratos. | ~160 |
| `site/tools/teste_fontes.html` + `.js` (novos) | Acusa qualquer `.leitura` sem `data-fonte`, e qualquer número em `.espec` sem fonte no mesmo capítulo. | ~70 |

**Pronto quando:**
- **Honestidade:** `teste_fontes.html` retorna zero faltas.
- **Hashes:** no `#cadeia`, o hash do bloco 0 é igual a `block_hash` do vetor; trocar 1 byte mostra "não confere mais".
- **Recusa:** a mensagem do `#utrax` é idêntica ao vetor `resultado_alterado`.
- **Desempenho:** critérios de FPS da Etapa 5 valendo em `visao`, `utrax`, `cadeia` e `nos`.
- **Celular:** a 390×844, nenhuma rolagem horizontal, e o palco 3D fica na metade de cima.

### Etapa 7: atos III e IV

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/sections/mercado.js` (novo) | Candidatos em linhas, régua de 7 etapas, conferência real. | ~140 |
| `site/js/sections/rodar.js` (novo) | Alvos, detecção local, `release.json` opcional, terminal. | ~90 |
| `site/js/sections/direct.js` | Cotas, chave, orçamento, gasto duplo. | 90 → ~150 |
| `site/js/sections/malha.js` | Malha em SVG, Dijkstra, pacote com AES-GCM real, duas vistas. | 106 → ~170 |
| `site/js/sections/fragmentacao.js` | Fachada, caixilho, trinca, recusa em duas camadas, régua do armazenamento. | 98 → ~190 |
| `site/js/sections/radio.js` e `economia.js` | Removidos: o rádio vira anotação, a IA fica estática e o AUR vai para `aur.js`. | −145 |
| `site/js/sections/caminho.js` → `onde.js` | Colunas e sulcos a partir de `data.js`. | 63 → ~120 |

**Pronto quando:**
- **Carga sob demanda:** com a aba Network aberta, os módulos do ato IV só baixam ao abrir o `<details>`.
- **Demonstrações:**
  - a recusa do 16×16 mostra as duas camadas;
  - o conteúdo cifrado do pacote volta com AES-GCM na vista do destinatário.
- **Mundo dormindo:** em `#rodar` e `#onde`, o HUD mostra 0 quadros por segundo.

### Etapa 8: epílogo, fim, rodapé e som

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/sections/escala.js` | Degraus verdadeiros, parada em p 0,5, botões no modo calmo. | 29 → ~100 |
| `site/js/sections/aberto.js` | Números de `contagem.json`, crates, downloads em linhas. | 71 → ~90 |
| `site/js/sections/fim.js` (novo) | Totais da visita. | ~70 |
| `site/js/audio.js` | `filtroAto`, `abafar`, `recuar`. | ~+60 |
| `site/js/relogio.js` (novo) | Batida com e sem som. | ~40 |

**Pronto quando:**
- **Totais:** os números do `#fim` batem com `totais.ler()`, conferido no console.
- **Som:** nenhum `AudioContext` é criado antes do primeiro toque no botão de som.
- **Ajustes:** sobrevivem a um recarregamento; "Sem 3D" não baixa o Three na visita seguinte.

### Etapa 9: acabamento HIGH

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/world/pos.js` (novo) | Tudo de 5.6. | ~230 |
| `site/js/world/motor.js` | `compor()` único, `compileAsync` em ocioso com as peças de todos os atos anexadas com `visible = true`, `aquecerEstudio()`. | ~+70 |

**Pronto quando:**
- **GPU no HIGH:** tempo por quadro ≤ 10 ms com `?debug=gpu`.
- **Troca de ato:** nenhum quadro acima de 50 ms na primeira entrada de cada ato.
- **Recaída:** `?debug=semfloat` funciona com `UnsignedByteType`.
- **Emenda com pós:** a do golpe continua ≤ 8/255.

### Etapa 10: vídeo e narração

| Arquivo | Mudança | Tamanho |
|---|---|---|
| `site/js/sections/edit.js` | Selos e lema por `t()`, cores de 5.10, compatível com `compor()`. | ~+50 / −25 |
| `site/locales/*.js` | Cena 8: "mais de duzentos testes, no gabarito Python e no nó em Rust" (sem número que envelhece) e "a conexão entre os nós já é cifrada". Cena 9: "Agora, o que não prometemos." | ~4 chaves × 4 |
| `site/assets/voz/pt-BR/08.mp3` e `09.mp3` | Regerados com a mesma voz e o mesmo modelo do `site/README.md`. Os `.mp3` ficam fora do Git e **são publicados junto**. | — |

**Pronto quando:**
- **Gravação:** 10 cenas em `.webm`, sem quadro preto no primeiro segundo de nenhuma cena.
- **Idiomas:** selos corretos nos 4 idiomas.
- **Legenda:** acompanha o áudio novo das cenas 8 e 9.
- **Duração:** a soma das 10 faixas confere com o "3 min" do rótulo (ajustar o rótulo se não conferir).

### Etapa 11: verificação final e publicação

1. **CSP:** servidor com CSP; console sem nenhuma violação em pt-BR, en, es e ja, com o vídeo aberto e a gravação iniciada. Só se isso passar, remover `'unsafe-inline'` de `style-src` em `_headers` e repetir.
2. **Idiomas:** `teste_idiomas.html`, `teste_estouro.html` e `teste_contraste.html` passando.
3. **Desempenho no LOW (Atom x5):** relatório por plano dentro dos critérios da Etapa 5, anexado ao commit como `site/dados/medicao-low.json`.
4. **Sem GPU (emulando um Atom de 2012):**
   - Edge com `--disable-gpu --disable-gpu-compositing` e CPU 4× no DevTools;
   - nível "2D";
   - nenhuma tarefa acima de 200 ms;
   - no máximo 3 quadros seguidos acima de 50 ms durante a abertura.
5. **Celular real:** Android intermediário com perfil detectado; mediana de 33 ms em `utrax` e `cadeia`; nenhuma rolagem horizontal; alvos de toque de 44 px ou mais.
6. **Movimento reduzido:** critério de 4.7.
7. **Honestidade (as duas buscas precisam voltar vazias):**
   - `git grep -n -i -E "#e0806b|ff5e4d|9cc4ff|6fe3a5|93b6cc|9fd6b4|f2b865|efcb92" site`;
   - `git grep -n -i -E "c[aá]lculo [uú]til|ao vivo|LIVE" site/index.html site/locales`.
8. **Números:** `extrair_dados.py` rodado no dia da publicação; os JSON commitados batem com o repositório; a Release só tem link se `release.json` existir.
9. **Sem JS e 2D:** textos completos e poses SVG trocando por ato.

---

## 8. Anti-padrões proibidos nesta execução

**Visual**
- Gradiente roxo, azul-neon ou "aurora". Texto com gradiente. Ouro.
- Azul, verde, salmão ou vermelho em qualquer estado.
- Emoji, ícones de biblioteca (lucide, heroicons e parecidos). Os únicos glifos são os 4 de estado, as 3 marcas de fonte e o corte.
- Cartões iguais em grade, bento, "três vantagens em colunas", cartão com barra lateral colorida.
- Glassmorphism, `backdrop-filter`, painel dentro de painel.
- Tudo centralizado. Só há dois centros na página.
- Orbe, blob, planeta, moeda, logo girando sozinho, marcas de registro de HUD, anéis girando, hexágonos, glitch, scanline.
- Luz quente decorativa: em hover, em progresso, em foco, em título, perto do AUR.
- Tachado em PREÇO, VENDA ou PROMESSA.
- Sala clara, mármore, "premium", "exclusivo".

**Movimento**
- Animar `filter`, blur, `clip-path`, `mask`, `box-shadow`, `mix-blend-mode`, ou qualquer propriedade de layout.
- Sequestro de rolagem, `scroll-snap` obrigatório, rolagem suave por JS, `animation-timeline`.
- Número rolando como caça-níquel. Contador de dinheiro. Barras oscilando. Razão "×" ao vivo. Curva subindo com brilho. Seta para cima. Porcentagem de variação. "24h".
- Selo piscando. Ponto pulsando. Efeito de digitação em gravações. Pulso contínuo na batida. Quique ou elástico.
- Cursor personalizado. Botão magnético. Marquee.
- Qualquer coisa que se mova sozinha depois da abertura, fora a poeira, o bloco que nasce na cadeia e o espectrograma na tela.

**Conteúdo e honestidade**
- "Cálculo útil" ou "trabalho útil" como promessa de utilidade externa (contradiz a §9A).
- Data prometida, "em breve", "LIVE", "ao vivo", "rede global".
- "Unlock", "revolucionário", "o futuro de", "get started" e parecidos.
- Número digitado à mão.
- GRAVADO sem data e commit. Animação de algo "real" sem fonte no mesmo quadro.
- Inglês solto fora de nomes próprios e identificadores da spec (que ficam em mono).
- Link de Release sem conferência.
- Rosto humano, depoimento, logo de parceiro, "trusted by". Música de terceiros. Voz sem o rótulo "voz sintética".

**Técnica e desempenho**
- PMREM, `MeshPhysicalMaterial`, transmission, `RectAreaLight`, KTX2/Basis, HDR/EXR ou addons do Three na página.
- `setDrawRange` sobre índices não embaralhados.
- Upload de `position` por quadro. Alocação por quadro. `getBoundingClientRect` por quadro.
- Mais de 6 elementos DOM mexidos por quadro de rolagem. Variável CSS animada numa subárvore grande.
- `will-change` permanente em muitos elementos.
- `gl_PointSize` sem teto. Ponto invisível com tamanho maior que zero. Canvas desenhando atrás de camada opaca.
- `resize` realocando o framebuffer a cada movimento da barra do Android.
- Importar `three.module.min.js` sem WebGL2 garantido.
- Worker, `blob:` em script, `eval`, wasm, script inline, atributo `style=""`, CDN ou fonte fora do Google Fonts.
- `innerHTML` com texto de locale fora do sanitizador de `i18n.js`.
- Novo arquivo de áudio além da narração. `AudioContext` antes do toque.

**Processo**
- Avaliar o visual antes da Etapa 1 estar pronta.
- Publicar o ramo `site-v2` antes dos critérios da Etapa 5.
- Mudar a fala do vídeo sem regerar o áudio da cena.
- Tratar a máquina do autor como referência de HIGH. O LOW é o design principal, e o 2D é identidade, não sobra.