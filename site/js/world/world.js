// Auron World: uma única rede de nós em Three.js que muda de forma conforme o capítulo.
// Um só objeto de pontos (sem um componente por nó), posições interpoladas na CPU,
// brilho e tamanho calculados no shader. Linhas compartilham o buffer de posições.
import * as THREE from "../../vendor/three.module.min.js";

const MODOS = ["rede", "logo", "cadeia", "grade", "malha", "global", "nucleo"];

const CAMERAS = {
  rede: { pos: [0, 10, 170], alvo: [0, 0, 0], linhas: 0.13, giro: 1 },
  logo: { pos: [0, 0, 150], alvo: [0, -23, 0], linhas: 0.05, giro: 0 },
  cadeia: { pos: [-10, 26, 128], alvo: [0, 0, 0], linhas: 0.03, giro: 0.25 },
  grade: { pos: [0, 78, 92], alvo: [0, -4, 0], linhas: 0.02, giro: 0.2 },
  malha: { pos: [0, 70, 138], alvo: [0, -10, 0], linhas: 0.13, giro: 0.35 },
  global: { pos: [0, 0, 250], alvo: [0, 0, 0], linhas: 0.1, giro: 0.6 },
  nucleo: { pos: [0, 26, 176], alvo: [0, 0, 0], linhas: 0.08, giro: 0.8 },
};

