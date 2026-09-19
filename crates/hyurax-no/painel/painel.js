// Hyurax · Minerador — lê o estado do nó a cada segundo e desenha o painel.
// Tudo o que aparece aqui vem de /api/estado: nada é simulado.

const $ = (id) => document.getElementById(id);
const fmt = new Intl.NumberFormat("pt-BR");
const fmt1 = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const calmo = matchMedia("(prefers-reduced-motion: reduce)").matches;

let ultimo = null;
let estacao = null;
let enviando = false;
const vistos = new Set();

// A estação 3D é opcional: sem WebGL, o resto do painel funciona igual.
import("./estacao.js")
  .then((m) => { estacao = m.criarEstacao($("cena"), { calmo }); if (ultimo) estacao.atualizar(ultimo); })
  .catch((e) => console.warn("estação 3D indisponível:", e));

function duracao(s) {
  s = Math.max(0, Math.floor(s));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
  return h ? `${h}h ${String(m).padStart(2, "0")}m` : m ? `${m}m ${String(x).padStart(2, "0")}s` : `${x}s`;
}
const hora = (unix) => new Date(unix * 1000).toLocaleTimeString("pt-BR", { hour12: false });
const curto = (h) => `${h.slice(0, 8)}…${h.slice(-6)}`;

function texto(id, v) {
  const el = $(id);
  if (el && el.textContent !== String(v)) el.textContent = v;
}

// ---------- comandos ----------
async function mandar(corpo) {
  if (enviando) return;
  enviando = true;
  try {
    await fetch("/api/minerar", {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: corpo,
    });
  } finally {
    enviando = false;
    ler();
  }
}
$("minerar").addEventListener("click", () => {
  if (!ultimo) return;
  mandar(`ligar=${ultimo.minerando ? 0 : 1}`);
});
$("menos").addEventListener("click", () => ultimo && mandar(`linhas=${ultimo.linhas - 1}`));
$("mais").addEventListener("click", () => ultimo && mandar(`linhas=${ultimo.linhas + 1}`));
$("copiar").addEventListener("click", async () => {
  if (!ultimo) return;
  try {
    await navigator.clipboard.writeText(ultimo.endereco);
    $("copiar").textContent = "copiado";
  } catch {
    getSelection().selectAllChildren($("endereco"));
    $("copiar").textContent = "selecionado";
  }
  setTimeout(() => ($("copiar").textContent = "copiar endereço"), 2000);
});

// ---------- desenho ----------
function desenharBarra(e) {
  texto("l-rede", e.rede.replace(/^hyurax-/, ""));
  texto("l-altura", fmt.format(e.altura));
  texto("l-ritmo", fmt1.format(e.ritmo));
  texto("l-pares", e.pares);
  texto("l-ligado", duracao(e.ligado_s));
  texto("linhas", e.linhas);
  texto("nucleos", e.nucleos);
  texto("memoria", fmt.format(e.memoria_mib * e.linhas));
  $("menos").disabled = e.linhas <= 1;
  $("mais").disabled = e.linhas >= e.nucleos;
  const b = $("minerar");
  b.setAttribute("aria-pressed", String(e.minerando));
  b.querySelector("span").textContent = e.minerando ? "MINERANDO" : "MINERAR";
  b.title = e.minerando ? "Clique para parar" : "Clique para começar a minerar";
  $("controles").hidden = !e.pode_mandar;
  $("so-ver").hidden = e.pode_mandar;
}

function desenharEstacao(e) {
  texto("e-tentativas", fmt.format(e.tentativas));
  texto("e-rodada", e.minerando && e.rodada_altura ? `bloco ${fmt.format(e.rodada_altura)} · ${duracao(e.rodada_s)}` : "—");
  texto("e-meus", `${fmt.format(e.meus_cadeia)}${e.meus ? ` · +${fmt.format(e.meus)} agora` : ""}`);
  texto("e-perdidos", fmt.format(e.perdidos));
  texto("estado-rodada", e.minerando ? `minerando o bloco ${fmt.format(e.rodada_altura || e.altura + 1)}` : "parado");
}

function desenharLateral(e) {
  texto("endereco", e.endereco);
  texto("saldo", e.saldo);
  texto("imaturo", e.imaturo);
  texto("recompensa", e.recompensa);
  texto("maturidade", e.maturidade);
  texto("r-nome", e.rede);
  texto("r-emitido", e.emitido);
  texto("r-mempool", e.mempool);
  texto("r-trabalho", e.trabalho);
  texto("r-ponta", curto(e.ponta));
  $("r-ponta").title = e.ponta;
}

function desenharLivro(e) {
  const corpo = $("livro");
  const linhas = [...e.blocos].reverse().map((b) => {
    const tr = document.createElement("tr");
    if (b.meu) tr.classList.add("meu");
    if (vistos.size && !vistos.has(b.hash)) {
      tr.classList.add("novo");
      setTimeout(() => tr.classList.remove("novo"), 1600);
    }
    const q = document.createElement("i");
    q.className = b.meu ? "q meu" : "q";
    const alt = document.createElement("td");
    alt.append(q, fmt.format(b.altura));
    const cel = (t) => Object.assign(document.createElement("td"), { textContent: t });
    const hash = cel(curto(b.hash));
    hash.title = b.hash;
    tr.append(
      alt,
      hash,
      cel(b.altura === 0 ? "gênese" : b.intervalo ? `${b.intervalo}s` : "—"),
      cel(b.n ? `${b.n}×${b.n}` : "—"),
      cel(b.txs),
    );
    return tr;
  });
  corpo.replaceChildren(...linhas);
  e.blocos.forEach((b) => vistos.add(b.hash));
}

