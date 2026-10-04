/// <reference types="node" />
// O núcleo do app contra os mesmos vetores do Python e do Rust:
// `node --test teste/` (Node 22+, tipos removidos na hora).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { deHex, hex } from "../src/nucleo/bytes.ts";
import * as carteira from "../src/nucleo/carteira.ts";
import * as endereco from "../src/nucleo/endereco.ts";
import { normalizarEndereco } from "../src/nucleo/no.ts";
import * as tx from "../src/nucleo/transacao.ts";
import { textoDeUnidades, unidadesDeTexto } from "../src/nucleo/valor.ts";

const AQUI = dirname(fileURLToPath(import.meta.url));
const VETORES = join(AQUI, "..", "..", "vectors");
const vetor = (nome: string) => JSON.parse(readFileSync(join(VETORES, nome), "utf-8")).data;
const b = (t: string): Uint8Array => {
  const x = deHex(t);
  if (!x) throw new Error(`hex inválido: ${t}`);
  return x;
};

test("endereços batem com vectors/enderecos.json, mensagens inclusive", () => {
  const d = vetor("enderecos.json");
  for (const v of d.validos) {
    for (const rede of ["mainnet", "testnet", "regtest"]) {
      assert.equal(endereco.mostrar(b(v.endereco), `hyurax-${rede}`), v[rede]);
      assert.equal(hex(endereco.ler(v[rede], `hyurax-${rede}`)), v.endereco);
    }
  }
  for (const a of d.aceitos) assert.equal(hex(endereco.ler(a.texto, a.rede)), a.endereco, a.texto);
  assert.ok(d.recusas.length >= 10);
  for (const r of d.recusas) assert.throws(() => endereco.ler(r.texto, r.rede), { message: r.erro }, r.texto);
});

test("chave pública e endereço batem com vectors/crypto_ed25519.json", () => {
  for (const c of vetor("crypto_ed25519.json")) {
    const segredo = b(c.secret);
    assert.equal(hex(tx.chavePublica(segredo)), c.public_key);
    assert.equal(hex(tx.enderecoDoSegredo(segredo)), c.address);
  }
});

test("transferências assinadas aqui são idênticas às de vectors/transactions.json", () => {
  const segredos = new Map<string, Uint8Array>();
  for (const c of vetor("crypto_ed25519.json")) segredos.set(c.address, b(c.secret));
  const transfers = vetor("transactions.json").transfers;
  assert.ok(transfers.length >= 1);
  for (const t of transfers) {
    const segredo = segredos.get(t.sender);
    assert.ok(segredo, `sem o segredo de ${t.sender}`);
    // o magic está dentro da mensagem assinada, logo depois do domínio
    const payload = b(t.signing_payload);
    const magic = payload.slice(tx.SIGNING_DOMAIN.length, tx.SIGNING_DOMAIN.length + 4);
    const saidas = t.outputs.map((o: any) => ({ destino: b(o.recipient), ativo: b(o.asset_id), valor: BigInt(o.amount) }));
    const assinada = tx.assinar(segredo!, magic, saidas, BigInt(t.fee), BigInt(t.nonce));
    assert.equal(hex(tx.mensagemAssinada(assinada, magic)), t.signing_payload);
    assert.equal(hex(assinada.assinatura), t.signature);
    assert.equal(hex(tx.codificar(assinada)), t.encoded);
    assert.equal(hex(tx.txid(assinada)), t.txid);
  }
});

test("o app recusa antes de assinar o que o consenso recusaria", () => {
  const segredo = new Uint8Array(32).fill(0x11);
  const eu = tx.enderecoDoSegredo(segredo);
  const outro = new Uint8Array(20).fill(3);
  const magic = new Uint8Array([0x48, 0x59, 0x58, 0x52]);
  const s = (destino: Uint8Array, valor: bigint) => ({ destino, ativo: tx.HYX, valor });
  assert.throws(() => tx.assinar(segredo, magic, [s(outro, 0n)], 0n, 0n), /positivo/);
  assert.throws(() => tx.assinar(segredo, magic, [s(eu, 1n)], 0n, 0n), /iguais/);
  assert.throws(() => tx.assinar(segredo, magic, [], 0n, 0n), /saídas/);
  assert.throws(() => tx.assinar(segredo, magic, [s(outro, 1n), s(outro, 2n)], 0n, 0n), /repetidas/);
  assert.throws(() => tx.assinar(segredo, magic, [s(outro, (1n << 64n) - 1n)], 1n, 0n), /estoura/);
  // fora de ordem entra em ordem
  const a = new Uint8Array(20).fill(9);
  const t = tx.assinar(segredo, magic, [s(a, 5n), s(outro, 7n)], 0n, 4n);
  assert.deepEqual(t.saidas.map((x) => x.valor), [7n, 5n]);
});

test("a carteira do PC abre aqui, e a daqui sai igual à do PC", async () => {
  const gabarito = readFileSync(join(AQUI, "dados", "carteira-v2.txt"), "utf-8").replace(/\r\n/g, "\n");
  const segredo = Uint8Array.from({ length: 32 }, (_, i) => i);
  const aleatorio = Uint8Array.from({ length: 28 }, (_, i) => (i * 7 + 3) & 0xff);
  assert.equal(hex(carteira.enderecoDoArquivo(gabarito)), hex(tx.enderecoDoSegredo(segredo)));
  assert.deepEqual(await carteira.abrir(gabarito, "senha do gabarito 2026"), segredo);
  await assert.rejects(carteira.abrir(gabarito, "senha errada 123"), /senha errada ou arquivo alterado/);
  const adulterado = gabarito.replace(/endereco=.*/, `endereco=${"00".repeat(20)}`);
  await assert.rejects(carteira.abrir(adulterado, "senha do gabarito 2026"), /senha errada ou arquivo alterado/);
  assert.equal(await carteira.cifrar(segredo, "senha do gabarito 2026", aleatorio), gabarito);
  await assert.rejects(carteira.cifrar(segredo, "curta", aleatorio), /pelo menos 10/);
  const guloso = gabarito.replace("memoria_kib=65536", "memoria_kib=4194304");
  await assert.rejects(carteira.abrir(guloso, "senha do gabarito 2026"), /recusado/);
});

test("valores: texto do usuário para unidades e de volta", () => {
  assert.equal(unidadesDeTexto("1,5"), 150_000_000n);
  assert.equal(unidadesDeTexto("1.5"), 150_000_000n);
  assert.equal(unidadesDeTexto(" 0,00000001 "), 1n);
  assert.equal(unidadesDeTexto(",5"), 50_000_000n);
  assert.equal(unidadesDeTexto("184467440737,09551615"), (1n << 64n) - 1n);
  for (const ruim of ["", "abc", "1,2,3", "0,123456789", "-1", "184467440737,09551616", ","]) {
    assert.throws(() => unidadesDeTexto(ruim), /valor inválido/, ruim);
  }
  assert.equal(textoDeUnidades(123_456_789_000n), "1.234,56789");
  assert.equal(textoDeUnidades(100_000_000n), "1,00");
  assert.equal(textoDeUnidades(1n), "0,00000001");
  assert.equal(textoDeUnidades(0n), "0,00");
});

test("endereço do nó", () => {
  assert.equal(normalizarEndereco("192.168.0.10"), "http://192.168.0.10:8800");
  assert.equal(normalizarEndereco("192.168.0.10:9000/"), "http://192.168.0.10:9000");
  assert.equal(normalizarEndereco("https://no.exemplo.org"), "https://no.exemplo.org");
  assert.throws(() => normalizarEndereco("  "), /endereço do nó/);
});
