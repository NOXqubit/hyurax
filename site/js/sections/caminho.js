// ROADMAP em 3D (seis ramos saindo do centro, com giro pelo cursor) e WHERE WE ARE.
import { el, selo } from "../ui.js";
import { RAMOS, ESTADOS, CORES } from "../data.js";

const NS = "http://www.w3.org/2000/svg";
const s = (tag, attrs = {}, texto) => { const e = document.createElementNS(NS, tag); for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v); if (texto !== undefined) e.textContent = texto; return e; };

export function iniciar({ t, calmo, movel, aoMudarIdioma }) {
  const svg = document.getElementById("roteiro-svg");
  const cenaR = document.getElementById("cena-r");
  const caixa = document.getElementById("roteiro-3d");

  function desenhar() {
    svg.replaceChildren();
    svg.append(s("circle", { cx: 0, cy: 0, r: 250, fill: "none", stroke: "rgba(236,234,230,0.05)" }), s("circle", { cx: 0, cy: 0, r: 160, fill: "none", stroke: "rgba(236,234,230,0.05)" }));
    RAMOS.forEach((ramo, k) => {
      const ang = (k / RAMOS.length) * Math.PI * 2 - Math.PI / 2;
      const cx = Math.cos(ang), cy = Math.sin(ang);
      const fim = 118 + ramo.itens.length * 16;
      svg.append(s("line", { x1: cx * 46, y1: cy * 46, x2: cx * fim, y2: cy * fim, stroke: "rgba(236,234,230,0.14)" }));
      ramo.itens.forEach(([id, estado], i) => {
        const d = 78 + i * 20, x = cx * d, y = cy * d;
        const g = s("g");
        const c = s("circle", { cx: x, cy: y, r: 4.2, fill: estado === "implementado" ? CORES[estado] : estado === "desenvolvimento" ? CORES[estado] : "#070708", stroke: CORES[estado], "stroke-width": 1.4 });
        if (estado === "pesquisa") c.setAttribute("stroke-dasharray", "2 2");
        g.append(c, s("title", {}, `${t(`cam.itens.${id}`)} — ${t(`estado.${estado}`)}`));
        svg.append(g);
      });
      const lx = cx * (fim + 22), ly = cy * (fim + 22);
      svg.append(s("text", { x: lx, y: ly + 4, "text-anchor": Math.abs(cx) < 0.2 ? "middle" : cx > 0 ? "start" : "end", class: "ramo-nome" }, t(`cam.ramos.${ramo.id}`)));
    });
    svg.append(s("circle", { cx: 0, cy: 0, r: 44, fill: "rgba(243,226,196,0.06)", stroke: "rgba(243,226,196,0.55)" }), s("text", { x: 0, y: 5, "text-anchor": "middle", class: "centro-nome" }, "AURON"));

    document.getElementById("legenda-estados").replaceChildren(...ESTADOS.map((e) => selo(e, t)));
    document.getElementById("roteiro-listas").replaceChildren(...RAMOS.map((ramo) => el("div", {},
      el("h3", { text: t(`cam.ramos.${ramo.id}`) }),
      el("ul", {}, ramo.itens.map(([id, estado]) => el("li", {}, el("span", { text: t(`cam.itens.${id}`) }), selo(estado, t)))))));
  }
  // giro 3D suave pelo cursor
  if (!calmo && !movel) {
    caixa.addEventListener("pointermove", (e) => {
      const r = caixa.getBoundingClientRect(), x = (e.clientX - r.left) / r.width - 0.5, y = (e.clientY - r.top) / r.height - 0.5;
      cenaR.style.transform = `rotateX(${(-y * 22 + 14).toFixed(2)}deg) rotateY(${(x * 26).toFixed(2)}deg)`;
    });
    caixa.addEventListener("pointerleave", () => { cenaR.style.transform = "rotateX(14deg) rotateY(0deg)"; });
    cenaR.style.transform = "rotateX(14deg) rotateY(0deg)";
  }

  // ---------- onde estamos ----------
  const colunas = document.getElementById("onde-colunas");
  const MAPA = { live: "implementado", building: "desenvolvimento", research: "pesquisa", future: "planejado" };
  // "LIVE" sugeriria uma rede no ar; o que existe é código testado, então o rótulo diz isso.
  const ROTULO = { live: "IMPLEMENTED", building: "BUILDING", research: "RESEARCH", future: "FUTURE" };
  function onde() {
    colunas.replaceChildren(...Object.keys(MAPA).map((k) => {
      const c = t(`onde.colunas.${k}`);
      return el("section", {}, el("span", { class: "selo", "data-estado": MAPA[k] }, el("i"), el("span", { text: `${ROTULO[k]} · ${c.titulo}` })),
        el("ul", {}, c.itens.map(([a, b]) => el("li", {}, el("span", { text: a }), el("small", { text: b })))));
    }));
  }
  desenhar(); onde();
  aoMudarIdioma(() => { desenhar(); onde(); });
}
