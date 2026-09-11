// BLOCKCHAIN: transações entram na fila, viram bloco, passam pela validação e entram na cadeia.
// Hash do cabeçalho de 158 bytes e raiz de Merkle são SHA-512 reais, sobre dados simulados.
import { el, etapas, espera, formataAUR } from "../ui.js";
import { curto } from "../simulation/engine.js";

export function iniciar({ t, engine, calmo, aoMudarIdioma }) {
  const secao = document.getElementById("cadeia");
  const mempoolEl = document.getElementById("mempool");
  const trilho = document.getElementById("trilho");
  const candidato = document.getElementById("candidato");
  const candCorpo = document.getElementById("candidato-corpo");
  const checagens = document.getElementById("checagens");
  const detalhe = document.getElementById("tx-detalhe");
  const botaoPausa = document.getElementById("cadeia-pausa");
  const marca = etapas(document.getElementById("cadeia-etapas"), ["TRANSACTION", "MEMPOOL", "BLOCK", "VALIDATION", "CHAIN"]);

  const fila = [], cadeia = [];
  let pausado = false, visivel = false, ocupado = false, escolhida = null;
  const data = (s) => new Date(s * 1000).toISOString().replace("T", " ").slice(0, 19) + " UTC";

  function chipTx(tx, nova) {
    return el("button", { type: "button", class: "tx-chip" + (nova ? " nova" : ""), onclick: () => mostrarTx(tx), "aria-label": `TX ${tx.id ? tx.id.slice(0, 8) : ""}` },
      el("span", {}, "TX ", el("b", { text: tx.id ? tx.id.slice(0, 8) : "…" })), el("span", { text: `${formataAUR(tx.valor)} AUR` }));
  }
  function desenharFila() {
    mempoolEl.replaceChildren(el("h4", { text: "MEMPOOL" }), ...fila.map((tx) => { const c = chipTx(tx, tx._nova); tx._nova = false; return c; }));
    if (!fila.length) mempoolEl.append(el("p", { class: "texto-2 mono", style: "font-size:.7rem;margin:0", text: t("cadeia.aguardando") }));
  }
  function mostrarTx(tx) {
    escolhida = tx;
    const c = (k) => t(`cadeia.campos.${k}`);
    const dl = el("dl", { class: "campos" },
      el("dt", { text: c("ativo") }), el("dd", { text: "AUR · 0x00…00 (32 bytes)" }),
      el("dt", { text: c("valor") }), el("dd", { text: `${formataAUR(tx.valor)} AUR` }),
      el("dt", { text: c("remetente") }), el("dd", { text: curto(tx.remetente, 6) }),
      el("dt", { text: c("destinatario") }), el("dd", { text: curto(tx.destinatario, 6) }),
      el("dt", { text: c("nonce") }), el("dd", { text: String(tx.nonce) }),
      el("dt", { text: c("taxa") }), el("dd", { text: `${formataAUR(tx.taxa)} AUR` }),
      el("dt", { text: c("assinatura") }), el("dd", {}, curto(tx.assinatura, 10), el("small", { text: `Ed25519 · ${t("cadeia.sim_assinatura")}` })),
      el("dt", { text: c("estado") }), el("dd", { class: tx.bloco !== null ? "ok" : "", text: tx.bloco !== null ? t("cadeia.confirmada", { h: tx.bloco }) : t("cadeia.na_fila") }),
      el("dt", { text: "ID" }), el("dd", { text: tx.id ? curto(tx.id, 10) : "…" }),
    );
    detalhe.replaceChildren(dl);
  }
  function cartaoBloco(b, novo) {
    const c = (k) => t(`cadeia.bloco.${k}`);
    return el("article", { class: "bloco" + (novo ? " novo" : "") },
      el("header", {}, el("b", { text: "BLOCK" }), el("span", { text: `#${b.altura}` })),
      el("dl", { class: "campos" },
        el("dt", { text: c("hash") }), el("dd", { text: curto(b.hash, 7) }),
        el("dt", { text: c("anterior") }), el("dd", { text: curto(b.anterior, 7) }),
        el("dt", { text: c("horario") }), el("dd", { text: data(b.horario) }),
        el("dt", { text: c("merkle") }), el("dd", { text: curto(b.merkle, 7) }),
        el("dt", { text: c("transacoes") }), el("dd", { text: String(b.txs.length) }),
      ),
      el("div", { class: "txs" }, b.txs.map((tx) => el("button", { type: "button", onclick: () => mostrarTx(tx), text: tx.id.slice(0, 6) }))),
    );
  }
  function desenharCadeia(novo) {
    trilho.replaceChildren(...cadeia.slice(-10).map((b, i, arr) => cartaoBloco(b, novo && i === arr.length - 1)));
    trilho.scrollLeft = trilho.scrollWidth;
  }

  async function novaTx() {
    const tx = engine.createTransaction();
    await engine.idDaTransacao(tx);
    tx._nova = true;
    fila.push(tx);
    if (fila.length > 9) fila.shift();
    marca(0); await espera(calmo ? 0 : 350); marca(1);
    desenharFila();
  }

  async function fecharBloco() {
    if (ocupado || !fila.length) return;
    ocupado = true;
    const txs = fila.splice(0, Math.min(5, fila.length));
    desenharFila();
    const altura = cadeia.length ? cadeia[cadeia.length - 1].altura + 1 : 0;
    marca(2);
    candCorpo.textContent = t("cadeia.montando", { h: altura, n: txs.length });
    checagens.replaceChildren(...t("cadeia.checagens").map((c) => el("li", { text: c })));
    await espera(calmo ? 0 : 700);
    marca(3); candidato.classList.add("validando");
    for (const li of checagens.children) { await espera(calmo ? 0 : 320); li.classList.add("ok"); }
    const bloco = await engine.mineBlock(cadeia[cadeia.length - 1], txs);
    txs.forEach((tx) => { tx.bloco = bloco.altura; });
    cadeia.push(bloco);
    marca(4); candidato.classList.remove("validando");
    candCorpo.textContent = `#${bloco.altura} · ${curto(bloco.hash, 8)} · ${bloco.tamanhoCabecalho} bytes`;
    desenharCadeia(true);
    if (escolhida) mostrarTx(escolhida);
    ocupado = false;
  }

  // ritmo acelerado da demonstração
  let ultimoTx = 0, ultimoBloco = 0;
  async function batida() {
    if (pausado || !visivel || document.hidden) return;
    const agora = performance.now();
    if (agora - ultimoTx > 1500 && fila.length < 8) { ultimoTx = agora; await novaTx(); }
    if (agora - ultimoBloco > 9000 && fila.length >= 2) { ultimoBloco = agora; await fecharBloco(); }
  }
  if (!calmo) setInterval(batida, 500);
  new IntersectionObserver((es) => { visivel = es[0].isIntersecting; }).observe(secao);

  document.getElementById("cadeia-tx").addEventListener("click", async () => { await novaTx(); if (fila.length >= 3 && calmo) await fecharBloco(); });
  botaoPausa.addEventListener("click", () => { pausado = !pausado; botaoPausa.textContent = t(pausado ? "ui.continuar" : "ui.pausar"); });

  // estado inicial visível: três blocos e algumas transações na fila
  (async () => {
    for (let b = 0; b < 3; b++) {
      const txs = []; for (let i = 0; i < 2 + b; i++) { const tx = engine.createTransaction(); await engine.idDaTransacao(tx); txs.push(tx); }
      const bloco = await engine.mineBlock(cadeia[cadeia.length - 1], txs);
      txs.forEach((tx) => { tx.bloco = bloco.altura; });
      cadeia.push(bloco);
    }
    for (let i = 0; i < 3; i++) { const tx = engine.createTransaction(); await engine.idDaTransacao(tx); fila.push(tx); }
    desenharCadeia(false); desenharFila(); marca(1);
    candCorpo.textContent = t("cadeia.aguardando");
    ultimoBloco = performance.now();
  })();

  aoMudarIdioma(() => {
    desenharFila(); desenharCadeia(false);
    botaoPausa.textContent = t(pausado ? "ui.continuar" : "ui.pausar");
    if (!ocupado) candCorpo.textContent = t("cadeia.aguardando");
    if (escolhida) mostrarTx(escolhida);
  });
}
