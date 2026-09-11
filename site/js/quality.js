// Detecta a capacidade gráfica e escolhe um perfil HIGH, MEDIUM ou LOW.
// O mundo 3D ainda rebaixa o perfil sozinho se os quadros ficarem lentos.

export const PERFIS = {
  HIGH: { nos: 1500, pacotes: 140, dpr: 2, antialias: true },
  MEDIUM: { nos: 780, pacotes: 80, dpr: 1.5, antialias: true },
  LOW: { nos: 320, pacotes: 36, dpr: 1, antialias: false },
};

export function detectarQualidade() {
  const calmo = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const movel = matchMedia("(pointer: coarse)").matches || innerWidth < 760;
  let gl = null;
  try {
    const c = document.createElement("canvas");
    gl = c.getContext("webgl2") || c.getContext("webgl");
  } catch { gl = null; }
  if (!gl) return { nivel: "NONE", calmo, movel, perfil: null };

  let placa = "";
  const ext = gl.getExtension("WEBGL_debug_renderer_info");
  if (ext) placa = String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) || "");
  const lose = gl.getExtension("WEBGL_lose_context");
  if (lose) lose.loseContext();

  const nucleos = navigator.hardwareConcurrency || 4;
  const memoria = navigator.deviceMemory || 4;
  const software = /swiftshader|llvmpipe|software|basic render/i.test(placa);
  // Intel HD sem número (Atom), HD 2000–4xxx e GMA; Mali e Adreno antigos
  const integradaAntiga = /intel.*(gma|hd graphics(?! [5-9]\d{2}\b)(?! p\d))|mali-[34]|adreno \(tm\) [1-4]\d\d/i.test(placa);

  let nivel = "HIGH";
  if (movel || nucleos <= 4 || memoria <= 4) nivel = "MEDIUM";
  if (software || integradaAntiga || memoria <= 2 || (movel && nucleos <= 4)) nivel = "LOW";
  return { nivel, calmo, movel, placa, perfil: PERFIS[nivel] };
}
