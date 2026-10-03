// Hyurax / Ultrax — tela ULTRAX: controle do worker, recursos (limite
// escolhido ao lado do uso medido, nunca um no lugar do outro), tarefas
// executando, placar local, passos e histórico.

import { $, el, texto, fmt, compacto, curto, hora, selo, valorComOrigem, trocar } from "../util.js";
import { estado, ouvir } from "../estado.js";
import { postar } from "../api.js";
import { fatos, tabela, barra, linhaDeRegistro, deslizante, acertar, chave, marcar } from "./comum.js";

const c = {};
let montado = false;
let ultimoGpu = null;

async function mandar(campos) {
  const r = await postar("/ultrax", campos);
  const s = $("u-controles-nota");
  s.className = r.ok ? "nota" : "nota saida erro";
  s.textContent = r.ok ? "" : r.erro;
}

function montarControles(e) {
  const u = e.ultrax;
  const ramTotal = e.metricas?.sistema?.ram_total_mib || 4096;
  c.ligar = chave({ id: "u-ligar", rotulo: "ULTRAX ligado", aoMudar: (v) => mandar({ ligar: v ? "1" : "0" }) });
  c.linhas = deslizante({ id: "u-linhas", rotulo: "Linhas de CPU", min: 1, max: u.nucleos, valor: u.linhas, formato: (v) => `${v} de ${u.nucleos}`, aoSoltar: (v) => mandar({ linhas: v }) });
  c.limite = deslizante({ id: "u-limite", rotulo: "Limite por linha", min: 10, max: 100, passo: 5, valor: u.uso_cpu, formato: (v) => `${v}%`, aoSoltar: (v) => mandar({ limite_cpu: v }) });
  c.memoria = deslizante({ id: "u-memoria", rotulo: "Teto de memória", min: 32, max: Math.max(64, Math.min(16384, Math.floor(ramTotal / 2))), passo: 32, valor: u.memoria_mib, formato: (v) => `${fmt(v)} MiB`, aoSoltar: (v) => mandar({ memoria_mib: v }) });
  c.gpu = chave({ id: "u-gpu", rotulo: "Usar a GPU (WebGL 2 nesta janela; a CPU confere cada resultado)", aoMudar: (v) => mandar({ gpu: v ? "1" : "0" }) });
  c.gpuLimite = deslizante({ id: "u-gpu-limite", rotulo: "Fatia de tempo da GPU", min: 10, max: 100, passo: 5, valor: u.gpu?.uso ?? 50, formato: (v) => `${v}%`, aoSoltar: (v) => mandar({ gpu_limite: v }) });
  c.debug = chave({ id: "u-debug", rotulo: "Modo DEBUG (telemetria de cada passo no registro)", aoMudar: (v) => mandar({ debug: v ? "1" : "0" }) });
  trocar("u-controles", c.ligar.caixa, c.linhas.caixa, c.limite.caixa, c.memoria.caixa, c.gpu.caixa, c.gpuLimite.caixa, c.debug.caixa);
  montado = true;
}

function controles(e) {
  const u = e.ultrax;
  if (!montado) montarControles(e);
  marcar(c.ligar, u.ligado);
  acertar(c.linhas, u.linhas, (v) => `${v} de ${u.nucleos}`);
  acertar(c.limite, u.uso_cpu, (v) => `${v}%`);
  acertar(c.memoria, u.memoria_mib, (v) => `${fmt(v)} MiB`);
  marcar(c.gpu, u.gpu?.ligada);
  acertar(c.gpuLimite, u.gpu?.uso ?? 50, (v) => `${v}%`);
  marcar(c.debug, u.debug);
  const pode = !!e.pode_mandar;
  for (const k of Object.values(c)) k.entrada.disabled = !pode;
  if (!pode) texto("u-controles-nota", "Só leitura: comando vale só no computador onde o programa roda.");
}

function recursos(e) {
  const u = e.ultrax;
  const m = e.metricas || {};
  fatos("u-recursos", [
    ["Linhas de CPU", `${u.linhas} de ${u.nucleos} núcleos lógicos`],
    ["Limite por linha", [el("span", { class: "num" }, `${u.uso_cpu}%`), " ", selo("AJUSTE", "fatia de tempo que cada linha pode usar; não é o uso")]],
    ["CPU deste programa", valorComOrigem(m.cpu_processo, 1)],
    ["CPU da máquina", valorComOrigem(m.cpu_total, 0)],
    ["Memória reservada", [el("span", { class: "num" }, `${fmt(u.reservada_mib, 1)} MiB`), " ", selo("REAL", "somada pelo próprio worker para as tarefas em curso")]],
    ["Teto de memória", [el("span", { class: "num" }, `${fmt(u.memoria_mib)} MiB`), " ", selo("AJUSTE")]],
    ["RAM deste programa", valorComOrigem(m.ram_processo, 0)],
    ["GPU", u.gpu?.ligada ? `${u.gpu.nome || "esperando a janela"} · fatia ${u.gpu.uso}% (AJUSTE)` : "desligada"],
    ["Ritmo da GPU", u.gpu?.ligada && u.gpu.ritmo ? [el("span", { class: "num" }, `${compacto(u.gpu.ritmo)} ops/s`), " ", selo("REAL", "medido na última conta da GPU")] : "—"],
    ["GPU 3D da máquina", valorComOrigem(m.gpu_3d, 0)],
    ["Worker", el("span", { class: "num", title: u.worker }, curto(u.worker, 16))],
  ]);
  const notas = [];
  if (u.gpu?.ligada && ultimoGpu?.erro) notas.push(`GPU: ${ultimoGpu.erro}`);
  if (u.gpu?.ligada && !e.pode_mandar) notas.push("A GPU só calcula pela janela do próprio computador.");
  texto("u-gpu-nota", notas.join(" ") || "O uso de GPU no WebGL 2 não é medido pelo navegador: o número da GPU vem do contador do sistema, para todos os programas juntos.");
}

