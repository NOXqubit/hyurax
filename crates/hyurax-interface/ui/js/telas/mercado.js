// Hyurax / Ultrax — Mercado de máquinas: o anúncio desta máquina, as
// máquinas anunciadas por outros nós e os aluguéis.
//
// Tudo sai de /api/v1/nuvem. Preço em créditos de computação (não são
// dinheiro nem HYX). A reputação mostrada é a que ESTE nó mediu conferindo
// trabalho da máquina; o benchmark do anúncio é ESTIMADO pelo dono.

import { $, el, texto, fmt, curto, selo, trocar } from "../util.js";
import { postar } from "../api.js";
import { estado } from "../estado.js";
import { nuvem, aoMudar, mostrar, esconder, ler } from "../nuvem.js";
import { creditos, metricas, marcarEstado, estadoEl, tabela, resultado, vazio, bytes } from "./comum.js";

const NOMES = { capacidade: "Capacidade", maquina_inteira: "Máquina inteira", venda: "Venda" };
let alugando = null; // o anúncio escolhido para alugar
let formularioPreenchido = false;

function cartao(a, { minha = false } = {}) {
  const d = nuvem.dados;
  const pode = estado.atual?.pode_mandar;
  const titulo = `${NOMES[a.tipo] || a.tipo} · ${a.linhas} linha(s) · ${fmt(a.ram_mib / 1024, 1)} GiB`;
  const conexao = minha ? estadoEl(d?.ajustes?.anunciar ? "ok" : "parado", d?.ajustes?.anunciar ? "anunciando" : "não anunciada") : a.conectado ? estadoEl("ok", "conectada") : estadoEl("parado", "fora da rede agora");
  const p = a.por_1000_creditos;
  const preco =
    a.tipo === "venda"
      ? el("dl", { class: "preco" }, el("dt", {}, "Preço pedido"), el("dd", { class: "total" }, a.preco_venda_centavos ? `R$ ${fmt(a.preco_venda_centavos / 100, 2)}` : "a combinar"), el("dt", {}, "Negociação"), el("dd", {}, "fora da rede"))
      : el(
          "dl",
          { class: "preco", title: "por 1.000 créditos de computação verificados" },
          el("dt", {}, "1.000 créditos verificados"),
          el("dd", { class: "total" }, creditos(p?.total_mili)),
          el("dt", {}, `ao proprietário (${d?.ajustes?.provedor_pct ?? "—"}%)`),
          el("dd", {}, creditos(p?.proprietario_mili)),
          el("dt", {}, `comissão HYURAX (${d?.ajustes?.plataforma_pct ?? "—"}%)`),
          el("dd", {}, creditos(p?.plataforma_mili)),
          el("dt", {}, "reserva"),
          el("dd", {}, creditos(p?.reserva_mili)),
        );
  const rep = a.reputacao == null ? "sem histórico neste nó" : `${fmt(a.reputacao)} de 1000 · ${a.classe}`;
  const fatos = el(
    "dl",
    { class: "fatos" },
    el("dt", {}, "GPU"),
    el("dd", {}, a.gpu ? `${a.gpu}${a.vram_mib ? ` · ${fmt(a.vram_mib)} MiB` : ""}` : "nenhuma declarada"),
    el("dt", {}, "Disco oferecido"),
    el("dd", { class: "num" }, a.disco_mib ? bytes(a.disco_mib * 1024 * 1024) : "nenhum"),
    el("dt", {}, "Armazenamento"),
    el("dd", { class: "num" }, a.disco_mib ? `${creditos(a.preco_gb_mes_mili)} / GB·mês` : "—"),
    el("dt", {}, "Benchmark"),
    el("dd", {}, a.creditos_hora_estimado ? [`${creditos(a.creditos_hora_estimado)} / h `, selo("ESTIMADO", "medido pelo próprio dono")] : "não informado"),
    minha ? null : el("dt", {}, "Reputação"),
    minha ? null : el("dd", {}, rep),
    minha ? null : el("dt", {}, "Identidade"),
    minha ? null : el("dd", { class: "num", title: a.worker }, curto(a.worker, 14)),
  );
  const pode_alugar = !minha && a.tipo !== "venda" && a.preco_credito_mili > 0 && a.conectado && pode;
  return el(
    "article",
    { class: "cartao" },
    el("div", { class: "cartao-cab" }, el("span", { class: "cartao-titulo" }, titulo), conexao),
    a.descricao ? el("p", { class: "nota", style: "margin:0" }, a.descricao) : null,
    fatos,
    preco,
    minha
      ? null
      : el(
          "div",
          { class: "acoes" },
          el(
            "button",
            {
              type: "button",
              class: "botao-leve",
              disabled: !pode_alugar,
              title: pode_alugar ? "" : a.tipo === "venda" ? "anúncio de venda" : !a.conectado ? "a máquina não está conectada" : "comando só pela janela deste computador",
              onclick: () => abrirAluguel(a),
            },
            a.tipo === "venda" ? "Só listagem" : "Alugar capacidade",
          ),
        ),
  );
}

