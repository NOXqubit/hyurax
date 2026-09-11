// Apresentação narrada: oito cenas sobre o mundo 3D, com legenda e voz sintética do aparelho
// no idioma escolhido, quando o aparelho tiver essa voz.
export function iniciar({ t, idioma, mundo, aoMudarIdioma, restaurarMundo }) {
  const sessao = document.getElementById("sessao-video");
  const titulo = document.getElementById("titulo-v"), fala = document.getElementById("fala-v");
  const bPlay = document.getElementById("v-play"), bVoz = document.getElementById("v-voz"), aviso = document.getElementById("aviso-voz-v");
  const barras = [...document.getElementById("v-progresso").children];
  const voz = "speechSynthesis" in window ? window.speechSynthesis : null;
  let atual = 0, tocando = false, comVoz = !!voz, timer = 0, raf = 0, inicio = 0, falaAtual = null, focoAntes = null;
  const cenas = () => t("video.cenas");

  function vozIdioma() {
    if (!voz) return null;
    const l = idioma().toLowerCase(), base = l.slice(0, 2), vs = voz.getVoices();
    return vs.find((v) => v.lang.toLowerCase().replace("_", "-") === l) || vs.find((v) => v.lang.toLowerCase().startsWith(base)) || null;
  }
  function interface_() {
    bPlay.textContent = tocando ? t("ui.pausar") : t("ui.assistir");
    bVoz.textContent = comVoz ? t("ui.voz_on") : t("ui.voz_off");
    bVoz.setAttribute("aria-pressed", String(comVoz));
    aviso.textContent = vozIdioma() ? t("ui.voz_sintetica") : t("ui.sem_voz");
  }
  if (voz) voz.addEventListener("voiceschanged", interface_);

  function mostrar() {
    const c = cenas()[atual];
    titulo.textContent = `${atual + 1} / ${cenas().length} · ${c.titulo}`;
    fala.textContent = c.fala;
    mundo.definirModo(c.mundo);
    if (c.mundo === "global") mundo.definirZoom(1);
    barras.forEach((b, k) => { b.classList.toggle("feito", k < atual); b.classList.toggle("atual", k === atual); b.style.setProperty("--p", "0"); });
  }
  function limpar() { clearTimeout(timer); cancelAnimationFrame(raf); if (voz) voz.cancel(); falaAtual = null; }
  function progresso() {
    if (!tocando) return;
    const p = Math.min(1, (performance.now() - inicio) / (cenas()[atual].seg * 1000));
    barras[atual].style.setProperty("--p", p.toFixed(3));
    raf = requestAnimationFrame(progresso);
  }
  function tocarCena() {
    limpar(); mostrar();
    inicio = performance.now(); raf = requestAnimationFrame(progresso);
    let terminouFala = !comVoz, passouTempo = false;
    const talvez = () => { if (terminouFala && passouTempo && tocando) avancar(); };
    timer = setTimeout(() => { passouTempo = true; talvez(); }, cenas()[atual].seg * 1000);
    const v = comVoz ? vozIdioma() : null;
    if (v) {
      const u = new SpeechSynthesisUtterance(cenas()[atual].fala);
      u.voice = v; u.lang = v.lang;
      u.onend = () => { if (falaAtual === u) { terminouFala = true; talvez(); } };
      u.onerror = () => { terminouFala = true; talvez(); };
      falaAtual = u; voz.speak(u);
    } else terminouFala = true;
  }
  function avancar() {
    if (atual >= cenas().length - 1) { tocando = false; limpar(); barras.forEach((b) => { b.classList.add("feito"); b.classList.remove("atual"); }); interface_(); return; }
    atual++; tocarCena();
  }
  function tocar() { tocando = true; interface_(); tocarCena(); }
  function pausar() { tocando = false; limpar(); interface_(); }

  function abrir() {
    focoAntes = document.activeElement;
    sessao.hidden = false; document.body.style.overflow = "hidden";
    atual = 0; interface_(); mostrar(); tocar(); bPlay.focus();
  }
  function fechar() {
    pausar(); sessao.hidden = true; document.body.style.overflow = "";
    restaurarMundo();
    if (focoAntes) focoAntes.focus();
  }
  document.getElementById("abre-video").addEventListener("click", abrir);
  document.getElementById("fecha-video").addEventListener("click", fechar);
  sessao.addEventListener("keydown", (e) => {
    if (e.key === "Escape") fechar();
    if (e.key === "Tab") { // mantém o foco dentro da apresentação
      const f = [...sessao.querySelectorAll("button")]; const i = f.indexOf(document.activeElement);
      if (e.shiftKey && i <= 0) { e.preventDefault(); f[f.length - 1].focus(); } else if (!e.shiftKey && i === f.length - 1) { e.preventDefault(); f[0].focus(); }
    }
  });
  bPlay.addEventListener("click", () => (tocando ? pausar() : (atual >= cenas().length - 1 && !tocando ? (atual = 0, tocar()) : tocar())));
  document.getElementById("v-prox").addEventListener("click", () => { atual = Math.min(atual + 1, cenas().length - 1); tocando ? tocarCena() : mostrar(); });
  document.getElementById("v-ant").addEventListener("click", () => { atual = Math.max(atual - 1, 0); tocando ? tocarCena() : mostrar(); });
  bVoz.addEventListener("click", () => { comVoz = !comVoz && !!voz; interface_(); if (tocando) tocarCena(); });
  if (!voz) comVoz = false;
  interface_();
  aoMudarIdioma(() => { interface_(); if (!sessao.hidden) { if (tocando) tocarCena(); else mostrar(); } });
}
