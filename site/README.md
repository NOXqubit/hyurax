# Site da Auron Technology

Site institucional com um mundo 3D navegável, demonstrações interativas e quatro
idiomas (português, inglês, espanhol e japonês). É estático, sem etapa de build:
HTML, CSS e módulos JavaScript servidos como estão.

## Rodar localmente

```
python site/tools/build_downloads.py
python site/tools/build_narracao.py
python site/tools/servidor_dev.py
```

O `build_narracao.py` grava a narração do vídeo em `assets/voz/pt-BR/*.wav`
com a voz Microsoft Maria do Windows. Só roda no Windows, e os arquivos ficam
fora do Git por serem gerados. Sem eles, o vídeo cai na voz do navegador e,
não havendo nenhuma, mostra só a legenda.

### Trocar a voz da narração

O vídeo procura a narração nesta ordem, por cena (`01` a `10`):

1. `assets/voz/pt-BR/01.mp3` … `10.mp3`: voz gravada por uma pessoa, ou gerada
   por um serviço de voz realista;
2. `assets/voz/pt-BR/01.wav` … `10.wav`: a voz do Windows, gerada pelo
   `build_narracao.py`;
3. a voz do navegador; sem nenhuma, só a legenda.

A narração atual em `.mp3` foi gerada em 13/09/2026 no ElevenLabs, com a voz
de biblioteca "Will - For institutional videos" (`IKpiSijWzlhOL6uX83EH`),
modelo `eleven_multilingual_v2`. No texto enviado à voz, "AUR" vira "A U R" e
"Argon2id" vira "Argon dois I D". Os arquivos ficam fora do Git, como os
`.wav`; antes de uso comercial, confira a licença do plano da conta.

Para trocar a voz, basta pôr os `.mp3` na pasta, um por cena, com o texto de
`edit.cenas[n].fala` em `locales/pt-BR.js`. A legenda acompanha qualquer um
deles pelo tempo do áudio. Se a voz não for de uma pessoa real, o rótulo
"voz sintética" continua na tela.

As palavras estrangeiras que a voz lê errado ("hash", "Rust", "Argon2id")
ficam em `locales/pronuncia.js`. O mesmo dicionário vale para a voz do
navegador e para o `build_narracao.py`; a legenda continua mostrando a
palavra escrita do jeito certo.

Abra `http://127.0.0.1:8766`. O servidor de desenvolvimento desliga o cache,
para cada alteração aparecer no próximo recarregamento. Os módulos JavaScript
não funcionam abrindo o `index.html` direto do disco (`file://`).

## Por que não Next.js

O pedido original sugeria Next.js, React e Tailwind. A escolha foi JavaScript
puro com Three.js pelos motivos abaixo:

- **Máquina de desenvolvimento fraca.** Um build de Next.js num Atom com
  3,4 GB de RAM é lento e pode falhar.
- **Menos dependências.** O site só depende de Three.js e de um gerador de
  QR, ambos copiados em `vendor/`. Não há `node_modules` para auditar.
- **Segurança.** O CSP pode ser estrito: todo script é arquivo próprio, sem
  script inline e sem `eval`.
- **Hospedagem em qualquer lugar.** GitHub Pages, Cloudflare Pages, Netlify
  ou um servidor comum.

A estrutura segue a mesma separação pedida: motor 3D, motor de simulação,
capítulos e idiomas.

## Estrutura

```
site/
  index.html              um HTML só; o texto vem de locales/
  styles/main.css         sistema visual (preto, grafite, prata, luz quente)
  js/
    main.js               ponto de entrada: idioma, mundo, navegação
    i18n.js               troca de idioma, sanitização do HTML dos textos
    quality.js            perfis HIGH / MEDIUM / LOW pela capacidade gráfica
    data.js               estado real de cada item do caminho, endereço de doação
    ui.js                 peças reutilizadas pelos capítulos
    world/world.js        Auron World: uma rede de nós que muda de forma
    world/fallback2d.js   versão 2D quando não há WebGL
    simulation/engine.js  SimulationEngine: toda a lógica das demonstrações
    sections/*.js         um arquivo por capítulo
  locales/                pt-BR (referência), en, es, ja e o glossário
  vendor/                 three.js r170 e qrcode-generator 1.4.4
  assets/                 logo, favicon e imagens das redes sociais
  tools/                  pacote de downloads, servidor local, imagens de marca
  downloads/              gerado; fora do Git
  _headers                cabeçalhos de segurança para Cloudflare Pages / Netlify
```

## Regras de conteúdo

- **Nada é inventado.** Números reais (testes, parâmetros da especificação)
  vêm do repositório. Todo dado de demonstração é marcado como SIMULATION, e
  o motor marca cada objeto com `simulation: true`.
- **Estados sem mistura:** Implementado (existe em código e passa nos testes),
  Em desenvolvimento, Pesquisa e Planejado. O site usa "Implementado" em vez
  de "LIVE", porque nenhuma rede pública está no ar.
- **Sem preço, sem venda, sem promessa de valorização.**
- **Conta de verdade quando o navegador permite:** hash SHA-512 do cabeçalho
  de 158 bytes, raiz de Merkle RFC 6962, AES-GCM na fragmentação e Freivalds
  no UTRAX.

## Idiomas

Todos os arquivos de `locales/` precisam ter exatamente as mesmas chaves do
`pt-BR.js`. Os nomes do `glossary.js` não se traduzem. O idioma é escolhido
por `?lang=`, depois pela última escolha guardada no navegador, depois pelo
idioma do navegador.

## Desempenho

- Todos os nós são um único objeto de pontos, e as linhas reaproveitam o
  mesmo buffer.
- O perfil gráfico sai da placa de vídeo, do número de núcleos e da memória.
  Se os quadros ficarem lentos, o mundo rebaixa sozinho a resolução, depois
  os nós e os pacotes.
- Com `prefers-reduced-motion`, nada se move e o mundo é desenhado parado.
- Cada animação de capítulo só roda enquanto está visível na tela.

## Testes

Com o servidor de desenvolvimento no ar, os dois testes rodam sem interface,
no Edge, em tempo acelerado:

```
msedge --headless=new --virtual-time-budget=500000 --dump-dom http://127.0.0.1:8766/tools/teste_video.html
msedge --headless=new --virtual-time-budget=30000  --dump-dom http://127.0.0.1:8766/tools/teste_idiomas.html
```

- **teste_video.html** abre o vídeo, desliga a voz e acompanha as dez cenas até
  o fim. Pega o pior defeito possível: travar numa cena e não sair mais.
- **teste_idiomas.html** confere que os quatro idiomas carregam, têm as mesmas
  chaves e mantêm os marcadores `{x}`. Uma chave faltando faz aparecer
  português no meio do inglês.

`?debug=1` na URL do site expõe `__auron` (mundo e perfil) e `__video`
(estado do vídeo) no console. Com `&auto=1` o vídeo abre sozinho.

## Antes de publicar num domínio

1. Trocar `SEU-DOMINIO` em `sitemap.xml` e `robots.txt`.
2. Colocar URLs absolutas em `og:image` e `twitter:image` no `index.html`.
3. Rodar `python site/tools/build_downloads.py` depois do último commit.
