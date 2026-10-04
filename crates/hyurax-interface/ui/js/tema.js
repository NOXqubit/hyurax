// Hyurax / Ultrax — o tema (Cloud Design 2.0), aplicado antes de desenhar:
// escuro (padrão), claro ou o do sistema. A escolha fica só neste computador.
(function () {
  var t = "escuro";
  try { t = localStorage.getItem("hyurax-tema") || "escuro"; } catch (e) { /* sem armazenamento */ }
  if (t !== "claro" && t !== "sistema") t = "escuro";
  document.documentElement.setAttribute("data-tema", t);
})();
