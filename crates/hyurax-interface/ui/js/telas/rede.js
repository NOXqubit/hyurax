// Hyurax / Ultrax — Rede: este nó, as sementes e as outras máquinas do dono
// (só leitura, pelo resumo público de cada uma).

import { $, fmt } from "../util.js";
import { postar } from "../api.js";
import { fatos, tabela, resultado } from "./comum.js";

function preencher(id, lista) {
  const t = $(id);
  if (document.activeElement === t || t.dataset.mexido === "1") return;
  t.value = (lista || []).join("\n");
}

export function montar() {
  for (const id of ["r-lista", "r-maquinas-lista"]) $(id).addEventListener("input", (e) => { e.target.dataset.mexido = "1"; });
  $("r-sementes").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const r = await postar("/sementes", { lista: $("r-lista").value });
    resultado("r-saida", r, r.ok ? `Salvas ${r.dados.sementes.length} semente(s); conectando.` : "");
    if (r.ok) $("r-lista").dataset.mexido = "";
  });
  $("r-maquinas").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const r = await postar("/maquinas", { lista: $("r-maquinas-lista").value });
    resultado("r-maquinas-saida", r, r.ok ? `Salvas ${r.dados.maquinas.length} máquina(s).` : "");
    if (r.ok) $("r-maquinas-lista").dataset.mexido = "";
  });
}

export function atualizar(e) {
  const n = e.no || {};
  fatos("r-fatos", [
    ["Rede", `${e.rede?.nome} · ${e.rede?.tipo}`],
    ["Aviso", e.rede?.aviso],
    ["Porta P2P", fmt(n.porta_p2p)],
    ["Pares conectados", fmt(n.pares)],
    ["Sincronização", n.pares ? (n.sincronizado ? "alcançou os pares" : "sincronizando") : "sem pares (só este nó)"],
    ["Altura local", fmt(n.altura)],
    ["Mempool", `${fmt(n.mempool)} tx`],
    ["Bytes trafegados", "PENDENTE (não medido na 1.0)"],
  ]);
  const janela = e.modo === "janela" && e.pode_mandar;
  for (const x of $("r-sementes").elements) x.disabled = !janela;
  for (const x of $("r-maquinas").elements) x.disabled = !e.pode_mandar;
  preencher("r-lista", e.ajustes?.sementes);
  preencher("r-maquinas-lista", e.ajustes?.maquinas);
  tabela(
    "r-maquinas-tabela",
    [{ t: "Máquina" }, { t: "Estado" }, { t: "Altura", num: true }, { t: "Pares", num: true }, { t: "Mineração" }, { t: "ULTRAX" }, { t: "CPU do programa", num: true }, { t: "Saldo", num: true }],
    (e.maquinas || []).map((m) => {
      const r = m.resumo || {};
      if (!m.ok) return [m.alvo, `sem resposta: ${m.erro}`, "", "", "", "", "", ""];
      // a resposta veio assinada pela chave desta máquina (vista na primeira vez)
      return [
        { v: m.alvo, title: m.chave ? `chave ${m.chave}` : "" },
        `${r.produto} ${r.versao} · ${r.rede} · assinado ${(m.chave || "").slice(0, 8)}…`,
        { v: fmt(r.altura), num: true },
        { v: fmt(r.pares), num: true },
        r.minerando ? `${r.linhas} de ${r.nucleos} · ${fmt(r.ritmo, 2)} tent./s` : "parada",
        r.ultrax_ligado ? "ligado" : "desligado",
        { v: r.cpu_processo?.valor != null ? `${fmt(r.cpu_processo.valor, 1)}%` : "—", num: true },
        { v: r.saldo ? `${r.saldo} HYX` : "—", num: true, title: r.endereco || "" },
      ];
    }),
    { vazio: "nenhuma máquina na lista" },
  );
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}

