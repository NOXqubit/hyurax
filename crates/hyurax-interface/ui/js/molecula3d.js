// Hyurax — molécula em 3D a partir do SMILES, sem biblioteca de fora.
//
// O que é de verdade e o que é aproximado:
// - o grafo (átomos, ligações, ordem, cargas) é o do SMILES da AqSolDB;
// - os hidrogênios implícitos são completados pela valência;
// - a geometria sai de "geometria de distâncias": comprimento de ligação pelos
//   raios covalentes (encurtado nas duplas, triplas e aromáticas), ângulo pela
//   hibridização (109,5°, 120° ou 180°), anel aromático plano e átomos que não
//   se ligam mantidos afastados. Resolve-se em 4D e achata-se para 3D, o que
//   evita nós presos. É uma conformação plausível, não uma otimização
//   quântica nem a estrutura de um cristal;
// - a vibração é dinâmica clássica (Verlet) nas mesmas molas, com
//   termostato: mostra como a molécula se mexe, não é um campo de força
//   parametrizado.

import { lerSmiles } from "./moleculas.js";

// raio covalente (Å), raio de van der Waals (Å), cor CPK
const ELEMENTOS = {
  H: [0.31, 1.2, "#F4F5F7"],
  C: [0.76, 1.7, "#4A4F5C"],
  N: [0.71, 1.55, "#3D6BF2"],
  O: [0.66, 1.52, "#F0453A"],
  F: [0.57, 1.47, "#7DDB6A"],
  P: [1.07, 1.8, "#F08A24"],
  S: [1.05, 1.8, "#E8C93A"],
  Cl: [1.02, 1.75, "#33C25B"],
  Br: [1.2, 1.85, "#A8382B"],
  I: [1.39, 1.98, "#8A3FB5"],
  B: [0.84, 1.92, "#F2A6A6"],
  Si: [1.11, 2.1, "#D9B38C"],
};
const PADRAO = [0.8, 1.8, "#E25DB4"];
const dados = (el) => ELEMENTOS[el] || PADRAO;
export const corDe = (el) => dados(el)[2];

// valência usual de cada elemento (com a carga ajustando N, O e S)
const VALENCIA = { C: 4, N: 3, O: 2, S: 2, P: 3, B: 3, F: 1, Cl: 1, Br: 1, I: 1, Si: 4 };

function sorteador(texto) {
  let h = 2166136261;
  for (const ch of texto) h = Math.imul(h ^ ch.charCodeAt(0), 16777619);
  return () => {
    h = Math.imul(h ^ (h >>> 15), 2246822507);
    h = Math.imul(h ^ (h >>> 13), 3266489909);
    h ^= h >>> 16;
    return (h >>> 0) / 4294967296;
  };
}

/** Completa os hidrogênios implícitos. Devolve um grafo novo. */
export function comHidrogenios(grafo) {
  const atomos = grafo.atomos.map((a) => ({ ...a }));
  const ligacoes = grafo.ligacoes.map((l) => ({ ...l }));
  const n = atomos.length;
  const soma = new Array(n).fill(0);
  const aromaticas = new Array(n).fill(0);
  for (const l of ligacoes) {
    soma[l.a] += l.ordem; soma[l.b] += l.ordem;
    if (l.ordem === 1.5) { aromaticas[l.a]++; aromaticas[l.b]++; }
  }
  for (let i = 0; i < n; i++) {
    const a = atomos[i];
    let h;
    if (a.hExplicito !== undefined) h = a.hExplicito;
    else if (a.colchete) h = 0;
    else {
      let v = VALENCIA[a.el];
      if (v === undefined) continue;
      // anel aromático: duas ligações de 1,5 contam 3 (uma dupla deslocalizada)
      let usada = soma[i];
      if (aromaticas[i] >= 2) usada = Math.round(usada - aromaticas[i] * 0.5 + 1);
      usada = Math.round(usada);
      if (a.el === "S" && usada > 2) v = usada <= 4 ? 4 : 6;
      if (a.el === "P" && usada > 3) v = 5;
      if (a.el === "N" && a.carga > 0) v = 4;
      if (a.el === "O" && a.carga < 0) v = 1;
      h = Math.max(0, v - usada);
    }
    for (let k = 0; k < h; k++) {
      atomos.push({ el: "H", arom: false, carga: 0 });
      ligacoes.push({ a: i, b: atomos.length - 1, ordem: 1 });
    }
  }
  return { atomos, ligacoes };
}

