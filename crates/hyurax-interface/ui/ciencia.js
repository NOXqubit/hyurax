// ✝ Provérbios 25:2 — “A glória dos reis é esquadrinhar a coisa.”
// Hyurax — a seção Ciência: JOBs, painel técnico, eventos, visão 3D e
// ULTRA BENCHMARK.
//
// Regra desta tela: nada inventado. Cada número vem de /api/ciencia, de
// /api/ciencia/eventos ou de /api/estado; cada cubo da visão 3D é uma
// unidade de JOB (ou um grupo delas, dito na legenda) e muda de cor e de
// altura quando um evento real chega. Nada se move sozinho: sem evento, a
// tela fica parada. O que o nó não mede aparece como "não medido".

import { desenharMolecula } from "/moleculas.js";

const $ = (id) => document.getElementById(id);
function texto(id, v) {
  const el = $(id);
  if (el && el.textContent !== String(v)) el.textContent = v;
}
const fmt = (n, casas = 0) => (Number.isFinite(n) ? n.toLocaleString("pt-BR", { maximumFractionDigits: casas, minimumFractionDigits: casas }) : "—");
const curto = (hex) => (hex ? `${hex.slice(0, 8)}…${hex.slice(-4)}` : "—");
const hora = (ms) => (ms ? new Date(ms).toLocaleTimeString("pt-BR", { hour12: false }) : "—");
const OPS_POR_MOLECULA = 183;

async function obter(caminho) {
  try {
    const r = await fetch(caminho, { cache: "no-store" });
    if (!r.ok) return null;
    return await r.json();
  } catch {
    return null;
  }
}

async function postar(caminho, campos = {}) {
  try {
    const r = await fetch(caminho, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams(campos),
    });
    const dados = r.status === 204 ? null : await r.json().catch(() => null);
    return { ok: r.ok, status: r.status, dados };
  } catch {
    return { ok: false, status: 0, dados: null };
  }
}

const erroDe = (res, padrao) => res?.dados?.erro || (res?.status === 403 ? "Só vale do próprio computador, com o programa destrancado." : padrao);

// ---------------------------------------------------------------------------
// Estado desta tela
// ---------------------------------------------------------------------------

const est = {
  lista: null, // /api/ciencia
  noEstado: null, // /api/estado (vem do painel.js)
  seq: 0,
  eventos: [], // os últimos, para o fluxo
  ativas: new Map(), // "job#i" -> último WORK_UNIT_PROGRESS
  selecionado: null, // JOB_ID escolhido para a visão 3D
  detalhe: null, // /api/ciencia/job/<id>
  historico: null, // /api/ciencia/job/<id>/historico (inspeção e reprodução)
  unidadeEscolhida: null,
  reproducao: null, // { t0, t1, relogio, vel, registros }
  pulsos: [], // { de, para, inicio } — só de eventos WORK_UNIT_ASSIGNED reais
  vazaoOps: [], // [ms, ops] das unidades conferidas, para a vazão real
};

addEventListener("hyurax:estado", (ev) => {
  est.noEstado = ev.detail;
});

// ---------------------------------------------------------------------------
// Motores: campos do formulário (faixas e unidades iguais às dos motores)
// ---------------------------------------------------------------------------

// Cada campo: rótulo, mínimo, máximo, padrão, e como vira o u32 da especificação.
const M = {
  1: { tamanho: ["Lado da matriz", 1, 1024, 256], unidades: 12 },
  2: { tamanho: ["Itens", 1, 256, 64], unidades: 12 },
  3: { tamanho: ["Lado da grade", 2, 256, 64], passos: ["Passos", 1, 1024, 100], unidades: 12 },
  4: { tamanho: ["Lote", 1, 256, 32], passos: ["Passos de treino", 1, 4096, 500], unidades: 6 },
  5: {
    tamanho: ["Indivíduos (N)", 2, 100000, 500],
    passos: ["Gerações", 1, 10000, 200],
    parametros: [
      ["Loci", 1, 64, 8, (v) => v],
      ["Mutação (por milhão)", 0, 10000, 0, (v) => v],
      ["Seleção s (−1 a +1)", -1, 1, 0.01, (v) => Math.round(v * 10000) + 10000, 0.001],
      ["Dominância h (%)", 0, 100, 50, (v) => v],
      ["Frequência inicial p0", 0, 1, 0.1, (v) => Math.round(v * 10000), 0.01],
    ],
    unidades: 20,
  },
  6: {
    tamanho: ["Plantas (N)", 4, 20000, 200],
    passos: ["Gerações de seleção", 1, 1000, 10],
    parametros: [
      ["QTL", 1, 200, 20, (v) => v],
      ["Arquitetura (código)", 0, 4294967295, 42, (v) => v],
      ["Selecionadas (%)", 1, 100, 20, (v) => v],
      ["Ruído ambiental (% do desvio genético)", 0, 1000, 50, (v) => v],
      ["Água (% da demanda)", 0, 200, 60, (v) => v],
      ["Nitrogênio (% da dose)", 0, 200, 100, (v) => v],
      ["Solo (% da referência)", 0, 200, 100, (v) => v],
      ["Interação G×E (%)", 0, 100, 30, (v) => v],
    ],
    unidades: 8,
  },
  7: {
    tamanho: ["Cidades", 4, 2000, 200],
    passos: ["Teto de passadas do 2-opt", 1, 10000, 1000],
    parametros: [["Instância (código)", 0, 4294967295, 11, (v) => v]],
    unidades: 12,
  },
  8: {
    tamanho: ["Moléculas por unidade", 1, 4096, 500],
    parametros: [
      ["Catálogo (0 = AqSolDB)", 0, 0, 0, (v) => v],
      ["Começa na molécula", 0, 8288, 0, (v) => v],
      ["Massa máxima (g/mol)", 0, 2000, 500, (v) => Math.round(v * 1000)],
      ["logP máximo", -20, 20, 5, (v) => Math.round(v * 1000) + 20000, 0.1],
      ["Doadores de H máximos", 0, 100, 5, (v) => v],
      ["Aceptores de H máximos", 0, 100, 10, (v) => v],
      ["Ligações giratórias máximas", 0, 100, 10, (v) => v],
      ["Área polar máxima (Å²)", 0, 1000, 140, (v) => Math.round(v * 1000)],
      ["log S previsto mínimo", -20, 20, -4, (v) => Math.round(v * 1000) + 20000, 0.1],
    ],
    unidades: 17,
  },
};

