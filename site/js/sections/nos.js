// NODES: uma rede 2D de nós simulados que trocam mensagens; toque num nó para abrir a ficha.
import { el, telaViva, laco } from "../ui.js";

export function iniciar({ t, engine, calmo, movel, aoMudarIdioma }) {
  const canvas = document.getElementById("nos-canvas");
  const ficha = document.getElementById("no-detalhe");
  const contagem = document.getElementById("nos-contagem");
  const total = movel ? 28 : 46;
  const nos = Array.from({ length: total }, (_, i) => engine.createNode(i));
  // posições em coordenadas 0..1, espalhadas com distância mínima
  const pos = [];
  for (let i = 0; i < total; i++) {
    let p, tent = 0;
    do { p = { x: 0.05 + engine.aleat() * 0.9, y: 0.08 + engine.aleat() * 0.84 }; tent++; }
    while (tent < 40 && pos.some((q) => (q.x - p.x) ** 2 + (q.y - p.y) ** 2 < 0.012));
    pos.push(p);
  }
  engine.connectNodes(nos, pos, 3);
  let alturaRede = 1840;
  nos.forEach((n) => { n.altura = alturaRede - (engine.aleat() < 0.15 ? 1 : 0); });
  let escolhido = 0, pulsos = [], acumulado = 0;
  const tela = telaViva(canvas, () => desenhar());

  function xy(i) { return [pos[i].x * tela.w, pos[i].y * tela.h]; }
  function desenhar() {
    const { ctx, w, h } = tela; if (!w) return;
    ctx.clearRect(0, 0, w, h);
    ctx.lineWidth = 1;
    nos.forEach((n, i) => n.vizinhos.forEach((j) => {
      if (j < i) return;
      const liga = i === escolhido || j === escolhido;
      ctx.strokeStyle = liga ? "rgba(243,226,196,0.5)" : "rgba(160,166,175,0.16)";
      const [x1, y1] = xy(i), [x2, y2] = xy(j); ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
    }));
    pulsos.forEach((p) => {
      const [x1, y1] = xy(p.a), [x2, y2] = xy(p.b), x = x1 + (x2 - x1) * p.t, y = y1 + (y2 - y1) * p.t;
      const g = ctx.createRadialGradient(x, y, 0, x, y, 7); g.addColorStop(0, "rgba(255,243,223,1)"); g.addColorStop(1, "rgba(243,226,196,0)");
      ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, 7, 0, 6.283); ctx.fill();
    });
    nos.forEach((n, i) => {
      const [x, y] = xy(i), sel = i === escolhido;
      ctx.fillStyle = sel ? "#fff3df" : n.ocupado ? "rgba(239,203,146,0.9)" : "rgba(210,214,220,0.85)";
      ctx.beginPath(); ctx.arc(x, y, sel ? 5 : 2.6, 0, 6.283); ctx.fill();
      if (sel) { ctx.strokeStyle = "rgba(243,226,196,0.6)"; ctx.beginPath(); ctx.arc(x, y, 11, 0, 6.283); ctx.stroke(); }
      if (sel || (!movel && i % 5 === 0)) { ctx.fillStyle = sel ? "#eceae6" : "rgba(142,145,153,0.8)"; ctx.font = "10px 'IBM Plex Mono', monospace"; ctx.fillText(`#${n.id}`, x + 8, y - 8); }
    });
  }
  function mostrar() {
    const n = nos[escolhido], f = (k) => t(`nos.ficha.${k}`), v = (k) => t(`nos.valores.${k}`);
    ficha.replaceChildren(
      el("h4", { text: `NODE #${n.id}` }),
      el("dl", { class: "campos" },
        el("dt", { text: f("status") }), el("dd", { class: "ok", text: v("online") }),
        el("dt", { text: f("identidade") }), el("dd", { text: v("verificada") }),
        el("dt", { text: f("computacao") }), el("dd", { text: n.ocupado ? v("ocupada") : v("disponivel") }),
        el("dt", { text: f("rede") }), el("dd", { text: v("conectado") }),
        el("dt", { text: f("reputacao") }), el("dd", { text: v("ativa") }),
        el("dt", { text: f("vizinhos") }), el("dd", { text: n.vizinhos.map((j) => "#" + nos[j].id).join(" · ") }),
        el("dt", { text: f("altura") }), el("dd", { text: String(n.altura) }),
      ),
      el("p", { class: "nota", style: "margin:1rem 0 0", text: t("nos.nota") }),
    );
    contagem.textContent = t("nos.contagem", { n: total });
  }
  canvas.addEventListener("click", (e) => {
    const r = canvas.getBoundingClientRect(), mx = e.clientX - r.left, my = e.clientY - r.top;
    let melhor = -1, d = 30 * 30;
    nos.forEach((_, i) => { const [x, y] = xy(i), dd = (x - mx) ** 2 + (y - my) ** 2; if (dd < d) { d = dd; melhor = i; } });
    if (melhor >= 0) { escolhido = melhor; mostrar(); desenhar(); }
  });
  laco(canvas, (dt) => {
    acumulado += dt;
    if (acumulado > 0.12) {
      acumulado = 0;
      const a = engine.int(total), viz = nos[a].vizinhos;
      if (viz.length) pulsos.push({ a, b: viz[engine.int(viz.length)], t: 0, v: 0.6 + engine.aleat() * 0.8 });
    }
    pulsos = pulsos.filter((p) => {
      p.t += p.v * dt;
      if (p.t >= 1) { if (engine.aleat() < 0.08) { alturaRede++; nos[p.b].altura = alturaRede; if (p.b === escolhido) mostrar(); } return false; }
      return true;
    });
    desenhar();
  }, calmo);
  mostrar(); desenhar();
  aoMudarIdioma(mostrar);
}
