// Hyurax / Ultrax — os pedidos à API local (hyurax-nucleo::api, versão 1).
//
// Leitura: GET. Comando: POST com formulário (x-www-form-urlencoded). O
// núcleo só aceita comando e leitura completa da janela deste computador:
// Host e Origin locais e a chave de sessão desta abertura, que chega pelo
// endereço (`#chave=…`) e vai em todo pedido no cabeçalho X-Hyurax-Chave.

const BASE = "/api/v1";
const GUARDA = "hyurax-chave";

let chave = "";
try {
  chave = localStorage.getItem(GUARDA) || "";
} catch {
  /* sem armazenamento: a chave vale só nesta página */
}

/** Lê a chave do endereço (`#chave=…`), guarda e tira do endereço. */
export function pegarChaveDoEndereco() {
  const m = /^#chave=([0-9a-f]{64})/.exec(location.hash);
  if (!m) return;
  chave = m[1];
  try {
    localStorage.setItem(GUARDA, chave);
  } catch {
    /* idem */
  }
  history.replaceState(null, "", `${location.pathname}#visao`);
}

/** A chave desta sessão (vazia se a página não veio pela janela). */
export function chaveDaSessao() {
  return chave;
}

function cabecalhos(extra = {}) {
  return chave ? { ...extra, "X-Hyurax-Chave": chave } : extra;
}

/** GET em JSON. Devolve { ok, status, dados, erro }. */
export async function obter(caminho) {
  try {
    const r = await fetch(`${BASE}${caminho}`, { cache: "no-store", headers: cabecalhos() });
    const dados = await r.json().catch(() => null);
    return { ok: r.ok, status: r.status, dados, erro: dados?.erro || (r.ok ? "" : erroDe(r.status)) };
  } catch (e) {
    return { ok: false, status: 0, dados: null, erro: `sem resposta do núcleo (${e.message || e})` };
  }
}

/** POST de formulário. Devolve { ok, status, dados, erro }. */
export async function postar(caminho, campos = {}) {
  try {
    const r = await fetch(`${BASE}${caminho}`, {
      method: "POST",
      headers: cabecalhos({ "Content-Type": "application/x-www-form-urlencoded" }),
      body: new URLSearchParams(Object.entries(campos).map(([k, v]) => [k, String(v)])),
    });
    const dados = r.status === 204 ? {} : await r.json().catch(() => null);
    return { ok: r.ok, status: r.status, dados, erro: dados?.erro || (r.ok ? "" : erroDe(r.status)) };
  } catch (e) {
    return { ok: false, status: 0, dados: null, erro: `sem resposta do núcleo (${e.message || e})` };
  }
}

function erroDe(status) {
  if (status === 401 || status === 403) return "recusado: comando só vale pela janela do programa neste computador";
  return `o núcleo respondeu ${status}`;
}

/** Endereço completo de uma rota da API (links de relatório e o fluxo, que
 * não mandam cabeçalho: a chave vai na busca). */
export function url(caminho) {
  if (!chave) return `${BASE}${caminho}`;
  return `${BASE}${caminho}${caminho.includes("?") ? "&" : "?"}chave=${chave}`;
}
