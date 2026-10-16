// HYURAX Compute — os painéis do cliente, do fornecedor e do administrador.
// Uma página; a tela vem do endereço depois do "#". Cada número vem da API
// (/api/v1/...); nada é inventado aqui. Texto que veio do servidor entra
// sempre como texto (textContent), nunca como HTML.

// ---------------------------------------------------------------------------
// ferramentas
// ---------------------------------------------------------------------------

const $ = (s, r = document) => r.querySelector(s);

/** Troca os filhos de `e`, ignorando o que for nulo ou falso. */
function trocar(e, ...filhos) {
  e.replaceChildren(...filhos.flat(Infinity).filter((f) => f !== null && f !== undefined && f !== false));
}

function h(tag, attrs = {}, ...filhos) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === null || v === undefined || v === false) continue;
    if (k === "class") e.className = v;
    else if (k.startsWith("on") && typeof v === "function") e.addEventListener(k.slice(2), v);
    else if (k === "style") Object.assign(e.style, v);
    else e.setAttribute(k, v === true ? "" : String(v));
  }
  for (const f of filhos.flat(Infinity)) {
    if (f === null || f === undefined || f === false) continue;
    e.append(f instanceof Node ? f : document.createTextNode(String(f)));
  }
  return e;
}

const SVG = "http://www.w3.org/2000/svg";
function s(tag, attrs = {}, ...filhos) {
  const e = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  for (const f of filhos) e.append(f);
  return e;
}

const ICONES = {
  painel: "M3 13h8V3H3zm10 8h8V11h-8zM3 21h8v-6H3zm10-18v6h8V3z",
  nova: "M12 5v14M5 12h14",
  tarefas: "M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01",
  saldo: "M3 7h18v10H3zM3 10h18M7 15h3",
  chave: "M15 7a4 4 0 1 1-4 4l-7 7v-3h3v-3h3l1.6-1.6A4 4 0 0 1 15 7z",
  pc: "M4 5h16v11H4zM8 20h8M12 16v4",
  ganhos: "M12 3v18M17 7.5c0-1.9-2.2-3.5-5-3.5S7 5.6 7 7.5 9.2 11 12 11s5 1.6 5 3.5S14.8 18 12 18s-5-1.6-5-3.5",
  admin: "M12 3l8 4v5c0 4.4-3.4 8.3-8 9-4.6-.7-8-4.6-8-9V7z",
  gente: "M16 20v-1a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v1M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM22 20v-1a4 4 0 0 0-3-3.9M16 3.1a4 4 0 0 1 0 7.8",
  alerta: "M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
  registro: "M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6M8 13h8M8 17h6",
  config: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
  conta: "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
  livro: "M4 19.5A2.5 2.5 0 0 1 6.5 17H20V3H6.5A2.5 2.5 0 0 0 4 5.5zM20 17v4H6.5A2.5 2.5 0 0 1 4 18.5",
  dinheiro: "M2 7h20v10H2zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM6 12h.01M18 12h.01",
};

