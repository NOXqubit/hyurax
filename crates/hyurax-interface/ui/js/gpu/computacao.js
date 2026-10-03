// Hyurax / Ultrax — o backend de GPU do ULTRAX: WebGL 2 usado para CALCULAR
// (não para desenhar), num worker da própria janela do programa.
//
// Funciona na GPU integrada (Intel, AMD Ryzen) e na placa de vídeo. A conta é
// C = A · B em inteiros de 32 bits: cada pixel de uma textura R32UI é uma
// entrada de C, e o shader soma os n produtos da linha de A pela coluna de B.
// As entradas de A e B ficam abaixo de 1000, então cada soma cabe folgada em
// 32 bits: o resultado é exato, sem ponto flutuante.
//
// O nó não confia na GPU: o resultado volta para ele, e a CPU confere por
// Freivalds (outro algoritmo) antes de creditar qualquer coisa.
//
// A cada faixa lida de volta, uma linha real de C vai para a cena 3D
// (`opcoes.faixa`): é o dado que a GPU acabou de calcular.
//
// O limite de uso é por tempo: a GPU calcula uma faixa de linhas, e a página
// descansa na proporção escolhida antes da próxima (com 50%, descansa o mesmo
// tempo que trabalhou). Faixas curtas também evitam que o Windows ache que a
// GPU travou.

const VERTICE = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const FRAGMENTO = `#version 300 es
precision highp float;
precision highp int;
precision highp usampler2D;
uniform usampler2D A;
uniform usampler2D B;
uniform int n;
uniform int linha0;
out uvec4 cor;
void main() {
  int j = int(gl_FragCoord.x);
  int i = int(gl_FragCoord.y) + linha0;
  uint soma = 0u;
  for (int k = 0; k < n; k++) {
    soma += texelFetch(A, ivec2(k, i), 0).r * texelFetch(B, ivec2(j, k), 0).r;
  }
  cor = uvec4(soma, 0u, 0u, 0u);
}`;

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

// Cria o motor, ou devolve null quando não há WebGL2 (e o painel diz isso).
// No worker usa OffscreenCanvas: a conta segue com a janela minimizada.
export function criarGpu() {
  const canvas = typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(1, 1) : document.createElement("canvas");
  canvas.width = canvas.height = 1;
  const gl = canvas.getContext("webgl2", { antialias: false, depth: false, stencil: false, powerPreference: "high-performance" });
  if (!gl) return null;
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  const nome = String(info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER) || "GPU");
  const compilar = (tipo, fonte) => {
    const sh = gl.createShader(tipo);
    gl.shaderSource(sh, fonte);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh) || "shader");
    return sh;
  };
  let programa;
  try {
    programa = gl.createProgram();
    gl.attachShader(programa, compilar(gl.VERTEX_SHADER, VERTICE));
    gl.attachShader(programa, compilar(gl.FRAGMENT_SHADER, FRAGMENTO));
    gl.linkProgram(programa);
    if (!gl.getProgramParameter(programa, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(programa) || "link");
  } catch (erro) {
    console.warn("GPU sem suporte ao shader inteiro:", erro);
    return null;
  }
  const maxTextura = gl.getParameter(gl.MAX_TEXTURE_SIZE);

  const textura = (largura, altura, dados) => {
    const t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R32UI, largura, altura, 0, gl.RED_INTEGER, gl.UNSIGNED_INT, dados);
    return t;
  };

  // C = A · B, faixa por faixa. `opcoes.uso()` diz a fatia de tempo (10 a 100),
  // `opcoes.parar()` interrompe, `opcoes.progresso(linhas)` conta o que já foi.
  async function multiplicar(A, B, n, opcoes) {
    if (n > maxTextura) throw new Error(`a GPU aceita texturas até ${maxTextura}, e a tarefa pede ${n}`);
    const tA = textura(n, n, A);
    const tB = textura(n, n, B);
    const C = new Uint32Array(n * n);
    let faixa = Math.max(1, Math.min(n, Math.floor(65536 / n)));
    const alvo = gl.createTexture();
    const fb = gl.createFramebuffer();
    try {
      gl.useProgram(programa);
      gl.uniform1i(gl.getUniformLocation(programa, "A"), 0);
      gl.uniform1i(gl.getUniformLocation(programa, "B"), 1);
      gl.uniform1i(gl.getUniformLocation(programa, "n"), n);
      const uLinha = gl.getUniformLocation(programa, "linha0");
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, tA);
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, tB);
      gl.activeTexture(gl.TEXTURE2);
      gl.bindTexture(gl.TEXTURE_2D, alvo);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.R32UI, n, n, 0, gl.RED_INTEGER, gl.UNSIGNED_INT, null);
      gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, alvo, 0);
      if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) throw new Error("a GPU não aceita desenhar em inteiros");
      let lido = new Uint32Array(n * faixa * 4);
      for (let linha0 = 0; linha0 < n; ) {
        if (opcoes.parar()) return null;
        const linhas = Math.min(faixa, n - linha0);
        const comeco = performance.now();
        gl.viewport(0, 0, n, linhas);
        gl.uniform1i(uLinha, linha0);
        gl.drawArrays(gl.TRIANGLES, 0, 3);
        if (lido.length < n * linhas * 4) lido = new Uint32Array(n * linhas * 4);
        // ler de volta espera a GPU terminar: é aqui que o tempo da faixa fecha
        gl.readPixels(0, 0, n, linhas, gl.RGBA_INTEGER, gl.UNSIGNED_INT, lido);
        for (let r = 0; r < linhas; r++) {
          const base = (linha0 + r) * n;
          for (let j = 0; j < n; j++) C[base + j] = lido[(r * n + j) * 4];
        }
        // uma linha real desta faixa para a cena: a do meio
        const meio = Math.floor(linhas / 2);
        opcoes.faixa?.(linha0 + meio, C.subarray((linha0 + meio) * n, (linha0 + meio + 1) * n));
        linha0 += linhas;
        opcoes.progresso(linha0);
        const gasto = performance.now() - comeco;
        // faixas de uns 60 ms: curtas para o limite responder, longas para render
        if (gasto < 30 && faixa < n) faixa = Math.min(n, faixa * 2);
        else if (gasto > 120 && faixa > 1) faixa = Math.max(1, Math.floor(faixa / 2));
        const uso = Math.max(10, Math.min(100, opcoes.uso()));
        const descanso = gasto * (100 / uso - 1);
        await dormir(Math.min(2000, Math.max(0, descanso)));
      }
      return C;
    } finally {
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.deleteFramebuffer(fb);
      for (const t of [tA, tB, alvo]) gl.deleteTexture(t);
    }
  }

  return { nome, maxTextura, multiplicar };
}
