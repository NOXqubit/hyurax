// Hyurax / Ultrax — Planos (docs/MONETIZACAO.md). O catálogo e o plano atual
// vêm do núcleo (/api/v1/planos); o plano pago só se ativa por voucher
// assinado pelo projeto, conferido pelo próprio programa.

import { $, el, texto, fmt, trocar } from "../util.js";
import { obter, postar } from "../api.js";
import { fatos } from "./comum.js";

const TEXTOS = {
  0: { desc: "Tudo do programa, sem pagar nada: a rede é sua.", itens: ["Nó, carteira, mineração e ULTRAX", "Nuvem P2P entre quem você conhece", "Mercado de máquinas: alugue e ofereça", "API externa no seu próprio nó", "Ajuda da comunidade"] },
  1: { desc: "Para quem pesquisa, estuda ou cria e precisa de prazo.", itens: ["150 mil créditos por mês na capacidade gerenciada (≈ 20 a 40 h de CPU)", "50 GB guardados com 3 guardiões", "Comissão do mercado de 15% para 10%", "Prioridade na fila", "Suporte por e-mail em 2 dias úteis", "Nota fiscal"] },
  2: { desc: "Para laboratório, startup ou time pequeno.", itens: ["1 milhão de créditos por mês (≈ 140 a 280 h de CPU)", "500 GB guardados com 3 guardiões", "Comissão do mercado de 8%", "10 contas gerenciadas da API", "Histórico de 1 ano", "Suporte em 1 dia útil"] },
  3: { desc: "Para empresa que precisa de contrato e garantia.", itens: ["Capacidade sob medida, inclusive dedicada", "Rede privada e auditoria", "SLA em contrato", "Comissão negociada", "Canal de suporte dedicado"] },
};

const est = { dados: null, periodo: "mensal", montado: false };

const reais = (centavos) => new Intl.NumberFormat("pt-BR", { style: "currency", currency: "BRL", minimumFractionDigits: centavos % 100 ? 2 : 0 }).format(centavos / 100);

function cartao(p, d) {
  const atual = d.plano_atual === p.id;
  const destaque = p.id === 1;
  const anual = est.periodo === "anual";
  const preco = p.mensal_centavos === 0
    ? [el("span", {}, "Grátis")]
    : anual
      ? [reais(p.anual_centavos), el("small", {}, "/ano")]
      : [p.id === 3 ? el("small", { style: "margin:0 4px 0 0" }, "a partir de") : null, reais(p.mensal_centavos), el("small", {}, "/mês")];
  let acao;
  if (atual) acao = el("button", { type: "button", class: "botao-leve", disabled: true }, "Seu plano");
  else if (p.id === 0) acao = el("button", { type: "button", class: "botao-leve", disabled: true }, "Já incluso no programa");
  else if (!d.venda_aberta) acao = el("button", { type: "button", class: "botao", disabled: true, title: "a venda abre em breve" }, "Em breve");
  else acao = el("a", { class: "botao", href: "https://github.com/NOXqubit/hyurax#planos", target: "_blank", rel: "noopener" }, p.id === 3 ? "Falar com a equipe" : `Assinar o ${p.nome}`);
  return el(
    "article",
    { class: `pl-cartao${destaque ? " destaque" : ""}${atual ? " atual" : ""}`, "aria-label": `Plano ${p.nome}` },
    atual ? el("span", { class: "pilula luz pl-selo" }, "Seu plano") : destaque ? el("span", { class: "pilula luz pl-selo" }, "Mais escolhido") : null,
    el("h3", { class: "pl-nome" }, p.nome),
    el("p", { class: "pl-desc" }, TEXTOS[p.id]?.desc ?? ""),
    el("p", { class: "pl-preco" }, ...preco),
    anual && p.anual_centavos ? el("small", { style: "margin-top:-8px;opacity:.75" }, `${reais(Math.round(p.anual_centavos / 12))} por mês, pago por ano`) : null,
    el("ul", { class: "pl-lista" }, ...(TEXTOS[p.id]?.itens ?? []).map((i) => el("li", {}, i))),
    acao,
  );
}

