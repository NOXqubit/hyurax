// Endereço Bech32m (BIP-350): o mesmo de `reference/hyurax/endereco.py` e
// do nó, conferido contra `vectors/enderecos.json` (mensagens inclusive).

const ALFABETO = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32M = 0x2bc830a3;
const GERADOR = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];
export const TAMANHO = 20;

export function prefixo(nomeDaRede: string): string {
  const nome = nomeDaRede.startsWith("hyurax-") ? nomeDaRede.slice(7) : nomeDaRede;
  return nome === "mainnet" ? "hyx" : nome === "testnet" ? "thyx" : "rhyx";
}

function polimodo(valores: number[]): number {
  let c = 1;
  for (const v of valores) {
    const topo = c >>> 25;
    c = (((c & 0x1ffffff) << 5) ^ v) >>> 0;
    for (let i = 0; i < 5; i++) if ((topo >>> i) & 1) c = (c ^ GERADOR[i]) >>> 0;
  }
  return c;
}

function expandir(hrp: string): number[] {
  const cs = [...hrp].map((x) => x.charCodeAt(0));
  return [...cs.map((x) => x >> 5), 0, ...cs.map((x) => x & 31)];
}

function reagrupar(dados: ArrayLike<number>, de: number, para: number, completar: boolean): number[] {
  let acumulado = 0;
  let bits = 0;
  const saida: number[] = [];
  const maximo = (1 << para) - 1;
  for (let k = 0; k < dados.length; k++) {
    const v = dados[k];
    if (v >> de) throw new Error("valor fora da faixa");
    acumulado = ((acumulado << de) | v) & 0xffffff;
    bits += de;
    while (bits >= para) {
      bits -= para;
      saida.push((acumulado >> bits) & maximo);
    }
  }
  if (completar) {
    if (bits) saida.push((acumulado << (para - bits)) & maximo);
  } else if (bits >= de || (acumulado << (para - bits)) & maximo) {
    throw new Error("endereço com bits que sobram");
  }
  return saida;
}

export function codificar(hrp: string, carga: Uint8Array): string {
  const dados = reagrupar(carga, 8, 5, true);
  const v = (polimodo([...expandir(hrp), ...dados, 0, 0, 0, 0, 0, 0]) ^ BECH32M) >>> 0;
  const verificador = [0, 1, 2, 3, 4, 5].map((i) => (v >>> (5 * (5 - i))) & 31);
  return hrp + "1" + [...dados, ...verificador].map((d) => ALFABETO[d]).join("");
}

function lerBech32m(texto: string): [string, number[]] {
  let t = texto.trim();
  if (t.length > 90 || t.length < 8) throw new Error("endereço com tamanho errado");
  if (/[a-z]/.test(t) && /[A-Z]/.test(t)) throw new Error("endereço mistura maiúsculas e minúsculas");
  t = t.toLowerCase();
  const sep = t.lastIndexOf("1");
  if (sep < 0) throw new Error("endereço sem o separador 1");
  const hrp = t.slice(0, sep);
  const resto = t.slice(sep + 1);
  if (!hrp || resto.length < 6) throw new Error("endereço incompleto");
  const dados: number[] = [];
  for (const c of resto) {
    const i = ALFABETO.indexOf(c);
    if (i < 0) throw new Error(`caractere que não existe em endereço: ${c}`);
    dados.push(i);
  }
  if (polimodo([...expandir(hrp), ...dados]) !== BECH32M) {
    throw new Error("o dígito verificador não confere: tem erro de digitação no endereço");
  }
  return [hrp, dados.slice(0, -6)];
}

/** Os 20 bytes do endereço, no formato com verificador da rede. */
export function mostrar(endereco: Uint8Array, nomeDaRede: string): string {
  if (endereco.length !== TAMANHO) throw new Error("endereço com tamanho errado");
  return codificar(prefixo(nomeDaRede), endereco);
}

/** Lê um endereço digitado, colado ou lido do QR. Lança com a explicação. */
export function ler(texto: string, nomeDaRede: string): Uint8Array {
  const t = texto.trim();
  if (t.length === 2 * TAMANHO && /^[0-9a-fA-F]+$/.test(t)) {
    const b = new Uint8Array(TAMANHO);
    for (let i = 0; i < TAMANHO; i++) b[i] = parseInt(t.slice(i * 2, i * 2 + 2), 16);
    return b;
  }
  const [hrp, dados] = lerBech32m(t);
  const esperado = prefixo(nomeDaRede);
  if (hrp !== esperado) {
    const deQual =
      hrp === "hyx" ? "da rede principal" : hrp === "thyx" ? "da rede de teste" : hrp === "rhyx" ? "da rede local" : "de outra coisa, não do Hyurax";
    throw new Error(`este endereço é ${deQual}; aqui a rede espera um que comece com ${esperado}1`);
  }
  const b = reagrupar(dados, 5, 8, false);
  if (b.length !== TAMANHO) throw new Error("endereço com tamanho errado");
  return Uint8Array.from(b);
}
