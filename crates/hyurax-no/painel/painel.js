// Hyurax — lê o estado do nó a cada segundo e desenha o painel.
// Tudo o que aparece aqui vem de /api/estado: nada é simulado.

import { desenharMolecula, desenharCurva } from "./moleculas.js";

const $ = (id) => document.getElementById(id);
const fmt = new Intl.NumberFormat("pt-BR");
const fmt1 = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const calmo = matchMedia("(prefers-reduced-motion: reduce)").matches;

let ultimo = null;
let enviando = false;
const vistos = new Set();

function duracao(s) {
  s = Math.max(0, Math.floor(s));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60;
  return h ? `${h}h ${String(m).padStart(2, "0")}m` : m ? `${m}m ${String(x).padStart(2, "0")}s` : `${x}s`;
}
// Intervalo entre blocos: segundos para o ritmo normal, e forma curta quando
// é longo (o primeiro bloco conta o tempo desde a gênese, que é fixa e antiga).
function intervalo(s) {
  if (s < 120) return `${s}s`;
  if (s < 7200) return `${Math.round(s / 60)}min`;
  if (s < 172800) return `${Math.round(s / 3600)}h`;
  return `${Math.round(s / 86400)}d`;
}
const hora = (unix) => new Date(unix * 1000).toLocaleTimeString("pt-BR", { hour12: false });
const dia = (unix) => new Date(unix * 1000).toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" });
const curto = (h) => `${h.slice(0, 8)}…${h.slice(-6)}`;
// O nó manda o dinheiro com ponto; em português se escreve com vírgula.
const reais = (v) => String(v || "0.00").replace(".", ",");
// HYX sem os zeros que não dizem nada: 1.50000000 vira 1,5.
function moeda(v) {
  const t = String(v ?? "0");
  if (!t.includes(".")) return t;
  const [inteiro, casas] = t.split(".");
  const enxuto = casas.replace(/0+$/, "");
  return enxuto ? `${inteiro},${enxuto}` : inteiro;
}

function texto(id, v) {
  const el = $(id);
  if (el && el.textContent !== String(v)) el.textContent = v;
}

// ---------- comandos ----------
// POST de formulário (só vale do próprio computador). Nunca lança:
// devolve { ok, status, dados }, com dados = JSON da resposta ou null.
async function postar(caminho, campos = "") {
  try {
    const r = await fetch(caminho, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams(campos),
    });
    const dados = r.status === 204 ? null : await r.json().catch(() => null);
    return { ok: r.ok, status: r.status, dados };
  } catch {
    return { ok: false, status: 0, dados: null };
  }
}

// O nó explica o erro em {"erro": "..."}; sem explicação, uma frase honesta.
function erroDe(res, padrao) {
  const e = res.dados && res.dados.erro;
  if (typeof e === "string" && e.trim()) return e;
  if (res.status === 0) return "Sem resposta do nó. O programa ainda está aberto?";
  if (res.status === 403) return "Esse comando só vale no computador que roda o programa.";
  // O nó de terminal (app: false) não tem as rotas de carteira, sementes e pasta.
  if (res.status === 404) return ultimo?.app === false
    ? "Isso só funciona no programa com janela do Hyurax."
    : "Esta versão do programa ainda não faz isso.";
  return padrao;
}

// Enquanto o pedido anda, o botão ignora cliques sem perder o foco
// (com disabled, o foco do teclado cairia fora do diálogo).
async function comBotao(botao, tarefa) {
  if (!botao || botao.getAttribute("aria-disabled") === "true") return;
  botao.setAttribute("aria-disabled", "true");
  try {
    await tarefa();
  } finally {
    botao.removeAttribute("aria-disabled");
  }
}

async function mandar(corpo) {
  if (enviando) return;
  enviando = true;
  await postar("/api/minerar", corpo);
  enviando = false;
  ler();
}
$("minerar").addEventListener("click", () => {
  // Backend antigo não manda pode_minerar: conta como "pode".
  if (!ultimo || ultimo.pode_minerar === false) return;
  mandar(`ligar=${ultimo.minerando ? 0 : 1}`);
});
$("menos").addEventListener("click", () => ultimo && mandar(`linhas=${ultimo.linhas - 1}`));
$("mais").addEventListener("click", () => ultimo && mandar(`linhas=${ultimo.linhas + 1}`));

// Copiar um texto sem depender da área de transferência: em webview ela pode
// não existir, e aí a segunda melhor coisa é selecionar para a pessoa copiar.
async function copiar(valor, alvo, botao, rotulo) {
  if (!valor) return;
  try {
    await navigator.clipboard.writeText(valor);
    botao.textContent = "copiado";
  } catch {
    if (alvo) getSelection().selectAllChildren(alvo);
    botao.textContent = "selecionado";
  }
  setTimeout(() => (botao.textContent = rotulo), 2000);
}
$("copiar").addEventListener("click", () => copiar(ultimo?.endereco, $("endereco"), $("copiar"), "copiar endereço"));

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
  const podeMinerar = e.pode_minerar !== false; // backend antigo não manda o campo
  b.disabled = !podeMinerar;
  b.setAttribute("aria-pressed", String(e.minerando));
  b.querySelector("span").textContent = e.minerando ? "MINERANDO" : "MINERAR";
  b.title = !podeMinerar
    ? "Sem carteira ainda: crie ou importe uma para minerar. A recompensa precisa de um endereço."
    : e.minerando ? "Clique para parar" : "Clique para começar a minerar";
  $("controles").hidden = !e.pode_mandar;
  $("so-ver").hidden = e.pode_mandar;
  texto("marca-rede", e.rede && e.rede.includes("mainnet") ? "rede principal" : "rede de teste");
}

function desenharMineracao(e) {
  texto("e-tentativas", fmt.format(e.tentativas));
  texto("e-rodada", e.minerando && e.rodada_altura ? `bloco ${fmt.format(e.rodada_altura)} · ${duracao(e.rodada_s)}` : "—");
  texto("e-meus", `${fmt.format(e.meus_cadeia)}${e.meus ? ` · +${fmt.format(e.meus)} agora` : ""}`);
  texto("e-perdidos", fmt.format(e.perdidos));
  const w = typeof e.watts === "number" ? e.watts : 0;
  texto("e-energia", e.minerando ? `${fmt1.format(w)} W · R$ ${reais(e.custo_mes)}/mês` : "0 W");
  texto("e-limite", typeof e.uso_cpu === "number" ? `${e.uso_cpu}%` : "—");
  texto("estado-rodada", e.minerando ? `minerando o bloco ${fmt.format(e.rodada_altura || e.altura + 1)}` : "parado");
}

function desenharLateral(e) {
  texto("endereco", e.endereco || "sem carteira");
  $("copiar").hidden = !e.endereco;
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
  const podeEnviar = e.pode_enviar === true;
  $("abrir-enviar").disabled = !podeEnviar;
  $("abrir-enviar").title = podeEnviar
    ? "Assinar e mandar HYX para outro endereço"
    : "Sem saldo gastável ainda. A recompensa de mineração libera depois da maturidade.";
  $("abrir-receber").disabled = !e.endereco;
  $("abrir-seguranca").disabled = e.pode_mandar !== true;
  desenharMovimentos(e);
}

// Os movimentos da carteira: o que está esperando primeiro, com sinal + e −.
function desenharMovimentos(e) {
  const lista = Array.isArray(e.historico) ? e.historico : [];
  $("sem-movimentos").hidden = lista.length > 0;
  $("movimentos").replaceChildren(...lista.map((m) => {
    const li = document.createElement("li");
    li.className = `${m.entrada ? "entrada" : "saida"}${m.pendente ? " esperando" : ""}`;
    const sinal = Object.assign(document.createElement("span"), { className: "sinal", textContent: m.entrada ? "+" : "−" });
    const quem = document.createElement("span");
    quem.className = "quem";
    const quando = m.pendente ? "esperando entrar num bloco" : `${dia(m.quando)} ${hora(m.quando)}`;
    quem.textContent = m.tipo === "recompensa"
      ? `bloco ${fmt.format(m.altura)} minerado · ${quando}`
      : `${m.entrada ? "de" : "para"} ${curto(m.outro || "")} · ${quando}`;
    quem.title = m.txid ? `transação ${m.txid}` : "";
    const quanto = Object.assign(document.createElement("span"), { className: "quanto", textContent: `${moeda(m.valor)} HYX` });
    li.append(sinal, quem, quanto);
    return li;
  }));
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
      cel(b.altura === 0 ? "gênese" : b.intervalo ? intervalo(b.intervalo) : "—"),
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
        textContent: { "meu-bloco": "meu bloco", perdido: "perdido", minerador: "minerador", rede: "rede", no: "nó", erro: "erro", enviado: "enviado", seguranca: "segurança", carteira: "carteira", mercado: "mercado", painel: "painel" }[ev.tipo] || ev.tipo,
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

// ---------- gráfico de ritmo, em barras ----------
// No estilo das telas de bolsa: cada barra é uma janela de 6 segundos. O corpo
// vai da primeira à última leitura da janela, e o risco fino mostra o mínimo e
// o máximo. Barra cheia = o ritmo subiu na janela; vazada = caiu.
const SEGUNDOS_POR_BARRA = 6;
const JANELA_S = 120;

function barrasDoRitmo(amostras) {
  const taxas = [];
  for (let i = 1; i < amostras.length; i++) {
    const dt = amostras[i][0] - amostras[i - 1][0];
    if (dt > 0) taxas.push([amostras[i][0], (amostras[i][1] - amostras[i - 1][1]) / dt]);
  }
  const barras = [];
  for (const [t, v] of taxas) {
    const cesto = Math.floor(t / SEGUNDOS_POR_BARRA);
    const ultima = barras[barras.length - 1];
    if (ultima && ultima.cesto === cesto) {
      ultima.fecha = v;
      ultima.alta = Math.max(ultima.alta, v);
      ultima.baixa = Math.min(ultima.baixa, v);
    } else {
      barras.push({ cesto, abre: v, fecha: v, alta: v, baixa: v });
    }
  }
  return barras;
}

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

  const m = { e: 46, d: 12, c: 12, b: 22 };
  const lw = w - m.e - m.d, lh = h - m.c - m.b;
  const t1 = e.amostras.length ? e.amostras[e.amostras.length - 1][0] : 0;
  const t0 = t1 - JANELA_S;
  const barras = barrasDoRitmo(e.amostras).filter((b) => b.cesto * SEGUNDOS_POR_BARRA >= t0 - SEGUNDOS_POR_BARRA);
  const maxV = Math.max(1, ...barras.map((b) => b.alta)) * 1.15;
  const X = (t) => m.e + ((t - t0) / JANELA_S) * lw;
  const Y = (v) => m.c + lh - (v / maxV) * lh;
  const largura = Math.max(3, lw / (JANELA_S / SEGUNDOS_POR_BARRA) - 3);

  g.font = "10px " + getComputedStyle(document.body).fontFamily;
  g.lineWidth = 1;
  for (let k = 0; k <= 3; k++) {
    const v = (maxV / 1.15) * (k / 3);
    const y = Math.round(Y(v)) + 0.5;
    g.strokeStyle = "rgba(243,243,241,0.08)";
    g.beginPath(); g.moveTo(m.e, y); g.lineTo(w - m.d, y); g.stroke();
    g.fillStyle = "rgba(243,243,241,0.38)";
    g.textAlign = "right"; g.textBaseline = "middle";
    g.fillText(fmt1.format(v), m.e - 6, y);
  }
  g.textAlign = "left"; g.textBaseline = "alphabetic";
  g.fillText("−2 min", m.e, h - 6);
  g.textAlign = "right";
  g.fillText("agora", w - m.d, h - 6);

  // Blocos achados na janela: risco vertical, cheio se foi meu.
  const nasceu = e.agora - e.ligado_s;
  for (const b of e.blocos) {
    const t = b.horario - nasceu;
    if (t < t0 || t > t1) continue;
    const x = Math.round(X(t)) + 0.5;
    g.strokeStyle = b.meu ? "rgba(243,243,241,0.75)" : "rgba(243,243,241,0.3)";
    g.setLineDash(b.meu ? [] : [3, 3]);
    g.beginPath(); g.moveTo(x, m.c); g.lineTo(x, m.c + lh); g.stroke();
  }
  g.setLineDash([]);

  for (const barra of barras) {
    const x = X(barra.cesto * SEGUNDOS_POR_BARRA + SEGUNDOS_POR_BARRA / 2);
    if (x < m.e - largura || x > w - m.d + largura) continue;
    const subiu = barra.fecha >= barra.abre;
    const topo = Y(Math.max(barra.abre, barra.fecha));
    const base = Y(Math.min(barra.abre, barra.fecha));
    g.strokeStyle = "rgba(243,243,241,0.55)";
    g.beginPath();
    g.moveTo(Math.round(x) + 0.5, Y(barra.alta));
    g.lineTo(Math.round(x) + 0.5, Y(barra.baixa));
    g.stroke();
    const alturaCorpo = Math.max(1.5, base - topo);
    const esq = Math.round(x - largura / 2) + 0.5;
    if (subiu) {
      g.fillStyle = "#f3f3f1";
      g.fillRect(esq, topo, largura, alturaCorpo);
    } else {
      g.strokeStyle = "#f3f3f1";
      g.strokeRect(esq, topo, largura, alturaCorpo);
    }
  }
  texto("g-atual", fmt1.format(e.ritmo));
}

