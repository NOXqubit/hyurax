// Hyurax / Ultrax — utilidades da tela: seleção, criação de elementos,
// números em pt-BR e o selo de origem de cada número.

export const $ = (id) => document.getElementById(id);

/** Cria um elemento: el("div", { class: "x", text: "oi" }, filho1, filho2). */
export function el(tag, atributos = {}, ...filhos) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(atributos)) {
    if (v === undefined || v === null || v === false) continue;
    if (k === "class") e.className = v;
    else if (k === "text") e.textContent = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (k === "dataset") Object.assign(e.dataset, v);
    else e.setAttribute(k, v === true ? "" : v);
  }
  for (const f of filhos.flat()) {
    if (f === null || f === undefined || f === false) continue;
    e.append(f instanceof Node ? f : document.createTextNode(String(f)));
  }
  return e;
}

/** Troca o texto de um elemento pelo id, se ele existir. */
export function texto(id, t) {
  const e = $(id);
  if (e && e.textContent !== String(t)) e.textContent = t;
}

const NUM = new Map();
/** Número em pt-BR com `casas` decimais. */
export function fmt(v, casas = 0) {
  if (v === null || v === undefined || Number.isNaN(Number(v))) return "—";
  let f = NUM.get(casas);
  if (!f) {
    f = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: casas, maximumFractionDigits: casas });
    NUM.set(casas, f);
  }
  return f.format(Number(v));
}

/** Número grande com prefixo (mil, mi, bi). */
export function compacto(v) {
  const n = Number(v);
  if (!Number.isFinite(n)) return "—";
  const a = Math.abs(n);
  if (a >= 1e12) return `${fmt(n / 1e12, 2)} tri`;
  if (a >= 1e9) return `${fmt(n / 1e9, 2)} bi`;
  if (a >= 1e6) return `${fmt(n / 1e6, 2)} mi`;
  if (a >= 1e4) return `${fmt(n / 1e3, 1)} mil`;
  return fmt(n, a < 10 && a % 1 ? 2 : 0);
}

/** Hora local de um instante em milissegundos. */
export function hora(ms) {
  if (!ms) return "—";
  return new Date(ms).toLocaleTimeString("pt-BR", { hour12: false });
}

/** Duração em segundos, legível. */
export function duracao(s) {
  if (!Number.isFinite(s)) return "—";
  if (s < 60) return `${fmt(s, s < 10 ? 1 : 0)} s`;
  if (s < 3600) return `${Math.floor(s / 60)} min ${Math.floor(s % 60)} s`;
  return `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
}

/** Os primeiros caracteres de um hash. */
export function curto(h, n = 12) {
  return h ? `${String(h).slice(0, n)}…` : "—";
}

// Origem de um número. A tela nunca mostra número sem dizer de onde veio.
const ORIGENS = {
  REAL: "medido agora",
  DERIVADO: "conta exata sobre um valor medido",
  ESTIMADO: "depende de um parâmetro não medido",
  SIMULADO: "cálculo de verdade sobre entrada gerada nesta máquina",
  AJUSTE: "escolha do dono, não medida",
  PENDENTE: "ainda não medido nesta máquina",
  MOCK: "só visual",
};

/** O selo da origem: REAL, DERIVADO, ESTIMADO, SIMULADO, AJUSTE ou PENDENTE. */
export function selo(origem, fonte) {
  const o = String(origem || "PENDENTE").toUpperCase();
  return el("span", { class: `selo selo-${o.toLowerCase()}`, title: fonte ? `${ORIGENS[o] || ""} · ${fonte}` : ORIGENS[o] || "" }, o);
}

/**
 * Um valor com origem ({valor, unidade, origem, fonte}) como elementos:
 * o número, a unidade e o selo. Sem valor: "—" e o motivo.
 */
export function valorComOrigem(v, casas = 1) {
  if (!v) return [el("span", { class: "num" }, "—")];
  const numero = v.valor === null || v.valor === undefined ? "—" : `${fmt(v.valor, casas)}${v.unidade ? ` ${v.unidade}` : ""}`;
  return [el("span", { class: "num" }, numero), " ", selo(v.origem, v.fonte)];
}

/** Troca o conteúdo de um elemento pelo id. */
export function trocar(id, ...filhos) {
  const e = $(id);
  if (e) e.replaceChildren(...filhos.flat().filter((f) => f !== null && f !== undefined && f !== false));
}

/** Escapa texto para atributo (o resto usa textContent). */
export function seguro(t) {
  return String(t ?? "").replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}
