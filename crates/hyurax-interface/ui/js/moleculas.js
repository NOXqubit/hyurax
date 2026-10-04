// Hyurax — desenho de moléculas a partir do SMILES, sem biblioteca de fora.
//
// O SMILES vem da base (AqSolDB, CC0) que a IA do ULTRAX usa. O desenho é o
// grafo da molécula de verdade: cada átomo e cada ligação do SMILES. A posição
// na tela sai de um arranjo por forças (ligação puxa, átomos se afastam), então
// é uma projeção plana aproximada, não uma estrutura cristalográfica.

const ORGANICOS = ["Cl", "Br", "B", "C", "N", "O", "P", "S", "F", "I"];
const AROMATICOS = ["b", "c", "n", "o", "p", "s"];
const ORDEM = { "-": 1, "=": 2, "#": 3, "$": 3, ":": 1.5 };

// Lê o SMILES num grafo { atomos: [{el, arom, carga}], ligacoes: [{a, b, ordem}] }.
// Estereoquímica (/ \ @) é ignorada: não muda o desenho plano.
export function lerSmiles(smiles) {
  const atomos = [];
  const ligacoes = [];
  const pilha = [];
  const aneis = new Map();
  let anterior = -1;
  let pendente = null;
  const ligar = (a, b, ordem) => {
    if (a < 0 || b < 0 || a === b) return;
    const aromatica = atomos[a].arom && atomos[b].arom;
    ligacoes.push({ a, b, ordem: ordem ?? (aromatica ? 1.5 : 1) });
  };
  for (let i = 0; i < smiles.length; ) {
    const c = smiles[i];
    if (c === "(") { pilha.push(anterior); i++; continue; }
    if (c === ")") { anterior = pilha.pop() ?? anterior; i++; continue; }
    if (c in ORDEM) { pendente = ORDEM[c]; i++; continue; }
    if (c === "/" || c === "\\" || c === ".") { i++; continue; }
    if (/[0-9%]/.test(c)) {
      let numero;
      if (c === "%") { numero = smiles.slice(i + 1, i + 3); i += 3; } else { numero = c; i++; }
      const aberto = aneis.get(numero);
      if (aberto) {
        ligar(aberto.atomo, anterior, pendente ?? aberto.ordem);
        aneis.delete(numero);
      } else {
        aneis.set(numero, { atomo: anterior, ordem: pendente });
      }
      pendente = null;
      continue;
    }
    let el = null, arom = false, carga = 0, hExplicito, colchete = false;
    if (c === "[") {
      const fim = smiles.indexOf("]", i);
      const dentro = smiles.slice(i + 1, fim < 0 ? smiles.length : fim);
      i = fim < 0 ? smiles.length : fim + 1;
      const m = /^\d*([A-Z][a-z]?|[a-z]{1,2})/.exec(dentro);
      el = m ? m[1] : "?";
      arom = el === el.toLowerCase();
      const cargas = /([+-]+)(\d*)$/.exec(dentro);
      if (cargas) carga = (cargas[1][0] === "+" ? 1 : -1) * (cargas[2] ? Number(cargas[2]) : cargas[1].length);
      // hidrogênios escritos no colchete ([nH], [NH3+]); sem H, nenhum
      const hs = /^\d*(?:[A-Z][a-z]?|[a-z]{1,2})[@]*H(\d?)/.exec(dentro);
      hExplicito = hs ? (hs[1] ? Number(hs[1]) : 1) : 0;
      colchete = true;
    } else {
      const dois = smiles.slice(i, i + 2);
      if (ORGANICOS.includes(dois)) { el = dois; i += 2; }
      else if (ORGANICOS.includes(c)) { el = c; i++; }
      else if (AROMATICOS.includes(c)) { el = c; arom = true; i++; }
      else { i++; continue; }
    }
    const nome = arom ? el[0].toUpperCase() + el.slice(1) : el;
    atomos.push({ el: nome, arom, carga, hExplicito, colchete });
    const novo = atomos.length - 1;
    if (anterior >= 0) ligar(anterior, novo, pendente);
    pendente = null;
    anterior = novo;
  }
  return { atomos, ligacoes };
}