function icone(nome) {
  return s("svg", { viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", "stroke-width": "1.8", "stroke-linecap": "round", "stroke-linejoin": "round", "aria-hidden": "true" }, s("path", { d: ICONES[nome] || ICONES.painel }));
}

function logo() {
  return s(
    "svg",
    { viewBox: "0 0 64 64", "aria-hidden": "true" },
    s("rect", { x: 0, y: 0, width: 64, height: 64, rx: 16, fill: "#1D1D1F" }),
    s("path", { d: "M18 39.5 46 24.5", stroke: "#F2B544", "stroke-width": 8, "stroke-linecap": "round" }),
    s("rect", { x: 15, y: 15, width: 9.5, height: 35, rx: 4.75, fill: "#ffffff" }),
    s("rect", { x: 39.5, y: 12, width: 9.5, height: 35, rx: 4.75, fill: "#ffffff" }),
  );
}

function marca(destino) {
  return h("a", { class: "marca", href: destino }, logo(), h("span", { class: "marca-texto" }, h("b", {}, "HYURAX"), h("span", {}, "Compute")));
}

const quando = (ms) => (ms ? new Date(ms).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" }) : "—");
const dia = (ms) => (ms ? new Date(ms).toLocaleDateString("pt-BR") : "—");
const num = (n, casas = 0) => (n === null || n === undefined || Number.isNaN(Number(n)) ? "—" : Number(n).toLocaleString("pt-BR", { maximumFractionDigits: casas, minimumFractionDigits: casas }));
const mib = (m) => (m >= 1024 ? `${num(m / 1024, 1)} GB` : `${num(m)} MB`);
const bytes = (b) => (b >= 1073741824 ? `${num(b / 1073741824, 1)} GB` : b >= 1048576 ? `${num(b / 1048576, 1)} MB` : `${num(Math.max(1, b / 1024))} KB`);
const duracao = (s) => {
  if (!s && s !== 0) return "—";
  if (s < 90) return `${Math.round(s)} s`;
  if (s < 5400) return `${Math.round(s / 60)} min`;
  if (s < 172800) return `${num(s / 3600, 1)} h`;
  return `${num(s / 86400, 1)} dias`;
};
const reais = (centavos, moeda = "BRL") => new Intl.NumberFormat(moeda === "USD" ? "en-US" : "pt-BR", { style: "currency", currency: moeda }).format((centavos || 0) / 100);

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

async function api(metodo, caminho, corpo) {
  const opcoes = { method: metodo, headers: { "X-Hyx-Pedido": "1" }, credentials: "same-origin" };
  if (corpo !== undefined) {
    opcoes.headers["Content-Type"] = "application/json";
    opcoes.body = JSON.stringify(corpo);
  }
  let r;
  try {
    r = await fetch(`/api/v1${caminho}`, opcoes);
  } catch {
    return { ok: false, status: 0, erro: "sem conexão com a plataforma; confira a internet" };
  }
  let dados = null;
  try {
    dados = await r.json();
  } catch {
    dados = null;
  }
  if (r.status === 401 && estado.eu && !caminho.startsWith("/auth/")) {
    estado.eu = null;
    aviso("A sessão terminou. Entre de novo.", true);
    ir("#/entrar");
  }
  return { ok: r.ok, status: r.status, dados, erro: dados?.erro || (r.ok ? "" : `erro ${r.status}`) };
}

const obter = (c) => api("GET", c);
const postar = (c, corpo = {}) => api("POST", c, corpo);

function aviso(texto, erro = false) {
  const a = h("div", { class: `aviso${erro ? " erro" : ""}` }, texto);
  $("#avisos").append(a);
  setTimeout(() => a.remove(), erro ? 7000 : 4000);
}

/** Pergunta numa janela. `campos`: [{ id, rotulo, tipo, valor, ajuda }]. Devolve os valores ou null. */
function perguntar(titulo, texto, campos = [], botao = "Confirmar") {
  const d = $("#dialogo");
  $("#dialogo-titulo").textContent = titulo;
  $("#dialogo-texto").textContent = texto || "";
  $("#dialogo-ok").textContent = botao;
  const caixa = $("#dialogo-campos");
  trocar(caixa, 
    ...campos.map((c) =>
      h(
        "label",
        { class: "campo" },
        c.rotulo,
        c.tipo === "textarea"
          ? h("textarea", { id: `dlg-${c.id}`, required: c.obrigatorio ? true : null }, c.valor || "")
          : h("input", { id: `dlg-${c.id}`, type: c.tipo || "text", value: c.valor ?? "", required: c.obrigatorio ? true : null, step: c.passo || null }),
        c.ajuda ? h("small", {}, c.ajuda) : null,
      ),
    ),
  );
  return new Promise((resolver) => {
    const fechar = (v) => {
      d.close();
      $("#dialogo-form").onsubmit = null;
      $("#dialogo-cancelar").onclick = null;
      resolver(v);
    };
    $("#dialogo-form").onsubmit = (ev) => {
      ev.preventDefault();
      const v = {};
      for (const c of campos) v[c.id] = $(`#dlg-${c.id}`).value;
      fechar(v);
    };
    $("#dialogo-cancelar").onclick = () => fechar(null);
    d.oncancel = () => fechar(null);
    d.showModal();
    caixa.querySelector("input, textarea")?.focus();
  });
}

// ---------------------------------------------------------------------------
// pedaços de tela
// ---------------------------------------------------------------------------

const NOMES_TAREFA = {
  CREATED: ["criada", ""], QUEUED: ["na fila", "info"], SCHEDULED: ["agendada", "info"], DISPATCHED: ["enviada", "luz"], RUNNING: ["rodando", "luz"],
  VERIFYING: ["conferindo", "luz"], COMPLETED: ["concluída", "ok"], FAILED: ["falhou", "falha"], CANCELLED: ["cancelada", ""], TIMEOUT: ["prazo vencido", "atencao"], REJECTED: ["recusada", "falha"],
};
const NOMES_NO = {
  IDLE: ["livre", "ok"], BUSY: ["trabalhando", "luz"], OFFLINE: ["desligado", ""], UNSTABLE: ["instável", "atencao"], PAUSED: ["pausado", ""],
  QUARANTINED: ["quarentena", "falha"], MAINTENANCE: ["manutenção", "atencao"], BANNED: ["banido", "falha"], REMOVED: ["removido", ""],
};
const NOMES_PAGAMENTO = { pendente: ["pendente", "info"], confirmado: ["confirmado", "ok"], cancelado: ["cancelado", ""], estornado: ["estornado", "falha"] };
const NOMES_SAQUE = { solicitado: ["solicitado", "info"], aprovado: ["aprovado", "luz"], pago: ["pago", "ok"], recusado: ["recusado", "falha"] };

function pilula(tabela, chave) {
  const [nome, classe] = tabela[chave] || [String(chave || "—").toLowerCase(), ""];
  return h("span", { class: `pilula ${classe}` }, nome);
}

function numeros(...itens) {
  const lista = itens.flat().filter(Boolean);
  const classe = lista.length === 3 ? "numeros n3" : lista.length % 3 === 0 ? "numeros n6" : "numeros";
  return h("div", { class: classe }, lista);
}

function numero(rotulo, valor, nota) {
  return h("div", { class: "numero" }, h("span", {}, rotulo), h("strong", {}, valor), nota ? h("span", {}, nota) : null);
}

function fatos(pares) {
  return h("dl", { class: "fatos" }, pares.filter(Boolean).map(([k, v, mono]) => [h("dt", {}, k), h("dd", { class: mono ? "mono" : null }, v ?? "—")]));
}

function tabela(colunas, linhas, aoClicar) {
  return h(
    "div",
    { class: "rolagem" },
    h(
      "table",
      {},
      h("thead", {}, h("tr", {}, colunas.map((c) => h("th", { class: c.n ? "n" : null, scope: "col" }, c.t)))),
      h(
        "tbody",
        {},
        linhas.map((l) =>
          h(
            "tr",
            { class: aoClicar ? "clicavel" : null, onclick: aoClicar ? () => aoClicar(l) : null },
            colunas.map((c) => h("td", { class: c.n ? "n" : null }, c.v(l))),
          ),
        ),
      ),
    ),
  );
}

function vazio(titulo, texto, acao) {
  return h("div", { class: "vazio" }, h("b", {}, titulo), texto ? h("span", {}, texto) : null, acao || null);
}

function barra(feitas, total, ok) {
  const pct = total > 0 ? Math.min(100, (feitas / total) * 100) : 0;
  const e = h("div", { class: `progresso${ok ? " ok" : ""}`, role: "progressbar", "aria-valuemin": 0, "aria-valuemax": total || 0, "aria-valuenow": feitas || 0 }, h("span", {}));
  e.firstChild.style.width = `${pct}%`;
  return e;
}

/** Gráfico de linha simples, com mínimo e máximo escritos. */
function grafico(rotulo, valores, unidade, maximoFixo) {
  const vals = valores.filter((v) => typeof v === "number" && Number.isFinite(v));
  const caixa = h("div", { class: "grafico" });
  const ultimo = vals.length ? vals[vals.length - 1] : null;
  caixa.append(h("div", { class: "grafico-cab" }, h("span", {}, rotulo), h("b", {}, ultimo === null ? "sem dado" : `${num(ultimo, 1)} ${unidade}`)));
  if (vals.length < 2) {
    caixa.append(h("small", {}, ultimo === null ? "O computador não informa este número." : "Juntando amostras…"));
    return caixa;
  }
  const max = maximoFixo ?? (Math.max(...vals) * 1.1 || 1);
  const L = 300;
  const A = 72;
  const passo = L / (vals.length - 1);
  const pontos = vals.map((v, i) => `${(i * passo).toFixed(1)},${(A - (Math.min(v, max) / max) * (A - 4) - 2).toFixed(1)}`);
  caixa.append(
    s(
      "svg",
      { viewBox: `0 0 ${L} ${A}`, preserveAspectRatio: "none", role: "img", "aria-label": `${rotulo}: de ${num(Math.min(...vals), 1)} a ${num(Math.max(...vals), 1)} ${unidade} nos últimos ${vals.length} pulsos` },
      s("line", { class: "eixo", x1: 0, y1: A - 1, x2: L, y2: A - 1 }),
      s("polygon", { class: "area-grafico", points: `0,${A} ${pontos.join(" ")} ${L},${A}` }),
      s("polyline", { class: "linha-grafico", points: pontos.join(" ") }),
    ),
  );
  caixa.append(h("small", {}, `mín. ${num(Math.min(...vals), 1)} · máx. ${num(Math.max(...vals), 1)} ${unidade} · últimos ${duracao(vals.length * 5)}`));
  return caixa;
}

function erroForm(msg) {
  return msg ? h("p", { class: "erro-form", role: "alert" }, msg) : null;
}

function cabecalho(titulo, subtitulo, ...acoes) {
  return h("header", { class: "tela-cab" }, h("div", {}, h("h1", {}, titulo), subtitulo ? h("p", { class: "subtitulo" }, subtitulo) : null), acoes.length ? h("div", { class: "acoes" }, acoes) : null);
}

function json(dados, rotulo = "ver os dados") {
  return h("details", { class: "json" }, h("summary", {}, rotulo), h("pre", {}, JSON.stringify(dados, null, 2)));
}

// ---------------------------------------------------------------------------
// estado e navegação
// ---------------------------------------------------------------------------

const estado = { eu: null, catalogo: null, mercado: null, atualizar: null, intervalo: null };

function papel(p) {
  return (estado.eu?.papeis || []).includes(p);
}

function ir(destino) {
  if (location.hash === destino) mostrar();
  else location.hash = destino;
}

const ROTAS = [
  [/^#\/$/, () => (estado.eu ? telaPainel() : telaInicio())],
  [/^#\/entrar$/, () => telaEntrar()],
  [/^#\/cadastro$/, () => telaCadastro()],
  [/^#\/recuperar$/, () => telaRecuperar()],
  [/^#\/precos$/, () => telaInicio("precos")],
  [/^#\/painel$/, () => telaPainel(), true],
  [/^#\/nova$/, () => telaNova(), true],
  [/^#\/tarefas$/, () => telaTarefas(), true],
  [/^#\/tarefa\/(\d+)$/, (m) => telaTarefa(Number(m[1])), true],
  [/^#\/saldo$/, () => telaSaldo(), true],
  [/^#\/chaves$/, () => telaChaves(), true],
  [/^#\/computadores$/, () => telaComputadores(), true],
  [/^#\/computador\/(\d+)$/, (m) => telaComputador(Number(m[1])), true],
  [/^#\/ganhos$/, () => telaGanhos(), true],
  [/^#\/conta$/, () => telaConta(), true],
  [/^#\/admin$/, () => telaAdmin(), true],
  [/^#\/admin\/usuarios$/, () => telaAdminUsuarios(), true],
  [/^#\/admin\/nos$/, () => telaAdminNos(), true],
  [/^#\/admin\/tarefas$/, () => telaAdminTarefas(), true],
  [/^#\/admin\/pagamentos$/, () => telaAdminPagamentos(), true],
  [/^#\/admin\/saques$/, () => telaAdminSaques(), true],
  [/^#\/admin\/alertas$/, () => telaAdminAlertas(), true],
  [/^#\/admin\/registro$/, () => telaAdminRegistro(), true],
  [/^#\/admin\/config$/, () => telaAdminConfig(), true],
  [/^#\/admin\/livro$/, () => telaAdminLivro(), true],
];

async function mostrar() {
  clearInterval(estado.intervalo);
  estado.atualizar = null;
  const hash = location.hash || "#/";
  const rota = ROTAS.find(([re]) => re.test(hash));
  if (!rota) return ir("#/");
  const [re, fn, precisaEntrar] = rota;
  if (precisaEntrar && !estado.eu) return ir("#/entrar");
  if (!precisaEntrar && estado.eu && /^#\/(entrar|cadastro|recuperar)$/.test(hash)) return ir("#/painel");
  const m = hash.match(re);
  const corpo = await fn(m);
  const raiz = $("#raiz");
  if (estado.eu && precisaEntrar !== undefined && hash !== "#/precos") trocar(raiz, casca(corpo, hash));
  else if (estado.eu && hash === "#/") trocar(raiz, casca(corpo, "#/painel"));
  else trocar(raiz, cascaPublica(corpo));
  $("#conteudo")?.focus({ preventScroll: true });
  if (estado.atualizar) estado.intervalo = setInterval(() => estado.atualizar?.(), 4000);
}

function cascaPublica(corpo) {
  return h(
    "div",
    {},
    h(
      "header",
      { class: "topo" },
      h(
        "div",
        { class: "topo-dentro" },
        marca("#/"),
        h(
          "nav",
          { "aria-label": "Principal" },
          h("a", { href: "#/precos" }, "Preços"),
          h("a", { href: "#/" }, "Como funciona"),
          estado.eu ? h("a", { class: "botao-escuro", href: "#/painel" }, "Abrir o painel") : [h("a", { href: "#/entrar" }, "Entrar"), h("a", { class: "botao", href: "#/cadastro" }, "Criar conta")],
        ),
      ),
    ),
    h("main", { id: "conteudo", tabindex: "-1" }, corpo),
    h(
      "footer",
      { class: "rodape" },
      h(
        "div",
        { class: "rodape-dentro" },
        h("span", {}, "HYURAX Compute · computação sob demanda, conferida antes de cobrar"),
        h("span", {}, h("a", { href: "/termos.html" }, "Termos de uso"), " · ", h("a", { href: "/privacidade.html" }, "Privacidade"), estado.mercado?.contato ? [" · Suporte: ", estado.mercado.contato] : null),
        h("span", {}, "Créditos são saldo de uso: não são criptomoeda, investimento nem promessa de valorização."),
      ),
    ),
  );
}

function casca(corpo, hash) {
  const filhos = { "#/tarefas": "#/tarefa/", "#/computadores": "#/computador/" };
  const item = (destino, rotulo, ic, contador) => {
    const ativo = hash === destino || (filhos[destino] && hash.startsWith(filhos[destino]));
    return h("a", { class: "item", href: destino, "aria-current": ativo ? "page" : null }, icone(ic), rotulo, contador ? h("span", { class: "contador" }, contador) : null);
  };
  const eu = estado.eu;
  return h(
    "div",
    { class: "app" },
    h(
      "nav",
      { class: "lateral", "aria-label": "Painel" },
      h("div", { class: "lateral-topo" }, marca("#/painel"), h("button", { type: "button", class: "botao-leve botao-pequeno abrir-menu", "aria-expanded": "false", onclick: (ev) => { const nav = ev.currentTarget.closest(".lateral"); const aberto = nav.classList.toggle("aberta"); ev.currentTarget.setAttribute("aria-expanded", String(aberto)); } }, "Menu")),
      h("div", { class: "lateral-itens" },
      papel("CUSTOMER") ? [h("div", { class: "grupo" }, "Comprar computação"), item("#/painel", "Visão geral", "painel"), item("#/nova", "Nova tarefa", "nova"), item("#/tarefas", "Tarefas", "tarefas"), item("#/saldo", "Saldo e faturas", "saldo"), item("#/chaves", "Chaves de API", "chave")] : null,
      papel("PROVIDER") ? [h("div", { class: "grupo" }, "Fornecer computação"), item("#/computadores", "Computadores", "pc"), item("#/ganhos", "Ganhos e saques", "ganhos")] : null,
      papel("ADMIN")
        ? [
            h("div", { class: "grupo" }, "Administração"),
            item("#/admin", "Painel", "admin"),
            item("#/admin/usuarios", "Usuários", "gente"),
            item("#/admin/nos", "Nós", "pc"),
            item("#/admin/tarefas", "Todas as tarefas", "tarefas"),
            item("#/admin/pagamentos", "Pagamentos", "dinheiro"),
            item("#/admin/saques", "Saques", "ganhos"),
            item("#/admin/alertas", "Alertas", "alerta"),
            item("#/admin/registro", "Registro", "registro"),
            item("#/admin/livro", "Livro", "livro"),
            item("#/admin/config", "Configuração", "config"),
          ]
        : null,
      h("div", { class: "grupo" }, "Conta"),
      item("#/conta", "Minha conta", "conta"),
      ),
      h(
        "div",
        { class: "lateral-pe" },
        h("b", {}, eu.nome),
        h("small", {}, eu.email),
        h(
          "div",
          { class: "acoes" },
          h("button", { type: "button", class: "botao-leve", onclick: trocarTema }, "Tema"),
          h("button", { type: "button", class: "botao-leve", onclick: sair }, "Sair"),
        ),
      ),
    ),
    h("main", { class: "conteudo", id: "conteudo", tabindex: "-1" }, corpo),
  );
}

function trocarTema() {
  const ordem = ["sistema", "claro", "escuro"];
  const atual = document.documentElement.dataset.tema || "sistema";
  const novo = ordem[(ordem.indexOf(atual) + 1) % ordem.length];
  document.documentElement.dataset.tema = novo;
  try {
    localStorage.setItem("hyx-tema", novo);
  } catch {
    /* sem armazenamento: o tema volta ao do sistema na próxima vez */
  }
  aviso(`Tema: ${novo}`);
}

async function sair() {
  await postar("/auth/sair");
  estado.eu = null;
  ir("#/");
}

async function carregarEu() {
  const r = await obter("/auth/eu");
  estado.eu = r.ok ? r.dados : null;
  const teste = r.ok ? r.dados.ambiente_teste : estado.mercado?.modo_teste;
  $("#faixa-teste").hidden = !teste;
}

async function catalogo() {
  if (!estado.catalogo) {
    const r = await obter("/catalogo");
    estado.catalogo = r.ok ? r.dados : { motores: [] };
  }
  return estado.catalogo;
}

// ---------------------------------------------------------------------------
// público
// ---------------------------------------------------------------------------

async function telaInicio(ancora) {
  const r = await obter("/marketplace");
  const m = r.ok ? r.dados : null;
  estado.mercado = m;
  $("#faixa-teste").hidden = !m?.modo_teste;
  const brl = m?.precos?.find((p) => p.moeda === "BRL");
  const cap = m?.capacidade || {};
  const vivo = h(
    "aside",
    { class: "painel-ao-vivo", "aria-label": "A rede agora" },
    h("span", { class: `ao-vivo${cap.nos_online ? "" : " fora"}` }, cap.nos_online ? "Capacidade online agora" : "Nenhum computador online agora"),
    h("dl", {}, h("div", {}, h("dt", {}, "Computadores"), h("dd", {}, num(cap.nos_online))), h("div", {}, h("dt", {}, "Núcleos de CPU"), h("dd", {}, num(cap.nucleos))), h("div", {}, h("dt", {}, "Memória"), h("dd", {}, mib(cap.ram_mib || 0))), h("div", {}, h("dt", {}, "GPUs"), h("dd", {}, num((cap.gpus || []).length)))),
    brl
      ? h(
          "div",
          {},
          h("div", { class: "preco-linha" }, h("span", {}, "CPU, 4 núcleos"), h("b", {}, `${brl.cpu_4_nucleos_hora}/h`)),
          h("div", { class: "preco-linha" }, h("span", {}, "Memória"), h("b", {}, `${brl.ram_gb_hora} por GB·h`)),
          h("div", { class: "preco-linha" }, h("span", {}, "GPU"), h("b", {}, `${brl.gpu_hora}/h`)),
        )
      : null,
  );
  const motores = m?.motores || [];
  const tela = h(
    "div",
    { class: "publico" },
    h(
      "section",
      { class: "abertura" },
      h(
        "div",
        { class: "abertura-texto" },
        h("span", { class: "rotulo" }, "Mercado de computação"),
        h("h1", {}, "Computação sob demanda, conferida antes de cobrar."),
        h("p", { class: "lide" }, "Quem precisa de processamento manda a tarefa e paga só o trabalho que passou na conferência. Quem tem computador parado empresta a máquina e recebe por isso, sem dar acesso a nada do que é seu."),
        h("div", { class: "acoes" }, h("a", { class: "botao botao-grande", href: "#/cadastro" }, "Quero processar"), h("a", { class: "botao-leve botao-grande", href: "#/cadastro" }, "Quero fornecer meu computador")),
      ),
      vivo,
    ),
    h(
      "section",
      { class: "secao" },
      h("div", { class: "secao-cab" }, h("span", { class: "rotulo" }, "Como funciona"), h("h2", {}, "Dois lados, um caminho só"), h("p", {}, "A plataforma escolhe o computador mais adequado para cada pedaço da tarefa, confere o resultado e só então cobra e paga.")),
      h(
        "div",
        { class: "dois-lados" },
        h(
          "div",
          { class: "lado" },
          h("h3", {}, "Para quem precisa de computação"),
          h(
            "ol",
            {},
            h("li", {}, h("div", {}, h("b", {}, "Crie a conta e adicione saldo"), h("span", {}, "Por Pix. O saldo é de uso: não vira investimento nem criptomoeda."))),
            h("li", {}, h("div", {}, h("b", {}, "Escolha o motor e o tamanho"), h("span", {}, "Você vê a estimativa antes, e só a estimativa fica reservada."))),
            h("li", {}, h("div", {}, h("b", {}, "Acompanhe e receba"), h("span", {}, "Cada unidade é conferida. No fim, você paga o que foi conferido e o resto volta."))),
          ),
        ),
        h(
          "div",
          { class: "lado" },
          h("h3", {}, "Para quem tem computador parado"),
          h(
            "ol",
            {},
            h("li", {}, h("div", {}, h("b", {}, "Crie a conta de fornecedor"), h("span", {}, "E gere um código de vínculo no painel."))),
            h("li", {}, h("div", {}, h("b", {}, "Cole o código no programa Hyurax"), h("span", {}, "Você decide quantos núcleos e quanta memória empresta, e pausa quando quiser."))),
            h("li", {}, h("div", {}, h("b", {}, "Receba pelo trabalho conferido"), h("span", {}, `A sua parte é ${num(m?.parte_do_fornecedor_pct ?? 70)}% do que o cliente paga, liberada depois de ${num(m?.retencao_dias ?? 7)} dias.`))),
          ),
        ),
      ),
    ),
    h(
      "section",
      { class: "secao", id: "precos" },
      h("div", { class: "secao-cab" }, h("span", { class: "rotulo" }, "Preços"), h("h2", {}, "Pague pelo trabalho conferido, não pelo tempo de tela"), h("p", {}, "Cada motor conta as operações que faz por um modelo de custo publicado. A plataforma confere o resultado e cobra essas operações, convertidas em horas de um núcleo de referência (1 bilhão de operações por segundo). Máquina lenta não cobra mais por isso.")),
      h(
        "div",
        { class: "bloco" },
        tabela(
          [{ t: "Moeda", v: (p) => p.moeda }, { t: "CPU por núcleo-hora", v: (p) => p.cpu_nucleo_hora, n: true }, { t: "CPU, 4 núcleos por hora", v: (p) => p.cpu_4_nucleos_hora, n: true }, { t: "Memória por GB-hora", v: (p) => p.ram_gb_hora, n: true }, { t: "GPU por hora", v: (p) => p.gpu_hora, n: true }],
          m?.precos || [],
        ),
        h("p", { class: "nota" }, "Preços por hora de um núcleo de referência (1 bilhão de operações por segundo). Antes de criar a tarefa você vê a estimativa; só ela fica reservada, e o que não for usado volta ao saldo."),
      ),
    ),
    h(
      "section",
      { class: "secao" },
      h("div", { class: "secao-cab" }, h("span", { class: "rotulo" }, "Motores disponíveis"), h("h2", {}, "O que dá para calcular hoje")),
      h("div", { class: "garantias" }, motores.map((x) => h("div", { class: "garantia" }, h("b", {}, x.categoria), h("span", {}, x.nome), x.gpu ? h("small", {}, "roda também na GPU") : null))),
    ),
    h(
      "section",
      { class: "secao" },
      h("div", { class: "secao-cab" }, h("span", { class: "rotulo" }, "Segurança"), h("h2", {}, "O cliente nunca entra no computador de ninguém")),
      h(
        "div",
        { class: "garantias" },
        h("div", { class: "garantia" }, h("b", {}, "SÓ MOTORES PRONTOS"), h("span", {}, "O pedido leva parâmetros, nunca código nem comando. Só os motores que vêm no programa rodam.")),
        h("div", { class: "garantia" }, h("b", {}, "SEM ARQUIVO, SEM REDE"), h("span", {}, "O motor não abre arquivo, não acessa a rede de casa e não vê nada do dono da máquina.")),
        h("div", { class: "garantia" }, h("b", {}, "RESULTADO CONFERIDO"), h("span", {}, "Prova matemática no servidor (Freivalds), concordância entre computadores de donos diferentes e reputação de cada nó.")),
        h("div", { class: "garantia" }, h("b", {}, "TUDO REGISTRADO"), h("span", {}, "Cada evento vai para um diário com hash encadeado. A equipe consegue parar uma tarefa, um nó ou tudo de uma vez.")),
      ),
    ),
  );
  if (ancora) setTimeout(() => document.getElementById(ancora)?.scrollIntoView(), 30);
  return tela;
}

function telaEntrar() {
  let erro = null;
  const form = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const r = await postar("/auth/entrar", { email: $("#en-email").value, senha: $("#en-senha").value });
        if (!r.ok) {
          erro?.remove();
          erro = erroForm(r.erro);
          form.prepend(erro);
          return;
        }
        await carregarEu();
        ir("#/painel");
      },
    },
    h("label", { class: "campo" }, "E-mail", h("input", { id: "en-email", type: "email", autocomplete: "username", required: true })),
    h("label", { class: "campo" }, "Senha", h("input", { id: "en-senha", type: "password", autocomplete: "current-password", required: true })),
    h("div", { class: "acoes" }, h("button", { type: "submit", class: "botao" }, "Entrar"), h("a", { href: "#/recuperar" }, "Esqueci a senha")),
  );
  return h("div", { class: "porta" }, h("h1", {}, "Entrar"), form, h("p", { class: "nota" }, "Ainda não tem conta? ", h("a", { href: "#/cadastro" }, "Criar conta")));
}

function telaCadastro() {
  let erro = null;
  const opcao = (id, titulo, texto, marcada) => h("label", { class: "papel-opcao" }, h("input", { id, type: "checkbox", checked: marcada ? true : null }), h("div", {}, h("b", {}, titulo), h("span", {}, texto)));
  const form = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const corpo = {
          nome: $("#cd-nome").value,
          email: $("#cd-email").value,
          senha: $("#cd-senha").value,
          moeda: $("#cd-moeda").value,
          pais: $("#cd-pais").value,
          cliente: $("#cd-cliente").checked,
          fornecedor: $("#cd-fornecedor").checked,
          aceite: $("#cd-aceite").checked,
        };
        const r = await postar("/auth/cadastro", corpo);
        if (!r.ok) {
          erro?.remove();
          erro = erroForm(r.erro);
          form.prepend(erro);
          return;
        }
        await carregarEu();
        trocar($("#raiz"), cascaPublica(telaCodigos(r.dados.codigos_de_recuperacao)));
      },
    },
    h("fieldset", { class: "campos" }, h("legend", { class: "visualmente-escondido" }, "Como você vai usar"), h("div", { class: "papeis" }, opcao("cd-cliente", "Comprar computação", "Mandar tarefas e pagar pelo que foi conferido.", true), opcao("cd-fornecedor", "Fornecer computação", "Emprestar um computador e receber por isso.", false))),
    h("label", { class: "campo" }, "Nome", h("input", { id: "cd-nome", type: "text", autocomplete: "name", required: true, maxlength: 80 })),
    h("label", { class: "campo" }, "E-mail", h("input", { id: "cd-email", type: "email", autocomplete: "email", required: true })),
    h("label", { class: "campo" }, "Senha", h("input", { id: "cd-senha", type: "password", autocomplete: "new-password", required: true, minlength: 10 }), h("small", {}, "Pelo menos 10 caracteres. Uma frase de três ou quatro palavras funciona bem.")),
    h(
      "div",
      { class: "campos duas" },
      h("label", { class: "campo" }, "Moeda", h("select", { id: "cd-moeda" }, h("option", { value: "BRL" }, "Real (R$)"), h("option", { value: "USD" }, "Dólar (US$)"))),
      h("label", { class: "campo" }, "País", h("input", { id: "cd-pais", type: "text", value: "BR", maxlength: 2, autocomplete: "country" })),
    ),
    h(
      "label",
      { class: "checar" },
      h("input", { id: "cd-aceite", type: "checkbox", required: true }),
      h("span", {}, "Li e aceito os ", h("a", { href: "/termos.html", target: "_blank", rel: "noopener" }, "termos de uso"), " e a ", h("a", { href: "/privacidade.html", target: "_blank", rel: "noopener" }, "política de privacidade"), ". Sei que saldo é crédito de uso, não investimento."),
    ),
    h("button", { type: "submit", class: "botao" }, "Criar conta"),
  );
  return h("div", { class: "porta" }, h("h1", {}, "Criar conta"), form, h("p", { class: "nota" }, "Já tem conta? ", h("a", { href: "#/entrar" }, "Entrar")));
}

function telaCodigos(codigos) {
  return h(
    "div",
    { class: "porta" },
    h("h1", {}, "Guarde os códigos de recuperação"),
    h(
      "div",
      { class: "bloco" },
      h("p", {}, "Se você esquecer a senha, um destes códigos troca a senha sem precisar de e-mail. Cada um vale uma vez. Eles não aparecem de novo."),
      h("ul", { class: "codigos" }, codigos.map((c) => h("li", {}, c))),
      h(
        "div",
        { class: "acoes" },
        h(
          "button",
          {
            type: "button",
            class: "botao-leve",
            onclick: async () => {
              try {
                await navigator.clipboard.writeText(codigos.join("\n"));
                aviso("Códigos copiados.");
              } catch {
                aviso("Não consegui copiar; anote à mão.", true);
              }
            },
          },
          "Copiar",
        ),
        h("a", { class: "botao", href: "#/painel" }, "Já guardei, continuar"),
      ),
    ),
  );
}

function telaRecuperar() {
  let msg = null;
  const form = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const r = await postar("/auth/recuperar", { email: $("#rc-email").value, codigo: $("#rc-codigo").value, nova: $("#rc-nova").value });
        msg?.remove();
        msg = r.ok ? h("p", { class: "ok-form" }, r.dados.texto) : erroForm(r.erro);
        form.prepend(msg);
      },
    },
    h("label", { class: "campo" }, "E-mail", h("input", { id: "rc-email", type: "email", required: true })),
    h("label", { class: "campo" }, "Código de recuperação", h("input", { id: "rc-codigo", type: "text", required: true, autocomplete: "one-time-code", placeholder: "abcd-efgh-ijkl" })),
    h("label", { class: "campo" }, "Senha nova", h("input", { id: "rc-nova", type: "password", required: true, minlength: 10, autocomplete: "new-password" })),
    h("button", { type: "submit", class: "botao" }, "Trocar a senha"),
  );
  return h("div", { class: "porta" }, h("h1", {}, "Recuperar a conta"), form, h("p", { class: "nota" }, "Perdeu os códigos? Fale com o suporte para conferirmos a conta por outro canal."));
}

// ---------------------------------------------------------------------------
// cliente
// ---------------------------------------------------------------------------

async function telaPainel() {
  if (!papel("CUSTOMER")) return papel("PROVIDER") ? telaComputadores() : papel("ADMIN") ? telaAdmin() : h("div", {}, "Sem papel ativo.");
  const [saldo, tarefas, fatura] = await Promise.all([obter("/saldo"), obter("/tarefas"), obter("/faturas")]);
  const sd = saldo.dados || {};
  const lista = tarefas.dados?.tarefas || [];
  const ativas = lista.filter((t) => !["COMPLETED", "FAILED", "CANCELLED", "TIMEOUT", "REJECTED"].includes(t.estado));
  const tela = h(
    "section",
    { class: "tela" },
    cabecalho(`Olá, ${estado.eu.nome.split(" ")[0]}`, "O seu saldo, o que está rodando e o que já gastou este mês.", h("a", { class: "botao", href: "#/nova" }, "Nova tarefa")),
    h(
      "div",
      { class: "heroi" },
      h("div", {}, h("small", {}, "Saldo disponível"), h("strong", {}, sd.disponivel?.texto || "—"), h("p", {}, `Reservado em tarefas em andamento: ${sd.reservado?.texto || "—"}`)),
      h("div", { class: "acoes" }, h("a", { class: "botao", href: "#/saldo" }, "Adicionar saldo"), h("a", { class: "botao-leve", href: "#/tarefas" }, "Ver tarefas")),
    ),
    numeros( numero("Tarefas em andamento", num(ativas.length)), numero("Tarefas no total", num(lista.length)), numero("Gasto neste mês", fatura.dados?.total_gasto || "—"), numero("Pago neste mês", fatura.dados?.total_pago || "—")),
    h(
      "div",
      { class: "bloco" },
      h("div", { class: "bloco-cab" }, h("h2", {}, "Em andamento"), h("a", { href: "#/tarefas" }, "Todas")),
      ativas.length ? listaTarefas(ativas) : vazio("Nada rodando agora", "Crie uma tarefa e acompanhe aqui cada unidade sendo calculada e conferida.", h("a", { class: "botao", href: "#/nova" }, "Nova tarefa")),
    ),
  );
  return tela;
}

function listaTarefas(lista) {
  return tabela(
    [
      { t: "Nº", v: (t) => `#${t.id}`, n: true },
      { t: "Motor", v: (t) => h("div", {}, t.nome, t.descricao ? h("span", { class: "nota" }, t.descricao.slice(0, 80)) : null) },
      { t: "Estado", v: (t) => pilula(NOMES_TAREFA, t.estado) },
      { t: "Progresso", v: (t) => h("div", {}, barra(t.progresso.concluidas, t.progresso.unidades, t.estado === "COMPLETED"), h("span", { class: "nota" }, `${num(t.progresso.concluidas)} de ${num(t.progresso.unidades)} unidades`)) },
      { t: "Custo", v: (t) => t.custo.cobrado || t.custo.ate_agora, n: true },
      { t: "Criada", v: (t) => quando(t.criada_ms) },
    ],
    lista,
    (t) => ir(`#/tarefa/${t.id}`),
  );
}

async function telaTarefas() {
  const r = await obter("/tarefas");
  const lista = r.dados?.tarefas || [];
  const corpo = h("div", { class: "bloco" }, lista.length ? listaTarefas(lista) : vazio("Nenhuma tarefa ainda", "A primeira tarefa leva um minuto para montar.", h("a", { class: "botao", href: "#/nova" }, "Nova tarefa")));
  estado.atualizar = async () => {
    const n = await obter("/tarefas");
    if (n.ok && n.dados.tarefas.length) trocar(corpo, listaTarefas(n.dados.tarefas));
  };
  return h("section", { class: "tela" }, cabecalho("Tarefas", "Clique numa tarefa para ver as unidades, a conferência e o resultado.", h("a", { class: "botao", href: "#/nova" }, "Nova tarefa")), corpo);
}

const CODIFICAR = {
  inteiro: (v) => Math.round(v),
  selecao: (v) => Math.round(v * 10000) + 10000,
  fracao: (v) => Math.round(v * 10000),
  mili: (v) => Math.round(v * 1000),
  mili_deslocado: (v) => Math.round(v * 1000) + 20000,
};

async function telaNova() {
  const cat = await catalogo();
  const motores = cat.motores || [];
  let escolhido = motores[0];
  const grade = h("div", { class: "motores", role: "group", "aria-label": "Motor" });
  const campos = h("div", { class: "campos" });
  const resumo = h("aside", { class: "bloco resumo", "aria-live": "polite" }, h("h2", {}, "Estimativa"), h("p", { class: "nota" }, "Preencha os campos para ver o preço."));
  let espera = null;

  const desenharGrade = () =>
    trocar(grade, 
      ...motores.map((m) =>
        h(
          "button",
          { type: "button", class: "motor", "aria-pressed": String(m === escolhido), onclick: () => { escolhido = m; desenharGrade(); desenharCampos(); } },
          h("span", { class: "cat" }, m.categoria),
          h("b", {}, m.nome),
          h("span", {}, m.descricao),
        ),
      ),
    );

  const campoNum = (id, c) =>
    h("label", { class: "campo" }, c.rotulo, h("input", { id, type: "number", min: c.min, max: c.max, step: c.passo || 1, value: c.padrao, oninput: estimarDepois }), h("small", {}, `de ${num(c.min, c.passo < 1 ? 2 : 0)} a ${num(c.max, c.passo < 1 ? 2 : 0)}`));

  function desenharCampos() {
    const m = escolhido;
    trocar(campos, 
      h("div", { class: "bloco" }, h("h2", {}, m.nome), h("p", { class: "nota" }, `Conferência: ${m.verificacao}. Cada unidade é ${m.unidade_significa}.`)),
      h(
        "div",
        { class: "bloco" },
        h("h3", {}, "O problema"),
        h("div", { class: "campos duas" }, campoNum("nt-tamanho", m.tamanho), m.passos ? campoNum("nt-passos", m.passos) : null, ...(m.parametros || []).map((p, i) => campoNum(`nt-p${i}`, p))),
      ),
      h(
        "div",
        { class: "bloco" },
        h("h3", {}, "Recursos"),
        h(
          "div",
          { class: "campos duas" },
          h("label", { class: "campo" }, "Unidades", h("input", { id: "nt-unidades", type: "number", min: 1, max: 100000, value: m.unidades, oninput: estimarDepois }), h("small", {}, `cada uma é ${m.unidade_significa}`)),
          h("label", { class: "campo" }, "Núcleos em paralelo", h("input", { id: "nt-paralelo", type: "number", min: 1, max: 256, value: 4, oninput: estimarDepois }), h("small", {}, "unidades rodando ao mesmo tempo")),
          h(
            "label",
            { class: "campo" },
            "Cópias por unidade",
            h("select", { id: "nt-copias", onchange: estimarDepois }, h("option", { value: "1" }, "1 · conferência por amostragem"), h("option", { value: "2" }, "2 · concordância sempre"), h("option", { value: "3" }, "3 · maioria de três")),
            h("small", {}, "em computadores de donos diferentes"),
          ),
          h(
            "label",
            { class: "campo" },
            "Prazo",
            h("select", { id: "nt-prazo", onchange: estimarDepois }, h("option", { value: "3600" }, "1 hora"), h("option", { value: "21600" }, "6 horas"), h("option", { value: "86400", selected: true }, "1 dia"), h("option", { value: "259200" }, "3 dias"), h("option", { value: "604800" }, "7 dias")),
          ),
        ),
        m.gpu ? h("label", { class: "checar" }, h("input", { id: "nt-gpu", type: "checkbox", onchange: estimarDepois }), h("span", {}, "Rodar na GPU (preço por hora de GPU)")) : null,
        h("label", { class: "campo" }, "O que você quer descobrir (opcional)", h("textarea", { id: "nt-descricao", maxlength: 2000 })),
      ),
    );
    estimarDepois();
  }

  function pedido() {
    const m = escolhido;
    const v = (id) => Number($(`#${id}`)?.value ?? 0);
    return {
      tipo: m.tipo,
      tamanho: CODIFICAR.inteiro(v("nt-tamanho")),
      passos: m.passos ? CODIFICAR.inteiro(v("nt-passos")) : 0,
      parametros: (m.parametros || []).map((p, i) => (CODIFICAR[p.codificacao] || CODIFICAR.inteiro)(v(`nt-p${i}`))),
      unidades: CODIFICAR.inteiro(v("nt-unidades")),
      paralelo: CODIFICAR.inteiro(v("nt-paralelo")),
      copias: Number($("#nt-copias")?.value || 1),
      prazo_s: Number($("#nt-prazo")?.value || 86400),
      gpu: !!$("#nt-gpu")?.checked,
      descricao: $("#nt-descricao")?.value || "",
    };
  }

  function estimarDepois() {
    clearTimeout(espera);
    espera = setTimeout(estimar, 350);
  }

  async function estimar() {
    const r = await postar("/tarefas/estimativa", pedido());
    if (!r.ok) {
      trocar(resumo, h("h2", {}, "Estimativa"), erroForm(r.erro));
      return;
    }
    const e = r.dados;
    trocar(resumo, 
      h("h2", {}, "Estimativa"),
      fatos([
        ["Unidades × cópias", `${num(e.unidades)} × ${num(e.copias)}`],
        ["Memória por unidade", bytes(e.memoria_por_unidade)],
        ["Tempo estimado", duracao(e.tempo_estimado_s)],
        ["Computadores online", `${num(e.capacidade.nos)} (${num(e.capacidade.fornecedores)} fornecedor(es))`],
        ["Saldo", e.saldo],
      ]),
      h("div", { class: "total" }, h("span", {}, "Reserva máxima"), h("strong", {}, e.reserva)),
      h("p", { class: "nota" }, e.como_cobra),
      e.avisos.length ? h("ul", { class: "avisos-lista" }, e.avisos.map((a) => h("li", {}, a))) : null,
      json(e.politica, "política de execução"),
      h("button", { type: "button", class: "botao botao-grande", onclick: criar }, "Criar tarefa"),
    );
  }

  async function criar() {
    const r = await postar("/tarefas", pedido());
    if (!r.ok) return aviso(r.erro, true);
    aviso(`Tarefa #${r.dados.id} criada. Reserva: ${r.dados.reserva}.`);
    ir(`#/tarefa/${r.dados.id}`);
  }

  desenharGrade();
  desenharCampos();
  return h("section", { class: "tela" }, cabecalho("Nova tarefa", "Escolha o motor, o tamanho do problema e quantas unidades. O preço aparece ao lado antes de você criar."), grade, h("div", { class: "montagem" }, campos, resumo));
}

async function telaTarefa(id) {
  const caixa = h("div", { class: "tela" });
  async function desenhar() {
    const [r, res] = await Promise.all([obter(`/tarefas/${id}`), obter(`/tarefas/${id}/resultado`)]);
    if (!r.ok) {
      trocar(caixa, cabecalho("Tarefa não encontrada"), h("a", { href: "#/tarefas" }, "Voltar às tarefas"));
      return;
    }
    const t = r.dados;
    const final = ["COMPLETED", "FAILED", "CANCELLED", "TIMEOUT", "REJECTED"].includes(t.estado);
    const v = t.verificacao;
    trocar(caixa, 
      h("a", { class: "migalha", href: "#/tarefas" }, "← Tarefas"),
      cabecalho(
        `Tarefa #${t.id}`,
        `${t.nome}${t.descricao ? ` — ${t.descricao}` : ""}`,
        pilula(NOMES_TAREFA, t.estado),
        !final
          ? h(
              "button",
              {
                type: "button",
                class: "botao-perigo",
                onclick: async () => {
                  const ok = await perguntar("Cancelar a tarefa?", "O que já foi conferido é cobrado; o resto da reserva volta ao saldo.", [], "Cancelar a tarefa");
                  if (!ok) return;
                  const c = await postar(`/tarefas/${id}/cancelar`);
                  if (!c.ok) aviso(c.erro, true);
                  desenhar();
                },
              },
              "Cancelar",
            )
          : null,
      ),
      numeros(
        numero("Unidades conferidas", `${num(t.progresso.concluidas)} / ${num(t.progresso.unidades)}`, t.progresso.falhas ? `${num(t.progresso.falhas)} falharam` : null),
        numero("Rodando agora", num(t.progresso.em_voo)),
        numero(final ? "Cobrado" : "Custo até agora", t.custo.cobrado || t.custo.ate_agora, `reserva ${t.custo.reserva}${t.custo.devolvido ? ` · devolvido ${t.custo.devolvido}` : ""}`),
        numero("Conferência", `${num(v.freivalds + v.concordancia)} provadas`, `${num(v.freivalds)} Freivalds · ${num(v.concordancia)} concordância · ${num(v.integridade)} só integridade · ${num(v.divergencias)} divergência(s)`),
      ),
      h("div", { class: "bloco" }, barra(t.progresso.concluidas, t.progresso.unidades, t.estado === "COMPLETED"), t.motivo ? h("p", { class: "nota" }, t.motivo) : null),
      t.em_voo_detalhe?.length
        ? h(
            "div",
            { class: "bloco" },
            h("h2", {}, "Unidades rodando"),
            tabela(
              [
                { t: "Unidade", v: (u) => `#${u.unidade}`, n: true },
                { t: "Estado", v: (u) => (u.executando ? pilula(NOMES_TAREFA, "RUNNING") : pilula(NOMES_TAREFA, "DISPATCHED")) },
                { t: "Progresso", v: (u) => (u.total ? h("div", {}, barra(u.feitas, u.total), h("span", { class: "nota" }, `${num((u.feitas / u.total) * 100)}%`)) : "—") },
                { t: "Prazo", v: (u) => quando(u.prazo_ms) },
              ],
              t.em_voo_detalhe,
            ),
          )
        : null,
      h(
        "div",
        { class: "grade-dupla" },
        h(
          "div",
          { class: "bloco" },
          h("h2", {}, "Resultado"),
          res.ok
            ? [
                h("p", {}, res.dados.texto),
                res.dados.unidades.length
                  ? h(
                      "div",
                      { class: "acoes" },
                      h("a", { class: "botao-leve", href: `/api/v1/tarefas/${id}/unidades/${res.dados.unidades[0].unidade}` }, "Baixar a primeira unidade"),
                      h("a", { class: "botao-leve", href: `/api/v1/tarefas/${id}/relatorio`, target: "_blank", rel: "noopener" }, "Relatório completo (JSON)"),
                    )
                  : null,
                json(res.dados.agregado, "números agregados"),
              ]
            : h("p", { class: "nota" }, res.erro),
        ),
        h(
          "div",
          { class: "bloco" },
          h("h2", {}, "Pedido"),
          fatos([
            ["Motor", t.tipo, true],
            ["Criada", quando(t.criada_ms)],
            ["Começou", quando(t.iniciada_ms)],
            ["Terminou", quando(t.concluida_ms)],
            ["Prazo", quando(t.prazo_ms)],
            ["Pela API", t.via_api ? "sim" : "não"],
            ["JOB da tarefa", `${t.job.slice(0, 24)}…`, true],
          ]),
          json(t.pedido, "parâmetros enviados"),
          json(t.politica, "política de execução"),
        ),
      ),
    );
    if (final) estado.atualizar = null;
  }
  estado.atualizar = desenhar;
  await desenhar();
  return caixa;
}

async function telaSaldo() {
  const mes = new Date().toISOString().slice(0, 7);
  const [saldo, pagamentos, fatura, extrato] = await Promise.all([obter("/saldo"), obter("/pagamentos"), obter(`/faturas/${mes}`), obter(`/extrato?mes=${mes}`)]);
  const sd = saldo.dados || {};
  const moeda = estado.eu.moeda;
  const instrucoes = h("div", {});
  const form = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const valor = Math.round(Number($("#sd-valor").value.replace(",", ".")) * 100);
        const r = await postar("/pagamentos", { valor_centavos: valor });
        if (!r.ok) {
          trocar(instrucoes, erroForm(r.erro));
          return;
        }
        const i = r.dados.instrucoes;
        trocar(instrucoes, 
          i.tipo === "pix"
            ? h("div", { class: "codigo-vinculo" }, h("small", {}, "Pague por Pix"), h("strong", {}, i.valor), fatos([["Chave Pix", i.chave, true], ["Titular", i.titular], ["Escreva na mensagem", i.mensagem, true]]), h("small", {}, i.texto))
            : h("p", { class: "ok-form" }, i.texto),
        );
        if (i.tipo !== "pix") setTimeout(() => mostrar(), 1200);
      },
    },
    h("h2", {}, "Adicionar saldo"),
    h("div", { class: "campos duas" }, h("label", { class: "campo" }, `Valor (${moeda})`, h("input", { id: "sd-valor", type: "number", min: 10, step: "0.01", value: "50", required: true }))),
    h("button", { type: "submit", class: "botao" }, "Gerar pagamento"),
    instrucoes,
  );
  return h(
    "section",
    { class: "tela" },
    cabecalho("Saldo e faturas", "O saldo é de uso: paga tarefas na plataforma. Não é investimento nem criptomoeda, e não se transfere entre contas."),
    numeros( numero("Disponível", sd.disponivel?.texto), numero("Reservado em tarefas", sd.reservado?.texto), numero(`Gasto em ${mes}`, fatura.dados?.total_gasto), numero(`Pago em ${mes}`, fatura.dados?.total_pago)),
    h(
      "div",
      { class: "grade-dupla" },
      form,
      h(
        "div",
        { class: "bloco" },
        h("h2", {}, "Pagamentos"),
        (pagamentos.dados?.pagamentos || []).length
          ? tabela(
              [
                { t: "Código", v: (p) => p.codigo, n: true },
                { t: "Valor", v: (p) => p.valor, n: true },
                { t: "Estado", v: (p) => pilula(NOMES_PAGAMENTO, p.estado) },
                { t: "Quando", v: (p) => quando(p.criado_ms) },
                {
                  t: "",
                  v: (p) =>
                    p.estado === "pendente"
                      ? h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: async (ev) => { ev.stopPropagation(); const r = await postar(`/pagamentos/${p.id}/cancelar`); if (!r.ok) aviso(r.erro, true); mostrar(); } }, "Cancelar")
                      : "",
                },
              ],
              pagamentos.dados.pagamentos,
            )
          : vazio("Nenhum pagamento ainda"),
      ),
    ),
    h(
      "div",
      { class: "bloco" },
      h("h2", {}, `Fatura de ${mes}`),
      (fatura.dados?.tarefas || []).length
        ? tabela([{ t: "Tarefa", v: (x) => `#${x.tarefa}`, n: true }, { t: "Motor", v: (x) => x.tipo }, { t: "Estado", v: (x) => pilula(NOMES_TAREFA, x.estado) }, { t: "Unidades", v: (x) => num(x.unidades), n: true }, { t: "Cobrado", v: (x) => x.cobrado, n: true }, { t: "Quando", v: (x) => quando(x.quando) }], fatura.dados.tarefas)
        : vazio("Nenhuma cobrança neste mês"),
      h("p", { class: "nota" }, fatura.dados?.nota || ""),
    ),
    h(
      "div",
      { class: "bloco" },
      h("h2", {}, "Extrato do mês"),
      (extrato.dados?.linhas || []).length
        ? tabela([{ t: "Quando", v: (x) => quando(x.t) }, { t: "Motivo", v: (x) => x.motivo.replaceAll("_", " ") }, { t: "Referência", v: (x) => x.referencia }, { t: "Conta", v: (x) => x.conta.split(":").pop() }, { t: "Valor", v: (x) => x.valor, n: true }], extrato.dados.linhas)
        : vazio("Sem movimento neste mês"),
    ),
  );
}

async function telaChaves() {
  const r = await obter("/chaves");
  const mostrada = h("div", {});
  const form = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const escopos = [...form.querySelectorAll("input[name=escopo]:checked")].map((x) => x.value);
        const c = await postar("/chaves", { nome: $("#ch-nome").value, escopos, limite_por_min: Number($("#ch-limite").value) });
        if (!c.ok) return trocar(mostrada, erroForm(c.erro));
        trocar(mostrada, h("div", { class: "chave-mostrada" }, h("b", {}, "A chave (aparece só agora):"), h("code", {}, c.dados.chave), h("small", {}, c.dados.aviso)));
        setTimeout(async () => trocar(lista, conteudo((await obter("/chaves")).dados?.chaves || [])), 100);
      },
    },
    h("h2", {}, "Nova chave"),
    h("label", { class: "campo" }, "Nome (onde vai ser usada)", h("input", { id: "ch-nome", type: "text", required: true, maxlength: 60, placeholder: "servidor do laboratório" })),
    h("fieldset", { class: "campos" }, h("legend", {}, "O que ela pode"), ...[["tarefas:ler", "Ler tarefas e resultados"], ["tarefas:criar", "Criar e cancelar tarefas"], ["saldo:ler", "Ler saldo e extrato"]].map(([v, t]) => h("label", { class: "checar" }, h("input", { type: "checkbox", name: "escopo", value: v, checked: v === "tarefas:ler" ? true : null }), h("span", {}, t)))),
    h("label", { class: "campo" }, "Pedidos por minuto", h("input", { id: "ch-limite", type: "number", min: 1, max: 600, value: 60 })),
    h("button", { type: "submit", class: "botao" }, "Criar chave"),
    mostrada,
  );
  const conteudo = (chaves) =>
    chaves.length
      ? tabela(
          [
            { t: "Nome", v: (c) => c.nome },
            { t: "Começo", v: (c) => h("code", {}, `${c.prefixo}…`) },
            { t: "Escopos", v: (c) => c.escopos.join(", ") },
            { t: "Limite", v: (c) => `${c.limite_por_min}/min`, n: true },
            { t: "Estado", v: (c) => (c.revogada_ms ? pilula({}, "revogada") : h("span", { class: "pilula ok" }, "ativa")) },
            {
              t: "",
              v: (c) =>
                c.revogada_ms
                  ? ""
                  : h(
                      "div",
                      { class: "acoes" },
                      h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: async () => { const x = await postar(`/chaves/${c.id}/rotacionar`); if (!x.ok) return aviso(x.erro, true); trocar(mostrada, h("div", { class: "chave-mostrada" }, h("b", {}, "Chave nova (a antiga parou de valer):"), h("code", {}, x.dados.chave))); trocar(lista, conteudo((await obter("/chaves")).dados?.chaves || [])); } }, "Rotacionar"),
                      h("button", { type: "button", class: "botao-perigo botao-pequeno", onclick: async () => { await postar(`/chaves/${c.id}/revogar`); trocar(lista, conteudo((await obter("/chaves")).dados?.chaves || [])); } }, "Revogar"),
                    ),
            },
          ],
          chaves,
        )
      : vazio("Nenhuma chave", "Use uma chave para mandar tarefas de um script ou servidor.");
  const lista = h("div", {}, conteudo(r.dados?.chaves || []));
  const origem = location.origin;
  return h(
    "section",
    { class: "tela" },
    cabecalho("Chaves de API", "A HYX_API_KEY deixa um programa seu mandar tarefas e ler resultados sem a sua senha. A chave aparece uma vez; depois, só o começo."),
    h("div", { class: "grade-dupla" }, form, h("div", { class: "bloco" }, h("h2", {}, "Suas chaves"), lista)),
    h(
      "div",
      { class: "bloco" },
      h("h2", {}, "Exemplo"),
      h("pre", { class: "comando" }, `export HYX_API_KEY=hyx_...\n\ncurl -H "Authorization: Bearer $HYX_API_KEY" ${origem}/api/v1/catalogo\n\ncurl -X POST -H "Authorization: Bearer $HYX_API_KEY" -H "Content-Type: application/json" \\\n  -d '{"tipo":"routing","tamanho":200,"passos":1000,"parametros":[11],"unidades":12}' \\\n  ${origem}/api/v1/tarefas\n\ncurl -H "Authorization: Bearer $HYX_API_KEY" ${origem}/api/v1/tarefas/1/resultado`),
    ),
  );
}

// ---------------------------------------------------------------------------
// fornecedor
// ---------------------------------------------------------------------------

function cartaoNo(n) {
  return h(
    "a",
    { class: "cartao", href: `#/computador/${n.id}` },
    h("div", { class: "cartao-cab" }, h("div", {}, h("b", {}, n.nome), h("small", {}, n.hardware.cpu_modelo || "CPU não informada")), pilula(NOMES_NO, n.estado)),
    h("div", { class: "specs" }, h("span", {}, h("b", {}, num(n.limites.linhas)), h("small", {}, "núcleos para a plataforma")), h("span", {}, h("b", {}, mib(n.limites.ram_mib)), h("small", {}, "memória")), h("span", {}, h("b", {}, `${n.reputacao.pontuacao}`), h("small", {}, `reputação (${n.reputacao.faixa})`))),
    fatos([["Disponibilidade", `${num(n.disponibilidade_pct, 1)}%`], ["Unidades aceitas", num(n.reputacao.aceitas)], ["Ganhos", n.ganhos.map((g) => g.texto).join(" · ") || "—"], ["Último pulso", quando(n.ultimo_pulso_ms)]]),
    n.motivo ? h("p", { class: "nota" }, n.motivo) : null,
  );
}

async function telaComputadores() {
  if (!papel("PROVIDER")) return telaAtivarPapel("fornecedor");
  const r = await obter("/nos");
  const lista = r.dados?.nos || [];
  const grade = h("div", { class: "cartoes" }, lista.map(cartaoNo));
  const vinculo = h("div", {});
  estado.atualizar = async () => {
    const n = await obter("/nos");
    if (n.ok) trocar(grade, ...n.dados.nos.map(cartaoNo));
  };
  return h(
    "section",
    { class: "tela" },
    cabecalho(
      "Computadores",
      "Os computadores que você empresta. Você escolhe quantos núcleos e quanta memória cada um dá, e pausa quando quiser.",
      h(
        "button",
        {
          type: "button",
          class: "botao",
          onclick: async () => {
            const c = await postar("/nos/vinculo");
            if (!c.ok) return aviso(c.erro, true);
            trocar(vinculo, 
              h(
                "div",
                { class: "codigo-vinculo" },
                h("small", {}, "Código de vínculo (vale 15 minutos, uma vez)"),
                h("strong", {}, c.dados.codigo),
                h("p", {}, "No programa Hyurax do computador: abra Fornecer, cole o endereço da plataforma e este código, e escolha os limites."),
                h("pre", { class: "comando" }, `hyurax-no agente --plataforma ${c.dados.plataforma} --codigo ${c.dados.codigo}`),
                h("small", {}, "A linha acima é para servidor sem tela (Linux ou Windows, pelo terminal)."),
              ),
            );
          },
        },
        "Adicionar computador",
      ),
    ),
    vinculo,
    lista.length ? grade : vazio("Nenhum computador ainda", "Gere um código de vínculo e cole no programa Hyurax do computador que vai trabalhar."),
    h(
      "div",
      { class: "bloco" },
      h("h2", {}, "O que roda no seu computador"),
      h("p", {}, "Só os motores científicos que vêm dentro do programa Hyurax. O cliente manda parâmetros, nunca código. O motor não abre arquivo, não usa a rede da sua casa e não vê nada do que é seu. A memória que você libera é reservada antes de cada unidade, e o programa para tudo quando você pausa."),
    ),
  );
}

async function telaComputador(id) {
  const caixa = h("div", { class: "tela" });
  async function desenhar() {
    const r = await obter(`/nos/${id}`);
    if (!r.ok) {
      trocar(caixa, cabecalho("Computador não encontrado"), h("a", { href: "#/computadores" }, "Voltar"));
      return;
    }
    const n = r.dados;
    const tel = n.telemetria || [];
    const pausado = n.estado === "PAUSED";
    trocar(caixa, 
      h("a", { class: "migalha", href: "#/computadores" }, "← Computadores"),
      cabecalho(
        n.nome,
        `${n.hardware.cpu_modelo || "CPU"} · ${num(n.hardware.nucleos)} núcleos · ${mib(n.hardware.ram_mib)}${n.hardware.gpu_modelo ? ` · ${n.hardware.gpu_modelo}` : ""} · ${n.hardware.so || ""}`,
        pilula(NOMES_NO, n.estado),
        h("button", { type: "button", class: "botao-leve", onclick: async () => { const x = await postar(`/nos/${id}/${pausado ? "retomar" : "pausar"}`); if (!x.ok) aviso(x.erro, true); desenhar(); } }, pausado ? "Retomar" : "Pausar"),
      ),
      numeros(
        numero("Reputação", `${n.reputacao.pontuacao}`, `${n.reputacao.faixa} · ${num(n.reputacao.verificadas)} conferidas · ${num(n.reputacao.incorretas)} erradas`),
        numero("Disponibilidade", `${num(n.disponibilidade_pct, 1)}%`, `desde ${dia(n.criado_ms)}`),
        numero("Unidades agora", num(n.abertas)),
        numero("Ganhos", n.ganhos.map((g) => g.texto).join(" · ") || "—"),
      ),
      h(
        "div",
        { class: "grade-dupla" },
        h("div", { class: "bloco" }, h("h2", {}, "Telemetria"), grafico("CPU da máquina", tel.map((x) => x.cpu), "%", 100), grafico("Memória usada", tel.map((x) => (x.ram_usada_mib == null ? null : x.ram_usada_mib / 1024)), "GB"), grafico("Temperatura", tel.map((x) => x.temperatura), "°C"), grafico("Potência estimada", tel.map((x) => x.potencia_w), "W")),
        formLimites(n, desenhar),
      ),
      h(
        "div",
        { class: "bloco" },
        h("h2", {}, "Últimas unidades"),
        (n.unidades || []).length
          ? tabela([{ t: "Tarefa", v: (u) => `#${u.tarefa}`, n: true }, { t: "Unidade", v: (u) => `#${u.unidade}`, n: true }, { t: "Motor", v: (u) => u.tipo }, { t: "Recebida", v: (u) => quando(u.emitida_ms) }, { t: "Desfecho", v: (u) => u.desfecho || "em andamento" }, { t: "Cálculo", v: (u) => (u.ms_calculo ? `${num(u.ms_calculo)} ms` : "—"), n: true }], n.unidades)
          : vazio("Nenhuma unidade ainda", "Quando houver tarefa que caiba nos limites deste computador, ela aparece aqui."),
      ),
      h(
        "div",
        { class: "bloco" },
        h("h2", {}, "Tirar da plataforma"),
        h("p", { class: "nota" }, "O que estiver rodando é cancelado. Para voltar, gere outro código de vínculo."),
        h("div", { class: "acoes" }, h("button", { type: "button", class: "botao-perigo", onclick: async () => { const ok = await perguntar("Remover este computador?", "O que estiver rodando é cancelado.", [], "Remover"); if (!ok) return; const x = await postar(`/nos/${id}/remover`); if (!x.ok) return aviso(x.erro, true); ir("#/computadores"); } }, "Remover computador")),
      ),
    );
  }
  await desenhar();
  estado.atualizar = desenhar;
  return caixa;
}

function formLimites(n, depois) {
  const form = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const r = await postar(`/nos/${n.id}/ajustes`, {
          nome: $("#lm-nome").value,
          regiao: $("#lm-regiao").value,
          limites: { linhas: Number($("#lm-linhas").value), ram_mib: Math.round(Number($("#lm-ram").value) * 1024), gpu: $("#lm-gpu").checked },
        });
        if (!r.ok) return aviso(r.erro, true);
        aviso("Limites salvos.");
        depois();
      },
    },
    h("h2", {}, "Limites"),
    h(
      "div",
      { class: "campos duas" },
      h("label", { class: "campo" }, "Nome", h("input", { id: "lm-nome", type: "text", value: n.nome, maxlength: 60 })),
      h("label", { class: "campo" }, "Região", h("input", { id: "lm-regiao", type: "text", value: n.regiao, maxlength: 24, placeholder: "BR-SP" })),
      h("label", { class: "campo" }, "Núcleos para a plataforma", h("input", { id: "lm-linhas", type: "number", min: 1, max: n.hardware.nucleos || 64, value: n.limites.linhas })),
      h("label", { class: "campo" }, "Memória (GB)", h("input", { id: "lm-ram", type: "number", min: 0.25, step: 0.25, value: (n.limites.ram_mib / 1024).toFixed(2) })),
    ),
    h("label", { class: "checar" }, h("input", { id: "lm-gpu", type: "checkbox", checked: n.limites.gpu ? true : null, disabled: n.hardware.gpu_modelo ? null : true }), h("span", {}, n.hardware.gpu_modelo ? `Aceitar trabalho na GPU (${n.hardware.gpu_modelo})` : "Sem GPU informada")),
    h("button", { type: "submit", class: "botao" }, "Salvar limites"),
  );
  return form;
}

async function telaGanhos() {
  if (!papel("PROVIDER")) return telaAtivarPapel("fornecedor");
  const r = await obter("/ganhos");
  const g = r.dados || {};
  const sd = g.saldo || {};
  const form = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const valor = Math.round(Number($("#gq-valor").value.replace(",", ".")) * 100);
        const x = await postar("/saques", { valor_centavos: valor, destino: $("#gq-pix").value });
        if (!x.ok) return aviso(x.erro, true);
        aviso(`Saque de ${x.dados.valor} pedido.`);
        mostrar();
      },
    },
    h("h2", {}, "Pedir saque"),
    h("p", { class: "nota" }, `Mínimo ${g.saque_minimo || "—"}. A empresa paga por Pix depois de conferir, em dias úteis.`),
    h("div", { class: "campos duas" }, h("label", { class: "campo" }, "Valor", h("input", { id: "gq-valor", type: "number", min: 1, step: "0.01", required: true })), h("label", { class: "campo" }, "Chave Pix", h("input", { id: "gq-pix", type: "text", required: true, maxlength: 80, placeholder: "CPF, e-mail, celular ou chave aleatória" }))),
    h("button", { type: "submit", class: "botao", disabled: (sd.ganhos_disponiveis?.centavos || 0) <= 0 ? true : null }, "Pedir saque"),
  );
  return h(
    "section",
    { class: "tela" },
    cabecalho("Ganhos e saques", `A sua parte é ${num(g.parte_do_fornecedor_pct)}% do que o cliente paga por unidade conferida. O ganho fica ${num(g.retencao_dias)} dias em retenção (para cobrir estorno e fraude) e depois libera para saque.`),
    numeros( numero("Em retenção", sd.ganhos_pendentes?.texto), numero("Disponível para saque", sd.ganhos_disponiveis?.texto), numero("Saque em andamento", sd.saques_em_andamento?.texto)),
    h(
      "div",
      { class: "grade-dupla" },
      form,
      h(
        "div",
        { class: "bloco" },
        h("h2", {}, "Saques"),
        (g.saques || []).length ? tabela([{ t: "Valor", v: (x) => x.valor, n: true }, { t: "Estado", v: (x) => pilula(NOMES_SAQUE, x.estado) }, { t: "Destino", v: (x) => x.destino }, { t: "Pedido", v: (x) => quando(x.criado_ms) }, { t: "Nota", v: (x) => x.nota || "" }], g.saques) : vazio("Nenhum saque ainda"),
      ),
    ),
    h(
      "div",
      { class: "bloco" },
      h("h2", {}, "Ganhos por tarefa"),
      (g.retidos || []).length
        ? tabela([{ t: "Tarefa", v: (x) => `#${x.tarefa}`, n: true }, { t: "Valor", v: (x) => x.valor, n: true }, { t: "Libera em", v: (x) => dia(x.libera_ms) }, { t: "Estado", v: (x) => (x.segurado ? h("span", { class: "pilula atencao" }, `segurado: ${x.segurado}`) : x.liberado ? h("span", { class: "pilula ok" }, "liberado") : h("span", { class: "pilula info" }, "em retenção")) }], g.retidos)
        : vazio("Nenhum ganho ainda", "Os ganhos aparecem quando uma tarefa com unidade feita pelo seu computador termina."),
    ),
  );
}

function telaAtivarPapel(qual) {
  return h(
    "section",
    { class: "tela" },
    cabecalho(qual === "fornecedor" ? "Fornecer computação" : "Comprar computação", qual === "fornecedor" ? "Ative o papel de fornecedor para emprestar um computador e receber pelo trabalho conferido." : "Ative o papel de cliente para mandar tarefas."),
    h("div", { class: "acoes" }, h("button", { type: "button", class: "botao", onclick: async () => { const r = await postar("/conta/papel", { papel: qual }); if (!r.ok) return aviso(r.erro, true); await carregarEu(); mostrar(); } }, `Ativar o papel de ${qual}`)),
  );
}

// ---------------------------------------------------------------------------
// conta
// ---------------------------------------------------------------------------

async function telaConta() {
  const [eu, sessoes] = await Promise.all([obter("/auth/eu"), obter("/conta/sessoes")]);
  const e = eu.dados || {};
  const formSenha = h(
    "form",
    {
      class: "bloco",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const r = await postar("/conta/senha", { atual: $("#ct-atual").value, nova: $("#ct-nova").value });
        if (!r.ok) return aviso(r.erro, true);
        aviso(r.dados.texto);
        formSenha.reset();
      },
    },
    h("h2", {}, "Trocar a senha"),
    h("label", { class: "campo" }, "Senha atual", h("input", { id: "ct-atual", type: "password", required: true, autocomplete: "current-password" })),
    h("label", { class: "campo" }, "Senha nova", h("input", { id: "ct-nova", type: "password", required: true, minlength: 10, autocomplete: "new-password" })),
    h("button", { type: "submit", class: "botao" }, "Trocar"),
  );
  return h(
    "section",
    { class: "tela" },
    cabecalho("Minha conta"),
    h(
      "div",
      { class: "grade-dupla" },
      h(
        "div",
        { class: "bloco" },
        h("h2", {}, "Dados"),
        fatos([["Nome", e.nome], ["E-mail", e.email], ["Papéis", (e.papeis || []).join(", ")], ["Moeda", e.moeda], ["País", e.pais], ["E-mail conferido", e.verificado ? "sim" : "ainda não"], ["Conta criada", dia(e.criado_ms)], ["Códigos de recuperação restantes", num(e.codigos_restantes)]]),
        h(
          "div",
          { class: "acoes" },
          !papel("CUSTOMER") ? h("button", { type: "button", class: "botao-leve", onclick: async () => { await postar("/conta/papel", { papel: "cliente" }); await carregarEu(); mostrar(); } }, "Ativar papel de cliente") : null,
          !papel("PROVIDER") ? h("button", { type: "button", class: "botao-leve", onclick: async () => { await postar("/conta/papel", { papel: "fornecedor" }); await carregarEu(); mostrar(); } }, "Ativar papel de fornecedor") : null,
          h(
            "button",
            {
              type: "button",
              class: "botao-leve",
              onclick: async () => {
                const v = await perguntar("Códigos novos", "Os códigos antigos param de valer. Confirme com a senha.", [{ id: "senha", rotulo: "Senha", tipo: "password", obrigatorio: true }], "Gerar");
                if (!v) return;
                const r = await postar("/conta/recuperacao", { senha: v.senha });
                if (!r.ok) return aviso(r.erro, true);
                trocar($("#raiz"), casca(telaCodigos(r.dados.codigos_de_recuperacao), "#/conta"));
              },
            },
            "Gerar códigos de recuperação novos",
          ),
        ),
      ),
      formSenha,
    ),
    h(
      "div",
      { class: "bloco" },
      h("div", { class: "bloco-cab" }, h("h2", {}, "Sessões abertas"), h("button", { type: "button", class: "botao-leve", onclick: async () => { const r = await postar("/conta/sessoes/encerrar"); aviso(`${r.dados?.encerradas ?? 0} sessão(ões) encerrada(s).`); mostrar(); } }, "Encerrar as outras")),
      tabela([{ t: "Navegador", v: (x) => x.navegador || "—" }, { t: "IP", v: (x) => x.ip }, { t: "Aberta", v: (x) => quando(x.criada_ms) }, { t: "Vale até", v: (x) => quando(x.expira_ms) }, { t: "", v: (x) => (x.esta ? h("span", { class: "pilula ok" }, "esta") : "") }], sessoes.dados?.sessoes || []),
    ),
  );
}

// ---------------------------------------------------------------------------
// administração
// ---------------------------------------------------------------------------

async function telaAdmin() {
  const r = await obter("/admin/painel");
  if (!r.ok) return h("section", { class: "tela" }, cabecalho("Administração"), erroForm(r.erro));
  const p = r.dados;
  const pr = p.produto;
  const q = p.qualidade;
  const si = p.sistema;
  return h(
    "section",
    { class: "tela" },
    cabecalho(
      "Painel da plataforma",
      "Produto, negócio e qualidade: os números que dizem se a plataforma funciona e se paga.",
      h(
        "button",
        {
          type: "button",
          class: si.parada_de_emergencia ? "botao" : "botao-perigo",
          onclick: async () => {
            const ligar = !si.parada_de_emergencia;
            const ok = await perguntar(ligar ? "Parada de emergência" : "Liberar a plataforma", ligar ? "Nada é despachado e tudo que roda é cancelado agora, em todos os nós." : "O despacho volta ao normal.", [], ligar ? "Parar tudo" : "Liberar");
            if (!ok) return;
            const x = await postar("/admin/emergencia", { ligado: ligar });
            if (!x.ok) return aviso(x.erro, true);
            mostrar();
          },
        },
        si.parada_de_emergencia ? "Liberar a plataforma" : "Parada de emergência",
      ),
    ),
    si.parada_de_emergencia ? h("p", { class: "erro-form" }, "PARADA DE EMERGÊNCIA LIGADA: nenhum trabalho está sendo despachado.") : null,
    h("h2", {}, "Produto"),
    numeros(
      numero("Usuários", num(pr.usuarios), `${num(pr.clientes)} clientes · ${num(pr.fornecedores)} fornecedores`),
      numero("Nós online", `${num(pr.nos_online)} / ${num(pr.nos)}`, `${num(pr.nos_em_quarentena)} em quarentena`),
      numero("Horas de computação", num(pr.horas_de_computacao, 2), "em núcleo de referência"),
      numero("Taxa de sucesso", `${num(pr.taxa_de_sucesso_pct, 1)}%`, Object.entries(pr.tarefas || {}).map(([k, v]) => `${NOMES_TAREFA[k]?.[0] || k}: ${v}`).join(" · ")),
    ),
    h("h2", {}, "Negócio"),
    ...p.negocio.map((m) =>
      numeros(
        numero(`GMV (${m.moeda})`, m.gmv, `${num(m.clientes_pagantes)} cliente(s) pagante(s)`),
        numero("Receita da plataforma", m.receita, `take rate ${num(m.take_rate_pct, 1)}%`),
        numero("Margem bruta", `${num(m.margem_bruta_pct, 1)}%`, `conferências: ${m.custo_conferencias} · taxas: ${m.taxas_de_pagamento}`),
        numero("Ganhos dos fornecedores", m.ganhos_fornecedores, `gasto médio por cliente ${m.gasto_medio_por_cliente}`),
        numero("Dinheiro em caixa", m.caixa, m.livro_fecha ? "o livro fecha em zero" : "O LIVRO NÃO FECHA"),
        numero("Saldo dos clientes", m.saldo_dos_clientes, "pago e ainda não gasto"),
      ),
    ),
    h("h2", {}, "Qualidade e sistema"),
    numeros(
      numero("No ar há", duracao(q.uptime_s), `${num(q.pedidos)} pedidos · ${num(q.erros_5xx)} erros 5xx`),
      numero("Latência", `${num(q.latencia_p50_ms)} ms`, `p95 ${num(q.latencia_p95_ms)} ms`),
      numero("Falha de execução", `${num(q.taxa_de_falha_pct, 1)}%`, "prazo vencido ou abandono"),
      numero("Falha de verificação", `${num(q.falha_de_verificacao_pct, 1)}%`, "entregas recusadas ou divergentes"),
      numero("Diário", num(si.eventos_no_diario), `última cópia ${quando(si.ultima_copia_ms)}${si.erro_da_copia ? ` · ERRO: ${si.erro_da_copia}` : ""}`),
      numero("Alertas abertos", num(si.alertas_abertos), `${num(si.tarefas_ativas)} tarefas ativas · ${num(si.atribuicoes_abertas)} unidades em voo`),
    ),
    h("div", { class: "bloco" }, h("h2", {}, "Selo do diário"), h("p", { class: "nota" }, "Guarde este valor fora do servidor de vez em quando. Se o diário for reescrito, o selo muda."), h("code", { class: "hash" }, si.selo_do_diario)),
  );
}

async function acaoAdmin(caminho, corpo, ok) {
  const r = await postar(caminho, corpo);
  if (!r.ok) {
    aviso(r.erro, true);
    return false;
  }
  if (ok) aviso(ok);
  mostrar();
  return true;
}

async function telaAdminUsuarios() {
  const busca = sessionStorage.getItem("hyx-busca") || "";
  const r = await obter(`/admin/usuarios?busca=${encodeURIComponent(busca)}`);
  const lista = r.dados?.usuarios || [];
  return h(
    "section",
    { class: "tela" },
    cabecalho("Usuários"),
    h(
      "form",
      { class: "filtros", onsubmit: (ev) => { ev.preventDefault(); sessionStorage.setItem("hyx-busca", $("#au-busca").value); mostrar(); } },
      h("label", { class: "campo" }, "Buscar por e-mail ou nome", h("input", { id: "au-busca", type: "text", value: busca })),
      h("button", { type: "submit", class: "botao-leve" }, "Buscar"),
    ),
    h(
      "div",
      { class: "bloco" },
      tabela(
        [
          { t: "Nº", v: (u) => u.id, n: true },
          { t: "Pessoa", v: (u) => h("div", {}, u.nome, h("span", { class: "nota" }, u.email)) },
          { t: "Papéis", v: (u) => u.papeis.join(", ") },
          { t: "Saldo", v: (u) => u.saldo, n: true },
          { t: "Ganhos", v: (u) => u.ganhos, n: true },
          { t: "Nós / tarefas", v: (u) => `${u.nos} / ${u.tarefas}`, n: true },
          { t: "Estado", v: (u) => (u.bloqueado ? h("span", { class: "pilula falha" }, "bloqueado") : h("span", { class: "pilula ok" }, u.verificado ? "ativo, conferido" : "ativo")) },
          {
            t: "",
            v: (u) =>
              h(
                "div",
                { class: "acoes" },
                u.bloqueado
                  ? h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: () => acaoAdmin(`/admin/usuarios/${u.id}/desbloquear`, {}, "Desbloqueado.") }, "Desbloquear")
                  : h("button", { type: "button", class: "botao-perigo botao-pequeno", onclick: async () => { const v = await perguntar(`Bloquear ${u.email}?`, "As tarefas dele param e os computadores entram em manutenção.", [{ id: "motivo", rotulo: "Motivo", obrigatorio: true }], "Bloquear"); if (v) acaoAdmin(`/admin/usuarios/${u.id}/bloquear`, v, "Bloqueado."); } }, "Bloquear"),
                !u.verificado ? h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: () => acaoAdmin(`/admin/usuarios/${u.id}/verificar`, {}, "Marcado como conferido.") }, "Conferido") : null,
                h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: async () => { const x = await postar(`/admin/usuarios/${u.id}/codigos`); if (!x.ok) return aviso(x.erro, true); await perguntar("Códigos de recuperação", `${x.dados.aviso}: ${x.dados.codigos.join("  ")}`, [], "Anotei"); } }, "Códigos"),
                h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: async () => { const v = await perguntar(`Ajuste de saldo de ${u.email}`, "Crédito (positivo) ou débito (negativo), saindo da receita da plataforma.", [{ id: "valor", rotulo: "Valor em centavos", tipo: "number", obrigatorio: true }, { id: "motivo", rotulo: "Motivo", obrigatorio: true }], "Lançar"); if (v) acaoAdmin("/admin/ajuste", { usuario: u.id, valor_centavos: Number(v.valor), motivo: v.motivo }, "Ajuste lançado."); } }, "Ajuste"),
              ),
          },
        ],
        lista,
      ),
    ),
  );
}

