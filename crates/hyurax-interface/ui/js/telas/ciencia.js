// Hyurax / Ultrax — Computação científica: novo JOB, lista, detalhe de um
// JOB (com o resultado agregado das unidades conferidas), trabalho entre
// nós, eventos e o ULTRA BENCHMARK.
//
// Cada número vem de /api/v1/ciencia ou dos eventos da ciência. O que roda
// de cada unidade aparece na cena 3D da Visão geral (amostras reais).

import { $, el, texto, fmt, compacto, curto, trocar } from "../util.js";
import { estado, ouvir } from "../estado.js";
import { obter, postar, url } from "../api.js";
import { desenharMolecula, desenharCurva } from "../moleculas.js";
import { fatos, tabela, barra, linhaDeRegistro, resultado } from "./comum.js";
import { montarHistorico } from "./historico.js";

// Os campos de cada motor: rótulo, mínimo, máximo, padrão, como vira o u32
// da especificação, passo. Faixas iguais às do motor (hyurax-ultrax).
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

const est = { lista: null, escolhido: null, detalhe: null, moleculas: new Map(), rotas: new Map(), lendo: false };

// ---------------------------------------------------------------------------
// Formulário
// ---------------------------------------------------------------------------

function montarFormulario() {
  const dominios = est.lista?.dominios || [];
  const sel = $("nj-dominio");
  if (sel.options.length === dominios.length) return;
  sel.replaceChildren(
    ...dominios.map((d) =>
      el("option", { value: String(d.codigo), disabled: d.motores.length === 0 }, d.motores.length ? `${d.nome} · ${d.motores.map((m) => m.descricao).join(", ")}` : `${d.nome} (sem motor: não calcula nada)`),
    ),
  );
  sel.value = "1";
  trocarDominio();
}

function trocarDominio() {
  const d = (est.lista?.dominios || []).find((x) => String(x.codigo) === $("nj-dominio").value);
  $("nj-motor").replaceChildren(...(d?.motores || []).map((m) => el("option", { value: String(m.codigo) }, m.descricao)));
  trocarMotor();
}

function campo(id, [rotulo, min, max, padrao, , passo]) {
  return el(
    "label",
    { for: id },
    el("small", {}, `${rotulo} (${fmt(min, passo ? 2 : 0)} a ${fmt(max, passo ? 2 : 0)})`),
    el("input", { id, type: "number", min: String(min), max: String(max), step: String(passo || 1), value: String(padrao), autocomplete: "off" }),
  );
}

function trocarMotor() {
  const cfg = M[$("nj-motor").value];
  const caixa = $("nj-campos");
  caixa.replaceChildren();
  if (!cfg) return;
  caixa.append(campo("nj-tamanho", cfg.tamanho));
  if (cfg.passos) caixa.append(campo("nj-passos", cfg.passos));
  (cfg.parametros || []).forEach((p, k) => caixa.append(campo(`nj-p${k}`, p)));
  $("nj-unidades").value = cfg.unidades;
  texto("nj-saida", "");
}

function pedido() {
  const cfg = M[$("nj-motor").value];
  if (!cfg) return null;
  const num = (id) => Number($(id)?.value);
  return {
    dominio: $("nj-dominio").value,
    tipo: $("nj-motor").value,
    tamanho: Math.round(num("nj-tamanho")),
    passos: cfg.passos ? Math.round(num("nj-passos")) : 0,
    parametros: (cfg.parametros || []).map((p, k) => p[4](num(`nj-p${k}`))).join(","),
    unidades: Math.round(num("nj-unidades")),
    nivel: $("nj-nivel").value,
    redundancia: Math.round(num("nj-redundancia")),
    prazo_s: Math.round(num("nj-prazo")),
    orcamento_milicreditos: Math.round(num("nj-orcamento") * 1000),
    descricao: $("nj-descricao").value,
  };
}

async function estimar() {
  const p = pedido();
  if (!p) return;
  const r = await postar("/ciencia/estimar", p);
  if (!r.ok) return resultado("nj-saida", r);
  const e = r.dados;
  resultado(
    "nj-saida",
    r,
    `Estimativa (ESTIMADO): ${compacto(e.operacoes)} operações de cálculo e ${compacto(e.operacoes_verificacao)} de conferência; ` +
      `${fmt(e.milicreditos / 1000, 3)} créditos (não é HYX); ${fmt(e.memoria_por_unidade / 1048576, 1)} MiB por unidade; ` +
      `cerca de ${fmt(e.segundos_estimados, 1)} s com ${e.linhas} linha(s) a ${fmt(e.ritmo / 1e6, 1)} M ops/s ` +
      (e.ritmo_medido ? "(ritmo medido nesta máquina)." : "(ritmo inicial: este motor ainda não foi medido nesta abertura, então o tempo é um chute).") +
      " É o teto do modelo de custo.",
  );
}

