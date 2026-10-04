// Conversa com um nó Hyurax pela API das carteiras leves (`/api/v1/leve/`,
// ver `crates/hyurax-nucleo/src/api/leve.rs`). O nó só lê a cadeia e repassa
// a transação já assinada aqui.

import { deHex, hex } from "./bytes.ts";
import { codificar, type Transferencia } from "./transacao.ts";

export type Info = {
  rede: string;
  prefixo: string;
  magic: Uint8Array;
  altura: number;
  pares: number;
  maturidade: number;
  versao: string;
  taxaSugerida: bigint;
};

export type Conta = { saldo: bigint; imaturo: bigint; proximoNonce: bigint; altura: number };

export type Movimento = {
  quando: number;
  altura: number;
  pendente: boolean;
  entrada: boolean;
  outro: string;
  valor: bigint;
  taxa: bigint;
  txid: string;
  tipo: "recompensa" | "transferencia";
};

const PRAZO_MS = 12_000;

/** "192.168.0.10:8800", "http://…" ou "https://…" viram uma base de URL. */
export function normalizarEndereco(entrada: string): string {
  let t = entrada.trim().replace(/\/+$/, "");
  if (!t) throw new Error("informe o endereço do nó");
  if (!/^https?:\/\//i.test(t)) t = `http://${t}`;
  const u = new URL(t);
  if (!u.port && u.protocol === "http:") u.port = "8800";
  return `${u.protocol}//${u.host}`;
}

async function pedir(base: string, caminho: string, init?: RequestInit): Promise<any> {
  const controle = new AbortController();
  const relogio = setTimeout(() => controle.abort(), PRAZO_MS);
  let r: Response;
  try {
    r = await fetch(`${base}/api/v1/leve/${caminho}`, { ...init, signal: controle.signal });
  } catch (e: any) {
    throw new Error(e?.name === "AbortError" ? "o nó não respondeu a tempo" : "não consegui falar com o nó (endereço certo? mesmo Wi-Fi?)");
  } finally {
    clearTimeout(relogio);
  }
  const corpo = await r.json().catch(() => null);
  if (r.status === 403) throw new Error(corpo?.erro || "este nó não está servindo carteiras de celular (Ajustes → Carteiras de celular, no PC)");
  if (!r.ok) throw new Error(corpo?.erro || `o nó respondeu ${r.status}`);
  return corpo;
}

export async function info(base: string): Promise<Info> {
  const j = await pedir(base, "info");
  const magic = deHex(String(j.magic));
  if (!magic || magic.length !== 4) throw new Error("o nó mandou um magic de rede inválido");
  return {
    rede: j.rede,
    prefixo: j.prefixo,
    magic,
    altura: j.altura,
    pares: j.pares,
    maturidade: j.maturidade,
    versao: j.versao,
    taxaSugerida: BigInt(j.taxa_sugerida_unidades ?? 0),
  };
}

export async function conta(base: string, endereco: string): Promise<Conta> {
  const j = await pedir(base, `conta/${encodeURIComponent(endereco)}`);
  return { saldo: BigInt(j.saldo_unidades), imaturo: BigInt(j.imaturo_unidades), proximoNonce: BigInt(j.proximo_nonce), altura: j.altura };
}

export async function historico(base: string, endereco: string): Promise<Movimento[]> {
  const j = await pedir(base, `historico/${encodeURIComponent(endereco)}`);
  return (j.movimentos ?? []).map((m: any) => ({
    quando: m.quando,
    altura: m.altura,
    pendente: m.pendente,
    entrada: m.entrada,
    outro: m.outro,
    valor: BigInt(m.valor_unidades),
    taxa: BigInt(m.taxa_unidades),
    txid: m.txid,
    tipo: m.tipo,
  }));
}

/** Entrega a transação assinada. Devolve o txid que o nó calculou. */
export async function enviar(base: string, t: Transferencia): Promise<string> {
  const j = await pedir(base, "transacao", { method: "POST", headers: { "Content-Type": "text/plain" }, body: hex(codificar(t)) });
  return j.txid;
}