// ---------- QR Code ----------
// A biblioteca é a mesma do site, servida pelo próprio programa. createSvgTag
// devolve SVG montado por ela, sem nada de fora.
function desenharQr(caixa, valor) {
  if (!caixa) return;
  if (caixa.dataset.valor === valor) return;
  caixa.dataset.valor = valor;
  caixa.replaceChildren();
  if (!valor || typeof globalThis.qrcode !== "function") return;
  try {
    const q = globalThis.qrcode(0, "M");
    q.addData(valor);
    q.make();
    caixa.innerHTML = q.createSvgTag({ cellSize: 4, margin: 1, scalable: true });
  } catch (erro) {
    console.warn("QR Code não desenhado:", erro);
  }
}

// ---------- a abertura do programa ----------
// Uma tela só, do primeiro quadro até o painel: a partida, o cadeado e, na
// primeira vez, os três passos da carteira. Sem carteira não há para onde
// mandar a recompensa, então essa parte não tem como ser pulada.
const SENHA_MINIMA = 10;
const PEDIDO_MAXIMO = 4000; // folga: o nó recusa corpo grande, e um carteira.txt tem menos de 1 KiB
const boot = $("boot");
let bootCena = "partida"; // partida · trava · 1 · 2 · 3
let bootOcupado = false;
let carteiraFeita = false;
let bootEncerrado = false;

const caracteres = (s) => [...s].length;

function mostrarCena(nome) {
  bootCena = nome;
  for (const id of ["termos", "partida", "trava", "1", "2", "3"]) {
    const cena = $(`cena-${id}`);
    if (cena) cena.hidden = id !== nome;
  }
  const numero = Number(nome);
  const passos = $("boot-passos");
  passos.hidden = !numero;
  passos.querySelectorAll("li").forEach((li) => {
    const n = Number(li.dataset.passo);
    li.classList.toggle("agora", n === numero);
    li.classList.toggle("feito", numero > n);
  });
  $("boot-avanco").style.width = numero ? `${(numero / 3) * 100}%` : "0";
  boot.hidden = false;
}

function fecharBoot() {
  if (boot.hidden) return;
  boot.hidden = true;
  bootEncerrado = true;
}

function acenderRegistro(acesa) {
  $("boot-registro").querySelectorAll("li").forEach((li) => li.classList.toggle("pronta", acesa));
}

function desenharBoot(e) {
  // Numa janela destacada não se faz abertura: ela mostra um painel só, e quem
  // cria carteira é a janela principal.
  if (destacado && e.trancado !== true) {
    fecharBoot();
    return;
  }
  // O nó respondeu: a partida terminou, mesmo que a tela seja a do cadeado.
  acenderRegistro(true);
  // Termos antes de tudo (só no programa com janela, que é quem grava o aceite).
  if (e.app === true && e.termos_aceitos === false && e.pode_mandar === true && e.trancado !== true) {
    if (bootCena !== "termos") {
      mostrarCena("termos");
      carregarTermos();
    }
    return;
  }
  if (e.trancado === true) {
    if (bootCena !== "trava") {
      mostrarCena("trava");
      $("codigo-trava").focus();
    }
    return;
  }
  const precisaCarteira = e.pode_mandar === true && e.carteira === false && !carteiraFeita;
  if (precisaCarteira) {
    // Volta para a abertura mesmo que o painel já tenha sido mostrado: sem
    // carteira, minerar não é possível.
    if (bootCena === "partida" || bootCena === "trava") mostrarCena("1");
    return;
  }
  // O passo 3 fica até a pessoa clicar: é ali que está o aviso de guardar a senha.
  if (bootCena === "3") return;
  if (bootCena === "2" && bootOcupado) return;
  fecharBoot();
}

// Termos: o botão só vale depois de ler até o fim e marcar que leu.
async function carregarTermos() {
  try {
    const r = await fetch("/api/termos", { cache: "no-store" });
    $("termos-texto").innerHTML = await r.text();
  } catch {
    texto("erro-termos", "Não consegui carregar os termos. Feche e abra o programa de novo.");
  }
  const caixa = $("termos-texto");
  const leuTudo = () => caixa.scrollTop + caixa.clientHeight >= caixa.scrollHeight - 24;
  const conferir = () => {
    if (leuTudo()) {
      $("termos-li").disabled = false;
      texto("termos-li-rotulo", "Li e aceito os termos de uso");
    }
  };
  caixa.addEventListener("scroll", conferir);
  conferir();
  caixa.focus();
}
$("termos-li").addEventListener("change", () => { $("termos-aceitar").disabled = !$("termos-li").checked; });
$("termos-aceitar").addEventListener("click", () => comBotao($("termos-aceitar"), async () => {
  texto("erro-termos", "");
  const res = await postar("/api/termos/aceitar");
  if (!res.ok) {
    texto("erro-termos", erroDe(res, "Não deu para gravar o aceite."));
    return;
  }
  mostrarCena("partida");
  await ler();
}));

// Passo 1 → 2.
$("boot-comecar").addEventListener("click", () => {
  mostrarCena("2");
  ($("c-importar").checked ? $("arquivo") : $("senha")).focus();
});

// Cadeado.
$("codigo-trava").addEventListener("input", () => {
  $("destravar").disabled = caracteres($("codigo-trava").value.replace(/\D/g, "")) !== 6;
  texto("erro-trava", "");
});
$("f-destravar").addEventListener("submit", (ev) => {
  ev.preventDefault();
  comBotao($("destravar"), async () => {
    texto("erro-trava", "");
    const res = await postar("/api/destravar", { codigo: $("codigo-trava").value });
    if (!res.ok) {
      texto("erro-trava", erroDe(res, "Não deu para destrancar."));
      return;
    }
    $("codigo-trava").value = "";
    $("destravar").disabled = true;
    await ler();
  });
});

// Passo 2, caminho A: criar carteira.
function trocarCaminho() {
  const importar = $("c-importar").checked;
  $("f-criar").hidden = importar;
  $("f-importar").hidden = !importar;
}
$("c-criar").addEventListener("change", trocarCaminho);
$("c-importar").addEventListener("change", trocarCaminho);

// Força da senha, como estimativa honesta: conta o alfabeto usado e desconta a
// repetição. Não é promessa de segurança, e a tela diz isso.
const CLASSES = [[/[a-z]/, 26], [/[A-Z]/, 26], [/[0-9]/, 10], [/[^a-zA-Z0-9]/, 33]];
function forcaDaSenha(s) {
  const n = caracteres(s);
  if (!n) return { pct: 0, frase: "" };
  const alfabeto = CLASSES.reduce((a, [re, tam]) => a + (re.test(s) ? tam : 0), 0) || 26;
  const distintos = new Set([...s]).size;
  const efetivo = Math.min(n, distintos * 2); // "aaaaaaaaaa" não vale dez caracteres
  const bits = Math.round(efetivo * Math.log2(alfabeto));
  const palavras = bits < 40 ? "fácil de adivinhar" : bits < 56 ? "razoável" : bits < 76 ? "boa" : "muito boa";
  return { pct: Math.max(4, Math.min(100, (bits / 90) * 100)), frase: `Cerca de ${bits} bits: ${palavras}. É uma estimativa, não uma garantia.` };
}

function conferirSenha() {
  const a = $("senha").value, b = $("senha2").value;
  const n = caracteres(a);
  const tamanho = n >= SENHA_MINIMA, iguais = b !== "" && a === b;
  texto("r-tamanho", tamanho ? `${n} caracteres: tamanho bom` : `${n} de no mínimo ${SENHA_MINIMA} caracteres`);
  texto("r-iguais", !b ? "Repita a senha no segundo campo" : iguais ? "As duas senhas batem" : "As duas senhas ainda não batem");
  $("r-tamanho").classList.toggle("cumprida", tamanho);
  $("r-iguais").classList.toggle("cumprida", iguais);
  $("criar").disabled = !(tamanho && iguais);
  const f = forcaDaSenha(a);
  $("forca-caixa").hidden = !n;
  $("forca-barra").style.width = `${f.pct}%`;
  texto("forca-texto", f.frase);
}
$("senha").addEventListener("input", conferirSenha);
$("senha2").addEventListener("input", conferirSenha);
$("ver-senha").addEventListener("click", () => {
  const ver = $("senha").type === "password";
  for (const id of ["senha", "senha2"]) $(id).type = ver ? "text" : "password";
  $("ver-senha").textContent = ver ? "ocultar" : "mostrar";
  $("ver-senha").setAttribute("aria-label", ver ? "Ocultar as senhas" : "Mostrar as senhas");
});

// Pedido em curso: os botões ficam aria-disabled (não disabled, que tiraria o
// foco do teclado) e o envio repetido é barrado por bootOcupado.
function ocupar(sim, botao, rotulo) {
  bootOcupado = sim;
  $("caminhos").disabled = sim;
  boot.setAttribute("aria-busy", String(sim));
  botao.textContent = rotulo;
  if (sim) botao.setAttribute("aria-disabled", "true");
  else botao.removeAttribute("aria-disabled");
}

