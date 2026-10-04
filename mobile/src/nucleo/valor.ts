// Dinheiro em unidades inteiras (1 HYX = 10^8), nunca ponto flutuante: a
// regra de `reference/hyurax/units.py`. No celular, a pessoa digita com
// vírgula ("1,5"); ponto também vale.

export const CASAS = 8;
const UNIDADE = 100_000_000n;
const U64_MAX = (1n << 64n) - 1n;

/** "1,5" ou "1.5" vira 150 000 000n. Lança com a explicação. */
export function unidadesDeTexto(texto: string): bigint {
  const t = texto.trim().replace(/\s/g, "").replace(",", ".");
  const erro = () => new Error(`valor inválido: ${texto.trim() || "vazio"} (até ${CASAS} casas, por exemplo 1,5)`);
  const m = /^(\d*)(?:\.(\d*))?$/.exec(t);
  if (!m || !/\d/.test(t)) throw erro();
  const inteiro = m[1] || "0";
  const fracao = m[2] ?? "";
  if (fracao.length > CASAS) throw erro();
  const v = BigInt(inteiro) * UNIDADE + BigInt((fracao + "00000000").slice(0, CASAS));
  if (v > U64_MAX) throw erro();
  return v;
}

/** Para a tela: "1.234,5" (pt-BR), sem zeros sobrando, com pelo menos 2 casas. */
export function textoDeUnidades(v: bigint, minimoDeCasas = 2): string {
  const negativo = v < 0n;
  const a = negativo ? -v : v;
  const inteiro = (a / UNIDADE).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  let fracao = (a % UNIDADE).toString().padStart(CASAS, "0").replace(/0+$/, "");
  if (fracao.length < minimoDeCasas) fracao = fracao.padEnd(minimoDeCasas, "0");
  return `${negativo ? "−" : ""}${inteiro}${fracao ? "," + fracao : ""}`;
}
