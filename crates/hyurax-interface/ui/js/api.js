// Hyurax / Ultrax — os pedidos à API local (hyurax-nucleo::api, versão 1).
//
// Leitura: GET. Comando: POST com formulário (x-www-form-urlencoded); o
// núcleo só aceita comando deste computador, com Host e Origin locais.

const BASE = "/api/v1";

/** GET em JSON. Devolve { ok, dados, erro }. */
export async function obter(caminho) {
  try {
    const r = await fetch(`${BASE}${caminho}`, { cache: "no-store" });
    const dados = await r.json().catch(() => null);
    return { ok: r.ok, dados, erro: dados?.erro || (r.ok ? "" : `o núcleo respondeu ${r.status}`) };
  } catch (e) {
    return { ok: false, dados: null, erro: `sem resposta do núcleo (${e.message || e})` };
  }
}

/** POST de formulário. Devolve { ok, dados, erro }. */
export async function postar(caminho, campos = {}) {
  try {
    const r = await fetch(`${BASE}${caminho}`, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams(Object.entries(campos).map(([k, v]) => [k, String(v)])),
    });
    const dados = r.status === 204 ? {} : await r.json().catch(() => null);
    return { ok: r.ok, dados, erro: dados?.erro || (r.ok ? "" : r.status === 403 ? "comando recusado: só vale deste computador" : `o núcleo respondeu ${r.status}`) };
  } catch (e) {
    return { ok: false, dados: null, erro: `sem resposta do núcleo (${e.message || e})` };
  }
}

/** Endereço completo de uma rota da API (links de relatório). */
export function url(caminho) {
  return `${BASE}${caminho}`;
}