$("f-criar").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const senha = $("senha").value, senha2 = $("senha2").value;
  if (bootOcupado || caracteres(senha) < SENHA_MINIMA || senha !== senha2) return;
  texto("erro-criar", "");
  $("dica-criar").hidden = false;
  ocupar(true, $("criar"), "Criando, aguarde…");
  const res = await postar("/api/carteira/nova", { senha, senha2 });
  ocupar(false, $("criar"), "Criar carteira");
  $("dica-criar").hidden = true;
  if (res.ok && typeof res.dados?.endereco === "string") {
    $("senha").value = "";
    $("senha2").value = "";
    conferirSenha();
    mostrarPronto(res.dados.endereco);
  } else {
    conferirSenha();
    texto("erro-criar", erroDe(res, "Não deu para criar a carteira. Tente de novo."));
  }
});

// Passo 2, caminho B: importar o carteira.txt.
function conferirImportar() {
  $("importar").disabled = !$("conteudo").value.trim();
}
$("conteudo").addEventListener("input", conferirImportar);
$("arquivo").addEventListener("change", () => {
  const arquivo = $("arquivo").files?.[0];
  texto("erro-importar", "");
  if (!arquivo) return;
  if (arquivo.size > 16 * 1024) {
    texto("erro-importar", "Esse arquivo é grande demais para ser um carteira.txt.");
    return;
  }
  const leitor = new FileReader();
  leitor.addEventListener("load", () => {
    $("conteudo").value = String(leitor.result ?? "");
    conferirImportar();
  });
  leitor.addEventListener("error", () => texto("erro-importar", "Não consegui ler esse arquivo."));
  leitor.readAsText(arquivo);
});
$("f-importar").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const conteudo = $("conteudo").value;
  if (bootOcupado || !conteudo.trim()) return;
  texto("erro-importar", "");
  if (new URLSearchParams({ conteudo }).toString().length > PEDIDO_MAXIMO) {
    texto("erro-importar", "Texto grande demais. Cole só o conteúdo do arquivo carteira.txt.");
    return;
  }
  ocupar(true, $("importar"), "Importando…");
  const res = await postar("/api/carteira/importar", { conteudo });
  ocupar(false, $("importar"), "Importar");
  if (res.ok && typeof res.dados?.endereco === "string") {
    $("conteudo").value = "";
    $("arquivo").value = "";
    conferirImportar();
    mostrarPronto(res.dados.endereco);
  } else {
    conferirImportar();
    texto("erro-importar", erroDe(res, "Não deu para importar a carteira. Confira o conteúdo do arquivo."));
  }
});

// Passo 3: o endereço, o QR de receber e o aviso que não pode passar batido.
function mostrarPronto(endereco) {
  carteiraFeita = true;
  mostrarCena("3");
  texto("pronto-endereco", endereco);
  desenharQr($("pronto-qr"), endereco);
  const dados = typeof ultimo?.dados === "string" ? ultimo.dados : "";
  texto("pronto-pasta", dados ? `O arquivo carteira.txt fica em ${dados}` : "");
  $("pronto-titulo").focus();
}
$("pronto-copiar").addEventListener("click", () => copiar($("pronto-endereco").textContent, $("pronto-endereco"), $("pronto-copiar"), "Copiar endereço"));
$("pronto-entendi").addEventListener("change", () => {
  $("comecar").disabled = !$("pronto-entendi").checked;
});

function abrirPasta(botao, idErro) {
  return comBotao(botao, async () => {
    texto(idErro, "");
    const res = await postar("/api/abrir-pasta");
    if (!res.ok) texto(idErro, erroDe(res, "Não deu para abrir a pasta."));
  });
}
$("abrir-pasta-carteira").addEventListener("click", () => abrirPasta($("abrir-pasta-carteira"), "erro-pronto"));
$("so-olhar").addEventListener("click", () => { fecharBoot(); ler(); });
$("comecar").addEventListener("click", () => comBotao($("comecar"), async () => {
  texto("erro-pronto", "");
  const res = await postar("/api/minerar", { ligar: "1" });
  if (!res.ok) {
    texto("erro-pronto", erroDe(res, "Não deu para ligar a mineração. Tente de novo."));
    return;
  }
  fecharBoot();
  ler();
}));

// ---------- enviar HYX ----------
// Três telas: escrever, conferir, comprovante. Enviar é definitivo, então a tela
// do meio existe para a pessoa reler o endereço antes de assinar.
const enviarDialogo = $("enviar");
let enviarPedido = null;

function faseEnviar(qual) {
  $("f-enviar").hidden = qual !== "escrever";
  $("enviar-conferir").hidden = qual !== "conferir";
  $("enviar-pronto").hidden = qual !== "pronto";
}

function abrirEnviar() {
  if (enviarDialogo.open || !ultimo || ultimo.pode_enviar !== true) return;
  enviarPedido = null;
  faseEnviar("escrever");
  for (const id of ["erro-enviar", "erro-assinar"]) texto(id, "");
  $("en-taxa").value = "";
  $("en-taxa").placeholder = ultimo.taxa_padrao || "0.00000000";
  texto("enviar-saldo", `${moeda(ultimo.saldo)} HYX gastáveis`);
  enviarDialogo.showModal();
  $("en-para").focus();
}
$("abrir-enviar").addEventListener("click", abrirEnviar);
$("enviar-fechar").addEventListener("click", () => enviarDialogo.close());
$("en-terminar").addEventListener("click", () => enviarDialogo.close());
$("en-voltar").addEventListener("click", () => { faseEnviar("escrever"); $("en-para").focus(); });
$("en-tudo").addEventListener("click", () => {
  if (!ultimo) return;
  // O máximo vem do nó, já descontada a taxa padrão; com taxa digitada, desconta dela.
  const taxa = Number(String($("en-taxa").value || ultimo.taxa_padrao || "0").replace(",", "."));
  const saldo = Number(String(ultimo.saldo || "0"));
  const sobra = Math.max(0, saldo - (Number.isFinite(taxa) ? taxa : 0));
  $("en-valor").value = sobra.toFixed(8);
});
enviarDialogo.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  ev.preventDefault();
  enviarDialogo.close();
});
enviarDialogo.addEventListener("close", () => { enviarPedido = null; $("en-senha").value = ""; $("en-codigo").value = ""; });

// A conferência rápida é feita aqui; depois o nó confere o pedido (inclusive o
// dígito verificador do endereço) antes de pedir a senha, e de novo ao assinar.
$("f-enviar").addEventListener("submit", (ev) => {
  ev.preventDefault();
  comBotao($("en-conferir"), () => conferirEnvio());
});

async function conferirEnvio() {
  texto("erro-enviar", "");
  const para = $("en-para").value.replace(/\s+/g, "");
  const valor = $("en-valor").value.trim().replace(",", ".");
  const taxa = $("en-taxa").value.trim().replace(",", ".");
  // o formato com verificador (thyx1…) ou o hexadecimal antigo; o nó confere o verificador
  if (!/^[0-9a-fA-F]{40}$/.test(para) && !/^[a-z]{2,5}1[02-9ac-hj-np-z]{20,80}$/i.test(para)) {
    texto("erro-enviar", "O endereço de destino começa com thyx1 (ou são 40 dígitos hexadecimais, no formato antigo). Confira se não faltou nem sobrou nada.");
    return;
  }
  if (para.toLowerCase() === String(ultimo?.endereco || "").toLowerCase()) {
    texto("erro-enviar", "Esse é o seu próprio endereço.");
    return;
  }
  if (!/^\d+(\.\d{1,8})?$/.test(valor) || Number(valor) <= 0) {
    texto("erro-enviar", "Escreva o valor com ponto e até 8 casas, por exemplo 1.5. Precisa ser maior que zero.");
    return;
  }
  if (taxa && !/^\d+(\.\d{1,8})?$/.test(taxa)) {
    texto("erro-enviar", "A taxa também usa ponto e até 8 casas.");
    return;
  }
  const taxaNum = Number(taxa || ultimo?.taxa_padrao || 0);
  const total = Number(valor) + taxaNum;
  const saldo = Number(ultimo?.saldo || 0);
  if (total > saldo) {
    texto("erro-enviar", `Não dá: valor mais taxa somam ${total.toFixed(8)} HYX e você tem ${saldo.toFixed(8)} gastáveis.`);
    return;
  }
  const conferido = await postar("/api/enviar", { para, valor, taxa, so_conferir: "1" });
  if (!conferido.ok || conferido.dados?.conferido !== true) {
    texto("erro-enviar", erroDe(conferido, "O nó não aceitou este envio."));
    return;
  }
  enviarPedido = { para, valor, taxa };
  texto("cf-para", conferido.dados.para || para);
  texto("cf-valor", moeda(Number(valor).toFixed(8)));
  texto("cf-taxa", moeda(taxaNum.toFixed(8)));
  texto("cf-total", moeda(total.toFixed(8)));
  texto("cf-sobra", moeda((saldo - total).toFixed(8)));
  const exige = ultimo?.seguranca?.exige_envio === true;
  $("en-codigo-caixa").hidden = !exige;
  texto("erro-assinar", "");
  faseEnviar("conferir");
  $("en-senha").focus();
}

$("en-assinar").addEventListener("click", () => comBotao($("en-assinar"), async () => {
  if (!enviarPedido) return;
  texto("erro-assinar", "");
  const senha = $("en-senha").value;
  if (!senha) {
    texto("erro-assinar", "Digite a senha da carteira.");
    return;
  }
  const corpo = { para: enviarPedido.para, valor: enviarPedido.valor, taxa: enviarPedido.taxa, senha };
  if (ultimo?.seguranca?.exige_envio === true) corpo.codigo = $("en-codigo").value;
  $("dica-enviar").hidden = false;
  $("en-assinar").textContent = "Assinando…";
  const res = await postar("/api/enviar", corpo);
  $("dica-enviar").hidden = true;
  $("en-assinar").textContent = "Assinar e enviar";
  if (!res.ok || !res.dados?.txid) {
    texto("erro-assinar", erroDe(res, "Não deu para enviar."));
    return;
  }
  $("en-senha").value = "";
  $("en-codigo").value = "";
  const d = res.dados;
  texto("en-resumo", `${moeda(d.valor)} HYX para ${curto(d.para)}${Number(d.taxa) > 0 ? `, com taxa de ${moeda(d.taxa)}` : ""}. Espalhada para ${d.pares} par(es).`);
  texto("en-txid", d.txid);
  faseEnviar("pronto");
  $("en-terminar").focus();
  ler();
}));

// ---------- receber ----------
const receber = $("receber");
$("abrir-receber").addEventListener("click", () => {
  if (receber.open || !ultimo?.endereco) return;
  texto("rc-endereco", ultimo.endereco);
  desenharQr($("rc-qr"), ultimo.endereco);
  receber.showModal();
  $("rc-fechar").focus();
});
$("rc-fechar").addEventListener("click", () => receber.close());
$("rc-copiar").addEventListener("click", () => copiar(ultimo?.endereco, $("rc-endereco"), $("rc-copiar"), "Copiar endereço"));
receber.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  ev.preventDefault();
  receber.close();
});

// ---------- segundo fator (código de 6 dígitos) ----------
const seguranca = $("seguranca");

function faseSeguranca(qual) {
  $("seg-desligado").hidden = qual !== "desligado";
  $("seg-ligando").hidden = qual !== "ligando";
  $("seg-ligado").hidden = qual !== "ligado";
}

