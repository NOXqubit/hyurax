// AURON RESONANCE + MESH + STORE-AND-FORWARD + AURON PACKET.
import { el, telaViva, laco } from "../ui.js";
import { curto } from "../simulation/engine.js";

const NOS = {
  A: [0.06, 0.5], B: [0.2, 0.26], D: [0.2, 0.74], K: [0.34, 0.5], C: [0.38, 0.18], E: [0.4, 0.8],
  F: [0.53, 0.38], G: [0.55, 0.72], H: [0.68, 0.16], I: [0.7, 0.55], J: [0.84, 0.3], L: [0.84, 0.78], Z: [0.94, 0.52],
};
const TIPOS = { bt: 3, wd: 2, wifi: 1.5, net: 1 };
const ARESTAS = [
  ["A", "B", "bt"], ["A", "D", "bt"], ["B", "C", "wd"], ["B", "K", "bt"], ["D", "K", "bt"], ["D", "E", "wd"], ["K", "F", "wifi"],
  ["C", "F", "wifi"], ["C", "H", "net"], ["E", "G", "bt"], ["E", "F", "bt"], ["F", "I", "wifi"], ["G", "I", "wd"], ["G", "L", "bt"],
  ["H", "J", "net"], ["I", "J", "wifi"], ["I", "Z", "bt"], ["J", "Z", "wd"], ["L", "Z", "wd"],
];

