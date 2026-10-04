// Hyurax / Ultrax — Mercado (Cloud Design 2.0): computadores de outras
// pessoas como numa loja de aplicativos. Um destaque (o de melhor reputação
// medida por ESTE nó, conectado agora), filtros, cartões e o aluguel. A
// oferta deste computador fica num painel que abre.
//
// Tudo sai de /api/v1/nuvem. Preço em créditos de computação (não são
// dinheiro nem HYX). O benchmark do anúncio é ESTIMADO pelo dono.

import { $, el, texto, fmt, curto, trocar } from "../util.js";
import { postar } from "../api.js";
import { estado } from "../estado.js";
import { nuvem, aoMudar, mostrar, esconder, ler } from "../nuvem.js";
import { creditos, marcarEstado, tabela, resultado, vazio, estadoEl } from "./comum.js";

const FILTROS = [
  ["todos", "Todos"],
  ["online", "Online agora"],
  ["gpu", "Com placa de vídeo"],
  ["disco", "Guardam arquivos"],
  ["venda", "À venda"],
];
const NOMES = { capacidade: "Capacidade", maquina_inteira: "Computador inteiro", venda: "À venda" };
let filtro = "todos";
let alugando = null;
let formularioPreenchido = false;

function svgMaquina() {
  const s = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  s.setAttribute("viewBox", "0 0 24 24");
  s.setAttribute("fill", "none");
  s.setAttribute("stroke", "currentColor");
  s.setAttribute("stroke-width", "1.7");
  s.setAttribute("stroke-linecap", "round");
  s.setAttribute("stroke-linejoin", "round");
  s.setAttribute("aria-hidden", "true");
  s.innerHTML = '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>';
  return s;
}

function nomeDa(a) {
  if (a.descricao) return a.descricao.length > 48 ? `${a.descricao.slice(0, 46)}…` : a.descricao;
  return `${NOMES[a.tipo] || a.tipo} · ${a.linhas} núcleo(s)`;
}

function preco(a) {
  if (a.tipo === "venda") return a.preco_venda_centavos ? `R$ ${fmt(a.preco_venda_centavos / 100, 2)}` : "a combinar";
  return fmt(a.preco_credito_mili / 1000, 2);
}

function reputacao(a) {
  if (a.reputacao == null) return "Ainda sem histórico com você";
  return `Reputação ${fmt(a.reputacao)} de 1000 · ${a.classe === "verificado" ? "verificado" : "em observação"}`;
}

function cartao(a, { minha = false } = {}) {
  const pode = estado.atual?.pode_mandar;
  const pode_alugar = !minha && a.tipo !== "venda" && a.preco_credito_mili > 0 && a.conectado && pode;
  const botao = minha
    ? null
    : el(
        "button",
        {
          type: "button",
          class: pode_alugar ? "botao-escuro" : "botao-leve",
          disabled: !pode_alugar,
          title: pode_alugar ? "" : a.tipo === "venda" ? "a venda acontece fora da rede" : !a.conectado ? "o computador não está conectado agora" : "comando só pela janela deste computador",
          onclick: () => abrirAluguel(a),
        },
        a.tipo === "venda" ? "Só anúncio" : a.conectado ? "Alugar" : "Fora da rede",
      );
  return el(
    "article",
    { class: "cartao" },
    el(
      "div",
      { class: "cartao-cab" },
      el("span", { class: `cartao-icone${a.gpu ? " gpu" : ""}` }, svgMaquina()),
      el(
        "span",
        { class: "cartao-titulo" },
        el("b", {}, minha ? "Este computador" : nomeDa(a)),
        el("small", {}, el("span", { class: `ponto${a.conectado || minha ? "" : " fora"}` }), minha ? (nuvem.dados?.ajustes?.anunciar ? "No mercado" : "Fora do mercado") : a.conectado ? "Online agora" : "Fora da rede agora"),
      ),
    ),
    el(
      "div",
      { class: "specs" },
      el("span", {}, el("b", {}, fmt(a.linhas)), el("small", {}, "núcleos")),
      el("span", {}, el("b", {}, fmt(a.ram_mib / 1024, 0)), el("small", {}, "GB de RAM")),
      el("span", {}, el("b", {}, a.disco_mib ? fmt(a.disco_mib / 1024, a.disco_mib < 10240 ? 1 : 0) : "—"), el("small", {}, "GB p/ arquivos")),
    ),
    el("small", {}, minha ? (a.gpu ? `GPU: ${a.gpu}` : "Sem placa de vídeo declarada") : `${reputacao(a)}${a.gpu ? ` · ${a.gpu}` : ""}`),
    el(
      "div",
      { class: "cartao-rodape" },
      el("strong", {}, preco(a), " ", a.tipo === "venda" ? null : el("small", {}, "cr / crédito")),
      botao,
    ),
  );
}