async function telaAdminNos() {
  const r = await obter("/admin/nos");
  const estadoNo = async (n, estado, rotulo) => {
    const v = await perguntar(`${rotulo}: ${n.nome}`, "O que estiver rodando neste nó é cancelado.", [{ id: "motivo", rotulo: "Motivo", obrigatorio: estado !== "NORMAL" }], rotulo);
    if (v) acaoAdmin(`/admin/nos/${n.id}/estado`, { estado, motivo: v.motivo }, "Feito.");
  };
  return h(
    "section",
    { class: "tela" },
    cabecalho("Nós", "Todos os computadores registrados. Quarentena e manutenção param o trabalho; banir é definitivo."),
    h(
      "div",
      { class: "bloco" },
      tabela(
        [
          { t: "Nº", v: (n) => n.id, n: true },
          { t: "Nome", v: (n) => h("div", {}, n.nome, h("span", { class: "nota" }, `dono ${n.dono} · ${n.hardware.cpu_modelo}`)) },
          { t: "Estado", v: (n) => pilula(NOMES_NO, n.estado) },
          { t: "Reputação", v: (n) => `${n.reputacao.pontuacao} (${n.reputacao.faixa})`, n: true },
          { t: "Aceitas / erradas", v: (n) => `${n.reputacao.aceitas} / ${n.reputacao.incorretas}`, n: true },
          { t: "Versão", v: (n) => n.versao },
          {
            t: "",
            v: (n) =>
              h(
                "div",
                { class: "acoes" },
                n.estado === "QUARANTINED" || n.estado === "MAINTENANCE" || n.estado === "BANNED"
                  ? h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: () => estadoNo(n, "NORMAL", "Liberar") }, "Liberar")
                  : [h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: () => estadoNo(n, "QUARANTINED", "Quarentena") }, "Quarentena"), h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: () => estadoNo(n, "MAINTENANCE", "Manutenção") }, "Manutenção")],
                n.estado !== "BANNED" ? h("button", { type: "button", class: "botao-perigo botao-pequeno", onclick: () => estadoNo(n, "BANNED", "Banir") }, "Banir") : null,
              ),
          },
        ],
        r.dados?.nos || [],
      ),
    ),
  );
}

