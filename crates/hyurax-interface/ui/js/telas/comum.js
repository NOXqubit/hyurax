// Hyurax / Ultrax — peças que as telas repetem: lista de fatos, tabela,
// registro, gráfico de linha e controle deslizante.

import { $, el, hora } from "../util.js";

/**
 * Preenche um <dl class="fatos"> com pares [rótulo, valor]. O valor pode ser
 * texto, número ou uma lista de elementos (ex.: valorComOrigem).
 */
export function fatos(id, pares) {
  const dl = $(id);
  if (!dl) return;
  const filhos = [];
  for (const par of pares) {
    if (!par) continue;
    const [rotulo, valor, dica] = par;
    filhos.push(el("dt", { title: dica }, rotulo));
    filhos.push(el("dd", {}, ...(Array.isArray(valor) ? valor : [valor ?? "—"])));
  }
  dl.replaceChildren(...filhos);
}

/**
 * Preenche uma <table>: cabeçalho [{t, num}] e linhas (cada uma, lista de
 * células: texto, elemento ou {v, num, title}). `vazio` aparece sem linhas.
 */
export function tabela(id, cabecalho, linhas, { vazio = "nada ainda", aoClicar, escolhido } = {}) {
  const t = $(id);
  if (!t) return;
  const thead = el("thead", {}, el("tr", {}, cabecalho.map((c) => el("th", { class: c.num ? "num" : null }, c.t ?? c))));
  const corpo = linhas.length
    ? linhas.map((l, i) => {
        const tr = el("tr", { class: [aoClicar ? "clicavel" : "", escolhido?.(i) ? "escolhido" : ""].join(" ").trim() || null });
        if (aoClicar) tr.addEventListener("click", () => aoClicar(i));
        for (const c of l) {
          if (c && typeof c === "object" && !(c instanceof Node) && !Array.isArray(c)) {
            tr.append(el("td", { class: c.num ? "num" : null, title: c.title }, ...(Array.isArray(c.v) ? c.v : [c.v ?? "—"])));
          } else {
            tr.append(el("td", {}, ...(Array.isArray(c) ? c : [c ?? "—"])));
          }
        }
        return tr;
      })
    : [el("tr", {}, el("td", { colspan: String(cabecalho.length), class: "nota" }, vazio))];
  t.replaceChildren(thead, el("tbody", {}, corpo));
}

/** Uma linha de registro: hora, categoria, texto. */
export function linhaDeRegistro(ms, categoria, texto) {
  return el("li", {}, el("span", {}, hora(ms)), el("span", { title: categoria }, categoria), el("span", {}, texto));
}

/** Uma barra de progresso de 0 a 1. */
export function barra(fracao) {
  const f = Math.max(0, Math.min(1, Number(fracao) || 0));
  return el("div", { class: "barra", role: "progressbar", "aria-valuenow": String(Math.round(f * 100)), "aria-valuemin": "0", "aria-valuemax": "100" }, el("span", { style: `width:${(f * 100).toFixed(1)}%` }));
}

/**
 * Gráfico de linhas num canvas. `series`: [{pontos: [[x, y]], cor, tracejado}].
 * Eixo y de 0 a `max` (ou ao maior valor). Sem pontos: só o quadro.
 */
export function grafico(id, series, { max, rotulo } = {}) {
  const c = $(id);
  if (!c || !c.clientWidth) return;
  const esc = devicePixelRatio || 1;
  const w = (c.width = Math.round(c.clientWidth * esc));
  const h = (c.height = Math.round(c.clientHeight * esc));
  const ctx = c.getContext("2d");
  ctx.clearRect(0, 0, w, h);
  const estilo = getComputedStyle(document.documentElement);
  const fio = estilo.getPropertyValue("--fio-forte").trim() || "#444";
  const tinta3 = estilo.getPropertyValue("--tinta-3").trim() || "#888";
  let x0 = Infinity, x1 = -Infinity, y1 = max ?? 0;
  for (const s of series) for (const [x, y] of s.pontos) {
    if (x < x0) x0 = x;
    if (x > x1) x1 = x;
    if (max === undefined && y > y1) y1 = y;
  }
  y1 = y1 || 1;
  const m = 4 * esc;
  ctx.strokeStyle = fio;
  ctx.lineWidth = esc;
  ctx.beginPath();
  ctx.moveTo(m, h - m);
  ctx.lineTo(w - m, h - m);
  ctx.stroke();
  ctx.fillStyle = tinta3;
  ctx.font = `${10 * esc}px ${estilo.getPropertyValue("--mono")}`;
  if (rotulo) ctx.fillText(rotulo(y1), m + 2 * esc, m + 10 * esc);
  if (!(x1 > x0)) return;
  for (const s of series) {
    if (s.pontos.length < 2) continue;
    ctx.strokeStyle = s.cor;
    ctx.lineWidth = 1.5 * esc;
    ctx.setLineDash(s.tracejado ? [4 * esc, 3 * esc] : []);
    ctx.beginPath();
    s.pontos.forEach(([x, y], i) => {
      const px = m + ((x - x0) / (x1 - x0)) * (w - 2 * m);
      const py = h - m - (Math.max(0, Math.min(y, y1)) / y1) * (h - 2 * m - 12 * esc);
      if (i) ctx.lineTo(px, py);
      else ctx.moveTo(px, py);
    });
    ctx.stroke();
  }
  ctx.setLineDash([]);
}

