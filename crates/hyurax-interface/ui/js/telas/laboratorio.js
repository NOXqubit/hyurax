// Hyurax / Ultrax — Moléculas em 3D. Uma molécula real da AqSolDB, montada
// em 3D a partir do SMILES (js/molecula3d.js), com a solubilidade medida em
// laboratório ao lado da prevista pela IA do ULTRAX e o erro típico dela.

import { $, el, texto, fmt, trocar } from "../util.js";
import { obter } from "../api.js";
import { fatos } from "./comum.js";
import { visualizador, corDe } from "../molecula3d.js";
import { lerSmiles } from "../moleculas.js";

// algumas conhecidas para a primeira abertura (busca por nome no catálogo)
const VITRINE = ["caffeine", "acetylsalicylic acid", "aspirin", "ibuprofen", "paracetamol", "acetaminophen", "glucose", "d-glucose", "sucrose", "vanillin", "menthol", "nicotine", "naphthalene", "cholesterol", "urea", "ethanol", "benzene"];
const NOMES = { H: "hidrogênio", C: "carbono", N: "nitrogênio", O: "oxigênio", F: "flúor", P: "fósforo", S: "enxofre", Cl: "cloro", Br: "bromo", I: "iodo", B: "boro", Si: "silício" };

let vis = null;
let total = 0;
let atual = null;
let montado = false;
let buscaVez = 0;

// log S de -10 a +2 ocupa a régua inteira
const naRegua = (logs) => `${Math.max(0, Math.min(100, ((logs + 10) / 12) * 100))}%`;

function faixa(logs) {
  if (logs >= 0) return "muito solúvel";
  if (logs >= -2) return "solúvel";
  if (logs >= -4) return "pouco solúvel";
  return "quase insolúvel";
}

function gramasPorLitro(logs, massaMili) {
  const g = 10 ** logs * (massaMili / 1000);
  if (!Number.isFinite(g) || g <= 0) return "—";
  if (g >= 1) return `${fmt(g, g >= 100 ? 0 : 1)} g`;
  if (g >= 0.001) return `${fmt(g * 1000, 1)} mg`;
  return `${fmt(g * 1e6, 2)} µg`;
}

async function abrir(indice) {
  const r = await obter(`/ciencia/molecula/${indice}`);
  if (!r.ok) {
    texto("lb-nome", "Não consegui carregar a molécula");
    return;
  }
  const m = r.dados;
  atual = m;
  total = m.total || total;
  texto("lb-nome", m.nome || m.id);
  texto("lb-formula", m.formula);
  for (const b of document.querySelectorAll("#lb-achadas button")) b.setAttribute("aria-current", String(Number(b.dataset.indice) === indice));
  vis.mostrar(m.smiles);
  legenda(m.smiles);
}

function legenda(smiles) {
  const els = new Set(lerSmiles(smiles).atomos.map((a) => a.el));
  els.add("H");
  trocar(
    "lb-legenda",
    [...els].sort().map((e) => el("li", {}, el("i", { style: `background:${corDe(e)}` }), NOMES[e] || e)),
  );
}

function ficha(m, info) {
  const medido = m.logs_medido_mili / 1000;
  const massa = m.descritores?.[0] ?? 0;
  const linhas = [
    ["Massa molar", `${fmt(massa / 1000, 2)} g/mol`],
    ["Átomos (com H)", info ? fmt(info.atomos) : "—"],
    ["Ligações", info ? fmt(info.ligacoes) : "—"],
    ["Medido em laboratório", `log S ${fmt(medido, 2)} · ${gramasPorLitro(medido, massa)} por litro`],
  ];
  if (m.previsto_mili != null) linhas.push(["Previsão da IA", `log S ${fmt(m.previsto_mili / 1000, 2)}`]);
  linhas.push(["No catálogo", `nº ${fmt(m.indice + 1)} de ${fmt(total)} · ${m.id}`]);
  fatos("lb-fatos", linhas);
  $("lb-p-medido").style.left = naRegua(medido);
  const prev = $("lb-p-previsto");
  prev.hidden = m.previsto_mili == null;
  if (m.previsto_mili == null) {
    trocar("lb-veredito", "Medido: ", el("b", {}, faixa(medido)), ". O modelo da IA não está embutido nesta montagem.");
    return;
  }
  const previsto = m.previsto_mili / 1000;
  const erro = (m.erro_tipico_mili ?? 0) / 1000;
  prev.style.left = naRegua(previsto);
  const diferenca = Math.abs(previsto - medido);
  const dentro = erro > 0 && diferenca <= erro;
  trocar(
    "lb-veredito",
    "Em água, ela é ", el("b", {}, faixa(medido)), " (ponto escuro, medido em laboratório). A IA do ULTRAX previu ",
    el("b", {}, faixa(previsto)), " (ponto amarelo): errou por ", el("b", {}, `${fmt(diferenca, 2)} log S`),
    erro > 0 ? `, ${dentro ? "dentro" : "fora"} do erro típico dela (±${fmt(erro, 2)}, medido em moléculas que ela não viu no treino).` : ".",
  );
}

