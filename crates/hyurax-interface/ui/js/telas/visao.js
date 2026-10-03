// Hyurax / Ultrax — Visão geral: o trabalho em foco (linha de leitura + cena
// 3D), esta máquina e os cartões de nó, carteira, mineração e ULTRAX.
//
// A cena só desenha amostras reais que os motores entregam pelo fluxo
// ("amostra"). A linha de leitura diz, do começo ao fim, o que é a tarefa,
// onde roda, o que já saiu dela, como foi conferida e o que ela muda na rede
// — sem prometer o que não acontece.

import { $, el, texto, fmt, compacto, curto, selo, valorComOrigem, trocar } from "../util.js";
import { estado, ouvir } from "../estado.js";
import { Cena } from "../cena/visualizadores.js";
import { fatos, linhaDeRegistro, grafico, cores } from "./comum.js";

let cena = null;
let info = null; // o resumo do que está na cena agora
let gpuNome = "";

function linhaDeLeitura() {
  const e = estado.atual;
  if (!info) {
    const u = e?.ultrax;
    texto("l-trabalho", u?.ligado ? "esperando a primeira amostra" : "nenhum");
    texto("l-trabalho-nota", u?.ligado ? "o ULTRAX está ligado" : "o ULTRAX está desligado");
    for (const id of ["como", "recurso", "resultado", "verificacao", "impacto"]) {
      texto(`l-${id}`, "—");
      texto(`l-${id}-nota`, "");
    }
    return;
  }
  const c = info.contexto;
  const ativa = (e?.ultrax?.ativas || []).find((a) => a.numero === c.tarefa);
  texto("l-trabalho", info.nome);
  texto("l-trabalho-nota", c.lab ? `tarefa #${c.tarefa} · carga LAB: entrada gerada nesta máquina (SIMULADO)` : `tarefa #${c.tarefa} · unidade ${c.unidade} do JOB ${curto(c.job, 10)} (pedido de verdade)`);
  texto("l-como", ativa?.metodo || c.resumo);
  texto("l-como-nota", ativa ? c.resumo : c.desafio ? "tarefa-desafio: a resposta já é conhecida pelo verificador" : "");
  texto("l-recurso", c.recurso === "GPU" ? `GPU · ${gpuNome || e?.ultrax?.gpu?.nome || "WebGL 2"}` : `CPU · linha ${c.linha}`);
  const m = e?.metricas;
  texto(
    "l-recurso-nota",
    c.recurso === "GPU"
      ? `GPU 3D da máquina: ${m?.gpu_3d?.valor != null ? `${fmt(m.gpu_3d.valor, 0)}% (REAL, todos os programas)` : "não medida"}`
      : `CPU do programa: ${m?.cpu_processo?.valor != null ? `${fmt(m.cpu_processo.valor, 1)}% da máquina (REAL)` : "não medida"} · limite ${e?.ultrax?.uso_cpu ?? "—"}% por linha (AJUSTE)`,
  );
  if (ativa) {
    texto("l-resultado", `${fmt((ativa.feitas / Math.max(1, ativa.total)) * 100, 0)}% calculado`);
    texto("l-resultado-nota", `${compacto(ativa.feitas)} de ${compacto(ativa.total)} operações · ${info.amostras} amostra(s) recebida(s)`);
  } else {
    texto("l-resultado", info.verificacao ? "entregue" : "em cálculo");
    texto("l-resultado-nota", `${info.amostras} amostra(s) do motor recebida(s)`);
  }
  const v = info.verificacao;
  if (!v) {
    texto("l-verificacao", ativa?.estado === "VERIFYING" ? "conferindo" : "depois do cálculo");
    texto("l-verificacao-nota", c.recurso === "GPU" ? "a CPU confere o resultado da GPU por Freivalds" : "recomputação ou prova do próprio motor");
  } else {
    const passou = /PASSED|SETTLEMENT/.test(v.evento);
    const falhou = /FAILED|EXPIRED|CANCELLED|ABANDONED/.test(v.evento);
    texto("l-verificacao", passou ? "aprovada" : falhou ? "não aprovada" : "conferindo");
    texto("l-verificacao-nota", `${v.evento}${v.detalhe ? ` · ${v.detalhe}` : ""}`);
  }
  if (c.lab) {
    texto("l-impacto", "nenhum na cadeia");
    texto("l-impacto-nota", "conta só no placar local (Work Score não é dinheiro nem HYX); trabalho na recompensa do bloco: PENDENTE (SPEC-02)");
  } else {
    texto("l-impacto", "soma no JOB");
    texto("l-impacto-nota", "gera créditos de computação (não são HYX); não muda a recompensa do bloco: PENDENTE (SPEC-02)");
  }
}

