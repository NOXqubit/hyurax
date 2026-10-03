// Hyurax / Ultrax — Carteira: criar ou importar, saldo, endereço com QR,
// envio em duas etapas (conferir, depois assinar com a senha), movimentos e
// o segundo fator (código de 6 dígitos).
//
// A chave nunca sai do núcleo: a tela só manda a senha na hora de assinar.

import { $, el, texto, fmt, curto } from "../util.js";
import { postar } from "../api.js";
import { fatos, tabela, resultado } from "./comum.js";

let enderecoDoQr = "";
let conferido = null; // o pedido conferido pelo núcleo, esperando a senha
let fase2fa = ""; // "", "configurando"
let exigeCodigo = false;

function qr(id, valor) {
  const caixa = $(id);
  if (!caixa) return;
  try {
    const q = globalThis.qrcode(0, "M");
    q.addData(valor);
    q.make();
    caixa.innerHTML = q.createSvgTag({ cellSize: 4, margin: 1, scalable: true });
  } catch {
    caixa.textContent = "QR indisponível";
  }
}

function semCarteira(e) {
  $("c-sem-carteira").hidden = false;
  $("c-com-carteira").hidden = true;
  const janela = e.modo === "janela" && e.pode_mandar;
  for (const f of ["c-nova", "c-importar"]) for (const x of $(f).elements) x.disabled = !janela;
  if (!janela) texto("c-nova-saida", e.modo !== "janela" ? "Neste modo a carteira é a do arquivo indicado na linha de comando (hyurax-no carteira)." : "Só deste computador.");
}

function comCarteira(e) {
  const c = e.carteira;
  $("c-sem-carteira").hidden = true;
  $("c-com-carteira").hidden = false;
  texto("c-saldo", `${c.saldo} HYX`);
  fatos("c-fatos", [
    ["Imaturo", `${c.imaturo} HYX`, "recompensa de bloco que ainda não passou da maturidade"],
    ["Pode enviar até", `${c.maximo_envio} HYX`],
    ["Taxa padrão", `${c.taxa_padrao} HYX`],
    ["Próximo nonce", fmt(c.proximo_nonce)],
    ["Rede", `${e.rede?.nome} · o HYX de teste não tem valor`],
  ]);
  texto("c-endereco", c.endereco);
  if (c.endereco !== enderecoDoQr) {
    enderecoDoQr = c.endereco;
    qr("c-qr", c.endereco);
  }
  const taxa = $("e-taxa");
  if (!taxa.value && document.activeElement !== taxa) taxa.placeholder = c.taxa_padrao;
  $("e-botao").disabled = !c.pode_enviar && !conferido;
  if (!c.pode_enviar && !conferido) texto("e-saida", !e.pode_mandar ? "Envio só deste computador." : "Sem saldo maduro para enviar.");
  tabela(
    "c-historico",
    [{ t: "Quando" }, { t: "Altura", num: true }, { t: "Tipo" }, { t: "Valor", num: true }, { t: "Taxa", num: true }, { t: "Outro lado" }, { t: "Transação" }],
    (c.historico || []).map((m) => [
      m.quando ? new Date(m.quando * 1000).toLocaleString("pt-BR", { hour12: false }) : "—",
      { v: m.pendente ? "na fila" : fmt(m.altura), num: true },
      m.tipo,
      { v: `${m.entrada ? "+" : "−"}${m.valor}`, num: true },
      { v: m.taxa, num: true },
      el("span", { class: "num", title: m.outro || "" }, curto(m.outro, 16)),
      el("span", { class: "num", title: m.txid }, curto(m.txid, 12)),
    ]),
    { vazio: "nenhum movimento ainda" },
  );
  seguranca(e);
}

// ---------- envio ----------
function limparEnvio() {
  conferido = null;
  $("e-confirmar").hidden = true;
  $("e-senha").value = "";
  $("e-codigo").value = "";
  $("e-botao").textContent = "Conferir";
  $("e-cancelar").hidden = true;
  for (const id of ["e-para", "e-valor", "e-taxa"]) $(id).disabled = false;
}

