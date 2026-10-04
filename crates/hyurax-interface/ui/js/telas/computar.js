// Hyurax / Ultrax — Computar (Cloud Design 2.0): rodar um cálculo em três
// toques. O que calcular (quatro motores com parâmetros prontos), quanto
// conferir (três níveis em português simples) e quantas partes. O custo sai
// de /ciencia/estimar; o pedido vai para /ciencia/submeter. Os ajustes finos
// de cada motor continuam em "Cálculos avançados".

import { $, el, texto, fmt, trocar } from "../util.js";
import { obter, postar } from "../api.js";
import { estado } from "../estado.js";
import { creditos, resultado } from "./comum.js";

// parâmetros prontos de cada motor (as mesmas faixas de "Cálculos avançados")
const MOTORES = [
  { id: 1, sigla: "M×", titulo: "Matrizes", texto: "A conta por trás da IA. Conferida sem refazer tudo (Freivalds).", tamanho: 128, passos: 0, parametros: [] },
  { id: 4, sigla: "IA", titulo: "IA de moléculas", texto: "Uma rede neural aprende a solubilidade de moléculas reais.", tamanho: 32, passos: 500, parametros: [] },
  { id: 7, sigla: "Rt", titulo: "Rotas", texto: "Acha um caminho curto entre 200 cidades.", tamanho: 200, passos: 1000, parametros: [11] },
  { id: 5, sigla: "Gn", titulo: "Genética", texto: "Simula uma população ao longo de 200 gerações.", tamanho: 500, passos: 200, parametros: [8, 0, 10100, 50, 1000] },
];
const NIVEIS = [
  { id: "rapido", titulo: "Rápido", texto: "Este computador refaz cada parte para conferir.", nivel: 2, redundancia: 1, maquinas: 1 },
  { id: "confiavel", titulo: "Confiável", texto: "Outro computador também calcula; as duas respostas têm de bater.", nivel: 3, redundancia: 2, maquinas: 2 },
  { id: "maximo", titulo: "Máximo", texto: "Três computadores; vale a maioria, conferida aqui.", nivel: 3, redundancia: 3, maquinas: 3 },
];
const NOMES = { RUNNING: ["luz", "Calculando"], PAUSED: ["", "Pausado"], "WAITING FOR NODES": ["atencao", "Esperando computadores"], COMPLETED: ["ok", "Pronto"], CANCELLED: ["", "Cancelado"], EXPIRED: ["falha", "Venceu o prazo"], "OUT OF BUDGET": ["atencao", "Sem créditos"] };

const est = { motor: 1, nivel: "rapido", catalogo: null, jobs: [], estimativa: null, pedindo: 0 };

function dominioDo(motor) {
  const d = (est.catalogo || []).find((x) => x.motores.some((m) => m.codigo === motor));
  return d?.codigo;
}

function pedido() {
  const m = MOTORES.find((x) => x.id === est.motor);
  const n = NIVEIS.find((x) => x.id === est.nivel);
  const dominio = dominioDo(m.id);
  if (!dominio) return null;
  return {
    dominio,
    tipo: m.id,
    tamanho: m.tamanho,
    passos: m.passos,
    parametros: m.parametros.join(","),
    unidades: Math.max(1, Math.round(Number($("cp-partes").value) || 1)),
    nivel: n.nivel,
    redundancia: n.redundancia,
    prazo_s: 0,
    orcamento_milicreditos: 0,
    descricao: `${m.titulo} · ${n.titulo.toLowerCase()}`,
  };
}

function desenharEscolhas() {
  trocar(
    "cp-motores",
    MOTORES.map((m) => {
      const b = el("button", { type: "button", class: "escolha", "aria-pressed": String(m.id === est.motor) }, el("span", { class: "sigla" }, m.sigla), el("b", {}, m.titulo), el("span", {}, m.texto));
      b.addEventListener("click", () => {
        est.motor = m.id;
        desenharEscolhas();
        estimar();
      });
      return b;
    }),
  );
  trocar(
    "cp-niveis",
    NIVEIS.map((n) => {
      const b = el("button", { type: "button", role: "radio", class: "escolha nivel", "aria-checked": String(n.id === est.nivel) }, el("b", {}, n.titulo), el("span", {}, n.texto));
      b.addEventListener("click", () => {
        est.nivel = n.id;
        desenharEscolhas();
        estimar();
      });
      return b;
    }),
  );
  resumo();
}