async function submeter(ev) {
  ev.preventDefault();
  const p = pedido();
  if (!p) return;
  const r = await postar("/ciencia/submeter", p);
  resultado("nj-saida", r, r.ok ? `JOB ${curto(r.dados.id, 16)} na fila. Ele roda quando o ULTRAX estiver ligado; as unidades aparecem na cena 3D da Visão geral.` : "");
  if (r.ok) {
    est.escolhido = r.dados.id;
    lerLista();
  }
}

// ---------------------------------------------------------------------------
// Lista, rede e detalhe
// ---------------------------------------------------------------------------

async function lerLista() {
  const r = await obter("/ciencia");
  if (!r.ok) return;
  est.lista = r.dados;
  montarFormulario();
  if (!est.escolhido && r.dados.jobs.length) est.escolhido = r.dados.jobs[0].id;
  lista();
  rede();
  lerDetalhe();
}

async function mudar(id, acao) {
  const r = await postar(`/ciencia/job/${id}/${acao}`);
  resultado("nj-saida", r, `JOB ${curto(id, 16)}: ${acao} feito.`);
  lerLista();
}

function botao(rotulo, fn) {
  return el("button", { type: "button", class: "botao-leve", onclick: (ev) => { ev.stopPropagation(); fn(); } }, rotulo);
}

function lista() {
  const jobs = est.lista?.jobs || [];
  tabela(
    "cj-lista",
    [{ t: "JOB" }, { t: "Trabalho" }, { t: "Estado" }, { t: "Progresso" }, { t: "Unidades", num: true }, { t: "Falhas", num: true }, { t: "Verificação" }, { t: "Créditos", num: true }, { t: "" }],
    jobs.map((j) => {
      const acoes = [];
      if (j.estado === "RUNNING" || j.estado === "WAITING FOR NODES") acoes.push(botao("Pausar", () => mudar(j.id, "pausar")));
      if (j.estado === "PAUSED" || j.estado === "OUT OF BUDGET") acoes.push(botao("Retomar", () => mudar(j.id, "retomar")));
      if (!["COMPLETED", "CANCELLED", "EXPIRED"].includes(j.estado)) acoes.push(botao("Cancelar", () => confirm("Cancelar este JOB?") && mudar(j.id, "cancelar")));
      return [
        el("span", { class: "num", title: j.id }, curto(j.id, 10)),
        el("span", { title: j.descricao || "" }, `${j.descricao_tipo} · ${j.resumo}`),
        j.estado + (j.motivo ? ` · ${j.motivo}` : ""),
        [barra(j.progresso), el("small", {}, `${fmt(j.progresso * 100, 1)}%`)],
        { v: `${fmt(j.feitas)}/${fmt(j.unidades)}`, num: true },
        { v: fmt(j.falhas), num: true },
        j.nivel_nome,
        // consumido, e de quanto quando há orçamento (JOB de conta da API externa sempre tem)
        { v: `${fmt(j.consumo.milicreditos / 1000, 3)}${j.orcamento_milicreditos ? ` de ${fmt(j.orcamento_milicreditos / 1000, 3)}` : ""}`, num: true },
        el("div", { class: "acoes" }, acoes),
      ];
    }),
    {
      vazio: "nenhum JOB ainda: monte um no formulário acima",
      aoClicar: (i) => { est.escolhido = jobs[i].id; est.detalhe = null; lista(); lerDetalhe(); },
      escolhido: (i) => jobs[i].id === est.escolhido,
    },
  );
}