async function buscar(termo) {
  const vez = ++buscaVez;
  const r = await obter(`/ciencia/moleculas?busca=${encodeURIComponent(termo)}`);
  if (vez !== buscaVez || !r.ok) return [];
  total = r.dados.total || total;
  return r.dados.achadas || [];
}

function listar(achadas) {
  trocar(
    "lb-achadas",
    achadas.length
      ? achadas.map((a) =>
          el("li", {}, el("button", { type: "button", "data-indice": String(a.indice), title: `${a.nome} (${a.formula})`, "aria-current": String(atual?.indice === a.indice), onclick: () => abrir(a.indice) }, a.nome || a.formula)),
        )
      : [el("li", { class: "nota" }, "Nada com esse nome. Tente em inglês (a base é internacional): caffeine, glucose…")],
  );
}

async function vitrine() {
  const achadas = [];
  const respostas = await Promise.all(VITRINE.map((nome) => obter(`/ciencia/moleculas?busca=${encodeURIComponent(nome)}`)));
  for (const [k, nome] of VITRINE.entries()) {
    const r = respostas[k];
    const exata = r.ok && (r.dados.achadas || []).find((a) => a.nome.toLowerCase() === nome);
    if (exata && !achadas.some((a) => a.indice === exata.indice)) achadas.push(exata);
    if (r.ok) total = r.dados.total || total;
  }
  listar(achadas);
  if (achadas.length) abrir(achadas[0].indice);
  else abrir(0);
}

function montarUmaVez() {
  if (montado) return;
  montado = true;
  vis = visualizador($("lb-canvas"), (info) => { if (atual) ficha(atual, info); });
  for (const b of document.querySelectorAll("[data-lb-modo]")) {
    b.addEventListener("click", () => {
      for (const o of document.querySelectorAll("[data-lb-modo]")) o.setAttribute("aria-pressed", String(o === b));
      vis.modo(b.dataset.lbModo);
    });
  }
  const alternar = (id, aoMudar) => {
    $(id).addEventListener("click", () => {
      const v = $(id).getAttribute("aria-pressed") !== "true";
      $(id).setAttribute("aria-pressed", String(v));
      aoMudar(v);
    });
  };
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) $("lb-girar").setAttribute("aria-pressed", "false");
  alternar("lb-h", (v) => vis.hidrogenios(v));
  alternar("lb-girar", (v) => vis.girar(v));
  alternar("lb-vibrar", (v) => {
    $("lb-temp-rotulo").hidden = !v;
    vis.vibrar(v, Number($("lb-temp").value));
  });
  $("lb-temp").addEventListener("input", (e) => {
    texto("lb-temp-valor", `${e.target.value} K`);
    vis.temperatura(Number(e.target.value));
  });
  let espera = 0;
  $("lb-busca").addEventListener("input", (e) => {
    clearTimeout(espera);
    const termo = e.target.value.trim();
    espera = setTimeout(async () => { if (termo.length >= 2) listar(await buscar(termo)); else if (!termo) vitrine(); }, 220);
  });
  $("lb-busca-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const achadas = await buscar($("lb-busca").value.trim());
    listar(achadas);
    if (achadas.length) abrir(achadas[0].indice);
  });
  $("lb-sortear").addEventListener("click", () => abrir(Math.floor(Math.random() * Math.max(total, 1))));
  vitrine();
}

export function aoMostrar() {
  montarUmaVez();
  if (atual && vis) {
    vis.girar($("lb-girar").getAttribute("aria-pressed") === "true");
    if ($("lb-vibrar").getAttribute("aria-pressed") === "true") vis.vibrar(true, Number($("lb-temp").value));
  }
}

export function aoEsconder() {
  vis?.parar();
}
