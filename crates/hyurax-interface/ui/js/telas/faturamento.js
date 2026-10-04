// Hyurax / Ultrax — Faturamento: o livro de contas encadeado por hash, as
// faturas por mês e as tarifas (provedor, plataforma, reserva).
//
// Créditos de computação, não dinheiro: nada aqui paga ninguém.

import { $, fmt, curto } from "../util.js";
import { postar } from "../api.js";
import { nuvem, aoMudar, mostrar, esconder, ler } from "../nuvem.js";
import { creditos, metricas, marcarEstado, estadoEl, tabela, resultado } from "./comum.js";

const TIPOS = {
  consumo: ["em-curso", "consumo"],
  provedor: ["ok", "provedor"],
  plataforma: ["parado", "plataforma"],
  reserva: ["parado", "reserva"],
  recibo: ["ok", "a receber"],
  credito: ["ok", "crédito"],
};
let tarifasPreenchidas = false;

function desenhar() {
  const l = nuvem.livro;
  const d = nuvem.dados;
  if (!l || !d) {
    if (nuvem.erro) marcarEstado("ft-estado", "falha", nuvem.erro);
    return;
  }
  marcarEstado(
    "ft-estado",
    l.integro ? "ok" : "falha",
    l.integro ? "livro íntegro: cada linha confere com a anterior" : `livro adulterado: a conferência quebra no lançamento ${l.quebra_em}`,
  );
  const t = l.totais || {};
  metricas("ft-metricas", [
    { rotulo: "Consumo registrado", valor: creditos(t.consumo || 0), nota: "JOBs, aluguéis e armazenamento" },
    { rotulo: "Aos provedores", valor: creditos(t.provedor || 0), nota: `${l.tarifas.provedor_pct}% de cada consumo de cliente` },
    { rotulo: "Comissão da plataforma", valor: creditos(t.plataforma || 0), nota: `${l.tarifas.plataforma_pct}%` },
    { rotulo: "Reserva", valor: creditos(t.reserva || 0), nota: `${l.tarifas.reserva_pct}% (o resto, para fechar a conta)` },
    { rotulo: "A receber", valor: creditos(t.recibo || 0), nota: "recibos assinados por quem alugou esta máquina" },
  ]);
  if (!tarifasPreenchidas) {
    $("ft-provedor").value = String(l.tarifas.provedor_pct);
    $("ft-plataforma").value = String(l.tarifas.plataforma_pct);
    tarifasPreenchidas = true;
    reserva();
  }
  tabela(
    "ft-faturas",
    [{ t: "Mês" }, { t: "Conta" }, { t: "Consumo", num: true }, { t: "Lançamentos", num: true }],
    (l.faturas || []).map((f) => [f.mes, f.conta ? `cliente #${f.conta}` : "este nó", { v: creditos(f.consumo_mili), num: true }, { v: fmt(f.lancamentos), num: true }]),
    { vazio: "nenhum consumo ainda" },
  );
  tabela(
    "ft-livro",
    [{ t: "#", num: true }, { t: "Quando" }, { t: "Tipo" }, { t: "Conta" }, { t: "JOB" }, { t: "Outra parte" }, { t: "Valor", num: true }, { t: "Nota" }, { t: "Hash" }],
    (l.linhas || []).map((x) => {
      const [forma, rotulo] = TIPOS[x.tipo] || ["parado", x.tipo];
      return [
        { v: fmt(x.seq), num: true },
        new Date(x.instante * 1000).toLocaleString("pt-BR", { hour12: false }),
        estadoEl(forma, rotulo),
        x.conta ? `#${x.conta}` : "—",
        x.job ? { v: curto(x.job, 10), title: x.job } : "—",
        x.contraparte ? { v: curto(x.contraparte, 10), title: x.contraparte } : "—",
        { v: creditos(x.valor_mili), num: true },
        x.nota || "—",
        { v: x.hash, title: "início do hash encadeado deste lançamento" },
      ];
    }),
    { vazio: "o livro ainda está vazio: o consumo dos JOBs entra a cada 10 s" },
  );
}

function reserva() {
  const p = Number($("ft-provedor").value) || 0;
  const pl = Number($("ft-plataforma").value) || 0;
  const r = 100 - p - pl;
  const s = $("ft-reserva");
  s.textContent = r < 0 ? "Provedor + plataforma passam de 100%." : `Reserva: ${r}% (o que sobra de cada consumo, para a conta sempre fechar).`;
  s.style.color = r < 0 ? "var(--falha)" : "";
}

export function montar() {
  $("ft-provedor").addEventListener("input", reserva);
  $("ft-plataforma").addEventListener("input", reserva);
  $("ft-tarifas").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const r = await postar("/nuvem/ajustes", { provedor_pct: $("ft-provedor").value, plataforma_pct: $("ft-plataforma").value });
    resultado("ft-saida", r, "Tarifas salvas: valem para os próximos lançamentos (os antigos não mudam).");
    if (r.ok) ler();
  });
  aoMudar(() => {
    if (!$("tela-faturamento").hidden) desenhar();
  });
}

export function aoMostrar() {
  mostrar();
  desenhar();
}

export function aoEsconder() {
  esconder();
}

