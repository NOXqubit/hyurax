// Hyurax / Ultrax — histórico de um JOB: reprodução no tempo e inspeção de
// cada unidade (Documento Mestre, §7).
//
// Só o que foi gravado: os eventos do JOB (eventos.jsonl) e o registro de
// cada unidade conferida (unidades.jsonl), lidos de /api/v1/ciencia/job/ID/
// historico. A reprodução anda pelos eventos na ordem e no horário em que
// aconteceram; nada é interpolado nem inventado entre eles.

import { el, fmt, compacto, curto, hora } from "../util.js";
import { obter } from "../api.js";

// estado de uma unidade a partir de um evento
const ESTADO = {
  WORK_UNIT_ASSIGNED: "voo",
  WORK_UNIT_STARTED: "voo",
  WORK_UNIT_PROGRESS: "voo",
  WORK_UNIT_REJECTED: "voo",
  WORK_UNIT_REQUEUED: "fila",
  WORK_UNIT_RETRY: "fila",
  WORK_UNIT_DIVERGENT: "fila",
  WORK_UNIT_VERIFIED: "feita",
  WORK_UNIT_CONSENSUS: "feita",
  WORK_UNIT_DUPLICATE: "feita",
  WORK_UNIT_FAILED: "falha",
};

function cor(estado, tema) {
  if (estado === "feita") return tema.tinta;
  if (estado === "voo") return tema.gpu;
  if (estado === "falha") return null; // contorno
  return tema.fio;
}

function temaAtual() {
  const s = getComputedStyle(document.documentElement);
  return { tinta: s.getPropertyValue("--tinta").trim(), fio: s.getPropertyValue("--fio-forte").trim(), gpu: s.getPropertyValue("--gpu").trim() };
}

/**
 * Monta o bloco de histórico de um JOB dentro de `caixa`. `total` é o número
 * de unidades do JOB.
 */
