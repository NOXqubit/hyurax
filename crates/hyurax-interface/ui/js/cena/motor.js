// Hyurax / Ultrax — o motor da cena 3D (WebGL 2, sem biblioteca).
//
// O motor só DESENHA. Quem decide o que aparece são os visualizadores
// (visualizadores.js), a partir das amostras reais dos motores de cálculo.
// Aqui não existe animação própria: a cena muda quando chega dado novo ou
// quando a pessoa mexe na câmera, e só então é redesenhada.
//
// Primitivas: caixas (instanciadas), linhas, pontos e malhas de triângulos,
// todas em coordenadas do mundo, com cor por elemento. Câmera orbital:
// arrastar gira, roda do mouse aproxima, Shift+arrastar (ou botão direito)
// desloca, duplo clique volta ao enquadramento.

const VS_CAIXA = `#version 300 es
layout(location=0) in vec3 p; layout(location=1) in vec3 nrm;
layout(location=2) in vec3 pos; layout(location=3) in vec3 tam; layout(location=4) in vec4 cor;
uniform mat4 mvp; out vec4 c; out float luz;
void main(){ gl_Position = mvp * vec4(pos + p * tam, 1.0);
  luz = 0.42 + 0.58 * max(dot(nrm, normalize(vec3(0.35, 1.0, 0.55))), 0.0); c = cor; }`;
const FS_CAIXA = `#version 300 es
precision mediump float; in vec4 c; in float luz; out vec4 o; void main(){ o = vec4(c.rgb * luz, c.a); }`;

const VS_SIMPLES = `#version 300 es
layout(location=0) in vec3 p; layout(location=1) in vec4 cor;
uniform mat4 mvp; uniform float tamanho; out vec4 c;
void main(){ gl_Position = mvp * vec4(p, 1.0); gl_PointSize = tamanho; c = cor; }`;
const FS_SIMPLES = `#version 300 es
precision mediump float; in vec4 c; out vec4 o; void main(){ o = c; }`;
const FS_PONTO = `#version 300 es
precision mediump float; in vec4 c; out vec4 o;
void main(){ vec2 d = gl_PointCoord - 0.5; if (dot(d, d) > 0.25) discard; o = c; }`;

const VS_MALHA = `#version 300 es
layout(location=0) in vec3 p; layout(location=1) in vec3 nrm; layout(location=2) in vec4 cor;
uniform mat4 mvp; out vec4 c; out float luz;
void main(){ gl_Position = mvp * vec4(p, 1.0);
  luz = 0.45 + 0.55 * abs(dot(normalize(nrm), normalize(vec3(0.35, 1.0, 0.55)))); c = cor; }`;

// cubo unitário com base em y = 0: a altura cresce para cima
function geometriaDoCubo() {
  const faces = [
    [[0, 1, 0], [[-0.5, 1, -0.5], [0.5, 1, -0.5], [0.5, 1, 0.5], [-0.5, 1, 0.5]]],
    [[0, -1, 0], [[-0.5, 0, 0.5], [0.5, 0, 0.5], [0.5, 0, -0.5], [-0.5, 0, -0.5]]],
    [[1, 0, 0], [[0.5, 0, -0.5], [0.5, 0, 0.5], [0.5, 1, 0.5], [0.5, 1, -0.5]]],
    [[-1, 0, 0], [[-0.5, 0, 0.5], [-0.5, 0, -0.5], [-0.5, 1, -0.5], [-0.5, 1, 0.5]]],
    [[0, 0, 1], [[0.5, 0, 0.5], [-0.5, 0, 0.5], [-0.5, 1, 0.5], [0.5, 1, 0.5]]],
    [[0, 0, -1], [[-0.5, 0, -0.5], [0.5, 0, -0.5], [0.5, 1, -0.5], [-0.5, 1, -0.5]]],
  ];
  const v = [];
  for (const [n, q] of faces) for (const k of [0, 1, 2, 0, 2, 3]) v.push(...q[k], ...n);
  return new Float32Array(v);
}

function mul(a, b) {
  const r = new Array(16).fill(0);
  for (let i = 0; i < 4; i++) for (let j = 0; j < 4; j++) for (let k = 0; k < 4; k++) r[j * 4 + i] += a[k * 4 + i] * b[j * 4 + k];
  return r;
}
function perspectiva(fov, asp, perto, longe) {
  const f = 1 / Math.tan(fov / 2);
  return [f / asp, 0, 0, 0, 0, f, 0, 0, 0, 0, (longe + perto) / (perto - longe), -1, 0, 0, (2 * longe * perto) / (perto - longe), 0];
}
function olhar(olho, alvo) {
  const sub = (a, b) => a.map((x, i) => x - b[i]);
  const norm = (a) => { const l = Math.hypot(...a) || 1; return a.map((x) => x / l); };
  const cruz = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const z = norm(sub(olho, alvo));
  const x = norm(cruz([0, 1, 0], z));
  const y = cruz(z, x);
  const d = (a) => -(a[0] * olho[0] + a[1] * olho[1] + a[2] * olho[2]);
  return [x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, d(x), d(y), d(z), 1];
}

