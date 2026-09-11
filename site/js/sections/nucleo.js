// AURON CORE: mapa clicável das quatro partes do núcleo.
import { el, selo } from "../ui.js";

export function iniciar({ t, aoMudarIdioma }) {
  const svg = document.getElementById("nucleo-svg");
  const corpo = document.getElementById("nucleo-detalhe");
  let parte = "blockchain";

  function mostrar() {
    const d = t(`nucleo.partes.${parte}`);
    svg.querySelectorAll(".no-botao").forEach((g) => g.setAttribute("aria-pressed", String(g.dataset.parte === parte)));
    const novo = selo(d.estado, t);
    novo.id = "nucleo-selo";
    document.getElementById("nucleo-selo").replaceWith(novo);
    corpo.replaceChildren(
      el("h3", { text: d.nome }),
      el("p", { text: d.texto }),
      el("ul", {}, d.itens.map((i) => el("li", { text: i }))),
    );
  }
  svg.querySelectorAll(".no-botao").forEach((g) => {
    const escolher = () => { parte = g.dataset.parte; mostrar(); };
    g.addEventListener("click", escolher);
    g.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); escolher(); } });
  });
  mostrar();
  aoMudarIdioma(mostrar);
}
