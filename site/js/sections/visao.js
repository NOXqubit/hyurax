// Capítulo 01 — O desperdício.
//
// Um espectrograma de hashes: cada linha é um SHA-512 calculado agora, neste
// aparelho, com os 64 bytes virando 64 tons de cinza. As linhas sobem e somem,
// e o contador mostra quantos foram calculados e quantos serviram para algo:
// zero. É a própria página demonstrando o desperdício da prova de trabalho
// comum. Depois a lâmina corta a tela e os bytes do último hash se encaixam
// numa matriz 8×8: a conta que o Auron exige que sirva.
//
// Custo: um canvas de 64 × 96 pixels ampliado pelo CSS, alguns hashes por
// quadro e só enquanto o capítulo está na tela.

import { sha512 } from "../simulation/engine.js";

const COLUNAS = 64, LINHAS = 96;
const faixa = (x, a, b) => Math.min(1, Math.max(0, (x - a) / (b - a)));

export function iniciar({ calmo, idioma }) {
  const secao = document.getElementById("visao");
  const tela = document.getElementById("espectro");
  const contador = document.getElementById("vis-n");
  const matriz = document.getElementById("vis-matriz");
  if (!secao || !tela || !globalThis.crypto?.subtle) return;

  tela.width = COLUNAS;
  tela.height = LINHAS;
  const ctx = tela.getContext("2d");
  const imagem = ctx.createImageData(COLUNAS, LINHAS);
  const px = imagem.data;
  for (let i = 3; i < px.length; i += 4) px[i] = 255;

  let total = 0, ultimo = new Uint8Array(64), visivel = false, rodando = false, fase = 0, esfria = 0;
  const formatar = (n) => n.toLocaleString(idioma());

  /** Empurra uma linha nova no topo: bytes viram tons entre aço e prata. */
  function empurrar(bytes) {
    px.copyWithin(COLUNAS * 4, 0, (LINHAS - 1) * COLUNAS * 4);
    for (let c = 0; c < COLUNAS; c++) {
      const v = bytes[c] / 255;
      // aço #24262A até prata #C4C7CC; a linha nova ganha um toque de luz
      let r = 36 + v * 160, g = 38 + v * 161, b = 42 + v * 162;
      const quente = 1 - esfria;
      r += 20 * quente * v; g += 8 * quente * v; b -= 18 * quente * v;
      const o = c * 4;
      px[o] = r; px[o + 1] = g; px[o + 2] = b;
    }
    // as linhas antigas escurecem: o que foi calculado some
    for (let l = 1; l < LINHAS; l++) {
      if (l % 6) continue;
      const base = l * COLUNAS * 4;
      for (let c = 0; c < COLUNAS; c++) {
        const o = base + c * 4;
        px[o] *= 0.97; px[o + 1] *= 0.97; px[o + 2] *= 0.97;
      }
    }
  }

  async function calcular(n) {
    for (let i = 0; i < n; i++) {
      const entrada = new TextEncoder().encode(`auron-desperdicio-${total}`);
      ultimo = await sha512(entrada);
      total++;
      empurrar(ultimo);
    }
    ctx.putImageData(imagem, 0, 0);
    contador.textContent = formatar(total);
  }

  function preencherMatriz() {
    matriz.replaceChildren(...Array.from({ length: 64 }, (_, i) => {
      const s = document.createElement("span");
      s.textContent = String(ultimo[i] % 10);
      s.style.setProperty("--i", String(i));
      return s;
    }));
  }

  let ultimoQuadro = 0;
  async function laco(agora) {
    if (!visivel || fase >= 3) { rodando = false; return; }
    rodando = true;
    if (agora - ultimoQuadro > 33) {
      ultimoQuadro = agora;
      await calcular(3);
    }
    requestAnimationFrame(laco);
  }

  function avaliar() {
    const r = secao.getBoundingClientRect();
    visivel = r.bottom > 0 && r.top < innerHeight;
    const p = faixa(-r.top, 0, Math.max(1, r.height - innerHeight));
    const nova = p < 0.3 ? 1 : p < 0.52 ? 2 : p < 0.78 ? 3 : 4;
    esfria = faixa(p, 0.25, 0.5);
    if (nova !== fase) {
      if (nova >= 3 && fase < 3) preencherMatriz();
      fase = nova;
      secao.dataset.fase = String(fase);
    }
    if (visivel && fase < 3 && !rodando) requestAnimationFrame(laco);
  }

  if (calmo) {
    // sem movimento: calcula um bloco de hashes uma vez e mostra o estado final
    calcular(LINHAS).then(() => { preencherMatriz(); secao.dataset.fase = "4"; });
    return;
  }
  let pedido = false;
  addEventListener("scroll", () => { if (!pedido) { pedido = true; requestAnimationFrame(() => { pedido = false; avaliar(); }); } }, { passive: true });
  addEventListener("resize", avaliar);
  calcular(24).then(avaliar);
}
