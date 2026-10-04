// O arquivo de carteira `HYURAX-CARTEIRA-v2`, o mesmo do programa do PC
// (`crates/hyurax-nucleo/src/carteira/arquivo.rs`): Argon2id sobre a senha,
// ChaCha20-Poly1305 sobre o segredo, `formato || endereço` como dado
// associado. Conferido contra `mobile/teste/dados/carteira-v2.txt`, que o
// Rust gera e confere também. Assim a mesma carteira abre no PC e no celular.

import { chacha20poly1305 } from "@noble/ciphers/chacha.js";
import { argon2idAsync } from "@noble/hashes/argon2.js";

import { deHex, hex, juntar, texto } from "./bytes.ts";
import { enderecoDoSegredo } from "./transacao.ts";

export const FORMATO = "HYURAX-CARTEIRA-v2";
export const SENHA_MINIMA = 10;
const MEMORIA_KIB = 64 * 1024;
const PASSADAS = 3;
const FAIXAS = 1;
/** Teto ao abrir: um arquivo adulterado não faz o celular reservar gigabytes. */
const MEMORIA_MAXIMA_KIB = 256 * 1024;

function campo(t: string, nome: string): string | undefined {
  for (const linha of t.split(/\r?\n/)) {
    const l = linha.trim();
    if (l.startsWith(nome + "=")) return l.slice(nome.length + 1);
  }
  return undefined;
}

function fixo(t: string, nome: string, tamanho: number): Uint8Array {
  const v = campo(t, nome);
  const b = v === undefined ? null : deHex(v);
  if (!b || b.length !== tamanho) throw new Error(`${nome} inválido`);
  return b;
}

export function senhaAceitavel(senha: string): void {
  if ([...senha].length < SENHA_MINIMA) throw new Error(`a senha precisa de pelo menos ${SENHA_MINIMA} caracteres`);
}

type Progresso = (fracao: number) => void;

async function derivar(senha: string, sal: Uint8Array, m: number, t: number, p: number, aoAvancar?: Progresso): Promise<Uint8Array> {
  if (m > MEMORIA_MAXIMA_KIB) throw new Error(`arquivo pede ${m} KiB de memória; recusado`);
  // asyncTick: devolve a vez à tela a cada ~10 ms, para ela não congelar
  return argon2idAsync(texto(senha), sal, { t, m, p, dkLen: 32, asyncTick: 10, onProgress: aoAvancar });
}

/** O endereço declarado no arquivo, sem pedir senha. */
export function enderecoDoArquivo(t: string): Uint8Array {
  if (campo(t, "segredo") !== undefined && campo(t, "formato") === undefined) {
    return enderecoDoSegredo(fixo(t, "segredo", 32));
  }
  return fixo(t, "endereco", 20);
}

/** Abre a carteira e devolve o segredo. Senha errada e arquivo alterado dão a mesma mensagem. */
export async function abrir(t: string, senha: string, aoAvancar?: Progresso): Promise<Uint8Array> {
  if (campo(t, "segredo") !== undefined && campo(t, "formato") === undefined) return fixo(t, "segredo", 32);
  const formato = campo(t, "formato");
  if (formato !== FORMATO) throw new Error("formato de carteira desconhecido");
  if (campo(t, "kdf") !== "argon2id") throw new Error("derivação de chave desconhecida");
  const numero = (nome: string): number => {
    const v = Number(campo(t, nome));
    if (!Number.isInteger(v) || v <= 0) throw new Error(`campo ${nome} inválido`);
    return v;
  };
  const endereco = enderecoDoArquivo(t);
  const chave = await derivar(senha, fixo(t, "sal", 16), numero("memoria_kib"), numero("passadas"), numero("faixas"), aoAvancar);
  const cifra = chacha20poly1305(chave, fixo(t, "nonce", 12), juntar(texto(formato), endereco));
  let segredo: Uint8Array;
  try {
    segredo = cifra.decrypt(juntar(fixo(t, "cifrado", 32), fixo(t, "etiqueta", 16)));
  } catch {
    throw new Error("senha errada ou arquivo alterado");
  }
  if (hex(enderecoDoSegredo(segredo)) !== hex(endereco)) throw new Error("senha errada ou arquivo alterado");
  return segredo;
}

/**
 * O texto do arquivo, igual ao do PC byte a byte. `aleatorio` traz 28 bytes
 * imprevisíveis (16 de sal e 12 de nonce), vindos do sorteio do sistema.
 */
export async function cifrar(segredo: Uint8Array, senha: string, aleatorio: Uint8Array, aoAvancar?: Progresso): Promise<string> {
  senhaAceitavel(senha);
  if (segredo.length !== 32 || aleatorio.length !== 28) throw new Error("tamanhos errados");
  const sal = aleatorio.slice(0, 16);
  const nonce = aleatorio.slice(16, 28);
  const endereco = enderecoDoSegredo(segredo);
  const chave = await derivar(senha, sal, MEMORIA_KIB, PASSADAS, FAIXAS, aoAvancar);
  const selado = chacha20poly1305(chave, nonce, juntar(texto(FORMATO), endereco)).encrypt(segredo);
  return (
    "# Carteira Hyurax de TESTE, cifrada com senha.\n" +
    "# Sem a senha, este arquivo não gasta nada. Sem este arquivo E a senha,\n" +
    "# o saldo fica perdido: guarde uma cópia e não esqueça a senha.\n" +
    "# A rede pública não existe e o HYX não tem valor.\n" +
    `formato=${FORMATO}\n` +
    `endereco=${hex(endereco)}\n` +
    "kdf=argon2id\n" +
    `memoria_kib=${MEMORIA_KIB}\n` +
    `passadas=${PASSADAS}\n` +
    `faixas=${FAIXAS}\n` +
    `sal=${hex(sal)}\n` +
    `nonce=${hex(nonce)}\n` +
    `cifrado=${hex(selado.slice(0, 32))}\n` +
    `etiqueta=${hex(selado.slice(32))}\n`
  );
}