function resumo() {
  const m = MOTORES.find((x) => x.id === est.motor);
  const n = NIVEIS.find((x) => x.id === est.nivel);
  const e = estado.atual;
  trocar(
    "cp-resumo",
    el("dt", {}, "Cálculo"),
    el("dd", {}, m.titulo),
    el("dt", {}, "Conferência"),
    el("dd", {}, n.titulo),
    el("dt", {}, "Computadores"),
    el("dd", {}, String(n.maquinas)),
  );
  const est_ = est.estimativa;
  texto("cp-custo", est_ ? creditos(est_.milicreditos) : "—");
  texto("cp-tempo", est_ ? `Cerca de ${fmt(est_.segundos_estimados, 0)} s nesta máquina${est_.ritmo_medido ? "" : " (estimativa inicial)"}. Parte errada não é cobrada.` : "");
  const fora = (e?.no?.pares || 0) + 1;
  texto(
    "cp-aviso",
    n.maquinas > 1 && fora < n.maquinas
      ? `Este nível precisa de ${n.maquinas} computadores; agora há ${fora} (este e ${fora - 1} conectado(s)). O cálculo espera os outros chegarem.`
      : "",
  );
  $("cp-enviar").disabled = !e?.pode_mandar || !dominioDo(m.id);
}

async function estimar() {
  const p = pedido();
  if (!p) return resumo();
  const meu = ++est.pedindo;
  const r = await postar("/ciencia/estimar", p);
  if (meu !== est.pedindo) return;
  est.estimativa = r.ok ? r.dados : null;
  resumo();
}

async function lerJobs() {
  const r = await obter("/ciencia");
  if (!r.ok) return;
  est.catalogo = r.dados.dominios || [];
  est.jobs = r.dados.jobs || [];
  desenharJobs();
  resumo();
}

function desenharJobs() {
  const jobs = est.jobs.slice(0, 8);
  trocar(
    "cp-lista",
    jobs.length
      ? jobs.map((j) => {
          const [cor, nome] = NOMES[j.estado] || ["", j.estado];
          const pct = Math.round((j.progresso || 0) * 100);
          return el(
            "div",
            { class: "linha-job" },
            el("span", { class: "nome" }, el("b", {}, j.descricao || j.descricao_tipo), el("small", {}, `${fmt(j.feitas)} de ${fmt(j.unidades)} partes · ${j.nivel_nome}${j.consumo?.milicreditos ? ` · ${creditos(j.consumo.milicreditos)}` : ""}`)),
            el("span", { class: "barra-caixa" }, el("span", { class: `progresso${j.estado === "COMPLETED" ? " ok" : ""}` }, el("span", { style: `width:${pct}%` })), el("span", { class: "num" }, `${pct}%`)),
            el("span", { class: `pilula ${cor}` }, nome),
          );
        })
      : [el("div", { class: "vazio" }, el("strong", {}, "Nenhum cálculo ainda"), el("span", {}, "Escolha um acima e toque em Enviar cálculo."))],
  );
}

export function montar() {
  desenharEscolhas();
  $("cp-partes").addEventListener("input", () => estimar());
  $("cp-enviar").addEventListener("click", async () => {
    const p = pedido();
    if (!p) return;
    const r = await postar("/ciencia/submeter", p);
    resultado("cp-saida", r, r.ok ? (estado.atual?.ultrax?.ligado ? "Enviado. As partes já estão sendo calculadas." : "Enviado. Ligue \"Ajudar a rede\" no Início para este computador calcular também.") : "");
    if (r.ok) lerJobs();
  });
}

let relogio = null;
export function aoMostrar() {
  lerJobs().then(estimar);
  relogio ??= setInterval(lerJobs, 3000);
}

export function aoEsconder() {
  clearInterval(relogio);
  relogio = null;
}

export function atualizar() {
  resumo();
}
