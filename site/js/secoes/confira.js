// Não confie. Confira: mexa num bloco antigo e veja a cadeia denunciar.
//
// Cinco blocos com cabeçalho real de 222 bytes (§7 da especificação) e SHA-512
// de verdade. Tocar num bloco muda um valor de uma transação dele: a raiz de
// Merkle muda, o hash do bloco muda, e o bloco seguinte, que guardou o hash
// antigo como "anterior", deixa de bater. A quebra desce até a ponta.

import { SimulationEngine, merkle, sha512, hex, curto } from "../simulation/engine.js";

const QUANTOS = 5;
// Posição da raiz de Merkle no cabeçalho: versão (2) + altura (8) + anterior (64).
const INICIO_MERKLE = 2 + 8 + 64;

export function iniciarConfira({ t }) {
  const lista = document.getElementById("cd-blocos");
  const status = document.getElementById("cd-status");
  const restaurar = document.getElementById("cd-restaurar");
  if (!lista) return { redesenhar() {} };

  const motor = new SimulationEngine(777);
  let blocos = [];      // como foram minerados
  let calculado = [];   // hash recalculado de cada bloco, depois de uma mexida
  let mexido = -1;

  async function montar() {
    blocos = [];
    let anterior = null;
    for (let i = 0; i < QUANTOS; i++) {
      const txs = Array.from({ length: 3 }, () => motor.createTransaction());
      anterior = await motor.mineBlock(anterior, txs);
      blocos.push(anterior);
    }
    calculado = blocos.map((b) => b.hash);
    mexido = -1;
    desenhar();
  }

  async function mexer(k) {
    if (mexido >= 0) return;
    const b = blocos[k];
    // Muda o último byte (parte do valor) da primeira transação.
    const txs = b.txs.map((tx, i) => {
      if (i !== 0) return tx.bytes;
      const c = tx.bytes.slice();
      c[c.length - 1] ^= 0x2a;
      return c;
    });
    const raiz = await merkle(txs);
    const cab = b.cabecalhoOriginal.slice();
    cab.set(raiz, INICIO_MERKLE);
    calculado = blocos.map((x) => x.hash);
    calculado[k] = hex(await sha512(cab));
    mexido = k;
    desenhar();
  }

  function campo(rotulo, valor, classe = "") {
    const dt = document.createElement("dt");
    dt.textContent = rotulo;
    const dd = document.createElement("dd");
    dd.className = classe;
    dd.textContent = valor;
    return [dt, dd];
  }

  function desenhar() {
    lista.replaceChildren(...blocos.map((b, k) => {
      const li = document.createElement("li");
      const botao = document.createElement("button");
      botao.type = "button";
      const quebrado = mexido >= 0 && k > mexido;
      botao.className = "bloco" + (k === mexido ? " mexido" : "") + (quebrado ? " quebrado" : "");
      botao.setAttribute("aria-label", t("confira.mexer", { a: b.altura }));
      botao.addEventListener("click", () => mexer(k));
      const liga = document.createElement("span");
      liga.className = "liga";
      liga.hidden = k === 0;
      const alt = document.createElement("span");
      alt.className = "altura";
      alt.textContent = t("confira.bloco", { a: b.altura });
      const dl = document.createElement("dl");
      dl.append(
        ...campo(t("confira.anterior"), curto(b.anterior, 4), "anterior"),
        ...campo(t("confira.merkle"), curto(k === mexido ? "≠ " + b.merkle : b.merkle, 4)),
        ...campo(t("confira.hash"), curto(calculado[k], 4), "hash"),
      );
      botao.append(liga, alt, dl);
      li.append(botao);
      return li;
    }));
    // O "li" não pode ter a classe do botão; a ligação desenha entre botões.
    lista.querySelectorAll("li").forEach((li) => { li.style.display = "contents"; });

    restaurar.hidden = mexido < 0;
    if (mexido < 0) {
      status.className = "veredito ok";
      status.textContent = t("confira.inteira", { n: blocos.length });
    } else {
      status.className = "veredito ruim";
      status.textContent = t("confira.quebrada", { a: blocos[mexido].altura, n: blocos.length - mexido - 1 });
    }
  }

  restaurar.addEventListener("click", () => {
    calculado = blocos.map((b) => b.hash);
    mexido = -1;
    desenhar();
  });

  montar();
  return { redesenhar: () => blocos.length && desenhar() };
}
