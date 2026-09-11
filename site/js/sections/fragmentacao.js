// 16×16: cifra de verdade (AES-GCM), corta em 256 fragmentos de 16 bytes, hash SHA-512 de cada um.
import { el, etapas, espera } from "../ui.js";
import { curto, temCripto } from "../simulation/engine.js";

const LETRAS = "ABCDEFGHIJKLMNOP";

export function iniciar({ t, engine, calmo, aoMudarIdioma }) {
  const tab = document.getElementById("tabuleiro");
  const form = document.getElementById("frag-form");
  const entrada = document.getElementById("frag-texto");
  const saida = document.getElementById("frag-saida");
  const detalhe = document.getElementById("frag-detalhe");
  const bRec = document.getElementById("frag-reconstruir");
  const bCor = document.getElementById("frag-corromper");
  const marca = etapas(document.getElementById("frag-etapas"), ["DATA", "ENCRYPTION", "FRAGMENTATION", "16×16", "DISTRIBUTION"]);
  tab.setAttribute("role", "group");

  const casas = [];
  tab.append(el("span", { class: "eixo" }));
  for (let c = 0; c < 16; c++) tab.append(el("span", { class: "eixo", text: LETRAS[c] }));
  for (let l = 0; l < 16; l++) {
    tab.append(el("span", { class: "eixo", text: String(l + 1) }));
    for (let c = 0; c < 16; c++) {
      const i = l * 16 + c, id = LETRAS[c] + String(l + 1).padStart(2, "0");
      const b = el("button", { type: "button", "aria-label": id, "aria-pressed": "false", onclick: () => escolher(i) });
      casas.push(b); tab.append(b);
    }
  }

  let pacote = null, escolhido = -1, rodando = false, ultimaMsg = null;
  const msg = (chave, vars, classe = "") => { ultimaMsg = [chave, vars, classe]; saida.className = "saida " + classe; saida.textContent = t(chave, vars); };

  function escolher(i) {
    if (!pacote) return;
    escolhido = i;
    casas.forEach((b, k) => b.setAttribute("aria-pressed", String(k === i)));
    const f = pacote.fragmentos[i], c = (k) => t(`frag.ficha.${k}`);
    detalhe.replaceChildren(el("dl", { class: "campos" },
      el("dt", { text: c("fragmento") }), el("dd", { text: f.id }),
      el("dt", { text: c("hash") }), el("dd", { text: curto(f.hash, 12) }),
      el("dt", { text: c("no") }), el("dd", { text: `#${LETRAS[f.no % 16]}${String(10 + f.no).padStart(2, "0")}` }),
      el("dt", { text: c("estado") }), el("dd", { class: f.estado === "corrompido" ? "erro" : f.estado === "conferido" ? "ok" : "", text: t(`frag.estados.${f.estado}`) }),
      el("dt", { text: c("bytes") }), el("dd", { text: [...f.bytes].map((x) => x.toString(16).padStart(2, "0")).join(" ") }),
    ));
  }

  async function fragmentar() {
    if (rodando) return;
    if (!temCripto) { msg("frag.sem_cripto", null, "erro"); return; }
    rodando = true; bRec.disabled = true; bCor.disabled = true;
    casas.forEach((b) => { b.className = ""; });
    marca(0); await espera(calmo ? 0 : 400);
    marca(1); msg("frag.cifrando");
    pacote = await engine.fragmentData(entrada.value || "DATA");
    await espera(calmo ? 0 : 500);
    marca(2); await espera(calmo ? 0 : 400);
    marca(3);
    for (let l = 0; l < 16; l++) { for (let c = 0; c < 16; c++) casas[l * 16 + c].classList.add("cheio"); if (!calmo) await espera(45); }
    marca(4);
    const nosUsados = new Set(pacote.fragmentos.map((f) => f.no)).size;
    const ordem = [...casas.keys()].sort(() => engine.aleat() - 0.5);
    for (let k = 0; k < ordem.length; k++) { casas[ordem[k]].classList.replace("cheio", "enviado"); if (!calmo && k % 8 === 0) await espera(16); }
    msg("frag.pronto", { n: pacote.tamanho, m: nosUsados });
    bRec.disabled = false; bCor.disabled = false; rodando = false;
    escolher(escolhido >= 0 ? escolhido : 6);
  }

  bRec.addEventListener("click", async () => {
    if (!pacote || rodando) return;
    rodando = true;
    const r = await engine.reconstructData(pacote);
    for (let i = 0; i < 256; i++) {
      const f = pacote.fragmentos[i];
      if (r.falha && f === r.falha) break;
      if (f.estado !== "corrompido") { f.estado = "conferido"; casas[i].className = "conferido"; }
      if (!calmo && i % 16 === 0) await espera(20);
    }
    if (r.ok) msg("frag.ok", { t: r.texto }, "ok");
    else msg("frag.falhou", { c: r.falha ? r.falha.id : "?" }, "erro");
    if (escolhido >= 0) escolher(escolhido);
    rodando = false;
  });
  bCor.addEventListener("click", () => {
    if (!pacote || rodando) return;
    const i = engine.int(256), f = pacote.fragmentos[i];
    f.bytes[engine.int(16)] ^= 1 << engine.int(8);
    f.estado = "corrompido"; casas[i].className = "corrompido";
    msg("frag.corrompido", { c: f.id }, "erro");
    escolher(i);
  });
  form.addEventListener("submit", (e) => { e.preventDefault(); escolhido = -1; fragmentar(); });

  // roda sozinho na primeira vez que o capítulo aparece
  const obs = new IntersectionObserver((es) => { if (es[0].isIntersecting) { obs.disconnect(); fragmentar(); } }, { threshold: 0.25 });
  obs.observe(tab);

  aoMudarIdioma(() => { if (ultimaMsg) msg(...ultimaMsg); if (escolhido >= 0) escolher(escolhido); });
}