async function telaAdminTarefas() {
  const r = await obter("/admin/tarefas");
  return h(
    "section",
    { class: "tela" },
    cabecalho("Todas as tarefas"),
    h(
      "div",
      { class: "bloco" },
      tabela(
        [
          { t: "Nº", v: (t) => `#${t.id}`, n: true },
          { t: "Cliente", v: (t) => t.cliente_email || t.cliente },
          { t: "Motor", v: (t) => t.tipo },
          { t: "Estado", v: (t) => pilula(NOMES_TAREFA, t.estado) },
          { t: "Unidades", v: (t) => `${t.progresso.concluidas}/${t.progresso.unidades}`, n: true },
          { t: "Custo", v: (t) => t.custo.cobrado || t.custo.ate_agora, n: true },
          { t: "Criada", v: (t) => quando(t.criada_ms) },
          {
            t: "",
            v: (t) =>
              ["COMPLETED", "FAILED", "CANCELLED", "TIMEOUT", "REJECTED"].includes(t.estado)
                ? ""
                : h("button", { type: "button", class: "botao-perigo botao-pequeno", onclick: async (ev) => { ev.stopPropagation(); const v = await perguntar(`Parar a tarefa #${t.id}?`, "KILL TASK: o que roda é cancelado; o conferido é cobrado.", [{ id: "motivo", rotulo: "Motivo", obrigatorio: true }], "Parar"); if (v) acaoAdmin(`/admin/tarefas/${t.id}/parar`, v, "Tarefa parada."); } }, "Parar"),
          },
        ],
        r.dados?.tarefas || [],
        (t) => ir(`#/tarefa/${t.id}`),
      ),
    ),
  );
}