/** As cores do gráfico a partir do tema. */
export function cores() {
  const e = getComputedStyle(document.documentElement);
  return { tinta: e.getPropertyValue("--tinta").trim(), tinta3: e.getPropertyValue("--tinta-3").trim(), gpu: e.getPropertyValue("--gpu").trim() };
}

/**
 * Um controle deslizante com rótulo e valor: devolve o <div class="controle">.
 * `aoSoltar(valor)` roda quando o dono solta (não a cada pixel).
 */
export function deslizante({ id, rotulo, min, max, passo = 1, valor, formato = (v) => v, aoSoltar }) {
  const saida = el("output", { for: id, class: "num" }, formato(valor));
  const entrada = el("input", { id, type: "range", min: String(min), max: String(max), step: String(passo), value: String(valor) });
  entrada.addEventListener("input", () => { saida.textContent = formato(Number(entrada.value)); });
  entrada.addEventListener("change", () => aoSoltar(Number(entrada.value)));
  return { caixa: el("div", { class: "controle" }, el("label", { for: id }, rotulo), entrada, saida), entrada, saida };
}

/** Atualiza um deslizante sem atrapalhar quem está mexendo nele. */
export function acertar(controle, valor, formato = (v) => v) {
  if (document.activeElement === controle.entrada) return;
  if (Number(controle.entrada.value) !== Number(valor)) controle.entrada.value = String(valor);
  controle.saida.textContent = formato(Number(valor));
}

/** Uma caixa de liga/desliga. */
export function chave({ id, rotulo, aoMudar }) {
  const entrada = el("input", { type: "checkbox", id });
  entrada.addEventListener("change", () => aoMudar(entrada.checked, entrada));
  return { caixa: el("label", { class: "chave", for: id }, entrada, el("span", {}, rotulo)), entrada };
}

/** Marca uma caixa sem atrapalhar quem está clicando. */
export function marcar(caixa, valor) {
  if (document.activeElement !== caixa.entrada) caixa.entrada.checked = !!valor;
}

/** Mostra o resultado de um comando num <p class="saida">. */
export function resultado(id, r, textoOk) {
  const s = $(id);
  if (!s) return;
  s.className = r.ok ? "saida" : "saida erro";
  s.textContent = r.ok ? textoOk : r.erro;
}

/** Milicréditos como créditos de computação (não são dinheiro nem HYX). */
export function creditos(mili, casas = 3) {
  if (mili === null || mili === undefined) return "—";
  const v = Number(mili) / 1000;
  return `${new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 0, maximumFractionDigits: casas }).format(v)} cr`;
}

/**
 * Cartões de número de destaque: [{rotulo, valor, unidade, nota, origem}].
 * O valor é texto; `origem` (REAL, DERIVADO…) vira selo.
 */
export function metricas(id, lista, seloDe) {
  const caixa = $(id);
  if (!caixa) return;
  const n = lista.filter(Boolean).length;
  caixa.style.setProperty("--colunas", String(n <= 5 ? n : Math.ceil(n / 2)));
  caixa.replaceChildren(
    ...lista.filter(Boolean).map((m) =>
      el(
        "div",
        { class: "metrica" },
        el("span", { class: "metrica-rotulo" }, m.rotulo),
        el("span", { class: "metrica-valor" }, m.valor ?? "—", m.unidade ? el("small", {}, m.unidade) : null),
        m.nota || m.origem ? el("span", { class: "metrica-nota" }, m.origem && seloDe ? seloDe(m.origem, m.fonte) : null, m.origem && m.nota ? " " : null, m.nota || null) : null,
      ),
    ),
  );
}

/** Um estado com forma e texto: tipo = ok, em-curso, atencao, falha ou parado. */
export function estadoEl(tipo, textoDoEstado, dica) {
  return el("span", { class: `estado estado-${tipo}`, title: dica }, textoDoEstado);
}

/** Troca um <span class="estado"> existente. */
export function marcarEstado(id, tipo, textoDoEstado) {
  const s = $(id);
  if (!s) return;
  s.className = `estado estado-${tipo}`;
  s.textContent = textoDoEstado;
}

/** Bytes legíveis (KiB, MiB, GiB). */
export function bytes(n) {
  const v = Number(n);
  if (!Number.isFinite(v)) return "—";
  const u = ["B", "KiB", "MiB", "GiB", "TiB"];
  let i = 0;
  let x = v;
  while (x >= 1024 && i < u.length - 1) {
    x /= 1024;
    i += 1;
  }
  return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: i ? 1 : 0 }).format(x)} ${u[i]}`;
}

/** Uma caixa de "nada ainda", com o que fazer. */
export function vazio(titulo, explicacao) {
  return el("div", { class: "vazio" }, el("strong", {}, titulo), explicacao ? el("span", {}, explicacao) : null);
}
