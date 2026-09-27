// Hyurax — a thread da GPU. Roda num worker do navegador, e não na página,
// porque página escondida (janela minimizada) tem os timers desacelerados; o
// worker segue no ritmo. A página só manda o que o dono escolheu (ligada e
// fatia de uso) e mostra o que este worker conta de volta.

import { criarGpu } from "./gpu.js";

let pedido = { ligada: false, uso: 50 };
onmessage = (ev) => { pedido = ev.data || pedido; };

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));
const contar = (dados) => postMessage(dados);

async function postar(caminho, campos) {
  try {
    const r = await fetch(caminho, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
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
    const pego = await postar("/api/ultrax/gpu/pegar", { nome: gpu.nome });
    if (!pego.ok || !pego.dados) {
      await dormir(2000);
      continue;
    }
    const { numero, n } = pego.dados;
    try {
      const r = await fetch(`/api/ultrax/gpu/entrada/${numero}`, { cache: "no-store" });
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
            postar(`/api/ultrax/gpu/progresso/${numero}`, { linhas: String(linhas) });
          }
        },
      });
      if (!C) {
        await postar(`/api/ultrax/gpu/cancelar/${numero}`, { motivo: "a GPU foi desligada no meio da conta" });
        continue;
      }
      await fetch(`/api/ultrax/gpu/resultado/${numero}`, { method: "POST", headers: { "Content-Type": "application/octet-stream" }, body: C });
      contar({ nome: gpu.nome, erro: "" });
    } catch (erro) {
      const motivo = `A GPU falhou: ${erro.message || erro}`;
      contar({ erro: motivo });
      await postar(`/api/ultrax/gpu/cancelar/${numero}`, { motivo });
      await dormir(5000);
    }
  }
}
laco();
