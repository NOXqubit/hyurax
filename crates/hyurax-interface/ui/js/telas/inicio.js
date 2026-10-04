// Hyurax / Ultrax — Início (Cloud Design 2.0): a tela que um novato vê
// primeiro. Diz em português simples se o computador está ajudando a rede,
// liga e desliga com um botão, mostra quatro números, os primeiros passos e
// o que aconteceu. Tudo sai do estado real do núcleo.
//
// Também cuida das boas-vindas da primeira abertura.

import { $, el, texto, fmt, trocar } from "../util.js";
import { postar } from "../api.js";
import { estado } from "../estado.js";
import { creditos } from "./comum.js";

const BEMVINDO = "hyurax-bemvindo";
let ultimo = null;
let mandando = false;

function saudacao() {
  const h = new Date().getHours();
  return h < 5 ? "Boa noite." : h < 12 ? "Bom dia." : h < 18 ? "Boa tarde." : "Boa noite.";
}

/** HYX de teste legível: "1100.00000000" vira "1.100,00". */
function hyx(t) {
  const n = Number(String(t ?? "").replace(",", "."));
  return Number.isFinite(n) ? new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(n) : "—";
}

function relativo(ms) {
  const s = Math.max(0, (Date.now() - ms) / 1000);
  if (s < 60) return "agora";
  if (s < 3600) return `há ${Math.floor(s / 60)} min`;
  if (s < 86400) return `há ${Math.floor(s / 3600)} h`;
  return new Date(ms).toLocaleDateString("pt-BR");
}

const ICONES = {
  luz: '<rect x="6" y="6" width="12" height="12" rx="2.5"/><path d="M9 2.5v3M15 2.5v3M9 18.5v3M15 18.5v3"/>',
  ok: '<path d="M12 3 5 6v5.5c0 4.4 3 7.9 7 9.5 4-1.6 7-5.1 7-9.5V6z"/><path d="m9 12 2.2 2.2L15.5 10"/>',
  info: '<circle cx="12" cy="5" r="2.2"/><circle cx="5" cy="18" r="2.2"/><circle cx="19" cy="18" r="2.2"/><path d="M10.9 6.9 6.1 16M13.1 6.9l4.8 9.1"/>',
  falha: '<path d="M12 8v5M12 16.5h.01"/><circle cx="12" cy="12" r="9"/>',
  neutro: '<path d="M4 6h16M4 12h16M4 18h10"/>',
};
const POR_CATEGORIA = { ciencia: "luz", ultrax: "luz", mineracao: "luz", nuvem: "ok", carteira: "ok", rede: "info", no: "info", erro: "falha" };

function icone(tipo) {
  const s = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  s.setAttribute("viewBox", "0 0 24 24");
  s.setAttribute("fill", "none");
  s.setAttribute("stroke", "currentColor");
  s.setAttribute("stroke-width", "1.8");
  s.setAttribute("stroke-linecap", "round");
  s.setAttribute("stroke-linejoin", "round");
  s.setAttribute("aria-hidden", "true");
  s.innerHTML = ICONES[tipo] || ICONES.neutro;
  return el("span", { class: `icone ${tipo}` }, s);
}

/** O computador está ajudando a rede: o worker ligado. */
function ajudando(e) {
  return !!e?.ultrax?.ligado;
}

async function alternarAjuda() {
  const e = estado.atual;
  if (!e?.pode_mandar || mandando) return;
  mandando = true;
  const ligar = !ajudando(e);
  // o worker e o "aceitar trabalho de outros nós" andam juntos aqui; os
  // ajustes finos ficam em Worker (ULTRAX)
  await postar("/ultrax", { ligar: ligar ? 1 : 0 });
  await postar("/ciencia/rede", { aceitar: ligar ? 1 : 0 });
  mandando = false;
}

function heroi(e) {
  const u = e.ultrax || {};
  const liga = ajudando(e);
  const pares = e.no?.pares || 0;
  const s = $("in-status");
  s.className = pares ? "heroi-status" : "heroi-status fora";
  s.textContent = pares ? `Online · ${pares} ${pares === 1 ? "computador conectado" : "computadores conectados"}` : "Ainda sem outros computadores conectados";
  $("in-orbe").classList.toggle("parado", !liga);
  texto("in-titulo", liga ? "Seu computador está ajudando a rede." : "Seu computador está só olhando.");
  texto(
    "in-explica",
    liga
      ? `Ele faz contas úteis usando ${u.linhas ?? "—"} de ${e.mineracao?.nucleos ?? "—"} núcleo(s), com limite de ${u.uso_cpu ?? "—"}% em cada um, e cada resultado é conferido antes de valer.${pares ? "" : " Sem outros computadores, ele trabalha em contas de treino até a rede chegar."}`
      : "Nada roda em segundo plano. Você continua vendo a rede, usando a nuvem e a carteira. Ligue quando quiser contribuir.",
  );
  const b = $("in-ajudar");
  b.setAttribute("aria-pressed", String(liga));
  b.disabled = !e.pode_mandar;
  b.title = e.pode_mandar ? "" : "comando só pela janela do programa neste computador";
  texto("in-ajudar-rotulo", liga ? "Ajudando" : "Pausado");
}