// As caixas de marcar só são sincronizadas ao abrir o diálogo: mexer nelas é o
// começo de um pedido que ainda vai pedir o código, e a leitura de cada segundo
// não pode desmarcar o que a pessoa acabou de marcar.
function desenharSeguranca(e) {
  const s = e.seguranca || {};
  texto("seg-estado", s.ligado ? "ligado" : "desligado");
  if (!seguranca.open) return;
  if ($("seg-ligando").hidden === false) return; // no meio de ligar: não mexe
  faseSeguranca(s.ligado ? "ligado" : "desligado");
}

function sincronizarSeguranca(e) {
  const s = e.seguranca || {};
  $("seg-exige").checked = s.exige_envio === true;
  $("seg-trava").checked = s.trava === true;
  $("seg-trava-nova").checked = false;
}

$("abrir-seguranca").addEventListener("click", () => {
  if (seguranca.open || !ultimo) return;
  for (const id of ["erro-seg", "erro-seg-confirmar", "erro-seg-mudar"]) texto(id, "");
  faseSeguranca(ultimo.seguranca?.ligado ? "ligado" : "desligado");
  sincronizarSeguranca(ultimo);
  desenharSeguranca(ultimo);
  seguranca.showModal();
  $("seg-fechar").focus();
});
$("seg-fechar").addEventListener("click", () => seguranca.close());
$("seg-cancelar").addEventListener("click", () => { faseSeguranca("desligado"); $("seg-codigo").value = ""; });
seguranca.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  ev.preventDefault();
  seguranca.close();
});

$("seg-ligar").addEventListener("click", () => comBotao($("seg-ligar"), async () => {
  texto("erro-seg", "");
  const res = await postar("/api/seguranca/comecar");
  if (!res.ok || !res.dados?.uri) {
    texto("erro-seg", erroDe(res, "Não deu para começar."));
    return;
  }
  desenharQr($("seg-qr"), res.dados.uri);
  texto("seg-segredo", res.dados.segredo || "—");
  $("seg-codigo").value = "";
  $("seg-confirmar").disabled = true;
  faseSeguranca("ligando");
  $("seg-codigo").focus();
}));

$("seg-codigo").addEventListener("input", () => {
  $("seg-confirmar").disabled = $("seg-codigo").value.replace(/\D/g, "").length !== 6;
  texto("erro-seg-confirmar", "");
});
$("f-seg-confirmar").addEventListener("submit", (ev) => {
  ev.preventDefault();
  comBotao($("seg-confirmar"), async () => {
    texto("erro-seg-confirmar", "");
    const res = await postar("/api/seguranca/confirmar", {
      codigo: $("seg-codigo").value,
      trava: $("seg-trava-nova").checked ? "1" : "0",
    });
    if (!res.ok) {
      texto("erro-seg-confirmar", erroDe(res, "Não deu para confirmar."));
      return;
    }
    $("seg-codigo").value = "";
    faseSeguranca("ligado");
    await ler();
  });
});

function mudarSeguranca(desligar) {
  return comBotao(desligar ? $("seg-desligar") : $("seg-salvar"), async () => {
    texto("erro-seg-mudar", "");
    const codigo = $("seg-codigo-mudar").value;
    if (codigo.replace(/\D/g, "").length !== 6) {
      texto("erro-seg-mudar", "Digite o código de 6 dígitos que está no celular agora.");
      return;
    }
    const campos = { codigo };
    if (desligar) campos.desligar = "1";
    else {
      campos.exige_envio = $("seg-exige").checked ? "1" : "0";
      campos.trava = $("seg-trava").checked ? "1" : "0";
    }
    const res = await postar("/api/seguranca/mudar", campos);
    if (!res.ok) {
      texto("erro-seg-mudar", erroDe(res, "Não deu para salvar."));
      return;
    }
    $("seg-codigo-mudar").value = "";
    await ler();
    if (desligar) faseSeguranca("desligado");
  });
}
$("f-seg-mudar").addEventListener("submit", (ev) => { ev.preventDefault(); mudarSeguranca(false); });
$("seg-desligar").addEventListener("click", () => mudarSeguranca(true));

// ---------- minhas máquinas ----------
// Soma esta máquina com as outras da lista. Só leitura: cada uma é comandada
// nela mesma.
function desenharMaquinas(e) {
  const lista = Array.isArray(e.maquinas) ? e.maquinas : [];
  $("mq-aviso").hidden = lista.length > 0;
  const responderam = lista.filter((m) => m.ok);
  texto("mq-total", lista.length ? `${responderam.length} de ${lista.length} respondendo · e esta` : "só esta máquina");
  const ritmo = (e.ritmo || 0) + responderam.reduce((a, m) => a + (m.ritmo || 0), 0);
  const nucleos = (e.minerando ? e.linhas || 0 : 0) + responderam.reduce((a, m) => a + (m.minerando ? m.linhas || 0 : 0), 0);
  const watts = (e.watts || 0) + responderam.reduce((a, m) => a + (m.watts || 0), 0);
  texto("mq-ritmo", `${fmt1.format(ritmo)} tent/s`);
  texto("mq-nucleos", fmt.format(nucleos));
  texto("mq-watts", `${fmt1.format(watts)} W`);

  const cel = (t, classe) => {
    const td = document.createElement("td");
    td.textContent = t;
    if (classe) td.className = classe;
    return td;
  };
  const aqui = document.createElement("tr");
  aqui.append(
    cel("este computador"),
    cel(e.minerando ? "minerando" : "parado"),
    cel(fmt1.format(e.ritmo || 0)),
    cel(e.minerando ? `${e.linhas}/${e.nucleos}` : `0/${e.nucleos}`),
    cel(fmt.format(e.altura || 0)),
  );
  const linhas = lista.map((m) => {
    const tr = document.createElement("tr");
    const alvo = cel(m.alvo);
    if (!m.ok) alvo.className = "caiu";
    const estado = cel(m.ok ? (m.minerando ? "minerando" : "parado") : "sem resposta");
    if (!m.ok && m.erro) {
      estado.title = m.erro;
      estado.className = "caiu";
    }
    tr.append(
      alvo,
      estado,
      cel(m.ok ? fmt1.format(m.ritmo || 0) : "—"),
      cel(m.ok ? `${m.minerando ? m.linhas : 0}/${m.nucleos}` : "—"),
      cel(m.ok ? fmt.format(m.altura || 0) : "—"),
    );
    if (m.ok && m.versao) tr.title = `Hyurax ${m.versao} · ${m.rede || ""}`;
    return tr;
  });
  $("maquinas").replaceChildren(aqui, ...linhas);
}

// ---------- aviso de bloco achado ----------
// Achar um bloco é raro: quando acontece, o programa chama. O gatilho é o
// evento "meu-bloco" que o nó registra, não uma contagem — assim o aviso diz a
// mesma coisa que o fluxo, e nunca aparece duas vezes pelo mesmo bloco.
let ultimoMeuBloco = null;
let somDoAviso = null;
let sumirAviso = null;

function tocarAviso() {
  try {
    const Contexto = globalThis.AudioContext || globalThis.webkitAudioContext;
    if (!Contexto) return;
    somDoAviso = somDoAviso || new Contexto();
    if (somDoAviso.state === "suspended") somDoAviso.resume();
    // Dois tons curtos, feitos aqui: nenhum arquivo, nenhuma internet.
    const agora = somDoAviso.currentTime;
    for (const [atraso, hz] of [[0, 880], [0.16, 1320]]) {
      const osc = somDoAviso.createOscillator();
      const vol = somDoAviso.createGain();
      osc.type = "sine";
      osc.frequency.value = hz;
      vol.gain.setValueAtTime(0.0001, agora + atraso);
      vol.gain.exponentialRampToValueAtTime(0.18, agora + atraso + 0.02);
      vol.gain.exponentialRampToValueAtTime(0.0001, agora + atraso + 0.14);
      osc.connect(vol).connect(somDoAviso.destination);
      osc.start(agora + atraso);
      osc.stop(agora + atraso + 0.16);
    }
  } catch (erro) {
    console.warn("som do aviso indisponível:", erro);
  }
}

function mostrarAviso(frase, comSom) {
  texto("aviso-bloco-texto", frase);
  $("aviso-bloco").hidden = false;
  if (comSom) tocarAviso();
  clearTimeout(sumirAviso);
  sumirAviso = setTimeout(() => ($("aviso-bloco").hidden = true), 14000);
}

function desenharAvisoDeBloco(e) {
  const meus = (e.eventos || []).filter((ev) => ev.tipo === "meu-bloco");
  const ultimo = meus[0]; // o fluxo vem do mais novo para o mais velho
  if (!ultimo) return;
  const chave = `${ultimo.quando}|${ultimo.texto}`;
  const primeiraLeitura = ultimoMeuBloco === null;
  if (chave === ultimoMeuBloco) return;
  ultimoMeuBloco = chave;
  // Na primeira leitura o bloco pode ser de horas atrás: só guarda a marca.
  if (primeiraLeitura || e.avisar_bloco === false) return;
  mostrarAviso(ultimo.texto, e.som_bloco === true);
}
$("aviso-fechar").addEventListener("click", () => { $("aviso-bloco").hidden = true; clearTimeout(sumirAviso); });

// ---------- ajustes ----------
const ajustes = $("ajustes");

// Marca o perfil que bate com o que está valendo, ou "manual".
function perfilAtual(e) {
  const n = Math.max(1, e.nucleos || 1);
  const combina = { leve: [Math.max(1, Math.floor(n / 4)), 35], equilibrado: [Math.max(1, Math.floor(n / 2)), 70], turbo: [n, 100] };
  for (const [nome, [linhas, uso]] of Object.entries(combina)) {
    if (e.linhas === linhas && e.uso_cpu === uso) return nome;
  }
  return "manual";
}

function desenharAjustes(e) {
  const perfil = perfilAtual(e);
  for (const nome of ["leve", "equilibrado", "turbo", "manual"]) {
    const alvo = $(`p-${nome}`);
    if (alvo) alvo.checked = nome === perfil;
  }
  const uso = typeof e.uso_cpu === "number" ? e.uso_cpu : 100;
  if (document.activeElement !== $("a-uso")) $("a-uso").value = String(uso);
  texto("a-uso-valor", `${uso}%`);
  $("a-linhas").max = String(Math.max(1, e.nucleos || 1));
  if (document.activeElement !== $("a-linhas")) $("a-linhas").value = String(e.linhas || 1);
  texto("a-linhas-valor", String(e.linhas || 1));
  texto("a-nucleos", String(e.nucleos || 1));
  texto("a-memoria", `Memória usada pela mineração: ${fmt.format(e.memoria_total_mib || 0)} MiB`);
  if (document.activeElement !== $("a-watts")) $("a-watts").value = String(e.watts_nucleo || 12);
  if (document.activeElement !== $("a-kwh")) $("a-kwh").value = String(e.centavos_kwh || 90);
  texto("a-custo", e.minerando
    ? `Agora: cerca de ${fmt1.format(e.watts || 0)} W, ou R$ ${reais(e.custo_mes)} por mês se ficar ligado o tempo todo.`
    : "Ligue a mineração para ver o consumo estimado.");
  $("a-na-rede").checked = e.na_rede === true;
  $("a-mercado").checked = e.mercado_ligado === true;
  $("a-qr-caixa").hidden = !e.na_rede;
  texto("a-url", e.url_celular || "—");
  if (e.na_rede) desenharQr($("a-qr"), e.url_celular || "");
  $("a-avisar").checked = e.avisar_bloco !== false;
  $("a-som").checked = e.som_bloco === true;
  texto("a-dados", typeof e.dados === "string" && e.dados ? e.dados : "—");
  texto("a-versao", typeof e.versao === "string" && e.versao ? e.versao : "—");
  texto("a-rede", e.rede || "—");
  texto("a-porta", typeof e.porta_p2p !== "number" ? "—" : e.porta_p2p ? String(e.porta_p2p) : "não recebe conexões");
}

