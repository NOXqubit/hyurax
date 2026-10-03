// Hyurax / Ultrax 1.0 — a tela: navegação, barra de estado, janelas
// (termos, cadeado) e o backend de GPU. Cada seção mora em js/telas/.

import { $, texto, trocar, fmt } from "./util.js";
import { conectar, estado, ouvir, avisar } from "./estado.js";
import { obter, postar, pegarChaveDoEndereco, chaveDaSessao } from "./api.js";
import * as visao from "./telas/visao.js";
import * as ultrax from "./telas/ultrax.js";
import * as ciencia from "./telas/ciencia.js";
import * as carteira from "./telas/carteira.js";
import * as cadeia from "./telas/cadeia.js";
import * as rede from "./telas/rede.js";
import * as registro from "./telas/registro.js";
import * as ajustes from "./telas/ajustes.js";

// a chave de sessão vem no endereço; tira dele antes de qualquer coisa
pegarChaveDoEndereco();

const TELAS = { visao, ultrax, ciencia, carteira, cadeia, rede, registro, ajustes };
let atual = "visao";

// ---------- navegação ----------
function mostrarTela(nome) {
  if (!TELAS[nome]) nome = "visao";
  atual = nome;
  for (const n of Object.keys(TELAS)) $(`tela-${n}`).hidden = n !== nome;
  for (const a of document.querySelectorAll("#navegacao a")) {
    if (a.dataset.tela === nome) a.setAttribute("aria-current", "page");
    else a.removeAttribute("aria-current");
  }
  TELAS[nome].aoMostrar?.(estado.atual);
  $("principal").scrollTop = 0;
}
window.addEventListener("hashchange", () => {
  if (location.hash.startsWith("#chave=")) {
    pegarChaveDoEndereco();
    location.reload();
    return;
  }
  mostrarTela(location.hash.slice(1));
});

// ---------- barra de estado ----------
function atualizarTopo(e) {
  texto("topo-versao", `v${e.versao}`);
  texto("topo-rede", e.rede?.tipo === "TESTNET" ? "REDE DE TESTE" : String(e.rede?.nome || "").toUpperCase());
  if (e.trancado) return;
  texto("topo-altura", fmt(e.no?.altura));
  texto("topo-pares", fmt(e.no?.pares));
  texto("topo-sinc", e.no?.pares ? (e.no.sincronizado ? "alcançou os pares" : "sincronizando…") : "sem pares");
  texto("topo-saldo", e.carteira?.existe ? `${e.carteira.saldo} HYX` : "sem carteira");
  const s = e.metricas?.sistema;
  if (s) texto("nav-maquina", `${s.cpu || "CPU"} · ${s.nucleos_logicos} núcleo(s)${s.ram_total_mib ? ` · ${fmt(s.ram_total_mib / 1024, 1)} GiB` : ""}`);
}

let jaConectou = false;
ouvir("conexao", (ligado) => {
  const d = $("topo-conexao");
  d.className = ligado ? "estado-ligado" : "estado-desligado";
  d.textContent = ligado ? "conectado" : "sem conexão (tentando de novo)";
  // a faixa só aparece depois de ter conectado uma vez (abrir não é falha)
  if (ligado) jaConectou = true;
  $("faixa-conexao").hidden = ligado || !jaConectou;
  document.body.classList.toggle("desconectado", !ligado && jaConectou);
});

// ---------- termos e cadeado ----------
let termosCarregados = false;
async function abrirTermos(obrigatorio) {
  const j = $("janela-termos");
  if (!termosCarregados) {
    const r = await fetch("/api/v1/termos", { cache: "no-store" });
    $("jt-texto").innerHTML = r.ok ? await r.text() : "<p>Não consegui carregar os termos.</p>";
    termosCarregados = true;
  }
  $("jt-aceitar").hidden = !obrigatorio;
  $("jt-li").closest("label").hidden = !obrigatorio;
  $("jt-fechar").hidden = obrigatorio;
  j.hidden = false;
  conferirRolagem();
}
function conferirRolagem() {
  const t = $("jt-texto");
  if (t.scrollTop + t.clientHeight >= t.scrollHeight - 24) {
    $("jt-li").disabled = false;
    texto("jt-rotulo", "Li e aceito os termos de uso");
  }
}
$("jt-texto").addEventListener("scroll", conferirRolagem);
$("jt-li").addEventListener("change", (e) => { $("jt-aceitar").disabled = !e.target.checked; });
$("jt-aceitar").addEventListener("click", async () => {
  const r = await postar("/termos/aceitar");
  if (r.ok) $("janela-termos").hidden = true;
});
$("jt-fechar").addEventListener("click", () => { $("janela-termos").hidden = true; });
export { abrirTermos };

