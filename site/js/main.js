// Hyurax — a rede viva. Liga idiomas, abertura 3D, experimentos e o resto.

import { IDIOMAS, carregar, aplicar, t, idiomaAtual, idiomaInicial } from "./i18n.js";
import { detectarQualidade } from "./quality.js";
import { Escritor, sha512, hex } from "./simulation/engine.js";
import { iniciarTrabalho } from "./secoes/trabalho.js";
import { iniciarConfira } from "./secoes/confira.js";
import { iniciarEter } from "./secoes/eter.js";
import { iniciarIa } from "./secoes/ia.js";

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
  // Os blocos que já estavam na fita também mudam de língua.
  document.querySelectorAll("#fita-trilho li").forEach(textoDoBloco);
}

// ---------- aviso de privacidade ----------
// Informa uma vez e sai. Não pede consentimento porque não há o que consentir:
// sem cookies, sem rastreamento, sem terceiros. O "já vi" fica no navegador,
// junto da escolha de idioma, e nada disso sai dele.
function mostrarAvisoDePrivacidade() {
  const aviso = document.getElementById("aviso-privacidade");
  if (!aviso) return;
  let visto = false;
  try { visto = localStorage.getItem("hyurax-aviso-privacidade") === "1"; } catch { /* sem armazenamento: mostra */ }
  aviso.hidden = visto;
  document.getElementById("aviso-ok")?.addEventListener("click", () => {
    aviso.hidden = true;
    try { localStorage.setItem("hyurax-aviso-privacidade", "1"); } catch { /* segue */ }
  });
}

// ---------- menu no celular ----------
// A barra se inverte sobre o que passa por baixo (mix-blend-mode); com o menu
// aberto ela precisa de fundo sólido, então sai do modo invertido.
const menu = document.getElementById("menu");
const abrirMenu = document.getElementById("abrir-menu");
const barra = document.getElementById("barra");
function fecharMenu() { barra.classList.remove("menu-aberto"); abrirMenu.setAttribute("aria-expanded", "false"); }
abrirMenu.addEventListener("click", () => {
  const abrir = !barra.classList.contains("menu-aberto");
  barra.classList.toggle("menu-aberto", abrir);
  abrirMenu.setAttribute("aria-expanded", String(abrir));
});
menu.addEventListener("click", (e) => { if (e.target.closest("a")) fecharMenu(); });

// ---------- lente: um círculo que inverte o que passa por baixo ----------
const lente = document.getElementById("lente");
if (lente && matchMedia("(pointer: fine)").matches && !calmo) {
  let x = -100, y = -100, lx = -100, ly = -100;
  addEventListener("pointermove", (e) => {
    x = e.clientX; y = e.clientY;
    lente.classList.toggle("grande", !!e.target.closest("a, button, .cel, .bloco"));
  }, { passive: true });
  const seguir = () => {
    lx += (x - lx) * 0.22; ly += (y - ly) * 0.22;
    lente.style.transform = `translate3d(${lx}px, ${ly}px, 0)`;
    requestAnimationFrame(seguir);
  };
  requestAnimationFrame(seguir);
}

// ---------- navegação ----------
const links = [...menu.querySelectorAll("a")];
const observarSecao = new IntersectionObserver((es) => {
  es.forEach((e) => {
    if (e.isIntersecting) {
      e.target.classList.add("visto");
      links.forEach((a) => a.setAttribute("aria-current", String(a.getAttribute("href") === `#${e.target.id}`)));
    }
  });
}, { rootMargin: "-35% 0px -55% 0px" });
document.querySelectorAll(".faixa, #topo").forEach((s) => observarSecao.observe(s));
// Quem abre a página já no meio (link com #) vê tudo sem esperar.
if (location.hash) document.querySelectorAll(".faixa").forEach((s) => s.classList.add("visto"));

// ---------- a fita: cada bloco da rede viva passa por ela ----------
// Duas listas iguais lado a lado: a animação anda metade do trilho e volta,
// sem emenda. O SHA-512 de cada bloco é de verdade.
const fita = document.getElementById("blocos-vivos");
const eco = document.getElementById("blocos-vivos-eco");
const NA_FITA = 10;
let anteriorVivo = new Uint8Array(64);
function textoDoBloco(li) {
  li.replaceChildren(
    Object.assign(document.createElement("i"), { textContent: "✓" }),
    Object.assign(document.createElement("span"), {
      textContent: t("topo.fita", { a: li.dataset.altura, tipo: t(`topo.tipo.${li.dataset.tipo}`), h: li.dataset.hash }),
    }),
  );
}
async function blocoVivo({ altura, tipo }) {
  const cab = new Escritor().u64(altura).fixo(anteriorVivo).u64(Math.floor(Date.now() / 1000)).bytes();
  const h = await sha512(cab);
  anteriorVivo = h;
  for (const lista of [fita, eco]) {
    const li = document.createElement("li");
    Object.assign(li.dataset, { altura: String(altura), tipo, hash: hex(h).slice(0, 12) + "…" });
    textoDoBloco(li);
    lista.prepend(li);
    while (lista.children.length > NA_FITA) lista.lastElementChild.remove();
  }
}

async function iniciarRede() {
  const canvas = document.getElementById("rede");
  if (qualidade.nivel === "NONE") {
    // Sem WebGL: a página segue inteira; só o painel ao vivo anda.
    let a = 9;
    if (!calmo) setInterval(() => blocoVivo({ altura: ++a, tipo: ["celular", "computador", "servidor"][a % 3] }), 3200);
    return;
  }
  try {
    const { criarRedeViva } = await import("./rede/viva.js");
    criarRedeViva(canvas, { nivel: qualidade.nivel, calmo, alturaInicial: 9, aoBloco: blocoVivo });
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
  secoes.push(iniciarTrabalho(ctx), iniciarConfira(ctx), iniciarEter(ctx), iniciarIa(ctx));
  mostrarAvisoDePrivacidade();
  // A fita já nasce cheia: a gênese e os primeiros blocos, antes de o 3D carregar.
  const TIPOS = ["celular", "computador", "celular", "servidor", "celular", "computador"];
  for (let a = 0; a < 10; a++) await blocoVivo({ altura: a, tipo: a ? TIPOS[a % TIPOS.length] : "genese" });
  iniciarRede();
})();