function montarFormulario() {
  const dominios = est.lista?.dominios || [];
  const sel = $("nj-dominio");
  if (!sel || sel.options.length === dominios.length) return;
  sel.replaceChildren();
  for (const d of dominios) {
    const o = document.createElement("option");
    o.value = d.codigo;
    o.textContent = d.motores.length ? `${d.nome} — ${d.motores.map((m) => m.descricao).join(", ")}` : `${d.nome} — sem motor (não calcula nada)`;
    o.disabled = d.motores.length === 0;
    sel.append(o);
  }
  sel.value = "1";
  trocarDominio();
}

function trocarDominio() {
  const d = (est.lista?.dominios || []).find((x) => String(x.codigo) === $("nj-dominio").value);
  const motor = $("nj-motor");
  motor.replaceChildren();
  for (const m of d?.motores || []) {
    const o = document.createElement("option");
    o.value = m.codigo;
    o.textContent = m.descricao;
    motor.append(o);
  }
  trocarMotor();
}

function campoNumero(id, [rotulo, min, max, padrao, , passo]) {
  const l = document.createElement("label");
  l.className = "nj-campo";
  l.htmlFor = id;
  const s = document.createElement("small");
  s.textContent = `${rotulo} (${fmt(min, passo ? 2 : 0)} a ${fmt(max, passo ? 2 : 0)})`;
  const i = document.createElement("input");
  Object.assign(i, { id, type: "number", min, max, step: passo || 1, value: padrao, autocomplete: "off" });
  l.append(s, i);
  return l;
}

function trocarMotor() {
  const cfg = M[$("nj-motor").value];
  const caixa = $("nj-campos");
  caixa.replaceChildren();
  if (!cfg) return;
  caixa.append(campoNumero("nj-tamanho", cfg.tamanho));
  if (cfg.passos) caixa.append(campoNumero("nj-passos", cfg.passos));
  (cfg.parametros || []).forEach((p, k) => caixa.append(campoNumero(`nj-p${k}`, p)));
  $("nj-unidades").value = cfg.unidades;
  texto("nj-saida", "");
}

function pedidoDoFormulario() {
  const cfg = M[$("nj-motor").value];
  if (!cfg) return null;
  const num = (id) => Number($(id)?.value);
  const parametros = (cfg.parametros || []).map((p, k) => p[4](num(`nj-p${k}`)));
  return {
    dominio: $("nj-dominio").value,
    tipo: $("nj-motor").value,
    tamanho: String(Math.round(num("nj-tamanho"))),
    passos: String(cfg.passos ? Math.round(num("nj-passos")) : 0),
    parametros: parametros.join(","),
    unidades: String(Math.round(num("nj-unidades"))),
    nivel: $("nj-nivel").value,
    redundancia: String(Math.round(num("nj-redundancia"))),
    prazo_s: String(Math.round(num("nj-prazo"))),
    orcamento_milicreditos: String(Math.round(num("nj-orcamento") * 1000)),
    descricao: $("nj-descricao").value,
  };
}

async function estimar() {
  const p = pedidoDoFormulario();
  if (!p) return;
  const r = await postar("/api/ciencia/estimar", p);
  if (!r.ok) {
    texto("nj-saida", erroDe(r, "Não deu para estimar."));
    return;
  }
  const e = r.dados;
  texto(
    "nj-saida",
    `Estimativa: ${fmt(e.operacoes)} operações de execução e ${fmt(e.operacoes_verificacao)} de conferência; ` +
      `${fmt(e.milicreditos / 1000, 3)} créditos (não é HYX); ${fmt(e.memoria_por_unidade / 1048576, 1)} MiB por unidade; ` +
      `cerca de ${fmt(e.segundos_estimados, 1)} s com ${e.linhas} linha(s) a ${fmt(e.ritmo / 1e6, 1)} M ops/s ` +
      (e.ritmo_medido ? "(ritmo medido nesta máquina). " : "(ritmo inicial: este motor ainda não foi medido nesta abertura, então o tempo é um chute). ") +
      `É o teto do modelo de custo: o trabalho real costuma ser menor.`,
  );
}

async function submeter() {
  const p = pedidoDoFormulario();
  if (!p) return;
  const r = await postar("/api/ciencia/submeter", p);
  if (!r.ok) {
    texto("nj-saida", erroDe(r, "O nó recusou o JOB."));
    return;
  }
  est.selecionado = r.dados.id;
  texto("nj-saida", `JOB ${curto(r.dados.id)} na fila. Ele roda quando o ULTRAX estiver ligado.`);
  lerLista();
}

// ---------------------------------------------------------------------------
// Leituras
// ---------------------------------------------------------------------------

async function lerLista() {
  const l = await obter("/api/ciencia");
  if (!l) return;
  est.lista = l;
  montarFormulario();
  if (!est.selecionado && l.jobs.length) est.selecionado = l.jobs[0].id;
  desenharLista();
  desenharPainelTecnico();
  desenharRede();
}

function desenharRede() {
  const r = est.lista?.rede;
  if (!r) return;
  const caixa = $("cj-aceitar");
  if (caixa && document.activeElement !== caixa) caixa.checked = !!r.aceitar;
  const reps = (r.reputacao || []).map((x) => `${x.worker.slice(0, 8)}… nota ${x.nota} (${x.verificadas} aceitas, ${x.divergentes} divergentes, ${x.recusadas} recusadas)`);
  texto(
    "cj-rede-texto",
    `${r.ofertas.length} worker(s) de outros nós oferecendo agora · ${r.remotas_executando} unidade(s) de outros nós rodando aqui, ${r.remotas_na_fila} na fila · ` +
      `${r.redundantes} unidade(s) nossas esperando outros nós · mensagens ${r.mensagens_recebidas} recebidas, ${r.mensagens_enviadas} enviadas` +
      (reps.length ? ` · reputação medida aqui: ${reps.join("; ")}` : ""),
  );
}

async function lerDetalhe() {
  if (!est.selecionado) return;
  const d = await obter(`/api/ciencia/job/${est.selecionado}`);
  if (d) {
    est.detalhe = d;
    desenharDetalhe();
  }
}

async function lerEventos() {
  const r = await obter(`/api/ciencia/eventos?desde=${est.seq}`);
  if (!r) return;
  est.seq = Math.max(est.seq, r.ultimo);
  for (const ev of r.eventos) receberEvento(ev);
  if (r.eventos.length) desenharEventos();
}

