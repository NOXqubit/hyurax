// Hyurax / Ultrax — os dados da nuvem (mercado, armazenamento, livro), lidos
// de /api/v1/nuvem e /api/v1/nuvem/livro. As três telas da nuvem usam este
// módulo: lê ao mostrar, de 3 em 3 s enquanto alguma delas está na tela, e
// logo depois de cada evento "nuvem" do núcleo.

import { obter } from "./api.js";
import { ouvir } from "./estado.js";

export const nuvem = {
  /** O último /api/v1/nuvem (null até chegar). */
  dados: null,
  /** O último /api/v1/nuvem/livro. */
  livro: null,
  /** O erro da última leitura ("" se leu). */
  erro: "",
};

const ouvintes = new Set();
let visiveis = 0;
let relogio = null;
let pendente = null;

/** Avisa quando os dados mudam. */
export function aoMudar(fn) {
  ouvintes.add(fn);
  return () => ouvintes.delete(fn);
}

/** Lê agora (as duas rotas). */
export async function ler() {
  const [d, l] = await Promise.all([obter("/nuvem"), obter("/nuvem/livro")]);
  nuvem.erro = d.ok ? "" : d.erro;
  if (d.ok) nuvem.dados = d.dados;
  if (l.ok) nuvem.livro = l.dados;
  for (const fn of ouvintes) {
    try {
      fn(nuvem);
    } catch (e) {
      console.error("ouvinte da nuvem:", e);
    }
  }
  return nuvem;
}

/** Uma tela da nuvem apareceu: lê já e passa a ler de 3 em 3 s. */
export function mostrar() {
  visiveis += 1;
  ler();
  relogio ??= setInterval(ler, 3000);
}

/** Uma tela da nuvem saiu. */
export function esconder() {
  visiveis = Math.max(0, visiveis - 1);
  if (!visiveis && relogio) {
    clearInterval(relogio);
    relogio = null;
  }
}

// um evento da nuvem (anúncio, fragmento guardado, reparo): relê logo
ouvir("nuvem", () => {
  if (!visiveis || pendente) return;
  pendente = setTimeout(() => {
    pendente = null;
    ler();
  }, 300);
});