async function telaAdminPagamentos() {
  const r = await obter("/admin/pagamentos");
  return h(
    "section",
    { class: "tela" },
    cabecalho("Pagamentos", "Pix manual: confira no extrato do banco o valor e o código na mensagem antes de confirmar."),
    h(
      "div",
      { class: "bloco" },
      tabela(
        [
          { t: "Código", v: (p) => p.codigo, n: true },
          { t: "Cliente", v: (p) => p.email || p.usuario },
          { t: "Valor", v: (p) => p.valor, n: true },
          { t: "Estado", v: (p) => pilula(NOMES_PAGAMENTO, p.estado) },
          { t: "Forma", v: (p) => p.provedor },
          { t: "Referência", v: (p) => p.referencia || p.nota || "" },
          { t: "Criado", v: (p) => quando(p.criado_ms) },
          {
            t: "",
            v: (p) =>
              h(
                "div",
                { class: "acoes" },
                p.estado === "pendente"
                  ? [
                      h("button", { type: "button", class: "botao botao-pequeno", onclick: async () => { const v = await perguntar(`Confirmar ${p.codigo} (${p.valor})?`, "O saldo entra na conta do cliente na hora.", [{ id: "referencia", rotulo: "Referência do Pix recebido", obrigatorio: true }, { id: "taxa", rotulo: "Taxa do banco, em centavos", tipo: "number", valor: "0" }], "Confirmar"); if (v) acaoAdmin(`/admin/pagamentos/${p.id}/confirmar`, { referencia: v.referencia, taxa_centavos: Number(v.taxa || 0) }, "Pagamento confirmado."); } }, "Confirmar"),
                      h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: async () => { const v = await perguntar(`Cancelar ${p.codigo}?`, "", [{ id: "motivo", rotulo: "Motivo", obrigatorio: true }], "Cancelar pagamento"); if (v) acaoAdmin(`/admin/pagamentos/${p.id}/cancelar`, v, "Cancelado."); } }, "Cancelar"),
                    ]
                  : null,
                p.estado === "confirmado" ? h("button", { type: "button", class: "botao-perigo botao-pequeno", onclick: async () => { const v = await perguntar(`Estornar ${p.codigo}?`, "Tira o valor do saldo do cliente. Se ficar negativo, a conta é bloqueada e os ganhos retidos das tarefas dele ficam segurados.", [{ id: "motivo", rotulo: "Motivo", obrigatorio: true }], "Estornar"); if (v) acaoAdmin(`/admin/pagamentos/${p.id}/estornar`, v, "Estorno lançado."); } }, "Estornar") : null,
              ),
          },
        ],
        r.dados?.pagamentos || [],
      ),
    ),
  );
}

