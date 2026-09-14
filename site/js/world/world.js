// Auron World: uma única rede de nós em Three.js que muda de forma conforme o capítulo.
// Um só objeto de pontos (sem um componente por nó), posições interpoladas na CPU,
// brilho e tamanho calculados no shader. Linhas compartilham o buffer de posições.
import * as THREE from "../../vendor/three.module.min.js";
import { criarHeroi } from "./heroi.js";

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
  // Tom de cinema para os objetos do vídeo (metal e luz). Os pontos e as linhas
  // do mundo ficam fora disso, para o site continuar com a mesma cara.
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  let dpr = Math.min(window.devicePixelRatio || 1, perfil.dpr);
  renderer.setPixelRatio(dpr);

  const cena = new THREE.Scene();
  cena.fog = new THREE.FogExp2(0x030303, 0.0023);
  const camera = new THREE.PerspectiveCamera(48, 1, 0.1, 3000);
  const grupo = new THREE.Group();
  cena.add(grupo);
  // O que fica preso à câmera: os objetos de explicação do vídeo. Assim eles
  // ficam sempre no mesmo lugar da tela, em qualquer modo do mundo.
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
  const matL = new THREE.LineBasicMaterial({ color: 0x8f959e, transparent: true, opacity: 0, depthWrite: false, blending: THREE.AdditiveBlending, toneMapped: false });
  grupo.add(new THREE.LineSegments(geoL, matL));

  // destaque: ligações do nó sob o cursor
  const MAXD = 16;
  const posD = new Float32Array(MAXD * 6);
  const geoD = new THREE.BufferGeometry();
  geoD.setAttribute("position", new THREE.BufferAttribute(posD, 3).setUsage(THREE.DynamicDrawUsage));
  geoD.setDrawRange(0, 0);
  geoD.boundingSphere = geo.boundingSphere;
  const matD = new THREE.LineBasicMaterial({ color: 0xf3e2c4, transparent: true, opacity: 0.55, depthWrite: false, blending: THREE.AdditiveBlending, toneMapped: false });
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
  // Durante o vídeo, só os objetos de explicação: a rede de pontos, as ligações
  // e os pacotes somem, e as contas por nó (milhares por quadro) param.
  let soObjetos = false;
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

    if (soObjetos) {
      animarHud(dt, tempo);
      uni.uTempo.value = tempo;
      renderer.render(cena, camera);
      requestAnimationFrame(quadro);
      return;
    }

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
    if (!calmo) heroi.desenhar(agora);
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
  function somenteObjetos(ativo) {
    soObjetos = !!ativo;
    grupo.visible = !soObjetos;
    if (soObjetos) {
      // câmera parada e centrada: os objetos ficam presos a ela
      camera.position.set(0, 0, 8);
      camera.lookAt(0, 0, 0);
    }
    if (calmo) desenharParado();
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
  // Objetos de explicação: uma peça 3D por assunto, grande, no centro da tela.
  // Cada uma mostra a lógica do que está sendo narrado, não é enfeite.
  //
  // Visual de vitrine: luz de estúdio (principal quente, contraluz fria e
  // reflexos de ambiente gerados uma vez), metal cromado, grafite, ouro e peças
  // que emitem luz com halo. Tudo sobre um pedestal com aro de luz e sombra
  // suave. Sem pós-processamento: o brilho vem de sprites aditivos, que custam
  // quase nada numa máquina fraca.
  //
  // Ficam presas à câmera, então não dependem do modo do mundo.
  // ------------------------------------------------------------------------
  const PRATA = 0xd3d7de, QUENTE = 0xffbf73, OURO = 0xf2b865, FRIO = 0x9cc4ff;
  const ACEITO = 0x6fe3a5, RECUSADO = 0xff5e4d, APAGADO = 0x3a3d44;
  const OBJETOS = {};
  let objetoAtual = null, estudioPronto = false, tempoHud = 0;
  const leve = q.nivel === "LOW";
  // Tamanho da vitrine: cabe entre o título (em cima) e a legenda (embaixo).
  const ESCALA_PALCO = 0.68;

  const suave = (x) => x * x * (3 - 2 * x);
  const saltito = (x) => { const c1 = 1.4, c3 = c1 + 1; return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2); };

  // ---------- estúdio: ambiente para reflexos e luzes presas à câmera ----------
  function prepararEstudio() {
    if (estudioPronto) return;
    estudioPronto = true;
    const sala = new THREE.Scene();
    const cubo = new THREE.BoxGeometry(1, 1, 1);
    const painel = (cor, forca, p, s) => {
      const m = new THREE.Mesh(cubo, new THREE.MeshBasicMaterial({ color: new THREE.Color(cor).multiplyScalar(forca) }));
      m.position.set(...p); m.scale.set(...s); sala.add(m);
    };
    painel(0xfff0dc, 5, [0, 7, 1], [9, 0.2, 7]);      // caixa de luz do teto, quente
    painel(0xd8e6ff, 2.4, [-8, 1.5, 3], [0.2, 6, 7]); // rebatedor frio à esquerda
    painel(0xffd29a, 3.2, [8, 0, -3], [0.2, 5, 5]);   // recorte quente à direita
    painel(0x1a1b1f, 1, [0, -6, 0], [30, 0.2, 30]);   // chão escuro
    const gerador = new THREE.PMREMGenerator(renderer);
    cena.environment = gerador.fromScene(sala, 0.04).texture;
    gerador.dispose();

    hud.add(new THREE.HemisphereLight(0xfff4e6, 0x0a0a0c, 0.55));
    const principal = new THREE.DirectionalLight(0xffe7c7, 2.4);
    principal.position.set(2.5, 4, 2);
    const contraluz = new THREE.DirectionalLight(0xa9c8ff, 1.6);
    contraluz.position.set(-3, 1.5, -4);
    hud.add(principal, contraluz);
  }

  // ---------- materiais ----------
  const cromo = (cor = PRATA, aspereza = 0.2) => new THREE.MeshStandardMaterial({ color: cor, metalness: 1, roughness: aspereza });
  const ouro = () => new THREE.MeshStandardMaterial({ color: OURO, metalness: 1, roughness: 0.18, emissive: 0x3a1f00, emissiveIntensity: 0.6 });
  const grafite = (cor = 0x2b2d33) => new THREE.MeshStandardMaterial({ color: cor, metalness: 0.55, roughness: 0.38 });
  const luz = (cor, forca = 2.2) => new THREE.MeshStandardMaterial({ color: 0x050505, emissive: cor, emissiveIntensity: forca, roughness: 0.35 });

  // ---------- geometrias ----------
  const cacheGeo = new Map();
  /** Caixa com cantos arredondados (a mesma medida final de uma BoxGeometry). */
  function caixa(w, h, d, raio = 0.03) {
    const chave = `${w}|${h}|${d}|${raio}`;
    if (cacheGeo.has(chave)) return cacheGeo.get(chave);
    const chanfro = Math.min(raio, d / 3, w / 4, h / 4);
    const iw = w - chanfro * 2, ih = h - chanfro * 2, r = Math.max(0.002, Math.min(raio - chanfro, iw / 2, ih / 2));
    const x = -iw / 2, y = -ih / 2, s = new THREE.Shape();
    s.moveTo(x + r, y); s.lineTo(x + iw - r, y); s.quadraticCurveTo(x + iw, y, x + iw, y + r);
    s.lineTo(x + iw, y + ih - r); s.quadraticCurveTo(x + iw, y + ih, x + iw - r, y + ih);
    s.lineTo(x + r, y + ih); s.quadraticCurveTo(x, y + ih, x, y + ih - r);
    s.lineTo(x, y + r); s.quadraticCurveTo(x, y, x + r, y);
    const geo = new THREE.ExtrudeGeometry(s, {
      depth: Math.max(0.001, d - chanfro * 2), bevelEnabled: true, bevelThickness: chanfro,
      bevelSize: chanfro, bevelSegments: leve ? 1 : 3, curveSegments: leve ? 2 : 5,
    });
    geo.center();
    cacheGeo.set(chave, geo);
    return geo;
  }
  const esfera = (raio, detalhe = leve ? 12 : 24) => new THREE.SphereGeometry(raio, detalhe, Math.round(detalhe * 0.75));

  /** Tubo reto entre dois pontos: substitui a linha de 1 pixel. */
  function tubo(a, b, raio, material) {
    const dir = new THREE.Vector3().subVectors(b, a);
    const m = new THREE.Mesh(new THREE.CylinderGeometry(raio, raio, dir.length(), 12, 1, true), material);
    m.position.copy(a).addScaledVector(dir, 0.5);
    m.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir.normalize());
    return m;
  }

  // ---------- brilho, sombra e partículas ----------
  const texturas = {};
  function textura(tipo) {
    if (texturas[tipo]) return texturas[tipo];
    const c = document.createElement("canvas");
    c.width = c.height = 128;
    const x = c.getContext("2d");
    const gr = x.createRadialGradient(64, 64, 0, 64, 64, 64);
    if (tipo === "halo") {
      gr.addColorStop(0, "rgba(255,255,255,1)");
      gr.addColorStop(0.18, "rgba(255,255,255,0.55)");
      gr.addColorStop(0.5, "rgba(255,255,255,0.12)");
      gr.addColorStop(1, "rgba(255,255,255,0)");
    } else {
      gr.addColorStop(0, "rgba(0,0,0,0.85)");
      gr.addColorStop(0.6, "rgba(0,0,0,0.35)");
      gr.addColorStop(1, "rgba(0,0,0,0)");
    }
    x.fillStyle = gr;
    x.fillRect(0, 0, 128, 128);
    texturas[tipo] = new THREE.CanvasTexture(c);
    texturas[tipo].colorSpace = THREE.SRGBColorSpace;
    return texturas[tipo];
  }
  function halo(cor, tamanho = 0.4, opacidade = 0.85) {
    const s = new THREE.Sprite(new THREE.SpriteMaterial({
      map: textura("halo"), color: cor, transparent: true, opacity: opacidade,
      blending: THREE.AdditiveBlending, depthWrite: false, toneMapped: false,
    }));
    s.scale.setScalar(tamanho);
    return s;
  }
  /** Uma esfera que emite luz, com o halo em volta. */
  function orbe(cor, raio = 0.05, forca = 3, tamanhoHalo = 0.45) {
    const grupo = new THREE.Group();
    const corpo = new THREE.Mesh(esfera(raio), luz(cor, forca));
    const aura = halo(cor, tamanhoHalo);
    grupo.add(corpo, aura);
    grupo.userData = { corpo, aura };
    return grupo;
  }

  function palco() {
    prepararEstudio();
    const g = new THREE.Group();
    g.position.set(0, -0.13, -3.6);
    g.rotation.x = 0.2;             // câmera um pouco de cima: dá volume
    g.visible = false;
    hud.add(g);

    // pedestal escuro e acetinado, com aro de luz quente
    const base = new THREE.Mesh(new THREE.CylinderGeometry(1.28, 1.34, 0.05, leve ? 40 : 80), grafite(0x121316));
    base.scale.z = 0.5;
    base.position.y = -0.8;
    const aro = new THREE.Mesh(new THREE.TorusGeometry(1.31, 0.007, 8, leve ? 60 : 120), luz(QUENTE, 1.8));
    aro.scale.set(1, 0.5, 1);
    aro.rotation.x = Math.PI / 2;
    aro.position.y = -0.774;
    const sombra = new THREE.Mesh(new THREE.PlaneGeometry(2.4, 1.1), new THREE.MeshBasicMaterial({
      map: textura("sombra"), transparent: true, depthWrite: false, toneMapped: false,
    }));
    sombra.rotation.x = -Math.PI / 2;
    sombra.position.y = -0.772;
    // recorte de luz quente atrás, como um spot no fundo do estúdio
    const fundo = halo(QUENTE, 4.2, 0.1);
    fundo.position.set(0, 0.25, -1.6);
    g.add(fundo, base, aro, sombra);

    // partículas discretas no ar
    let poeira = null;
    if (!leve && !calmo) {
      const n = 70, p = new Float32Array(n * 3);
      for (let i = 0; i < n; i++) p.set([(Math.random() - 0.5) * 2.8, Math.random() * 1.8 - 0.8, (Math.random() - 0.5) * 1.4], i * 3);
      const geo = new THREE.BufferGeometry();
      geo.setAttribute("position", new THREE.BufferAttribute(p, 3));
      poeira = new THREE.Points(geo, new THREE.PointsMaterial({
        map: textura("halo"), color: 0xffe2b8, size: 0.035, transparent: true, opacity: 0.55,
        blending: THREE.AdditiveBlending, depthWrite: false, toneMapped: false,
      }));
      g.add(poeira);
    }

    const peca = new THREE.Group();
    g.add(peca);
    return { g, peca, poeira, aro };
  }

  function criarObjeto(nome) {
    if (OBJETOS[nome]) return OBJETOS[nome];
    const { g, peca, poeira, aro } = palco();
    let atualizar = () => {};

    // --- três computadores iguais, conversando entre si -------------------
    if (nome === "nos") {
      const pontos = [new THREE.Vector3(-0.72, -0.3, 0), new THREE.Vector3(0.72, -0.3, 0), new THREE.Vector3(0, 0.48, 0)];
      const maquinas = pontos.map((v) => {
        const m = new THREE.Group();
        m.add(new THREE.Mesh(caixa(0.36, 0.42, 0.32, 0.06), cromo(0xc8ccd3, 0.24)));
        const faixas = [];
        for (let k = 0; k < 3; k++) {
          const f = new THREE.Mesh(caixa(0.24, 0.028, 0.02, 0.012), luz(QUENTE, 0.9));
          f.position.set(0, 0.11 - k * 0.075, 0.165);
          m.add(f);
          faixas.push(f);
        }
        const led = new THREE.Mesh(esfera(0.016), luz(ACEITO, 3));
        led.position.set(0.12, -0.15, 0.17);
        m.add(led);
        m.position.copy(v);
        peca.add(m);
        return { m, faixas, acende: 0 };
      });
      for (let i = 0; i < 3; i++) peca.add(tubo(pontos[i], pontos[(i + 1) % 3], 0.009, luz(QUENTE, 0.55)));
      const pulso = orbe(QUENTE, 0.045, 4, 0.5);
      peca.add(pulso);
      let t = 0, ultimo = 0;
      atualizar = (dt, tt) => {
        t = (t + dt * 0.3) % 3;
        const i = Math.floor(t), f = suave(t - i);
        pulso.position.lerpVectors(pontos[i], pontos[(i + 1) % 3], f);
        if (i !== ultimo) { maquinas[i].acende = 1; ultimo = i; }
        maquinas.forEach((mq, k) => {
          mq.acende = Math.max(0, mq.acende - dt * 1.4);
          mq.faixas.forEach((fx) => { fx.material.emissiveIntensity = 0.9 + mq.acende * 3; });
          mq.m.position.y = pontos[k].y + Math.sin(tt * 1.3 + k * 2) * 0.02;
          mq.m.rotation.y = Math.sin(tt * 0.6 + k) * 0.25;
        });
        pulso.userData.aura.scale.setScalar(0.45 + Math.sin(tt * 8) * 0.05);
        peca.rotation.y = Math.sin(tt * 0.35) * 0.3;
      };

    // --- cálculo jogado fora × cálculo aproveitado ------------------------
    } else if (nome === "calculo") {
      const perdidos = [], uteis = [];
      for (let i = 0; i < 7; i++) {
        const a = new THREE.Mesh(caixa(0.14, 0.14, 0.14, 0.03), grafite(0x3b3e45));
        a.material.transparent = true;
        a.position.set(-0.6, 0, 0);
        peca.add(a); perdidos.push(a);
        const b = new THREE.Mesh(caixa(0.14, 0.14, 0.14, 0.03), ouro());
        b.position.set(0.6, 0, 0);
        peca.add(b); uteis.push(b);
      }
      // à esquerda, um buraco escuro que engole o trabalho
      const buraco = new THREE.Mesh(new THREE.CircleGeometry(0.3, 48), new THREE.MeshBasicMaterial({ color: 0x000000 }));
      buraco.rotation.x = -Math.PI / 2;
      buraco.position.set(-0.6, -0.76, 0);
      const bordaBuraco = new THREE.Mesh(new THREE.TorusGeometry(0.3, 0.01, 8, 64), grafite(0x55585f));
      bordaBuraco.rotation.x = Math.PI / 2;
      bordaBuraco.position.copy(buraco.position);
      // à direita, um anel verde que colhe o resultado
      const colheita = new THREE.Mesh(new THREE.TorusGeometry(0.3, 0.018, 12, 64), luz(ACEITO, 2.2));
      colheita.rotation.x = Math.PI / 2;
      colheita.position.set(0.6, -0.74, 0);
      const auraColheita = halo(ACEITO, 1.0, 0.35);
      auraColheita.position.set(0.6, -0.68, 0);
      peca.add(buraco, bordaBuraco, colheita, auraColheita);
      let t = 0;
      atualizar = (dt, tt) => {
        t += dt * 0.5;
        perdidos.forEach((c, i) => {
          const f = ((t * 0.5 + i * 0.2) % 1.4) / 1.4;
          c.position.y = 0.72 - f * 1.45;
          c.material.opacity = Math.max(0, 1 - Math.pow(f, 3));
          c.scale.setScalar(1 - Math.pow(f, 4) * 0.8);
          c.rotation.z = f * 0.6;
        });
        let colheu = 0;
        uteis.forEach((c, i) => {
          const f = ((t * 0.5 + i * 0.2) % 1.4) / 1.4;
          c.position.y = 0.72 - f * 1.45;
          c.rotation.x += dt * 1.2; c.rotation.y += dt * 0.9;
          if (f > 0.92) colheu = Math.max(colheu, (f - 0.92) / 0.08);
        });
        colheita.material.emissiveIntensity = 2.2 + colheu * 3;
        auraColheita.material.opacity = 0.3 + colheu * 0.5;
        colheita.scale.setScalar(1 + Math.sin(tt * 2) * 0.03 + colheu * 0.06);
        peca.rotation.y = Math.sin(tt * 0.3) * 0.2;
      };

    // --- onde queremos chegar: genética, remédio e IA ----------------------
    } else if (nome === "ciencia") {
      // hélice de DNA: dois fios cromados e degraus que brilham
      const helice = new THREE.Group();
      helice.position.set(-0.82, 0.02, 0);
      const bolaPrata = cromo(PRATA, 0.18), bolaOuro = ouro();
      for (let i = 0; i < 18; i++) {
        const a = i * 0.6, y = -0.55 + i * 0.066;
        const p1 = new THREE.Vector3(Math.cos(a) * 0.17, y, Math.sin(a) * 0.17);
        const p2 = p1.clone().multiplyScalar(-1); p2.y = y;
        const b1 = new THREE.Mesh(esfera(0.03), bolaPrata); b1.position.copy(p1);
        const b2 = new THREE.Mesh(esfera(0.03), bolaOuro); b2.position.copy(p2);
        helice.add(b1, b2, tubo(p1, p2, 0.006, luz(i % 2 ? QUENTE : FRIO, 1.2)));
      }
      // molécula candidata a remédio
      const molecula = new THREE.Group();
      molecula.position.set(0, 0.02, 0);
      molecula.add(new THREE.Mesh(esfera(0.1), cromo(PRATA, 0.12)));
      const vidroAtomo = new THREE.MeshStandardMaterial({ color: QUENTE, metalness: 0.2, roughness: 0.12, emissive: 0x552a00, emissiveIntensity: 0.8 });
      for (let i = 0; i < 5; i++) {
        const a = (i / 5) * Math.PI * 2;
        const v = new THREE.Vector3(Math.cos(a) * 0.32, Math.sin(a) * 0.32, Math.sin(a * 2) * 0.16);
        const atomo = new THREE.Mesh(esfera(0.055), vidroAtomo);
        atomo.position.copy(v);
        molecula.add(atomo, tubo(new THREE.Vector3(), v, 0.012, cromo(PRATA, 0.3)));
      }
      // rede neural com sinais correndo entre as camadas
      const rede = new THREE.Group();
      rede.position.set(0.82, 0.02, 0);
      const camadas = [3, 4, 2], posCamada = [], sinais = [];
      camadas.forEach((n, c) => {
        const linha = [];
        for (let i = 0; i < n; i++) {
          const v = new THREE.Vector3(-0.26 + c * 0.26, (i - (n - 1) / 2) * 0.21, 0);
          const no = new THREE.Mesh(esfera(0.04), luz(c === 1 ? QUENTE : FRIO, 1.6));
          no.position.copy(v);
          rede.add(no);
          linha.push(v);
        }
        posCamada.push(linha);
      });
      const fioRede = luz(PRATA, 0.25);
      for (let c = 0; c < posCamada.length - 1; c++) {
        posCamada[c].forEach((a) => posCamada[c + 1].forEach((b) => {
          rede.add(tubo(a, b, 0.004, fioRede));
          if (sinais.length < 8 && Math.random() < 0.5) {
            const s = orbe(QUENTE, 0.016, 4, 0.14);
            rede.add(s);
            sinais.push({ s, a, b, f: Math.random() });
          }
        }));
      }
      peca.add(helice, molecula, rede);
      atualizar = (dt, tt) => {
        helice.rotation.y += dt * 0.9;
        molecula.rotation.y += dt * 0.5;
        molecula.rotation.x = Math.sin(tt * 0.4) * 0.5;
        rede.rotation.y = Math.sin(tt * 0.45) * 0.45;
        sinais.forEach((x) => { x.f = (x.f + dt * 0.7) % 1; x.s.position.lerpVectors(x.a, x.b, x.f); });
      };

    // --- a cadeia: mexer num bloco quebra todos os seguintes ---------------
    } else if (nome === "cadeia3d") {
      const blocos = [];
      for (let i = 0; i < 4; i++) {
        const x = -0.78 + i * 0.52;
        const bloco = new THREE.Group();
        bloco.position.set(x, 0, 0);
        const corpo = new THREE.Mesh(caixa(0.36, 0.36, 0.36, 0.06), cromo(0xc9cdd4, 0.22));
        const placa = new THREE.Mesh(caixa(0.24, 0.05, 0.02, 0.02), luz(QUENTE, 2));
        placa.position.set(0, -0.08, 0.185);
        const topo = new THREE.Mesh(caixa(0.22, 0.02, 0.02, 0.01), luz(QUENTE, 0.8));
        topo.position.set(0, 0.07, 0.185);
        bloco.add(corpo, placa, topo);
        peca.add(bloco);
        const elos = [];
        if (i > 0) {
          for (let k = 0; k < 2; k++) {
            const elo = new THREE.Mesh(new THREE.TorusGeometry(0.035, 0.009, 10, 28), cromo(PRATA, 0.15));
            elo.position.set(x - 0.26 + (k - 0.5) * 0.05, 0, 0);
            elo.rotation.y = k ? Math.PI / 2 : 0;
            peca.add(elo);
            elos.push(elo);
          }
        }
        blocos.push({ bloco, corpo, placa, elos, x });
      }
      let t = 0;
      atualizar = (dt, tt) => {
        t = (t + dt * 0.15) % 1;
        // dois terços do tempo tudo certo; depois o bloco 2 é adulterado e os
        // seguintes ficam vermelhos: é o encadeamento se quebrando.
        const adulterado = t > 0.6;
        const choque = adulterado ? Math.max(0, 1 - (t - 0.6) / 0.1) : 0;
        blocos.forEach((b, i) => {
          const quebrado = adulterado && i >= 1;
          b.placa.material.emissive.setHex(quebrado ? RECUSADO : QUENTE);
          b.placa.material.emissiveIntensity = quebrado ? 2.6 + Math.sin(tt * 18) * 0.6 : 2;
          b.corpo.material.color.setHex(quebrado ? 0xd9a29b : 0xc9cdd4);
          b.bloco.position.x = b.x + (i === 1 ? (Math.random() - 0.5) * 0.03 * choque : 0);
          b.bloco.position.y = Math.sin(tt * 1.4 + i * 0.8) * 0.025;
          b.bloco.rotation.y = Math.sin(tt * 0.5 + i) * 0.18;
          b.elos.forEach((elo, k) => {
            const abre = quebrado ? Math.min(1, (t - 0.6) / 0.15) : 0;
            elo.position.y = (k ? 1 : -1) * abre * 0.05;
            elo.material.color.setHex(quebrado ? RECUSADO : PRATA);
          });
        });
        peca.rotation.y = Math.sin(tt * 0.3) * 0.16;
      };

    // --- conferência: A × B = C, testada por um vetor ----------------------
    } else if (nome === "freivalds") {
      const fazerGrade = (x, material, alturas) => {
        const grade = new THREE.Group();
        grade.position.set(x, 0.02, 0);
        const celulas = [];
        for (let l = 0; l < 4; l++) {
          for (let c = 0; c < 4; c++) {
            const cel = new THREE.Mesh(caixa(0.085, 0.085, 0.05, 0.018), material.clone());
            const alto = alturas ? 0.6 + ((l * 7 + c * 3) % 5) * 0.25 : 1;
            cel.scale.z = alto;
            cel.position.set((c - 1.5) * 0.105, (1.5 - l) * 0.105, 0.025 * alto);
            grade.add(cel);
            celulas.push(cel);
          }
        }
        peca.add(grade);
        return celulas;
      };
      fazerGrade(-0.8, cromo(PRATA, 0.25), true);
      fazerGrade(-0.2, cromo(PRATA, 0.25), true);
      const C = fazerGrade(0.5, ouro(), true);
      // sinais × e =
      const barra = (x, y, rot) => {
        const m = new THREE.Mesh(caixa(0.09, 0.016, 0.016, 0.008), luz(PRATA, 0.9));
        m.position.set(x, y, 0.02); m.rotation.z = rot; peca.add(m);
      };
      barra(-0.5, 0.02, Math.PI / 4); barra(-0.5, 0.02, -Math.PI / 4);
      barra(0.15, 0.045, 0); barra(0.15, -0.005, 0);
      // o vetor r, que testa o resultado
      const vetor = [];
      for (let i = 0; i < 4; i++) {
        const cel = new THREE.Mesh(caixa(0.085, 0.085, 0.05, 0.018), luz(ACEITO, 1.2));
        cel.position.set(0.95, (1.5 - i) * 0.105 + 0.02, 0.03);
        peca.add(cel);
        vetor.push(cel);
      }
      // um feixe de luz varre o resultado
      const feixe = new THREE.Mesh(new THREE.PlaneGeometry(0.5, 0.02), new THREE.MeshBasicMaterial({
        color: ACEITO, transparent: true, opacity: 0.6, blending: THREE.AdditiveBlending, depthWrite: false, toneMapped: false,
      }));
      feixe.position.set(0.5, 0, 0.1);
      const anel = new THREE.Mesh(new THREE.TorusGeometry(0.2, 0.014, 12, 64), luz(ACEITO, 2.4));
      anel.rotation.x = Math.PI / 2;
      anel.position.set(0.5, -0.72, 0);
      const auraAnel = halo(ACEITO, 0.8, 0);
      auraAnel.position.set(0.5, -0.66, 0);
      peca.add(feixe, anel, auraAnel);
      let t = 0;
      atualizar = (dt, tt) => {
        t = (t + dt * 0.14) % 1;
        const errado = t > 0.55;
        const cor = errado ? RECUSADO : ACEITO;
        C.forEach((cel, i) => {
          const ruim = errado && i === 6;
          cel.material.emissive.setHex(ruim ? RECUSADO : 0x3a1f00);
          cel.material.emissiveIntensity = ruim ? 2.5 : 0.6;
          cel.scale.x = cel.scale.y = ruim ? 1.2 + Math.sin(tt * 12) * 0.06 : 1;
        });
        vetor.forEach((cel, i) => { cel.material.emissiveIntensity = 0.6 + 2 * Math.max(0, Math.sin(t * 14 - i * 0.7)); });
        const varre = (t % 0.25) / 0.25;
        feixe.position.y = 0.2 - varre * 0.36;
        feixe.material.color.setHex(cor);
        feixe.material.opacity = t > 0.08 ? 0.55 * Math.sin(varre * Math.PI) : 0;
        const aparece = Math.min(1, Math.max(0, (t - 0.3) / 0.08));
        anel.material.emissive.setHex(cor);
        anel.scale.setScalar(0.6 + 0.4 * saltito(aparece));
        anel.visible = aparece > 0;
        auraAnel.material.color.setHex(cor);
        auraAnel.material.opacity = aparece * 0.45;
        anel.rotation.z += dt * 0.8;
        peca.rotation.y = Math.sin(tt * 0.3) * 0.2;
      };

    // --- o pacote achando caminho, mesmo com um aparelho fora --------------
    } else if (nome === "pacote") {
      const lugares = [new THREE.Vector3(-0.85, -0.28, 0), new THREE.Vector3(-0.28, 0.38, 0),
                       new THREE.Vector3(0.3, -0.32, 0), new THREE.Vector3(0.88, 0.26, 0)];
      const aparelhos = lugares.map((v) => {
        const a = new THREE.Group();
        a.add(new THREE.Mesh(caixa(0.2, 0.36, 0.03, 0.035), grafite(0x24262b)));
        const tela = new THREE.Mesh(caixa(0.17, 0.31, 0.006, 0.025), luz(FRIO, 1.1));
        tela.position.z = 0.017;
        a.add(tela);
        a.position.copy(v);
        peca.add(a);
        return { a, tela };
      });
      const fioCima = luz(QUENTE, 1.4), fioBaixo = luz(QUENTE, 0.3);
      peca.add(tubo(lugares[0], lugares[1], 0.008, fioCima), tubo(lugares[1], lugares[3], 0.008, fioCima),
               tubo(lugares[0], lugares[2], 0.008, fioBaixo), tubo(lugares[2], lugares[3], 0.008, fioBaixo));
      const pacote = orbe(QUENTE, 0.045, 4, 0.5);
      const rastro = [halo(QUENTE, 0.3, 0.45), halo(QUENTE, 0.22, 0.28), halo(QUENTE, 0.15, 0.15)];
      peca.add(pacote, ...rastro);
      const antigas = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()];
      let t = 0;
      atualizar = (dt, tt) => {
        t = (t + dt * 0.13) % 2;
        // na segunda volta, o aparelho de cima cai e o pacote desce pelo outro caminho
        const caiu = t > 1;
        aparelhos[1].tela.material.emissive.setHex(caiu ? RECUSADO : FRIO);
        aparelhos[1].tela.material.emissiveIntensity = caiu ? 0.25 + (Math.sin(tt * 10) > 0 ? 0.3 : 0) : 1.1;
        fioCima.emissiveIntensity = caiu ? 0.08 : 1.4;
        fioBaixo.emissiveIntensity = caiu ? 1.4 : 0.3;
        const meio = caiu ? lugares[2] : lugares[1];
        const f = suave((t % 1) * 2 > 1 ? (t % 1) * 2 - 1 : (t % 1) * 2);
        if ((t % 1) * 2 < 1) pacote.position.lerpVectors(lugares[0], meio, f);
        else pacote.position.lerpVectors(meio, lugares[3], f);
        antigas[2].lerp(antigas[1], 0.35); antigas[1].lerp(antigas[0], 0.35); antigas[0].lerp(pacote.position, 0.35);
        rastro.forEach((r, i) => r.position.copy(antigas[i]));
        aparelhos.forEach((ap, i) => { ap.a.rotation.y = Math.sin(tt * 0.7 + i) * 0.3; ap.a.position.y = lugares[i].y + Math.sin(tt * 1.2 + i) * 0.02; });
        peca.rotation.y = Math.sin(tt * 0.3) * 0.15;
      };

    // --- fragmentação: um arquivo vira 256 pedaços num tabuleiro 16×16 -----
    } else if (nome === "fragmentos") {
      const total = 256;
      const material = new THREE.MeshStandardMaterial({ metalness: 0.85, roughness: 0.22 });
      const malha = new THREE.InstancedMesh(caixa(0.058, 0.058, 0.058, 0.012), material, total);
      const destino = [], origem = [], giro = [], cor = new THREE.Color();
      for (let i = 0; i < total; i++) {
        const c = i % 16, l = Math.floor(i / 16);
        destino.push(new THREE.Vector3((c - 7.5) * 0.078, (7.5 - l) * 0.078 + 0.05, 0));
        const dir = new THREE.Vector3(Math.random() - 0.5, Math.random() - 0.5, Math.random() - 0.5).normalize();
        origem.push(dir.multiplyScalar(0.5 + Math.random() * 0.6).add(new THREE.Vector3(0, 0.05, 0)));
        giro.push(new THREE.Vector3(Math.random() * 6, Math.random() * 6, Math.random() * 6));
        cor.setHex(OURO).lerp(new THREE.Color(PRATA), (c + l) / 30);
        malha.setColorAt(i, cor);
      }
      const nucleo = halo(QUENTE, 1.6, 0);
      nucleo.position.set(0, 0.05, -0.05);
      peca.add(nucleo, malha);
      const m = new THREE.Matrix4(), qt = new THREE.Quaternion(), e = new THREE.Euler(), v = new THREE.Vector3(), um = new THREE.Vector3(1, 1, 1);
      let t = 0;
      atualizar = (dt, tt) => {
        t = (t + dt * 0.12) % 1;
        // 0 a 0,45: espalha. 0,55 a 1: junta de volta no tabuleiro.
        const f = t < 0.45 ? 1 - t / 0.45 : t > 0.55 ? (t - 0.55) / 0.45 : 0;
        const junta = suave(f);
        for (let i = 0; i < total; i++) {
          v.copy(origem[i]).lerp(destino[i], junta);
          const solto = 1 - junta;
          e.set(giro[i].x * solto + tt * solto, giro[i].y * solto, giro[i].z * solto);
          qt.setFromEuler(e);
          m.compose(v, qt, um);
          malha.setMatrixAt(i, m);
        }
        malha.instanceMatrix.needsUpdate = true;
        nucleo.material.opacity = junta * 0.35;
        peca.rotation.y = Math.sin(tt * 0.35) * 0.35;
      };

    // --- duas implementações que precisam concordar, byte a byte -----------
    } else if (nome === "dois") {
      const torres = [-0.48, 0.48].map((x) => {
        const camadas = [];
        for (let i = 0; i < 5; i++) {
          const camada = new THREE.Mesh(caixa(0.44, 0.13, 0.26, 0.035), cromo(PRATA, 0.22));
          camada.position.set(x, 0.46 - i * 0.2, 0);
          const friso = new THREE.Mesh(caixa(0.3, 0.018, 0.01, 0.008), luz(PRATA, 0.5));
          friso.position.set(x, 0.46 - i * 0.2, 0.136);
          peca.add(camada, friso);
          camadas.push({ camada, friso });
        }
        return camadas;
      });
      const pontes = [];
      for (let i = 0; i < 5; i++) {
        const y = 0.46 - i * 0.2;
        const ponte = tubo(new THREE.Vector3(-0.26, y, 0), new THREE.Vector3(0.26, y, 0), 0.012, luz(ACEITO, 2.2));
        const aura = halo(ACEITO, 0.35, 0);
        aura.position.set(0, y, 0);
        peca.add(ponte, aura);
        pontes.push({ ponte, aura });
      }
      let t = 0;
      atualizar = (dt, tt) => {
        t = (t + dt * 0.2) % 1;
        const ate = t * 6;
        for (let i = 0; i < 5; i++) {
          const f = Math.min(1, Math.max(0, ate - i));
          const conferido = f > 0;
          pontes[i].ponte.scale.set(1, suave(f), 1);
          pontes[i].ponte.visible = conferido;
          pontes[i].aura.material.opacity = conferido ? 0.4 * (1 - Math.abs(f - 0.5)) + 0.15 : 0;
          torres.forEach((torre) => {
            torre[i].friso.material.emissive.setHex(conferido ? ACEITO : PRATA);
            torre[i].friso.material.emissiveIntensity = conferido ? 2.2 : 0.5;
          });
        }
        peca.rotation.y = Math.sin(tt * 0.35) * 0.35;
      };

    // --- por que precisamos de gente: cada pessoa que confere acende -------
    } else if (nome === "gente") {
      const quantos = 28, pessoas = [];
      const geoPessoa = new THREE.CapsuleGeometry(0.028, 0.05, 4, leve ? 8 : 14);
      for (let i = 0; i < quantos; i++) {
        const a = (i / quantos) * Math.PI * 2;
        const corpo = new THREE.Mesh(geoPessoa, new THREE.MeshStandardMaterial({ color: APAGADO, metalness: 0.3, roughness: 0.5, emissive: QUENTE, emissiveIntensity: 0 }));
        corpo.position.set(Math.cos(a) * 0.9, Math.sin(a) * 0.55 + 0.02, Math.sin(a * 3) * 0.1);
        const aura = halo(QUENTE, 0.22, 0);
        aura.position.copy(corpo.position);
        peca.add(corpo, aura);
        pessoas.push({ corpo, aura });
      }
      const nucleo = new THREE.Group();
      nucleo.position.y = 0.02;
      const casca = new THREE.Mesh(new THREE.IcosahedronGeometry(0.3, 1), new THREE.MeshStandardMaterial({ color: PRATA, metalness: 1, roughness: 0.25, wireframe: true }));
      const miolo = new THREE.Mesh(esfera(0.17), luz(QUENTE, 0.4));
      const auraNucleo = halo(QUENTE, 1.2, 0.1);
      nucleo.add(miolo, casca, auraNucleo);
      peca.add(nucleo);
      let t = 0;
      atualizar = (dt, tt) => {
        t = (t + dt * 0.11) % 1;
        const acesos = t * quantos;
        pessoas.forEach((p, i) => {
          const f = Math.min(1, Math.max(0, acesos - i));
          p.corpo.material.emissiveIntensity = f * 2.2;
          p.corpo.scale.setScalar(0.9 + saltito(f) * 0.25);
          p.aura.material.opacity = f * 0.6;
        });
        const parte = acesos / quantos;
        miolo.material.emissiveIntensity = 0.4 + parte * 3.2;
        auraNucleo.material.opacity = 0.1 + parte * 0.5;
        auraNucleo.scale.setScalar(1.0 + parte * 0.8);
        casca.rotation.y += dt * 0.35;
        casca.rotation.x += dt * 0.15;
        peca.rotation.y = Math.sin(tt * 0.3) * 0.12;
      };
    }

    // peças que ficam pequenas no pedestal ganham um pouco de tamanho
    peca.scale.setScalar({ cadeia3d: 1.35, freivalds: 1.3, dois: 1.15 }[nome] || 1);

    const objeto = {
      grupo: g, entrada: 0,
      atualizar(dt, tt) {
        // entrada: um pequeno salto elástico; depois, uma flutuação leve
        this.entrada = Math.min(1, this.entrada + dt / 0.9);
        g.scale.setScalar(ESCALA_PALCO * (0.62 + 0.38 * saltito(this.entrada)));
        peca.position.y = Math.sin(tt * 1.1) * 0.018;
        aro.material.emissiveIntensity = 1.6 + Math.sin(tt * 1.6) * 0.35;
        if (poeira) {
          const p = poeira.geometry.attributes.position;
          for (let i = 0; i < p.count; i++) {
            let y = p.getY(i) + dt * 0.04;
            if (y > 1.0) y = -0.8;
            p.setY(i, y);
          }
          p.needsUpdate = true;
        }
        atualizar(dt, tt);
      },
    };
    OBJETOS[nome] = objeto;
    return objeto;
  }

  function objeto(nome) {
    if (objetoAtual) objetoAtual.grupo.visible = false;
    objetoAtual = nome ? criarObjeto(nome) : null;
    if (objetoAtual) { objetoAtual.entrada = 0; objetoAtual.grupo.visible = true; }
  }

  function animarHud(dt) {
    tempoHud += dt;
    if (objetoAtual) objetoAtual.atualizar(dt, tempoHud);
  }

  tamanho();
  if (calmo) { intro = 1; modo = "logo"; mirar(); pos.set(F.logo); atrPos.needsUpdate = true; desenharParado(); }
  else { cam.pos.set(0, 0, 7); ligar(); }
  // o A de metal da abertura; em modo calmo fica o SVG
  const heroi = criarHeroi(renderer, q);

  return {
    nivel: q.nivel,
    definirModo, definirZoom, somenteObjetos,
    /** Liga o A de metal sobre o SVG da abertura. Devolve falso quando fica o SVG. */
    heroi: (elemento, aoTrocar) => { if (calmo) return false; heroi.armar(elemento, aoTrocar); return true; },
    objeto,
    fps: perfil.fps || 60,
    // O video desenha o quadro ampliado a partir do centro. Os objetos precisam
    // encolher na mesma medida para nao sairem da tela: uma escala uniforme em
    // torno da camera nao muda nem o tamanho aparente nem a posicao na tela
    // depois da ampliacao.
    hudEscala: (v) => hud.scale.setScalar(v > 0 ? 1 / v : 1),
    introducao: () => (intro >= 1 ? Promise.resolve() : new Promise((res) => { introResolve = res; })),
  };
}