function comprimento(a, b, ordem) {
  const r = dados(a)[0] + dados(b)[0];
  return r * (ordem === 3 ? 0.78 : ordem === 2 ? 0.87 : ordem === 1.5 ? 0.92 : 1);
}

/**
 * Restrições de distância: [a, b, alvo, peso, tipo]. tipo 0 = igual,
 * 1 = no mínimo (repulsão entre átomos que não se ligam).
 */
function restricoes(grafo) {
  const { atomos, ligacoes } = grafo;
  const n = atomos.length;
  const viz = Array.from({ length: n }, () => []);
  const ordemEntre = new Map();
  const chave = (a, b) => (a < b ? a * 100003 + b : b * 100003 + a);
  for (const l of ligacoes) {
    viz[l.a].push(l.b); viz[l.b].push(l.a);
    ordemEntre.set(chave(l.a, l.b), l.ordem);
  }
  const ligado = (a, b) => ordemEntre.get(chave(a, b));
  const r = [];
  const tem = new Set();
  const comp = new Map();
  for (const l of ligacoes) {
    const d = comprimento(atomos[l.a].el, atomos[l.b].el, l.ordem);
    comp.set(chave(l.a, l.b), d);
    r.push([l.a, l.b, d, 1, 0]);
    tem.add(chave(l.a, l.b));
  }
  // anéis pequenos: o menor ciclo que passa por cada ligação
  const aneis = [];
  const vistos = new Set();
  for (const l of ligacoes) {
    const anterior = new Map([[l.a, -1]]);
    let fila = [l.a];
    while (fila.length && !anterior.has(l.b)) {
      const prox = [];
      for (const x of fila) for (const y of viz[x]) {
        if ((x === l.a && y === l.b) || anterior.has(y)) continue;
        anterior.set(y, x); prox.push(y);
      }
      fila = prox;
    }
    if (!anterior.has(l.b)) continue;
    const ciclo = [];
    for (let x = l.b; x !== -1; x = anterior.get(x)) ciclo.push(x);
    if (ciclo.length < 3 || ciclo.length > 8) continue;
    const id = [...ciclo].sort((x, y) => x - y).join(",");
    if (!vistos.has(id)) { vistos.add(id); aneis.push(ciclo); }
  }
  // anel plano: todos os átomos com dupla/aromática, ou heteroátomo com par
  // livre (N, O, S) entre eles, como no pirrol e na cafeína
  const temPi = (v) => viz[v].some((w) => { const o = ligado(v, w); return o === 2 || o === 1.5 || o === 3; });
  const plano = new Array(n).fill(false);
  const aneisPlanos = [];
  for (const c of aneis) {
    const pi = c.filter(temPi).length;
    const ok = c.every((v) => temPi(v) || (["N", "O", "S"].includes(atomos[v].el) && viz[v].length <= 3));
    if (ok && pi >= c.length - 2) { aneisPlanos.push(c); for (const v of c) plano[v] = true; }
  }
  // ângulo pela hibridização do átomo central
  const angulo = new Array(n);
  for (let v = 0; v < n; v++) {
    let duplas = 0, tripla = false, arom = plano[v];
    for (const w of viz[v]) {
      const o = ligado(v, w);
      if (o === 3) tripla = true;
      if (o === 2) duplas++;
      if (o === 1.5) arom = true;
    }
    const el = atomos[v].el;
    angulo[v] = tripla || duplas >= 2 ? 180 : duplas || arom ? 120 : el === "O" ? 106 : el === "N" ? 108 : 109.47;
    const vs = viz[v];
    for (let i = 0; i < vs.length; i++) {
      for (let j = i + 1; j < vs.length; j++) {
        const a = vs[i], b = vs[j];
        const da = comp.get(chave(v, a)), db = comp.get(chave(v, b));
        const t = (angulo[v] * Math.PI) / 180;
        const d = Math.sqrt(da * da + db * db - 2 * da * db * Math.cos(t));
        if (!tem.has(chave(a, b))) { r.push([a, b, d, 0.6, 0]); tem.add(chave(a, b)); }
      }
    }
  }
  // anel aromático plano: 1-4 em cis (mesmo anel) ou trans
  const noAnel = (a, d, b, c) => {
    // a e d se alcançam por outro caminho de até 3 passos sem passar por b, c
    let fronteira = [a];
    const visto = new Set([a, b, c]);
    for (let passo = 0; passo < 3; passo++) {
      const prox = [];
      for (const x of fronteira) for (const y of viz[x]) {
        if (y === d) return true;
        if (!visto.has(y)) { visto.add(y); prox.push(y); }
      }
      fronteira = prox;
    }
    return false;
  };
  for (const l of ligacoes) {
    if (l.ordem !== 1.5) continue;
    const b = l.a, c = l.b;
    for (const a of viz[b]) {
      if (a === c || ligado(a, b) === undefined) continue;
      for (const d of viz[c]) {
        if (d === b || d === a || tem.has(chave(a, d))) continue;
        if (angulo[b] !== 120 || angulo[c] !== 120) continue;
        const L = comp.get(chave(b, c));
        const cis = noAnel(a, d, b, c);
        const ab = comp.get(chave(a, b)), cd = comp.get(chave(c, d));
        // posições planas com 120°: b na origem, c em (L, 0)
        const ax = -ab * 0.5, ay = ab * 0.8660254;
        const dx = L + cd * 0.5, dy = (cis ? 1 : -1) * cd * 0.8660254;
        r.push([a, d, Math.hypot(dx - ax, dy - ay), 0.35, 0]);
        tem.add(chave(a, d));
      }
    }
  }
  // anel plano: polígono regular (lado = média das ligações do anel)
  for (const c of aneisPlanos) {
    const k = c.length;
    let L = 0;
    for (let i = 0; i < k; i++) L += comp.get(chave(c[i], c[(i + 1) % k])) || 1.4;
    L /= k;
    for (let i = 0; i < k; i++) {
      for (let j = i + 2; j < k; j++) {
        const s = Math.min(j - i, k - (j - i));
        if (s < 2) continue;
        const d = (L * Math.sin((Math.PI * s) / k)) / Math.sin(Math.PI / k);
        const q = chave(c[i], c[j]);
        const existente = r.findIndex((x) => chave(x[0], x[1]) === q);
        if (existente >= 0) r[existente] = [c[i], c[j], d, 0.8, 0];
        else { r.push([c[i], c[j], d, 0.8, 0]); tem.add(q); }
      }
    }
  }
  // quem não se liga fica longe o bastante
  for (let a = 0; a < n; a++) {
    for (let b = a + 1; b < n; b++) {
      if (tem.has(chave(a, b))) continue;
      const min = (dados(atomos[a].el)[1] + dados(atomos[b].el)[1]) * 0.62;
      r.push([a, b, min, 0.2, 1]);
    }
  }
  return r;
}

