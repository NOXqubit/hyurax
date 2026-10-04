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
import { fatos, linhaDeRegistro, grafico, cores, metricas, creditos, marcarEstado, estadoEl, bytes } from "./comum.js";

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
    ["Rede (enviado · recebido)", m.rede_enviados ? [el("span", { class: "num" }, `${bytes(m.rede_enviados.valor)} · ${bytes(m.rede_recebidos?.valor ?? 0)}`), " ", selo(m.rede_enviados.origem, m.rede_enviados.fonte)] : valorComOrigem(null)],
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

/**
 * Os alertas do nó, todos tirados do estado real: [tipo, título, detalhe].
 * O app usa a mesma lista para o contador da navegação.
 */
export function alertasDe(e) {
  const a = [];
  if (!e) return a;
  if (!e.no?.pares) a.push(["atencao", "Sem pares conectados", "Sem outro nó, a cadeia não anda e a nuvem fica vazia. Veja Rede (sementes, porta, vizinhos)."]);
  else if (!e.no?.sincronizado) a.push(["em-curso", "Sincronizando a cadeia", `${e.no.pares} par(es) conectado(s).`]);
  if (!e.carteira?.existe) a.push(["atencao", "Nenhuma carteira", "Crie ou importe uma em Carteira para minerar e receber HYX de teste."]);
  else if (e.carteira?.sem_senha) a.push(["falha", "Carteira sem senha", "O segredo está em texto no disco. Proteja em Carteira."]);
  if (e.nuvem?.em_risco) a.push(["falha", `${e.nuvem.em_risco} arquivo(s) em risco`, "Menos fragmentos guardados que o necessário para reconstruir. Veja Armazenamento."]);
  if (e.nuvem?.degradados) a.push(["atencao", `${e.nuvem.degradados} arquivo(s) em reparo`, "Um guardião sumiu ou falhou; o fragmento está sendo reconstruído."]);
  if (e.nuvem && e.nuvem.livro_integro === false) a.push(["falha", "Livro de contas adulterado", "A conferência por hash quebrou. Veja Faturamento."]);
  if (e.atualizacao?.disponivel) a.push(["em-curso", `Versão ${e.atualizacao.disponivel.versao} disponível`, "Manifesto assinado conferido. Instale em Ajustes → Atualização."]);
  if (e.atualizacao?.erro) a.push(["atencao", "Busca de atualização falhou", e.atualizacao.erro]);
  return a;
}

function painel(e) {
  const m = e.metricas || {};
  const pct = (v) => (v?.valor == null ? "—" : fmt(v.valor, 0));
  const online = e.no?.pares > 0;
  marcarEstado("vg-estado", online ? "ok" : "atencao", online ? `online · ${e.no.pares} par(es)` : "sem pares");
  metricas(
    "vg-metricas",
    [
      { rotulo: "CPU da máquina", valor: pct(m.cpu_total), unidade: "%", origem: m.cpu_total?.origem, fonte: m.cpu_total?.fonte, nota: `programa: ${pct(m.cpu_processo)}%` },
      { rotulo: "RAM livre", valor: m.ram_livre?.valor == null ? "—" : fmt(m.ram_livre.valor / 1024, 1), unidade: "GiB", origem: m.ram_livre?.origem, fonte: m.ram_livre?.fonte },
      { rotulo: "GPU 3D", valor: pct(m.gpu_3d), unidade: "%", origem: m.gpu_3d?.origem || "PENDENTE", fonte: m.gpu_3d?.fonte, nota: "todos os programas" },
      { rotulo: "Temperatura", valor: "—", origem: "PENDENTE", nota: "sem leitura confiável sem administrador" },
      { rotulo: "JOBs ativos", valor: fmt(e.jobs?.ativos ?? 0), nota: `${fmt(e.jobs?.total ?? 0)} no total` },
      { rotulo: "Créditos consumidos", valor: creditos(e.jobs?.consumo_mili ?? 0), nota: "créditos de computação, não HYX" },
      { rotulo: "Arquivos na rede", valor: fmt(e.nuvem?.arquivos ?? 0), nota: e.nuvem?.guarda?.usado_bytes ? `guardando ${bytes(e.nuvem.guarda.usado_bytes)} de outros` : "cifrados aqui, em vários nós" },
      { rotulo: "Altura da cadeia", valor: fmt(e.no?.altura), nota: e.no?.sincronizado ? "alcançou os pares" : "rede de teste" },
    ],
    selo,
  );
  const al = alertasDe(e);
  trocar(
    "vg-alertas",
    al.length
      ? al.map(([tipo, titulo, det]) => el("li", {}, estadoEl(tipo, tipo === "falha" ? "falha" : tipo === "atencao" ? "atenção" : "em curso"), el("span", {}, titulo), el("small", {}, det)))
      : [el("li", {}, estadoEl("ok", "tudo certo"), el("span", {}, "Nenhum alerta agora"), el("small", {}, "Pares conectados, carteira protegida e arquivos íntegros."))],
  );
  const nv = e.nuvem || {};
  fatos("vg-nuvem", [
    ["Esta máquina no mercado", nv.anunciando ? "anunciada" : "não anunciada"],
    ["Máquinas vistas", fmt(nv.anuncios ?? 0)],
    ["Arquivos na rede", `${fmt(nv.arquivos ?? 0)}${nv.degradados ? ` · ${nv.degradados} em reparo` : ""}`],
    ["Guardando para outros", nv.guarda ? `${bytes(nv.guarda.usado_bytes)} · ${fmt(nv.guarda.fragmentos)} fragmento(s)` : "—"],
    ["Aluguéis", fmt(nv.alugueis ?? 0)],
    ["Consumo no livro", creditos(nv.consumo_mili ?? 0)],
    ["A receber (recibos)", creditos(nv.a_receber_mili ?? 0)],
  ]);
}

export function atualizar(e) {
  painel(e);
  maquina(e);
  cartoes(e);
  linhaDeLeitura();
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}