$("abrir-ajustes").addEventListener("click", () => {
  if (!ultimo || ajustes.open) return;
  const lista = Array.isArray(ultimo.sementes) ? ultimo.sementes.filter((s) => typeof s === "string") : [];
  $("sementes").value = lista.join(", ");
  const maquinas = Array.isArray(ultimo.maquinas) ? ultimo.maquinas.map((m) => m.alvo).filter(Boolean) : [];
  $("maquinas-lista").value = maquinas.join(", ");
  for (const id of ["a-abrir-erro", "sementes-ok", "sementes-erro", "maquinas-ok", "maquinas-erro"]) texto(id, "");
  desenharAjustes(ultimo);
  ajustes.showModal();
  $("sementes").focus();
});
$("ajustes-fechar").addEventListener("click", () => ajustes.close());
$("a-abrir").addEventListener("click", () => abrirPasta($("a-abrir"), "a-abrir-erro"));
$("sementes").addEventListener("input", () => texto("sementes-ok", ""));
$("f-sementes").addEventListener("submit", (ev) => {
  ev.preventDefault();
  comBotao($("sementes-salvar"), async () => {
    const lista = $("sementes").value.split(",").map((s) => s.trim()).filter(Boolean).join(",");
    texto("sementes-ok", "");
    texto("sementes-erro", "");
    const res = await postar("/api/sementes", { lista });
    if (!res.ok) {
      texto("sementes-erro", erroDe(res, "Não deu para salvar os nós semente."));
      return;
    }
    const salvas = res.dados?.sementes;
    if (Array.isArray(salvas)) $("sementes").value = salvas.filter((s) => typeof s === "string").join(", ");
    texto("sementes-ok", "Salvo.");
  });
});
$("maquinas-lista").addEventListener("input", () => texto("maquinas-ok", ""));
$("f-maquinas").addEventListener("submit", (ev) => {
  ev.preventDefault();
  comBotao($("maquinas-salvar"), async () => {
    const lista = $("maquinas-lista").value.split(",").map((s) => s.trim()).filter(Boolean).join(",");
    texto("maquinas-ok", "");
    texto("maquinas-erro", "");
    const res = await postar("/api/maquinas", { lista });
    if (!res.ok) {
      texto("maquinas-erro", erroDe(res, "Não deu para salvar as máquinas."));
      return;
    }
    const salvas = res.dados?.maquinas;
    if (Array.isArray(salvas)) $("maquinas-lista").value = salvas.filter((s) => typeof s === "string").join(", ");
    texto("maquinas-ok", salvas && salvas.length ? "Salvo. A primeira leitura leva alguns segundos." : "Salvo.");
    await ler();
  });
});
// Desempenho, energia e celular: cada controle manda o seu campo.
async function salvarAjuste(campos) {
  const res = await postar("/api/ajustes", campos);
  if (!res.ok) texto("a-abrir-erro", erroDe(res, "Não deu para salvar o ajuste."));
  await ler();
  return res.ok;
}
for (const nome of ["leve", "equilibrado", "turbo"]) {
  $(`p-${nome}`).addEventListener("change", () => salvarAjuste({ perfil: nome }));
}
$("p-manual").addEventListener("change", () => {});
$("a-uso").addEventListener("input", () => texto("a-uso-valor", `${$("a-uso").value}%`));
$("a-uso").addEventListener("change", () => salvarAjuste({ uso_cpu: $("a-uso").value }));
$("a-linhas").addEventListener("input", () => texto("a-linhas-valor", $("a-linhas").value));
$("a-linhas").addEventListener("change", () => salvarAjuste({ linhas: $("a-linhas").value }));
for (const id of ["a-watts", "a-kwh"]) {
  $(id).addEventListener("change", () => salvarAjuste({ watts_nucleo: $("a-watts").value, centavos_kwh: $("a-kwh").value }));
}
$("a-na-rede").addEventListener("change", () => salvarAjuste({ na_rede: $("a-na-rede").checked ? "1" : "0" }));
$("a-mercado").addEventListener("change", () => salvarAjuste({ mercado: $("a-mercado").checked ? "1" : "0" }));
$("a-avisar").addEventListener("change", () => salvarAjuste({ avisar_bloco: $("a-avisar").checked ? "1" : "0" }));
$("a-som").addEventListener("change", () => {
  const ligado = $("a-som").checked;
  // Toca uma vez ao ligar: serve de prova de que o som funciona nesta janela,
  // e é o clique que os navegadores exigem antes de deixar tocar.
  if (ligado) tocarAviso();
  salvarAjuste({ som_bloco: ligado ? "1" : "0" });
});

// Esc fecha (o navegador já faz; isto garante também em webview que não faça).
ajustes.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  ev.preventDefault();
  ajustes.close();
});

// O navegador pode devolver o estado dos campos ao recarregar: sincroniza.
trocarCaminho();
conferirSenha();
conferirImportar();

// ---------- o trabalho desta máquina ----------
// Mostra a conta de verdade que o bloco carrega: A × B = C, com o tamanho que a
// rede está pedindo agora. Nada aqui é ilustração: os números vêm do nó.
const trabalho = $("trabalho");
let animando = null;

function numeroGrande(n) {
  if (n >= 1e9) return `${fmt1.format(n / 1e9)} bilhões`;
  if (n >= 1e6) return `${fmt1.format(n / 1e6)} milhões`;
  if (n >= 1e3) return `${fmt1.format(n / 1e3)} mil`;
  return fmt.format(n);
}

function bytesLegiveis(b) {
  if (b >= 1024 * 1024) return `${fmt1.format(b / 1024 / 1024)} MiB`;
  if (b >= 1024) return `${fmt1.format(b / 1024)} KiB`;
  return `${fmt.format(b)} bytes`;
}

function desenharTrabalho(e) {
  if (!trabalho.open) return;
  const w = e.trabalho_util || {};
  const n = w.n || 0;
  texto("t-estado", e.minerando ? "calculando" : "parado");
  texto("t-n", fmt.format(n));
  texto("t-n2", fmt.format(n));
  texto("t-contas", `${numeroGrande(n * n * n)} multiplicações`);
  texto("t-rodadas", `${w.rodadas || 0} por bloco`);
  texto("t-bytes", bytesLegiveis(w.bytes || 0));
  texto("t-faixa", `de ${w.lado_min || 0}×${w.lado_min || 0} a ${w.lado_max || 0}×${w.lado_max || 0}`);
}

// O desenho: duas matrizes, a conta andando célula a célula, e o resultado.
function animarMatriz(agora) {
  if (!trabalho.open) { animando = null; return; }
  animando = requestAnimationFrame(animarMatriz);
  const cv = $("t-matriz");
  const g = cv.getContext("2d");
  const lado = 8, celulas = 9, tam = lado * celulas;
  const y0 = 40;
  const xs = [30, 150, 290];
  g.clearRect(0, 0, cv.width, cv.height);
  g.font = "13px " + getComputedStyle(document.body).fontFamily;
  g.fillStyle = "rgba(243,243,241,0.45)";
  g.textAlign = "center";
  g.fillText("A", xs[0] + tam / 2, y0 - 12);
  g.fillText("B", xs[1] + tam / 2, y0 - 12);
  g.fillText("C", xs[2] + tam / 2, y0 - 12);
  g.fillText("×", xs[0] + tam + 20, y0 + tam / 2);
  g.fillText("=", xs[1] + tam + 20, y0 + tam / 2);
  const passo = Math.floor(agora / 260) % (celulas * celulas);
  const linha = Math.floor(passo / celulas), coluna = passo % celulas;
  for (let m = 0; m < 3; m++) {
    for (let i = 0; i < celulas; i++) {
      for (let j = 0; j < celulas; j++) {
        const x = xs[m] + j * lado, y = y0 + i * lado;
        const ativa = (m === 0 && i === linha) || (m === 1 && j === coluna) || (m === 2 && i === linha && j === coluna);
        const feita = m === 2 && (i * celulas + j) < passo;
        if (ativa) {
          g.fillStyle = "#f3f3f1";
          g.fillRect(x, y, lado - 1.5, lado - 1.5);
        } else {
          g.fillStyle = feita ? "rgba(243,243,241,0.45)" : "rgba(243,243,241,0.14)";
          g.fillRect(x, y, lado - 1.5, lado - 1.5);
        }
      }
    }
  }
  g.fillStyle = "rgba(243,243,241,0.45)";
  g.textAlign = "left";
  g.fillText(`linha ${linha + 1} × coluna ${coluna + 1}`, 30, y0 + tam + 28);
  g.fillText("o desenho é um resumo: a conta real tem o tamanho da tabela ao lado", 30, y0 + tam + 46);
}

$("abrir-trabalho").addEventListener("click", () => {
  if (trabalho.open) return;
  trabalho.showModal();
  if (ultimo) desenharTrabalho(ultimo);
  if (!animando) animando = requestAnimationFrame(animarMatriz);
  $("trabalho-fechar").focus();
});
$("trabalho-fechar").addEventListener("click", () => trabalho.close());
trabalho.addEventListener("close", () => {
  if (animando) cancelAnimationFrame(animando);
  animando = null;
});
trabalho.addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  ev.preventDefault();
  trabalho.close();
});

// ---------- painéis: quais aparecem, em que ordem, e o modo destacado ----------
// A janela é do dono: ele fecha o que não quer, muda a ordem e pode abrir um
// painel sozinho numa janela à parte (endereço /?so=NOME).
// ---------- seções ----------
// A navegação à esquerda escolhe a seção; cada seção mostra os painéis dela.
// Dentro da seção, o dono ainda fecha e reordena painéis (menu Organizar).
const VISTAS = {
  inicio: { titulo: "Visão geral", paineis: ["inicio", "fluxo"] },
  ultrax: { titulo: "ULTRAX", paineis: ["ultrax", "verificacao", "historico", "telemetria"] },
  ia: { titulo: "IA · Moléculas", paineis: ["ia"] },
  ciencia: { titulo: "Ciência", paineis: ["ciencia", "visao3d", "novojob", "eventos", "bancada"] },
  mineracao: { titulo: "Mineração", paineis: ["mineracao", "ritmo", "livro"] },
  carteira: { titulo: "Carteira", paineis: ["carteira"] },
  rede: { titulo: "Rede", paineis: ["rede", "maquinas", "mercado"] },
};
let vista = "inicio";
try {
  const salva = localStorage.getItem("hyurax.vista");
  if (salva && VISTAS[salva]) vista = salva;
} catch { /* sem armazenamento: começa na visão geral */ }