/** Geometria 3D: [{x, y, z}] em Å, centrada na origem. */
export function geometria(grafo, semente) {
  const R = restricoes(grafo);
  let melhor = null;
  // moléculas grandes: uma partida só (o custo cresce com o quadrado)
  const partidas = grafo.atomos.length > 40 ? 1 : 3;
  for (let tentativa = 0; tentativa < partidas; tentativa++) {
    const g = embutir(grafo, R, `${semente || "hyurax"}#${tentativa}`);
    if (!melhor || g.tensao < melhor.tensao) melhor = g;
  }
  return { pos: melhor.pos, restricoes: R };
}

function embutir(grafo, R, semente) {
  const n = grafo.atomos.length;
  const rnd = sorteador(semente);
  const escala = 1.4 * Math.cbrt(n) + 1;
  const p = Array.from({ length: n }, () => [0, 1, 2, 3].map(() => (rnd() - 0.5) * 2 * escala));
  const PASSOS = 1400;
  for (let it = 0; it < PASSOS; it++) {
    const t = 1 - it / PASSOS;
    const passo = 0.08 + 0.12 * t;
    const quarta = it < PASSOS * 0.4 ? 0 : ((it - PASSOS * 0.4) / (PASSOS * 0.6)) * 0.5; // achata a 4ª dimensão
    const f = Array.from({ length: n }, () => [0, 0, 0, 0]);
    for (const [a, b, alvo, peso, tipo] of R) {
      const pa = p[a], pb = p[b];
      const d0 = pb[0] - pa[0], d1 = pb[1] - pa[1], d2 = pb[2] - pa[2], d3 = pb[3] - pa[3];
      const d = Math.sqrt(d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3) || 1e-6;
      if (tipo === 1 && d >= alvo) continue;
      const k = (peso * (d - alvo)) / d;
      f[a][0] += d0 * k; f[a][1] += d1 * k; f[a][2] += d2 * k; f[a][3] += d3 * k;
      f[b][0] -= d0 * k; f[b][1] -= d1 * k; f[b][2] -= d2 * k; f[b][3] -= d3 * k;
    }
    for (let i = 0; i < n; i++) {
      f[i][3] -= p[i][3] * quarta;
      const m = Math.hypot(f[i][0], f[i][1], f[i][2], f[i][3]);
      const lim = m > 1 ? 1 / m : 1;
      for (let c = 0; c < 4; c++) p[i][c] += f[i][c] * passo * lim;
    }
  }
  const pos = p.map((q) => ({ x: q[0], y: q[1], z: q[2] }));
  const c = pos.reduce((s, q) => ({ x: s.x + q.x / n, y: s.y + q.y / n, z: s.z + q.z / n }), { x: 0, y: 0, z: 0 });
  for (const q of pos) { q.x -= c.x; q.y -= c.y; q.z -= c.z; }
  let tensao = 0;
  for (const [a, b, alvo, peso, tipo] of R) {
    const d = Math.hypot(pos[a].x - pos[b].x, pos[a].y - pos[b].y, pos[a].z - pos[b].z);
    if (tipo === 1 && d >= alvo) continue;
    tensao += peso * (d - alvo) ** 2;
  }
  return { pos, tensao };
}

