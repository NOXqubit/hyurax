// AURON DIRECT: pagamento P2P como fluxo de dados, online e offline (voucher com orçamento limitado).
import { el, etapas, telaViva, laco, sequencia } from "../ui.js";
import { curto } from "../simulation/engine.js";

export function iniciar({ t, engine, calmo, aoMudarIdioma }) {
  const bOn = document.getElementById("direct-online"), bOff = document.getElementById("direct-offline");
  const bEnviar = document.getElementById("direct-enviar"), bDuplo = document.getElementById("direct-duplo");
  const olEtapas = document.getElementById("direct-etapas");
  const explica = document.getElementById("direct-explica");
  const campos = document.getElementById("direct-campos");
  const orc = document.getElementById("direct-orcamento"), orcBarra = document.getElementById("orcamento-barra"), orcTexto = document.getElementById("orcamento-texto");
  const frase = document.getElementById("direct-frase");
  const canvas = document.getElementById("direct-canvas");
  const tela = telaViva(canvas);
  const seq = sequencia();
  let modo = "online", passo = 0, alvo = 0, capsula = 0, conflito = false, orcamento = 20, marca = () => {}, sim = engine.simulatePayment("online");
  const txExemplo = engine.createTransaction();
  const alice = curto(txExemplo.remetente, 5), bob = curto(txExemplo.destinatario, 5);

  const lista = () => t(`direct.${modo}`);
  function montar() {
    marca = etapas(olEtapas, lista().map((x) => x[0]));
    marca(passo, conflito && modo === "offline" && passo === 5);
    orc.hidden = modo !== "offline"; frase.hidden = modo !== "offline"; bDuplo.hidden = modo !== "offline" || passo < lista().length - 1;
    bOn.setAttribute("aria-pressed", String(modo === "online")); bOff.setAttribute("aria-pressed", String(modo === "offline"));
    bEnviar.textContent = modo === "online" ? t("direct.enviar") : t("direct.enviar").replace("12,5", "4").replace("12.5", "4");
    orcBarra.style.width = `${(orcamento / 20) * 100}%`; orcTexto.textContent = `${orcamento} / 20 AUR`;
    explicar();
  }
  function explicar() {
    const l = lista()[passo];
    explica.textContent = conflito && modo === "offline" && passo === 5 ? t("direct.duplo_msg") : l[1] + (modo === "offline" && passo === 2 ? " " + t("direct.offline_valor") + "." : "");
    const c = (k) => t(`direct.campos.${k}`);
    const itens = [[c("ativo"), "AUR"], [c("valor"), modo === "online" ? "12.5 AUR" : "4 AUR"], [c("de"), `Alice · ${alice}`], [c("para"), `Bob · ${bob}`]];
    if (modo === "offline") itens.push([c("voucher"), curto(sim.voucher, 6)], [c("contador"), `${sim.contador} → ${sim.contador + 1}`], [c("validade"), "24h"]);
    if (passo >= (modo === "online" ? 3 : 2)) itens.push([c("assinatura"), curto(sim.assinatura, 8)]);
    campos.replaceChildren(...itens.flatMap(([k, v]) => [el("dt", { text: k }), el("dd", { text: v })]));
  }
  function desenhar() {
    const { ctx, w, h } = tela; if (!w) return;
    ctx.clearRect(0, 0, w, h);
    const y = h / 2, x0 = 34, x1 = w - 34;
    ctx.strokeStyle = modo === "offline" ? "rgba(160,166,175,0.35)" : "rgba(160,166,175,0.22)";
    ctx.setLineDash(modo === "offline" ? [2, 5] : []); ctx.lineWidth = 1; ctx.beginPath(); ctx.moveTo(x0, y); ctx.lineTo(x1, y); ctx.stroke(); ctx.setLineDash([]);
    [[x0, "ALICE"], [x1, "BOB"]].forEach(([x, n]) => {
      ctx.fillStyle = "#0e0f11"; ctx.strokeStyle = "rgba(236,234,230,0.55)"; ctx.beginPath(); ctx.arc(x, y, 14, 0, 6.283); ctx.fill(); ctx.stroke();
      ctx.fillStyle = "#8e9199"; ctx.font = "9px 'IBM Plex Mono', monospace"; ctx.textAlign = "center"; ctx.fillText(n, x, y + 30);
    });
    const x = x0 + (x1 - x0) * capsula;
    // rastro de dados: bytes em hexadecimal atrás da cápsula
    ctx.font = "9px 'IBM Plex Mono', monospace"; ctx.textAlign = "center";
    for (let k = 1; k < 9; k++) {
      const xx = x - k * 22; if (xx < x0 + 16) break;
      ctx.fillStyle = `rgba(243,226,196,${(0.5 - k * 0.05).toFixed(2)})`;
      ctx.fillText(sim.assinatura.slice(k * 2, k * 2 + 2), xx, y - 12 + (k % 2) * 24);
    }
    const cor = conflito && modo === "offline" && passo >= 5 ? "224,128,107" : "243,226,196";
    const g = ctx.createRadialGradient(x, y, 0, x, y, 22); g.addColorStop(0, `rgba(${cor},0.9)`); g.addColorStop(1, `rgba(${cor},0)`);
    ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, 22, 0, 6.283); ctx.fill();
    ctx.fillStyle = "#070707"; ctx.fillText("AUR", x, y + 3);
    ctx.textAlign = "left";
  }
  laco(canvas, () => { capsula += (alvo - capsula) * 0.06; desenhar(); }, calmo);

  async function enviar() {
    const s = seq.nova();
    conflito = false; passo = 0; alvo = 0; capsula = 0; sim = engine.simulatePayment(modo);
    if (modo === "offline") orcamento = 20;
    bEnviar.disabled = true; montar();
    const n = lista().length;
    for (let i = 0; i < n; i++) {
      passo = i; alvo = n > 1 ? i / (n - 1) : 1;
      if (modo === "offline" && i === 3) orcamento = 16;
      montar(); if (calmo) { capsula = alvo; desenhar(); }
      if (!(await s.espera(calmo ? 0 : 1500))) return;
    }
    bEnviar.disabled = false; montar();
  }
  async function duplo() {
    conflito = true; passo = 5; alvo = 5 / 6; orcamento = 12; montar(); desenhar();
  }
  bOn.addEventListener("click", () => { seq.cancelar(); modo = "online"; passo = 0; alvo = 0; conflito = false; bEnviar.disabled = false; montar(); });
  bOff.addEventListener("click", () => { seq.cancelar(); modo = "offline"; passo = 0; alvo = 0; conflito = false; orcamento = 20; bEnviar.disabled = false; montar(); });
  bEnviar.addEventListener("click", enviar);
  bDuplo.addEventListener("click", duplo);
  montar(); desenhar();
  const obs = new IntersectionObserver((es) => { if (es[0].isIntersecting) { obs.disconnect(); enviar(); } }, { threshold: 0.4 });
  obs.observe(canvas);
  aoMudarIdioma(montar);
}