async function telaAdminSaques() {
  const r = await obter("/admin/saques");
  return h(
    "section",
    { class: "tela" },
    cabecalho("Saques", "Pague por Pix pelo banco da empresa e marque como pago com a referência."),
    h(
      "div",
      { class: "bloco" },
      tabela(
        [
          { t: "Nº", v: (x) => x.id, n: true },
          { t: "Fornecedor", v: (x) => x.email || x.usuario },
          { t: "Valor", v: (x) => x.valor, n: true },
          { t: "Chave Pix", v: (x) => h("code", {}, x.destino) },
          { t: "Estado", v: (x) => pilula(NOMES_SAQUE, x.estado) },
          { t: "Pedido", v: (x) => quando(x.criado_ms) },
          {
            t: "",
            v: (x) =>
              x.estado === "solicitado" || x.estado === "aprovado"
                ? h(
                    "div",
                    { class: "acoes" },
                    x.estado === "solicitado" ? h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: () => acaoAdmin(`/admin/saques/${x.id}/aprovar`, {}, "Aprovado.") }, "Aprovar") : null,
                    h("button", { type: "button", class: "botao botao-pequeno", onclick: async () => { const v = await perguntar(`Marcar o saque ${x.id} como pago`, "", [{ id: "nota", rotulo: "Referência do Pix enviado", obrigatorio: true }], "Marcar como pago"); if (v) acaoAdmin(`/admin/saques/${x.id}/pagar`, v, "Marcado como pago."); } }, "Pago"),
                    h("button", { type: "button", class: "botao-perigo botao-pequeno", onclick: async () => { const v = await perguntar(`Recusar o saque ${x.id}?`, "O valor volta ao disponível do fornecedor.", [{ id: "nota", rotulo: "Motivo", obrigatorio: true }], "Recusar"); if (v) acaoAdmin(`/admin/saques/${x.id}/recusar`, v, "Recusado."); } }, "Recusar"),
                  )
                : x.nota || "",
          },
        ],
        r.dados?.saques || [],
      ),
    ),
  );
}

