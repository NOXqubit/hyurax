// STORAGE e AURON INTELLIGENCE / MACHINE ECONOMY: fluxos em etapas e um mercado de recursos simulado.
import { el, etapas } from "../ui.js";

export function iniciar({ t, engine, calmo, aoMudarIdioma }) {
  // ---------- armazenamento: etapas que acendem em sequência ----------
  const olArm = document.getElementById("arm-etapas");
  let passoArm = 0;
  function desenharArm() {
    olArm.replaceChildren(...t("arm.etapas").map(([n, txt], i) => el("li", { class: i < passoArm ? "feita" : i === passoArm ? "ativa" : "" },
      el("span", { text: n }), el("small", { class: "texto-2", style: "font-family:var(--display);letter-spacing:0;text-transform:none;font-size:.82rem;margin:.3rem 0 0 .1rem", text: txt }))));
  }
  desenharArm();

  // ---------- inteligência e economia ----------
  const abas = document.getElementById("ia-abas");
  const olIa = document.getElementById("ia-etapas");
  const explica = document.getElementById("ia-explica");
  const recursosEl = document.getElementById("ia-recursos");
  const estadoEl = document.getElementById("ia-estado");
  let aba = "agente", passoIa = 0, marca = () => {};
  const RECURSOS = ["COMPUTE", "STORAGE", "BANDWIDTH", "TASKS", "SERVICES"];
  const mercado = RECURSOS.map(() => ({ oferta: 0.3 + engine.aleat() * 0.6, procura: 0.3 + engine.aleat() * 0.6 }));

  function montarIa() {
    [...abas.children].forEach((b) => b.setAttribute("aria-selected", String(b.dataset.aba === aba)));
    recursosEl.hidden = aba !== "mercado";
    if (aba === "mercado") {
      olIa.replaceChildren(...["COMPUTE", "STORAGE", "BANDWIDTH", "TASKS", "SERVICES"].map((r) => el("li", {}, el("span", { text: r }))));
      marca = () => {};
      explica.textContent = t("ia.mercado");
      desenharMercado();
      estadoEl.replaceChildren();
      return;
    }
    const lista = t(`ia.${aba}`);
    marca = etapas(olIa, lista.map((x) => x[0]));
    passoIa = Math.min(passoIa, lista.length - 1);
    marca(passoIa);
    explica.textContent = lista[passoIa][1];
    estadoEl.replaceChildren(el("p", { class: "nota", style: "margin-top:1rem", text: aba === "logistica" ? "" : t("ia.politica") }));
  }
  function desenharMercado() {
    recursosEl.replaceChildren(...RECURSOS.map((r, i) => {
      const m = mercado[i];
      return el("div", { class: "recurso" }, el("b", { text: r }),
        el("div", { class: "barras" }, el("i", { class: "oferta", style: `width:${(m.oferta * 100).toFixed(0)}%` }), el("i", { class: "procura", style: `width:${(m.procura * 100).toFixed(0)}%` })),
        el("span", { text: `${(m.procura / m.oferta).toFixed(2)}×` }));
    }), el("p", { class: "mono texto-2", style: "font-size:.66rem;margin:.3rem 0 0" }, el("span", { style: "color:var(--prata)", text: "━ " + t("ia.oferta") }), "   ", el("span", { style: "color:var(--luz)", text: "━ " + t("ia.procura") }), "   SIMULATION"));
  }
  abas.addEventListener("click", (e) => { const b = e.target.closest("button[data-aba]"); if (!b) return; aba = b.dataset.aba; passoIa = 0; montarIa(); });
  abas.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    const bs = [...abas.children], i = bs.findIndex((b) => b.dataset.aba === aba), j = (i + (e.key === "ArrowRight" ? 1 : bs.length - 1)) % bs.length;
    aba = bs[j].dataset.aba; passoIa = 0; montarIa(); bs[j].focus();
  });
  montarIa();

  // relógio compartilhado: avança os fluxos só quando estão na tela
  const visivel = { arm: false, ia: false };
  new IntersectionObserver((es) => { visivel.arm = es[0].isIntersecting; }).observe(olArm);
  new IntersectionObserver((es) => { visivel.ia = es[0].isIntersecting; }).observe(olIa);
  if (!calmo) setInterval(() => {
    if (document.hidden) return;
    if (visivel.arm) { passoArm = (passoArm + 1) % 6; desenharArm(); }
    if (visivel.ia) {
      if (aba === "mercado") { mercado.forEach((m) => { m.oferta = Math.min(0.95, Math.max(0.12, m.oferta + (engine.aleat() - 0.5) * 0.18)); m.procura = Math.min(0.95, Math.max(0.12, m.procura + (engine.aleat() - 0.5) * 0.18)); }); desenharMercado(); }
      else { const lista = t(`ia.${aba}`); passoIa = (passoIa + 1) % lista.length; marca(passoIa); explica.textContent = lista[passoIa][1]; }
    }
  }, 1700);
  aoMudarIdioma(() => { desenharArm(); montarIa(); });
}