async function enviar(ev) {
  ev.preventDefault();
  const campos = { para: $("e-para").value.trim(), valor: $("e-valor").value.trim().replace(",", "."), taxa: ($("e-taxa").value.trim() || $("e-taxa").placeholder).replace(",", ".") };
  if (!conferido) {
    const r = await postar("/carteira/conferir", campos);
    if (!r.ok) return resultado("e-saida", r);
    conferido = campos;
    for (const id of ["e-para", "e-valor", "e-taxa"]) $(id).disabled = true;
    texto("e-resumo", `Enviar ${r.dados.valor} HYX de teste para ${r.dados.para}, com taxa de ${r.dados.taxa} HYX. Confira o endereço: transação na cadeia não volta.`);
    $("e-confirmar").hidden = false;
    $("e-codigo-rotulo").hidden = !exigeCodigo;
    $("e-cancelar").hidden = false;
    $("e-botao").textContent = "Assinar e enviar";
    texto("e-saida", "");
    $("e-senha").focus();
    return;
  }
  const r = await postar("/carteira/enviar", { ...conferido, senha: $("e-senha").value, codigo: $("e-codigo").value.trim() });
  if (!r.ok) return resultado("e-saida", r);
  resultado("e-saida", r, `Enviado: transação ${curto(r.dados.txid, 16)} (nonce ${r.dados.nonce}), anunciada a ${r.dados.pares} par(es). Entra num bloco quando alguém minerar.`);
  limparEnvio();
  $("e-para").value = "";
  $("e-valor").value = "";
}

// ---------- segundo fator ----------
function seguranca(e) {
  const s = e.carteira.seguranca || {};
  exigeCodigo = !!(s.ligado && s.exige_envio);
  const caixa = $("c-seguranca");
  if (fase2fa === "configurando") return;
  const desenhado = caixa.dataset.fase;
  const fase = s.ligado ? "ligado" : "desligado";
  if (desenhado === fase && caixa.dataset.exige === String(s.exige_envio) && caixa.dataset.trava === String(s.trava)) return;
  caixa.dataset.fase = fase;
  caixa.dataset.exige = String(s.exige_envio);
  caixa.dataset.trava = String(s.trava);
  if (!s.ligado) {
    caixa.replaceChildren(
      el("p", {}, "Com o segundo fator ligado, enviar HYX (e, se quiser, abrir o programa) pede o código de 6 dígitos de um aplicativo autenticador (TOTP), além da senha."),
      el("button", { type: "button", class: "botao-leve", disabled: !e.pode_mandar, onclick: comecar2fa }, "Ligar o segundo fator"),
      el("p", { class: "saida", id: "s-saida", role: "status" }),
    );
    return;
  }
  const exige = el("input", { type: "checkbox", id: "s-exige" });
  exige.checked = !!s.exige_envio;
  const trava = el("input", { type: "checkbox", id: "s-trava" });
  trava.checked = !!s.trava;
  const codigo = el("input", { id: "s-codigo", inputmode: "numeric", maxlength: "6", autocomplete: "one-time-code" });
  const mudar = async (desligar) => {
    const r = await postar("/seguranca/mudar", { codigo: codigo.value.trim(), desligar: desligar ? "1" : "0", exige_envio: exige.checked ? "1" : "0", trava: trava.checked ? "1" : "0" });
    resultado("s-saida", r, desligar ? "Segundo fator desligado." : "Salvo.");
    codigo.value = "";
    caixa.dataset.fase = "";
  };
  caixa.replaceChildren(
    el("p", {}, "Ligado."),
    el("label", { class: "chave", for: "s-exige" }, exige, el("span", {}, "Pedir o código para enviar HYX")),
    el("label", { class: "chave", for: "s-trava" }, trava, el("span", {}, "Trancar o programa ao abrir (pede o código para destrancar)")),
    el("label", { for: "s-codigo" }, el("small", {}, "Código atual de 6 dígitos (para salvar ou desligar)"), codigo),
    el(
      "div",
      { class: "acoes" },
      el("button", { type: "button", class: "botao-leve", onclick: () => mudar(false) }, "Salvar"),
      el("button", { type: "button", class: "botao-leve", onclick: () => confirm("Desligar o segundo fator?") && mudar(true) }, "Desligar"),
    ),
    el("p", { class: "saida", id: "s-saida", role: "status" }),
  );
}

