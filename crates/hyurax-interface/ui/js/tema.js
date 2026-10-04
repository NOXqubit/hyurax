// Hyurax / Ultrax — o tema (Cloud Design 2.0), aplicado antes de desenhar:
// claro (padrão), escuro ou o do sistema. A escolha fica só neste computador.
(function () {
  var t = "claro";
  try { t = localStorage.getItem("hyurax-tema") || "claro"; } catch (e) { /* sem armazenamento */ }
  if (t !== "escuro" && t !== "sistema") t = "claro";
  document.documentElement.setAttribute("data-tema", t);
})();