function receberEvento(ev) {
  est.eventos.push(ev);
  if (est.eventos.length > 300) est.eventos.shift();
  const chave = `${ev.job_id}#${ev.unit_index}`;
  if (ev.event === "WORK_UNIT_PROGRESS") est.ativas.set(chave, ev);
  if (["WORK_UNIT_VERIFIED", "WORK_UNIT_REQUEUED", "WORK_UNIT_FAILED", "WORK_UNIT_RETRY"].includes(ev.event)) est.ativas.delete(chave);
  if (ev.event === "WORK_UNIT_VERIFIED" && ev.throughput && ev.execution_time_ms) {
    est.vazaoOps.push([ev.timestamp, ev.throughput * ev.execution_time_ms / 1000]);
  }
  if (ev.event === "WORK_UNIT_ASSIGNED" && ev.job_id === est.selecionado && !est.reproducao) {
    est.pulsos.push({ indice: ev.unit_index, inicio: performance.now() });
  }
}

// ---------------------------------------------------------------------------
// Painel técnico
// ---------------------------------------------------------------------------

function desenharPainelTecnico() {
  const jobs = est.lista?.jobs || [];
  const e = est.noEstado;
  const u = e?.ultrax;
  const soma = (f) => jobs.reduce((s, j) => s + (f(j) || 0), 0);
  const pares = Number(e?.pares) || 0;
  const ofertas = est.lista?.rede?.ofertas?.length || 0;
  texto("ct-nos", `${1 + pares}`);
  texto("ct-nos-nota", pares ? `este nó e ${pares} par(es) conectado(s); ${ofertas} oferecendo trabalho` : "só este nó");
  texto("ct-jobs", fmt(jobs.filter((j) => j.estado === "RUNNING").length));
  texto("ct-jobs-nota", `${jobs.length} no total; ${jobs.filter((j) => j.estado === "WAITING FOR NODES").length} esperando outros nós`);
  texto("ct-unidades", `${fmt(soma((j) => j.feitas))} / ${fmt(soma((j) => j.unidades))}`);
  texto("ct-voo", fmt(soma((j) => j.em_voo)));
  texto("ct-fila", u ? fmt(u.fila?.length ?? u.fila ?? 0) : "—");
  texto("ct-cpu", u ? `${u.linhas} × ${u.uso_cpu}%` : "—");
  const ultimoProgresso = [...est.ativas.values()].pop();
  texto("ct-cpu-nota", ultimoProgresso?.cpu_usage != null ? `limite por linha; medido na última unidade: ${fmt(ultimoProgresso.cpu_usage, 0)}%` : "limite por linha");
  texto("ct-gpu", u?.gpu?.ligada ? `${u.gpu.uso}% (limite)` : "desligada");
  texto("ct-gpu-nota", u?.gpu?.ligada ? `${u.gpu.nome || "GPU"} · ${fmt((u.gpu.ritmo || 0) / 1e6, 0)} M ops/s medidos · uso e temperatura da placa: indisponíveis pelo WebGL` : "as unidades de JOB rodam na CPU");
  texto("ct-vram", "não medida");
  texto("ct-ram", u ? `${fmt(u.reservada_mib ?? 0, 1)} MiB` : "—");
  texto("ct-ram-nota", u ? `reservada pelo ULTRAX, teto ${u.memoria_mib} MiB` : "");
  const rd = est.lista?.rede;
  texto("ct-rede", rd ? `${fmt(rd.mensagens_recebidas)} ↓ · ${fmt(rd.mensagens_enviadas)} ↑` : "—");
  // vazão real: operações das unidades conferidas no último minuto
  const agora = Date.now();
  est.vazaoOps = est.vazaoOps.filter(([t]) => agora - t < 60000);
  const opsMinuto = est.vazaoOps.reduce((s, [, o]) => s + o, 0);
  texto("ct-vazao", est.vazaoOps.length ? `${fmt(opsMinuto / 60 / 1e6, 2)} M ops/s` : "—");
  texto("ct-ops", fmt(soma((j) => j.consumo.operacoes)));
  const candidatos = jobs.filter((j) => j.tipo === "molecular-screening").reduce((s, j) => s + j.consumo.operacoes / OPS_POR_MOLECULA, 0);
  texto("ct-candidatos", fmt(candidatos));
  texto("ct-verificadas", fmt(soma((j) => j.feitas)));
  texto("ct-falhas", fmt(soma((j) => j.falhas)));
  texto("ct-repetidas", fmt(soma((j) => j.recusas + j.repetidas)));
  texto("ct-energia", e?.watts != null ? `${fmt(e.watts, 0)} W (estimativa)` : "não medida");
  texto("ct-tempo", `${fmt(soma((j) => j.consumo.cpu_ms) / 1000, 1)} s de CPU`);
  texto("ct-creditos", fmt(soma((j) => j.consumo.milicreditos) / 1000, 3));
}

// ---------------------------------------------------------------------------
// Lista de JOBs
// ---------------------------------------------------------------------------

function botao(rotulo, fn, classe = "link") {
  const b = document.createElement("button");
  b.type = "button";
  b.className = classe;
  b.textContent = rotulo;
  b.addEventListener("click", fn);
  return b;
}

async function mudar(id, acao) {
  const r = await postar(`/api/ciencia/job/${id}/${acao}`);
  if (!r.ok) alert(erroDe(r, "Não deu."));
  lerLista();
}