function aoMudar(i) {
  info = i;
  $("cena-vazia").hidden = true;
  const c = i.contexto;
  const s = $("cena-selo");
  s.hidden = false;
  s.replaceChildren(
    el("span", { class: c.recurso === "GPU" ? "recurso-gpu" : null }, c.recurso),
    ` · tarefa #${c.tarefa} · `,
    c.lab ? selo("SIMULADO", "carga LAB: entrada gerada nesta máquina; a conta é de verdade") : selo("REAL", `unidade ${c.unidade} do JOB ${curto(c.job, 10)}`),
    i.seguindo ? " · seguindo a mais recente" : "",
  );
  texto("cena-titulo", i.nome);
  trocar("cena-legenda", i.legenda.map((t) => el("li", {}, t)));
  atualizarFoco();
  linhaDeLeitura();
}

function atualizarFoco() {
  const sel = $("cena-foco");
  if (!cena || document.activeElement === sel) return;
  const lista = cena.lista();
  const atual = cena.seguir ? "" : cena.foco;
  const opcoes = [el("option", { value: "" }, "seguir a mais recente")].concat(
    lista.map((t) => el("option", { value: t.chave }, `${t.contexto.recurso} #${t.contexto.tarefa} · ${t.tipo}${t.contexto.lab ? " (LAB)" : " (JOB)"}`)),
  );
  sel.replaceChildren(...opcoes);
  sel.value = atual || "";
}

function maquina(e) {
  const m = e.metricas;
  if (!m) return;
  const s = m.sistema || {};
  const gpus = (s.gpus || []).map((g) => `${g.nome}${g.memoria_mib ? ` (${fmt(g.memoria_mib)} MiB)` : ""} · ${g.tipo}`).join("; ");
  fatos("maquina-fatos", [
    ["Processador", s.cpu || "—"],
    ["Núcleos", `${s.nucleos_fisicos ?? "—"} físicos · ${s.nucleos_logicos ?? "—"} lógicos`],
    ["Memória", s.ram_total_mib ? `${fmt(s.ram_total_mib / 1024, 1)} GiB` : "—"],
    ["GPU", gpus ? [gpus, " ", selo("DERIVADO", "tipo pelo nome do adaptador")] : "nenhuma detectada"],
    ["CPU da máquina", valorComOrigem(m.cpu_total, 0)],
    ["CPU deste programa", valorComOrigem(m.cpu_processo, 1)],
    ["RAM usada", valorComOrigem(m.ram_usada, 0)],
    ["RAM deste programa", valorComOrigem(m.ram_processo, 0)],
    ["GPU 3D", valorComOrigem(m.gpu_3d, 0)],
    ["GPU cálculo", valorComOrigem(m.gpu_calculo, 0)],
    ["Memória dedicada da GPU", valorComOrigem(m.gpu_memoria_dedicada, 0)],
    ["Memória compartilhada da GPU", valorComOrigem(m.gpu_memoria_compartilhada, 0)],
    ["Temperatura", valorComOrigem(m.temperatura)],
    ["Energia", valorComOrigem(m.energia, 0)],
    ["Custo por mês", valorComOrigem(m.custo_mes, 2)],
  ]);
  texto("maquina-nota", `${m.fonte || ""}${m.problema ? ` · ${m.problema}` : ""} · linhas: máquina (cheia), programa (tracejada), GPU 3D (azul)`);
  const h = m.historico || [];
  const cor = cores();
  grafico("maquina-grafico", [
    { pontos: h.filter((l) => l[1] != null).map((l) => [l[0], l[1]]), cor: cor.tinta },
    { pontos: h.filter((l) => l[2] != null).map((l) => [l[0], l[2]]), cor: cor.tinta, tracejado: true },
    { pontos: h.filter((l) => l[3] != null).map((l) => [l[0], l[3]]), cor: cor.gpu },
  ], { max: 100, rotulo: () => "100%" });
}