export function iniciar({ t, engine, calmo, aoMudarIdioma }) {
  // ---------- malha ----------
  const canvas = document.getElementById("malha-canvas");
  const rotaEl = document.getElementById("malha-rota");
  const tela = telaViva(canvas);
  const grafo = { arestas: ARESTAS.map(([a, b, tp]) => ({ a, b, tipo: tp, peso: TIPOS[tp] })) };
  const fora = new Set();
  let rota = engine.routeMessage(grafo, "A", "Z", fora), avanco = 0;

  function mostrarRota() {
    rotaEl.textContent = rota ? `${t("res.rota")}: ${rota.join(" → ")}` : t("res.sem_rota");
  }
  function pt(n) { return [NOS[n][0] * tela.w, NOS[n][1] * tela.h]; }
  function desenhar() {
    const { ctx, w, h } = tela; if (!w) return;
    ctx.clearRect(0, 0, w, h);
    const naRota = new Set(); if (rota) for (let i = 0; i < rota.length - 1; i++) naRota.add([rota[i], rota[i + 1]].sort().join());
    grafo.arestas.forEach((ar) => {
      const [x1, y1] = pt(ar.a), [x2, y2] = pt(ar.b), ativa = naRota.has([ar.a, ar.b].sort().join()), morta = fora.has(ar.a) || fora.has(ar.b);
      ctx.setLineDash(ar.tipo === "bt" ? [1.5, 4] : ar.tipo === "wd" ? [6, 5] : []);
      ctx.lineWidth = ar.tipo === "net" ? 2.6 : 1.2;
      ctx.strokeStyle = morta ? "rgba(224,128,107,0.18)" : ativa ? "rgba(243,226,196,0.85)" : "rgba(160,166,175,0.28)";
      ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
    });
    ctx.setLineDash([]);
    Object.keys(NOS).forEach((n) => {
      const [x, y] = pt(n), morto = fora.has(n), ponta = n === "A" || n === "Z";
      ctx.fillStyle = "#0b0b0d"; ctx.strokeStyle = morto ? "rgba(224,128,107,0.8)" : ponta ? "rgba(255,243,223,0.9)" : "rgba(210,214,220,0.6)";
      ctx.lineWidth = 1.2; ctx.beginPath(); ctx.arc(x, y, ponta ? 13 : 10, 0, 6.283); ctx.fill(); ctx.stroke();
      ctx.fillStyle = morto ? "#e0806b" : "#eceae6"; ctx.font = "10px 'IBM Plex Mono', monospace"; ctx.textAlign = "center"; ctx.fillText(morto ? "×" : n, x, y + 3.5);
    });
    ctx.textAlign = "left";
    if (rota && rota.length > 1) {
      const seg = avanco * (rota.length - 1), k = Math.min(rota.length - 2, Math.floor(seg)), f = seg - k;
      const [x1, y1] = pt(rota[k]), [x2, y2] = pt(rota[k + 1]), x = x1 + (x2 - x1) * f, y = y1 + (y2 - y1) * f;
      const g = ctx.createRadialGradient(x, y, 0, x, y, 10); g.addColorStop(0, "rgba(255,243,223,1)"); g.addColorStop(1, "rgba(243,226,196,0)");
      ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, 10, 0, 6.283); ctx.fill();
    }
  }
  laco(canvas, (dt) => { avanco = (avanco + dt * 0.22) % 1; desenhar(); }, calmo);
  document.getElementById("malha-derrubar").addEventListener("click", () => {
    if (!rota || rota.length <= 2) return;
    const meio = rota.slice(1, -1);
    fora.add(meio[engine.int(meio.length)]);
    rota = engine.routeMessage(grafo, "A", "Z", fora); avanco = 0; mostrarRota(); desenhar();
  });
  document.getElementById("malha-restaurar").addEventListener("click", () => { fora.clear(); rota = engine.routeMessage(grafo, "A", "Z", fora); avanco = 0; mostrarRota(); desenhar(); });
  mostrarRota(); desenhar();

  // ---------- store-and-forward ----------
  const olSf = document.getElementById("sf-passos");
  let passoSf = 0;
  function desenharSf() {
    olSf.replaceChildren(...t("res.sf").map(([ic, txt], i) => el("li", { class: [i < passoSf ? "feito" : "", i === passoSf ? "ativo" : "", i === 1 || i === 3 ? "guarda" : ""].join(" ") },
      el("span", { class: "ic", text: ic }), el("b", { text: txt }))));
  }
  let visivelSf = false;
  new IntersectionObserver((es) => { visivelSf = es[0].isIntersecting; }).observe(olSf);
  if (!calmo) setInterval(() => { if (visivelSf && !document.hidden) { passoSf = (passoSf + 1) % 7; desenharSf(); } }, 1900);
  desenharSf();

  // ---------- pacote ----------
  const bPacote = document.getElementById("pacote"), camposEl = document.getElementById("pacote-campos"), listaEl = document.getElementById("pacote-lista");
  const bDono = document.getElementById("visao-dono"), bRelay = document.getElementById("visao-relay");
  let pacote = null, relay = false;
  const conteudo = "Olá, Z. Chegou pela malha.";
  function desenharPacote() {
    if (!pacote) return;
    document.getElementById("pacote-id").textContent = `packet_id ${curto(pacote.id, 10)}`;
    const data = (s) => new Date(s * 1000).toISOString().replace("T", " ").slice(0, 19);
    const linhas = [
      ["VERSION", String(pacote.versao)], ["PACKET ID", curto(pacote.id, 12)], ["SENDER", "identity:A"], ["RECIPIENT", "identity:Z"],
      ["SESSION", curto(pacote.sessao, 8)], ["TIMESTAMP", data(pacote.horario)], ["EXPIRATION", data(pacote.expiracao)], ["PRIORITY", pacote.prioridade],
      ["PAYLOAD HASH", curto(pacote.payloadHash, 10)], ["FLAGS", pacote.flags], ["SIGNATURE", curto(pacote.assinatura, 10)],
    ];
    const itens = linhas.flatMap(([k, v]) => [el("dt", { text: k }), el("dd", { text: v })]);
    itens.push(el("dt", { text: "ENCRYPTED PAYLOAD" }), el("dd", { class: "cifrado" }, curto(pacote.payload, 14), el("small", { text: t("res.rotulos.cifrado") })));
    itens.push(el("dt", { text: "CONTENT" }), relay ? el("dd", { class: "erro", text: `— ${t("res.rotulos.so_cabecalho")}` }) : el("dd", { class: "ok", text: t("res.conteudo") }));
    listaEl.replaceChildren(...itens);
    bDono.setAttribute("aria-pressed", String(!relay)); bRelay.setAttribute("aria-pressed", String(relay));
  }
  bPacote.addEventListener("click", () => {
    const abrir = camposEl.hidden;
    camposEl.hidden = !abrir; bPacote.setAttribute("aria-expanded", String(abrir));
  });
  bDono.addEventListener("click", () => { relay = false; desenharPacote(); });
  bRelay.addEventListener("click", () => { relay = true; desenharPacote(); });
  engine.sendPacket({ remetente: "identity:A", destinatario: "identity:Z", conteudo }).then((p) => { pacote = p; desenharPacote(); });

  aoMudarIdioma(() => { mostrarRota(); desenharSf(); desenharPacote(); });
}