function desenharLista() {
  const caixa = $("cj-lista");
  const jobs = est.lista?.jobs || [];
  texto("cj-total", jobs.length ? `${jobs.length} JOB(s)` : "nenhum JOB");
  $("cj-vazio").hidden = jobs.length > 0;
  caixa.replaceChildren(
    ...jobs.map((j) => {
      const linha = document.createElement("article");
      linha.className = "cj-job" + (j.id === est.selecionado ? " escolhido" : "");
      const topo = document.createElement("header");
      const nome = document.createElement("b");
      nome.textContent = `${j.descricao_tipo} · ${j.resumo}`;
      const estado = document.createElement("span");
      estado.className = "cj-estado";
      estado.textContent = j.estado;
      topo.append(nome, estado);
      const barra = document.createElement("div");
      barra.className = "cj-barra";
      barra.setAttribute("role", "progressbar");
      barra.setAttribute("aria-valuenow", String(Math.round(j.progresso * 100)));
      const cheio = document.createElement("i");
      cheio.style.width = `${(j.progresso * 100).toFixed(2)}%`;
      barra.append(cheio);
      const fatos = document.createElement("p");
      fatos.className = "cj-fatos";
      fatos.textContent =
        `JOB ${curto(j.id)} · ${fmt(j.feitas)}/${fmt(j.unidades)} conferidas · ${j.falhas} falha(s) · ${j.em_voo} em voo · ` +
        `verificação ${j.nivel_nome} · ${fmt(j.consumo.milicreditos / 1000, 3)} créditos` + (j.motivo ? ` · ${j.motivo}` : "");
      const agregado = document.createElement("p");
      agregado.className = "cj-agregado";
      agregado.textContent = j.agregado;
      const acoes = document.createElement("div");
      acoes.className = "cj-acoes";
      acoes.append(botao("ver na 3D", () => escolher(j.id)));
      if (j.estado === "RUNNING" || j.estado === "WAITING FOR NODES") acoes.append(botao("pausar", () => mudar(j.id, "pausar")));
      if (j.estado === "PAUSED" || j.estado === "OUT OF BUDGET") acoes.append(botao("retomar", () => mudar(j.id, "retomar")));
      if (!["COMPLETED", "CANCELLED", "EXPIRED"].includes(j.estado)) acoes.append(botao("cancelar", () => confirm("Cancelar este JOB?") && mudar(j.id, "cancelar")));
      for (const f of ["json", "csv", "pdf"]) {
        const a = document.createElement("a");
        a.className = "link";
        a.href = `/api/ciencia/job/${j.id}/relatorio.${f}`;
        a.textContent = `relatório ${f.toUpperCase()}`;
        a.target = "_blank";
        acoes.append(a);
      }
      acoes.append(botao("reproduzir", () => reproduzir(j.id)));
      if (j.descricao) {
        const d = document.createElement("p");
        d.className = "cj-descricao";
        d.textContent = j.descricao;
        linha.append(topo, d, barra, fatos, agregado, acoes);
      } else {
        linha.append(topo, barra, fatos, agregado, acoes);
      }
      return linha;
    }),
  );
}

function escolher(id) {
  est.selecionado = id;
  est.detalhe = null;
  est.historico = null;
  est.unidadeEscolhida = null;
  est.reproducao = null;
  $("v3-inspecao-caixa").hidden = true;
  texto("v3-modo", "AO VIVO");
  desenharLista();
  lerDetalhe();
}

// ---------------------------------------------------------------------------
// Fluxo de eventos
// ---------------------------------------------------------------------------

function desenharEventos() {
  const caixa = $("ev-lista");
  if (!caixa) return;
  const ultimos = est.eventos.slice(-120).reverse();
  caixa.replaceChildren(
    ...ultimos.map((ev) => {
      const l = document.createElement("div");
      l.className = "ev-linha";
      const partes = [
        hora(ev.timestamp),
        ev.event,
        ev.work_unit_id || (ev.job_id ? curto(ev.job_id) : ""),
        ev.workload_type || "",
        ev.verification_status || "",
        ev.throughput != null ? `${fmt(ev.throughput / 1e6, 2)} M ops/s` : "",
        ev.result_hash ? `RESULT ${curto(ev.result_hash)}` : "",
        ev.operation || "",
      ];
      l.textContent = partes.filter(Boolean).join("  ·  ");
      l.dataset.tipo = ev.event;
      return l;
    }),
  );
}

// ---------------------------------------------------------------------------
// Visão 3D (WebGL2, sem biblioteca)
// ---------------------------------------------------------------------------

const V = { gl: null, prog: null, vao: null, instancias: null, n: 0, yaw: 0.8, pitch: 0.75, dist: 1.0, cubos: [], arrastando: null };
const COR = {
  pendente: [0.16, 0.16, 0.18],
  fila: [0.42, 0.44, 0.5],
  executando: [0.91, 0.69, 0.29],
  verificando: [0.55, 0.82, 0.9],
  feita: [0.93, 0.93, 0.91],
  falha: [0.85, 0.25, 0.22],
  no: [0.95, 0.95, 0.95],
  par: [0.6, 0.6, 0.62],
  pulso: [0.91, 0.69, 0.29],
};

const VS = `#version 300 es
layout(location=0) in vec3 p; layout(location=1) in vec3 nrm;
layout(location=2) in vec3 pos; layout(location=3) in vec3 tam; layout(location=4) in vec3 cor;
uniform mat4 mvp; out vec3 c; out float l;
void main(){ vec3 w = pos + p * tam; gl_Position = mvp * vec4(w,1.0);
  l = 0.45 + 0.55 * max(dot(nrm, normalize(vec3(0.4,1.0,0.6))), 0.0); c = cor; }`;
const FS = `#version 300 es
precision mediump float; in vec3 c; in float l; out vec4 o; void main(){ o = vec4(c*l,1.0); }`;

function iniciar3D() {
  const canvas = $("v3-canvas");
  if (!canvas || V.gl) return;
  const gl = canvas.getContext("webgl2", { antialias: true });
  if (!gl) {
    texto("v3-legenda", "Este navegador não tem WebGL2: a visão 3D não aparece, e os números continuam nos outros painéis.");
    return;
  }
  const sh = (tipo, src) => {
    const s = gl.createShader(tipo);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    return s;
  };
  const prog = gl.createProgram();
  gl.attachShader(prog, sh(gl.VERTEX_SHADER, VS));
  gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, FS));
  gl.linkProgram(prog);
  // cubo unitário com base em y = 0 (a altura cresce para cima)
  const f = [
    [[0, 1, 0], [[-0.5, 1, -0.5], [0.5, 1, -0.5], [0.5, 1, 0.5], [-0.5, 1, 0.5]]],
    [[0, -1, 0], [[-0.5, 0, 0.5], [0.5, 0, 0.5], [0.5, 0, -0.5], [-0.5, 0, -0.5]]],
    [[1, 0, 0], [[0.5, 0, -0.5], [0.5, 0, 0.5], [0.5, 1, 0.5], [0.5, 1, -0.5]]],
    [[-1, 0, 0], [[-0.5, 0, 0.5], [-0.5, 0, -0.5], [-0.5, 1, -0.5], [-0.5, 1, 0.5]]],
    [[0, 0, 1], [[0.5, 0, 0.5], [-0.5, 0, 0.5], [-0.5, 1, 0.5], [0.5, 1, 0.5]]],
    [[0, 0, -1], [[-0.5, 0, -0.5], [0.5, 0, -0.5], [0.5, 1, -0.5], [-0.5, 1, -0.5]]],
  ];
  const vert = [];
  for (const [n, q] of f) for (const k of [0, 1, 2, 0, 2, 3]) vert.push(...q[k], ...n);
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const vb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vb);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vert), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);
  const ib = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, ib);
  for (const [loc, off] of [[2, 0], [3, 12], [4, 24]]) {
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 36, off);
    gl.vertexAttribDivisor(loc, 1);
  }
  Object.assign(V, { gl, prog, vao, instancias: ib });
  canvas.addEventListener("pointerdown", (e) => { V.arrastando = [e.clientX, e.clientY, false]; canvas.setPointerCapture(e.pointerId); });
  canvas.addEventListener("pointermove", (e) => {
    if (!V.arrastando) return;
    // (arrastar gira a câmera; nada gira sozinho)
    const [x, y] = V.arrastando;
    if (Math.abs(e.clientX - x) + Math.abs(e.clientY - y) > 3) V.arrastando[2] = true;
    V.yaw += (e.clientX - x) * 0.008;
    V.pitch = Math.min(1.45, Math.max(0.15, V.pitch + (e.clientY - y) * 0.008));
    V.arrastando[0] = e.clientX;
    V.arrastando[1] = e.clientY;
  });
  canvas.addEventListener("pointerup", (e) => {
    const moveu = V.arrastando?.[2];
    V.arrastando = null;
    if (!moveu) escolherCubo(e);
  });
  canvas.addEventListener("wheel", (e) => { e.preventDefault(); V.dist = Math.min(3, Math.max(0.35, V.dist * (e.deltaY > 0 ? 1.1 : 0.9))); }, { passive: false });
  requestAnimationFrame(quadro);
}

