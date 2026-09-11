// Peças pequenas reutilizadas pelos capítulos.

export const espera = (ms) => new Promise((r) => setTimeout(r, ms));

export function el(tag, props = {}, ...filhos) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === "class") e.className = v;
    else if (k === "text") e.textContent = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (k === "dataset") Object.assign(e.dataset, v);
    else if (v !== undefined && v !== null && v !== false) e.setAttribute(k, v === true ? "" : v);
  }
  filhos.flat().forEach((f) => { if (f !== null && f !== undefined) e.append(f instanceof Node ? f : String(f)); });
  return e;
}

export function selo(estado, t) {
  return el("span", { class: "selo", "data-estado": estado }, el("i"), el("span", { text: t(`estado.${estado}`) }));
}

// Preenche uma lista de etapas (<ol class="etapas">) e devolve uma função que marca a atual.
export function etapas(ol, nomes) {
  ol.replaceChildren(...nomes.map((n) => el("li", {}, el("span", { text: n }))));
  return (atual, erro = false) => {
    [...ol.children].forEach((li, i) => {
      li.classList.toggle("feita", i < atual);
      li.classList.toggle("ativa", i === atual && !erro);
      li.classList.toggle("erro", i === atual && erro);
    });
  };
}

// Um canvas que acompanha o tamanho do contêiner, com densidade de pixels limitada.
export function telaViva(canvas, desenhar) {
  const ctx = canvas.getContext("2d");
  const s = { w: 0, h: 0, ctx };
  const medir = (redesenha = true) => {
    const r = canvas.getBoundingClientRect(), dpr = Math.min(window.devicePixelRatio || 1, 2);
    s.w = r.width; s.h = r.height;
    canvas.width = Math.max(1, Math.round(r.width * dpr)); canvas.height = Math.max(1, Math.round(r.height * dpr));
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    if (redesenha && desenhar) desenhar(s);
  };
  medir(false); // o primeiro desenho fica com quem chamou; o ResizeObserver cuida dos seguintes
  new ResizeObserver(medir).observe(canvas);
  return s;
}

// Roda um laço de animação só enquanto o elemento está na tela e a aba visível.
export function laco(alvo, passo, calmo) {
  let ativo = false, visivel = false, ultimo = 0;
  const f = (agora) => {
    if (!ativo) return;
    const dt = Math.min(0.05, (agora - (ultimo || agora)) / 1000); ultimo = agora;
    passo(dt, agora / 1000);
    requestAnimationFrame(f);
  };
  const avaliar = () => {
    const deve = visivel && !document.hidden && !calmo;
    if (deve && !ativo) { ativo = true; ultimo = 0; requestAnimationFrame(f); }
    if (!deve) ativo = false;
  };
  new IntersectionObserver((es) => { visivel = es[0].isIntersecting; avaliar(); }, { rootMargin: "80px" }).observe(alvo);
  document.addEventListener("visibilitychange", avaliar);
  if (calmo) passo(0, 0);
  return { redesenhar: () => passo(0, performance.now() / 1000) };
}

// Executa uma sequência de passos com pausa, cancelável se o capítulo recomeçar.
export function sequencia() {
  let geracao = 0;
  return {
    nova() { geracao++; const g = geracao; return { vivo: () => g === geracao, espera: async (ms) => { await espera(ms); return g === geracao; } }; },
    cancelar() { geracao++; },
  };
}

export const formataAUR = (unidades, casas = 8) => {
  const s = String(unidades).padStart(casas + 1, "0");
  const inteiro = s.slice(0, -casas), frac = s.slice(-casas).replace(/0+$/, "");
  return frac ? `${inteiro}.${frac}` : inteiro;
};
