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

// A estação 3D é opcional: sem WebGL, com a cena desligada nos ajustes, ou em
// máquina fraca, o resto do painel funciona igual. Ela só nasce depois da
// primeira leitura, porque é o estado que diz a qualidade escolhida.
let criandoEstacao = false;
function garantirEstacao(e) {
  const qualidade = e.cena || "auto";
  if (qualidade === "desligada") {
    $("cena").hidden = true;
    return;
  }
  $("cena").hidden = false;
  if (estacao || criandoEstacao) return;
  criandoEstacao = true;
  import("./estacao.js")
    .then((m) => { estacao = m.criarEstacao($("cena"), { calmo, qualidade }); if (ultimo) estacao.atualizar(ultimo); })
    .catch((erro) => console.warn("estação 3D indisponível:", erro));
}

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
const curto = (h) => `${h.slice(0, 8)}…${h.slice(-6)}`;
// O nó manda o dinheiro com ponto; em português se escreve com vírgula.
const reais = (v) => String(v || "0.00").replace(".", ",");

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
$("copiar").addEventListener("click", async () => {
  if (!ultimo || !ultimo.endereco) return;
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

function desenharEstacao(e) {
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

// ---------- primeira abertura: criar ou importar a carteira ----------
// Sem carteira não há para onde mandar a recompensa: o diálogo é obrigatório e
// só fecha depois que ela existe. Backend antigo, sem o campo "carteira", conta
// como "já tem carteira" e o diálogo nunca abre.
const SENHA_MINIMA = 10;
const PEDIDO_MAXIMO = 4000; // folga: o nó recusa corpo grande, e um carteira.txt tem menos de 1 KiB
const boas = $("boas");
let boasFase = "escolha"; // "escolha" → "pronto"
let boasPodeFechar = false;
let boasOcupado = false;
let carteiraFeita = false;

// Conta como o nó conta (caracteres, não bytes nem unidades UTF-16).
const caracteres = (s) => [...s].length;

function abrirBoas() {
  if (boas.open) return;
  boasPodeFechar = false;
  boas.showModal();
  if (boasFase === "pronto") $("pronto-titulo").focus();
  else ($("c-importar").checked ? $("arquivo") : $("senha")).focus();
}
function fecharBoas() {
  boasPodeFechar = true;
  boas.close();
}
// Esc não fecha; e se o navegador fechar mesmo assim (Esc repetido), reabre.
boas.addEventListener("cancel", (ev) => ev.preventDefault());
boas.addEventListener("close", () => { if (!boasPodeFechar) abrirBoas(); });

function desenharBoas(e) {
  const precisa = e.pode_mandar === true && e.carteira === false && !carteiraFeita;
  if (precisa) abrirBoas();
  // Carteira criada por outra aba: some daqui, mas nunca no meio de um pedido.
  else if (boas.open && boasFase === "escolha" && !boasOcupado) fecharBoas();
}

function trocarCaminho() {
  const importar = $("c-importar").checked;
  $("f-criar").hidden = importar;
  $("f-importar").hidden = !importar;
}
$("c-criar").addEventListener("change", trocarCaminho);
$("c-importar").addEventListener("change", trocarCaminho);

// Pedido em curso: os botões ficam aria-disabled (não disabled, que tiraria o foco
// do teclado) e o envio repetido é barrado por boasOcupado.
function ocupar(sim, botao, rotulo) {
  boasOcupado = sim;
  $("caminhos").disabled = sim;
  boas.setAttribute("aria-busy", String(sim));
  botao.textContent = rotulo;
  if (sim) botao.setAttribute("aria-disabled", "true");
  else botao.removeAttribute("aria-disabled");
}

// Criar: senha com no mínimo 10 caracteres, repetida igual.
function conferirSenha() {
  const a = $("senha").value, b = $("senha2").value;
  const n = caracteres(a);
  const tamanho = n >= SENHA_MINIMA, iguais = b !== "" && a === b;
  texto("r-tamanho", tamanho ? `${n} caracteres: tamanho bom` : `${n} de no mínimo ${SENHA_MINIMA} caracteres`);
  texto("r-iguais", !b ? "Repita a senha no segundo campo" : iguais ? "As duas senhas batem" : "As duas senhas ainda não batem");
  $("r-tamanho").classList.toggle("cumprida", tamanho);
  $("r-iguais").classList.toggle("cumprida", iguais);
  $("criar").disabled = !(tamanho && iguais);
}
$("senha").addEventListener("input", conferirSenha);
$("senha2").addEventListener("input", conferirSenha);
$("ver-senha").addEventListener("click", () => {
  const ver = $("senha").type === "password";
  for (const id of ["senha", "senha2"]) $(id).type = ver ? "text" : "password";
  $("ver-senha").textContent = ver ? "ocultar" : "mostrar";
  $("ver-senha").setAttribute("aria-label", ver ? "Ocultar as senhas" : "Mostrar as senhas");
});
$("f-criar").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const senha = $("senha").value, senha2 = $("senha2").value;
  if (boasOcupado || caracteres(senha) < SENHA_MINIMA || senha !== senha2) return;
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

// Importar: o texto do carteira.txt, colado ou lido do arquivo.
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
  if (boasOcupado || !conteudo.trim()) return;
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

// Pronto: o endereço, o aviso honesto e o botão de começar.
function mostrarPronto(endereco) {
  carteiraFeita = true;
  boasFase = "pronto";
  $("boas-escolha").hidden = true;
  $("boas-pronto").hidden = false;
  texto("pronto-endereco", endereco);
  const dados = typeof ultimo?.dados === "string" ? ultimo.dados : "";
  texto("pronto-pasta", dados ? `O arquivo carteira.txt fica na pasta ${dados}` : "");
  boas.setAttribute("aria-labelledby", "pronto-titulo");
  boas.setAttribute("aria-describedby", "pronto-aviso");
  if (boas.open) $("pronto-titulo").focus();
  else abrirBoas();
}

// Enquanto o pedido anda, o botão ignora cliques sem perder o foco
// (com disabled, o foco do teclado cairia fora do diálogo).
async function comBotao(botao, tarefa) {
  if (botao.getAttribute("aria-disabled") === "true") return;
  botao.setAttribute("aria-disabled", "true");
  try {
    await tarefa();
  } finally {
    botao.removeAttribute("aria-disabled");
  }
}

function abrirPasta(botao, idErro) {
  return comBotao(botao, async () => {
    texto(idErro, "");
    const res = await postar("/api/abrir-pasta");
    if (!res.ok) texto(idErro, erroDe(res, "Não deu para abrir a pasta."));
  });
}
$("abrir-pasta-carteira").addEventListener("click", () => abrirPasta($("abrir-pasta-carteira"), "erro-pronto"));
$("comecar").addEventListener("click", () => comBotao($("comecar"), async () => {
  texto("erro-pronto", "");
  const res = await postar("/api/minerar", { ligar: "1" });
  if (!res.ok) {
    texto("erro-pronto", erroDe(res, "Não deu para ligar a mineração. Tente de novo."));
    return;
  }
  fecharBoas();
  ler();
}));

// ---------- ajustes ----------
const ajustes = $("ajustes");

let qrDesenhado = "";

// Marca o perfil que bate com o que está valendo, ou "manual".
function perfilAtual(e) {
  const n = Math.max(1, e.nucleos || 1);
  const combina = { leve: [Math.max(1, Math.floor(n / 4)), 35], equilibrado: [Math.max(1, Math.floor(n / 2)), 70], turbo: [n, 100] };
  for (const [nome, [linhas, uso]] of Object.entries(combina)) {
    if (e.linhas === linhas && e.uso_cpu === uso) return nome;
  }
  return "manual";
}

function desenharQr(url) {
  const caixa = $("a-qr");
  if (qrDesenhado === url) return;
  qrDesenhado = url;
  caixa.replaceChildren();
  if (!url || typeof globalThis.qrcode !== "function") return;
  const q = globalThis.qrcode(0, "M");
  q.addData(url);
  q.make();
  // createSvgTag devolve SVG puro montado pela biblioteca, sem dado externo.
  caixa.innerHTML = q.createSvgTag({ cellSize: 4, margin: 1, scalable: true });
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
  $("a-qr-caixa").hidden = !e.na_rede;
  texto("a-url", e.url_celular || "—");
  if (e.na_rede) desenharQr(e.url_celular || "");
  for (const nome of ["auto", "alta", "media", "baixa", "desligada"]) {
    const alvo = $(`c-${nome}`);
    if (alvo) alvo.checked = (e.cena || "auto") === nome;
  }
  texto("a-dados", typeof e.dados === "string" && e.dados ? e.dados : "—");
  texto("a-versao", typeof e.versao === "string" && e.versao ? e.versao : "—");
  texto("a-rede", e.rede || "—");
  texto("a-porta", typeof e.porta_p2p !== "number" ? "—" : e.porta_p2p ? String(e.porta_p2p) : "não recebe conexões");
}

$("abrir-ajustes").addEventListener("click", () => {
  if (!ultimo || ajustes.open) return;
  const lista = Array.isArray(ultimo.sementes) ? ultimo.sementes.filter((s) => typeof s === "string") : [];
  $("sementes").value = lista.join(", ");
  for (const id of ["a-abrir-erro", "sementes-ok", "sementes-erro"]) texto(id, "");
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
// Desempenho, energia, celular e cena: cada controle manda o seu campo.
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
for (const nome of ["auto", "alta", "media", "baixa", "desligada"]) {
  $(`c-${nome}`).addEventListener("change", async () => {
    // A qualidade da cena muda o desenho inteiro: recarrega a página para valer já.
    if (await salvarAjuste({ cena: nome })) location.reload();
  });
}

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

// ---------- ciclo ----------
async function ler() {
  try {
    const r = await fetch("/api/estado", { cache: "no-store" });
    const e = await r.json();
    ultimo = e;
    $("desligado").hidden = true;
    desenharBoas(e);
    if (ajustes.open) desenharAjustes(e);
    if (trabalho.open) desenharTrabalho(e);
    desenharBarra(e);
    desenharEstacao(e);
    desenharLateral(e);
    desenharLivro(e);
    desenharFluxo(e);
    desenharFita(e);
    desenharGrafico(e);
    garantirEstacao(e);
    estacao?.atualizar(e);
  } catch {
    $("desligado").hidden = false;
  }
}
setInterval(() => { texto("relogio", new Date().toLocaleTimeString("pt-BR", { hour12: false })); }, 1000);
setInterval(ler, 1000);
addEventListener("resize", () => ultimo && desenharGrafico(ultimo));
ler();
