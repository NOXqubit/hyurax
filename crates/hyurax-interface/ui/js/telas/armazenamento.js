// Hyurax / Ultrax — Arquivos (Cloud Design 2.0): arrastar um arquivo e ele é
// cifrado aqui, cortado em pedaços e espalhado por computadores diferentes.
// Cada arquivo mostra os pedaços (guardado, enviando, perdido), o estado em
// palavras e as ações. O que este nó guarda para outros fica num painel.

import { $, el, texto, fmt, trocar } from "../util.js";
import { postar, postarBytes, url } from "../api.js";
import { estado } from "../estado.js";
import { nuvem, aoMudar, mostrar, esconder, ler } from "../nuvem.js";
import { marcarEstado, resultado, bytes } from "./comum.js";

const SAUDE = {
  integro: ["ok", "Protegido"],
  degradado: ["atencao", "Consertando"],
  em_risco: ["falha", "Em risco"],
  enviando: ["info", "Enviando"],
};
const MAXIMO = 64 * 1024 * 1024;

function explicar() {
  const k = Math.max(1, Number($("ar-k").value) || 1);
  const m = Math.max(0, Number($("ar-m").value) || 0);
  texto(
    "ar-explica",
    `Até 64 MB. Ele é trancado com a sua chave aqui mesmo, cortado em ${k + m} pedaços e cada pedaço vai para um computador diferente. ${m ? `Se até ${m} ${m === 1 ? "deles sumir" : "deles sumirem"}, os outros refazem o que faltou.` : "Sem pedaço de reserva: se um computador sumir, o arquivo se perde."}`,
  );
}

function fragmentos(a) {
  return el(
    "span",
    { class: "fragmentos", role: "img", "aria-label": `${a.guardados} de ${a.k + a.m} pedaços guardados` },
    a.fragmentos.map((f) =>
      el("span", {
        class: `fragmento ${f.estado}${f.paridade ? " paridade" : ""}`,
        title: `pedaço ${f.indice + 1}${f.paridade ? " (reserva)" : ""} · ${f.estado} · ${f.conectado ? "computador online" : "computador fora da rede"}`,
      }),
    ),
  );
}

function extensao(nome) {
  const m = /\.([a-z0-9]{1,4})$/i.exec(nome || "");
  return m ? m[1].toUpperCase() : "ARQ";
}

function desenhar() {
  const d = nuvem.dados;
  if (!d) {
    if (nuvem.erro) marcarEstado("ar-estado", "falha", nuvem.erro);
    return;
  }
  const arquivos = d.arquivos || [];
  const risco = arquivos.filter((a) => a.saude === "em_risco").length;
  const reparo = arquivos.filter((a) => a.saude === "degradado").length;
  const guardioes = (d.mercado || []).filter((a) => a.disco_mib > 0 && a.conectado && a.tipo !== "venda").length;
  marcarEstado(
    "ar-estado",
    risco ? "falha" : reparo ? "atencao" : guardioes ? "ok" : "parado",
    risco ? `${risco} em risco` : reparo ? `${reparo} consertando` : `${guardioes} computador(es) podem guardar agora`,
  );
  const pode = estado.atual?.pode_mandar;
  trocar(
    "ar-arquivos",
    arquivos.length
      ? arquivos.map((a) => {
          const [cor, nome] = SAUDE[a.saude] || ["", a.saude];
          return el(
            "div",
            { class: "linha-arquivo" },
            el("span", { class: "ext" }, extensao(a.nome)),
            el("span", { class: "nome" }, el("b", {}, a.nome), el("small", {}, a.noticia || `${bytes(a.tamanho)} · ${a.k + a.m} pedaços em computadores diferentes`)),
            fragmentos(a),
            el("span", { class: `pilula ${cor}` }, a.recuperando ? "Recuperando" : nome),
            el(
              "span",
              { class: "acoes" },
              a.recuperado
                ? el("a", { class: "botao-leve", href: url(`/nuvem/arquivo/${a.arquivo}`), download: a.nome }, "Baixar")
                : el("button", { type: "button", class: "botao-leve", disabled: !pode || a.recuperando, onclick: () => comando("/nuvem/recuperar", a, "pedido aos computadores que guardam") }, "Trazer de volta"),
              el("button", { type: "button", class: "botao-leve botao-perigo", disabled: !pode, onclick: () => apagar(a), "aria-label": `Apagar ${a.nome}` }, "Apagar"),
            ),
          );
        })
      : [el("div", { class: "vazio" }, el("strong", {}, "Nenhum arquivo ainda"), el("span", {}, guardioes ? "Arraste um arquivo para a área acima." : "Para guardar, é preciso ter computadores conectados que ofereçam espaço (veja o Mercado)."))],
  );
  trocar(
    "ar-guarda",
    el("dt", {}, "Espaço oferecido"),
    el("dd", { class: "num" }, d.guarda.cota_mib ? bytes(d.guarda.cota_mib * 1024 * 1024) : "nenhum"),
    el("dt", {}, "Em uso por outros"),
    el("dd", { class: "num" }, bytes(d.guarda.usado_bytes)),
    el("dt", {}, "Pedaços guardados"),
    el("dd", { class: "num" }, fmt(d.guarda.fragmentos)),
  );
}

async function comando(rota, a, textoOk) {
  const r = await postar(rota, { arquivo: a.arquivo });
  resultado("ar-saida", r, `"${a.nome}": ${textoOk}.`);
  ler();
}

async function apagar(a) {
  if (!confirm(`Apagar "${a.nome}"? Os computadores que guardam recebem o pedido para apagar, e este computador esquece a chave: sem ela, o arquivo não volta.`)) return;
  await comando("/nuvem/apagar", a, "apagado");
}

async function enviar(f) {
  const s = $("ar-saida");
  if (!f) return;
  if (f.size > MAXIMO) {
    s.className = "saida erro";
    s.textContent = "Arquivo grande demais: o máximo é 64 MB.";
    return;
  }
  s.className = "saida";
  s.textContent = `Trancando e cortando "${f.name}"…`;
  const r = await postarBytes("/nuvem/guardar", await f.arrayBuffer(), { nome: f.name, k: $("ar-k").value, m: $("ar-m").value });
  resultado("ar-saida", r, r.ok ? `"${f.name}" trancado e a caminho dos computadores.` : "");
  if (r.ok) ler();
}

export function montar() {
  $("ar-k").addEventListener("input", explicar);
  $("ar-m").addEventListener("input", explicar);
  explicar();
  $("ar-arquivo").addEventListener("change", () => {
    enviar($("ar-arquivo").files?.[0]);
    $("ar-arquivo").value = "";
  });
  const zona = $("ar-form");
  zona.addEventListener("submit", (ev) => ev.preventDefault());
  zona.addEventListener("dragover", (ev) => {
    ev.preventDefault();
    zona.classList.add("sobre");
  });
  zona.addEventListener("dragleave", () => zona.classList.remove("sobre"));
  zona.addEventListener("drop", (ev) => {
    ev.preventDefault();
    zona.classList.remove("sobre");
    if (estado.atual?.pode_mandar) enviar(ev.dataTransfer?.files?.[0]);
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

