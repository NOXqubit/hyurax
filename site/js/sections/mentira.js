// Capítulo 03 — Pegue a mentira sem refazer a conta.
//
// A peça central do site. Alguém entrega C dizendo que C = A · B. O verificador
// não refaz o produto: sorteia um vetor r a partir do SHA-512 do próprio C e
// confere A · (B · r) contra C · r, linha por linha (algoritmo de Freivalds). Tudo
// calculado de verdade no navegador. No fim a pessoa adultera um número de C e
// vê a linha exata ser recusada.
//
// A rolagem avança as fases; a interação só depende de cliques e teclado.

import { el } from "../ui.js";
import { SimulationEngine, Escritor, sha512, hex } from "../simulation/engine.js";

const N = 8, RODADAS = 3;
const faixa = (x, a, b) => Math.min(1, Math.max(0, (x - a) / (b - a)));

export function iniciar({ t, engine, calmo, idioma, aoMudarIdioma }) {
  const secao = document.getElementById("utrax");
  if (!secao) return;
  const placaA = document.getElementById("placa-a");
  const placaB = document.getElementById("placa-b");
  const placaC = document.getElementById("placa-c");
  const vetorEl = document.getElementById("vetor-r");
  const semEl = document.getElementById("semente-c");
  const compEl = document.getElementById("comparacao");
  const veredito = document.getElementById("mentira-veredito");
  const refazer = document.getElementById("mentira-refazer");

  let A, B, C, mexido = null, estado = null;
  const fmt = (n) => n.toLocaleString(idioma());

  /** A mesma derivação do motor: r_i = 1 + (SHA-512(C)[k·n + i] mod 97). */
  async function conferir() {
    const bytes = new Escritor();
    C.flat().forEach((x) => bytes.u32(x));
    const semente = await sha512(bytes.bytes());
    const rodadas = [];
    for (let k = 0; k < RODADAS; k++) {
      const r = Array.from({ length: N }, (_, i) => 1 + (semente[(k * N + i) % 64] % 97));
      const esq = SimulationEngine.vezes(A, SimulationEngine.vezes(B, r));
      const dir = SimulationEngine.vezes(C, r);
      const falha = esq.findIndex((x, i) => x !== dir[i]);
      rodadas.push({ r, esq, dir, falha });
      if (falha >= 0) break;
    }
    estado = { semente: hex(semente), rodadas };
  }

  function desenharPlaca(alvo, M, tipo) {
    alvo.replaceChildren(...M.flatMap((linha, i) => linha.map((v, j) => {
      const nivel = Math.min(1, v / (tipo === "c" ? 180 : 9));
      const props = { class: "cel", style: `--v:${nivel.toFixed(3)}` };
      if (tipo === "c") {
        return el("button", {
          ...props, type: "button",
          class: `cel${mexido && mexido[0] === i && mexido[1] === j ? " mexido" : ""}`,
          "aria-label": t("mentira.celula", { l: i + 1, c: j + 1, v }),
          onclick: () => mentir(i, j),
        }, el("span", { text: String(v) }));
      }
      return el("span", props, el("span", { text: String(v) }));
    })));
  }

  function desenharConferencia() {
    const primeira = estado.rodadas[0];
    const ultima = estado.rodadas[estado.rodadas.length - 1];
    vetorEl.replaceChildren(...primeira.r.map((x) => el("span", { text: String(x) })));
    semEl.textContent = `sha512(C) = ${estado.semente.slice(0, 12)}…${estado.semente.slice(-6)}`;
    compEl.replaceChildren(...ultima.esq.map((x, i) => {
      const igual = x === ultima.dir[i];
      return el("div", { class: `linha${igual ? " igual" : " diferente"}`, style: `--i:${i}` },
        el("b", { class: "num", text: fmt(x) }),
        el("i", { text: igual ? "=" : "≠", "aria-hidden": "true" }),
        el("b", { class: "num", text: fmt(ultima.dir[i]) }));
    }));
    const falha = ultima.falha;
    secao.classList.toggle("recusado", falha >= 0);
    veredito.replaceChildren(
      el("span", { class: "ponto", "aria-hidden": "true" }),
      el("span", {
        text: falha >= 0
          ? t("mentira.recusado", { l: falha + 1, k: estado.rodadas.length })
          : t("mentira.aceito", { k: RODADAS }),
      }),
    );
  }

  async function novaTarefa() {
    A = engine.matriz(N); B = engine.matriz(N); C = SimulationEngine.multiplica(A, B); mexido = null;
    await conferir();
    desenharPlaca(placaA, A, "a"); desenharPlaca(placaB, B, "b"); desenharPlaca(placaC, C, "c");
    desenharConferencia();
  }

  async function mentir(i, j) {
    C = C.map((l) => l.slice());
    C[i][j] += 1;
    mexido = [i, j];
    await conferir();
    desenharPlaca(placaC, C, "c");
    desenharConferencia();
    const alvo = placaC.querySelectorAll(".cel")[i * N + j];
    if (alvo) alvo.focus();
  }

  refazer.addEventListener("click", novaTarefa);
  aoMudarIdioma(() => { if (estado) { desenharPlaca(placaC, C, "c"); desenharConferencia(); } });

  // ---------------------------------------------------------- rolagem
  let fase = 0;
  function avaliar() {
    const r = secao.getBoundingClientRect();
    if (r.bottom < 0 || r.top > innerHeight) return;
    const p = faixa(-r.top, 0, Math.max(1, r.height - innerHeight));
    const nova = p < 0.16 ? 1 : p < 0.34 ? 2 : p < 0.52 ? 3 : p < 0.7 ? 4 : 5;
    if (nova !== fase) {
      fase = nova;
      for (let k = 1; k <= 5; k++) secao.classList.toggle(`ate-${k}`, fase >= k);
      secao.dataset.fase = String(fase);
    }
  }
  novaTarefa().then(() => {
    if (calmo) { for (let k = 1; k <= 5; k++) secao.classList.add(`ate-${k}`); secao.dataset.fase = "5"; return; }
    let pedido = false;
    addEventListener("scroll", () => { if (!pedido) { pedido = true; requestAnimationFrame(() => { pedido = false; avaliar(); }); } }, { passive: true });
    addEventListener("resize", avaliar);
    avaliar();
  });
}
