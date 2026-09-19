// Trabalho útil: pegue a mentira sem refazer a conta (Freivalds).
//
// Alguém entrega C dizendo que C = A · B. O verificador não refaz o produto:
// tira um vetor r do SHA-512 do próprio C e compara A·(B·r) com C·r, linha por
// linha. Tudo calculado de verdade aqui. Tocar num número de C é mentir: a
// conferência roda de novo e aponta a linha errada.

import { SimulationEngine, Escritor, sha512, hex } from "../simulation/engine.js";

const N = 6;
const RODADAS = 3;
const MODULO = 97;

export function iniciarTrabalho({ t, idioma }) {
  const gA = document.getElementById("fv-a");
  const gB = document.getElementById("fv-b");
  const gC = document.getElementById("fv-c");
  const rEl = document.getElementById("fv-r");
  const sementeEl = document.getElementById("fv-semente");
  const linhasEl = document.getElementById("fv-linhas");
  const veredito = document.getElementById("fv-veredito");
  const nova = document.getElementById("fv-nova");
  if (!gA || !gC) return { redesenhar() {} };

  const motor = new SimulationEngine(4242);
  let A, B, C, mexido = null, resultado = null;
  const fmt = (x) => x.toLocaleString(idioma());

  async function conferir() {
    const bytes = new Escritor();
    C.flat().forEach((x) => bytes.u32(x));
    const semente = await sha512(bytes.bytes());
    const rodadas = [];
    for (let k = 0; k < RODADAS; k++) {
      const r = Array.from({ length: N }, (_, i) => 1 + (semente[(k * N + i) % 64] % MODULO));
      const esq = SimulationEngine.vezes(A, SimulationEngine.vezes(B, r));
      const dir = SimulationEngine.vezes(C, r);
      const falha = esq.findIndex((x, i) => x !== dir[i]);
      rodadas.push({ r, esq, dir, falha });
      if (falha >= 0) break;
    }
    resultado = { semente: hex(semente), rodadas };
  }

  function celula(v, extra = {}) {
    const b = document.createElement(extra.botao ? "button" : "span");
    b.className = "cel" + (extra.mexido ? " mexido" : "");
    b.style.setProperty("--v", Math.min(1, v / (extra.botao ? 260 : 9)).toFixed(3));
    b.textContent = String(v);
    if (extra.botao) {
      b.type = "button";
      b.setAttribute("aria-label", t("trabalho.celula", { l: extra.i + 1, c: extra.j + 1, v }));
      b.addEventListener("click", () => mentir(extra.i, extra.j));
    }
    return b;
  }

  function desenhar() {
    gA.replaceChildren(...A.flat().map((v) => celula(v)));
    gB.replaceChildren(...B.flat().map((v) => celula(v)));
    gC.replaceChildren(...C.flatMap((linha, i) => linha.map((v, j) =>
      celula(v, { botao: true, i, j, mexido: mexido && mexido[0] === i && mexido[1] === j }))));
    if (!resultado) return;
    const ultima = resultado.rodadas[resultado.rodadas.length - 1];
    rEl.replaceChildren(...ultima.r.map((x) => { const s = document.createElement("span"); s.textContent = x; return s; }));
    sementeEl.textContent = t("trabalho.semente", { h: resultado.semente.slice(0, 12) + "…" });
    linhasEl.replaceChildren(...ultima.esq.map((x, i) => {
      const d = document.createElement("div");
      const bate = x === ultima.dir[i];
      d.className = "linha " + (bate ? "bate" : "falha");
      d.innerHTML = "";
      const a = document.createElement("span"); a.textContent = fmt(x);
      const s = document.createElement("span"); s.textContent = bate ? "=" : "≠";
      const b = document.createElement("span"); b.textContent = fmt(ultima.dir[i]);
      d.append(a, s, b);
      return d;
    }));
    const usadas = resultado.rodadas.length;
    const custo = 3 * N * N * usadas;
    if (ultima.falha < 0) {
      veredito.className = "veredito ok";
      veredito.textContent = t("trabalho.aceito", { r: usadas, custo: fmt(custo), refazer: fmt(N * N * N) });
    } else {
      veredito.className = "veredito ruim";
      veredito.textContent = t("trabalho.recusado", { r: usadas, l: ultima.falha + 1 });
    }
  }

  async function honesta() {
    A = motor.matriz(N);
    B = motor.matriz(N);
    C = SimulationEngine.multiplica(A, B);
    mexido = null;
    await conferir();
    desenhar();
  }

  async function mentir(i, j) {
    C = C.map((l) => l.slice());
    C[i][j] += 1 + motor.int(9);
    mexido = [i, j];
    await conferir();
    desenhar();
  }

  nova.addEventListener("click", honesta);
  honesta();
  return { redesenhar: () => A && desenhar() };
}