async function telaAdminAlertas() {
  const r = await obter("/admin/alertas");
  const classe = { baixa: "", media: "atencao", alta: "falha", critica: "falha" };
  return h(
    "section",
    { class: "tela" },
    cabecalho("Alertas", "Segurança e fraude: o que os sinais automáticos acharam."),
    h(
      "div",
      { class: "bloco" },
      (r.dados?.alertas || []).length
        ? tabela(
            [
              { t: "Quando", v: (a) => quando(a.criado_ms) },
              { t: "Tipo", v: (a) => h("code", {}, a.tipo) },
              { t: "Severidade", v: (a) => h("span", { class: `pilula ${classe[a.severidade] || ""}` }, a.severidade) },
              { t: "Alvo", v: (a) => a.alvo },
              { t: "Detalhe", v: (a) => a.detalhe },
              { t: "", v: (a) => (a.resolvido ? h("span", { class: "pilula ok" }, "resolvido") : h("button", { type: "button", class: "botao-leve botao-pequeno", onclick: async () => { const v = await perguntar("Resolver o alerta", a.detalhe, [{ id: "nota", rotulo: "O que foi feito", obrigatorio: true }], "Resolver"); if (v) acaoAdmin(`/admin/alertas/${a.id}/resolver`, v, "Resolvido."); } }, "Resolver")) },
            ],
            r.dados.alertas,
          )
        : vazio("Nenhum alerta"),
    ),
  );
}