async function comecar2fa() {
  const r = await postar("/seguranca/comecar");
  if (!r.ok) return resultado("s-saida", r);
  fase2fa = "configurando";
  const caixa = $("c-seguranca");
  const codigo = el("input", { id: "s-codigo-novo", inputmode: "numeric", maxlength: "6", autocomplete: "one-time-code" });
  const trava = el("input", { type: "checkbox", id: "s-trava-novo" });
  const qrCaixa = el("div", { class: "qr", id: "s-qr", role: "img", "aria-label": "QR Code para o aplicativo autenticador" });
  caixa.replaceChildren(
    el("p", {}, "1. No aplicativo autenticador, leia o QR Code (ou digite o segredo)."),
    qrCaixa,
    el("p", { class: "endereco" }, r.dados.segredo),
    el("p", {}, "2. Digite o código que aparecer no aplicativo."),
    el("label", { for: "s-codigo-novo" }, el("small", {}, "Código de 6 dígitos"), codigo),
    el("label", { class: "chave", for: "s-trava-novo" }, trava, el("span", {}, "Também trancar o programa ao abrir")),
    el(
      "div",
      { class: "acoes" },
      el("button", {
        type: "button",
        class: "botao",
        onclick: async () => {
          const c = await postar("/seguranca/confirmar", { codigo: codigo.value.trim(), trava: trava.checked ? "1" : "0" });
          resultado("s-saida", c, "Segundo fator ligado.");
          if (c.ok) {
            fase2fa = "";
            caixa.dataset.fase = "";
          }
        },
      }, "Confirmar"),
      el("button", { type: "button", class: "botao-leve", onclick: () => { fase2fa = ""; caixa.dataset.fase = ""; } }, "Desistir"),
    ),
    el("p", { class: "saida", id: "s-saida", role: "status" }),
  );
  qr("s-qr", r.dados.uri);
  codigo.focus();
}

// ---------- criar e importar ----------
async function criar(ev) {
  ev.preventDefault();
  const r = await postar("/carteira/nova", { senha: $("c-senha").value, senha2: $("c-senha2").value });
  resultado("c-nova-saida", r, r.ok ? `Carteira criada: ${r.dados.endereco}. Guarde a senha: ninguém consegue recuperá-la.` : "");
  if (r.ok) $("c-senha").value = $("c-senha2").value = "";
}

async function importar(ev) {
  ev.preventDefault();
  const r = await postar("/carteira/importar", { conteudo: $("c-conteudo").value });
  resultado("c-importar-saida", r, r.ok ? `Importada: ${r.dados.endereco}${r.dados.sem_senha ? ". Aviso: este arquivo guarda o segredo sem senha; proteja com hyurax-no carteira cifrar." : "."}` : "");
  if (r.ok) $("c-conteudo").value = "";
}

export function montar() {
  $("c-nova").addEventListener("submit", criar);
  $("c-importar").addEventListener("submit", importar);
  $("c-enviar").addEventListener("submit", enviar);
  for (const id of ["e-para", "e-valor", "e-taxa"]) $(id).addEventListener("input", () => texto("e-saida", ""));
  $("c-enviar").addEventListener("reset", limparEnvio);
}

export function atualizar(e) {
  if (!e.carteira) return;
  if (e.carteira.existe) comCarteira(e);
  else semCarteira(e);
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}

