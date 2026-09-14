// Capítulo 04 — Mexa num bloco antigo.
//
// Uma cadeia de blocos de verdade no navegador: cada cabeçalho tem os 222 bytes
// da AURON-SPEC-01 (§7) e o hash é SHA-512 calculado aqui. A régua mostra os
// campos no tamanho real. Mexer numa transação de um bloco antigo recalcula a
// raiz de Merkle e o hash dele: o bloco seguinte guardava o hash antigo, então o
// elo se solta e a cadeia apaga em cascata dali para frente.

import { el } from "../ui.js";
import { merkle, sha512, hex, curto } from "../simulation/engine.js";

// Campos do cabeçalho v2, na ordem da especificação (tamanho em bytes).
export const CAMPOS = [
  ["versao", 2], ["altura", 8], ["anterior", 64], ["merkle", 64], ["util", 64], ["horario", 8], ["bits", 4], ["nonce", 8],
];
const MAX_BLOCOS = 6;
const OFFSET_MERKLE = 2 + 8 + 64;

export function iniciar({ t, engine, calmo, aoMudarIdioma }) {
  const trilho = document.getElementById("cadeia-trilho");
  const regua = document.getElementById("cadeia-regua");
  const leitura = document.getElementById("cadeia-leitura");
  const status = document.getElementById("cadeia-status");
  const botaoNovo = document.getElementById("cadeia-novo");
  const botaoRestaurar = document.getElementById("cadeia-restaurar");
  const secao = document.getElementById("cadeia");
  if (!trilho) return;

  const blocos = [];
  let selecionado = 0, ocupado = false;

  async function minerar() {
    const anterior = blocos[blocos.length - 1] || null;
    const txs = [];
    for (let i = 0; i < 2 + engine.int(3); i++) { const tx = engine.createTransaction(); await engine.idDaTransacao(tx); txs.push(tx); }
    const b = await engine.mineBlock(anterior, txs);
    b.hashOriginal = b.hash;
    b.adulterado = false;
    blocos.push(b);
    if (blocos.length > MAX_BLOCOS) blocos.shift();
    return b;
  }

  /** Um elo vale se o bloco guarda exatamente o hash atual do anterior. */
  function quebradoDesde() {
    for (let i = 1; i < blocos.length; i++) if (blocos[i].anterior !== blocos[i - 1].hash) return i;
    return -1;
  }

  function valorDoCampo(b, campo) {
    const off = CAMPOS.slice(0, CAMPOS.findIndex(([c]) => c === campo)).reduce((s, [, n]) => s + n, 0);
    const n = CAMPOS.find(([c]) => c === campo)[1];
    return hex(b.cabecalho.slice(off, off + n));
  }

  function desenharRegua() {
    const b = blocos[selecionado];
    if (!b) return;
    regua.replaceChildren(...CAMPOS.map(([campo, n]) => el("button", {
      type: "button", class: `campo campo-${campo}`, style: `--n:${n}`,
      "aria-label": `${t(`cadeia4.campos.${campo}`)}: ${n} bytes`,
      onmouseenter: () => ler(campo), onfocus: () => ler(campo), onclick: () => ler(campo),
    }, el("span", { class: "nome", text: t(`cadeia4.campos.${campo}`) }), el("span", { class: "bytes", text: String(n) }))));
    ler("anterior");
  }

  function ler(campo) {
    const b = blocos[selecionado];
    if (!b) return;
    regua.querySelectorAll(".campo").forEach((x) => x.classList.toggle("ativo", x.classList.contains(`campo-${campo}`)));
    const v = valorDoCampo(b, campo);
    leitura.replaceChildren(
      el("span", { class: "rotulo", text: `${t(`cadeia4.campos.${campo}`)} · ${CAMPOS.find(([c]) => c === campo)[1]} bytes` }),
      el("span", { class: "valor", text: v.length > 40 ? curto(v, 16) : v }),
    );
  }

  function desenhar() {
    const quebra = quebradoDesde();
    secao.classList.toggle("quebrada", quebra >= 0);
    trilho.replaceChildren(...blocos.flatMap((b, i) => {
      const apagado = quebra >= 0 && i >= quebra;
      const partes = [];
      if (i > 0) partes.push(el("span", { class: `elo${apagado && i === quebra ? " solto" : apagado ? " apagado" : ""}`, "aria-hidden": "true" }));
      partes.push(el("article", {
        class: `bloco${apagado ? " apagado" : ""}${b.adulterado ? " adulterado" : ""}${i === selecionado ? " selecionado" : ""}`,
        style: `--atraso:${apagado ? (i - quebra) * 120 : 0}ms`,
      },
      el("button", { type: "button", class: "abre", onclick: () => { selecionado = i; desenhar(); desenharRegua(); }, "aria-label": t("cadeia4.ver", { h: b.altura }) },
        el("span", { class: "altura", text: `#${b.altura}` }),
        el("span", { class: "linha" }, el("small", { text: t("cadeia4.campos.anterior") }), el("b", { text: curto(b.anterior, 5) })),
        el("span", { class: "linha" }, el("small", { text: "hash" }), el("b", { text: curto(b.hash, 5) })),
        el("span", { class: "linha" }, el("small", { text: "tx" }), el("b", { text: String(b.txs.length) }))),
      i < blocos.length - 1 && !b.adulterado
        ? el("button", { type: "button", class: "mexer", onclick: () => adulterar(i), text: t("cadeia4.mexer") })
        : null,
      ));
      return partes;
    }));
    status.textContent = quebra >= 0
      ? t("cadeia4.quebrada", { h: blocos[quebra].altura, n: blocos.length - quebra })
      : t("cadeia4.inteira", { n: blocos.length });
    botaoRestaurar.hidden = quebra < 0;
    botaoNovo.disabled = quebra >= 0 || ocupado;
  }

  /** Muda o valor de uma transação e recalcula Merkle e hash, de verdade. */
  async function adulterar(i) {
    const b = blocos[i];
    const tx = b.txs[0];
    const bytes = tx.bytes.slice();
    bytes[bytes.length - 70] ^= 0x01;           // um bit do valor (u64 antes da assinatura)
    const raiz = await merkle([bytes, ...b.txs.slice(1).map((x) => x.bytes)]);
    const cab = b.cabecalho.slice();
    cab.set(raiz, OFFSET_MERKLE);
    b.cabecalho = cab;
    b.hash = hex(await sha512(cab));
    b.adulterado = true;
    selecionado = i;
    desenhar();
    desenharRegua();
    ler("merkle");
  }

  async function restaurar() {
    for (const b of blocos) {
      if (!b.adulterado) continue;
      b.cabecalho = b.cabecalhoOriginal.slice();
      b.hash = b.hashOriginal;
      b.adulterado = false;
    }
    desenhar();
    desenharRegua();
  }

  async function novo() {
    if (ocupado || quebradoDesde() >= 0) return;
    ocupado = true; desenhar();
    await minerar();
    selecionado = blocos.length - 1;
    ocupado = false;
    desenhar(); desenharRegua();
  }

  botaoNovo.addEventListener("click", novo);
  botaoRestaurar.addEventListener("click", restaurar);
  aoMudarIdioma(() => { desenhar(); desenharRegua(); });

  (async () => {
    for (let i = 0; i < 5; i++) await minerar();
    selecionado = 1;
    desenhar(); desenharRegua();
    if (calmo) return;
    // um bloco novo a cada 10 segundos, só com o capítulo na tela e a cadeia inteira
    let visivel = false;
    new IntersectionObserver((es) => { visivel = es[0].isIntersecting; }, { threshold: 0.2 }).observe(secao);
    setInterval(() => { if (visivel && !document.hidden) novo(); }, 10000);
  })();
}
