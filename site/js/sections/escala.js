// ZOOM OUT: a rolagem dentro do capítulo afasta a câmera de um nó até a infraestrutura global.
const DEGRAUS = ["ONE NODE", "TEN NODES", "HUNDREDS", "THOUSANDS", "NETWORK", "GLOBAL INFRASTRUCTURE"];

export function iniciar({ mundo }) {
  const secao = document.getElementById("escala");
  const contagem = document.getElementById("zoom-contagem");
  const degraus = [...document.getElementById("zoom-degraus").children];
  let atual = -1, ativo = false;

  function progresso() {
    const r = secao.getBoundingClientRect();
    const total = r.height - window.innerHeight;
    return total > 0 ? Math.min(1, Math.max(0, -r.top / total)) : 0;
  }
  function atualizar() {
    if (!ativo) return;
    const p = progresso();
    mundo.definirZoom(p);
    const k = Math.min(DEGRAUS.length - 1, Math.floor(p * DEGRAUS.length));
    if (k !== atual) {
      atual = k;
      contagem.textContent = DEGRAUS[k];
      degraus.forEach((d, i) => d.classList.toggle("ativo", i === k));
    }
  }
  new IntersectionObserver((es) => { ativo = es[0].isIntersecting; atualizar(); }).observe(secao);
  window.addEventListener("scroll", atualizar, { passive: true });
  atualizar();
}