function rede() {
  const r = est.lista?.rede;
  if (!r) return;
  const caixa = $("cj-aceitar");
  if (document.activeElement !== caixa) caixa.checked = !!r.aceitar;
  caixa.disabled = !estado.atual?.pode_mandar;
  fatos("cj-rede", [
    ["Workers de outros nós oferecendo", fmt(r.ofertas?.length || 0)],
    ["Unidades de outros nós aqui", `${fmt(r.remotas_executando)} rodando · ${fmt(r.remotas_na_fila)} na fila`],
    ["Nossas unidades em outros nós", fmt(r.redundantes)],
    ["Mensagens", `${fmt(r.mensagens_recebidas)} recebidas · ${fmt(r.mensagens_enviadas)} enviadas`],
    // Gold Score e "verificado" são a visão DESTE nó (docs/REPUTACAO.md)
    ...[...(r.reputacao || [])].sort((a, b) => (b.gold ?? 0) - (a.gold ?? 0)).slice(0, 6).map((x) => [
      el("span", { class: "num", title: x.worker }, curto(x.worker, 10)),
      el(
        "span",
        { title: x.verificado ? "nó verificado: 90 dias, 100 unidades, até 1% de recusas, ativo em 30 dias" : `falta: ${(x.falta || []).join("; ")}` },
        `${x.verificado ? "verificado · " : ""}Gold ${x.gold ?? "—"} · nota ${x.nota} · ${x.verificadas} aceitas · ${x.divergentes} divergentes · ${x.recusadas} recusadas`,
      ),
    ]),
  ]);
}

async function lerDetalhe() {
  if (!est.escolhido || est.lendo || $("tela-ciencia").hidden) return;
  est.lendo = true;
  const r = await obter(`/ciencia/job/${est.escolhido}`);
  est.lendo = false;
  if (!r.ok) return;
  est.detalhe = r.dados;
  detalhe();
}

function coresDoCanvas() {
  const s = getComputedStyle(document.documentElement);
  return { tinta: s.getPropertyValue("--tinta").trim(), fio: s.getPropertyValue("--fio-forte").trim(), fraca: s.getPropertyValue("--tinta-3").trim(), fonte: s.getPropertyValue("--mono").trim() };
}

/** O mapa das unidades: feitas, com falha, em voo e por fazer. */
function mapaDeUnidades(d) {
  const total = d.unidades;
  const caixas = Math.min(total, 400);
  const estadoDa = new Array(caixas).fill(0);
  const marca = (a, b, v) => {
    for (let i = a; i < b; i++) estadoDa[Math.floor((i * caixas) / total)] = Math.max(estadoDa[Math.floor((i * caixas) / total)], v);
  };
  for (const [a, b] of d.faixas_feitas || []) marca(a, b, 2);
  for (const [a, b] of d.faixas_falhas || []) marca(a, b, 3);
  for (const v of d.em_voo_lista || []) marca(v.indice, v.indice + 1, 1);
  const c = el("canvas", { class: "grafico", "aria-label": "Mapa das unidades do JOB" });
  requestAnimationFrame(() => {
    const esc = devicePixelRatio || 1;
    const w = (c.width = c.clientWidth * esc);
    const h = (c.height = c.clientHeight * esc);
    const ctx = c.getContext("2d");
    const cor = coresDoCanvas();
    const col = Math.ceil(Math.sqrt((caixas * w) / h));
    const lin = Math.ceil(caixas / col);
    const lado = Math.min(w / col, h / lin);
    estadoDa.forEach((s, i) => {
      const x = (i % col) * lado;
      const y = Math.floor(i / col) * lado;
      ctx.fillStyle = s === 2 ? cor.tinta : s === 1 ? getComputedStyle(document.documentElement).getPropertyValue("--gpu") : cor.fio;
      if (s === 3) {
        ctx.strokeStyle = cor.tinta;
        ctx.strokeRect(x + 1.5, y + 1.5, lado - 3, lado - 3);
      } else ctx.fillRect(x + 1, y + 1, lado - 2, lado - 2);
    });
  });
  return c;
}