function irPara(nome) {
  if (!VISTAS[nome]) return;
  vista = nome;
  try { localStorage.setItem("hyurax.vista", nome); } catch { /* segue sem lembrar */ }
  document.querySelectorAll(".vista").forEach((b) => {
    if (b.dataset.vista === nome) b.setAttribute("aria-current", "page");
    else b.removeAttribute("aria-current");
  });
  texto("vista-titulo", VISTAS[nome].titulo);
  document.title = nome === "inicio" ? "Hyurax" : `${VISTAS[nome].titulo} · Hyurax`;
  aplicarPaineis(listaPaineis);
  document.querySelector(".grade")?.scrollTo({ top: 0 });
  if (ultimo) {
    // o que depende de tamanho (molécula, gráfico) se desenha de novo no lugar novo
    dispatchEvent(new Event("resize"));
  }
}
document.querySelectorAll(".vista").forEach((b) => b.addEventListener("click", () => irPara(b.dataset.vista)));
document.querySelectorAll("[data-ir]").forEach((b) => b.addEventListener("click", () => irPara(b.dataset.ir)));

const NOMES_PAINEIS = {
  inicio: "Visão geral",
  ultrax: "ULTRAX · trabalho ativo",
  ia: "IA · moléculas",
  verificacao: "Verificação e contribuição",
  mineracao: "Mineração",
  historico: "Histórico de tarefas",
  telemetria: "Telemetria do ULTRAX",
  carteira: "Carteira",
  rede: "Rede",
  livro: "Livro de blocos",
  ritmo: "Ritmo",
  fluxo: "Fluxo de eventos",
  mercado: "Mercado",
  maquinas: "Minhas máquinas",
  ciencia: "Ciência · painel técnico e JOBs",
  visao3d: "Visão 3D do cálculo",
  novojob: "Novo JOB",
  eventos: "Eventos dos JOBs",
  bancada: "ULTRA BENCHMARK",
};
const PAINEIS_PADRAO = "inicio,ultrax,ia,verificacao,carteira,mineracao,historico,telemetria,livro,fluxo,rede,ritmo,-mercado,-maquinas,ciencia,visao3d,novojob,eventos,bancada";
const destacado = new URLSearchParams(location.search).get("so");
let listaPaineis = PAINEIS_PADRAO;

function pecas(lista) {
  return lista.split(",").map((s) => s.trim()).filter(Boolean).map((item) => ({
    nome: item.startsWith("-") ? item.slice(1) : item,
    aberto: !item.startsWith("-"),
  }));
}

function aplicarPaineis(lista) {
  listaPaineis = lista;
  const partes = pecas(lista);
  document.querySelectorAll("[data-painel]").forEach((secao) => {
    const nome = secao.dataset.painel;
    const parte = partes.find((p) => p.nome === nome);
    const naVista = (VISTAS[vista]?.paineis || []).includes(nome);
    // a visão geral não se fecha: é para onde o programa sempre volta
    const visivel = destacado ? nome === destacado : naVista && (nome === "inicio" || !!parte?.aberto);
    secao.hidden = !visivel;
    secao.style.order = String(partes.findIndex((p) => p.nome === nome));
  });
  document.body.classList.toggle("destacado", !!destacado);
}

async function salvarPaineis(lista) {
  const res = await postar("/api/ajustes", { paineis: lista });
  if (!res.ok) {
    texto("paineis-erro", erroDe(res, "Não deu para salvar a escolha dos painéis."));
    return false;
  }
  aplicarPaineis(lista);
  desenharListaPaineis();
  return true;
}

function desenharListaPaineis() {
  const partes = pecas(listaPaineis);
  const ul = $("lista-paineis");
  ul.replaceChildren(...partes.map((parte, i) => {
    const li = document.createElement("li");
    const rotulo = document.createElement("label");
    const caixa = document.createElement("input");
    caixa.type = "checkbox";
    caixa.checked = parte.aberto;
    caixa.addEventListener("change", () => {
      const novas = partes.map((p, j) => (j === i ? `${caixa.checked ? "" : "-"}${p.nome}` : `${p.aberto ? "" : "-"}${p.nome}`));
      salvarPaineis(novas.join(","));
    });
    rotulo.append(caixa, Object.assign(document.createElement("span"), { textContent: NOMES_PAINEIS[parte.nome] || parte.nome }));
    const mover = (delta) => {
      const j = i + delta;
      if (j < 0 || j >= partes.length) return;
      const novas = partes.map((p) => `${p.aberto ? "" : "-"}${p.nome}`);
      [novas[i], novas[j]] = [novas[j], novas[i]];
      salvarPaineis(novas.join(","));
    };
    const sobe = Object.assign(document.createElement("button"), { type: "button", textContent: "↑", title: "Subir" });
    sobe.disabled = i === 0;
    sobe.addEventListener("click", () => mover(-1));
    const desce = Object.assign(document.createElement("button"), { type: "button", textContent: "↓", title: "Descer" });
    desce.disabled = i === partes.length - 1;
    desce.addEventListener("click", () => mover(1));
    const janela = Object.assign(document.createElement("button"), { type: "button", textContent: "⧉", title: "Abrir em janela separada" });
    janela.addEventListener("click", () => window.open(`/?so=${parte.nome}`, "_blank", "noopener"));
    li.append(rotulo, sobe, desce, janela);
    return li;
  }));
}

// Cada painel ganha o seu botão de fechar, sem repetir HTML.
function prepararBotoesDePainel() {
  document.querySelectorAll("[data-painel]").forEach((secao) => {
    const cabecalho = secao.querySelector(".rotulo");
    if (!cabecalho || cabecalho.querySelector(".fechar-painel")) return;
    const fechar = document.createElement("button");
    fechar.type = "button";
    fechar.className = "fechar-painel";
    fechar.textContent = "×";
    fechar.title = "Fechar este painel (volta pelo menu Painéis)";
    fechar.setAttribute("aria-label", `Fechar o painel ${NOMES_PAINEIS[secao.dataset.painel] || ""}`);
    fechar.addEventListener("click", () => {
      const novas = pecas(listaPaineis).map((p) => (p.nome === secao.dataset.painel ? `-${p.nome}` : `${p.aberto ? "" : "-"}${p.nome}`));
      salvarPaineis(novas.join(","));
    });
    const fim = cabecalho.querySelector(".rotulo-fim");
    if (fim) fim.append(fechar);
    else cabecalho.append(fechar);
  });
}

$("abrir-paineis").addEventListener("click", () => {
  if ($("paineis").open) return;
  texto("paineis-erro", "");
  desenharListaPaineis();
  $("paineis").showModal();
});
$("paineis-fechar").addEventListener("click", () => $("paineis").close());
$("paineis-padrao").addEventListener("click", () => salvarPaineis(PAINEIS_PADRAO));
$("paineis").addEventListener("keydown", (ev) => {
  if (ev.key !== "Escape") return;
  ev.preventDefault();
  $("paineis").close();
});

// ---------- ULTRAX ----------
// Tudo aqui sai de /api/estado → ultrax. O progresso é o que o worker contou,
// em unidades reais do trabalho: nada anda sozinho na tela.
const CICLO = ["CREATED", "QUEUED", "ASSIGNED", "EXECUTING", "SUBMITTED", "VERIFYING", "VERIFIED", "SETTLED"];
const TIPOS = {
  matrix: "Multiplicação de matrizes",
  knapsack: "Otimização da mochila",
  diffusion: "Difusão de calor",
  "ai-training": "Treino de rede neural",
};
const CELULAS_MAX = 64;
let ultraxOcupado = false;

async function mandarUltrax(campos) {
  if (ultraxOcupado) return false;
  ultraxOcupado = true;
  const res = await postar("/api/ultrax", campos);
  ultraxOcupado = false;
  await ler();
  return res.ok;
}

// Ligar mostra o trabalho na hora: se o painel estava fechado, ele abre.
async function alternarUltrax() {
  const u = ultimo?.ultrax;
  if (!u) return;
  const ligar = !u.ligado;
  if (ligar) irPara("ultrax");
  if (ligar && !pecas(listaPaineis).some((p) => p.nome === "ultrax" && p.aberto)) {
    const novas = pecas(listaPaineis).map((p) => `${p.nome === "ultrax" || p.aberto ? "" : "-"}${p.nome}`);
    await salvarPaineis(novas.join(","));
  }
  await mandarUltrax({ ligar: ligar ? "1" : "0" });
}
$("ultrax").addEventListener("click", alternarUltrax);
$("u-ligar").addEventListener("click", alternarUltrax);
$("u-menos").addEventListener("click", () => ultimo?.ultrax && mandarUltrax({ linhas: String(ultimo.ultrax.linhas - 1) }));
$("u-mais").addEventListener("click", () => ultimo?.ultrax && mandarUltrax({ linhas: String(ultimo.ultrax.linhas + 1) }));
for (const uso of [25, 50, 75, 100]) {
  $(`u-cpu-${uso}`).addEventListener("change", () => mandarUltrax({ uso_cpu: String(uso) }));
}
$("u-memoria").addEventListener("change", () => mandarUltrax({ memoria_mib: $("u-memoria").value }));
$("u-debug").addEventListener("change", () => mandarUltrax({ debug: $("u-debug").checked ? "1" : "0" }));
$("u-gpu").addEventListener("change", () => mandarUltrax({ gpu: $("u-gpu").checked ? "1" : "0" }));
for (const uso of [25, 50, 75, 100]) {
  $(`u-gpu-${uso}`).addEventListener("change", () => mandarUltrax({ gpu_uso: String(uso) }));
}

// ---------- a GPU desta janela ----------
// A conta na GPU roda numa thread à parte (gpu-trabalhador.js), que segue com
// a janela minimizada. Aqui só vai o que o dono escolheu, e volta o nome da
// GPU ou o erro. O nó confere cada resultado na CPU antes de creditar.
let gpuErro = "";
let trabalhadorGpu = null;
function avisarGpu(e) {
  const u = e.ultrax;
  const ligada = !!(u && u.ligado && u.gpu?.ligada && e.pode_mandar);
  if (ligada && !trabalhadorGpu && typeof Worker !== "undefined") {
    try {
      trabalhadorGpu = new Worker("/gpu-trabalhador.js", { type: "module" });
      trabalhadorGpu.onmessage = (ev) => {
        if (typeof ev.data?.erro === "string") gpuErro = ev.data.erro;
      };
    } catch (erro) {
      gpuErro = `A GPU não pôde começar: ${erro.message || erro}`;
    }
  }
  trabalhadorGpu?.postMessage({ ligada, uso: u?.gpu?.uso ?? 50 });
}
$("v-worker").addEventListener("click", () => copiar(ultimo?.ultrax?.worker, $("v-worker"), $("v-worker"), curto(ultimo?.ultrax?.worker || "")));

// "7377.75" do nó vira "7.377,75": milhar com ponto, decimal com vírgula.
function virgula(t) {
  const [inteiro, casas] = String(t ?? "0").split(".");
  const n = Number(inteiro);
  const texto = Number.isFinite(n) ? fmt.format(n) : inteiro;
  return casas ? `${texto},${casas}` : texto;
}
const segundos = (ms) => `${fmt1.format((ms || 0) / 1000)} s`;
function horaMs(ms) {
  const d = new Date(ms);
  return `${d.toLocaleTimeString("pt-BR", { hour12: false })}.${String(d.getMilliseconds()).padStart(3, "0")}`;
}
function el(tag, classe, conteudo) {
  const x = document.createElement(tag);
  if (classe) x.className = classe;
  if (conteudo !== undefined) x.textContent = conteudo;
  return x;
}