function mat(a, b) {
  const r = new Array(16).fill(0);
  for (let i = 0; i < 4; i++) for (let j = 0; j < 4; j++) for (let k = 0; k < 4; k++) r[j * 4 + i] += a[k * 4 + i] * b[j * 4 + k];
  return r;
}
function perspectiva(fov, asp, perto, longe) {
  const f = 1 / Math.tan(fov / 2);
  return [f / asp, 0, 0, 0, 0, f, 0, 0, 0, 0, (longe + perto) / (perto - longe), -1, 0, 0, (2 * longe * perto) / (perto - longe), 0];
}
function olhar(olho, alvo) {
  const sub = (a, b) => a.map((v, i) => v - b[i]);
  const norm = (v) => { const l = Math.hypot(...v) || 1; return v.map((x) => x / l); };
  const cruz = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const z = norm(sub(olho, alvo));
  const x = norm(cruz([0, 1, 0], z));
  const y = cruz(z, x);
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  return [x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot(x, olho), -dot(y, olho), -dot(z, olho), 1];
}

/// O estado de cada unidade (ou grupo), só do que o nó informou ou do registro gravado.
function estadosDasUnidades() {
  const d = est.detalhe;
  if (!d) return null;
  const total = d.unidades;
  const cubos = Math.min(total, 4096);
  const grupo = Math.ceil(total / cubos);
  const n = Math.ceil(total / grupo);
  const feitas = new Float32Array(n);
  const estado = new Array(n).fill("pendente");
  const progresso = new Float32Array(n);
  const marcar = (a, b, fn) => {
    for (let g = Math.floor(a / grupo); g <= Math.floor((b - 1) / grupo) && g < n; g++) {
      const ini = Math.max(a, g * grupo);
      const fim = Math.min(b, (g + 1) * grupo);
      fn(g, fim - ini);
    }
  };
  const rep = est.reproducao;
  if (rep) {
    const t = rep.relogio;
    for (const r of rep.registros) {
      const g = Math.floor(r.indice / grupo);
      if (g >= n) continue;
      if (t >= r.verificada) feitas[g] += 1;
      else if (t >= r.fim) estado[g] = "verificando";
      else if (t >= r.inicio) { estado[g] = "executando"; progresso[g] = (t - r.inicio) / Math.max(1, r.fim - r.inicio); }
      else if (t >= r.despachada) estado[g] = "fila";
    }
  } else {
    for (const [a, b] of d.faixas_feitas || []) marcar(a, b, (g, k) => (feitas[g] += k));
    for (const [a, b] of d.faixas_falhas || []) marcar(a, b, (g) => (estado[g] = "falha"));
    for (const v of d.em_voo_lista || []) {
      const g = Math.floor(v.indice / grupo);
      if (g < n) estado[g] = v.inicio ? "executando" : "fila";
    }
    for (const ev of est.ativas.values()) {
      if (ev.job_id !== d.id) continue;
      const g = Math.floor(ev.unit_index / grupo);
      if (g >= n) continue;
      estado[g] = ev.verification_status === "VERIFYING" ? "verificando" : "executando";
      progresso[g] = ev.progress || 0;
    }
  }
  for (let g = 0; g < n; g++) {
    const tam = Math.min(grupo, total - g * grupo);
    if (feitas[g] >= tam && estado[g] === "pendente") estado[g] = "feita";
    feitas[g] /= tam;
  }
  return { n, grupo, estado, feitas, progresso };
}

