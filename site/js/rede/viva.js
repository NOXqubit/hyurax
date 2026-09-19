// A rede viva: a abertura do site.
//
// Nós espalhados em aglomerados (como cidades), cada um ligado aos vizinhos
// mais próximos. De tempos em tempos um nó minera um bloco, e o bloco se
// espalha de verdade pelo grafo, salto a salto: cada pulso anda por uma
// ligação e, quando chega, o nó acende porque conferiu. De vez em quando um nó
// solta uma onda azul: é o Éter.
//
// Tudo é desenhado; o que se espalha é a regra da rede (inundação por vizinhos),
// não dados reais. O painel "ao vivo" ao lado mostra hashes SHA-512 de verdade
// calculados no navegador para cada bloco simulado.

import * as THREE from "../../vendor/three.module.min.js";

const TIPOS = [
  { nome: "servidor", tamanho: 3.1, parte: 0.08 },
  { nome: "computador", tamanho: 2.2, parte: 0.34 },
  { nome: "celular", tamanho: 1.45, parte: 0.58 },
];
const LUZ = new THREE.Color("#f2d9a8");
const LUZ_FORTE = new THREE.Color("#fff4e0");
const ETER = new THREE.Color("#a9d8ff");

const QUANTOS = { HIGH: 240, MEDIUM: 160, LOW: 96 };
const MAX_PULSOS = 220;
const MAX_ONDAS = 6;

