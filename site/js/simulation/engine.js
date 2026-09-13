// SimulationEngine: toda a lógica das demonstrações, separada do visual.
// Tudo o que sai daqui leva `simulation: true`. Onde o navegador permite, a conta é
// real (SHA-512, AES-GCM, Freivalds); os dados em si são inventados para demonstração.

export const SIMULATION = true;

const LETRAS = "ABCDEFGHIJKLMNOP";
const sutil = globalThis.crypto && globalThis.crypto.subtle ? globalThis.crypto.subtle : null;
export const temCripto = !!sutil;

export function hex(bytes, max) {
  const b = max ? bytes.subarray(0, max) : bytes;
  let s = "";
  for (let i = 0; i < b.length; i++) s += b[i].toString(16).padStart(2, "0");
  return s;
}
export const curto = (h, n = 8) => (h.length > n * 2 + 1 ? h.slice(0, n) + "…" + h.slice(-n) : h);

export async function sha512(bytes) {
  if (!sutil) return new Uint8Array(64);
  return new Uint8Array(await sutil.digest("SHA-512", bytes));
}

// Escrita canônica: inteiros big-endian e bytes variáveis com prefixo de tamanho u32,
// como no auron-codec.
export class Escritor {
  constructor() { this.partes = []; this.total = 0; }
  u8(v) { return this.#pedaco(Uint8Array.of(v & 0xff)); }
  u16(v) { const b = new Uint8Array(2); new DataView(b.buffer).setUint16(0, v); return this.#pedaco(b); }
  u32(v) { const b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, v); return this.#pedaco(b); }
  u64(v) { const b = new Uint8Array(8); new DataView(b.buffer).setBigUint64(0, BigInt(v)); return this.#pedaco(b); }
  fixo(bytes) { return this.#pedaco(bytes); }
  variavel(bytes) { this.u32(bytes.length); return this.#pedaco(bytes); }
  bytes() { const s = new Uint8Array(this.total); let o = 0; for (const p of this.partes) { s.set(p, o); o += p.length; } return s; }
  #pedaco(b) { this.partes.push(b); this.total += b.length; return this; }
}

// Merkle no padrão RFC 6962, com SHA-512: folha = H(0x00 || dado), nó = H(0x01 || e || d).
export async function merkle(folhas) {
  if (folhas.length === 0) return sha512(new Uint8Array(0));
  const junta = (pref, ...bs) => { const e = new Escritor().u8(pref); bs.forEach((b) => e.fixo(b)); return e.bytes(); };
  async function raiz(lista) {
    if (lista.length === 1) return sha512(junta(0, lista[0]));
    let k = 1; while (k * 2 < lista.length) k *= 2;
    return sha512(junta(1, await raiz(lista.slice(0, k)), await raiz(lista.slice(k))));
  }
  return raiz(folhas);
}

function mulberry32(semente) {
  let a = semente >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class SimulationEngine {
  constructor(semente = 2026) {
    this.aleat = mulberry32(semente);
    this.contasSim = Array.from({ length: 9 }, () => this.bytes(20));
    this.nonces = new Map();
  }
  int(n) { return Math.floor(this.aleat() * n); }
  bytes(n) { const b = new Uint8Array(n); for (let i = 0; i < n; i++) b[i] = this.int(256); return b; }
  escolhe(lista) { return lista[this.int(lista.length)]; }

  // ---------- nós ----------
  createNode(i) {
    const id = LETRAS[i % 16] + String(1 + ((i * 7) % 99)).padStart(2, "0");
    return { simulation: true, i, id, online: true, ocupado: this.aleat() < 0.3, altura: 0, vizinhos: [] };
  }
  connectNodes(nos, pos, k = 3) {
    nos.forEach((a, i) => {
      const perto = nos.map((_, j) => ({ j, d: (pos[i].x - pos[j].x) ** 2 + (pos[i].y - pos[j].y) ** 2 }))
        .filter((o) => o.j !== i).sort((p, q) => p.d - q.d).slice(0, k);
      perto.forEach(({ j }) => { if (!a.vizinhos.includes(j)) a.vizinhos.push(j); if (!nos[j].vizinhos.includes(i)) nos[j].vizinhos.push(i); });
    });
    return nos;
  }

  // ---------- transações e blocos ----------
  createTransaction() {
    const de = this.int(this.contasSim.length);
    let para = this.int(this.contasSim.length - 1); if (para >= de) para++;
    const nonce = (this.nonces.get(de) || 0) + 1; this.nonces.set(de, nonce);
    const valor = (1 + this.int(4000)) * 250000;          // unidades de 10^-8 AUR
    const taxa = (1 + this.int(20)) * 1000;
    const assinatura = this.bytes(64);
    const corpo = new Escritor().u16(2).u8(1).fixo(this.contasSim[de]).u64(nonce).u64(taxa)
      .u32(1).fixo(this.contasSim[para]).fixo(new Uint8Array(32)).u64(valor).variavel(assinatura).bytes();
    return {
      simulation: true, ativo: "AUR", valor, taxa, nonce,
      remetente: hex(this.contasSim[de]), destinatario: hex(this.contasSim[para]),
      assinatura: hex(assinatura), bytes: corpo, id: null, bloco: null,
    };
  }
  async idDaTransacao(tx) { tx.id = hex(await sha512(tx.bytes), 32); return tx.id; }

  async mineBlock(anterior, txs) {
    const altura = anterior ? anterior.altura + 1 : 0;
    const prev = anterior ? anterior.hashBytes : new Uint8Array(64);
    const raiz = await merkle(txs.map((t) => t.bytes));
    const horario = Math.floor(Date.now() / 1000);
    const bits = 0x1f00ffff;
    const nonce = this.int(2 ** 31);
    // Cabeçalho versão 2, de 222 bytes, como no §7 da AURON-SPEC-01. A prova de
    // trabalho útil é simulada; o compromisso dela (useful_root) é SHA-512 real.
    const prova = new TextEncoder().encode(`AURON-UPOW-PROOF-v1 simulada ${altura} ${nonce}`);
    const utilRaiz = await sha512(prova);
    const cab = new Escritor().u16(2).u64(altura).fixo(prev).fixo(raiz).fixo(utilRaiz).u64(horario).u32(bits).u64(nonce).bytes();
    const h = await sha512(cab);
    return {
      simulation: true, altura, hash: hex(h), hashBytes: h, anterior: hex(prev), merkle: hex(raiz),
      horario, bits, nonce, tamanhoCabecalho: cab.length, txs,
    };
  }

  // ---------- UTRAX: Freivalds com desafio tirado do próprio resultado ----------
  matriz(n, max = 9) { return Array.from({ length: n }, () => Array.from({ length: n }, () => this.int(max + 1))); }
  static multiplica(A, B) { return A.map((l) => B[0].map((_, j) => l.reduce((s, x, k) => s + x * B[k][j], 0))); }
  static vezes(M, v) { return M.map((l) => l.reduce((s, x, j) => s + x * v[j], 0)); }
  async verifyTask(A, B, C, rodadas = 3) {
    const n = A.length;
    const bytesC = new Escritor(); C.flat().forEach((x) => bytesC.u32(x));
    const semente = await sha512(bytesC.bytes());
    for (let k = 0; k < rodadas; k++) {
      const r = Array.from({ length: n }, (_, i) => 1 + (semente[(k * n + i) % 64] % 97));
      const esq = SimulationEngine.vezes(A, SimulationEngine.vezes(B, r));
      const dir = SimulationEngine.vezes(C, r);
      if (esq.some((x, i) => x !== dir[i])) return { ok: false, rodada: k + 1, custo: 3 * n * n * (k + 1), refazer: n * n * n, semente: hex(semente) };
    }
    return { ok: true, rodada: rodadas, custo: 3 * n * n * rodadas, refazer: n * n * n, semente: hex(semente) };
  }

  // ---------- fragmentação 16×16 ----------
  async fragmentData(texto) {
    if (!sutil) throw new Error("sem-cripto");
    const util = new TextEncoder().encode(texto);
    const claro = new Uint8Array(4080);
    new DataView(claro.buffer).setUint16(0, util.length);        // prefixo de tamanho
    claro.set(util, 2);
    crypto.getRandomValues(claro.subarray(2 + util.length));       // enchimento aleatório
    const chave = await sutil.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const cifrado = new Uint8Array(await sutil.encrypt({ name: "AES-GCM", iv }, chave, claro)); // 4096 bytes
    const fragmentos = [];
    for (let i = 0; i < 256; i++) {
      const pedaco = cifrado.slice(i * 16, i * 16 + 16);
      fragmentos.push({
        simulation: true, i, id: LETRAS[i % 16] + String(Math.floor(i / 16) + 1).padStart(2, "0"),
        bytes: pedaco, hash: hex(await sha512(pedaco)), no: this.int(64), estado: "armazenado",
      });
    }
    return { chave, iv, fragmentos, tamanho: cifrado.length };
  }
  async reconstructData(pacote) {
    const junto = new Uint8Array(4096);
    for (const f of pacote.fragmentos) {
      if (hex(await sha512(f.bytes)) !== f.hash) return { ok: false, falha: f };
      junto.set(f.bytes, f.i * 16);
    }
    try {
      const claro = new Uint8Array(await sutil.decrypt({ name: "AES-GCM", iv: pacote.iv }, pacote.chave, junto));
      const n = new DataView(claro.buffer).getUint16(0);
      return { ok: true, texto: new TextDecoder().decode(claro.subarray(2, 2 + n)) };
    } catch { return { ok: false, falha: null }; }
  }

  // ---------- mesh ----------
  routeMessage(grafo, de, para, fora = new Set()) {
    const dist = new Map([[de, 0]]), veio = new Map(), aberto = new Set([de]);
    while (aberto.size) {
      let u = null; for (const x of aberto) if (u === null || dist.get(x) < dist.get(u)) u = x;
      aberto.delete(u);
      if (u === para) break;
      for (const ar of grafo.arestas) {
        const v = ar.a === u ? ar.b : ar.b === u ? ar.a : null;
        if (v === null || fora.has(v)) continue;
        const nd = dist.get(u) + ar.peso;
        if (!dist.has(v) || nd < dist.get(v)) { dist.set(v, nd); veio.set(v, u); aberto.add(v); }
      }
    }
    if (!dist.has(para)) return null;
    const caminho = [para]; while (caminho[0] !== de) caminho.unshift(veio.get(caminho[0]));
    return caminho;
  }

  async sendPacket({ remetente, destinatario, conteudo }) {
    const agora = Math.floor(Date.now() / 1000);
    const payload = this.bytes(Math.max(24, conteudo.length + 16));
    const payloadHash = await sha512(payload);
    const cab = new Escritor().u16(1).u8(2).variavel(new TextEncoder().encode(remetente)).variavel(new TextEncoder().encode(destinatario))
      .fixo(this.bytes(16)).u64(agora).u64(agora + 86400).u8(2).fixo(payloadHash).u16(0).bytes();
    const id = await sha512(new Escritor().variavel(cab).variavel(payload).bytes());
    return {
      simulation: true, versao: 1, tipo: "MESSAGE", id: hex(id), remetente, destinatario,
      sessao: hex(this.bytes(16)), horario: agora, expiracao: agora + 86400, prioridade: "P2 normal",
      payloadHash: hex(payloadHash), flags: "0x0000", assinatura: hex(this.bytes(64)), payload: hex(payload),
    };
  }

  simulatePayment(modo) {
    const voucher = hex(this.bytes(16));
    return { simulation: true, modo, voucher, contador: 18492, assinatura: hex(this.bytes(64)) };
  }
}