// As fatias do trabalho, pela fase: é o que dá para contar de verdade.
function fatias(a) {
  if (a.estado === "VERIFYING") {
    if (a.desafio) return [1, "comparação com a resposta do gabarito"];
    if (a.tipo === "matrix") return [4, "rodadas de Freivalds"];
    if (a.tipo === "knapsack") return [a.tamanho, "itens recalculados"];
    if (a.tipo === "ai-training") return [a.passos, "passos de treino refeitos"];
    return [a.passos, "passos refeitos"];
  }
  if (a.tipo === "ai-training") return [a.passos, "passos de treino"];
  if (a.tipo === "matrix") return [a.tamanho, "linhas de C calculadas"];
  if (a.tipo === "knapsack") return [a.tamanho, "itens da programação dinâmica"];
  return [a.passos, "passos da difusão"];
}

function cartaoDaTarefa(a, agora) {
  const art = el("article", "u-tarefa");
  const cab = el("header");
  const naGpu = a.dispositivo === "GPU";
  cab.append(el("span", "u-cat", `${a.categoria} · ${naGpu ? "GPU · " : ""}${ultimo.ultrax.modo}`), el("span", "u-num num", `#${String(a.numero).padStart(8, "0")}`));
  const titulo = el("h3", "u-titulo", TIPOS[a.tipo] || a.descricao);
  titulo.append(el("b", "num", a.resumo));
  if (a.desafio) titulo.append(el("span", "u-desafio", "DESAFIO"));
  const ciclo = el("ol", "u-ciclo");
  ciclo.setAttribute("aria-label", `Ciclo de vida: agora em ${a.estado}`);
  const aqui = CICLO.indexOf(a.estado);
  CICLO.forEach((nome, i) => ciclo.append(el("li", i < aqui ? "feito" : i === aqui ? "agora" : "", nome)));

  const [total, nome] = fatias(a);
  const fracao = a.total > 0 ? Math.min(1, a.feitas / a.total) : 0;
  const feitas = Math.min(total, Math.floor(fracao * total));
  const celulas = Math.max(1, Math.min(CELULAS_MAX, total));
  const grade = el("div", "u-unidades");
  grade.style.setProperty("--colunas", String(Math.min(celulas, 32)));
  grade.setAttribute("role", "img");
  grade.setAttribute("aria-label", `${fmt.format(feitas)} de ${fmt.format(total)} ${nome}`);
  for (let i = 0; i < celulas; i++) {
    const inicio = (i * total) / celulas, fim = ((i + 1) * total) / celulas;
    const parte = (fracao * total - inicio) / (fim - inicio);
    grade.append(el("i", parte >= 1 ? "cheia" : parte > 0 ? "meia" : ""));
  }
  const prog = el("div", "u-progresso");
  const esq = el("span");
  esq.append(el("b", "num", `${fmt.format(feitas)} de ${fmt.format(total)}`), document.createTextNode(` ${nome}`));
  const operacoes = a.estado === "VERIFYING" ? a.operacoes : a.feitas;
  prog.append(esq, el("span", "num", `${numeroGrande(operacoes)} operações · ${Math.round(fracao * 100)}%`));

  const fatos = el("dl", "u-fatos");
  const fato = (dt, dd, titulo) => {
    const d = el("div");
    const v = el("dd", "num", dd);
    if (titulo) v.title = titulo;
    d.append(el("dt", "", dt), v);
    fatos.append(d);
  };
  fato("Verificação", a.metodo);
  fato("Quem confere", ultimo.ultrax.verificador || "esta máquina");
  fato("Recursos", naGpu ? `GPU (WebGL2) · ${fmt1.format(a.memoria_mib)} MiB` : `linha ${a.linha} · ${fmt1.format(a.memoria_mib)} MiB`);
  fato("Começou há", duracao((agora - a.inicio) / 1000));
  fato("INPUT_HASH", a.entrada ? curto(a.entrada) : "—", a.entrada);
  fato("TASK_ID", curto(a.id), a.id);
  art.append(cab, titulo, ciclo, grade, prog, fatos);
  return art;
}

function desenharUltrax(e) {
  const u = e.ultrax;
  const botao = $("ultrax");
  if (!u) {
    botao.hidden = true;
    return;
  }
  botao.hidden = false;
  botao.setAttribute("aria-pressed", String(u.ligado));
  botao.querySelector("span").textContent = u.ligado ? "TRABALHANDO" : "ULTRAX";
  botao.title = u.ligado ? "ULTRAX ligado (modo LAB). Clique para parar." : "Ligar o ULTRAX: trabalho útil verificável, modo LAB";
  texto("u-modo", `${u.modo} · ${u.selo}`);
  const ativas = Array.isArray(u.ativas) ? u.ativas : [];
  texto("u-estado", !u.ligado ? "desligado" : ativas.length ? `${ativas.length} tarefa${ativas.length > 1 ? "s" : ""} rodando` : "preparando a próxima tarefa");
  $("u-controles").hidden = !e.pode_mandar;
  $("u-ligar").hidden = !e.pode_mandar;
  $("u-vazio").hidden = u.ligado;
  texto("u-linhas", u.linhas);
  texto("u-nucleos", u.nucleos);
  $("u-menos").disabled = u.linhas <= 1;
  $("u-mais").disabled = u.linhas >= u.nucleos;
  for (const uso of [25, 50, 75, 100]) $(`u-cpu-${uso}`).checked = u.uso_cpu === uso;
  const memoria = $("u-memoria");
  if (document.activeElement !== memoria) {
    if (![...memoria.options].some((o) => o.value === String(u.memoria_mib))) {
      memoria.append(new Option(`até ${fmt.format(u.memoria_mib)} MiB`, String(u.memoria_mib)));
    }
    memoria.value = String(u.memoria_mib);
  }
  $("u-debug").checked = u.debug === true;
  const g = u.gpu || {};
  $("u-gpu").checked = g.ligada === true;
  for (const uso of [25, 50, 75, 100]) $(`u-gpu-${uso}`).checked = g.uso === uso;
  $("u-gpu-controles").hidden = !e.pode_mandar;
  texto("u-gpu-nome", gpuErro || (g.ligada
    ? `${g.nome || "detectando a GPU…"} · trabalha enquanto esta janela estiver aberta; a CPU confere cada resultado`
    : "Ligue para a GPU (integrada ou placa de vídeo) também trabalhar, pelo WebGL2 desta janela. A CPU confere cada resultado."));

  const agora = e.agora ? e.agora * 1000 : Date.now();
  const cartoes = ativas.map((a) => cartaoDaTarefa(a, Math.max(agora, Date.now())));
  if (u.ligado && cartoes.length === 0) {
    cartoes.push(el("p", "u-espera", u.fila > 0
      ? "Entre uma tarefa e outra: a próxima já está na fila."
      : "Preparando a próxima tarefa: o gerador cria uma por segundo, com o tamanho medido para esta máquina."));
  }
  $("u-ativas").replaceChildren(...cartoes);
  $("u-ativas").hidden = !u.ligado && ativas.length === 0;
  texto("u-r-cpu", `${u.uso_cpu}% × ${u.linhas}`);
  $("u-r-cpu").title = `até ${u.uso_cpu}% da CPU em cada uma das ${u.linhas} linha(s)`;
  texto("u-r-mem", `${fmt1.format(u.reservada_mib)}/${fmt.format(u.memoria_mib)} MiB`);
  $("u-r-mem").title = "memória reservada pelas tarefas em curso, do teto escolhido";
  texto("u-r-fila", fmt.format(u.fila));
  texto("u-r-gpu", !g.ligada ? "desligada" : g.nome ? `${g.uso}% · ${g.nome}` : `${g.uso}%`);
  $("u-r-gpu").title = g.nome || "";

  desenharVerificacao(e, u);
  desenharIa(u.ia);
  desenharHistoricoUltrax(u);
  desenharTelemetria(u);
}