function numeros(e) {
  const p = e.ultrax?.placar || {};
  texto("in-trabalho", fmt(p.liquidadas ?? 0));
  texto("in-creditos", creditos(e.jobs?.consumo_mili ?? 0).replace(" cr", ""));
  if (e.carteira?.existe) {
    trocar("in-saldo", hyx(e.carteira.saldo), el("small", {}, "HYX"));
    texto("in-saldo-nota", Number(String(e.carteira.imaturo).replace(",", ".")) > 0 ? `+ ${hyx(e.carteira.imaturo)} HYX liberando (recompensa recente)` : "HYX de teste, sem valor real");
  } else {
    texto("in-saldo", "—");
    texto("in-saldo-nota", "crie uma carteira para receber");
  }
  texto("in-arquivos", fmt(e.nuvem?.arquivos ?? 0));
}

function passos(e) {
  const lista = [
    [!!e.carteira?.existe, "Criar a carteira", "Ela fica cifrada com a sua senha, neste computador.", "#carteira", "Criar"],
    [ajudando(e), "Deixar o computador ajudar a rede", "Ele faz contas úteis e você ganha créditos.", null, "Ligar"],
    [(e.nuvem?.arquivos ?? 0) > 0, "Guardar o primeiro arquivo", "Ele é cifrado aqui antes de sair.", "#armazenamento", "Guardar"],
    [!!e.nuvem?.anunciando, "Oferecer seu computador no mercado", "Outras pessoas alugam a capacidade livre.", "#mercado", "Oferecer"],
  ];
  const feitos = lista.filter(([f]) => f).length;
  texto("in-passos-cont", `${feitos} de ${lista.length}`);
  $("in-passos-barra").style.width = `${(feitos / lista.length) * 100}%`;
  trocar(
    "in-passos",
    lista.map(([feito, titulo, nota, href, acao]) =>
      el(
        "li",
        { class: feito ? "feito" : null },
        el("span", { class: "marca", "aria-hidden": "true" }),
        el("span", { class: "texto" }, el("b", {}, titulo), feito ? null : el("small", {}, nota)),
        feito
          ? el("span", { class: "visualmente-escondido" }, "feito")
          : href
            ? el("a", { href }, acao)
            : el("button", { type: "button", class: "botao-leve", disabled: !e.pode_mandar, onclick: alternarAjuda }, acao),
      ),
    ),
  );
}

function atividade(e) {
  const regs = (e.registros || []).filter((r) => r.dados?.texto).slice(-6).reverse();
  trocar(
    "in-atividade",
    regs.length
      ? regs.map((r) =>
          el(
            "li",
            {},
            icone(POR_CATEGORIA[r.dados.categoria] || "neutro"),
            el("span", { class: "texto" }, el("span", {}, r.dados.texto)),
            el("time", { datetime: new Date(r.quando_ms).toISOString() }, relativo(r.quando_ms)),
          ),
        )
      : [el("li", {}, icone("neutro"), el("span", { class: "texto" }, el("span", {}, "Nada ainda. Quando algo acontecer na rede, aparece aqui.")))],
  );
}

function mineracao(e) {
  const m = e.mineracao || {};
  const s = $("in-min-estado");
  s.className = `estado ${m.ligada ? "estado-ok" : "estado-parado"}`;
  s.textContent = m.ligada ? "minerando" : "parada";
  texto(
    "in-min-texto",
    !e.carteira?.existe
      ? "Para minerar, crie uma carteira: é para ela que vão as recompensas de teste."
      : m.ligada
        ? `Competindo pelos blocos com ${m.linhas} linha(s). ${fmt(m.meus_sessao ?? 0)} bloco(s) nesta sessão, ${fmt(m.meus_cadeia ?? 0)} na cadeia.`
        : "Parada. Ligue para competir pelos blocos de teste (o HYX de teste não tem valor).",
  );
}

export function atualizar(e) {
  ultimo = e;
  texto("t-inicio", saudacao());
  texto("in-data", new Date().toLocaleDateString("pt-BR", { weekday: "long", day: "numeric", month: "long" }).replace(/^./, (c) => c.toUpperCase()));
  heroi(e);
  numeros(e);
  passos(e);
  atividade(e);
  mineracao(e);
  boasVindas(e);
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}

// ---------- boas-vindas ----------
let escolha = "contribuir";
let bvMostrada = false;

function boasVindas(e) {
  if (bvMostrada || !e?.pode_mandar) return;
  let visto = "1";
  try {
    visto = localStorage.getItem(BEMVINDO) || "";
  } catch {
    /* sem armazenamento: não insiste */
  }
  bvMostrada = true;
  if (!visto) abrirBoasVindas();
}

export function abrirBoasVindas() {
  $("boasvindas").hidden = false;
  $("bv-continuar").focus();
}

export function montar() {
  $("in-ajudar").addEventListener("click", alternarAjuda);
  for (const b of document.querySelectorAll("#boasvindas [data-bv]")) {
    b.addEventListener("click", () => {
      escolha = b.dataset.bv;
      for (const o of document.querySelectorAll("#boasvindas [data-bv]")) o.setAttribute("aria-pressed", String(o === b));
    });
  }
  $("bv-continuar").addEventListener("click", async () => {
    try {
      localStorage.setItem(BEMVINDO, escolha);
    } catch {
      /* vale só nesta abertura */
    }
    if (escolha === "contribuir" && !ajudando(estado.atual)) await alternarAjuda();
    $("boasvindas").hidden = true;
    location.hash = escolha === "usar" ? "#computar" : "#inicio";
  });
  if (ultimo) atualizar(ultimo);
}
