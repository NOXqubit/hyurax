// Hyurax / Ultrax — a thread do backend de GPU. Roda num worker, e não na página,
// porque página escondida (janela minimizada) tem os timers desacelerados; o
// worker segue no ritmo. A página só manda o que o dono escolheu (ligada e
// fatia de uso) e mostra o que este worker conta de volta.

import { criarGpu } from "./computacao.js";

let pedido = { ligada: false, uso: 50 };
let chave = "";
onmessage = (ev) => {
  const d = ev.data || {};
  if (typeof d.chave === "string") chave = d.chave;
  if ("ligada" in d) pedido = d;
};
const cabecalhos = (extra) => (chave ? { ...extra, "X-Hyurax-Chave": chave } : extra);

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));
const contar = (dados) => postMessage(dados);

async function postar(caminho, campos) {
  try {
    const r = await fetch(caminho, {
      method: "POST",
      headers: cabecalhos({ "Content-Type": "application/x-www-form-urlencoded" }),
      body: new URLSearchParams(campos || {}),
    });
    const dados = r.status === 204 ? null : await r.json().catch(() => null);
    return { ok: r.ok, dados };
  } catch {
    return { ok: false, dados: null };
  }
}

let gpu;
async function laco() {
  for (;;) {
    if (!pedido.ligada) {
      await dormir(1000);
      continue;
    }
    if (gpu === undefined) {
      gpu = criarGpu();
      contar(gpu ? { nome: gpu.nome } : { erro: "Esta janela não tem WebGL2: a GPU não pode ser usada aqui." });
    }
    if (!gpu) {
      await dormir(5000);
      continue;
    }
    const pego = await postar("/api/v1/gpu/pegar", { nome: gpu.nome });
    if (!pego.ok || !pego.dados) {
      await dormir(2000);
      continue;
    }
    const { numero, n, origem, job, unidade } = pego.dados;
    const contexto = {
      tarefa: numero,
      job: job ?? null,
      unidade: unidade ?? null,
      linha: 0,
      recurso: "GPU",
      lab: origem !== "JOB",
      desafio: false,
      resumo: `multiplicação de matrizes ${n}×${n} na GPU (${origem === "JOB" ? `unidade ${unidade} de um JOB` : "carga LAB"})`,
      tamanho: n,
      passos: 0,
      parametros: [],
    };
    try {
      const r = await fetch(`/api/v1/gpu/entrada/${numero}`, { cache: "no-store", headers: cabecalhos({}) });
      if (!r.ok) throw new Error(`entrada ${r.status}`);
      const tudo = new Uint32Array(await r.arrayBuffer());
      let aviso = 0;
      const C = await gpu.multiplicar(tudo.subarray(0, n * n), tudo.subarray(n * n), n, {
        uso: () => pedido.uso,
        parar: () => !pedido.ligada,
        progresso: (linhas) => {
          const agora = performance.now();
          if (agora - aviso > 800 || linhas === n) {
            aviso = agora;
            postar(`/api/v1/gpu/progresso/${numero}`, { linhas: String(linhas) });
          }
        },
        // a linha que a GPU acabou de calcular vai para a cena (no máximo 128 colunas)
        faixa: (linha, valores) => {
          const aCada = Math.max(1, Math.ceil(valores.length / 128));
          const recorte = [];
          for (let j = 0; j < valores.length; j += aCada) recorte.push(valores[j]);
          contar({ amostra: { contexto, amostra: { tipo: "matrix", lado: n, linha, valores: recorte, a_cada: aCada } } });
        },
      });
      if (!C) {
        await postar(`/api/v1/gpu/cancelar/${numero}`, { motivo: "a GPU foi desligada no meio da conta" });
        continue;
      }
      await fetch(`/api/v1/gpu/resultado/${numero}`, { method: "POST", headers: cabecalhos({ "Content-Type": "application/octet-stream" }), body: C });
      contar({ nome: gpu.nome, erro: "" });
    } catch (erro) {
      const motivo = `A GPU falhou: ${erro.message || erro}`;
      contar({ erro: motivo });
      await postar(`/api/v1/gpu/cancelar/${numero}`, { motivo });
      await dormir(5000);
    }
  }
}
laco();