async function telaAdminRegistro() {
  const tipo = sessionStorage.getItem("hyx-reg-tipo") || "";
  const busca = sessionStorage.getItem("hyx-reg-busca") || "";
  const r = await obter(`/admin/registro?tipo=${encodeURIComponent(tipo)}&busca=${encodeURIComponent(busca)}`);
  return h(
    "section",
    { class: "tela" },
    cabecalho("Registro de auditoria", `O diário inteiro tem ${num(r.dados?.total)} eventos, cada um preso ao anterior por hash. Senhas, tokens e chaves nunca aparecem: só os hashes.`, h("a", { class: "botao-leve", href: "/api/v1/admin/copia" }, "Baixar cópia do diário")),
    h(
      "form",
      { class: "filtros", onsubmit: (ev) => { ev.preventDefault(); sessionStorage.setItem("hyx-reg-tipo", $("#rg-tipo").value); sessionStorage.setItem("hyx-reg-busca", $("#rg-busca").value); mostrar(); } },
      h("label", { class: "campo" }, "Tipo", h("input", { id: "rg-tipo", type: "text", value: tipo, placeholder: "LOGIN, TASK_CREATED, ADMIN_ACTION…" })),
      h("label", { class: "campo" }, "Texto", h("input", { id: "rg-busca", type: "text", value: busca })),
      h("button", { type: "submit", class: "botao-leve" }, "Filtrar"),
    ),
    h("div", { class: "bloco" }, tabela([{ t: "Nº", v: (x) => x.n, n: true }, { t: "Quando", v: (x) => quando(x.t) }, { t: "Evento", v: (x) => h("code", {}, x.tipo) }, { t: "Dados", v: (x) => json(x.evento, "abrir") }], r.dados?.eventos || [])),
  );
}

async function telaAdminLivro() {
  const r = await obter("/admin/livro");
  const d = r.dados || {};
  return h(
    "section",
    { class: "tela" },
    cabecalho("Livro", d.fecha ? "Partida dobrada: a soma de todas as contas é zero, em cada moeda." : "ATENÇÃO: o livro não fecha em zero."),
    h("div", { class: "grade-dupla" }, h("div", { class: "bloco" }, h("h2", {}, "Contas com saldo"), tabela([{ t: "Conta", v: (c) => h("code", {}, c.conta) }, { t: "Moeda", v: (c) => c.moeda }, { t: "Saldo", v: (c) => c.texto, n: true }], d.contas || [])), h("div", { class: "bloco" }, h("h2", {}, "Como ler"), h("p", { class: "nota" }, "O saldo de cada conta é o que a plataforma deve àquela parte. O caixa é a contrapartida do dinheiro de verdade: quando entra um pagamento ele desce, quando sai um saque ou uma taxa ele sobe. O dinheiro que a empresa tem é o caixa com o sinal trocado."))),
    h("div", { class: "bloco" }, h("h2", {}, "Últimos lançamentos"), tabela([{ t: "Nº", v: (l) => l.id, n: true }, { t: "Quando", v: (l) => quando(l.t) }, { t: "Motivo", v: (l) => l.motivo }, { t: "Referência", v: (l) => l.referencia }, { t: "Linhas", v: (l) => l.linhas.map((x) => `${x.conta} ${x.valor}`).join(" · ") }], d.lancamentos || [])),
  );
}

async function telaAdminConfig() {
  const r = await obter("/admin/config");
  if (!r.ok) return h("section", { class: "tela" }, erroForm(r.erro));
  const c = r.dados;
  const cat = await catalogo();
  const campoNum = (id, rotulo, valor, ajuda, passo) => h("label", { class: "campo" }, rotulo, h("input", { id, type: "number", value: valor, step: passo || 1 }), ajuda ? h("small", {}, ajuda) : null);
  const precos = (m) =>
    h(
      "div",
      { class: "bloco" },
      h("h3", {}, `Preços em ${m}`),
      h(
        "div",
        { class: "campos duas" },
        campoNum(`cf-${m}-cpu`, "CPU por núcleo-hora (centavos)", (c.tabelas[m].cpu_nucleo_hora / 10000).toFixed(4), "", "0.0001"),
        campoNum(`cf-${m}-ram`, "Memória por GB-hora (centavos)", (c.tabelas[m].ram_gib_hora / 10000).toFixed(4), "", "0.0001"),
        campoNum(`cf-${m}-gpu`, "GPU por hora (centavos)", (c.tabelas[m].gpu_hora / 10000).toFixed(4), "", "0.0001"),
        campoNum(`cf-${m}-ops`, "Ritmo do núcleo de referência (operações/s)", c.tabelas[m].ops_por_s_cpu),
      ),
    );
  const form = h(
    "form",
    {
      class: "tela",
      onsubmit: async (ev) => {
        ev.preventDefault();
        const nova = structuredClone(c);
        for (const m of Object.keys(nova.tabelas)) {
          nova.tabelas[m].cpu_nucleo_hora = Math.round(Number($(`#cf-${m}-cpu`).value) * 10000);
          nova.tabelas[m].ram_gib_hora = Math.round(Number($(`#cf-${m}-ram`).value) * 10000);
          nova.tabelas[m].gpu_hora = Math.round(Number($(`#cf-${m}-gpu`).value) * 10000);
          nova.tabelas[m].ops_por_s_cpu = Math.round(Number($(`#cf-${m}-ops`).value));
        }
        nova.fornecedor_bp = Math.round(Number($("#cf-parte").value) * 100);
        nova.retencao_dias = Number($("#cf-retencao").value);
        nova.pix = { chave: $("#cf-pix-chave").value.trim(), titular: $("#cf-pix-titular").value.trim() };
        nova.cadastro_aberto = $("#cf-cadastro").checked;
        nova.versao_minima_agente = $("#cf-versao").value.trim();
        nova.contato = $("#cf-contato").value.trim();
        nova.motores_desligados = cat.motores.filter((m) => !$(`#cf-motor-${m.tipo}`).checked).map((m) => m.tipo);
        nova.politica.unidades_max = Number($("#cf-unidades").value);
        nova.politica.tarefas_por_hora = Number($("#cf-por-hora").value);
        nova.politica.cadastros_por_ip_dia = Number($("#cf-por-ip").value);
        const x = await postar("/admin/config", { config: nova, resumo: $("#cf-resumo").value });
        if (!x.ok) return aviso(x.erro, true);
        aviso("Configuração salva e registrada no diário.");
        mostrar();
      },
    },
    cabecalho("Configuração", "Preços, comissão, retenção, Pix, motores e limites. Toda mudança vai para o diário com o seu nome."),
    h("div", { class: "grade-dupla" }, ...Object.keys(c.tabelas).map(precos)),
    h(
      "div",
      { class: "grade-dupla" },
      h(
        "div",
        { class: "bloco" },
        h("h3", {}, "Divisão e retenção"),
        h("div", { class: "campos duas" }, campoNum("cf-parte", "Parte do fornecedor (%)", c.fornecedor_bp / 100, "o resto é a comissão da plataforma", "0.01"), campoNum("cf-retencao", "Retenção dos ganhos (dias)", c.retencao_dias)),
        h("h3", {}, "Pix para o pagamento manual"),
        h("div", { class: "campos duas" }, h("label", { class: "campo" }, "Chave Pix da empresa", h("input", { id: "cf-pix-chave", type: "text", value: c.pix.chave })), h("label", { class: "campo" }, "Titular", h("input", { id: "cf-pix-titular", type: "text", value: c.pix.titular }))),
        h("p", { class: "nota" }, "Sem chave Pix, o botão de pagamento mostra que a venda ainda não abriu."),
      ),
      h(
        "div",
        { class: "bloco" },
        h("h3", {}, "Motores (kill switch por categoria)"),
        ...cat.motores.map((m) => h("label", { class: "checar" }, h("input", { id: `cf-motor-${m.tipo}`, type: "checkbox", checked: c.motores_desligados.includes(m.tipo) ? null : true }), h("span", {}, `${m.nome} (${m.tipo})`))),
        h("h3", {}, "Acesso e limites"),
        h("label", { class: "checar" }, h("input", { id: "cf-cadastro", type: "checkbox", checked: c.cadastro_aberto ? true : null }), h("span", {}, "Cadastro de contas aberto")),
        h("div", { class: "campos duas" }, h("label", { class: "campo" }, "Versão mínima do agente", h("input", { id: "cf-versao", type: "text", value: c.versao_minima_agente })), h("label", { class: "campo" }, "Contato do suporte", h("input", { id: "cf-contato", type: "text", value: c.contato || "", maxlength: 200, placeholder: "suporte@… ou endereço" })), campoNum("cf-unidades", "Unidades por tarefa", c.politica.unidades_max), campoNum("cf-por-hora", "Tarefas por cliente por hora", c.politica.tarefas_por_hora), campoNum("cf-por-ip", "Contas por IP por dia", c.politica.cadastros_por_ip_dia)),
      ),
    ),
    h("div", { class: "bloco" }, h("label", { class: "campo" }, "O que mudou (vai para o registro)", h("input", { id: "cf-resumo", type: "text", maxlength: 300 })), h("div", { class: "acoes" }, h("button", { type: "submit", class: "botao" }, "Salvar configuração")), json(c, "configuração inteira")),
  );
  return form;
}

// ---------------------------------------------------------------------------
// partida
// ---------------------------------------------------------------------------

try {
  const t = localStorage.getItem("hyx-tema");
  if (t) document.documentElement.dataset.tema = t;
} catch {
  /* sem armazenamento */
}

window.addEventListener("hashchange", mostrar);
(async () => {
  await carregarEu();
  if (!location.hash) location.hash = estado.eu ? "#/painel" : "#/";
  await mostrar();
})();