function abrirAluguel(a) {
  alugando = a;
  $("mk-aluguel-bloco").hidden = false;
  texto("mk-aluguel-titulo", `Alugar: ${nomeDa(a)}`);
  texto(
    "mk-aluguel-nota",
    `As partes deste cálculo vão só para esse computador, a ${fmt(a.preco_credito_mili / 1000, 2)} cr por crédito conferido. Cada parte volta e é refeita aqui antes de contar: resposta errada não é paga e pesa na reputação dele.`,
  );
  $("mk-aluguel-bloco").scrollIntoView({ behavior: "smooth", block: "start" });
}

function destaque(lista) {
  const melhor = lista.filter((a) => a.conectado && a.tipo !== "venda" && a.preco_credito_mili > 0).sort((x, y) => (y.reputacao ?? -1) - (x.reputacao ?? -1) || x.preco_credito_mili - y.preco_credito_mili)[0];
  const caixa = $("mk-destaque");
  if (!melhor) {
    caixa.hidden = true;
    return;
  }
  caixa.hidden = false;
  const b = el("button", { type: "button", class: "botao", disabled: !estado.atual?.pode_mandar, onclick: () => abrirAluguel(melhor) }, "Alugar");
  caixa.replaceChildren(
    el("span", { class: "icone-grande" }, svgMaquina()),
    el(
      "div",
      { class: "texto" },
      el("span", { class: "rotulo" }, melhor.reputacao != null ? "Melhor reputação com você" : "Online agora"),
      el("h2", {}, nomeDa(melhor)),
      el("p", {}, `${fmt(melhor.linhas)} núcleo(s), ${fmt(melhor.ram_mib / 1024, 0)} GB de RAM${melhor.gpu ? `, ${melhor.gpu}` : ""}. ${reputacao(melhor)}.`),
    ),
    el("div", { class: "preco-grande" }, el("strong", {}, preco(melhor), " ", el("small", {}, "cr / crédito")), b),
  );
}