function tabela(d) {
  const c = d.catalogo;
  const linha = (rotulo, f) => el("tr", {}, el("th", { scope: "row" }, rotulo), ...c.map((p) => el("td", {}, f(p))));
  const marca = (v) => (v ? el("span", { class: "sim", "aria-label": "sim" }, "✓") : el("span", { class: "nao", "aria-label": "não" }, "—"));
  trocar(
    "pl-tabela",
    el("thead", {}, el("tr", {}, el("th", {}, ""), ...c.map((p) => el("th", { scope: "col" }, p.nome)))),
    el(
      "tbody",
      {},
      linha("Preço por mês", (p) => (p.mensal_centavos ? (p.id === 3 ? `a partir de ${reais(p.mensal_centavos)}` : reais(p.mensal_centavos)) : "grátis")),
      linha("Programa completo", () => marca(true)),
      linha("Créditos por mês (capacidade gerenciada)", (p) => (p.creditos_mes ? fmt(p.creditos_mes) : marca(false))),
      linha("Armazenamento gerenciado", (p) => (p.armazenamento_gib ? `${fmt(p.armazenamento_gib)} GB` : marca(false))),
      linha("Comissão da plataforma no mercado", (p) => `${p.comissao_bp / 100}%`),
      linha("Prioridade na fila", (p) => marca(p.id >= 1)),
      linha("Nota fiscal", (p) => marca(p.id >= 1)),
      linha("SLA em contrato", (p) => marca(p.id === 3)),
    ),
  );
}

function render() {
  const d = est.dados;
  if (!d) return;
  trocar("pl-grade", ...d.catalogo.map((p) => cartao(p, d)));
  tabela(d);
  texto("pl-worker", d.worker || "—");
  const nome = d.catalogo.find((p) => p.id === d.plano_atual)?.nome ?? "Comunidade";
  const v = d.voucher;
  fatos("pl-atual", [
    ["Plano", nome],
    ["Válido até", v ? new Date(v.fim_ms).toLocaleDateString("pt-BR") : "sem prazo (grátis)"],
    ["Créditos por mês", v ? fmt(v.creditos_mes) : "—"],
    ["Armazenamento gerenciado", v ? `${fmt(v.armazenamento_gib)} GB` : "—"],
    ["Comissão da plataforma", `${d.comissao_pct}%`],
    ["Voucher", v ? `série ${v.serie}` : "nenhum"],
  ]);
  const na = $("nav-plano");
  if (na) {
    na.hidden = !v;
    na.textContent = v ? nome : "";
  }
  $("pl-aviso").hidden = !!d.venda_aberta;
}

async function carregar() {
  const r = await obter("/planos");
  if (r.ok) {
    est.dados = r.dados;
    render();
  }
}

export function montar() {
  if (est.montado) return;
  est.montado = true;
  for (const b of document.querySelectorAll("#pl-periodo button")) {
    b.addEventListener("click", () => {
      est.periodo = b.dataset.periodo;
      for (const o of document.querySelectorAll("#pl-periodo button")) o.setAttribute("aria-pressed", String(o === b));
      render();
    });
  }
  $("pl-copiar").addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText($("pl-worker").textContent);
      texto("pl-copiar", "Copiado");
      setTimeout(() => texto("pl-copiar", "Copiar"), 1800);
    } catch {
      /* sem área de transferência: o código está na tela para copiar à mão */
    }
  });
  $("pl-form").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const s = $("pl-saida");
    const r = await postar("/planos/ativar", { voucher: $("pl-voucher").value.trim() });
    s.className = r.ok ? "saida" : "saida erro";
    if (r.ok) {
      const nome = est.dados?.catalogo.find((p) => p.id === r.dados.plano)?.nome ?? "pago";
      s.textContent = `Plano ${nome} ativado até ${new Date(r.dados.fim_ms).toLocaleDateString("pt-BR")}.`;
      $("pl-voucher").value = "";
      carregar();
    } else {
      s.textContent = r.erro;
    }
  });
}

export function aoMostrar(e) {
  const pode = !!e?.pode_mandar;
  for (const x of $("pl-form").elements) x.disabled = !pode;
  carregar();
}

export function atualizar() {}

// o selo do plano na navegação aparece já na abertura
setTimeout(carregar, 1500);
