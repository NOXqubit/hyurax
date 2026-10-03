// Hyurax / Ultrax — Ajustes: estimativa de energia, painel na rede local,
// avisos, pastas do programa e termos de uso.

import { $, el, texto, hora, fmt } from "../util.js";
import { postar } from "../api.js";
import { fatos, resultado } from "./comum.js";

let urlDoQr = "";
let abrirTermos = () => {};

async function mudar(campos) {
  const r = await postar("/ajustes", campos);
  if (!r.ok) resultado("a-energia-saida", r);
}

function caixa(id, valor) {
  const c = $(id);
  if (document.activeElement !== c) c.checked = !!valor;
}

function campo(id, valor) {
  const c = $(id);
  if (document.activeElement !== c && c.dataset.mexido !== "1") c.value = valor;
}

export function montar(opcoes) {
  abrirTermos = opcoes.abrirTermos;
  for (const id of ["a-watts", "a-kwh"]) $(id).addEventListener("input", (e) => { e.target.dataset.mexido = "1"; });
  $("a-energia").addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const r = await postar("/ajustes", { watts_nucleo: Math.round(Number($("a-watts").value)), centavos_kwh: Math.round(Number($("a-kwh").value) * 100) });
    resultado("a-energia-saida", r, "Salvo. A estimativa de energia e de custo usa estes valores.");
    if (r.ok) $("a-watts").dataset.mexido = $("a-kwh").dataset.mexido = "";
  });
  $("a-na-rede").addEventListener("change", (e) => mudar({ na_rede: e.target.checked ? "1" : "0" }));
  $("a-avisar").addEventListener("change", (e) => mudar({ avisar_bloco: e.target.checked ? "1" : "0" }));
  $("a-som").addEventListener("change", (e) => mudar({ som_bloco: e.target.checked ? "1" : "0" }));
  $("a-abrir").addEventListener("click", async () => {
    const r = await postar("/abrir-pasta");
    if (!r.ok) resultado("a-energia-saida", r);
  });
  $("a-termos").addEventListener("click", () => abrirTermos(false));
  $("a-atu-buscar").addEventListener("click", async () => {
    const r = await postar("/atualizacao/buscar");
    resultado("a-atu-saida", r, "Buscando o manifesto e conferindo a assinatura…");
  });
  $("a-atu-instalar").addEventListener("click", async () => {
    if (!confirm("Baixar a versão nova, conferir e abrir o instalador? O programa fecha para ele atualizar.")) return;
    const r = await postar("/atualizacao/instalar");
    resultado("a-atu-saida", r, "Baixando e conferindo o instalador…");
  });
}

function atualizacao(e) {
  const a = e.atualizacao;
  if (!a) return;
  const janela = e.modo === "janela" && e.pode_mandar;
  const d = a.disponivel;
  fatos("a-atu", [
    ["Versão instalada", a.atual],
    ["Chave de lançamento", a.chave ? "embutida" : "PENDENTE: este programa foi montado sem a chave"],
    ["Última busca", a.verificado_ms ? hora(a.verificado_ms) : "ainda não"],
    ["Versão nova", d ? `${d.versao} (${fmt(d.tamanho / 1048576, 1)} MiB, assinada e conferida)${d.notas ? ` · ${d.notas}` : ""}` : "nenhuma"],
    a.erro ? ["Problema", a.erro] : null,
    a.baixando ? ["Agora", "baixando e conferindo o instalador"] : null,
  ]);
  $("a-atu-buscar").disabled = !janela || !a.chave;
  $("a-atu-instalar").hidden = !d;
  $("a-atu-instalar").disabled = !janela || a.baixando;
}

export function atualizar(e) {
  const a = e.ajustes;
  if (!a) return;
  const janela = e.modo === "janela" && e.pode_mandar;
  for (const x of $("a-energia").elements) x.disabled = !janela;
  for (const id of ["a-na-rede", "a-avisar", "a-som", "a-abrir"]) $(id).disabled = !janela;
  campo("a-watts", a.watts_nucleo);
  campo("a-kwh", (a.centavos_kwh / 100).toFixed(2));
  caixa("a-na-rede", a.na_rede);
  caixa("a-avisar", a.avisar_bloco);
  caixa("a-som", a.som_bloco);
  $("a-som").disabled = !janela || !a.avisar_bloco;
  const qr = $("a-qr");
  if (a.na_rede && a.url_celular) {
    texto("a-url", `No celular, na mesma rede: ${a.url_celular} (só leitura).`);
    qr.hidden = false;
    if (urlDoQr !== a.url_celular) {
      urlDoQr = a.url_celular;
      try {
        const q = globalThis.qrcode(0, "M");
        q.addData(a.url_celular);
        q.make();
        qr.innerHTML = q.createSvgTag({ cellSize: 4, margin: 1, scalable: true });
      } catch {
        qr.textContent = "QR indisponível";
      }
    }
  } else {
    texto("a-url", a.na_rede ? "Sem endereço na rede local." : "O painel só abre neste computador.");
    qr.hidden = true;
  }
  fatos("a-pastas", [
    ["Configuração e carteira", el("span", { class: "num" }, a.pasta_config || "—")],
    ["Dados (cadeia, ULTRAX, registros)", el("span", { class: "num" }, a.pasta_dados || "—")],
    ["Versão", `${e.produto} ${e.versao}`],
    ["Rede", `${e.rede?.nome} (${e.rede?.tipo})`],
  ]);
  atualizacao(e);
  texto("a-termos-estado", e.termos?.aceitos ? `Versão ${e.termos.versao} aceita neste computador.` : `Versão ${e.termos?.versao ?? "—"} ainda não aceita.`);
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}