function desenharFluxo(e) {
  const lista = $("fluxo");
  lista.replaceChildren(
    ...e.eventos.map((ev) => {
      const li = document.createElement("li");
      li.className = ev.tipo;
      const t = Object.assign(document.createElement("time"), { textContent: hora(ev.quando) });
      const tipo = Object.assign(document.createElement("span"), {
        className: "tipo",
        textContent: { "meu-bloco": "meu bloco", perdido: "perdido", minerador: "minerador", rede: "rede", no: "nó", erro: "erro" }[ev.tipo] || ev.tipo,
      });
      li.append(t, tipo, Object.assign(document.createElement("span"), { textContent: ev.texto }));
      return li;
    }),
  );
}

function desenharFita(e) {
  const trilho = $("fita-trilho");
  const partes = [...e.blocos].reverse().slice(0, 14).map((b) => {
    const s = document.createElement("span");
    const forte = Object.assign(document.createElement("b"), { textContent: `#${fmt.format(b.altura)}` });
    s.append(b.meu ? "■ " : "□ ", forte, ` ${curto(b.hash)} · ${b.n ? `${b.n}×${b.n}` : "gênese"} · ${b.txs} tx`);
    return s;
  });
  const chave = e.blocos.map((b) => b.hash).join();
  if (trilho.dataset.chave !== chave) {
    trilho.dataset.chave = chave;
    trilho.replaceChildren(...partes);
  }
}

// ---------- gráfico de ritmo ----------
function desenharGrafico(e) {
  const cv = $("grafico");
  const dpr = Math.min(devicePixelRatio || 1, 2);
  const w = cv.clientWidth, h = cv.clientHeight;
  if (!w || !h) return;
  if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) {
    cv.width = Math.round(w * dpr);
    cv.height = Math.round(h * dpr);
  }
  const g = cv.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  const a = e.amostras;
  // Tentativas por segundo, suavizadas em janela de 5 amostras.
  const serie = [];
  for (let i = 5; i < a.length; i++) {
    const dt = a[i][0] - a[i - 5][0];
    serie.push([a[i][0], dt > 0 ? (a[i][1] - a[i - 5][1]) / dt : 0]);
  }
  const m = { e: 44, d: 12, c: 12, b: 22 };
  const lw = w - m.e - m.d, lh = h - m.c - m.b;
  const maxV = Math.max(1, ...serie.map((p) => p[1])) * 1.15;
  const t1 = a.length ? a[a.length - 1][0] : 0, t0 = t1 - 120;
  const X = (t) => m.e + ((t - t0) / 120) * lw;
  const Y = (v) => m.c + lh - (v / maxV) * lh;

  g.font = "10px " + getComputedStyle(document.body).fontFamily;
  g.fillStyle = "rgba(243,243,241,0.38)";
  g.strokeStyle = "rgba(243,243,241,0.08)";
  g.lineWidth = 1;
  // Grade horizontal com 3 marcas no valor real.
  for (let k = 0; k <= 3; k++) {
    const v = (maxV / 1.15) * (k / 3);
    const y = Math.round(Y(v)) + 0.5;
    g.beginPath(); g.moveTo(m.e, y); g.lineTo(w - m.d, y); g.stroke();
    g.textAlign = "right"; g.textBaseline = "middle";
    g.fillText(fmt1.format(v), m.e - 6, y);
  }
  g.textAlign = "left"; g.textBaseline = "alphabetic";
  g.fillText("−2 min", m.e, h - 6);
  g.textAlign = "right";
  g.fillText("agora", w - m.d, h - 6);

  // Blocos que chegaram na janela: marca vertical, sólida se for meu.
  const inicio = e.agora - e.ligado_s;
  for (const b of e.blocos) {
    const t = b.horario - inicio;
    if (t < t0 || t > t1) continue;
    const x = Math.round(X(t)) + 0.5;
    g.strokeStyle = b.meu ? "rgba(243,243,241,0.9)" : "rgba(243,243,241,0.35)";
    g.setLineDash(b.meu ? [] : [3, 3]);
    g.beginPath(); g.moveTo(x, m.c); g.lineTo(x, m.c + lh); g.stroke();
  }
  g.setLineDash([]);
  if (serie.length > 1) {
    g.beginPath();
    serie.forEach(([t, v], i) => (i ? g.lineTo(X(t), Y(v)) : g.moveTo(X(t), Y(v))));
    g.strokeStyle = "#f3f3f1";
    g.lineWidth = 1.5;
    g.stroke();
    g.lineTo(X(serie[serie.length - 1][0]), m.c + lh);
    g.lineTo(X(serie[0][0]), m.c + lh);
    g.closePath();
    g.fillStyle = "rgba(243,243,241,0.06)";
    g.fill();
  }
  texto("g-atual", fmt1.format(e.ritmo));
}

// ---------- ciclo ----------
async function ler() {
  try {
    const r = await fetch("/api/estado", { cache: "no-store" });
    const e = await r.json();
    ultimo = e;
    $("desligado").hidden = true;
    desenharBarra(e);
    desenharEstacao(e);
    desenharLateral(e);
    desenharLivro(e);
    desenharFluxo(e);
    desenharFita(e);
    desenharGrafico(e);
    estacao?.atualizar(e);
  } catch {
    $("desligado").hidden = false;
  }
}
setInterval(() => { texto("relogio", new Date().toLocaleTimeString("pt-BR", { hour12: false })); }, 1000);
setInterval(ler, 1000);
addEventListener("resize", () => ultimo && desenharGrafico(ultimo));
ler();