function quadro() {
  requestAnimationFrame(quadro);
  const { gl } = V;
  if (!gl || $("painel-visao3d")?.hidden) return;
  const canvas = gl.canvas;
  const esc = devicePixelRatio || 1;
  const w = Math.max(1, Math.round(canvas.clientWidth * esc));
  const h = Math.max(1, Math.round(canvas.clientHeight * esc));
  if (canvas.width !== w || canvas.height !== h) Object.assign(canvas, { width: w, height: h });
  if (est.reproducao) avancarReproducao();
  const u = estadosDasUnidades();
  const dados = [];
  V.cubos = [];
  const lado = u ? Math.ceil(Math.sqrt(u.n)) : 1;
  const passo = 1 / lado;
  if (u) {
    for (let g = 0; g < u.n; g++) {
      const x = (g % lado) * passo - 0.5 + passo / 2;
      const z = Math.floor(g / lado) * passo - 0.5 + passo / 2;
      const e = u.estado[g];
      let altura = 0.02;
      let cor = COR.pendente;
      if (e === "feita" || u.feitas[g] > 0) { altura = 0.02 + 0.12 * u.feitas[g]; cor = COR.feita; }
      if (e === "fila") { altura = 0.03; cor = COR.fila; }
      if (e === "executando") { altura = 0.03 + 0.2 * u.progresso[g]; cor = COR.executando; }
      if (e === "verificando") { altura = 0.03 + 0.2 * u.progresso[g]; cor = COR.verificando; }
      if (e === "falha") { altura = 0.12; cor = COR.falha; }
      const t = passo * 0.82;
      dados.push(x, 0, z, t, altura, t, ...cor);
      V.cubos.push({ g, x, y: altura, z });
    }
  }
  // este nó, acima do lote, e os pares conectados em volta (só os que existem)
  const no = [0, 0.55, 0];
  dados.push(no[0], no[1], no[2], 0.05, 0.05, 0.05, ...COR.no);
  const pares = Number(est.noEstado?.pares) || 0;
  for (let k = 0; k < Math.min(pares, 32); k++) {
    const a = (k / Math.max(1, pares)) * Math.PI * 2;
    dados.push(Math.cos(a) * 0.85, 0.35, Math.sin(a) * 0.85, 0.035, 0.035, 0.035, ...COR.par);
  }
  // pulsos: um por evento WORK_UNIT_ASSIGNED real, do nó até o cubo da unidade
  const agora = performance.now();
  est.pulsos = est.pulsos.filter((p) => agora - p.inicio < 700);
  for (const p of est.pulsos) {
    const alvo = u && V.cubos[Math.floor(p.indice / u.grupo)];
    if (!alvo) continue;
    const t = (agora - p.inicio) / 700;
    dados.push(no[0] + (alvo.x - no[0]) * t, no[1] + (alvo.y - no[1]) * t, no[2] + (alvo.z - no[2]) * t, 0.02, 0.02, 0.02, ...COR.pulso);
  }
  gl.viewport(0, 0, w, h);
  gl.clearColor(0.024, 0.024, 0.027, 1);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  gl.enable(gl.DEPTH_TEST);
  gl.useProgram(V.prog);
  const olho = [Math.cos(V.yaw) * Math.cos(V.pitch) * 1.6 * V.dist, Math.sin(V.pitch) * 1.6 * V.dist, Math.sin(V.yaw) * Math.cos(V.pitch) * 1.6 * V.dist];
  V.mvp = mat(perspectiva(0.8, w / h, 0.01, 20), olhar(olho, [0, 0.08, 0]));
  gl.uniformMatrix4fv(gl.getUniformLocation(V.prog, "mvp"), false, new Float32Array(V.mvp));
  gl.bindVertexArray(V.vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, V.instancias);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(dados), gl.DYNAMIC_DRAW);
  gl.drawArraysInstanced(gl.TRIANGLES, 0, 36, dados.length / 9);
  if (u) {
    texto(
      "v3-legenda",
      `${u.n === est.detalhe.unidades ? "cada cubo é uma unidade" : `cada cubo reúne ${u.grupo} unidades`} · altura = progresso real · ` +
        `${est.reproducao ? "reprodução do registro gravado" : "ao vivo, pelos eventos do nó"} · arraste para girar, role para aproximar, clique num cubo para inspecionar`,
    );
  }
}

function escolherCubo(e) {
  if (!V.mvp || !V.cubos.length) return;
  const r = V.gl.canvas.getBoundingClientRect();
  const mx = ((e.clientX - r.left) / r.width) * 2 - 1;
  const my = -(((e.clientY - r.top) / r.height) * 2 - 1);
  let melhor = null;
  for (const c of V.cubos) {
    const m = V.mvp;
    const x = m[0] * c.x + m[4] * c.y + m[8] * c.z + m[12];
    const y = m[1] * c.x + m[5] * c.y + m[9] * c.z + m[13];
    const wv = m[3] * c.x + m[7] * c.y + m[11] * c.z + m[15];
    const d = Math.hypot(x / wv - mx, y / wv - my);
    if (!melhor || d < melhor.d) melhor = { d, g: c.g };
  }
  if (melhor && melhor.d < 0.08) inspecionar(melhor.g);
}

// ---------------------------------------------------------------------------
// Inspecionar cálculo
// ---------------------------------------------------------------------------

async function inspecionar(grupoOuIndice) {
  const d = est.detalhe;
  if (!d) return;
  const grupo = Math.ceil(d.unidades / Math.min(d.unidades, 4096));
  const indice = grupoOuIndice * grupo;
  est.unidadeEscolhida = indice;
  if (!est.historico) est.historico = await obter(`/api/ciencia/job/${d.id}/historico`);
  const reg = (est.historico?.unidades || []).find((u) => u.indice === indice);
  const voo = (d.em_voo_lista || []).find((v) => v.indice === indice);
  const prog = est.ativas.get(`${d.id}#${indice}`);
  const ev = (est.historico?.eventos || []).filter((x) => x.unit_index === indice);
  const campos = [
    ["JOB", d.id],
    ["WORK UNIT", `${d.id.slice(0, 16)}#${indice}${grupo > 1 ? ` (primeira do grupo de ${grupo})` : ""}`],
    ["NODE (worker)", reg?.worker || ev.find((x) => x.node_id)?.node_id || "—"],
    ["INPUT HASH", reg?.entrada || voo?.entrada || "—"],
    ["OUTPUT HASH", reg?.resultado || "ainda não há"],
    ["TAREFA (TASK_ID)", reg?.tarefa || "—"],
    ["DESPACHADA", hora(reg?.despachada || voo?.despachada)],
    ["INÍCIO", hora(reg?.inicio || voo?.inicio)],
    ["FIM", hora(reg?.fim)],
    ["CONFERIDA", hora(reg?.verificada)],
    ["EXECUTION TIME", reg ? `${fmt(reg.ms_calculo)} ms de cálculo + ${fmt(reg.ms_verificacao)} ms de conferência` : prog ? "em execução" : "—"],
    ["CPU", reg ? `${fmt(Math.min(100, (reg.ms_calculo * 100) / Math.max(1, reg.fim - reg.inicio)), 0)}% da linha (cálculo ÷ relógio)` : prog?.cpu_usage != null ? `limite ${fmt(prog.cpu_usage, 0)}%` : "—"],
    ["GPU", reg?.gpu || "não usada"],
    ["VRAM", "não medida (WebGL)"],
    ["RAM", reg ? `${fmt(reg.memoria / 1048576, 2)} MiB reservados` : "—"],
    ["THROUGHPUT", reg ? `${fmt(reg.operacoes / Math.max(1, reg.ms_calculo) / 1000, 2)} M ops/s` : prog?.throughput ? `${fmt(prog.throughput / 1e6, 2)} M ops/s` : "—"],
    ["OPERAÇÕES", reg ? `${fmt(reg.operacoes)} + ${fmt(reg.operacoes_verificacao)} na conferência` : prog ? prog.operation : "—"],
    ["VERIFICATION", reg ? `${reg.verificacao} · ${reg.metodo}` : voo ? "PENDING" : "—"],
  ];
  const dl = $("v3-inspecao");
  dl.replaceChildren(
    ...campos.flatMap(([k, v]) => {
      const dt = document.createElement("dt");
      dt.textContent = k;
      const dd = document.createElement("dd");
      dd.textContent = v;
      return [dt, dd];
    }),
  );
  $("v3-inspecao-caixa").hidden = false;
  texto("v3-inspecao-titulo", `Inspecionar cálculo · unidade ${indice}`);
}