async function detalhe() {
  const d = est.detalhe;
  if (!d) return;
  $("cj-detalhe-bloco").hidden = false;
  $("cj-historico-bloco").hidden = false;
  if (est.historicoDe && est.historicoDe !== d.id) {
    // outro JOB escolhido: o histórico aberto era do anterior
    est.historicoDe = null;
    $("cj-historico").replaceChildren();
  }
  texto("cj-detalhe-titulo", `JOB ${curto(d.id, 16)} · ${d.descricao_tipo}`);
  const fatosDl = el("dl", { class: "fatos", id: "cj-detalhe-fatos" });
  const dados = d.dados || {};
  const c = d.consumo;
  const figura = el("canvas", { class: "grafico", style: "height:220px" });
  const legenda = el("p", { class: "nota" });
  const links = el(
    "div",
    { class: "acoes" },
    ...["json", "csv", "pdf"].map((f) => el("a", { class: "botao-leve", href: url(`/ciencia/job/${d.id}/relatorio.${f}`), target: "_blank", rel: "noopener" }, `Relatório ${f.toUpperCase()}`)),
    el("a", { class: "botao-leve", href: "#visao" }, "Ver as unidades na cena 3D"),
  );
  trocar(
    "cj-detalhe",
    el("div", { class: "grade-dupla" }, el("div", {}, fatosDl), el("div", {}, mapaDeUnidades(d), el("p", { class: "nota" }, "Unidades: cheias = conferidas, azuis = calculando agora, contornadas = falharam, apagadas = por fazer."))),
    el("p", {}, d.agregado),
    figura,
    legenda,
    links,
  );
  fatos("cj-detalhe-fatos", [
    ["Trabalho", d.resumo],
    ["Área", d.dominio],
    ["Descrição", d.descricao || "—"],
    ["Estado", d.estado + (d.motivo ? ` · ${d.motivo}` : "")],
    ["Unidades", `${fmt(d.feitas)} conferidas de ${fmt(d.unidades)} · ${fmt(d.falhas)} falha(s) · ${fmt(d.em_voo)} em voo`],
    ["Verificação", `${d.nivel_nome} · redundância ${d.redundancia}`],
    ["Recusas / repetidas", `${fmt(d.recusas)} / ${fmt(d.repetidas)}`],
    ["Operações", `${compacto(c.operacoes)} de cálculo · ${compacto(c.operacoes_verificacao)} de conferência`],
    ["Tempo", `${fmt(c.cpu_ms / 1000, 1)} s de CPU · ${fmt(c.gpu_ms / 1000, 1)} s de GPU`],
    ["Créditos", `${fmt(c.milicreditos / 1000, 3)} (não é HYX)`],
    ["Nós que calcularam", fmt(d.nos)],
    ["Resumo do resultado", el("span", { class: "num", title: d.resumo_hash }, curto(d.resumo_hash, 16))],
  ]);
  const cores = coresDoCanvas();
  await new Promise((r) => requestAnimationFrame(r));
  if (d.tipo === "molecular-screening" && dados.topo?.length) {
    const t = dados.topo[0];
    let m = est.moleculas.get(t.indice);
    if (!m) {
      const r = await obter(`/ciencia/molecula/${t.indice}`);
      if (r.ok) est.moleculas.set(t.indice, (m = r.dados));
    }
    if (m) {
      desenharMolecula(figura, m.smiles, cores);
      legenda.textContent = `A de maior nota até aqui: ${m.nome || m.id} (${m.formula}) · log S medido ${fmt(m.logs_medido_mili / 1000, 2)}, previsto ${fmt(t.previsto_mili / 1000, 2)} (nota ${t.nota}). Estrutura plana desenhada do SMILES; coordenadas 3D reais: PENDENTE. Erro do modelo nas triadas: ${fmt(dados.rmse_log_s, 2)} log S.`;
    }
  } else if (d.tipo === "routing" && dados.rota?.length) {
    const chave = `${d.parametros[0]}/${d.tamanho}`;
    let inst = est.rotas.get(chave);
    if (!inst) {
      const r = await obter(`/ciencia/rotas/${chave}`);
      if (r.ok) est.rotas.set(chave, (inst = r.dados));
    }
    if (inst) {
      const esc = devicePixelRatio || 1;
      const w = (figura.width = figura.clientWidth * esc);
      const h = (figura.height = figura.clientHeight * esc);
      const s = Math.min(w, h) * 0.92;
      const p = (q) => [(w - s) / 2 + (q[0] / inst.lado) * s, (h - s) / 2 + (q[1] / inst.lado) * s];
      const ctx = figura.getContext("2d");
      ctx.strokeStyle = cores.tinta;
      ctx.lineWidth = 1.2 * esc;
      ctx.beginPath();
      dados.rota.forEach((k, i) => { const [x, y] = p(inst.cidades[k]); if (i) ctx.lineTo(x, y); else ctx.moveTo(x, y); });
      ctx.closePath();
      ctx.stroke();
      ctx.fillStyle = cores.tinta;
      for (const q of inst.cidades) { const [x, y] = p(q); ctx.fillRect(x - 1.5 * esc, y - 1.5 * esc, 3 * esc, 3 * esc); }
      legenda.textContent = `Melhor rota entre ${dados.unidades} partida(s): comprimento ${fmt(dados.melhor)} (unidade ${dados.indice}); média ${fmt(dados.media, 1)} ± ${fmt(dados.desvio, 1)}. É um ótimo local do 2-opt, não o ótimo global.`;
    }
  } else if (d.tipo === "population-genetics" && dados.frequencia?.length) {
    desenharCurva(figura, dados.frequencia, cores);
    legenda.textContent = `Frequência média do alelo A por geração, nas ${dados.unidades} réplica(s) conferidas (${dados.loci} loci). Fixados ${dados.fixados}, perdidos ${dados.perdidos}.`;
  } else if (d.tipo === "crop-breeding" && dados.media_g?.length) {
    desenharCurva(figura, dados.media_g, cores);
    legenda.textContent = `Valor genético médio por geração, nas ${dados.unidades} réplica(s) conferidas. Fator ambiental ${fmt(dados.fator, 3)} (limitado por ${dados.limitante}). Modelo didático: não prevê safra real.`;
  } else {
    figura.hidden = true;
  }
}

