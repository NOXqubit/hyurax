// Codificação canônica (seção 3 da especificação; `crates/hyurax-codec`):
// inteiros big-endian, bytes variáveis com prefixo u32, listas com contagem
// u32. Só a escrita: o celular monta transações, não lê blocos.

import { juntar } from "./bytes.ts";

const U32_MAX = 0xffffffff;
const U64_MAX = (1n << 64n) - 1n;

export class Escritor {
  private partes: Uint8Array[] = [];

  u8(v: number): this {
    if (!Number.isInteger(v) || v < 0 || v > 0xff) throw new Error(`u8 fora da faixa: ${v}`);
    this.partes.push(Uint8Array.of(v));
    return this;
  }

  u16(v: number): this {
    if (!Number.isInteger(v) || v < 0 || v > 0xffff) throw new Error(`u16 fora da faixa: ${v}`);
    this.partes.push(Uint8Array.of(v >> 8, v & 0xff));
    return this;
  }

  u32(v: number): this {
    if (!Number.isInteger(v) || v < 0 || v > U32_MAX) throw new Error(`u32 fora da faixa: ${v}`);
    this.partes.push(Uint8Array.of((v >>> 24) & 0xff, (v >>> 16) & 0xff, (v >>> 8) & 0xff, v & 0xff));
    return this;
  }

  u64(v: bigint): this {
    if (v < 0n || v > U64_MAX) throw new Error(`u64 fora da faixa: ${v}`);
    const b = new Uint8Array(8);
    let x = v;
    for (let i = 7; i >= 0; i--) {
      b[i] = Number(x & 0xffn);
      x >>= 8n;
    }
    this.partes.push(b);
    return this;
  }

  fixo(b: Uint8Array, tamanho: number): this {
    if (b.length !== tamanho) throw new Error(`esperava ${tamanho} bytes, vieram ${b.length}`);
    this.partes.push(b.slice());
    return this;
  }

  variavel(b: Uint8Array): this {
    this.u32(b.length);
    this.partes.push(b.slice());
    return this;
  }

  lista<T>(itens: readonly T[], cada: (e: this, item: T) => void): this {
    this.u32(itens.length);
    for (const i of itens) cada(this, i);
    return this;
  }

  bytes(): Uint8Array {
    return juntar(...this.partes);
  }
}
