// USEFULPOW / UTRAX: mercado de tarefas com conferência de Freivalds de verdade.
import { el, etapas, espera, selo, sequencia } from "../ui.js";
import { SimulationEngine, curto } from "../simulation/engine.js";

export function iniciar({ t, engine, calmo, aoMudarIdioma }) {
  const marca = etapas(document.getElementById("utrax-etapas"), ["TASK", "TASK MARKET", "NODE SELECTION", "COMPUTATION", "VERIFICATION", "PROOF", "REWARD"]);
  const candEl = document.getElementById("utrax-candidatos");
  const vered = document.getElementById("utrax-veredito");
  const prova = document.getElementById("utrax-prova");
  const trapaca = document.getElementById("utrax-trapaca");
  const botao = document.getElementById("utrax-rodar");
  const seq = sequencia();
  const N = 8;
  let A = engine.matriz(N), B = engine.matriz(N), C = SimulationEngine.multiplica(A, B), mexido = null;
  let estado = { passo: -1, executor: "", verificador: "", resultado: null };

  function matriz(id, M, marcado) {
    document.getElementById(id).replaceChildren(...M.flatMap((l, i) => l.map((x, j) => el("span", { class: marcado && marcado[0] === i && marcado[1] === j ? "mexido" : "", text: String(x) }))));
  }
  function candidatos(escolhido, verificador) {
    candEl.replaceChildren(...["C07", "F12", "K31", "M04", "P22"].map((id) => el("div", { class: id === escolhido ? "escolhido" : id === verificador ? "verificador" : "" },
      el("b", { text: `#${id}` }), el("span", { text: id === escolhido ? "EXECUTOR" : id === verificador ? "VERIFIER" : "BID" }))));
  }
  function tarefas() {
    document.getElementById("utrax-tarefas").replaceChildren(...t("utrax.tipos").map((tp) => el("li", {}, el("b", { text: tp.nome }), selo(tp.estado, t), el("p", { text: tp.texto }))));
  }
  function explicar() {
    if (estado.passo < 0) { vered.className = "veredito"; vered.textContent = t("utrax.etapas")[0]; return; }
    const r = estado.resultado;
    if (estado.passo < 6 || !r) { vered.className = "veredito"; vered.textContent = t("utrax.etapas")[estado.passo].replace("{n}", estado.executor).replace("{v}", estado.verificador); }
    if (r) {
      vered.className = "veredito " + (r.ok ? "ok" : "erro");
      vered.replaceChildren(el("span", { text: r.ok ? t("utrax.aceito", { k: r.rodada }) : t("utrax.recusado", { k: r.rodada }) }),
        el("small", { text: r.ok ? t("utrax.aceito_s", { c: r.custo, r: r.refazer }) : t("utrax.recusado_s") }));
      const p = (k) => t(`utrax.prova.${k}`);
      prova.replaceChildren(
        el("dt", { text: p("tarefa") }), el("dd", { text: "MATMUL 8×8 · AURON-UTRAX-FREIVALDS-v1" }),
        el("dt", { text: p("executor") }), el("dd", { text: `#${estado.executor}` }),
        el("dt", { text: p("verificador") }), el("dd", { text: `#${estado.verificador}` }),
        el("dt", { text: p("rodadas") }), el("dd", { text: String(r.rodada) }),
        el("dt", { text: p("custo") }), el("dd", { text: `${r.custo} × / ${r.refazer} ×` }),
        el("dt", { text: p("resultado") }), el("dd", { text: curto(r.semente, 10) }),
        el("dt", { text: p("recompensa") }), el("dd", { class: r.ok ? "ok" : "erro", text: r.ok ? t("utrax.recompensa_v") : t("utrax.retida") }),
      );
    }
  }

  async function rodar() {
    const s = seq.nova();
    const pausa = (ms) => s.espera(calmo ? 0 : ms);
    botao.disabled = true;
    estado = { passo: 0, executor: "", verificador: "", resultado: null };
    prova.replaceChildren();
    A = engine.matriz(N); B = engine.matriz(N); C = SimulationEngine.multiplica(A, B); mexido = null;
    matriz("utrax-a", A); matriz("utrax-b", B); document.getElementById("utrax-c").replaceChildren();
    candidatos(); marca(0); explicar();
    if (!(await pausa(1100))) return;
    estado.passo = 1; marca(1); explicar(); if (!(await pausa(1100))) return;
    const ids = ["C07", "F12", "K31", "M04", "P22"], e = engine.int(5); let v = engine.int(4); if (v >= e) v++;
    estado.executor = ids[e]; estado.verificador = ids[v];
    estado.passo = 2; marca(2); candidatos(ids[e], ids[v]); explicar(); if (!(await pausa(1200))) return;
    estado.passo = 3; marca(3); explicar();
    if (trapaca.checked) { const i = engine.int(N), j = engine.int(N); C = C.map((l) => l.slice()); C[i][j] += 1 + engine.int(5); mexido = [i, j]; }
    matriz("utrax-c", C, mexido); if (!(await pausa(1200))) return;
    estado.passo = 4; marca(4); explicar();
    const r = await engine.verifyTask(A, B, C, 3);
    if (!(await pausa(1300))) return;
    if (!r.ok) { marca(4, true); estado.resultado = r; estado.passo = 6; explicar(); botao.disabled = false; return; }
    estado.passo = 5; marca(5); explicar(); if (!(await pausa(900))) return;
    estado.passo = 6; marca(6); estado.resultado = r; explicar();
    botao.disabled = false;
  }

  botao.addEventListener("click", rodar);
  matriz("utrax-a", A); matriz("utrax-b", B); matriz("utrax-c", C);
  candidatos(); tarefas(); explicar();
  const obs = new IntersectionObserver((es) => { if (es[0].isIntersecting) { obs.disconnect(); rodar(); } }, { threshold: 0.3 });
  obs.observe(botao);
  aoMudarIdioma(() => { tarefas(); explicar(); });
}
