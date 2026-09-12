// Ponto de entrada: idioma, mundo 3D (ou 2D), navegação e capítulos.
import { carregar, aplicar, t, html, idiomaInicial, idiomaAtual, IDIOMAS } from "./i18n.js";
import { detectarQualidade } from "./quality.js";
import { SimulationEngine } from "./simulation/engine.js";
import { criarAudio } from "./audio.js";
import { el } from "./ui.js";

const q = detectarQualidade();
const ouvintes = [];
const aoMudarIdioma = (fn) => ouvintes.push(fn);

async function criarMundo() {
  const canvas = document.getElementById("mundo");
  if (q.perfil) {
    try { return (await import("./world/world.js")).criarMundo(canvas, q); }
    catch (e) { console.error("3D indisponível", e); }
  }
  const c2 = document.createElement("canvas");
  c2.id = "mundo-2d"; c2.setAttribute("aria-hidden", "true");
  canvas.replaceWith(c2);
  return (await import("./world/fallback2d.js")).criarMundo2D(c2, q.calmo);
}

function menuIdiomas() {
  const botao = document.getElementById("idioma-atual"), lista = document.getElementById("idioma-lista");
  const fechar = () => { lista.hidden = true; botao.setAttribute("aria-expanded", "false"); };
  function desenhar() {
    botao.textContent = IDIOMAS.find((i) => i.codigo === idiomaAtual()).curto;
    botao.setAttribute("aria-label", `Idioma / Language: ${IDIOMAS.find((i) => i.codigo === idiomaAtual()).nome}`);
    lista.replaceChildren(...IDIOMAS.map((i) => el("li", {}, el("button", { type: "button", lang: i.codigo, "aria-current": String(i.codigo === idiomaAtual()), onclick: () => { fechar(); trocar(i.codigo); } }, i.nome, el("span", { text: i.curto })))));
  }
  botao.addEventListener("click", () => { const abrir = lista.hidden; lista.hidden = !abrir; botao.setAttribute("aria-expanded", String(abrir)); if (abrir) lista.querySelector("button").focus(); });
  document.addEventListener("click", (e) => { if (!e.target.closest("#idiomas")) fechar(); });
  document.addEventListener("keydown", (e) => { if (e.key === "Escape") fechar(); });
  desenhar();
  aoMudarIdioma(desenhar);
}

async function trocar(codigo) {
  await carregar(codigo);
  aplicar();
  ouvintes.forEach((fn) => { try { fn(); } catch (e) { console.error(e); } });
  try { const u = new URL(location.href); u.searchParams.set("lang", codigo); history.replaceState(null, "", u); } catch { /* sem history em alguns visualizadores */ }
}

function navegacao(mundo) {
  const barra = document.getElementById("barra");
  const gaveta = document.getElementById("gaveta"), abre = document.getElementById("abre-menu"), fecha = document.getElementById("fecha-menu");
  const abrirGaveta = (sim) => { gaveta.hidden = !sim; abre.setAttribute("aria-expanded", String(sim)); document.body.style.overflow = sim ? "hidden" : ""; if (sim) fecha.focus(); else abre.focus(); };
  abre.addEventListener("click", () => abrirGaveta(true));
  fecha.addEventListener("click", () => abrirGaveta(false));
  gaveta.querySelectorAll("a").forEach((a) => a.addEventListener("click", () => { gaveta.hidden = true; document.body.style.overflow = ""; abre.setAttribute("aria-expanded", "false"); }));
  gaveta.addEventListener("keydown", (e) => { if (e.key === "Escape") abrirGaveta(false); });

  const secoes = [...document.querySelectorAll("[data-mundo]")];
  const links = [...document.querySelectorAll(".menu a")];
  let modoAtual = "", pedido = false;
  function avaliar() {
    pedido = false;
    barra.classList.toggle("rolou", window.scrollY > 40);
    const meio = window.innerHeight * 0.5;
    const s = secoes.find((x) => { const r = x.getBoundingClientRect(); return r.top <= meio && r.bottom >= meio; }) || secoes[0];
    const modo = s.dataset.mundo;
    document.body.classList.toggle("mundo-ao-fundo", !["topo", "escala", "fim"].includes(s.id));
    if (modo !== modoAtual && document.getElementById("palco-edit").hidden) { modoAtual = modo; mundo.definirModo(modo); }
    links.forEach((a) => a.setAttribute("aria-current", String(a.getAttribute("href") === `#${s.id}`)));
  }
  window.addEventListener("scroll", () => { if (!pedido) { pedido = true; requestAnimationFrame(avaliar); } }, { passive: true });
  avaliar();
  return () => { modoAtual = ""; avaliar(); };
}

function revelar() {
  if (q.calmo || !("IntersectionObserver" in window)) return;
  const obs = new IntersectionObserver((es) => es.forEach((e) => { if (e.isIntersecting) { e.target.classList.add("visto"); obs.unobserve(e.target); } }), { rootMargin: "0px 0px -6% 0px" });
  document.querySelectorAll(".revela").forEach((x) => { if (x.getBoundingClientRect().top > window.innerHeight) obs.observe(x); });
}

// Musica de fundo: trilha propria, desligada por padrao. Som so comeca com um
// toque da pessoa, que e o que o navegador exige e o que a boa educacao pede.
function musica(audio) {
  const botao = document.getElementById("som");
  if (!botao) return;
  if (!audio) { botao.hidden = true; return; }
  let ligada = false;
  botao.addEventListener("click", async () => {
    ligada = !ligada;
    botao.setAttribute("aria-pressed", String(ligada));
    if (ligada) {
      const pronto = await audio.iniciar();
      if (pronto) audio.tocar({ volume: 0.32 });
      else { ligada = false; botao.setAttribute("aria-pressed", "false"); }
    } else {
      audio.parar({ suave: true });
    }
    try { localStorage.setItem("auron-musica", ligada ? "1" : "0"); } catch { /* sem armazenamento */ }
  });
}

async function iniciar() {
  await carregar(idiomaInicial());
  aplicar();
  // maquina fraca ou celular: sem desfoque de vidro e sem sombras caras
  document.body.classList.toggle("leve", q.nivel === "LOW" || q.movel);
  if (!q.calmo) document.getElementById("topo").classList.add("entrando");
  if (q.calmo) document.querySelectorAll(".anima").forEach((x) => x.remove());

  const mundo = await criarMundo();
  document.getElementById("qualidade").textContent = mundo.nivel === "2D" ? t("ui.sem_webgl") : `${t("ui.qualidade")}: ${mundo.nivel}`;
  aoMudarIdioma(() => { document.getElementById("qualidade").textContent = mundo.nivel === "2D" ? t("ui.sem_webgl") : `${t("ui.qualidade")}: ${mundo.nivel}`; });

  const restaurarMundo = navegacao(mundo);
  menuIdiomas();
  revelar();

  const engine = new SimulationEngine(2026);
  const audio = criarAudio();
  musica(audio);
  const ctx = { t, html, engine, mundo, audio, calmo: q.calmo, movel: q.movel, aoMudarIdioma, idioma: idiomaAtual, restaurarMundo };
  const capitulos = ["nucleo", "cadeia", "nos", "fragmentacao", "radio", "utrax", "direct", "malha", "seguranca", "economia", "escala", "caminho", "aberto", "edit"];
  for (const nome of capitulos) {
    try { (await import(`./sections/${nome}.js`)).iniciar(ctx); }
    catch (e) { console.error(`capítulo ${nome}`, e); }
  }
}

iniciar();
