// Versão 2D do mundo, para quando o WebGL não existe. Mesma interface do mundo 3D.
export function criarMundo2D(canvas, calmo) {
  const ctx = canvas.getContext("2d");
  let w = 0, h = 0, nos = [], ligacoes = [], pulsos = [], rodando = false;
  let semente = 17;
  const aleat = () => { semente = (semente * 16807) % 2147483647; return (semente - 1) / 2147483646; };

  function montar() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    w = window.innerWidth; h = window.innerHeight;
    canvas.width = Math.round(w * dpr); canvas.height = Math.round(h * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    semente = 17;
    const n = Math.round(Math.min(110, Math.max(30, (w * h) / 12000)));
    nos = Array.from({ length: n }, () => ({ x: aleat() * w, y: aleat() * h, vx: (aleat() - 0.5) * 0.1, vy: (aleat() - 0.5) * 0.1 }));
    ligacoes = [];
    nos.forEach((a, i) => {
      nos.map((b, j) => ({ j, d: (a.x - b.x) ** 2 + (a.y - b.y) ** 2 })).filter((o) => o.j !== i).sort((p, q) => p.d - q.d).slice(0, 3)
        .forEach((o) => { if (!ligacoes.some((l) => l.a === o.j && l.b === i)) ligacoes.push({ a: i, b: o.j }); });
    });
  }
  function desenhar() {
    ctx.fillStyle = "#030303"; ctx.fillRect(0, 0, w, h);
    ctx.lineWidth = 1; ctx.strokeStyle = "rgba(160,166,175,0.12)";
    ligacoes.forEach((l) => { const a = nos[l.a], b = nos[l.b]; ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y); ctx.stroke(); });
    nos.forEach((n) => { ctx.fillStyle = "rgba(200,205,212,0.7)"; ctx.beginPath(); ctx.arc(n.x, n.y, 1.6, 0, 6.283); ctx.fill(); });
    pulsos.forEach((p) => {
      const a = nos[p.l.a], b = nos[p.l.b], x = a.x + (b.x - a.x) * p.t, y = a.y + (b.y - a.y) * p.t;
      const g = ctx.createRadialGradient(x, y, 0, x, y, 8); g.addColorStop(0, "rgba(255,243,223,0.95)"); g.addColorStop(1, "rgba(243,226,196,0)");
      ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, 8, 0, 6.283); ctx.fill();
    });
  }
  let quadro = 0;
  function passo() {
    if (!rodando) return;
    quadro++;
    nos.forEach((n) => { n.x += n.vx; n.y += n.vy; if (n.x < 0 || n.x > w) n.vx *= -1; if (n.y < 0 || n.y > h) n.vy *= -1; });
    if (ligacoes.length && quadro % 12 === 0) pulsos.push({ l: ligacoes[Math.floor(Math.random() * ligacoes.length)], t: 0, v: 0.01 + Math.random() * 0.01 });
    pulsos = pulsos.filter((p) => (p.t += p.v) < 1);
    desenhar();
    requestAnimationFrame(passo);
  }
  montar(); desenhar();
  window.addEventListener("resize", () => { montar(); desenhar(); });
  if (!calmo) {
    rodando = true; requestAnimationFrame(passo);
    document.addEventListener("visibilitychange", () => { rodando = !document.hidden; if (rodando) requestAnimationFrame(passo); });
  }
    // Mesma interface do mundo 3D, para o vídeo não precisar saber em qual dos
  // dois está rodando.
  return {
    nivel: "2D", fps: 30,
    definirModo() {}, definirZoom() {}, somenteObjetos() {}, objeto() {}, hudEscala() {},
    introducao: () => Promise.resolve(),
  };
}
