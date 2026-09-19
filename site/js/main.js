// Hyurax — a rede viva. Liga idiomas, abertura 3D, experimentos e o resto.

import { IDIOMAS, carregar, aplicar, t, idiomaAtual, idiomaInicial } from "./i18n.js";
import { detectarQualidade } from "./quality.js";
import { Escritor, sha512, hex } from "./simulation/engine.js";
import { iniciarTrabalho } from "./secoes/trabalho.js";
import { iniciarConfira } from "./secoes/confira.js";
import { iniciarEter } from "./secoes/eter.js";

const BITCOIN = "bc1qkp7d90t9tnmuv2rwq742pwc8pnet28a59zdzt7";
document.documentElement.classList.add("js");
const qualidade = detectarQualidade();
const calmo = qualidade.calmo;
const secoes = [];

// ---------- idiomas ----------
const botaoIdioma = document.getElementById("idioma-atual");
const listaIdioma = document.getElementById("idioma-lista");
function montarIdiomas() {
  const atual = IDIOMAS.find((i) => i.codigo === idiomaAtual());
  botaoIdioma.textContent = atual ? atual.curto : "PT";
  botaoIdioma.setAttribute("aria-label", t("ui.idioma"));
  listaIdioma.replaceChildren(...IDIOMAS.map((i) => {
    const li = document.createElement("li");
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("aria-current", String(i.codigo === idiomaAtual()));
    b.append(i.nome, Object.assign(document.createElement("span"), { textContent: i.curto }));
    b.addEventListener("click", async () => { fecharIdiomas(); await trocarIdioma(i.codigo); });
    li.append(b);
    return li;
  }));
}
function fecharIdiomas() { listaIdioma.hidden = true; botaoIdioma.setAttribute("aria-expanded", "false"); }
botaoIdioma.addEventListener("click", () => {
  const abrir = listaIdioma.hidden;
  listaIdioma.hidden = !abrir;
  botaoIdioma.setAttribute("aria-expanded", String(abrir));
});
document.addEventListener("click", (e) => { if (!e.target.closest("#idiomas")) fecharIdiomas(); });
document.addEventListener("keydown", (e) => { if (e.key === "Escape") { fecharIdiomas(); fecharMenu(); } });

async function trocarIdioma(codigo) {
  await carregar(codigo);
  aplicar();
  montarIdiomas();
  montarEstado();
  secoes.forEach((s) => s.redesenhar());
  // Os blocos que já estavam no painel também mudam de língua.
  listaVivos.querySelectorAll("span[data-tipo]").forEach((s) => {
    s.textContent = `#${s.dataset.altura} · ${t(`topo.tipo.${s.dataset.tipo}`)}`;
  });
}

// ---------- menu no celular ----------
const menu = document.getElementById("menu");
const abrirMenu = document.getElementById("abrir-menu");
function fecharMenu() { menu.classList.remove("aberto"); abrirMenu.setAttribute("aria-expanded", "false"); }
abrirMenu.addEventListener("click", () => {
  const abrir = !menu.classList.contains("aberto");
  menu.classList.toggle("aberto", abrir);
  abrirMenu.setAttribute("aria-expanded", String(abrir));
});
menu.addEventListener("click", (e) => { if (e.target.closest("a")) fecharMenu(); });

// ---------- barra e navegação ----------
const barra = document.getElementById("barra");
const aoRolar = () => barra.classList.toggle("solida", scrollY > 40);
addEventListener("scroll", aoRolar, { passive: true });
aoRolar();
const links = [...menu.querySelectorAll("a")];
const observarSecao = new IntersectionObserver((es) => {
  es.forEach((e) => {
    if (e.isIntersecting) {
      e.target.classList.add("visto");
      links.forEach((a) => a.setAttribute("aria-current", String(a.getAttribute("href") === `#${e.target.id}`)));
    }
  });
}, { rootMargin: "-35% 0px -55% 0px" });
document.querySelectorAll(".capitulo").forEach((s) => observarSecao.observe(s));
// Quem abre a página já no meio (link com #) vê tudo sem esperar.
if (location.hash) document.querySelectorAll(".capitulo").forEach((s) => s.classList.add("visto"));

