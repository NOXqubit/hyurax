# Site do Hyurax

A rede viva: a abertura é a própria rede em 3D, com nós que conferem blocos
salto a salto, e o resto do site mostra, com contas de verdade no navegador,
o que o projeto faz. Estático, sem etapa de build, em quatro idiomas
(português, inglês, espanhol e japonês).

## Rodar localmente

```
python site/tools/servidor_dev.py 8767
```

e abra `http://localhost:8767`.

## Como está organizado

| Arquivo | O que faz |
|---|---|
| `index.html` | A página inteira, com as chaves de tradução em `data-i18n` |
| `styles/main.css` | Preto e branco puro: os capítulos alternam claro e escuro, e estado se mostra por forma, nunca por cor |
| `js/main.js` | Liga idiomas, abertura, experimentos, menu, doação e o quadro de estado |
| `js/rede/viva.js` | A abertura em Three.js: nós em aglomerados, blocos se espalhando pelo grafo, ondas do Éter |
| `js/secoes/trabalho.js` | Freivalds de verdade: toque num número de C e veja a linha exata ser recusada |
| `js/secoes/confira.js` | Cinco blocos com cabeçalho de 222 bytes e SHA-512 real: mexa num e veja a cadeia denunciar |
| `js/secoes/eter.js` | Um arquivo em 24 pedaços com SHA-512 e raiz de Merkle reais, atravessando caminhos que você pode cortar |
| `js/simulation/engine.js` | Motor de simulação: SHA-512, Merkle (RFC 6962), blocos, matrizes |
| `js/quality.js` | Escolhe o perfil gráfico (e a abertura ainda reduz a resolução sozinha se a máquina engasgar) |
| `locales/*.js` | Os textos; os quatro idiomas têm exatamente as mesmas chaves |

## Regras que o site segue

- **Honestidade**: o que é simulação diz que é simulação; o que está só projetado
  (Bluetooth, rádio LoRa, mainnet) aparece como projetado. Nenhuma promessa de
  lucro, nenhuma venda.
- **Movimento reduzido**: com `prefers-reduced-motion`, a rede aparece parada e
  os experimentos mostram o resultado sem animar.
- **Máquina fraca**: perfil gráfico por aparelho, pausa fora da tela e queda de
  resolução automática.
- **Segurança**: todo JavaScript vem de arquivo (`_headers` traz a política de
  conteúdo); nada de script inline.

## Download do minerador

A seção `#baixar` aponta o botão principal para
`https://github.com/NOXqubit/hyurax/releases/latest/download/hyurax-windows-x86_64.zip`.
Esse endereço só funciona se a Release mais recente (que não seja pré-lançamento
nem rascunho) trouxer um arquivo com **exatamente** esse nome; se o nome mudar,
mude o `href` em `index.html` junto. O link secundário leva à página de todas as
versões, onde ficam as somas SHA-256.

Os capítulos alternam claro e escuro; quem acrescentar uma seção no meio
precisa inverter as que vêm depois para manter a alternância.

## O que ficou de fora nesta versão

O vídeo narrado do site antigo falava o nome Auron e foi retirado. Um vídeo novo
precisa de narração nova.