function cartoes(e) {
  const no = e.no || {};
  fatos("vg-no", [
    ["Rede", `${e.rede?.nome} · ${e.rede?.tipo}`],
    ["Altura", fmt(no.altura)],
    ["Pares", fmt(no.pares)],
    ["Sincronização", no.pares ? (no.sincronizado ? "alcançou os pares" : "sincronizando") : "sem pares"],
    ["Mempool", `${fmt(no.mempool)} tx`],
    ["Próxima recompensa", `${no.recompensa} HYX de teste`],
    ["Prova útil", `matriz ${no.prova_util?.n}×${no.prova_util?.n}`],
  ]);
  const c = e.carteira || {};
  fatos("vg-carteira", c.existe
    ? [
        ["Saldo", `${c.saldo} HYX`],
        ["Imaturo", `${c.imaturo} HYX`],
        ["Endereço", el("span", { class: "num", title: c.endereco }, curto(c.endereco, 18))],
        ["Segundo fator", c.seguranca?.ligado ? "ligado" : "desligado"],
      ]
    : [["Carteira", "nenhuma neste computador"]]);
  const mi = e.mineracao || {};
  fatos("vg-mineracao", [
    ["Estado", mi.ligada ? "minerando" : "parada"],
    ["Linhas", `${mi.linhas} de ${mi.nucleos} núcleos`],
    ["Limite por linha", valorComOrigem(mi.limite_cpu, 0)],
    ["Ritmo", valorComOrigem(mi.ritmo, 2)],
    ["Blocos meus", `${fmt(mi.meus_sessao)} nesta sessão · ${fmt(mi.meus_cadeia)} na cadeia`],
    ["Perdidos na corrida", fmt(mi.perdidos)],
  ]);
  const u = e.ultrax || {};
  const p = u.placar || {};
  fatos("vg-ultrax", [
    ["Estado", u.ligado ? `ligado · ${u.modo}` : "desligado"],
    ["Linhas", `${u.linhas} × limite ${u.uso_cpu}%`],
    ["Executando", `${fmt(u.ativas?.length || 0)} · fila ${fmt(u.fila)}`],
    ["Liquidadas", fmt(p.liquidadas)],
    ["Recusadas", fmt(p.recusadas)],
    ["Desafios", `${fmt(p.desafios_certos)} certos · ${fmt(p.desafios_errados)} errados`],
    ["Work Score", [el("span", { class: "num" }, p.work_score ?? "—"), " ", selo("DERIVADO", "operações verificadas; não é dinheiro nem HYX")]],
    ["GPU", u.gpu?.ligada ? `ligada · ${u.gpu.tarefas} tarefa(s)` : "desligada"],
  ]);
  trocar("vg-registro", (e.registros || []).slice(0, 14).map((r) => linhaDeRegistro(r.quando_ms, r.dados?.categoria, r.dados?.texto)));
}

export function montar() {
  cena = new Cena($("cena"), aoMudar);
  if (!cena.ok) {
    $("cena-vazia").textContent = "WebGL 2 indisponível nesta janela: a visualização 3D fica desligada. O cálculo, a verificação e o resto do painel continuam iguais.";
  }
  ouvir("amostra", (d) => cena.receber(d));
  ouvir("tarefa", (ev) => cena.passoDeTarefa(ev));
  ouvir("gpu", (d) => { if (d.nome) gpuNome = d.nome; });
  $("cena-foco").addEventListener("change", (ev) => {
    const v = ev.target.value;
    if (v) cena.focar(v);
    else cena.seguirAMaisRecente();
  });
  $("cena-reenquadrar").addEventListener("click", () => cena.motor.reenquadrar());
}

export function atualizar(e) {
  maquina(e);
  cartoes(e);
  linhaDeLeitura();
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}