// ---------- a rede viva ----------
const listaVivos = document.getElementById("blocos-vivos");
let anteriorVivo = new Uint8Array(64);
async function blocoVivo({ altura, tipo }) {
  // Cabeçalho mínimo com o hash do anterior: o SHA-512 é de verdade.
  const cab = new Escritor().u64(altura).fixo(anteriorVivo).u64(Math.floor(Date.now() / 1000)).bytes();
  const h = await sha512(cab);
  anteriorVivo = h;
  const li = document.createElement("li");
  const esq = document.createElement("span");
  esq.dataset.altura = String(altura);
  esq.dataset.tipo = tipo;
  esq.textContent = `#${altura} · ${t(`topo.tipo.${tipo}`)}`;
  const dir = document.createElement("b");
  dir.textContent = hex(h).slice(0, 10) + "…";
  li.append(esq, dir);
  listaVivos.prepend(li);
  while (listaVivos.children.length > 5) listaVivos.lastElementChild.remove();
}

async function iniciarRede() {
  const canvas = document.getElementById("rede");
  if (qualidade.nivel === "NONE") {
    // Sem WebGL: a página segue inteira; só o painel ao vivo anda.
    let a = 0;
    blocoVivo({ altura: ++a, tipo: "celular" });
    if (!calmo) setInterval(() => blocoVivo({ altura: ++a, tipo: ["celular", "computador", "servidor"][a % 3] }), 3200);
    return;
  }
  try {
    const { criarRedeViva } = await import("./rede/viva.js");
    criarRedeViva(canvas, { nivel: qualidade.nivel, calmo, aoBloco: blocoVivo });
  } catch (erro) {
    console.warn("rede 3D indisponível:", erro);
  }
}

// ---------- onde estamos ----------
function montarEstado() {
  const quadro = document.getElementById("quadro-estado");
  const colunas = [["pronto", "estado.pronto"], ["andamento", "estado.andamento"], ["planejado", "estado.planejado"]];
  quadro.replaceChildren(...colunas.map(([classe, chave]) => {
    const col = document.createElement("div");
    col.className = `coluna-estado ${classe}`;
    const h3 = document.createElement("h3");
    h3.append(document.createElement("i"), t(`${chave}.titulo`));
    col.append(h3);
    const itens = t(`${chave}.itens`);
    (Array.isArray(itens) ? itens : []).forEach(([titulo, texto]) => {
      const item = document.createElement("div");
      item.className = "item-estado";
      item.append(Object.assign(document.createElement("b"), { textContent: titulo }),
        Object.assign(document.createElement("span"), { textContent: texto }));
      col.append(item);
    });
    return col;
  }));
}

// ---------- doação ----------
function montarDoacao() {
  const qr = document.getElementById("qr");
  const desenharQr = () => {
    if (typeof globalThis.qrcode !== "function" || qr.childElementCount) return;
    const q = globalThis.qrcode(0, "M");
    q.addData(`bitcoin:${BITCOIN}`);
    q.make();
    qr.innerHTML = q.createSvgTag({ cellSize: 4, margin: 0, scalable: true });
  };
  if (document.readyState === "complete") desenharQr(); else addEventListener("load", desenharQr);
  const copiar = document.getElementById("copiar");
  copiar.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(BITCOIN);
      copiar.textContent = t("apoie.copiado");
    } catch {
      copiar.textContent = t("apoie.selecione");
      getSelection().selectAllChildren(document.getElementById("endereco"));
    }
    setTimeout(() => { copiar.textContent = t("apoie.copiar"); }, 2400);
  });
}

// ---------- o laço: as bolinhas andam pelos trilhos ----------
function animarLaco() {
  const svg = document.querySelector(".fluxo svg");
  if (!svg || calmo) return;
  const [ida, volta] = svg.querySelectorAll(".trilho");
  const [bIda, bVolta] = svg.querySelectorAll(".bola");
  let inicio = null;
  const passo = (agora) => {
    requestAnimationFrame(passo);
    if (inicio === null) inicio = agora;
    const f = ((agora - inicio) / 2600) % 1;
    const g = ((agora - inicio + 1300) / 2600) % 1;
    const p = ida.getPointAtLength(ida.getTotalLength() * f);
    const q = volta.getPointAtLength(volta.getTotalLength() * g);
    bIda.setAttribute("cx", p.x); bIda.setAttribute("cy", p.y);
    bVolta.setAttribute("cx", q.x); bVolta.setAttribute("cy", q.y);
  };
  requestAnimationFrame(passo);
}

// ---------- início ----------
(async () => {
  await carregar(idiomaInicial());
  aplicar();
  montarIdiomas();
  montarEstado();
  montarDoacao();
  animarLaco();
  const ctx = { t, idioma: idiomaAtual, calmo };
  secoes.push(iniciarTrabalho(ctx), iniciarConfira(ctx), iniciarEter(ctx));
  // A gênese aparece na hora: o painel não fica vazio enquanto o 3D carrega.
  await blocoVivo({ altura: 0, tipo: "genese" });
  iniciarRede();
})();