function aleatorio(semente) {
  let s = semente >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const VERTICE = /* glsl */ `
  attribute float tamanho;
  attribute float brilho;
  attribute vec3 cor;
  uniform float escala;
  varying float vBrilho;
  varying vec3 vCor;
  void main() {
    vec4 mv = modelViewMatrix * vec4(position, 1.0);
    gl_Position = projectionMatrix * mv;
    gl_PointSize = tamanho * escala * (1.0 + brilho * 1.6) / max(-mv.z, 0.1);
    vBrilho = brilho;
    vCor = cor;
  }
`;
const FRAGMENTO = /* glsl */ `
  uniform float anel;
  varying float vBrilho;
  varying vec3 vCor;
  void main() {
    vec2 p = gl_PointCoord - 0.5;
    float d = length(p);
    if (d > 0.5) discard;
    float a;
    if (anel > 0.5) {
      a = smoothstep(0.5, 0.44, d) * smoothstep(0.36, 0.44, d) * vBrilho;
    } else {
      float halo = pow(smoothstep(0.5, 0.0, d), 1.7);
      float nucleo = smoothstep(0.2, 0.05, d);
      a = halo * (0.45 + 0.55 * vBrilho) + nucleo * (0.7 + 0.3 * vBrilho);
    }
    gl_FragColor = vec4(vCor * (0.75 + vBrilho * 0.9), a);
  }
`;

function material(escala, anel = false) {
  return new THREE.ShaderMaterial({
    uniforms: { escala: { value: escala }, anel: { value: anel ? 1 : 0 } },
    vertexShader: VERTICE,
    fragmentShader: FRAGMENTO,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });
}

/**
 * Cria a rede no canvas. Devolve { parar }.
 * aoBloco({ altura, no, tipo }) é chamado quando um nó minera um bloco.
 */
export function criarRedeViva(canvas, { nivel = "MEDIUM", calmo = false, aoBloco = () => {} } = {}) {
  const sorte = aleatorio(2026);
  const n = QUANTOS[nivel] || QUANTOS.MEDIUM;
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: nivel !== "LOW", alpha: true, powerPreference: "low-power" });
  let dpr = Math.min(window.devicePixelRatio || 1, nivel === "HIGH" ? 2 : nivel === "MEDIUM" ? 1.5 : 1);
  renderer.setPixelRatio(dpr);
  renderer.setClearColor(0x000000, 0);

  const cena = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(42, 1, 0.1, 200);
  camera.position.set(0, 0.6, 26);
  const grupo = new THREE.Group();
  cena.add(grupo);

  // ---------- nós em aglomerados ----------
  const centros = Array.from({ length: 8 }, (_, i) => new THREE.Vector3(
    (sorte() - 0.35) * 30,
    (sorte() - 0.5) * 12,
    (sorte() - 0.5) * 12 + (i % 2 ? -2 : 2),
  ));
  const pos = [];
  const tipo = [];
  for (let i = 0; i < n; i++) {
    const c = centros[i % centros.length];
    const espalho = 2.2 + sorte() * 3.4;
    const g = () => (sorte() + sorte() + sorte() - 1.5) * espalho;
    pos.push(new THREE.Vector3(c.x + g() * 1.3, c.y + g() * 0.7, c.z + g()));
    const r = sorte();
    tipo.push(r < TIPOS[0].parte ? 0 : r < TIPOS[0].parte + TIPOS[1].parte ? 1 : 2);
  }

  // ---------- ligações: cada nó aos 2 ou 3 mais próximos ----------
  const vizinhos = Array.from({ length: n }, () => new Set());
  for (let i = 0; i < n; i++) {
    const perto = [];
    for (let j = 0; j < n; j++) if (j !== i) perto.push([pos[i].distanceToSquared(pos[j]), j]);
    perto.sort((a, b) => a[0] - b[0]);
    const k = tipo[i] === 0 ? 4 : tipo[i] === 1 ? 3 : 2;
    for (let m = 0; m < k; m++) { vizinhos[i].add(perto[m][1]); vizinhos[perto[m][1]].add(i); }
  }
  // Os aglomerados se ligam entre si pelos servidores: a rede é uma só.
  const servidores = tipo.map((t, i) => (t === 0 ? i : -1)).filter((i) => i >= 0);
  for (let s = 1; s < servidores.length; s++) {
    const a = servidores[s], b = servidores[s - 1];
    vizinhos[a].add(b); vizinhos[b].add(a);
  }
  const arestas = [];
  for (let i = 0; i < n; i++) for (const j of vizinhos[i]) if (i < j) arestas.push([i, j]);

  const linhasPos = new Float32Array(arestas.length * 6);
  arestas.forEach(([a, b], k) => {
    linhasPos.set([pos[a].x, pos[a].y, pos[a].z, pos[b].x, pos[b].y, pos[b].z], k * 6);
  });
  const geoLinhas = new THREE.BufferGeometry();
  geoLinhas.setAttribute("position", new THREE.BufferAttribute(linhasPos, 3));
  const linhas = new THREE.LineSegments(geoLinhas, new THREE.LineBasicMaterial({
    color: LUZ, transparent: true, opacity: 0.16, blending: THREE.AdditiveBlending, depthWrite: false,
  }));
  grupo.add(linhas);

  // ---------- pontos dos nós ----------
  const geoNos = new THREE.BufferGeometry();
  geoNos.setAttribute("position", new THREE.BufferAttribute(new Float32Array(pos.flatMap((p) => [p.x, p.y, p.z])), 3));
  geoNos.setAttribute("tamanho", new THREE.BufferAttribute(new Float32Array(tipo.map((t) => TIPOS[t].tamanho)), 1));
  const brilhoNos = new Float32Array(n);
  geoNos.setAttribute("brilho", new THREE.BufferAttribute(brilhoNos, 1));
  const corNos = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) LUZ.toArray(corNos, i * 3);
  geoNos.setAttribute("cor", new THREE.BufferAttribute(corNos, 3));
  const matNos = material(34);
  grupo.add(new THREE.Points(geoNos, matNos));

  // ---------- pulsos (blocos andando pelas ligações) ----------
  const geoPulsos = new THREE.BufferGeometry();
  const pulsosPos = new Float32Array(MAX_PULSOS * 3).fill(9999);
  const pulsosBrilho = new Float32Array(MAX_PULSOS);
  const pulsosCor = new Float32Array(MAX_PULSOS * 3);
  for (let i = 0; i < MAX_PULSOS; i++) LUZ_FORTE.toArray(pulsosCor, i * 3);
  geoPulsos.setAttribute("position", new THREE.BufferAttribute(pulsosPos, 3));
  geoPulsos.setAttribute("tamanho", new THREE.BufferAttribute(new Float32Array(MAX_PULSOS).fill(1.9), 1));
  geoPulsos.setAttribute("brilho", new THREE.BufferAttribute(pulsosBrilho, 1));
  geoPulsos.setAttribute("cor", new THREE.BufferAttribute(pulsosCor, 3));
  grupo.add(new THREE.Points(geoPulsos, material(34)));
  const pulsos = []; // { a, b, inicio, dur, slot }
  const livres = Array.from({ length: MAX_PULSOS }, (_, i) => i);

  // ---------- ondas do Éter ----------
  const geoOndas = new THREE.BufferGeometry();
  const ondasPos = new Float32Array(MAX_ONDAS * 3).fill(9999);
  const ondasTam = new Float32Array(MAX_ONDAS);
  const ondasBrilho = new Float32Array(MAX_ONDAS);
  const ondasCor = new Float32Array(MAX_ONDAS * 3);
  for (let i = 0; i < MAX_ONDAS; i++) ETER.toArray(ondasCor, i * 3);
  geoOndas.setAttribute("position", new THREE.BufferAttribute(ondasPos, 3));
  geoOndas.setAttribute("tamanho", new THREE.BufferAttribute(ondasTam, 1));
  geoOndas.setAttribute("brilho", new THREE.BufferAttribute(ondasBrilho, 1));
  geoOndas.setAttribute("cor", new THREE.BufferAttribute(ondasCor, 3));
  grupo.add(new THREE.Points(geoOndas, material(34, true)));
  const ondas = []; // { no, inicio, slot }

  // ---------- a propagação de um bloco ----------
  let altura = 0;
  const alcancado = new Map(); // bloco -> Set de nós
  function propagar(bloco, de, agora) {
    for (const para of vizinhos[de]) {
      if (alcancado.get(bloco).has(para) || !livres.length) continue;
      alcancado.get(bloco).add(para);
      const dur = 0.16 + pos[de].distanceTo(pos[para]) * 0.07;
      pulsos.push({ bloco, a: de, b: para, inicio: agora, dur, slot: livres.pop() });
    }
  }
  function minerar(agora) {
    altura += 1;
    // Celulares e computadores mineram mais vezes por serem maioria; o servidor não é especial.
    const no = Math.floor(sorte() * n);
    alcancado.set(altura, new Set([no]));
    brilhoNos[no] = 1.6;
    propagar(altura, no, agora);
    aoBloco({ altura, no, tipo: TIPOS[tipo[no]].nome });
    // Blocos antigos saem da memória.
    for (const b of alcancado.keys()) if (b < altura - 4) alcancado.delete(b);
  }
  function soltarOnda(agora) {
    if (ondas.length >= MAX_ONDAS) return;
    const usados = new Set(ondas.map((o) => o.slot));
    const slot = [...Array(MAX_ONDAS).keys()].find((s) => !usados.has(s));
    ondas.push({ no: Math.floor(sorte() * n), inicio: agora, slot });
  }

  // ---------- tamanho ----------
  function ajustar() {
    const w = canvas.clientWidth || 1, h = canvas.clientHeight || 1;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    // Em tela estreita, afasta para caber a rede inteira.
    camera.position.z = w < 700 ? 38 : 26;
    camera.updateProjectionMatrix();
    const escala = h * dpr * 0.3;
    grupo.children.forEach((o) => { if (o.material?.uniforms?.escala) o.material.uniforms.escala.value = escala; });
  }
  const observador = new ResizeObserver(ajustar);
  observador.observe(canvas);
  ajustar();

  // ---------- movimento do mouse ----------
  let alvoX = 0, alvoY = 0;
  const aoMover = (e) => {
    alvoX = (e.clientX / innerWidth - 0.5) * 0.35;
    alvoY = (e.clientY / innerHeight - 0.5) * 0.18;
  };
  addEventListener("pointermove", aoMover, { passive: true });

  // ---------- laço de desenho ----------
  const fps = nivel === "LOW" ? 24 : nivel === "MEDIUM" ? 40 : 60;
  const passoMin = 1 / fps;
  let visivel = true, ultimo = 0, proximoBloco = 0.6, proximaOnda = 2.5, tempo = 0, lentos = 0;
  const vis = new IntersectionObserver((es) => { visivel = es[0].isIntersecting; }, { threshold: 0.02 });
  vis.observe(canvas);

  function passo(dt) {
    tempo += dt;
    if (tempo >= proximoBloco) { minerar(tempo); proximoBloco = tempo + 2.4 + sorte() * 1.6; }
    if (tempo >= proximaOnda) { soltarOnda(tempo); proximaOnda = tempo + 3 + sorte() * 4; }

    for (let i = 0; i < n; i++) brilhoNos[i] = Math.max(0, brilhoNos[i] - dt * 1.1);

    for (let k = pulsos.length - 1; k >= 0; k--) {
      const p = pulsos[k];
      const f = (tempo - p.inicio) / p.dur;
      if (f >= 1) {
        brilhoNos[p.b] = Math.max(brilhoNos[p.b], 1);
        pulsosPos[p.slot * 3] = 9999;
        pulsosBrilho[p.slot] = 0;
        livres.push(p.slot);
        pulsos.splice(k, 1);
        if (alcancado.has(p.bloco)) propagar(p.bloco, p.b, tempo);
        continue;
      }
      const a = pos[p.a], b = pos[p.b];
      pulsosPos[p.slot * 3] = a.x + (b.x - a.x) * f;
      pulsosPos[p.slot * 3 + 1] = a.y + (b.y - a.y) * f;
      pulsosPos[p.slot * 3 + 2] = a.z + (b.z - a.z) * f;
      pulsosBrilho[p.slot] = 1;
    }

    for (let k = ondas.length - 1; k >= 0; k--) {
      const o = ondas[k];
      const f = (tempo - o.inicio) / 2.6;
      if (f >= 1) { ondasPos[o.slot * 3] = 9999; ondasBrilho[o.slot] = 0; ondas.splice(k, 1); continue; }
      const p = pos[o.no];
      ondasPos.set([p.x, p.y, p.z], o.slot * 3);
      ondasTam[o.slot] = 4 + f * 34;
      ondasBrilho[o.slot] = (1 - f) * 0.55;
    }

    geoNos.attributes.brilho.needsUpdate = true;
    geoPulsos.attributes.position.needsUpdate = true;
    geoPulsos.attributes.brilho.needsUpdate = true;
    geoOndas.attributes.position.needsUpdate = true;
    geoOndas.attributes.tamanho.needsUpdate = true;
    geoOndas.attributes.brilho.needsUpdate = true;

    grupo.rotation.y = Math.sin(tempo * 0.035) * 0.28 + alvoX;
    grupo.rotation.x += (alvoY - grupo.rotation.x) * 0.04;
  }

  let quadro = 0;
  function laco(agora) {
    quadro = requestAnimationFrame(laco);
    if (!visivel || document.hidden) { ultimo = agora; return; }
    const dt = Math.min(0.1, (agora - ultimo) / 1000);
    if (dt < passoMin) return;
    ultimo = agora;
    // Máquina engasgando: reduz a resolução em vez de travar a página.
    if (dt > 0.07 && ++lentos > 30 && dpr > 0.75) { dpr = Math.max(0.75, dpr - 0.25); renderer.setPixelRatio(dpr); ajustar(); lentos = 0; }
    passo(dt);
    renderer.render(cena, camera);
  }

  if (calmo) {
    // Movimento reduzido: um retrato parado da rede, com alguns nós acesos.
    for (let i = 0; i < n; i += 7) brilhoNos[i] = 0.9;
    geoNos.attributes.brilho.needsUpdate = true;
    aoBloco({ altura: 1, no: 0, tipo: TIPOS[tipo[0]].nome });
    renderer.render(cena, camera);
  } else {
    quadro = requestAnimationFrame((t) => { ultimo = t; laco(t); });
  }

  return {
    parar() {
      cancelAnimationFrame(quadro);
      observador.disconnect();
      vis.disconnect();
      removeEventListener("pointermove", aoMover);
      renderer.dispose();
    },
  };
}
