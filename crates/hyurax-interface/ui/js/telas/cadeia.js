// Hyurax / Ultrax — Cadeia e mineração: controle da mineração (o limite é
// escolha do dono; o ritmo é medido), a ponta da cadeia e os blocos recentes.

import { $, el, fmt, compacto, curto, valorComOrigem, selo } from "../util.js";
import { postar } from "../api.js";
import { fatos, tabela, grafico, cores, deslizante, acertar, chave, marcar } from "./comum.js";

const c = {};
let montado = false;

async function mandar(campos) {
  const r = await postar("/mineracao", campos);
  const s = $("m-saida");
  if (s) {
    s.className = r.ok ? "saida" : "saida erro";
    s.textContent = r.ok ? "" : r.erro;
  }
}

function montarControles(e) {
  const m = e.mineracao;
  c.ligar = chave({ id: "m-ligar", rotulo: "Minerar (a recompensa vai para a carteira deste computador)", aoMudar: (v) => mandar({ ligar: v ? "1" : "0" }) });
  c.linhas = deslizante({ id: "m-linhas", rotulo: "Linhas", min: 1, max: m.nucleos, valor: m.linhas, formato: (v) => `${v} de ${m.nucleos}`, aoSoltar: (v) => mandar({ linhas: v }) });
  c.limite = deslizante({ id: "m-limite", rotulo: "Limite por linha", min: 10, max: 100, passo: 5, valor: m.limite_cpu.valor, formato: (v) => `${v}%`, aoSoltar: (v) => mandar({ limite_cpu: v }) });
  const perfis = el(
    "div",
    { class: "acoes" },
    ...[["leve", "Leve"], ["equilibrado", "Equilibrado"], ["turbo", "Turbo"]].map(([p, nome]) => el("button", { type: "button", class: "botao-leve", onclick: () => mandar({ perfil: p }) }, nome)),
  );
  c.perfis = perfis;
  $("m-controles").replaceChildren(c.ligar.caixa, perfis, c.linhas.caixa, c.limite.caixa, el("p", { class: "saida", id: "m-saida", role: "status" }));
  montado = true;
}

function mineracao(e) {
  const m = e.mineracao;
  if (!montado) montarControles(e);
  marcar(c.ligar, m.ligada);
  acertar(c.linhas, m.linhas, (v) => `${v} de ${m.nucleos}`);
  acertar(c.limite, m.limite_cpu.valor, (v) => `${v}%`);
  const pode = !!e.pode_mandar;
  c.ligar.entrada.disabled = !pode || !e.carteira?.existe;
  c.linhas.entrada.disabled = c.limite.entrada.disabled = !pode;
  for (const b of c.perfis.children) b.disabled = !pode;
  if (!e.carteira?.existe && pode) $("m-saida").textContent = "Crie ou importe uma carteira para minerar.";
  fatos("m-fatos", [
    ["Estado", m.ligada ? (m.rodada ? `minerando o bloco ${fmt(m.rodada.altura)} há ${fmt(m.rodada.segundos, 0)} s` : "minerando") : "parada"],
    ["Ritmo", valorComOrigem(m.ritmo, 2)],
    ["Tempo por tentativa", m.ms_tentativa ? `${fmt(m.ms_tentativa, 0)} ms` : "—"],
    ["Tentativas", compacto(m.tentativas)],
    ["Limite por linha", valorComOrigem(m.limite_cpu, 0)],
    ["CPU deste programa", valorComOrigem(e.metricas?.cpu_processo, 1)],
    ["Memória do Argon2id", `${fmt(m.memoria_mib, m.memoria_mib < 10 ? 2 : 0)} MiB (${m.linhas} linha(s))`],
    ["Blocos deste nó", `${fmt(m.meus_sessao)} nesta sessão · ${fmt(m.meus_cadeia)} na cadeia`],
    ["Perdidos na corrida", fmt(m.perdidos)],
  ]);
  // ritmo: diferença entre amostras consecutivas (tentativas somadas)
  const a = m.amostras || [];
  const pontos = [];
  for (let i = 1; i < a.length; i++) {
    const dt = a[i][0] - a[i - 1][0];
    if (dt > 0) pontos.push([a[i][0], (a[i][1] - a[i - 1][1]) / dt]);
  }
  grafico("m-grafico", [{ pontos, cor: cores().tinta }], { rotulo: (max) => `${fmt(max, 2)} tentativas/s` });
}

function cadeia(e) {
  const n = e.no || {};
  const p = n.prova_util || {};
  fatos("k-fatos", [
    ["Rede", `${e.rede?.nome} (${e.rede?.tipo})`],
    ["Altura", fmt(n.altura)],
    ["Ponta", el("span", { class: "num", title: n.ponta }, curto(n.ponta, 16))],
    ["Trabalho acumulado", el("span", { class: "num", title: n.trabalho }, compacto(Number(n.trabalho)))],
    ["Emitido", `${n.emitido} HYX de teste`],
    ["Recompensa do próximo bloco", `${n.recompensa} HYX`],
    ["Maturidade", `${fmt(n.maturidade)} blocos`],
    ["Prova de trabalho útil", `${p.familia} · ${p.n}×${p.n} · ${p.rodadas} rodadas (lado ${p.lado_min} a ${p.lado_max})`],
    ["Onde roda a prova útil", [el("span", {}, "CPU"), " ", selo("REAL", "a matriz do bloco é pequena: na CPU leva menos de 1 ms")]],
    ["Mempool", `${fmt(n.mempool)} transação(ões)`],
  ]);
  const blocos = (e.blocos || []).slice().reverse();
  tabela(
    "k-blocos",
    [{ t: "Altura", num: true }, { t: "Hash" }, { t: "Horário" }, { t: "Intervalo", num: true }, { t: "Txs", num: true }, { t: "Prova útil", num: true }, { t: "Dificuldade" }, { t: "" }],
    blocos.map((b) => [
      { v: fmt(b.altura), num: true },
      el("span", { class: "num", title: b.hash }, curto(b.hash, 16)),
      new Date(b.horario * 1000).toLocaleString("pt-BR", { hour12: false }),
      { v: b.intervalo ? `${fmt(b.intervalo)} s` : "—", num: true },
      { v: fmt(b.txs), num: true },
      { v: b.n ? `${b.n}×${b.n}` : "—", num: true },
      el("span", { class: "num" }, b.bits),
      b.meu ? "deste nó" : "",
    ]),
    { vazio: "só o bloco de origem por enquanto" },
  );
}

export function atualizar(e) {
  if (!e.mineracao) return;
  mineracao(e);
  cadeia(e);
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}