function desenhar() {
  const d = nuvem.dados;
  if (!d) {
    if (nuvem.erro) marcarEstado("mk-estado", "falha", nuvem.erro);
    return;
  }
  const lista = d.mercado || [];
  trocar(
    "mk-filtros",
    FILTROS.map(([id, nome]) => {
      const b = el("button", { type: "button", class: "chip", "aria-pressed": String(id === filtro) }, nome);
      b.addEventListener("click", () => {
        filtro = id;
        desenhar();
      });
      return b;
    }),
  );
  destaque(lista);
  const visiveis = lista.filter(
    (a) => filtro === "todos" || (filtro === "online" && a.conectado) || (filtro === "gpu" && a.gpu) || (filtro === "disco" && a.disco_mib > 0) || (filtro === "venda" && a.tipo === "venda"),
  );
  trocar(
    "mk-lista",
    visiveis.length
      ? visiveis.map((a) => cartao(a))
      : vazio(
          lista.length ? "Nenhum computador neste filtro" : "Nenhum computador anunciado ainda",
          lista.length ? "Escolha outro filtro acima." : "Os anúncios chegam pelos computadores conectados a este. Sem conexões (veja Rede), o mercado fica vazio: não existe servidor central de onde buscar.",
        ),
  );
  if (!formularioPreenchido) {
    const aj = d.ajustes;
    $("mk-anunciar").checked = aj.anunciar;
    $("mk-tipo").value = aj.tipo;
    $("mk-preco").value = String(aj.preco_credito_mili / 1000);
    $("mk-gb").value = String(aj.preco_gb_mes_mili / 1000);
    $("mk-disco").value = String(aj.disco_mib);
    $("mk-venda").value = String(aj.preco_venda_centavos / 100);
    $("mk-descricao").value = aj.descricao;
    formularioPreenchido = true;
    alternarVenda();
  }
  const m = d.maquina;
  trocar(
    "mk-previa",
    cartao(
      {
        tipo: d.ajustes.tipo,
        linhas: m.linhas,
        ram_mib: m.ram_mib,
        gpu: m.gpu,
        disco_mib: d.ajustes.disco_mib,
        preco_credito_mili: d.ajustes.preco_credito_mili,
        preco_venda_centavos: d.ajustes.preco_venda_centavos,
        conectado: true,
      },
      { minha: true },
    ),
  );
  tabela(
    "mk-alugueis",
    [{ t: "Cálculo" }, { t: "Computador" }, { t: "Preço", num: true }, { t: "Conferido", num: true }, { t: "Valor", num: true }, { t: "Recibo assinado", num: true }, { t: "Agora" }],
    (d.alugueis || []).map((a) => [
      { v: curto(a.job, 10), title: a.job },
      { v: curto(a.fornecedor, 10), title: a.fornecedor },
      { v: creditos(a.preco_credito_mili), num: true },
      { v: creditos(a.creditos_mili), num: true },
      { v: creditos(a.total_mili), num: true },
      { v: creditos(a.recibo_mili), num: true },
      a.conectado ? estadoEl("ok", "online") : estadoEl("parado", "fora"),
    ]),
    { vazio: "nenhum aluguel ainda" },
  );
}

function alternarVenda() {
  $("mk-venda-rotulo").hidden = $("mk-tipo").value !== "venda";
}

export function montar() {
  $("mk-tipo").addEventListener("change", alternarVenda);
  $("mk-oferecer").addEventListener("click", () => {
    $("mk-oferta").open = true;
  });
  $("mk-form").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const mili = (id) => Math.round(Number($(id).value || 0) * 1000);
    const r = await postar("/nuvem/ajustes", {
      anunciar: $("mk-anunciar").checked ? 1 : 0,
      tipo: $("mk-tipo").value,
      preco_credito_mili: mili("mk-preco"),
      preco_gb_mes_mili: mili("mk-gb"),
      disco_mib: Math.max(0, Math.round(Number($("mk-disco").value || 0))),
      preco_venda_centavos: Math.round(Number($("mk-venda").value || 0) * 100),
      descricao: $("mk-descricao").value.trim(),
    });
    resultado("mk-saida", r, "Salvo. O anúncio sai assinado para os computadores conectados em instantes.");
    if (r.ok) ler();
  });
  $("mk-al-cancelar").addEventListener("click", () => {
    alugando = null;
    $("mk-aluguel-bloco").hidden = true;
  });
  $("mk-aluguel").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    if (!alugando) return;
    const r = await postar("/nuvem/alugar", {
      worker: alugando.worker,
      dominio: 7, // matemática
      tipo: 1, // multiplicação de matrizes, conferida por Freivalds
      tamanho: $("mk-al-lado").value,
      unidades: $("mk-al-unidades").value,
      orcamento_milicreditos: Math.round(Number($("mk-al-orcamento").value || 0) * 1000),
      descricao: $("mk-al-descricao").value.trim() || "aluguel de capacidade",
    });
    resultado("mk-al-saida", r, r.ok ? "Enviado só para esse computador. Acompanhe em Computar." : "");
    if (r.ok) ler();
  });
  aoMudar(() => {
    if (!$("tela-mercado").hidden) desenhar();
  });
}

export function aoMostrar() {
  mostrar();
  desenhar();
}

export function aoEsconder() {
  esconder();
}