function abrirAluguel(a) {
  alugando = a;
  $("mk-aluguel-bloco").hidden = false;
  texto("mk-aluguel-titulo", `Alugar capacidade de ${curto(a.worker, 12)}`);
  texto(
    "mk-aluguel-nota",
    `As unidades deste JOB vão só para essa máquina, a ${creditos(a.preco_credito_mili)} por crédito verificado. Cada unidade é refeita aqui antes de contar (concordância 2 de 2): resultado errado não é pago e pesa na reputação dela.`,
  );
  $("mk-aluguel-bloco").scrollIntoView({ behavior: "smooth", block: "start" });
}

function desenhar() {
  const d = nuvem.dados;
  if (!d) {
    if (nuvem.erro) marcarEstado("mk-estado", "falha", nuvem.erro);
    return;
  }
  const lista = d.mercado || [];
  const conectadas = lista.filter((a) => a.conectado).length;
  marcarEstado("mk-estado", d.ajustes.anunciar ? "ok" : "parado", d.ajustes.anunciar ? "esta máquina está anunciada" : "esta máquina não está anunciada");
  metricas("mk-metricas", [
    { rotulo: "Máquinas anunciadas", valor: fmt(lista.length), nota: "vistas por este nó, assinadas" },
    { rotulo: "Conectadas agora", valor: fmt(conectadas), nota: "pares com anúncio válido" },
    { rotulo: "Com disco oferecido", valor: fmt(lista.filter((a) => a.disco_mib > 0 && a.conectado).length), nota: "podem guardar fragmentos" },
    { rotulo: "Seus aluguéis", valor: fmt((d.alugueis || []).length), nota: "JOBs restritos a um fornecedor" },
  ]);
  // o formulário só é preenchido uma vez (não atropela quem está digitando)
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
        vram_mib: m.vram_mib,
        disco_mib: d.ajustes.disco_mib,
        preco_credito_mili: d.ajustes.preco_credito_mili,
        preco_gb_mes_mili: d.ajustes.preco_gb_mes_mili,
        preco_venda_centavos: d.ajustes.preco_venda_centavos,
        descricao: d.ajustes.descricao,
        creditos_hora_estimado: m.creditos_hora_estimado,
        por_1000_creditos: partilha(d.ajustes.preco_credito_mili * 1000, d.ajustes),
      },
      { minha: true },
    ),
  );
  const filtro = $("mk-filtro").value;
  const filtrada = lista.filter((a) => (!filtro ? true : filtro === "conectadas" ? a.conectado : a.tipo === filtro));
  trocar(
    "mk-lista",
    filtrada.length
      ? filtrada.map((a) => cartao(a))
      : vazio(
          lista.length ? "Nenhuma máquina neste filtro" : "Nenhuma máquina anunciada ainda",
          lista.length ? "Troque o filtro acima." : "Os anúncios chegam pelos pares conectados. Sem pares (veja Rede), o mercado fica vazio: não há servidor central de onde buscar.",
        ),
  );
  tabela(
    "mk-alugueis",
    [{ t: "JOB" }, { t: "Fornecedor" }, { t: "Preço / crédito", num: true }, { t: "Créditos verificados", num: true }, { t: "Valor", num: true }, { t: "Recibo assinado", num: true }, { t: "Fornecedor agora" }],
    (d.alugueis || []).map((a) => [
      { v: curto(a.job, 12), title: a.job },
      { v: curto(a.fornecedor, 12), title: a.fornecedor },
      { v: creditos(a.preco_credito_mili), num: true },
      { v: creditos(a.creditos_mili), num: true },
      { v: creditos(a.total_mili), num: true },
      { v: creditos(a.recibo_mili), num: true, title: "créditos já reconhecidos em recibo assinado por você" },
      a.conectado ? estadoEl("ok", "conectado") : estadoEl("parado", "fora"),
    ]),
    { vazio: "nenhum aluguel ainda: escolha uma máquina acima" },
  );
}

/** A mesma partilha que o núcleo faz (para a prévia do anúncio desta máquina). */
function partilha(total, aj) {
  const p = Math.floor((total * aj.provedor_pct) / 100);
  const pl = Math.floor((total * aj.plataforma_pct) / 100);
  return { total_mili: total, proprietario_mili: p, plataforma_mili: pl, reserva_mili: total - p - pl };
}

function alternarVenda() {
  const venda = $("mk-tipo").value === "venda";
  $("mk-venda-rotulo").hidden = !venda;
}

export function montar() {
  $("mk-tipo").addEventListener("change", alternarVenda);
  $("mk-filtro").addEventListener("change", desenhar);
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
    resultado("mk-saida", r, "Salvo. O anúncio novo sai assinado na próxima volta (2 s) para os pares conectados.");
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
    resultado("mk-al-saida", r, r.ok ? `JOB ${curto(r.dados.job, 12)} submetido só para essa máquina. Acompanhe em JOBs científicos.` : "");
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