export async function montarHistorico(caixa, jobId, total) {
  caixa.replaceChildren(el("p", { class: "nota" }, "lendo o histórico gravado…"));
  const r = await obter(`/ciencia/job/${jobId}/historico`);
  if (!r.ok) {
    caixa.replaceChildren(el("p", { class: "nota" }, "sem histórico gravado para este JOB."));
    return;
  }
  const eventos = (r.dados.eventos || []).filter((e) => Number.isFinite(e.timestamp)).sort((a, b) => a.timestamp - b.timestamp || a.seq - b.seq);
  const registros = r.dados.unidades || [];
  if (!eventos.length) {
    caixa.replaceChildren(el("p", { class: "nota" }, "o JOB ainda não tem eventos gravados."));
    return;
  }
  const n = Math.max(1, total);
  const caixas = Math.min(n, 400);
  const t0 = eventos[0].timestamp;
  const t1 = eventos[eventos.length - 1].timestamp;

  const regua = el("input", { type: "range", min: "0", max: String(eventos.length), step: "1", value: String(eventos.length), id: "h-regua", "aria-label": "Posição na linha do tempo dos eventos" });
  const tocar = el("button", { type: "button", class: "botao-leve" }, "Reproduzir");
  const quando = el("span", { class: "num" });
  const mapa = el("canvas", { class: "grafico", style: "height:180px;cursor:pointer", "aria-label": "Unidades do JOB no instante escolhido; clique numa para inspecionar" });
  const lista = el("ol", { class: "registro" });
  const inspecao = el("div", { class: "bloco", hidden: true });
  caixa.replaceChildren(
    el("div", { class: "acoes", style: "align-items:center" }, tocar, regua, quando),
    mapa,
    el("p", { class: "nota" }, `${eventos.length} evento(s) gravado(s) de ${hora(t0)} a ${hora(t1)}. Cheias: conferidas; azuis: calculando; contornadas: falha; apagadas: na fila. Clique numa unidade para ver o registro dela.`),
    lista,
    inspecao,
  );

  // estado de cada unidade depois dos k primeiros eventos
  function estadoAte(k) {
    const est = new Map();
    for (let i = 0; i < k; i++) {
      const e = eventos[i];
      const s = ESTADO[e.event];
      if (s && Number.isFinite(e.unit_index)) est.set(e.unit_index, s);
    }
    return est;
  }

  function desenhar(k) {
    const est = estadoAte(k);
    const tema = temaAtual();
    const esc = devicePixelRatio || 1;
    const w = (mapa.width = Math.round(mapa.clientWidth * esc));
    const h = (mapa.height = Math.round(mapa.clientHeight * esc));
    const ctx = mapa.getContext("2d");
    ctx.clearRect(0, 0, w, h);
    const col = Math.ceil(Math.sqrt((caixas * w) / Math.max(h, 1)));
    const lin = Math.ceil(caixas / col);
    const lado = Math.min(w / col, h / lin);
    mapa._geo = { col, lado, caixas };
    for (let c = 0; c < caixas; c++) {
      // uma caixa pode juntar várias unidades: vale o pior estado delas
      const de = Math.floor((c * n) / caixas);
      const ate = Math.max(de + 1, Math.floor(((c + 1) * n) / caixas));
      let s = "fila";
      for (let i = de; i < ate; i++) {
        const x = est.get(i) || "fila";
        if (x === "falha") { s = "falha"; break; }
        if (x === "voo") s = "voo";
        else if (x === "feita" && s === "fila") s = "feita";
      }
      const x = (c % col) * lado;
      const y = Math.floor(c / col) * lado;
      const k2 = cor(s, tema);
      if (k2) {
        ctx.fillStyle = k2;
        ctx.fillRect(x + 1, y + 1, lado - 2, lado - 2);
      } else {
        ctx.strokeStyle = tema.tinta;
        ctx.strokeRect(x + 1.5, y + 1.5, lado - 3, lado - 3);
      }
    }
    const atual = eventos[Math.max(0, k - 1)];
    quando.textContent = k === 0 ? "antes do primeiro evento" : `${hora(atual.timestamp)} · evento ${k} de ${eventos.length}`;
    lista.replaceChildren(
      ...eventos.slice(Math.max(0, k - 12), k).reverse().map((e) =>
        el("li", {}, el("span", {}, hora(e.timestamp)), el("span", { title: e.event }, e.event), el("span", {}, [Number.isFinite(e.unit_index) ? `unidade ${e.unit_index}` : "", e.verification_status || "", e.operation || ""].filter(Boolean).join(" · "))),
      ),
    );
  }

  function inspecionar(indice) {
    const regs = registros.filter((x) => x.indice === indice);
    const evs = eventos.filter((e) => e.unit_index === indice);
    inspecao.hidden = false;
    inspecao.replaceChildren(
      el("h3", {}, `Unidade ${indice}`),
      regs.length
        ? el(
            "dl",
            { class: "fatos" },
            ...regs.flatMap((u) => [
              el("dt", {}, "Worker"), el("dd", { class: "num", title: u.worker }, curto(u.worker, 16)),
              el("dt", {}, "Entrada (INPUT_HASH)"), el("dd", { class: "num", title: u.entrada }, curto(u.entrada, 16)),
              el("dt", {}, "Resultado (RESULT_HASH)"), el("dd", { class: "num", title: u.resultado }, curto(u.resultado, 16)),
              el("dt", {}, "Operações"), el("dd", { class: "num" }, `${compacto(u.operacoes)} + ${compacto(u.operacoes_verificacao)} de conferência`),
              el("dt", {}, "Tempo"), el("dd", { class: "num" }, `${fmt(u.ms_calculo)} ms de cálculo · ${fmt(u.ms_verificacao)} ms de conferência`),
              el("dt", {}, "Verificação"), el("dd", {}, `${u.verificacao} · ${u.metodo}`),
              el("dt", {}, "Recurso"), el("dd", {}, u.gpu ? `GPU (${u.gpu})` : `CPU${u.linha !== null && u.linha !== undefined ? `, linha ${u.linha}` : ""}`),
              el("dt", {}, "Programa"), el("dd", {}, u.programa || "versão não registrada (JOB de antes da 1.0)"),
            ]),
          )
        : el("p", { class: "nota" }, "sem registro conferido desta unidade (ainda na fila, em voo ou com falha)."),
      el("ol", { class: "registro" }, ...evs.map((e) => el("li", {}, el("span", {}, hora(e.timestamp)), el("span", {}, e.event), el("span", {}, [e.verification_status, e.operation].filter(Boolean).join(" · "))))),
    );
  }

  mapa.addEventListener("click", (ev) => {
    const g = mapa._geo;
    if (!g) return;
    const esc = devicePixelRatio || 1;
    const b = mapa.getBoundingClientRect();
    const cx = Math.floor(((ev.clientX - b.left) * esc) / g.lado);
    const cy = Math.floor(((ev.clientY - b.top) * esc) / g.lado);
    const c = cy * g.col + cx;
    if (c < 0 || c >= g.caixas) return;
    inspecionar(Math.floor((c * n) / g.caixas));
  });

  let tocando = null;
  function parar() {
    if (tocando) clearTimeout(tocando);
    tocando = null;
    tocar.textContent = "Reproduzir";
  }
  tocar.addEventListener("click", () => {
    if (tocando) return parar();
    let k = Number(regua.value) >= eventos.length ? 0 : Number(regua.value);
    tocar.textContent = "Parar";
    // a reprodução respeita o ritmo real dos eventos, acelerado para caber em
    // ~20 s, e nunca mais rápido que 60 ms por passo
    const escala = Math.max(1, (t1 - t0) / 20000);
    const passo = () => {
      k += 1;
      regua.value = String(k);
      desenhar(k);
      if (k >= eventos.length) return parar();
      const espera = Math.max(60, Math.min(1500, (eventos[k].timestamp - eventos[k - 1].timestamp) / escala));
      tocando = setTimeout(passo, espera);
    };
    passo();
  });
  regua.addEventListener("input", () => {
    parar();
    desenhar(Number(regua.value));
  });
  requestAnimationFrame(() => desenhar(eventos.length));
}
