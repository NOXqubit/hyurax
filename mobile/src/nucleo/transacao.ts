// Transferência assinada no aparelho (seção 5 da especificação;
// `crates/hyurax-tx`). O nó só recebe os bytes prontos: o segredo não sai
// do celular. Conferido contra `vectors/transactions.json`.

import { ed25519 } from "@noble/curves/ed25519.js";
import { sha512 } from "@noble/hashes/sha2.js";

import { comparar, juntar, texto } from "./bytes.ts";
import { Escritor } from "./codec.ts";
import { TAMANHO } from "./endereco.ts";

export const KIND_TRANSFER = 1;
export const TRANSFER_VERSION = 2;
export const SIG_CODE_ED25519 = 1;
export const SIGNING_DOMAIN = texto("HYURAX-TX-v2");
export const SIG_ED25519_V1 = texto("SIG-ED25519-V1");
/** O ativo HYX: 32 bytes zero. */
export const HYX = new Uint8Array(32);
/** 1 HYX = 10^8 unidades. */
export const UNIDADE = 100_000_000n;
export const MAX_SAIDAS = 16;

export type Saida = { destino: Uint8Array; ativo: Uint8Array; valor: bigint };

export type Transferencia = {
  remetente: Uint8Array;
  saidas: Saida[];
  taxa: bigint;
  nonce: bigint;
  chavePublica: Uint8Array;
  assinatura: Uint8Array;
};

export function chavePublica(segredo: Uint8Array): Uint8Array {
  if (segredo.length !== 32) throw new Error("segredo de tamanho errado");
  return ed25519.getPublicKey(segredo);
}

/** Endereço = os 20 primeiros bytes de SHA-512("SIG-ED25519-V1" || chave pública). */
export function enderecoDaChave(publica: Uint8Array): Uint8Array {
  return sha512(juntar(SIG_ED25519_V1, publica)).slice(0, TAMANHO);
}

export const enderecoDoSegredo = (segredo: Uint8Array): Uint8Array => enderecoDaChave(chavePublica(segredo));

function corpo(t: Omit<Transferencia, "assinatura">): Escritor {
  return new Escritor()
    .u8(KIND_TRANSFER)
    .u16(TRANSFER_VERSION)
    .u8(SIG_CODE_ED25519)
    .fixo(t.remetente, TAMANHO)
    .u64(t.taxa)
    .u64(t.nonce)
    .lista(t.saidas, (e, s) => {
      e.fixo(s.destino, TAMANHO).fixo(s.ativo, 32).u64(s.valor);
    })
    .variavel(t.chavePublica);
}

/** O que é assinado: domínio || magic da rede || corpo. */
export function mensagemAssinada(t: Omit<Transferencia, "assinatura">, magic: Uint8Array): Uint8Array {
  if (magic.length !== 4) throw new Error("magic da rede tem 4 bytes");
  return juntar(SIGNING_DOMAIN, magic, corpo(t).bytes());
}

export function codificar(t: Transferencia): Uint8Array {
  return corpo(t).variavel(t.assinatura).bytes();
}

export const txid = (t: Transferencia): Uint8Array => sha512(codificar(t));

/**
 * Monta e assina. As saídas vão em ordem estrita de (destino, ativo), como o
 * consenso exige; valor zero, destino igual ao remetente e saídas repetidas
 * são recusados aqui, antes de gastar uma ida ao nó.
 */
export function assinar(segredo: Uint8Array, magic: Uint8Array, saidas: Saida[], taxa: bigint, nonce: bigint): Transferencia {
  const publica = chavePublica(segredo);
  const remetente = enderecoDaChave(publica);
  if (saidas.length === 0 || saidas.length > MAX_SAIDAS) throw new Error(`de 1 a ${MAX_SAIDAS} saídas`);
  const ordenadas = [...saidas].sort((a, b) => comparar(a.destino, b.destino) || comparar(a.ativo, b.ativo));
  for (let i = 0; i < ordenadas.length; i++) {
    const s = ordenadas[i];
    if (s.valor <= 0n) throw new Error("valor deve ser positivo");
    if (comparar(s.destino, remetente) === 0) throw new Error("origem e destino iguais");
    if (i > 0 && comparar(ordenadas[i - 1].destino, s.destino) === 0 && comparar(ordenadas[i - 1].ativo, s.ativo) === 0) {
      throw new Error("saídas repetidas para o mesmo destino e ativo");
    }
  }
  const total = ordenadas.reduce((n, s) => n + s.valor, taxa);
  if (total > (1n << 64n) - 1n) throw new Error("valor mais taxa estoura a faixa");
  const semAssinatura = { remetente, saidas: ordenadas, taxa, nonce, chavePublica: publica };
  const assinatura = ed25519.sign(mensagemAssinada(semAssinatura, magic), segredo);
  return { ...semAssinatura, assinatura };
}
