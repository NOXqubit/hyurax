// Hyurax / Ultrax — Segurança: quem é este nó, o que protege cada caminho, o
// que ainda falta, e os eventos de segurança e de erro. Tudo sai do estado
// que o núcleo manda; nada é decoração.

import { el, curto, trocar } from "../util.js";
import { fatos, linhaDeRegistro, marcarEstado, estadoEl } from "./comum.js";

const CATEGORIAS = new Set(["seguranca", "erro", "carteira", "sistema", "rede"]);

export function atualizar(e) {
  const s = e.seguranca;
  if (!s) return;
  const atu = e.atualizacao || {};
  const pendencias = [!s.segundo_fator, s.assinatura_windows === "PENDENTE", !e.no?.pares].filter(Boolean).length;
  marcarEstado("sg-estado", pendencias ? "atencao" : "ok", pendencias ? `${pendencias} ponto(s) de atenção` : "sem ponto de atenção");
  fatos("sg-identidade", [
    ["Versão", `${e.versao} · rede de teste`],
    ["Identidade do nó (Noise)", el("span", { class: "num", title: s.identidade_no }, curto(s.identidade_no, 20))],
    ["Chave do worker (assina provas e anúncios)", el("span", { class: "num", title: s.worker }, curto(s.worker, 20))],
    ["SHA-512 deste executável", s.executavel_sha512 ? el("span", { class: "num", title: s.executavel_sha512 }, curto(s.executavel_sha512, 24)) : e.pode_mandar ? "calculando…" : "só nesta janela"],
    ["Chave de lançamento embutida", s.chave_de_lancamento ? "sim: atualização só com manifesto assinado" : "não (programa compilado sem ela)"],
    ["Última busca de atualização", atu.verificado_ms ? new Date(atu.verificado_ms).toLocaleString("pt-BR", { hour12: false }) : "ainda não buscou"],
    ["Versão nova conferida", atu.disponivel ? `${atu.disponivel.versao} (assinatura conferida)` : "nenhuma"],
  ]);
  trocar(
    "sg-protecoes",
    [
      [true, "API local", s.api_local],
      [true, "Entre nós", s.cifra_entre_nos],
      [!!s.segundo_fator, "Segundo fator", s.segundo_fator ? "ligado: códigos de 6 dígitos para destrancar e enviar" : "desligado: ligue em Carteira → Segundo fator"],
      [s.assinatura_windows !== "PENDENTE", "Assinatura do programa (Windows)", s.assinatura_windows === "PENDENTE" ? "PENDENTE: confira o SHA-512 acima contra o da Release" : s.assinatura_windows],
      [true, "Armazenamento na rede", "cifrado aqui (ChaCha20-Poly1305); quem guarda não vê o conteúdo"],
      [true, "Livro de contas", e.nuvem?.livro_integro === false ? "ADULTERADO: veja Faturamento" : "encadeado por hash, conferido ao abrir"],
    ].map(([ok, nome, nota]) => el("li", {}, estadoEl(ok ? "ok" : "atencao", ok ? "ativo" : "atenção"), el("span", {}, nome), el("small", {}, nota))),
  );
  const regs = (e.registros || []).filter((r) => CATEGORIAS.has(r.dados?.categoria)).slice(-80);
  trocar("sg-eventos", regs.length ? regs.map((r) => linhaDeRegistro(r.quando_ms, r.dados?.categoria, r.dados?.texto)) : [el("li", { class: "nota" }, "nenhum evento recente")]);
}

export function aoMostrar(e) {
  if (e) atualizar(e);
}