// Pseudo-aleatório determinístico: a mesma molécula sai sempre igual.
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

// Arranjo por forças: mola nas ligações (comprimento 1), mola mais longa entre
// vizinhos de vizinhos (1,73: o ângulo de 120 graus), e repulsão entre todos.
export function arranjar(grafo, semente) {
  const n = grafo.atomos.length;
  const rnd = sorteador(semente);
  const vizinhos = Array.from({ length: n }, () => []);
  for (const l of grafo.ligacoes) { vizinhos[l.a].push(l.b); vizinhos[l.b].push(l.a); }
  // começa numa espiral, que já separa quase tudo
  const pos = Array.from({ length: n }, (_, i) => ({
    x: Math.cos(i * 2.4) * (0.6 + i * 0.35) + rnd() * 0.1,
    y: Math.sin(i * 2.4) * (0.6 + i * 0.35) + rnd() * 0.1,
  }));
  const pares = [];
  for (const l of grafo.ligacoes) pares.push([l.a, l.b, 1]);
  for (let v = 0; v < n; v++) {
    const viz = vizinhos[v];
    for (let i = 0; i < viz.length; i++) for (let j = i + 1; j < viz.length; j++) pares.push([viz[i], viz[j], 1.73]);
  }
  for (let passo = 0; passo < 500; passo++) {
    const t = 0.1 * (1 - passo / 500) + 0.01;
    const f = pos.map(() => ({ x: 0, y: 0 }));
    for (let a = 0; a < n; a++) {
      for (let b = a + 1; b < n; b++) {
        let dx = pos[a].x - pos[b].x, dy = pos[a].y - pos[b].y;
        const d2 = Math.max(dx * dx + dy * dy, 0.01);
        const r = 0.35 / d2;
        f[a].x += dx * r; f[a].y += dy * r;
        f[b].x -= dx * r; f[b].y -= dy * r;
      }
    }
    for (const [a, b, alvo] of pares) {
      const dx = pos[b].x - pos[a].x, dy = pos[b].y - pos[a].y;
      const d = Math.max(Math.hypot(dx, dy), 0.001);
      const k = (d - alvo) / d * 0.9;
      f[a].x += dx * k; f[a].y += dy * k;
      f[b].x -= dx * k; f[b].y -= dy * k;
    }
    for (let a = 0; a < n; a++) {
      const m = Math.hypot(f[a].x, f[a].y);
      const lim = m > 1 ? 1 / m : 1;
      pos[a].x += f[a].x * t * lim * 3;
      pos[a].y += f[a].y * t * lim * 3;
    }
  }
  return pos;
}