/** Desvio médio das ligações em relação ao alvo (Å): qualidade da geometria. */
export function desvioDasLigacoes(grafo, pos) {
  let soma = 0;
  for (const l of grafo.ligacoes) {
    const a = pos[l.a], b = pos[l.b];
    soma += Math.abs(Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z) - comprimento(grafo.atomos[l.a].el, grafo.atomos[l.b].el, l.ordem));
  }
  return grafo.ligacoes.length ? soma / grafo.ligacoes.length : 0;
}

/**
 * Visualizador: arraste para girar, roda do mouse para aproximar.
 * Devolve { mostrar(smiles), modo(nome), hidrogenios(bool), vibrar(bool), parar() }.
 */
export function visualizador(canvas, aoMudar) {
  const g = canvas.getContext("2d");
  const calmo = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let grafoCompleto = null, geo = null, base = null;
  let rot = { a: -0.5, b: 0.35 }; // azimute e elevação
  let zoom = 1, modoAtual = "bastoes", comH = true, vibrando = false, girando = !calmo;
  let vel = null, quadro = 0, arrastando = null, ultimoToque = 0, temperatura = 300;

  function mostrar(smiles) {
    // deixa a tela pintar o nome antes da conta, que leva um instante
    setTimeout(() => montar(smiles), 30);
  }

  function montar(smiles) {
    const plano = lerSmiles(smiles || "");
    grafoCompleto = comHidrogenios(plano);
    geo = grafoCompleto.atomos.length ? geometria(grafoCompleto, smiles) : null;
    base = geo ? geo.pos.map((q) => ({ ...q })) : null;
    vel = base ? base.map(() => ({ x: 0, y: 0, z: 0 })) : null;
    zoom = 1;
    aoMudar?.({
      atomos: grafoCompleto.atomos.length,
      pesados: plano.atomos.length,
      ligacoes: grafoCompleto.ligacoes.length,
      desvio: geo ? desvioDasLigacoes(grafoCompleto, geo.pos) : 0,
    });
    if (!quadro) quadro = requestAnimationFrame(desenhar);
  }

  // dinâmica clássica nas mesmas molas, com termostato de Berendsen simples
  function passoDeVibracao() {
    const pos = geo.pos, n = pos.length;
    const dt = 0.02;
    const f = pos.map(() => ({ x: 0, y: 0, z: 0 }));
    for (const [a, b, alvo, peso, tipo] of geo.restricoes) {
      const dx = pos[b].x - pos[a].x, dy = pos[b].y - pos[a].y, dz = pos[b].z - pos[a].z;
      const d = Math.hypot(dx, dy, dz) || 1e-6;
      if (tipo === 1 && d >= alvo) continue;
      const k = (peso * 40 * (d - alvo)) / d;
      f[a].x += dx * k; f[a].y += dy * k; f[a].z += dz * k;
      f[b].x -= dx * k; f[b].y -= dy * k; f[b].z -= dz * k;
    }
    let cin = 0;
    for (let i = 0; i < n; i++) {
      const m = grafoCompleto.atomos[i].el === "H" ? 1 : 12;
      vel[i].x += (f[i].x / m) * dt; vel[i].y += (f[i].y / m) * dt; vel[i].z += (f[i].z / m) * dt;
      cin += m * (vel[i].x ** 2 + vel[i].y ** 2 + vel[i].z ** 2);
    }
    const alvo = (temperatura / 300) * 0.004 * n;
    const ajuste = cin > 0 ? Math.sqrt(Math.min(4, Math.max(0.25, alvo / cin))) : 1;
    for (let i = 0; i < n; i++) {
      // chute térmico pequeno para nunca parar
      vel[i].x = vel[i].x * ajuste + (Math.random() - 0.5) * 0.002 * temperatura / 300;
      vel[i].y = vel[i].y * ajuste + (Math.random() - 0.5) * 0.002 * temperatura / 300;
      vel[i].z = vel[i].z * ajuste + (Math.random() - 0.5) * 0.002 * temperatura / 300;
      pos[i].x += vel[i].x * dt * 10; pos[i].y += vel[i].y * dt * 10; pos[i].z += vel[i].z * dt * 10;
    }
  }

  function desenhar() {
    quadro = 0;
    if (!geo || !canvas.isConnected) return;
    const esc = devicePixelRatio || 1;
    const w = canvas.clientWidth, h = canvas.clientHeight;
    if (!w || !h) { quadro = requestAnimationFrame(desenhar); return; }
    if (canvas.width !== Math.round(w * esc)) { canvas.width = Math.round(w * esc); canvas.height = Math.round(h * esc); }
    g.setTransform(esc, 0, 0, esc, 0, 0);
    g.clearRect(0, 0, w, h);
    if (girando && !arrastando && performance.now() - ultimoToque > 2500) rot.a += 0.004;
    if (vibrando) for (let k = 0; k < 3; k++) passoDeVibracao();

    const { atomos, ligacoes } = grafoCompleto;
    const ca = Math.cos(rot.a), sa = Math.sin(rot.a), cb = Math.cos(rot.b), sb = Math.sin(rot.b);
    const raio = Math.max(...base.map((q) => Math.hypot(q.x, q.y, q.z))) + 1.6;
    const s = (Math.min(w, h) / (2 * raio)) * zoom;
    const P = geo.pos.map((q) => {
      const x1 = q.x * ca + q.z * sa, z1 = -q.x * sa + q.z * ca;
      const y2 = q.y * cb - z1 * sb, z2 = q.y * sb + z1 * cb;
      const persp = 1 / (1 - z2 / (raio * 6));
      return { x: w / 2 + x1 * s * persp, y: h / 2 + y2 * s * persp, z: z2, k: persp };
    });
    const visivel = (i) => comH || atomos[i].el !== "H";
    const raioAtomo = (i) => (modoAtual === "esferas" ? dados(atomos[i].el)[1] * 0.95 : atomos[i].el === "H" ? 0.22 : 0.34) * s * P[i].k;
    // pinta de trás para frente: ligações e átomos juntos pela profundidade
    const itens = [];
    if (modoAtual !== "esferas") {
      for (const l of ligacoes) if (visivel(l.a) && visivel(l.b)) itens.push({ z: (P[l.a].z + P[l.b].z) / 2 - 0.01, l });
    }
    atomos.forEach((_, i) => { if (visivel(i)) itens.push({ z: P[i].z, i }); });
    itens.sort((x, y) => x.z - y.z);
    const luz = (z) => 0.55 + 0.45 * Math.min(1, Math.max(0, (z + raio) / (2 * raio)));
    for (const it of itens) {
      if (it.l) {
        const { a, b, ordem } = it.l;
        const A = P[a], B = P[b];
        const dx = B.x - A.x, dy = B.y - A.y, d = Math.hypot(dx, dy) || 1;
        const nx = -dy / d, ny = dx / d;
        const larg = 0.13 * s * ((A.k + B.k) / 2);
        const mx = (A.x + B.x) / 2, my = (A.y + B.y) / 2;
        const tracos = ordem === 2 ? [-1, 1] : ordem === 3 ? [-1.6, 0, 1.6] : ordem === 1.5 ? [-0.8, 0.8] : [0];
        g.globalAlpha = luz(it.z);
        for (const off of tracos) {
          const ox = nx * off * larg * 1.25, oy = ny * off * larg * 1.25;
          const lw = tracos.length > 1 ? larg * 0.7 : larg;
          g.lineCap = "round";
          g.lineWidth = lw;
          g.strokeStyle = corDe(atomos[a].el);
          g.beginPath(); g.moveTo(A.x + ox, A.y + oy); g.lineTo(mx + ox, my + oy); g.stroke();
          g.strokeStyle = corDe(atomos[b].el);
          g.beginPath(); g.moveTo(mx + ox, my + oy); g.lineTo(B.x + ox, B.y + oy); g.stroke();
        }
        g.globalAlpha = 1;
      } else {
        const i = it.i, q = P[i], r = Math.max(1.5, raioAtomo(i));
        const cor = corDe(atomos[i].el);
        const gr = g.createRadialGradient(q.x - r * 0.35, q.y - r * 0.4, r * 0.1, q.x, q.y, r);
        gr.addColorStop(0, "#FFFFFF");
        gr.addColorStop(0.28, cor);
        gr.addColorStop(1, misturar(cor, "#000000", 0.55));
        g.globalAlpha = luz(it.z);
        g.fillStyle = gr;
        g.beginPath(); g.arc(q.x, q.y, r, 0, Math.PI * 2); g.fill();
        g.globalAlpha = 1;
      }
    }
    if (girando || vibrando || arrastando) quadro = requestAnimationFrame(desenhar);
  }
  const redesenhar = () => { if (!quadro) quadro = requestAnimationFrame(desenhar); };

  canvas.addEventListener("pointerdown", (e) => {
    arrastando = { x: e.clientX, y: e.clientY };
    canvas.focus({ preventScroll: true });
    canvas.setPointerCapture(e.pointerId);
    redesenhar();
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!arrastando) return;
    rot.a += (e.clientX - arrastando.x) * 0.01;
    rot.b = Math.max(-1.5, Math.min(1.5, rot.b + (e.clientY - arrastando.y) * 0.01));
    arrastando = { x: e.clientX, y: e.clientY };
    ultimoToque = performance.now();
  });
  const soltar = () => { arrastando = null; ultimoToque = performance.now(); redesenhar(); };
  canvas.addEventListener("pointerup", soltar);
  canvas.addEventListener("pointercancel", soltar);
  // a roda só aproxima depois de clicar na molécula: antes, rola a página
  canvas.addEventListener("wheel", (e) => {
    if (document.activeElement !== canvas) return;
    e.preventDefault();
    zoom = Math.max(0.5, Math.min(3, zoom * (e.deltaY < 0 ? 1.1 : 0.9)));
    redesenhar();
  }, { passive: false });
  canvas.addEventListener("keydown", (e) => {
    const passos = { ArrowLeft: [-0.15, 0], ArrowRight: [0.15, 0], ArrowUp: [0, -0.15], ArrowDown: [0, 0.15] };
    if (passos[e.key]) {
      e.preventDefault();
      rot.a += passos[e.key][0]; rot.b = Math.max(-1.5, Math.min(1.5, rot.b + passos[e.key][1]));
      ultimoToque = performance.now();
      redesenhar();
    } else if (e.key === "+" || e.key === "-") {
      zoom = Math.max(0.5, Math.min(3, zoom * (e.key === "+" ? 1.15 : 0.87)));
      redesenhar();
    }
  });

  return {
    mostrar,
    modo(m) { modoAtual = m; redesenhar(); },
    hidrogenios(v) { comH = v; redesenhar(); },
    girar(v) { girando = v; redesenhar(); },
    vibrar(v, kelvin) {
      vibrando = v;
      if (kelvin) temperatura = kelvin;
      if (!v && base) { geo.pos = base.map((q) => ({ ...q })); vel = base.map(() => ({ x: 0, y: 0, z: 0 })); }
      redesenhar();
    },
    temperatura(k) { temperatura = k; },
    parar() { girando = false; vibrando = false; if (quadro) cancelAnimationFrame(quadro); quadro = 0; },
  };
}

function misturar(a, b, t) {
  const p = (c) => [1, 3, 5].map((k) => parseInt(c.slice(k, k + 2), 16));
  const [x, y] = [p(a), p(b)];
  return `rgb(${x.map((v, i) => Math.round(v + (y[i] - v) * t)).join(",")})`;
}