export class Motor {
  /** @param {HTMLCanvasElement} canvas */
  constructor(canvas) {
    this.canvas = canvas;
    const gl = canvas.getContext("webgl2", { antialias: true, premultipliedAlpha: false });
    this.gl = gl;
    this.ok = !!gl;
    this.camera = { yaw: 0.85, pitch: 0.62, dist: 2.4, alvo: [0, 0.2, 0] };
    this.inicial = { ...this.camera, alvo: [...this.camera.alvo] };
    this.cena = { caixas: new Float32Array(0), linhas: new Float32Array(0), pontos: new Float32Array(0), malha: new Float32Array(0), tamanhoPonto: 6 };
    this.sujo = true;
    if (!gl) return;
    this.progCaixa = this.programa(VS_CAIXA, FS_CAIXA);
    this.progLinha = this.programa(VS_SIMPLES, FS_SIMPLES);
    this.progPonto = this.programa(VS_SIMPLES, FS_PONTO);
    this.progMalha = this.programa(VS_MALHA, FS_CAIXA);
    // caixas: cubo + atributos por instância (pos, tam, cor)
    this.vaoCaixa = gl.createVertexArray();
    gl.bindVertexArray(this.vaoCaixa);
    const vb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vb);
    gl.bufferData(gl.ARRAY_BUFFER, geometriaDoCubo(), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);
    this.bufCaixas = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.bufCaixas);
    for (const [loc, n, off] of [[2, 3, 0], [3, 3, 12], [4, 4, 24]]) {
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, n, gl.FLOAT, false, 40, off);
      gl.vertexAttribDivisor(loc, 1);
    }
    // linhas e pontos: posição + cor
    this.vaoLinha = this.vaoSimples();
    this.vaoPonto = this.vaoSimples();
    // malha: posição + normal + cor
    this.vaoMalha = gl.createVertexArray();
    gl.bindVertexArray(this.vaoMalha);
    this.bufMalha = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.bufMalha);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 40, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 40, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.FLOAT, false, 40, 24);
    gl.bindVertexArray(null);
    this.ligarControles();
    const quadro = () => {
      if (this.sujo) this.desenhar();
      requestAnimationFrame(quadro);
    };
    requestAnimationFrame(quadro);
  }

  programa(vs, fs) {
    const gl = this.gl;
    const sh = (tipo, src) => {
      const s = gl.createShader(tipo);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) || "shader");
      return s;
    };
    const p = gl.createProgram();
    gl.attachShader(p, sh(gl.VERTEX_SHADER, vs));
    gl.attachShader(p, sh(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) || "link");
    return { p, mvp: gl.getUniformLocation(p, "mvp"), tamanho: gl.getUniformLocation(p, "tamanho") };
  }

  vaoSimples() {
    const gl = this.gl;
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 28, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 4, gl.FLOAT, false, 28, 12);
    gl.bindVertexArray(null);
    return { vao, buf };
  }

  /**
   * Troca a cena. Cada lista é plana:
   * - caixas: [x, y, z, lx, ly, lz, r, g, b, a] por caixa (base em y);
   * - linhas: pares de vértices [x, y, z, r, g, b, a];
   * - pontos: [x, y, z, r, g, b, a];
   * - malha: triângulos [x, y, z, nx, ny, nz, r, g, b, a] por vértice.
   */
  mostrar({ caixas = [], linhas = [], pontos = [], malha = [], tamanhoPonto = 6 } = {}) {
    const f = (x) => (x instanceof Float32Array ? x : new Float32Array(x));
    this.cena = { caixas: f(caixas), linhas: f(linhas), pontos: f(pontos), malha: f(malha), tamanhoPonto };
    if (this.ok) {
      const gl = this.gl;
      gl.bindBuffer(gl.ARRAY_BUFFER, this.bufCaixas);
      gl.bufferData(gl.ARRAY_BUFFER, this.cena.caixas, gl.DYNAMIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.vaoLinha.buf);
      gl.bufferData(gl.ARRAY_BUFFER, this.cena.linhas, gl.DYNAMIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.vaoPonto.buf);
      gl.bufferData(gl.ARRAY_BUFFER, this.cena.pontos, gl.DYNAMIC_DRAW);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.bufMalha);
      gl.bufferData(gl.ARRAY_BUFFER, this.cena.malha, gl.DYNAMIC_DRAW);
    }
    this.sujo = true;
  }

  /** Volta a câmera ao enquadramento inicial. */
  reenquadrar() {
    this.camera = { ...this.inicial, alvo: [...this.inicial.alvo] };
    this.sujo = true;
  }

  matriz() {
    const c = this.camera;
    const olho = [
      c.alvo[0] + Math.cos(c.yaw) * Math.cos(c.pitch) * c.dist,
      c.alvo[1] + Math.sin(c.pitch) * c.dist,
      c.alvo[2] + Math.sin(c.yaw) * Math.cos(c.pitch) * c.dist,
    ];
    const w = this.canvas.width || 1;
    const h = this.canvas.height || 1;
    return mul(perspectiva(0.75, w / h, 0.02, 40), olhar(olho, c.alvo));
  }

  /** Posição na tela (px, relativa ao canvas) de um ponto do mundo, ou null atrás da câmera. */
  projetar(p) {
    const m = this.matriz();
    const x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
    const y = m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13];
    const w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
    if (w <= 0) return null;
    const r = this.canvas.getBoundingClientRect();
    return [((x / w + 1) / 2) * r.width, ((1 - y / w) / 2) * r.height];
  }

  desenhar() {
    this.sujo = false;
    const { gl, canvas } = this;
    if (!gl) return;
    const esc = Math.min(devicePixelRatio || 1, 2);
    const w = Math.max(1, Math.round(canvas.clientWidth * esc));
    const h = Math.max(1, Math.round(canvas.clientHeight * esc));
    if (canvas.width !== w || canvas.height !== h) Object.assign(canvas, { width: w, height: h });
    gl.viewport(0, 0, w, h);
    gl.clearColor(0.028, 0.028, 0.032, 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    const mvp = new Float32Array(this.matriz());
    const { caixas, linhas, pontos, malha, tamanhoPonto } = this.cena;
    if (malha.length) {
      gl.useProgram(this.progMalha.p);
      gl.uniformMatrix4fv(this.progMalha.mvp, false, mvp);
      gl.bindVertexArray(this.vaoMalha);
      gl.drawArrays(gl.TRIANGLES, 0, malha.length / 10);
    }
    if (caixas.length) {
      gl.useProgram(this.progCaixa.p);
      gl.uniformMatrix4fv(this.progCaixa.mvp, false, mvp);
      gl.bindVertexArray(this.vaoCaixa);
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 36, caixas.length / 10);
    }
    if (linhas.length) {
      gl.useProgram(this.progLinha.p);
      gl.uniformMatrix4fv(this.progLinha.mvp, false, mvp);
      gl.bindVertexArray(this.vaoLinha.vao);
      gl.drawArrays(gl.LINES, 0, linhas.length / 7);
    }
    if (pontos.length) {
      gl.useProgram(this.progPonto.p);
      gl.uniformMatrix4fv(this.progPonto.mvp, false, mvp);
      gl.uniform1f(this.progPonto.tamanho, tamanhoPonto * esc);
      gl.bindVertexArray(this.vaoPonto.vao);
      gl.drawArrays(gl.POINTS, 0, pontos.length / 7);
    }
    gl.bindVertexArray(null);
    this.aoDesenhar?.();
  }

  ligarControles() {
    const c = this.canvas;
    let arrasto = null;
    c.addEventListener("contextmenu", (e) => e.preventDefault());
    c.addEventListener("pointerdown", (e) => {
      arrasto = { x: e.clientX, y: e.clientY, deslocar: e.shiftKey || e.button === 2 };
      c.setPointerCapture(e.pointerId);
    });
    c.addEventListener("pointermove", (e) => {
      if (!arrasto) return;
      const dx = e.clientX - arrasto.x;
      const dy = e.clientY - arrasto.y;
      arrasto.x = e.clientX;
      arrasto.y = e.clientY;
      const cam = this.camera;
      if (arrasto.deslocar) {
        const k = cam.dist * 0.0022;
        cam.alvo[0] += (Math.sin(cam.yaw) * dx - Math.cos(cam.yaw) * Math.sin(cam.pitch) * dy) * k;
        cam.alvo[1] += Math.cos(cam.pitch) * dy * k;
        cam.alvo[2] += (-Math.cos(cam.yaw) * dx - Math.sin(cam.yaw) * Math.sin(cam.pitch) * dy) * k;
      } else {
        cam.yaw += dx * 0.008;
        cam.pitch = Math.min(1.5, Math.max(-0.2, cam.pitch + dy * 0.008));
      }
      this.sujo = true;
    });
    const soltar = () => { arrasto = null; };
    c.addEventListener("pointerup", soltar);
    c.addEventListener("pointercancel", soltar);
    c.addEventListener("wheel", (e) => {
      e.preventDefault();
      this.camera.dist = Math.min(12, Math.max(0.4, this.camera.dist * (e.deltaY > 0 ? 1.1 : 0.91)));
      this.sujo = true;
    }, { passive: false });
    c.addEventListener("dblclick", () => this.reenquadrar());
    c.tabIndex = 0;
    c.addEventListener("keydown", (e) => {
      const cam = this.camera;
      const passos = { ArrowLeft: [-0.12, 0], ArrowRight: [0.12, 0], ArrowUp: [0, 0.08], ArrowDown: [0, -0.08] };
      if (passos[e.key]) {
        cam.yaw += passos[e.key][0];
        cam.pitch = Math.min(1.5, Math.max(-0.2, cam.pitch + passos[e.key][1]));
      } else if (e.key === "+" || e.key === "=") cam.dist = Math.max(0.4, cam.dist * 0.9);
      else if (e.key === "-") cam.dist = Math.min(12, cam.dist * 1.1);
      else if (e.key === "0") return this.reenquadrar();
      else return;
      e.preventDefault();
      this.sujo = true;
    });
    new ResizeObserver(() => { this.sujo = true; }).observe(c);
  }
}
