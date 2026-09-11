// RADIO RESEARCH: visualização conceitual. Um transmissor emite ondas; fragmentos seguem
// caminhos diferentes por antenas intermediárias e se juntam no destino.
import { telaViva, laco } from "../ui.js";

export function iniciar({ t, calmo, aoMudarIdioma }) {
  const canvas = document.getElementById("radio-canvas");
  const estadoEl = document.getElementById("radio-estado");
  const tela = telaViva(canvas);
  let tempo = 0, fase = 0;
  const antenas = [[0.5, 0.22], [0.5, 0.5], [0.5, 0.78], [0.3, 0.36], [0.7, 0.64]];
  const caminhos = [[0, 3], [1], [2, 4], [3, 1], [4]];
  let frags = [];

  function novoCiclo() {
    frags = Array.from({ length: 10 }, (_, i) => ({ caminho: caminhos[i % caminhos.length], t: -i * 0.12, feito: false }));
  }
  novoCiclo();

  function ponto(frag, w, h) {
    const pts = [[0.1, 0.5], ...frag.caminho.map((k) => antenas[k]), [0.9, 0.5]];
    const tt = Math.max(0, Math.min(1, frag.t)) * (pts.length - 1), k = Math.min(pts.length - 2, Math.floor(tt)), f = tt - k;
    return [(pts[k][0] + (pts[k + 1][0] - pts[k][0]) * f) * w, (pts[k][1] + (pts[k + 1][1] - pts[k][1]) * f) * h];
  }

  function desenhar() {
    const { ctx, w, h } = tela; if (!w) return;
    ctx.clearRect(0, 0, w, h);
    const tx = 0.1 * w, ty = 0.5 * h, rx = 0.9 * w;
    // ondas concêntricas saindo do transmissor
    for (let k = 0; k < 6; k++) {
      const r = ((tempo * 60 + k * 70) % 420);
      ctx.strokeStyle = `rgba(147,182,204,${(0.35 * (1 - r / 420)).toFixed(3)})`;
      ctx.lineWidth = 1; ctx.beginPath(); ctx.arc(tx, ty, r, -1.1, 1.1); ctx.stroke();
    }
    // antenas intermediárias
    antenas.forEach(([x, y]) => {
      const X = x * w, Y = y * h;
      ctx.strokeStyle = "rgba(160,166,175,0.5)"; ctx.beginPath(); ctx.moveTo(X, Y + 10); ctx.lineTo(X, Y - 6); ctx.stroke();
      ctx.beginPath(); ctx.moveTo(X - 6, Y + 10); ctx.lineTo(X, Y - 6); ctx.lineTo(X + 6, Y + 10); ctx.stroke();
      ctx.fillStyle = "rgba(210,214,220,0.9)"; ctx.beginPath(); ctx.arc(X, Y - 7, 2.2, 0, 6.283); ctx.fill();
    });
    // transmissor e receptor
    [[tx, "TX"], [rx, "RX"]].forEach(([x, rot]) => {
      ctx.fillStyle = "#0e0f11"; ctx.strokeStyle = "rgba(236,234,230,0.5)"; ctx.beginPath(); ctx.arc(x, ty, 16, 0, 6.283); ctx.fill(); ctx.stroke();
      ctx.fillStyle = "#eceae6"; ctx.font = "10px 'IBM Plex Mono', monospace"; ctx.textAlign = "center"; ctx.fillText(rot, x, ty + 3.5);
    });
    ctx.textAlign = "left";
    // fragmentos
    let chegaram = 0;
    frags.forEach((f) => {
      if (f.t <= 0) return;
      const [x, y] = ponto(f, w, h);
      if (f.t >= 1) { chegaram++; return; }
      const g = ctx.createRadialGradient(x, y, 0, x, y, 7); g.addColorStop(0, "rgba(255,243,223,1)"); g.addColorStop(1, "rgba(243,226,196,0)");
      ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, 7, 0, 6.283); ctx.fill();
    });
    // barra de reconstrução no receptor
    ctx.fillStyle = "rgba(29,31,35,0.9)"; ctx.fillRect(rx - 30, ty + 26, 60, 4);
    ctx.fillStyle = chegaram === frags.length ? "#9fd6b4" : "#f3e2c4"; ctx.fillRect(rx - 30, ty + 26, 60 * (chegaram / frags.length), 4);
    const novaFase = chegaram === frags.length ? 3 : chegaram > 0 ? 2 : frags.some((f) => f.t > 0.15) ? 1 : 0;
    if (novaFase !== fase) { fase = novaFase; estadoEl.textContent = t("radio.estados")[fase]; }
  }

  let pausaFim = 0;
  laco(canvas, (dt) => {
    tempo += dt;
    if (frags.every((f) => f.t >= 1)) { pausaFim += dt; if (pausaFim > 1.8) { pausaFim = 0; novoCiclo(); } }
    else frags.forEach((f) => { f.t += dt * 0.22; });
    desenhar();
  }, calmo);
  if (calmo) { frags.forEach((f) => { f.t = 0.55; }); desenhar(); }
  estadoEl.textContent = t("radio.estados")[fase];
  aoMudarIdioma(() => { estadoEl.textContent = t("radio.estados")[fase]; });
}
