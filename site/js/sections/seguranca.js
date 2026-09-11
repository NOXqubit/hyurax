// SECURITY: os mecanismos com o estado real de cada um, e uma simulação de ataque Sybil.
import { el, selo, telaViva, laco, sequencia } from "../ui.js";

export function iniciar({ t, engine, calmo, movel, aoMudarIdioma }) {
  const grade = document.getElementById("mecanismos");
  function mecanismos() {
    grade.replaceChildren(...t("seg.mecanismos").map(([nome, texto, estado]) => el("div", {}, selo(estado, t), el("b", { text: nome }), el("p", { text: texto }))),
      el("div", {}, selo("desenvolvimento", t), el("b", { text: "Security review" }), el("p", { text: t("seg.revisao") })));
  }
  mecanismos();

  // ---------- Sybil ----------
  const canvas = document.getElementById("sybil-canvas");
  const explica = document.getElementById("sybil-explica");
  const estadosEl = document.getElementById("sybil-estados");
  const tela = telaViva(canvas);
  const seq = sequencia();
  let estado = "normal";
  const nHonestos = movel ? 20 : 30;
  const honestos = Array.from({ length: nHonestos }, (_, i) => {
    const a = (i / nHonestos) * Math.PI * 2, r = 0.26 + engine.aleat() * 0.12;
    return { x: 0.36 + Math.cos(a) * r * 0.8, y: 0.5 + Math.sin(a) * r };
  });
  const ligacoes = [];
  honestos.forEach((_, i) => { ligacoes.push([i, (i + 1) % honestos.length]); if (i % 3 === 0) ligacoes.push([i, (i + 7) % honestos.length]); });
  let falsos = [];

  function marcar() {
    [...estadosEl.children].forEach((s) => s.classList.toggle("ativo", s.dataset.e === estado));
    explica.textContent = t(`seg.sybil.${estado}`);
  }
  function desenhar() {
    const { ctx, w, h } = tela; if (!w) return;
    ctx.clearRect(0, 0, w, h);
    if (estado === "quarentena" || estado === "revalidacao") {
      ctx.strokeStyle = "rgba(224,128,107,0.45)"; ctx.setLineDash([4, 4]);
      ctx.strokeRect(w * 0.72, h * 0.12, w * 0.24, h * 0.62); ctx.setLineDash([]);
      ctx.fillStyle = "rgba(224,128,107,0.8)"; ctx.font = "9px 'IBM Plex Mono', monospace"; ctx.fillText("QUARANTINE", w * 0.73, h * 0.1);
    }
    ctx.lineWidth = 1; ctx.strokeStyle = "rgba(160,166,175,0.22)";
    ligacoes.forEach(([a, b]) => { ctx.beginPath(); ctx.moveTo(honestos[a].x * w, honestos[a].y * h); ctx.lineTo(honestos[b].x * w, honestos[b].y * h); ctx.stroke(); });
    falsos.forEach((f) => {
      if (f.alvo !== null && estado === "suspeito") {
        const hn = honestos[f.alvo]; ctx.strokeStyle = "rgba(239,203,146,0.12)";
        ctx.beginPath(); ctx.moveTo(f.x * w, f.y * h); ctx.lineTo(hn.x * w, hn.y * h); ctx.stroke();
      }
    });
    honestos.forEach((n) => { ctx.fillStyle = "rgba(220,224,230,0.9)"; ctx.beginPath(); ctx.arc(n.x * w, n.y * h, 3, 0, 6.283); ctx.fill(); });
    falsos.forEach((f) => {
      ctx.fillStyle = f.cor; ctx.beginPath(); ctx.arc(f.x * w, f.y * h, 1.8, 0, 6.283); ctx.fill();
    });
  }
  laco(canvas, () => {
    falsos.forEach((f) => { f.x += (f.tx - f.x) * 0.05; f.y += (f.ty - f.y) * 0.05; });
    desenhar();
  }, calmo);

  async function atacar() {
    const s = seq.nova(), p = (ms) => s.espera(calmo ? 0 : ms);
    estado = "normal"; falsos = []; marcar(); desenhar();
    if (!(await p(900))) return;
    const n = movel ? 70 : 150;
    falsos = Array.from({ length: n }, () => {
      const x = 0.86 + (engine.aleat() - 0.5) * 0.14, y = 0.85 + (engine.aleat() - 0.5) * 0.14;
      const alvo = engine.int(honestos.length), hn = honestos[alvo];
      return { x, y, tx: hn.x + (engine.aleat() - 0.5) * 0.12, ty: hn.y + (engine.aleat() - 0.5) * 0.12, alvo, cor: "rgba(239,203,146,0.85)" };
    });
    estado = "suspeito"; marcar();
    if (!(await p(3200))) return;
    estado = "quarentena"; marcar();
    falsos.forEach((f) => { f.tx = 0.74 + engine.aleat() * 0.2; f.ty = 0.15 + engine.aleat() * 0.56; f.cor = "rgba(224,128,107,0.85)"; f.alvo = null; });
    if (!(await p(3200))) return;
    estado = "revalidacao"; marcar();
    falsos.forEach((f, i) => { if (i % 25 === 0) { f.cor = "rgba(147,182,204,0.95)"; f.tx = 0.62; f.ty = 0.2 + (i / n) * 0.6; } });
    if (calmo) { falsos.forEach((f) => { f.x = f.tx; f.y = f.ty; }); desenhar(); }
  }
  document.getElementById("sybil-atacar").addEventListener("click", atacar);
  marcar(); desenhar();
  aoMudarIdioma(() => { mecanismos(); marcar(); });
}
