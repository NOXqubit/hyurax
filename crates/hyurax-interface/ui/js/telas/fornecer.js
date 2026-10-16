// Hyurax / Ultrax — Fornecer: este computador como fornecedor da plataforma
// HYURAX Compute (V1). O estado vem do agente do núcleo (/api/v1/plataforma);
// o vínculo usa o código gerado no painel da plataforma.

import { $, el, texto, fmt } from "../util.js";
import { obter, postar } from "../api.js";
import { fatos } from "./comum.js";

const ESTADOS = {
  IDLE: "livre, esperando trabalho",
  BUSY: "trabalhando",
  PAUSED: "pausado",
  QUARANTINED: "em quarentena",
  MAINTENANCE: "em manutenção",
  ONLINE: "conectado",
  OFFLINE: "desligado",
  REMOVED: "removido do painel",
};

const est = { dados: null, montado: false, nucleos: 1 };

function metrica(rotulo, valor, nota) {
  return el("div", { class: "metrica" }, el("span", { class: "metrica-rotulo" }, rotulo), el("span", { class: "metrica-valor" }, valor), nota ? el("span", { class: "metrica-nota" }, nota) : null);
}

function render() {
  const d = est.dados;
  if (!d) return;
  $("fo-desvinculado").hidden = d.vinculado;
  $("fo-vinculado").hidden = !d.vinculado;
  const nav = $("nav-fornecer");
  if (nav) {
    nav.hidden = !(d.vinculado && d.ligado && d.unidades_rodando > 0);
    nav.textContent = String(d.unidades_rodando || "");
  }
  if (!d.vinculado) {
    if (!$("fo-url").value && d.url) $("fo-url").value = d.url;
    return;
  }
  const ligado = !!d.ligado;
  $("fo-ligar").setAttribute("aria-pressed", String(ligado));
  texto("fo-ligar-texto", ligado ? "Fornecendo" : "Pausado");
  const st = $("fo-status");
  st.className = `heroi-status${d.conectado && ligado ? "" : " fora"}`;
  st.textContent = !ligado ? "pausado por você" : d.conectado ? ESTADOS[d.estado] || d.estado || "conectado" : "sem conexão com a plataforma";
  texto("fo-titulo", d.nome ? `${d.nome} · nó ${d.no}` : `Nó ${d.no}`);
  texto("fo-mensagem", d.ultimo_erro || d.mensagem || (d.atualizar ? "Atualize o programa para voltar a receber trabalho." : "Quando houver tarefa que caiba nos limites, ela roda no ULTRAX deste computador."));
  const r = d.resumo || {};
  $("fo-numeros").replaceChildren(
    metrica("Unidades rodando", fmt(d.unidades_rodando), `${fmt(d.unidades_na_fila)} na fila`),
    metrica("Entregues nesta abertura", fmt(d.enviados), `${fmt(d.aceitos)} aceitas · ${fmt(d.recusados)} recusadas`),
    metrica("Reputação", r.reputacao !== undefined ? fmt(r.reputacao) : "—", r.faixa ? `${r.faixa} · ${fmt(r.aceitas)} aceitas no total` : "atualiza a cada 30 s"),
    metrica("Ganhos", r.conta ? r.conta.disponivel : "—", r.conta ? `${r.conta.pendente} em retenção` : "no painel da plataforma"),
  );
  if (document.activeElement?.closest?.("#fo-form-lim") === null || !document.activeElement?.closest?.("#fo-form-lim")) {
    $("fo-lim-linhas").value = d.linhas;
    $("fo-lim-linhas").max = String(est.nucleos);
    $("fo-lim-ram").value = d.ram_mib;
    $("fo-lim-gpu").checked = !!d.gpu;
  }
  fatos("fo-fatos", [
    ["Plataforma", d.url],
    ["Número do nó", fmt(d.no)],
    ["Último pulso", d.ultimo_pulso_ms ? new Date(d.ultimo_pulso_ms).toLocaleTimeString("pt-BR") : "—"],
    ["Chave deste computador", el("code", {}, `${d.chave.slice(0, 16)}…`)],
  ]);
}

async function carregar() {
  const r = await obter("/plataforma");
  if (r.ok) {
    est.dados = r.dados;
    render();
  }
}

export function montar() {
  if (est.montado) return;
  est.montado = true;
  $("fo-form").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const s = $("fo-saida");
    s.className = "saida";
    s.textContent = "Vinculando…";
    $("fo-vincular").disabled = true;
    const r = await postar("/plataforma/vincular", {
      url: $("fo-url").value.trim(),
      codigo: $("fo-codigo").value.trim(),
      nome: $("fo-nome").value.trim(),
      linhas: $("fo-linhas").value,
      ram_mib: $("fo-ram").value,
      gpu: $("fo-gpu").checked ? "1" : "0",
    });
    $("fo-vincular").disabled = false;
    s.className = r.ok ? "saida" : "saida erro";
    s.textContent = r.ok ? `Vinculado como nó ${r.dados.no}.` : r.erro;
    if (r.ok) carregar();
  });
  $("fo-ligar").addEventListener("click", async () => {
    const ligar = $("fo-ligar").getAttribute("aria-pressed") !== "true";
    await postar("/plataforma/ligar", { ligar: ligar ? "1" : "0" });
    carregar();
  });
  $("fo-form-lim").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    await postar("/plataforma/ajustes", { linhas: $("fo-lim-linhas").value, ram_mib: $("fo-lim-ram").value, gpu: $("fo-lim-gpu").checked ? "1" : "0" });
    carregar();
  });
  $("fo-desvincular").addEventListener("click", async () => {
    if (!window.confirm("Desvincular este computador? O que estiver rodando é cancelado. Para voltar, gere outro código no painel.")) return;
    await postar("/plataforma/desvincular", {});
    carregar();
  });
}

export function aoMostrar(e) {
  const n = e?.metricas?.sistema?.nucleos_logicos;
  if (n) {
    est.nucleos = n;
    $("fo-linhas").max = String(n);
    if (Number($("fo-linhas").value) === 1 && n > 2) $("fo-linhas").value = String(Math.max(1, Math.floor(n / 2)));
  }
  const pode = !!e?.pode_mandar;
  for (const f of ["fo-form", "fo-form-lim"]) for (const x of $(f).elements) x.disabled = !pode;
  carregar();
}

let ultimaLeitura = 0;
export function atualizar() {
  // o estado chega a cada segundo; o agente pulsa a cada 5 s
  if ($("tela-fornecer").hidden || Date.now() - ultimaLeitura < 3000) return;
  ultimaLeitura = Date.now();
  carregar();
}

// o contador na navegação aparece já na abertura
setTimeout(carregar, 2000);