$("jv-form").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const r = await postar("/destravar", { codigo: $("jv-codigo").value.trim() });
  const s = $("jv-saida");
  s.className = r.ok ? "saida" : "saida erro";
  s.textContent = r.ok ? "destrancado" : r.erro;
  if (r.ok) $("jv-codigo").value = "";
});

// ---------- backend de GPU (WebGL 2 num worker desta janela) ----------
let trabalhadorGpu = null;
function cuidarDaGpu(e) {
  const g = e.ultrax?.gpu;
  const querer = !!(e.pode_mandar && e.ultrax?.ligado && g?.ligada);
  if (querer && !trabalhadorGpu) {
    trabalhadorGpu = new Worker("/js/gpu/trabalhador.js", { type: "module" });
    trabalhadorGpu.postMessage({ chave: chaveDaSessao() });
    trabalhadorGpu.onmessage = (m) => {
      const d = m.data || {};
      if (d.amostra) avisar("amostra", d.amostra);
      if (d.erro !== undefined || d.nome) avisar("gpu", d);
    };
  }
  trabalhadorGpu?.postMessage({ ligada: querer, uso: g?.uso ?? 50 });
}

// ---------- aviso de bloco deste nó ----------
let som = null;
ouvir("bloco", (b) => {
  const aj = estado.atual?.ajustes;
  if (!b.meu || aj?.avisar_bloco === false) return;
  const a = $("aviso-bloco");
  a.textContent = `Bloco ${fmt(b.altura)} minerado por este nó · +${b.recompensa} HYX de teste (libera depois da maturidade)`;
  a.hidden = false;
  setTimeout(() => { a.hidden = true; }, 8000);
  if (aj?.som_bloco) {
    try {
      som ??= new AudioContext();
      const o = som.createOscillator();
      const g = som.createGain();
      o.frequency.value = 880;
      g.gain.setValueAtTime(0.08, som.currentTime);
      g.gain.exponentialRampToValueAtTime(0.0001, som.currentTime + 0.4);
      o.connect(g).connect(som.destination);
      o.start();
      o.stop(som.currentTime + 0.4);
    } catch { /* sem áudio */ }
  }
});

// ---------- aviso de JOB que terminou ou parou ----------
const AVISOS_DE_JOB = {
  JOB_COMPLETED: "concluído: o relatório está pronto em Computação científica",
  JOB_EXPIRED: "venceu o prazo antes de terminar",
  JOB_OUT_OF_BUDGET: "parou: o orçamento de créditos acabou",
  JOB_WAITING_FOR_NODES: "esperando workers de outros nós para continuar",
};
let avisoAte = 0;
ouvir("ciencia", (d) => {
  const msg = AVISOS_DE_JOB[d?.event];
  if (!msg || !d.job_id) return;
  const a = $("aviso-bloco");
  a.textContent = `JOB ${String(d.job_id).slice(0, 10)}… ${msg}`;
  a.hidden = false;
  avisoAte = Date.now() + 8000;
  setTimeout(() => { if (Date.now() >= avisoAte) a.hidden = true; }, 8100);
});

// ---------- tudo junto ----------
ouvir("estado", (e) => {
  atualizarTopo(e);
  $("janela-trava").hidden = !e.trancado;
  if (!e.trancado && e.termos && !e.termos.aceitos && $("janela-termos").hidden) abrirTermos(true);
  if (e.trancado) return;
  cuidarDaGpu(e);
  for (const [nome, tela] of Object.entries(TELAS)) {
    if (nome === atual || tela.sempre) tela.atualizar?.(e);
  }
});

for (const tela of Object.values(TELAS)) tela.montar?.({ abrirTermos });
mostrarTela(location.hash.slice(1) || "visao");
conectar();
// o estado chega pelo fluxo; esta leitura só adianta o primeiro quadro
obter("/estado").then((r) => {
  if (r.ok) avisar("estado", r.dados);
  else if (r.status === 401) {
    // aberto num navegador sem a chave desta abertura do programa
    const d = $("topo-conexao");
    d.className = "estado-desligado";
    d.textContent = "sem a chave: abra pela janela do programa (ou pelo endereço que o hyurax-no painel mostrou)";
  }
});
trocar("vg-registro");