// ---------------------------------------------------------------------------
// Eventos e benchmark
// ---------------------------------------------------------------------------

function eventos() {
  trocar(
    "cj-eventos",
    estado.ciencia.slice(0, 120).map((ev) =>
      linhaDeRegistro(
        ev.timestamp,
        ev.event,
        [ev.work_unit_id || (ev.job_id ? curto(ev.job_id, 10) : ""), ev.workload_type, ev.verification_status, ev.throughput != null ? `${fmt(ev.throughput / 1e6, 2)} M ops/s` : "", ev.result_hash ? `resultado ${curto(ev.result_hash, 10)}` : "", ev.operation]
          .filter(Boolean)
          .join(" · "),
      ),
    ),
  );
}

async function lerBenchmark() {
  const r = await obter("/ciencia/benchmarks");
  if (!r.ok) return;
  const b = r.dados;
  $("b-rodar").disabled = b.rodando || !estado.atual?.pode_mandar;
  texto("b-estado", b.rodando ? "medindo… (alguns minutos; a máquina fica ocupada)" : b.ultimo?.erro ? `falhou: ${b.ultimo.erro}` : b.ultimo ? `última medida · GPU: ${b.ultimo.gpu} · nós: ${b.ultimo.nos}` : "ainda não medido nesta máquina");
  tabela(
    "b-tabela",
    [{ t: "Métrica" }, { t: "Base", num: true }, { t: "Atual", num: true }, { t: "Ganho", num: true }, { t: "Unidade" }],
    (b.ultimo?.linhas || []).map((l) => [
      el("span", { title: l.nota }, l.nome + (l.primeira ? " (primeira medida = base)" : "")),
      { v: fmt(l.base, 1), num: true },
      { v: fmt(l.atual, 1), num: true },
      { v: `${fmt(l.ganho, 2)}×`, num: true },
      l.unidade,
    ]),
    { vazio: "sem medida" },
  );
}

async function rodarBenchmark() {
  if (!confirm("O ULTRA BENCHMARK usa a CPU inteira por alguns minutos. Medir agora?")) return;
  const r = await postar("/ciencia/benchmark");
  resultado("b-estado", r, "medindo…");
  lerBenchmark();
}

// ---------------------------------------------------------------------------

export function montar() {
  $("nj-dominio").addEventListener("change", trocarDominio);
  $("nj-motor").addEventListener("change", trocarMotor);
  $("nj-estimar").addEventListener("click", estimar);
  $("nj-form").addEventListener("submit", submeter);
  $("b-rodar").addEventListener("click", rodarBenchmark);
  $("cj-historico-abrir").addEventListener("click", () => {
    const d = est.detalhe;
    if (!d) return;
    est.historicoDe = d.id;
    montarHistorico($("cj-historico"), d.id, d.unidades);
  });
  $("cj-aceitar").addEventListener("change", async (ev) => {
    const r = await postar("/ciencia/rede", { aceitar: ev.target.checked ? "1" : "0" });
    if (!r.ok) ev.target.checked = !ev.target.checked;
    lerLista();
  });
  let pendente = false;
  ouvir("ciencia", () => {
    if ($("tela-ciencia").hidden || pendente) return;
    pendente = true;
    setTimeout(() => { pendente = false; eventos(); }, 400);
  });
}

let ultimaLeitura = 0;
let leituras = 0;
export function atualizar() {
  // a lista é lida a cada 2 s enquanto a tela está aberta; o benchmark, a cada 6 s
  if (Date.now() - ultimaLeitura < 2000) return;
  ultimaLeitura = Date.now();
  lerLista();
  if (++leituras % 3 === 0) lerBenchmark();
}

export function aoMostrar() {
  ultimaLeitura = Date.now();
  lerLista();
  lerBenchmark();
  eventos();
}