// ---------- IA ----------
// A molécula é de verdade (base AqSolDB) e a previsão vem do melhor modelo
// treinado nesta máquina. O desenho só é refeito quando a molécula muda.
let moleculaDesenhada = "";
let curvaDesenhada = "";
function coresDoTema() {
  const css = getComputedStyle(document.documentElement);
  return {
    tinta: css.getPropertyValue("--tinta").trim() || "#f3f3f1",
    fraca: css.getPropertyValue("--tinta-3").trim() || "rgba(243,243,241,.38)",
    fio: css.getPropertyValue("--fio-forte").trim() || "rgba(243,243,241,.28)",
    fonte: getComputedStyle(document.body).fontFamily,
  };
}
// log S (mol/L) e massa molar dão a solubilidade em g/L, que se lê melhor.
function gramasPorLitro(logsMili, massaMili) {
  const gl = Math.pow(10, logsMili / 1000) * (massaMili / 1000);
  if (gl >= 100) return `${fmt.format(Math.round(gl))} g/L`;
  if (gl >= 1) return `${fmt1.format(gl)} g/L`;
  if (gl >= 0.001) return `${fmt1.format(gl * 1000)} mg/L`;
  return `${fmt1.format(gl * 1e6)} µg/L`;
}
const logs = (mili) => `log S ${(mili / 1000).toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;

function desenharIa(ia) {
  if (!ia || $("painel-ia").hidden) return;
  texto("ia-treinos", ia.treinos ? `${fmt.format(ia.treinos)} treino${ia.treinos > 1 ? "s" : ""}` : "sem treino ainda");
  texto("ia-validacao", fmt.format(ia.validacao || 0));
  texto("ia-rmse", typeof ia.melhor_rmse_mili === "number" ? `± ${(ia.melhor_rmse_mili / 1000).toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}` : "—");
  const a = ia.amostra;
  if (a) {
    texto("ia-nome", a.nome || a.id);
    texto("ia-formula", `${a.formula} · ${fmt1.format(a.massa_mili / 1000)} g/mol · ${a.id}`);
    texto("ia-medido", `${logs(a.medido_mili)} · ${gramasPorLitro(a.medido_mili, a.massa_mili)}`);
    if (typeof a.previsto_mili === "number") {
      texto("ia-previsto", `${logs(a.previsto_mili)} · ${gramasPorLitro(a.previsto_mili, a.massa_mili)}`);
      const dif = (a.previsto_mili - a.medido_mili) / 1000;
      texto("ia-diferenca", `${dif >= 0 ? "+" : "−"}${Math.abs(dif).toLocaleString("pt-BR", { maximumFractionDigits: 2, minimumFractionDigits: 2 })} em log S`);
    } else {
      texto("ia-previsto", "ainda não há modelo treinado");
      texto("ia-diferenca", "—");
    }
    if (a.smiles !== moleculaDesenhada) {
      moleculaDesenhada = a.smiles;
      $("ia-desenho").setAttribute("aria-label", `Estrutura de ${a.nome}: ${a.smiles}`);
      desenharMolecula($("ia-desenho"), a.smiles, coresDoTema());
    }
  }
  const curva = Array.isArray(ia.ultima_curva) ? ia.ultima_curva : [];
  const chave = curva.join(",");
  if (chave !== curvaDesenhada) {
    curvaDesenhada = chave;
    desenharCurva($("ia-curva"), curva, coresDoTema());
  }
}
addEventListener("resize", () => { moleculaDesenhada = ""; curvaDesenhada = ""; });

function desenharVerificacao(e, u) {
  const p = u.placar || {};
  texto("v-desde", `${u.modo} · ${fmt.format(p.geradas || 0)} tarefas geradas`);
  texto("v-score", virgula(p.work_score));
  texto("v-nota", fmt.format(p.nota ?? 500));
  $("v-nota-barra").style.width = `${Math.max(0, Math.min(100, (p.nota ?? 500) / 10))}%`;
  texto("v-enviadas", fmt.format(p.enviadas || 0));
  texto("v-verificadas", `${fmt.format(p.verificadas || 0)}${typeof p.taxa === "number" ? ` · ${fmt1.format(p.taxa / 10)}% de acerto` : ""}`);
  texto("v-recusadas", fmt.format(p.recusadas || 0));
  texto("v-disputa", `${fmt.format(p.disputadas || 0)} · ${fmt.format(p.divergentes || 0)}`);
  texto("v-desafios", `${fmt.format(p.desafios_certos || 0)} · ${fmt.format(p.desafios_errados || 0)}`);
  texto("v-paradas", `${fmt.format(p.canceladas || 0)} · ${fmt.format(p.expiradas || 0)} · ${fmt.format(p.abandonadas || 0)}`);
  texto("v-operacoes", numeroGrande(p.operacoes_verificadas || 0));
  texto("v-tempo", `${segundos(p.ms_calculo)} · ${segundos(p.ms_verificacao)}`);
  texto("v-ligado", duracao(p.segundos_ligado || 0));
  texto("v-verificador", u.verificador || "esta máquina");
  texto("v-no-estado", `${e.pares > 0 ? "conectado" : "no ar, sem pares"} · ${e.versao || "—"}`);
  texto("v-no-pares", `${fmt.format(e.pares || 0)} · latência não medida no LAB`);
  const worker = $("v-worker");
  if (worker.textContent === "—" || !["copiado", "selecionado"].includes(worker.textContent)) texto("v-worker", curto(u.worker || ""));
  worker.title = `WORKER_ID ${u.worker || ""} — copiar`;
}

function desenharHistoricoUltrax(u) {
  const h = Array.isArray(u.historico) ? u.historico : [];
  $("u-historico-vazio").hidden = h.length > 0;
  $("u-historico").replaceChildren(...h.map((x) => {
    const tr = el("tr", x.estado === "SETTLED" ? "meu" : x.estado === "REJECTED" ? "recusada" : "");
    const primeira = el("td", "num");
    primeira.append(el("i", x.estado === "SETTLED" ? "q meu" : "q"), document.createTextNode(`#${String(x.numero).padStart(8, "0")}`));
    const trabalho = el("td", "", `${TIPOS[x.tipo] || x.tipo} ${x.resumo}${x.desafio ? " · desafio" : ""}`);
    const estado = el("td");
    estado.append(el("span", "estado", x.estado));
    if (x.nota) estado.title = x.nota;
    tr.append(
      primeira,
      trabalho,
      estado,
      el("td", "num", x.operacoes ? numeroGrande(x.operacoes) : "—"),
      el("td", "num", x.ms_calculo ? segundos(x.ms_calculo) : "—"),
      el("td", "", x.estado === "SETTLED" || x.estado === "REJECTED" ? x.metodo : x.nota || "—"),
    );
    return tr;
  }));
}

function desenharTelemetria(u) {
  texto("u-debug-estado", u.debug ? "DEBUG ligado · também em PASTA/ultrax/telemetria.log" : "DEBUG desligado");
  const t = Array.isArray(u.telemetria) ? u.telemetria : [];
  $("u-telemetria").replaceChildren(...t.slice(0, 60).map((m) => {
    const li = el("li");
    const quando = el("time", "num", horaMs(m.ms));
    quando.dateTime = new Date(m.ms).toISOString();
    const bom = m.evento === "VERIFICATION PASSED" || m.evento === "SETTLEMENT COMPLETED";
    const ruim = /FAILED|CANCELLED|EXPIRED|ABANDONED|ERROR/.test(m.evento);
    const corpo = el("span");
    corpo.append(el("span", `t-evento${bom ? " bom" : ruim ? " ruim" : ""}`, m.evento));
    if (m.detalhe) corpo.append(el("span", "t-detalhe", m.detalhe));
    li.append(quando, el("span", "t-num num", m.tarefa ? `#${String(m.tarefa).padStart(8, "0")}` : "—"), corpo);
    return li;
  }));
}

// ---------- visão geral ----------
function desenharInicio(e) {
  const u = e.ultrax || {};
  const p = u.placar || {};
  const ativas = Array.isArray(u.ativas) ? u.ativas : [];
  const frases = [];
  if (!u.ligado) {
    frases.push("O ULTRAX está desligado.");
  } else if (ativas.length) {
    const a = ativas[0];
    const pct = a.total > 0 ? Math.round((100 * a.feitas) / a.total) : 0;
    const onde = a.dispositivo === "GPU" ? " na GPU" : "";
    const fase = a.estado === "VERIFYING" ? "conferindo" : "calculando";
    frases.push(`O ULTRAX está ${fase} ${(TIPOS[a.tipo] || a.descricao).toLowerCase()} ${a.resumo}${onde} (${pct}%)${ativas.length > 1 ? `, e mais ${ativas.length - 1}` : ""}.`);
  } else {
    frases.push("O ULTRAX está ligado, preparando a próxima tarefa.");
  }
  frases.push(e.minerando ? `A mineração procura o bloco ${fmt.format(e.rodada_altura || e.altura + 1)}.` : "A mineração está parada.");
  if (e.pares === 0) frases.push("Sem pares por enquanto.");
  texto("i-frase", frases.join(" "));
  texto("i-score", virgula(p.work_score));
  texto("i-ultrax-estado", u.ligado ? (u.gpu?.ligada ? "CPU e GPU trabalhando" : "trabalhando") : "desligado");
  texto("i-tarefas", `${fmt.format(p.liquidadas || 0)} tarefas verificadas · reputação ${fmt.format(p.nota ?? 500)}/1000`);
  const ia = u.ia || {};
  texto("i-ia-erro", typeof ia.melhor_rmse_mili === "number" ? `± ${(ia.melhor_rmse_mili / 1000).toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}` : "—");
  texto("i-ia-mol", ia.amostra ? `${ia.amostra.nome} · ${ia.amostra.formula}` : "sem treino ainda");
  texto("i-min-blocos", fmt.format(e.meus_cadeia || 0));
  texto("i-min-estado", e.minerando ? "minerando" : "parada");
  texto("i-min-ritmo", `${fmt1.format(e.ritmo || 0)} tent/s · ${fmt.format(e.tentativas || 0)} tentativas`);
  texto("i-saldo", moeda(e.saldo));
  texto("i-imaturo", `esperando liberar: ${moeda(e.imaturo)} HYX`);
  texto("i-altura", fmt.format(e.altura || 0));
  texto("i-pares", `${fmt.format(e.pares || 0)} par${e.pares === 1 ? "" : "es"}`);
  texto("i-ponta", e.ponta ? `ponta ${curto(e.ponta)}` : "—");

  // indicadores da navegação: o que está vivo aparece invertido
  const marca = (id, textoDaMarca, vivo) => {
    const el = $(id);
    if (!el) return;
    if (el.textContent !== textoDaMarca) el.textContent = textoDaMarca;
    el.classList.toggle("vivo", !!vivo);
  };
  const a = ativas[0];
  marca("n-ultrax", u.ligado ? (a && a.total ? `${Math.round((100 * a.feitas) / a.total)}%` : "ON") : "", u.ligado);
  marca("n-ia", ia.treinos ? fmt.format(ia.treinos) : "", false);
  marca("n-mineracao", e.minerando ? "ON" : "", e.minerando);
  marca("n-rede", String(e.pares ?? 0), false);
}

// ---------- mercado ----------
function desenharMercado(e) {
  const ligado = e.mercado_ligado === true;
  const moedas = Array.isArray(e.mercado) ? e.mercado : [];
  $("m-aviso").hidden = ligado && moedas.length > 0;
  texto("m-quando", !ligado ? "desligado" : e.mercado_quando ? hora(e.mercado_quando) : "buscando…");
  if (ligado && moedas.length === 0) {
    texto("m-aviso", "Buscando os preços… se não vier nada, pode ser internet fora ou o site de preços fora do ar.");
    $("m-aviso").hidden = false;
  }
  const dinheiro = (v, casas) => v.toLocaleString("pt-BR", { minimumFractionDigits: casas, maximumFractionDigits: casas });
  $("mercado").replaceChildren(...moedas.map((m) => {
    const tr = document.createElement("tr");
    const cel = (txt, classe) => {
      const td = document.createElement("td");
      td.textContent = txt;
      if (classe) td.className = classe;
      return td;
    };
    const casas = m.brl >= 100 ? 0 : 2;
    tr.append(
      cel(m.nome),
      cel(dinheiro(m.brl, casas)),
      cel(`US$ ${dinheiro(m.usd, casas)}`),
      cel(`${dinheiro(Math.abs(m.variacao), 1)}%`, m.variacao >= 0 ? "num sobe" : "num desce"),
    );
    return tr;
  }));
}

// ---------- ciclo ----------
async function ler() {
  try {
    const r = await fetch("/api/estado", { cache: "no-store" });
    const e = await r.json();
    ultimo = e;
    // a seção Ciência (ciencia.js) usa o mesmo estado, sem perguntar de novo
    dispatchEvent(new CustomEvent("hyurax:estado", { detail: e }));
    $("desligado").hidden = true;
    texto("boot-versao", e.versao ? `Hyurax ${e.versao}` : "Hyurax");
    desenharBoot(e);
    // Trancado: o nó não manda saldo, blocos nem endereço, então não há painel
    // para desenhar. A tela do cadeado é a única coisa na frente.
    if (e.trancado === true) return;
    if (ajustes.open) desenharAjustes(e);
    if (trabalho.open) desenharTrabalho(e);
    desenharSeguranca(e);
    desenharBarra(e);
    desenharMineracao(e);
    desenharInicio(e);
    desenharUltrax(e);
    avisarGpu(e);
    desenharLateral(e);
    desenharLivro(e);
    desenharFluxo(e);
    if (typeof e.paineis === "string" && e.paineis !== listaPaineis) { aplicarPaineis(e.paineis); if ($("paineis").open) desenharListaPaineis(); }
    desenharMercado(e);
    desenharMaquinas(e);
    desenharAvisoDeBloco(e);
    desenharFita(e);
    desenharGrafico(e);
  } catch {
    $("desligado").hidden = !bootEncerrado;
    if (!bootEncerrado && bootCena === "partida") {
      texto("erro-partida", "O painel não respondeu. Se isto não sair em alguns segundos, feche e abra o programa de novo.");
    }
  }
}
setInterval(() => { texto("relogio", new Date().toLocaleTimeString("pt-BR", { hour12: false })); }, 1000);
setInterval(ler, 1000);
addEventListener("resize", () => ultimo && desenharGrafico(ultimo));
prepararBotoesDePainel();
irPara(vista);
ler();
