// Hyurax / Ultrax — Registro: tudo o que o núcleo anotou, com filtro por
// categoria e busca. O arquivo completo fica na pasta de dados.

import { $, el, texto, trocar } from "../util.js";
import { linhaDeRegistro } from "./comum.js";

let ultimo = null;

function desenhar() {
  const e = ultimo;
  if (!e) return;
  const regs = e.registros || [];
  const filtro = $("g-filtro");
  const categorias = [...new Set(regs.map((r) => r.dados?.categoria).filter(Boolean))].sort();
  const atuais = [...filtro.options].slice(1).map((o) => o.value);
  for (const c of categorias) if (!atuais.includes(c)) filtro.append(el("option", { value: c }, c));
  const cat = filtro.value;
  const busca = $("g-busca").value.trim().toLowerCase();
  const lista = regs
    .filter((r) => (!cat || r.dados?.categoria === cat) && (!busca || String(r.dados?.texto).toLowerCase().includes(busca)));
  trocar("g-lista", lista.map((r) => linhaDeRegistro(r.quando_ms, r.dados?.categoria, r.dados?.texto)));
  const pasta = e.ajustes?.pasta_dados;
  texto("g-arquivo", `${lista.length} de ${regs.length} registro(s) recentes nesta tela.${pasta ? ` O registro completo fica em ${pasta}${pasta.includes("\\") ? "\\" : "/"}registros.` : ""}`);
}

export function montar() {
  $("g-filtro").addEventListener("change", desenhar);
  $("g-busca").addEventListener("input", desenhar);
}

export function atualizar(e) {
  ultimo = e;
  desenhar();
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}