// ---------- formas ----------
function aleatorio(semente) {
  let a = semente >>> 0;
  return () => { a = (a + 0x6d2b79f5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
function gauss(r) { return (r() + r() + r() - 1.5) * 1.15; }
function naCasca(r, rMin, rMax) {
  const u = r() * 2 - 1, th = r() * Math.PI * 2, rr = rMin + (rMax - rMin) * r();
  const s = Math.sqrt(1 - u * u);
  return [Math.cos(th) * s * rr, u * rr * 0.6, Math.sin(th) * s * rr];
}

// O "A" da marca, em coordenadas 0..100 do SVG.
const A_EXTERNO = [[50, 6], [95, 92], [76, 92], [50, 42], [24, 92], [5, 92]];
function dentroPoligono(x, y, p) {
  let d = false;
  for (let i = 0, j = p.length - 1; i < p.length; j = i++) {
    const [xi, yi] = p[i], [xj, yj] = p[j];
    if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) d = !d;
  }
  return d;
}
function naLamina(x, y) {
  // lâmina curva: entre duas curvas que sobem da esquerda para a direita
  if (x < 8 || x > 78.5) return false;
  const t = (x - 8) / 70.5;
  const cima = 90 - 33 * Math.pow(t, 0.72);
  const baixo = 90 - 24.5 * Math.pow(t, 0.85);
  return y > cima && y < baixo;
}
function pontoNoA(r, borda) {
  if (borda) {
    // contorno: sorteia uma aresta do polígono proporcional ao comprimento
    const p = A_EXTERNO, i = Math.floor(r() * p.length), [x1, y1] = p[i], [x2, y2] = p[(i + 1) % p.length];
    const t = r();
    return [x1 + (x2 - x1) * t, y1 + (y2 - y1) * t];
  }
  for (let k = 0; k < 60; k++) {
    const x = r() * 100, y = r() * 100;
    if (dentroPoligono(x, y, A_EXTERNO) || naLamina(x, y)) return [x, y];
  }
  return [50, 50];
}

function formas(N, sem) {
  const r = aleatorio(sem);
  const f = Object.fromEntries(MODOS.map((m) => [m, new Float32Array(N * 3)]));
  const put = (m, i, x, y, z) => { f[m][i * 3] = x; f[m][i * 3 + 1] = y; f[m][i * 3 + 2] = z; };

  // rede: aglomerados sobre um elipsoide, mais uma nuvem de ligação
  const centros = Array.from({ length: 9 }, (_, k) => {
    const a = (k / 9) * Math.PI * 2 + r() * 0.4;
    return [Math.cos(a) * (40 + r() * 45), (r() - 0.5) * 46, Math.sin(a) * (30 + r() * 40)];
  });
  for (let i = 0; i < N; i++) {
    if (i === 0) { put("rede", 0, 0, 0, 0); continue; }
    if (r() < 0.78) { const c = centros[i % centros.length]; put("rede", i, c[0] + gauss(r) * 15, c[1] + gauss(r) * 11, c[2] + gauss(r) * 15); }
    else { const p = naCasca(r, 10, 120); put("rede", i, p[0] * 1.3, p[1], p[2]); }
  }

  // fundo distante: estrelas para os modos em que parte dos nós forma uma figura
  const fundo = (m, i) => { const p = naCasca(r, 170, 330); put(m, i, p[0], p[1] * 1.4, p[2] - 60); };

  // logo: 64% dos nós desenham o A (metade no contorno, metade no preenchimento)
  // o A fica acima do nome, como na fachada; com poucos nós, quase todos vão para o A
  const esc = 0.58, parteA = N < 500 ? 0.82 : 0.66;
  for (let i = 0; i < N; i++) {
    if (i === 0 || r() < parteA) {
      const [x, y] = pontoNoA(r, r() < 0.5);
      put("logo", i, (x - 50) * esc, (58 - y) * esc, gauss(r) * 1.6);
    } else fundo("logo", i);
  }

  // cadeia: sete blocos com nós nas arestas e elos entre eles
  const arestas = [];
  for (const a of [-1, 1]) for (const b of [-1, 1]) { arestas.push([[-1, a, b], [1, a, b]], [[a, -1, b], [a, 1, b]], [[a, b, -1], [a, b, 1]]); }
  for (let i = 0; i < N; i++) {
    const q = r();
    if (i === 0 || q < 0.56) {
      const bloco = Math.floor(r() * 7), cx = (bloco - 3) * 30, s = 8;
      const [p1, p2] = arestas[Math.floor(r() * arestas.length)], t = r();
      put("cadeia", i, cx + (p1[0] + (p2[0] - p1[0]) * t) * s, (p1[1] + (p2[1] - p1[1]) * t) * s, (p1[2] + (p2[2] - p1[2]) * t) * s);
    } else if (q < 0.64) {
      const elo = Math.floor(r() * 6), x = (elo - 3) * 30 + 8 + r() * 14;
      put("cadeia", i, x, gauss(r) * 0.4, gauss(r) * 0.4);
    } else fundo("cadeia", i);
  }

  // grade: 16×16 casas num plano; o resto vira fragmentos flutuando ou fundo
  const passo = 6.2;
  for (let i = 0; i < N; i++) {
    if (i < 256) { const c = i % 16, l = Math.floor(i / 16); put("grade", i, (c - 7.5) * passo, 0, (l - 7.5) * passo); }
    else if (r() < 0.35) { const c = Math.floor(r() * 16), l = Math.floor(r() * 16); put("grade", i, (c - 7.5) * passo + gauss(r), 4 + r() * 26, (l - 7.5) * passo + gauss(r)); }
    else fundo("grade", i);
  }

  // malha: um território plano com cidades (aglomerados) e aparelhos soltos
  const cidades = Array.from({ length: 12 }, () => [(r() - 0.5) * 240, (r() - 0.5) * 150]);
  for (let i = 0; i < N; i++) {
    if (i === 0) { put("malha", 0, 0, 0, 0); continue; }
    if (r() < 0.7) { const c = cidades[i % cidades.length]; put("malha", i, c[0] + gauss(r) * 12, gauss(r) * 1.5, c[1] + gauss(r) * 12); }
    else put("malha", i, (r() - 0.5) * 280, gauss(r) * 2, (r() - 0.5) * 180);
  }

  // global: esfera de Fibonacci; o nó 0 fica de frente para a câmera
  const R = 62, ouro = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < N; i++) {
    const y = 1 - (i / (N - 1)) * 2, rad = Math.sqrt(1 - y * y), th = ouro * i;
    put("global", i, Math.cos(th) * rad * R, y * R, Math.sin(th) * rad * R);
  }
  {
    let melhor = 0, dMelhor = Infinity;
    for (let i = 0; i < N; i++) { const d = (f.global[i * 3] ** 2) + (f.global[i * 3 + 1] ** 2) + (f.global[i * 3 + 2] - R) ** 2; if (d < dMelhor) { dMelhor = d; melhor = i; } }
    for (let k = 0; k < 3; k++) { const tmp = f.global[k]; f.global[k] = f.global[melhor * 3 + k]; f.global[melhor * 3 + k] = tmp; }
  }

  // núcleo: um centro denso e três órbitas inclinadas
  const orbitas = [[30, 0.35], [52, -0.5], [78, 0.18]];
  for (let i = 0; i < N; i++) {
    if (i === 0 || r() < 0.16) { const p = naCasca(r, 0, 12); put("nucleo", i, p[0], p[1] * 1.6, p[2]); continue; }
    if (r() < 0.8) {
      const [raio, incl] = orbitas[Math.floor(r() * 3)], a = r() * Math.PI * 2, rr = raio + gauss(r) * 1.5;
      const x = Math.cos(a) * rr, z = Math.sin(a) * rr;
      put("nucleo", i, x, z * Math.sin(incl) + gauss(r) * 0.8, z * Math.cos(incl));
    } else fundo("nucleo", i);
  }
  return f;
}

// ligações: os 3 vizinhos mais próximos na forma "rede", com grade espacial
function vizinhos(p, N, k = 3, raio = 30) {
  const cel = raio, mapa = new Map(), chave = (x, y, z) => `${x},${y},${z}`;
  for (let i = 0; i < N; i++) {
    const c = chave(Math.floor(p[i * 3] / cel), Math.floor(p[i * 3 + 1] / cel), Math.floor(p[i * 3 + 2] / cel));
    if (!mapa.has(c)) mapa.set(c, []); mapa.get(c).push(i);
  }
  const pares = new Set(), adj = Array.from({ length: N }, () => []), lista = [];
  for (let i = 0; i < N; i++) {
    const cx = Math.floor(p[i * 3] / cel), cy = Math.floor(p[i * 3 + 1] / cel), cz = Math.floor(p[i * 3 + 2] / cel);
    const cand = [];
    for (let dx = -1; dx <= 1; dx++) for (let dy = -1; dy <= 1; dy++) for (let dz = -1; dz <= 1; dz++) {
      const l = mapa.get(chave(cx + dx, cy + dy, cz + dz)); if (!l) continue;
      for (const j of l) if (j !== i) { const d = (p[i * 3] - p[j * 3]) ** 2 + (p[i * 3 + 1] - p[j * 3 + 1]) ** 2 + (p[i * 3 + 2] - p[j * 3 + 2]) ** 2; if (d < raio * raio) cand.push([d, j]); }
    }
    cand.sort((a, b) => a[0] - b[0]);
    for (const [, j] of cand.slice(0, k)) {
      const c = i < j ? i * N + j : j * N + i;
      if (!pares.has(c)) { pares.add(c); lista.push(i, j); adj[i].push(j); adj[j].push(i); }
    }
  }
  return { indices: lista, adj };
}

function ordemPorDistancia(p, N, origem) {
  const ox = p[origem * 3], oy = p[origem * 3 + 1], oz = p[origem * 3 + 2];
  const idx = Array.from({ length: N }, (_, i) => i);
  const d = idx.map((i) => (p[i * 3] - ox) ** 2 + (p[i * 3 + 1] - oy) ** 2 + (p[i * 3 + 2] - oz) ** 2);
  idx.sort((a, b) => d[a] - d[b]);
  const ordem = new Float32Array(N);
  idx.forEach((i, rank) => { ordem[i] = N > 1 ? rank / (N - 1) : 0; });
  return ordem;
}

const VERT = `
attribute float aTam; attribute float aBrilho; attribute float aSemente; attribute float aOrdemA; attribute float aOrdemB;
uniform float uTempo; uniform float uEscala; uniform float uRevela; uniform float uQualOrdem; uniform float uOnda;
varying float vBrilho; varying float vVis;
void main() {
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  float cintila = 0.82 + 0.18 * sin(uTempo * 1.1 + aSemente * 57.0);
  float onda = uOnda * smoothstep(0.94, 1.0, sin(position.x * 0.055 - uTempo * 1.4));
  vBrilho = clamp(aBrilho + onda, 0.0, 1.6);
  float ordem = mix(aOrdemA, aOrdemB, uQualOrdem);
  vVis = 1.0 - smoothstep(uRevela - 0.002, uRevela + 0.0005, ordem);
  gl_PointSize = aTam * uEscala * cintila * (1.0 + vBrilho * 1.1) / max(-mv.z, 1.0);
  gl_Position = projectionMatrix * mv;
}`;
const FRAG = `
precision mediump float;
uniform vec3 uCor; uniform vec3 uQuente;
varying float vBrilho; varying float vVis;
void main() {
  vec2 c = gl_PointCoord - 0.5; float d = length(c);
  float miolo = smoothstep(0.22, 0.0, d);
  float halo = exp(-d * d * 16.0) * 0.55;
  float a = (miolo + halo) * vVis;
  if (a < 0.01) discard;
  vec3 cor = mix(uCor, uQuente, clamp(vBrilho, 0.0, 1.0));
  gl_FragColor = vec4(cor * a, a);
}`;

export function criarMundo(canvas, q) {
  const perfil = q.perfil, calmo = q.calmo, N = perfil.nos;
  const renderer = new THREE.WebGLRenderer({
    canvas, antialias: perfil.antialias, alpha: false,
    powerPreference: q.nivel === "LOW" ? "low-power" : "high-performance",
  });
  const intervaloQuadro = 1000 / (perfil.fps || 60);
  renderer.setClearColor(0x030303, 1);
  let dpr = Math.min(window.devicePixelRatio || 1, perfil.dpr);
  renderer.setPixelRatio(dpr);

  const cena = new THREE.Scene();
  cena.fog = new THREE.FogExp2(0x030303, 0.0023);
  const camera = new THREE.PerspectiveCamera(48, 1, 0.1, 3000);
  const grupo = new THREE.Group();
  cena.add(grupo);
  // O que fica preso à câmera: a assistente e os objetos de explicação. Assim
  // eles ficam sempre no mesmo lugar da tela, em qualquer modo do mundo.
  cena.add(camera);
  const hud = new THREE.Group();
  camera.add(hud);

  const F = formas(N, 1187);
  const pos = new Float32Array(F.rede);
  const { indices, adj } = vizinhos(F.rede, N);
  const r = aleatorio(99);
  const tam = new Float32Array(N), brilho = new Float32Array(N), semente = new Float32Array(N);
  const reforco = N < 500 ? 1.45 : N < 1000 ? 1.15 : 1;
  for (let i = 0; i < N; i++) { tam[i] = (i === 0 ? 4.2 : 1.6 + r() * 1.8) * 48 * reforco; semente[i] = r(); }
  const ordemA = ordemPorDistancia(F.rede, N, 0), ordemB = ordemPorDistancia(F.global, N, 0);

  const geo = new THREE.BufferGeometry();
  const atrPos = new THREE.BufferAttribute(pos, 3).setUsage(THREE.DynamicDrawUsage);
  const atrBrilho = new THREE.BufferAttribute(brilho, 1).setUsage(THREE.DynamicDrawUsage);
  geo.setAttribute("position", atrPos);
  geo.setAttribute("aTam", new THREE.BufferAttribute(tam, 1));
  geo.setAttribute("aBrilho", atrBrilho);
  geo.setAttribute("aSemente", new THREE.BufferAttribute(semente, 1));
  geo.setAttribute("aOrdemA", new THREE.BufferAttribute(ordemA, 1));
  geo.setAttribute("aOrdemB", new THREE.BufferAttribute(ordemB, 1));
  geo.boundingSphere = new THREE.Sphere(new THREE.Vector3(), 400);
  const uni = {
    uTempo: { value: 0 }, uEscala: { value: 1 }, uRevela: { value: 1 }, uQualOrdem: { value: 0 }, uOnda: { value: 0 },
    uCor: { value: new THREE.Color(0.72, 0.745, 0.78) }, uQuente: { value: new THREE.Color(1.0, 0.9, 0.74) },
  };
  const pontos = new THREE.Points(geo, new THREE.ShaderMaterial({ vertexShader: VERT, fragmentShader: FRAG, uniforms: uni, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }));
  grupo.add(pontos);

  const geoL = new THREE.BufferGeometry();
  geoL.setAttribute("position", atrPos);
  geoL.setIndex(indices);
  geoL.boundingSphere = geo.boundingSphere;
  const matL = new THREE.LineBasicMaterial({ color: 0x8f959e, transparent: true, opacity: 0, depthWrite: false, blending: THREE.AdditiveBlending });
  grupo.add(new THREE.LineSegments(geoL, matL));

  // destaque: ligações do nó sob o cursor
  const MAXD = 16;
  const posD = new Float32Array(MAXD * 6);
  const geoD = new THREE.BufferGeometry();
  geoD.setAttribute("position", new THREE.BufferAttribute(posD, 3).setUsage(THREE.DynamicDrawUsage));
  geoD.setDrawRange(0, 0);
  geoD.boundingSphere = geo.boundingSphere;
  const matD = new THREE.LineBasicMaterial({ color: 0xf3e2c4, transparent: true, opacity: 0.55, depthWrite: false, blending: THREE.AdditiveBlending });
  grupo.add(new THREE.LineSegments(geoD, matD));

  // pacotes: pontos quentes andando pelas ligações
  const P = calmo ? 0 : perfil.pacotes;
  const pacotes = Array.from({ length: P }, () => ({ a: 0, b: 0, t: 1, v: 0 }));
  const posP = new Float32Array(Math.max(P, 1) * 3);
  const geoP = new THREE.BufferGeometry();
  geoP.setAttribute("position", new THREE.BufferAttribute(posP, 3).setUsage(THREE.DynamicDrawUsage));
  const tamP = new Float32Array(Math.max(P, 1)).fill(150), brP = new Float32Array(Math.max(P, 1)).fill(1), semP = new Float32Array(Math.max(P, 1)), zP = new Float32Array(Math.max(P, 1));
  geoP.setAttribute("aTam", new THREE.BufferAttribute(tamP, 1));
  geoP.setAttribute("aBrilho", new THREE.BufferAttribute(brP, 1));
  geoP.setAttribute("aSemente", new THREE.BufferAttribute(semP, 1));
  geoP.setAttribute("aOrdemA", new THREE.BufferAttribute(zP, 1));
  geoP.setAttribute("aOrdemB", new THREE.BufferAttribute(zP, 1));
  geoP.boundingSphere = geo.boundingSphere;
  const uniP = { ...uni, uRevela: { value: 1 }, uOnda: { value: 0 } };
  const pontosP = new THREE.Points(geoP, new THREE.ShaderMaterial({ vertexShader: VERT, fragmentShader: FRAG, uniforms: uniP, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending }));
  grupo.add(pontosP);

  // ---------- estado ----------
  let modo = "rede", zoom = 1, largura = 1, altura = 1;
  const cam = { pos: new THREE.Vector3(0, 0, 8), alvo: new THREE.Vector3() };
  const camAlvo = { pos: new THREE.Vector3(), alvo: new THREE.Vector3() };
  let opLinhas = 0, giroAlvo = 1, giro = 0;
  const mouse = { x: 0, y: 0, sx: 0, sy: 0, mexeu: false, px: -1e9, py: -1e9 };
  let destacado = -1, rodando = false, ultimo = performance.now(), tempo = 0;
  let intro = calmo ? 1 : 0, introResolve = null;
  const medidas = [];

  function tamanho() {
    largura = window.innerWidth; altura = window.innerHeight;
    renderer.setSize(largura, altura, false);
    camera.aspect = largura / altura; camera.updateProjectionMatrix();
    uni.uEscala.value = altura * dpr * 0.5 / Math.tan((camera.fov * Math.PI) / 360) / 100;
    uniP.uEscala.value = uni.uEscala.value;
  }

  function mirar() {
    const c = CAMERAS[modo];
    let [x, y, z] = c.pos;
    const aspecto = largura / altura;
    const afasta = aspecto < 1 ? Math.min(2.1, 0.95 / aspecto) : 1;
    if (modo === "global") { const d = 6 + (zoom ** 1.6) * 250; x = 0; y = 0; z = 62 + d; }
    camAlvo.pos.set(x * afasta, y * afasta, z * afasta);
    camAlvo.alvo.set(...c.alvo);
    giroAlvo = c.giro;
  }

  function spawnPacote(p, origem) {
    const a = origem ?? Math.floor(Math.random() * N);
    const viz = adj[a]; if (!viz.length) return;
    p.a = a; p.b = viz[Math.floor(Math.random() * viz.length)]; p.t = 0; p.v = 0.006 + Math.random() * 0.012;
  }

  function atualizarDestaque() {
    if (!mouse.mexeu || q.movel) return;
    mouse.mexeu = false;
    const v = new THREE.Vector3(), m = pontos.matrixWorld;
    let melhor = -1, dMelhor = 38 * 38;
    const passo = N > 900 ? 2 : 1;
    for (let i = 0; i < N; i += passo) {
      v.set(pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]).applyMatrix4(m).project(camera);
      if (v.z > 1) continue;
      const sx = (v.x * 0.5 + 0.5) * largura, sy = (-v.y * 0.5 + 0.5) * altura;
      const d = (sx - mouse.px) ** 2 + (sy - mouse.py) ** 2;
      if (d < dMelhor) { dMelhor = d; melhor = i; }
    }
    if (melhor !== destacado) {
      destacado = melhor;
      if (melhor >= 0) pacotes.slice(0, 4).forEach((p) => spawnPacote(p, melhor));
    }
  }

  function quadro(agora) {
    if (!rodando) return;
    // teto de quadros por segundo: o resto do navegador precisa de folga
    if (agora - ultimo < intervaloQuadro - 1) { requestAnimationFrame(quadro); return; }
    const bruto = (agora - ultimo) / 1000; ultimo = agora;
    const dt = Math.min(0.1, bruto); tempo += dt;
    medir(bruto);
    // suavizações medidas em tempo, não em quadros: iguais em máquina rápida e lenta
    const suave = (taxa) => 1 - Math.exp(-dt * taxa);

    // introdução: um ponto, a câmera se afasta, a rede aparece, depois vira o A
    if (intro < 1) {
      intro = Math.min(1, intro + dt / 4.2);
      const e = 1 - Math.pow(1 - intro, 3);
      uni.uRevela.value = Math.max(0.0015, Math.pow(e, 2.2));
      cam.pos.set(0, e * 10, 7 + e * 160);
      cam.alvo.set(0, 0, 0);
      if (intro >= 1) { definirModo(modoPedido); if (introResolve) introResolve(); }
    } else {
      cam.pos.lerp(camAlvo.pos, suave(1.7));
      cam.alvo.lerp(camAlvo.alvo, suave(2.1));
    }

    // parallax do mouse
    const km = suave(2.4); mouse.sx += (mouse.x - mouse.sx) * km; mouse.sy += (mouse.y - mouse.sy) * km;
    camera.position.set(cam.pos.x + mouse.sx * 9, cam.pos.y - mouse.sy * 6, cam.pos.z);
    camera.lookAt(cam.alvo);

    // forma
    if (intro >= 1) {
      const alvo = F[modo], k = suave(2.1);
      for (let i = 0; i < N * 3; i++) pos[i] += (alvo[i] - pos[i]) * k;
      atrPos.needsUpdate = true;
    }
    giro += (giroAlvo - giro) * suave(1.2);
    if (modo === "logo" || modo === "grade") grupo.rotation.y += (0 - grupo.rotation.y) * suave(1.8);
    else grupo.rotation.y += 0.054 * giro * dt;

    // linhas
    const alvoL = intro < 1 ? 0.2 * intro : CAMERAS[modo].linhas;
    opLinhas += (alvoL - opLinhas) * suave(2.4); matL.opacity = opLinhas;

    // brilho: decai, e o nó destacado com vizinhos acende
    atualizarDestaque();
    const apaga = Math.pow(0.93, dt * 60); for (let i = 0; i < N; i++) brilho[i] *= apaga;
    if (destacado >= 0) { brilho[destacado] = 1.4; adj[destacado].forEach((j) => { brilho[j] = Math.max(brilho[j], 0.7); }); }
    atrBrilho.needsUpdate = true;
    let nD = 0;
    if (destacado >= 0) {
      for (const j of adj[destacado].slice(0, MAXD)) {
        posD.set([pos[destacado * 3], pos[destacado * 3 + 1], pos[destacado * 3 + 2], pos[j * 3], pos[j * 3 + 1], pos[j * 3 + 2]], nD * 6); nD++;
      }
      geoD.attributes.position.needsUpdate = true;
    }
    geoD.setDrawRange(0, nD * 2);

    // pacotes só onde há ligações visíveis
    const comPacotes = opLinhas > 0.06;
    for (let k = 0; k < pacotes.length; k++) {
      const p = pacotes[k];
      if (p.t >= 1) { if (comPacotes && Math.random() < 3 * dt) spawnPacote(p); else { posP.set([0, 0, -9999], k * 3); continue; } }
      p.t += p.v * dt * 60;
      if (p.t >= 1) { brilho[p.b] = Math.max(brilho[p.b], 0.9); if (Math.random() < 0.6) spawnPacote(p, p.b); continue; }
      const a = p.a * 3, b = p.b * 3;
      posP.set([pos[a] + (pos[b] - pos[a]) * p.t, pos[a + 1] + (pos[b + 1] - pos[a + 1]) * p.t, pos[a + 2] + (pos[b + 2] - pos[a + 2]) * p.t], k * 3);
    }
    if (P) { geoP.attributes.position.needsUpdate = true; geoP.setDrawRange(0, pacotes.length); }
    pontosP.visible = comPacotes;

    animarHud(dt, tempo);
    uni.uTempo.value = tempo; uniP.uTempo.value = tempo;
    uni.uOnda.value += ((modo === "cadeia" ? 1 : 0) - uni.uOnda.value) * suave(3);
    if (intro >= 1) {
      const revelaAlvo = modo === "global" ? Math.max(0.0012, Math.pow(N, zoom - 1) + 0.0005) : 1;
      uni.uRevela.value += (revelaAlvo - uni.uRevela.value) * suave(5);
      uni.uQualOrdem.value += ((modo === "global" ? 1 : 0) - uni.uQualOrdem.value) * suave(6);
    }
    renderer.render(cena, camera);
    requestAnimationFrame(quadro);
  }

  // rebaixa a qualidade sozinho se os quadros ficarem lentos
  let rebaixado = false;
  function medir(dt) {
    if (rebaixado) return;
    medidas.push(dt);
    if (medidas.length < 45) return;
    const media = medidas.reduce((a, b) => a + b, 0) / medidas.length;
    medidas.length = 0;
    if (media <= 0.03) { if (intro >= 1) rebaixado = true; return; }
    if (dpr > 1) { dpr = 1; renderer.setPixelRatio(1); tamanho(); return; }
    // ainda lento: metade dos nós, menos pacotes, sem suavização de bordas
    rebaixado = true;
    pacotes.length = Math.floor(pacotes.length / 3);
    geo.setDrawRange(0, Math.floor(N * 0.5));
    geoL.setDrawRange(0, Math.floor(indices.length * 0.35));
  }

  let modoPedido = "logo";
  function definirModo(m) {
    if (!MODOS.includes(m)) return;
    modoPedido = m;
    if (intro < 1) return;
    modo = m; mirar();
    if (calmo) { pos.set(F[modo]); atrPos.needsUpdate = true; cam.pos.copy(camAlvo.pos); cam.alvo.copy(camAlvo.alvo); opLinhas = CAMERAS[modo].linhas; matL.opacity = opLinhas; desenharParado(); }
  }
  function definirZoom(p) { zoom = Math.max(0, Math.min(1, p)); if (modo === "global") mirar(); if (calmo) desenharParado(); }

  function desenharParado() {
    uni.uRevela.value = modo === "global" ? Math.max(0.0012, Math.pow(N, zoom - 1)) : 1;
    uni.uQualOrdem.value = modo === "global" ? 1 : 0;
    camera.position.copy(camAlvo.pos); camera.lookAt(camAlvo.alvo);
    renderer.render(cena, camera);
  }

  const ligar = () => { if (!rodando && !calmo) { rodando = true; ultimo = performance.now(); requestAnimationFrame(quadro); } };
  const desligar = () => { rodando = false; };
  window.addEventListener("resize", () => { tamanho(); mirar(); if (calmo) desenharParado(); });
  window.addEventListener("pointermove", (e) => {
    mouse.x = (e.clientX / largura) * 2 - 1; mouse.y = (e.clientY / altura) * 2 - 1;
    mouse.px = e.clientX; mouse.py = e.clientY; mouse.mexeu = true;
  }, { passive: true });
  document.addEventListener("visibilitychange", () => (document.hidden ? desligar() : ligar()));

  // ------------------------------------------------------------------------
  // Assistente: um rosto de máquina, feito de arames. Nenhum rosto humano
  // inventado. A boca abre com o nível da voz.
  // ------------------------------------------------------------------------
  let cabeca = null, boca = 0, bocaAlvo = 0, bocaMalha = null;
  const aneis = [];

  function criarAssistente() {
    if (cabeca) return cabeca;
    cabeca = new THREE.Group();
    cabeca.position.set(-1.42, -0.48, -3.2);

    const prata = new THREE.LineBasicMaterial({ color: 0xc9ccd1, transparent: true, opacity: 0.6 });
    const arame = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.IcosahedronGeometry(0.42, 1)), prata);
    const casco = new THREE.Mesh(new THREE.IcosahedronGeometry(0.4, 1), new THREE.MeshBasicMaterial({ color: 0x0a0b0d }));
    cabeca.add(casco, arame);

    const luz = new THREE.MeshBasicMaterial({ color: 0xfff3df });
    for (const lado of [-1, 1]) {
      const olho = new THREE.Mesh(new THREE.SphereGeometry(0.042, 8, 8), luz);
      olho.position.set(lado * 0.14, 0.08, 0.3);
      cabeca.add(olho);
    }
    bocaMalha = new THREE.Mesh(new THREE.BoxGeometry(0.26, 1, 0.03), new THREE.MeshBasicMaterial({ color: 0xf3e2c4 }));
    bocaMalha.position.set(0, -0.14, 0.3);
    bocaMalha.scale.y = 0.02;
    cabeca.add(bocaMalha);

    const quente = new THREE.MeshBasicMaterial({ color: 0xf3e2c4, transparent: true, opacity: 0.45 });
    for (let i = 0; i < 2; i++) {
      const anel = new THREE.Mesh(new THREE.TorusGeometry(0.5 + i * 0.08, 0.004, 4, 48), quente);
      anel.rotation.set(i ? 1.1 : 0.35, i ? 0.4 : 0.9, 0);
      cabeca.add(anel);
      aneis.push(anel);
    }
    hud.add(cabeca);
    return cabeca;
  }

  // ------------------------------------------------------------------------
  // Objetos de explicação: peças 3D presas ao lado direito da tela, uma por
  // cena. São poucas malhas de propósito, para não pesar.
  // ------------------------------------------------------------------------
  const OBJETOS = {};
  let objetoAtual = null;

  function baseDoObjeto() {
    const g = new THREE.Group();
    g.position.set(1.42, -0.05, -3.2);
    g.visible = false;
    hud.add(g);
    return g;
  }
  function arames(geometria, cor, opacidade = 0.85) {
    return new THREE.LineSegments(
      new THREE.EdgesGeometry(geometria),
      new THREE.LineBasicMaterial({ color: cor, transparent: true, opacity: opacidade }),
    );
  }

  function criarObjeto(nome) {
    if (OBJETOS[nome]) return OBJETOS[nome];
    const g = baseDoObjeto();
    let atualizar = () => {};

    if (nome === "moeda") {
      const disco = new THREE.Mesh(new THREE.CylinderGeometry(0.42, 0.42, 0.06, 40), new THREE.MeshBasicMaterial({ color: 0x15171a }));
      disco.rotation.x = Math.PI / 2;
      const aro = new THREE.Mesh(new THREE.TorusGeometry(0.42, 0.012, 6, 48), new THREE.MeshBasicMaterial({ color: 0xc9ccd1 }));
      const barra = (x, y, ang, comp) => {
        const m = new THREE.Mesh(new THREE.BoxGeometry(0.045, comp, 0.02), new THREE.MeshBasicMaterial({ color: 0xeceae6 }));
        m.position.set(x, y, 0.045);
        m.rotation.z = ang;
        return m;
      };
      g.add(disco, aro, barra(-0.1, 0, 0.3, 0.44), barra(0.1, 0, -0.3, 0.44), barra(0, -0.12, -0.32, 0.28));
      atualizar = (dt) => { g.rotation.y += dt * 0.7; };
    } else if (nome === "blocos") {
      const cubos = [];
      for (let i = 0; i < 4; i++) {
        const c = arames(new THREE.BoxGeometry(0.26, 0.26, 0.26), 0xc9ccd1, 0.8);
        c.position.set(-0.42 + i * 0.28, 0, 0);
        g.add(c);
        cubos.push(c);
      }
      const elo = new THREE.Mesh(new THREE.SphereGeometry(0.035, 8, 8), new THREE.MeshBasicMaterial({ color: 0xfff3df }));
      g.add(elo);
      let t = 0;
      atualizar = (dt) => {
        t = (t + dt * 0.35) % 1;
        elo.position.set(-0.42 + t * 0.84, 0, 0);
        cubos.forEach((c, i) => { c.rotation.y += dt * (0.2 + i * 0.05); });
        g.rotation.y = Math.sin(t * 6.283) * 0.25;
      };
    } else if (nome === "tarefa") {
      const origem = new THREE.Mesh(new THREE.TorusGeometry(0.16, 0.008, 6, 32), new THREE.MeshBasicMaterial({ color: 0xc9ccd1 }));
      origem.position.set(-0.45, 0, 0);
      const executor = new THREE.Mesh(new THREE.TorusGeometry(0.2, 0.01, 6, 32), new THREE.MeshBasicMaterial({ color: 0xc9ccd1 }));
      const carga = arames(new THREE.BoxGeometry(0.14, 0.14, 0.14), 0xfff3df, 1);
      const aceito = new THREE.Mesh(new THREE.TorusGeometry(0.28, 0.012, 6, 40), new THREE.MeshBasicMaterial({ color: 0x9fd6b4, transparent: true, opacity: 0 }));
      g.add(origem, executor, carga, aceito);
      let t = 0;
      atualizar = (dt) => {
        t = (t + dt * 0.28) % 1;
        const ida = Math.min(1, t / 0.5);
        carga.position.set(-0.45 + ida * 0.45, Math.sin(ida * Math.PI) * 0.12, 0);
        carga.rotation.x += dt * 1.2;
        carga.rotation.y += dt * 0.9;
        executor.rotation.z += dt * 0.6;
        const surge = t > 0.62 ? Math.min(0.9, (t - 0.62) * 6) : 0;
        aceito.material.opacity = surge * (1 - Math.max(0, t - 0.9) * 10);
        aceito.scale.setScalar(t > 0.62 ? 1 + (t - 0.62) * 0.3 : 1);
      };
    } else if (nome === "pacote") {
      const pontos = [[-0.42, -0.12], [0, 0.16], [0.42, -0.12]];
      for (const [x, y] of pontos) {
        const aparelho = arames(new THREE.BoxGeometry(0.16, 0.24, 0.02), 0xc9ccd1, 0.75);
        aparelho.position.set(x, y, 0);
        g.add(aparelho);
      }
      const pacote = new THREE.Mesh(new THREE.SphereGeometry(0.05, 10, 10), new THREE.MeshBasicMaterial({ color: 0xfff3df }));
      g.add(pacote);
      let t = 0;
      atualizar = (dt) => {
        t = (t + dt * 0.3) % 1;
        const i = t < 0.5 ? 0 : 1;
        const f = t < 0.5 ? t * 2 : t * 2 - 1;
        const [x1, y1] = pontos[i], [x2, y2] = pontos[i + 1];
        pacote.position.set(x1 + (x2 - x1) * f, y1 + (y2 - y1) * f + Math.sin(f * Math.PI) * 0.18, 0.05);
      };
    }
    OBJETOS[nome] = { grupo: g, atualizar };
    return OBJETOS[nome];
  }

  function objeto(nome) {
    if (objetoAtual) objetoAtual.grupo.visible = false;
    objetoAtual = nome ? criarObjeto(nome) : null;
    if (objetoAtual) objetoAtual.grupo.visible = true;
  }

  function assistente(mostrar) {
    if (!mostrar) {
      if (cabeca) cabeca.visible = false;
      objeto(null);
      return;
    }
    criarAssistente().visible = true;
  }

  function animarHud(dt, t) {
    if (cabeca && cabeca.visible) {
      boca += (bocaAlvo - boca) * (1 - Math.exp(-dt * 18));
      bocaMalha.scale.y = 0.02 + boca * 0.9;
      cabeca.rotation.y = Math.sin(t * 0.6) * 0.18;
      cabeca.rotation.x = Math.sin(t * 0.4) * 0.06;
      cabeca.position.y = -0.48 + Math.sin(t * 0.8) * 0.015;
      aneis.forEach((anel, i) => { anel.rotation.z += dt * (0.4 + i * 0.25); });
    }
    if (objetoAtual) objetoAtual.atualizar(dt, t);
  }

  tamanho();
  if (calmo) { intro = 1; modo = "logo"; mirar(); pos.set(F.logo); atrPos.needsUpdate = true; desenharParado(); }
  else { cam.pos.set(0, 0, 7); ligar(); }

  return {
    nivel: q.nivel,
    definirModo, definirZoom,
    assistente, objeto,
    // O video desenha o quadro ampliado a partir do centro. A assistente e os
    // objetos precisam encolher na mesma medida para nao sairem da tela: uma
    // escala uniforme em torno da camera nao muda nem o tamanho aparente nem a
    // posicao na tela depois da ampliacao.
    hudEscala: (v) => hud.scale.setScalar(v > 0 ? 1 / v : 1),
    definirBoca: (v) => { bocaAlvo = Math.max(0, Math.min(1, v)); },
    introducao: () => (intro >= 1 ? Promise.resolve() : new Promise((res) => { introResolve = res; })),
  };
}
