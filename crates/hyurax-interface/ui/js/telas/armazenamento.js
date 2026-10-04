// Hyurax / Ultrax — Armazenamento distribuído: guardar um arquivo cifrado em
// vários nós, ver a saúde de cada fragmento, recuperar e apagar; e o que este
// nó guarda para outros.

import { $, el, texto, fmt, curto, trocar } from "../util.js";
import { postar, postarBytes, url } from "../api.js";
import { estado } from "../estado.js";
import { nuvem, aoMudar, mostrar, esconder, ler } from "../nuvem.js";
import { metricas, marcarEstado, estadoEl, tabela, resultado, bytes } from "./comum.js";

const SAUDE = {
  integro: ["ok", "íntegro"],
  degradado: ["atencao", "degradado: reparando"],
  em_risco: ["falha", "em risco: menos de k fragmentos"],
  enviando: ["em-curso", "enviando"],
};

function explicar() {
  const k = Math.max(1, Number($("ar-k").value) || 1);
  const m = Math.max(0, Number($("ar-m").value) || 0);
  texto(
    "ar-explica",
    `${k} + ${m} = ${k + m} nós guardiões diferentes. Aguenta perder ${m} deles sem perder o arquivo; ocupa ${fmt((k + m) / k, 2)}× o tamanho na rede. Precisa de ${k + m} nós conectados que ofereçam disco.`,
  );
}

function fragmentos(a) {
  return el(
    "div",
    { class: "fragmentos", role: "img", "aria-label": `${a.guardados} de ${a.k + a.m} fragmentos guardados` },
    a.fragmentos.map((f) =>
      el(
        "span",
        {
          class: `fragmento ${f.estado}${f.paridade ? " paridade" : ""}`,
          title: `fragmento ${f.indice} (${f.paridade ? "paridade" : "dados"}) · ${f.estado} · guardião ${curto(f.guardiao, 10)}${f.conectado ? " (conectado)" : " (fora da rede)"} · ${f.desafios_restantes} desafio(s) restante(s)`,
        },
        String(f.indice),
      ),
    ),
  );
}

function desenhar() {
  const d = nuvem.dados;
  if (!d) {
    if (nuvem.erro) marcarEstado("ar-estado", "falha", nuvem.erro);
    return;
  }
  const arquivos = d.arquivos || [];
  const guardioes = (d.mercado || []).filter((a) => a.disco_mib > 0 && a.conectado && a.tipo !== "venda").length;
  const problemas = arquivos.filter((a) => a.saude === "em_risco").length;
  const reparando = arquivos.filter((a) => a.saude === "degradado").length;
  marcarEstado("ar-estado", problemas ? "falha" : reparando ? "atencao" : "ok", problemas ? `${problemas} arquivo(s) em risco` : reparando ? `${reparando} em reparo` : arquivos.length ? "tudo íntegro" : "nenhum arquivo ainda");
  metricas("ar-metricas", [
    { rotulo: "Arquivos na rede", valor: fmt(arquivos.length), nota: "cifrados aqui; a chave não sai" },
    { rotulo: "Íntegros", valor: fmt(arquivos.filter((a) => a.saude === "integro").length), nota: "todos os fragmentos conferidos" },
    { rotulo: "Guardiões disponíveis", valor: fmt(guardioes), nota: "nós conectados que oferecem disco" },
    { rotulo: "Guardando para outros", valor: bytes(d.guarda.usado_bytes), nota: `${fmt(d.guarda.fragmentos)} fragmento(s) · cota ${d.guarda.cota_mib ? bytes(d.guarda.cota_mib * 1024 * 1024) : "nenhuma"}` },
  ]);
  const pode = estado.atual?.pode_mandar;
  tabela(
    "ar-arquivos",
    [{ t: "Arquivo" }, { t: "Tamanho", num: true }, { t: "k + m" }, { t: "Fragmentos" }, { t: "Saúde" }, { t: "Última notícia" }, { t: "" }],
    arquivos.map((a) => {
      const [tipo, rotulo] = SAUDE[a.saude] || ["parado", a.saude];
      const acoes = el(
        "div",
        { class: "acoes" },
        a.recuperado
          ? el("a", { class: "botao-leve", href: url(`/nuvem/arquivo/${a.arquivo}`), download: a.nome }, "Baixar")
          : el("button", { type: "button", class: "botao-leve", disabled: !pode || a.recuperando, onclick: () => comando("/nuvem/recuperar", a, "recuperação pedida aos guardiões") }, a.recuperando ? "Recuperando…" : "Recuperar"),
        el("button", { type: "button", class: "botao-leve botao-perigo", disabled: !pode, onclick: () => apagar(a) }, "Apagar"),
      );
      return [
        { v: a.nome, title: `id ${a.arquivo}` },
        { v: bytes(a.tamanho), num: true },
        `${a.k} + ${a.m}`,
        fragmentos(a),
        estadoEl(tipo, rotulo),
        { v: a.noticia || "—" },
        acoes,
      ];
    }),
    { vazio: "nenhum arquivo guardado na rede ainda" },
  );
  trocar(
    "ar-guarda",
    el("dt", {}, "Cota oferecida"),
    el("dd", { class: "num" }, d.guarda.cota_mib ? bytes(d.guarda.cota_mib * 1024 * 1024) : "nenhuma (não guarda)"),
    el("dt", {}, "Em uso"),
    el("dd", { class: "num" }, bytes(d.guarda.usado_bytes)),
    el("dt", {}, "Fragmentos guardados"),
    el("dd", { class: "num" }, fmt(d.guarda.fragmentos)),
    el("dt", {}, "Preço anunciado"),
    el("dd", { class: "num" }, `${fmt(d.ajustes.preco_gb_mes_mili / 1000, 3)} cr / GB·mês`),
  );
}

async function comando(rota, a, textoOk) {
  const r = await postar(rota, { arquivo: a.arquivo });
  resultado("ar-saida", r, `"${a.nome}": ${textoOk}`);
  ler();
}

async function apagar(a) {
  if (!confirm(`Apagar "${a.nome}" da rede? Os guardiões recebem o pedido para apagar os fragmentos e este computador esquece a chave: sem ela, o arquivo não volta.`)) return;
  await comando("/nuvem/apagar", a, "apagado");
}

export function montar() {
  $("ar-k").addEventListener("input", explicar);
  $("ar-m").addEventListener("input", explicar);
  explicar();
  $("ar-form").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const f = $("ar-arquivo").files?.[0];
    const s = $("ar-saida");
    if (!f) {
      s.className = "saida erro";
      s.textContent = "Escolha um arquivo primeiro.";
      return;
    }
    if (f.size > 64 * 1024 * 1024) {
      s.className = "saida erro";
      s.textContent = "Arquivo grande demais: o máximo é 64 MiB.";
      return;
    }
    s.className = "saida";
    s.textContent = `Cifrando e dividindo "${f.name}"…`;
    const r = await postarBytes("/nuvem/guardar", await f.arrayBuffer(), { nome: f.name, k: $("ar-k").value, m: $("ar-m").value });
    resultado("ar-saida", r, r.ok ? `"${f.name}" cifrado e a caminho dos guardiões (id ${curto(r.dados.arquivo, 12)}).` : "");
    if (r.ok) {
      $("ar-arquivo").value = "";
      ler();
    }
  });
  aoMudar(() => {
    if (!$("tela-armazenamento").hidden) desenhar();
  });
}

export function aoMostrar() {
  mostrar();
  desenhar();
}

export function aoEsconder() {
  esconder();
}