// ---------------------------------------------------------------------------
// Reprodução de um JOB gravado
// ---------------------------------------------------------------------------

async function reproduzir(id) {
  escolher(id);
  await lerDetalhe();
  const h = await obter(`/api/ciencia/job/${id}/historico`);
  if (!h || !h.unidades.length) {
    texto("v3-modo", "sem registro para reproduzir");
    return;
  }
  est.historico = h;
  const registros = h.unidades.map((u) => ({
    indice: u.indice,
    despachada: u.despachada || u.inicio,
    inicio: u.inicio,
    fim: u.fim,
    verificada: u.verificada || u.fim,
  }));
  const t0 = Math.min(...registros.map((r) => r.despachada));
  const t1 = Math.max(...registros.map((r) => r.verificada));
  est.reproducao = { t0, t1, relogio: t0, vel: Number($("v3-vel").value) || 1, ultimo: performance.now(), registros };
  texto("v3-modo", "REPRODUÇÃO");
}

function avancarReproducao() {
  const r = est.reproducao;
  const agora = performance.now();
  r.relogio += (agora - r.ultimo) * r.vel;
  r.ultimo = agora;
  if (r.relogio > r.t1) r.relogio = r.t1;
  texto("v3-modo", `REPRODUÇÃO · ${hora(r.relogio)} · ${fmt(((r.relogio - r.t0) / Math.max(1, r.t1 - r.t0)) * 100, 0)}% do tempo gravado`);
}

// ---------------------------------------------------------------------------
// Detalhe do motor (2D): o que a unidade está calculando
// ---------------------------------------------------------------------------

const cacheMoleculas = new Map();
const cacheRotas = new Map();

function coresDoCanvas() {
  return { tinta: "#f4f4f2", fraca: "rgba(244,244,242,.5)", fio: "rgba(244,244,242,.18)", fonte: getComputedStyle(document.body).fontFamily };
}

function curva(canvas, series, rotulos) {
  const ctx = canvas.getContext("2d");
  const esc = devicePixelRatio || 1;
  const w = (canvas.width = canvas.clientWidth * esc);
  const h = (canvas.height = canvas.clientHeight * esc);
  ctx.clearRect(0, 0, w, h);
  const todos = series.flat().filter(Number.isFinite);
  if (!todos.length) return;
  const min = Math.min(...todos);
  const max = Math.max(...todos);
  const m = 28 * esc;
  ctx.font = `${10 * esc}px IBM Plex Mono, monospace`;
  ctx.fillStyle = "rgba(244,244,242,.5)";
  ctx.fillText(fmt(max, 3), 4 * esc, m - 6 * esc);
  ctx.fillText(fmt(min, 3), 4 * esc, h - 6 * esc);
  const cores = ["#f4f4f2", "#e8b04a"];
  series.forEach((s, k) => {
    ctx.strokeStyle = cores[k % 2];
    ctx.lineWidth = 1.5 * esc;
    ctx.beginPath();
    s.forEach((v, i) => {
      const x = m + ((w - 2 * m) * i) / Math.max(1, s.length - 1);
      const y = h - m - ((h - 2 * m) * (v - min)) / Math.max(1e-12, max - min);
      if (i) ctx.lineTo(x, y);
      else ctx.moveTo(x, y);
    });
    ctx.stroke();
    ctx.fillStyle = cores[k % 2];
    ctx.fillText(rotulos[k] || "", m, (14 + k * 13) * esc);
  });
}