function ativas(e) {
  const u = e.ultrax;
  tabela(
    "u-ativas",
    [{ t: "#" }, { t: "Recurso" }, { t: "Trabalho" }, { t: "Origem" }, { t: "Método" }, { t: "Estado" }, { t: "Progresso" }, { t: "Operações", num: true }, { t: "Memória", num: true }, { t: "Há" }],
    (u.ativas || []).map((a) => [
      { v: String(a.numero), num: true },
      el("span", { class: a.dispositivo === "GPU" ? "recurso-gpu" : null }, a.dispositivo === "GPU" ? "GPU" : `CPU ${a.linha}`),
      el("span", { title: a.descricao }, a.resumo),
      a.job ? [selo("REAL", `JOB ${a.job}`), ` JOB ${curto(a.job, 8)} #${a.indice}`] : [selo("SIMULADO", "carga LAB")," LAB", a.desafio ? " · desafio" : ""],
      a.metodo,
      a.estado,
      [barra(a.feitas / Math.max(1, a.total)), el("small", {}, `${fmt((a.feitas / Math.max(1, a.total)) * 100, 0)}%`)],
      { v: compacto(a.operacoes || a.feitas), num: true },
      { v: `${fmt(a.memoria_mib, 1)} MiB`, num: true },
      a.inicio ? `${Math.max(0, Math.round((e.agora_ms - a.inicio) / 1000))} s` : "—",
    ]),
    { vazio: u.ligado ? "nenhuma tarefa agora (a fila está sendo preparada)" : "o ULTRAX está desligado" },
  );
}

function placar(e) {
  const p = e.ultrax.placar || {};
  fatos("u-placar", [
    ["Liquidadas", fmt(p.liquidadas)],
    ["Recusadas", fmt(p.recusadas)],
    ["Canceladas / expiradas", `${fmt(p.canceladas)} / ${fmt(p.expiradas)}`],
    ["Abandonadas", fmt(p.abandonadas)],
    ["Desafios", `${fmt(p.desafios_certos)} certos · ${fmt(p.desafios_errados)} errados`],
    ["Operações verificadas", compacto(p.operacoes_verificadas)],
    ["Operações sem crédito", compacto(p.operacoes_sem_credito)],
    ["Work Score", [el("span", { class: "num" }, p.work_score ?? "—"), " ", selo("DERIVADO", "soma das operações verificadas; não é dinheiro nem HYX")]],
    ["Tempo de cálculo", `${fmt((p.ms_calculo || 0) / 1000, 1)} s`],
    ["Tempo de verificação", `${fmt((p.ms_verificacao || 0) / 1000, 1)} s`],
    ["Por tipo", `matriz ${fmt(p.matriz)} · mochila ${fmt(p.mochila)} · difusão ${fmt(p.difusao)} · IA ${fmt(p.ia)}`],
    ["Reputação", p.taxa !== null && p.taxa !== undefined ? `nota ${p.nota} · acerto ${fmt(p.taxa * 100, 1)}%` : `nota ${p.nota ?? "—"}`],
  ]);
}

function passos() {
  trocar("u-passos", estado.tarefas.slice(0, 80).map((t) => linhaDeRegistro(t.quando, `#${t.tarefa}`, `${t.evento}${t.detalhe ? ` · ${t.detalhe}` : ""}`)));
}

function historico(e) {
  const h = (e.ultrax.historico || []).slice().reverse().slice(0, 60);
  tabela(
    "u-historico",
    [{ t: "#" }, { t: "Quando" }, { t: "Trabalho" }, { t: "Método" }, { t: "Estado" }, { t: "Operações", num: true }, { t: "Cálculo", num: true }, { t: "Resultado" }, { t: "Nota" }],
    h.map((x) => [
      { v: String(x.numero), num: true },
      hora(x.fim),
      el("span", { title: x.tipo }, x.resumo),
      x.metodo,
      x.estado,
      { v: compacto(x.operacoes), num: true },
      { v: `${fmt(x.ms_calculo)} ms`, num: true },
      el("span", { class: "num", title: x.resultado }, curto(x.resultado, 12)),
      x.nota || (x.desafio ? "desafio" : ""),
    ]),
    { vazio: "nenhuma tarefa encerrada ainda" },
  );
}

export function montar() {
  ouvir("tarefa", () => { if (!$("tela-ultrax").hidden) passos(); });
  ouvir("gpu", (d) => { ultimoGpu = d; });
}

export function atualizar(e) {
  if (!e.ultrax) return;
  controles(e);
  recursos(e);
  ativas(e);
  placar(e);
  historico(e);
}

export function aoMostrar(e) {
  passos();
  if (e) atualizar(e);
}
