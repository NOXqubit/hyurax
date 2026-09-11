// Internacionalização: um HTML só, textos vindos de /locales/<idioma>.js.
// Elementos usam data-i18n (texto), data-i18n-html (texto com ênfase) e
// data-i18n-attr="atributo:chave;outro:chave".

export const IDIOMAS = [
  { codigo: "pt-BR", curto: "PT", nome: "Português" },
  { codigo: "en", curto: "EN", nome: "English" },
  { codigo: "es", curto: "ES", nome: "Español" },
  { codigo: "ja", curto: "JA", nome: "日本語" },
];
const CHAVE_LOCAL = "auron-idioma";
const cache = {};
let dic = {}, base = {}, atual = "pt-BR";

async function carregarDic(codigo) {
  if (!cache[codigo]) cache[codigo] = (await import(`../locales/${codigo}.js`)).default;
  return cache[codigo];
}

export async function carregar(codigo) {
  base = await carregarDic("pt-BR");
  dic = await carregarDic(codigo);
  atual = codigo;
  document.documentElement.lang = codigo;
  try { localStorage.setItem(CHAVE_LOCAL, codigo); } catch { /* sem armazenamento: tudo bem */ }
}

export const idiomaAtual = () => atual;

function busca(obj, chave) {
  return chave.split(".").reduce((o, k) => (o && k in o ? o[k] : undefined), obj);
}
export function t(chave, vars) {
  let v = busca(dic, chave);
  if (v === undefined) v = busca(base, chave);
  if (v === undefined) return chave;
  if (typeof v === "string" && vars) v = v.replace(/\{(\w+)\}/g, (_, k) => (k in vars ? vars[k] : `{${k}}`));
  return v;
}

// Só estas marcas passam para o HTML, sem atributos. O resto vira texto.
const PERMITIDAS = new Set(["EM", "STRONG", "B", "SPAN", "BR"]);
function limpar(html) {
  const tpl = document.createElement("template");
  tpl.innerHTML = html;
  const saida = document.createDocumentFragment();
  const copia = (origem, destino) => {
    origem.childNodes.forEach((n) => {
      if (n.nodeType === Node.TEXT_NODE) destino.appendChild(document.createTextNode(n.textContent));
      else if (n.nodeType === Node.ELEMENT_NODE) {
        if (PERMITIDAS.has(n.tagName)) { const el = document.createElement(n.tagName.toLowerCase()); copia(n, el); destino.appendChild(el); }
        else copia(n, destino);
      }
    });
  };
  copia(tpl.content, saida);
  return saida;
}
export function html(el, chave, vars) { el.replaceChildren(limpar(t(chave, vars))); }

export function aplicar(raiz = document) {
  raiz.querySelectorAll("[data-i18n]").forEach((el) => { el.textContent = t(el.dataset.i18n); });
  raiz.querySelectorAll("[data-i18n-html]").forEach((el) => html(el, el.dataset.i18nHtml));
  raiz.querySelectorAll("[data-i18n-attr]").forEach((el) => {
    el.dataset.i18nAttr.split(";").forEach((par) => { const [a, k] = par.split(":"); el.setAttribute(a.trim(), t(k.trim())); });
  });
  document.title = t("meta.titulo");
  const desc = document.querySelector('meta[name="description"]');
  if (desc) desc.setAttribute("content", t("meta.descricao"));
}

export function idiomaInicial() {
  const ok = (c) => IDIOMAS.some((i) => i.codigo === c);
  const doLink = new URLSearchParams(location.search).get("lang");
  if (doLink && ok(doLink)) return doLink;
  try { const s = localStorage.getItem(CHAVE_LOCAL); if (s && ok(s)) return s; } catch { /* nada */ }
  for (const l of navigator.languages || [navigator.language || ""]) {
    const x = l.toLowerCase();
    if (x.startsWith("pt")) return "pt-BR";
    if (x.startsWith("es")) return "es";
    if (x.startsWith("ja")) return "ja";
    if (x.startsWith("en")) return "en";
  }
  return "pt-BR";
}