// Desenha no canvas, com as cores do tema passadas pelo painel.
export function desenharMolecula(canvas, smiles, cores) {
  const g = canvas.getContext("2d");
  const escala = window.devicePixelRatio || 1;
  const largura = canvas.clientWidth, altura = canvas.clientHeight;
  if (!largura || !altura) return;
  canvas.width = Math.round(largura * escala);
  canvas.height = Math.round(altura * escala);
  g.setTransform(escala, 0, 0, escala, 0, 0);
  g.clearRect(0, 0, largura, altura);
  const grafo = lerSmiles(smiles || "");
  if (!grafo.atomos.length) return;
  const pos = arranjar(grafo, smiles);
  const xs = pos.map((p) => p.x), ys = pos.map((p) => p.y);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const margem = 22;
  const s = Math.min((largura - 2 * margem) / Math.max(x1 - x0, 1), (altura - 2 * margem) / Math.max(y1 - y0, 1), 42);
  const cx = largura / 2 - ((x0 + x1) / 2) * s, cy = altura / 2 - ((y0 + y1) / 2) * s;
  const P = pos.map((p) => ({ x: cx + p.x * s, y: cy + p.y * s }));
  const rotulado = (a) => grafo.atomos[a].el !== "C" || grafo.atomos[a].carga !== 0;
  g.lineCap = "round";
  g.strokeStyle = cores.tinta;
  g.lineWidth = 1.6;
  for (const l of grafo.ligacoes) {
    let a = P[l.a], b = P[l.b];
    const dx = b.x - a.x, dy = b.y - a.y, d = Math.hypot(dx, dy) || 1;
    // encurta a ponta que tem letra, para a linha não passar por cima
    const corte = 9;
    if (rotulado(l.a)) a = { x: a.x + (dx / d) * corte, y: a.y + (dy / d) * corte };
    if (rotulado(l.b)) b = { x: b.x - (dx / d) * corte, y: b.y - (dy / d) * corte };
    const nx = (-dy / d) * 3.2, ny = (dx / d) * 3.2;
    const linha = (ox, oy, tracejada) => {
      g.setLineDash(tracejada ? [3, 3] : []);
      g.beginPath();
      g.moveTo(a.x + ox, a.y + oy);
      g.lineTo(b.x + ox, b.y + oy);
      g.stroke();
    };
    if (l.ordem === 2) { linha(nx, ny); linha(-nx, -ny); }
    else if (l.ordem === 3) { linha(0, 0); linha(nx * 1.6, ny * 1.6); linha(-nx * 1.6, -ny * 1.6); }
    else if (l.ordem === 1.5) { linha(0, 0); linha(nx * 1.4, ny * 1.4, true); }
    else linha(0, 0);
  }
  g.setLineDash([]);
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.font = `600 13px ${cores.fonte}`;
  grafo.atomos.forEach((at, i) => {
    if (!rotulado(i)) return;
    const sinal = at.carga ? (Math.abs(at.carga) > 1 ? Math.abs(at.carga) : "") + (at.carga > 0 ? "+" : "−") : "";
    g.fillStyle = cores.tinta;
    g.fillText(at.el, P[i].x, P[i].y);
    if (sinal) {
      g.font = `600 9px ${cores.fonte}`;
      g.fillText(sinal, P[i].x + 9, P[i].y - 7);
      g.font = `600 13px ${cores.fonte}`;
    }
  });
}

// Curva do erro do último treino: um ponto por passo resumido.
export function desenharCurva(canvas, valores, cores) {
  const g = canvas.getContext("2d");
  const escala = window.devicePixelRatio || 1;
  const largura = canvas.clientWidth, altura = canvas.clientHeight;
  if (!largura || !altura) return;
  canvas.width = Math.round(largura * escala);
  canvas.height = Math.round(altura * escala);
  g.setTransform(escala, 0, 0, escala, 0, 0);
  g.clearRect(0, 0, largura, altura);
  if (!valores || valores.length < 2) return;
  const max = Math.max(...valores), min = Math.min(...valores, 0);
  const x = (i) => 4 + (i / (valores.length - 1)) * (largura - 8);
  const y = (v) => altura - 14 - ((v - min) / Math.max(max - min, 1)) * (altura - 24);
  g.strokeStyle = cores.fio;
  g.lineWidth = 1;
  g.beginPath();
  g.moveTo(4, altura - 14);
  g.lineTo(largura - 4, altura - 14);
  g.stroke();
  g.strokeStyle = cores.tinta;
  g.lineWidth = 1.5;
  g.beginPath();
  valores.forEach((v, i) => (i ? g.lineTo(x(i), y(v)) : g.moveTo(x(i), y(v))));
  g.stroke();
  g.fillStyle = cores.fraca;
  g.font = `10px ${cores.fonte}`;
  g.textAlign = "left";
  g.fillText("início do treino", 4, altura - 3);
  g.textAlign = "right";
  g.fillText("fim", largura - 4, altura - 3);
}