async function desenharDetalhe() {
  const d = est.detalhe;
  const canvas = $("v3-detalhe");
  if (!d || !canvas) return;
  texto("v3-job", `${d.descricao_tipo} · ${d.resumo} · ${fmt(d.feitas)}/${fmt(d.unidades)} conferidas · ${d.estado}`);
  const dados = d.dados || {};
  const ativa = [...est.ativas.values()].find((e) => e.job_id === d.id);
  if (d.tipo === "molecular-screening") {
    // a molécula que a linha avalia agora, pelo progresso real (183 operações por molécula)
    let indice = null;
    let origem = "";
    if (ativa) {
      const inicio = d.parametros[1] + ativa.unit_index * d.tamanho;
      indice = inicio + Math.floor((ativa.progress * d.tamanho * OPS_POR_MOLECULA) / OPS_POR_MOLECULA);
      origem = `avaliada agora pela unidade ${ativa.unit_index} (pelo progresso real)`;
    } else if (dados.topo?.length) {
      indice = dados.topo[0].indice;
      origem = "a de maior nota até aqui";
    }
    if (indice == null) return;
    let m = cacheMoleculas.get(indice);
    if (!m) {
      m = await obter(`/api/ciencia/molecula/${indice}`);
      if (m) cacheMoleculas.set(indice, m);
    }
    if (!m) return;
    desenharMolecula(canvas, m.smiles, coresDoCanvas());
    const topo = dados.topo?.find((t) => t.indice === indice);
    texto(
      "v3-detalhe-texto",
      `${m.nome} (${m.id}, ${m.formula}) — ${origem}. log S medido ${fmt(m.logs_medido_mili / 1000, 2)}` +
        (topo ? `; previsto ${fmt(topo.previsto_mili / 1000, 2)} (nota ${topo.nota})` : "") +
        `. Erro do modelo nas moléculas triadas: ${fmt(dados.rmse_log_s, 2)} log S.`,
    );
  } else if (d.tipo === "routing" && dados.rota?.length) {
    const chave = `${d.parametros[0]}/${d.tamanho}`;
    let inst = cacheRotas.get(chave);
    if (!inst) {
      inst = await obter(`/api/ciencia/rotas/${chave}`);
      if (inst) cacheRotas.set(chave, inst);
    }
    if (!inst) return;
    const ctx = canvas.getContext("2d");
    const esc = devicePixelRatio || 1;
    const w = (canvas.width = canvas.clientWidth * esc);
    const h = (canvas.height = canvas.clientHeight * esc);
    const s = Math.min(w, h) * 0.9;
    const ox = (w - s) / 2;
    const oy = (h - s) / 2;
    const p = (c) => [ox + (c[0] / inst.lado) * s, oy + (c[1] / inst.lado) * s];
    ctx.clearRect(0, 0, w, h);
    ctx.strokeStyle = "#e8b04a";
    ctx.lineWidth = 1.2 * esc;
    ctx.beginPath();
    dados.rota.forEach((k, i) => {
      const [x, y] = p(inst.cidades[k]);
      if (i) ctx.lineTo(x, y);
      else ctx.moveTo(x, y);
    });
    ctx.closePath();
    ctx.stroke();
    ctx.fillStyle = "#f4f4f2";
    for (const c of inst.cidades) {
      const [x, y] = p(c);
      ctx.fillRect(x - 1.5 * esc, y - 1.5 * esc, 3 * esc, 3 * esc);
    }
    texto("v3-detalhe-texto", `Melhor rota entre ${dados.unidades} partida(s): comprimento ${fmt(dados.melhor)} (unidade ${dados.indice}); média ${fmt(dados.media, 1)} ± ${fmt(dados.desvio, 1)}. Ótimo local do 2-opt, não o global.`);
  } else if (d.tipo === "population-genetics" && dados.frequencia?.length) {
    curva(canvas, [dados.frequencia, dados.heterozigosidade], ["frequência média de A", "heterozigosidade 2p(1−p)"]);
    texto("v3-detalhe-texto", `${dados.unidades} réplica(s), ${dados.loci} loci: por geração, a frequência média de A e a heterozigosidade esperada, das réplicas conferidas. Fixados ${dados.fixados}, perdidos ${dados.perdidos}.`);
  } else if (d.tipo === "crop-breeding" && dados.media_g?.length) {
    curva(canvas, [dados.media_g, dados.media_p], ["valor genético médio", "fenótipo médio"]);
    texto("v3-detalhe-texto", `${dados.unidades} réplica(s): médias por geração das réplicas conferidas. Fator ambiental ${fmt(dados.fator, 3)} (limitado por ${dados.limitante}). Modelo didático: não prevê safra real.`);
  } else if (d.tipo === "matrix" && ativa) {
    // linhas de C já calculadas: a matriz é feita linha a linha, na ordem
    const n = d.tamanho;
    const feitas = Math.floor(ativa.progress * n);
    const ctx = canvas.getContext("2d");
    const esc = devicePixelRatio || 1;
    const w = (canvas.width = canvas.clientWidth * esc);
    const h = (canvas.height = canvas.clientHeight * esc);
    ctx.clearRect(0, 0, w, h);
    const lado = Math.min(w, h) * 0.9;
    const faixas = Math.min(n, 128);
    for (let k = 0; k < faixas; k++) {
      ctx.fillStyle = k < (feitas * faixas) / n ? "#f4f4f2" : "rgba(244,244,242,.1)";
      ctx.fillRect((w - lado) / 2, (h - lado) / 2 + (k * lado) / faixas, lado, Math.max(1, lado / faixas - 1));
    }
    texto("v3-detalhe-texto", `Unidade ${ativa.unit_index}: ${feitas} de ${n} linhas de C = A·B calculadas (pelo progresso real). Depois, Freivalds confere com outro algoritmo.`);
  } else {
    const ctx = canvas.getContext("2d");
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    texto("v3-detalhe-texto", d.agregado);
  }
}

// ---------------------------------------------------------------------------
// ULTRA BENCHMARK
// ---------------------------------------------------------------------------

async function lerBenchmark() {
  const b = await obter("/api/ciencia/benchmarks");
  if (!b) return;
  texto("b-estado", b.rodando ? "medindo… (leva alguns minutos; a máquina fica ocupada)" : b.ultimo ? "última medida abaixo" : "ainda não medido nesta máquina");
  $("b-rodar").disabled = b.rodando;
  const tabela = $("b-tabela");
  if (!b.ultimo || b.ultimo.erro) {
    if (b.ultimo?.erro) texto("b-estado", `falhou: ${b.ultimo.erro}`);
    return;
  }
  tabela.replaceChildren(
    ...b.ultimo.linhas.map((l) => {
      const tr = document.createElement("tr");
      for (const v of [l.nome, fmt(l.base, 1), fmt(l.atual, 1), fmt(l.alvo, 1), `${fmt(l.ganho, 2)}×`, l.unidade]) {
        const td = document.createElement("td");
        td.textContent = v;
        tr.append(td);
      }
      tr.title = l.nota;
      if (l.primeira) tr.className = "b-primeira";
      return tr;
    }),
  );
  texto("b-nota", `${b.ultimo.linhas.some((l) => l.primeira) ? "Métricas marcadas: primeira medida, que vira a base. " : ""}GPU: ${b.ultimo.gpu}. Nós: ${b.ultimo.nos}.`);
}

async function rodarBenchmark() {
  if (!confirm("O ULTRA BENCHMARK usa a CPU inteira por alguns minutos. Rodar agora?")) return;
  const r = await postar("/api/ciencia/benchmark");
  if (!r.ok) alert(erroDe(r, "Não deu para começar."));
  lerBenchmark();
}

// ---------------------------------------------------------------------------
// Liga tudo
// ---------------------------------------------------------------------------

function ligar() {
  if (!$("painel-ciencia")) return;
  $("nj-dominio").addEventListener("change", trocarDominio);
  $("nj-motor").addEventListener("change", trocarMotor);
  $("nj-estimar").addEventListener("click", estimar);
  $("nj-form").addEventListener("submit", (e) => {
    e.preventDefault();
    submeter();
  });
  $("b-rodar").addEventListener("click", rodarBenchmark);
  $("cj-aceitar").addEventListener("change", async (e) => {
    const r = await postar("/api/ciencia/rede", { aceitar: e.target.checked ? "1" : "0" });
    if (!r.ok) {
      e.target.checked = !e.target.checked;
      alert(erroDe(r, "Não deu para mudar."));
    }
    lerLista();
  });
  $("v3-vel").addEventListener("change", () => est.reproducao && (est.reproducao.vel = Number($("v3-vel").value) || 1));
  $("v3-ao-vivo").addEventListener("click", () => est.selecionado && escolher(est.selecionado));
  $("v3-inspecao-fechar").addEventListener("click", () => ($("v3-inspecao-caixa").hidden = true));
  iniciar3D();
  lerLista();
  lerBenchmark();
  setInterval(lerLista, 2000);
  setInterval(lerEventos, 1000);
  setInterval(lerDetalhe, 1500);
  setInterval(() => !$("b-tabela")?.closest("[hidden]") && lerBenchmark(), 4000);
}

ligar();
