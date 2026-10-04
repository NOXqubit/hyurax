// Hyurax / Ultrax — os visualizadores: amostra real do motor → cena 3D.
//
// Cada tipo de trabalho tem o seu. Todos seguem a mesma regra:
// - a entrada é só a amostra que o motor entregou (hyurax_ultrax::observador),
//   ou, na GPU, o que a própria GPU devolveu;
// - o que falta (linhas que não foram amostradas, coordenadas 3D de moléculas)
//   aparece como falta na legenda, nunca preenchido com número inventado;
// - a cena só muda quando chega amostra nova.

import { Motor } from "./motor.js";
import { obter } from "../api.js";
import { arranjar, lerSmiles } from "../moleculas.js";
import { fmt, compacto } from "../util.js";

// tons: CPU em branco, GPU em cinza-azulado, o resto na escala de cinza
const CPU = [0.93, 0.93, 0.9];
const GPU = [0.62, 0.74, 0.95];
const APAGADO = [0.36, 0.36, 0.38];
const tom = (base, t, a = 1) => [base[0] * (0.35 + 0.65 * t), base[1] * (0.35 + 0.65 * t), base[2] * (0.35 + 0.65 * t), a];
const corDo = (recurso) => (recurso === "GPU" ? GPU : CPU);
const lim = (x) => Math.max(0, Math.min(1, x));

function caixa(lista, x, y, z, lx, ly, lz, c) {
  lista.push(x, y, z, lx, Math.max(ly, 0.002), lz, c[0], c[1], c[2], c[3] ?? 1);
}
function linha(lista, a, b, c) {
  lista.push(a[0], a[1], a[2], c[0], c[1], c[2], c[3] ?? 1, b[0], b[1], b[2], c[0], c[1], c[2], c[3] ?? 1);
}
function ponto(lista, p, c) {
  lista.push(p[0], p[1], p[2], c[0], c[1], c[2], c[3] ?? 1);
}
function triangulo(lista, a, b, c, cor) {
  const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
  const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
  for (const p of [a, b, c]) lista.push(p[0], p[1], p[2], n[0], n[1], n[2], cor[0], cor[1], cor[2], cor[3] ?? 1);
}
function base(linhas) {
  // o chão: um quadro de -1 a 1, só para dar referência de espaço
  const c = [0.2, 0.2, 0.22, 1];
  const q = [[-1, 0, -1], [1, 0, -1], [1, 0, 1], [-1, 0, 1]];
  for (let i = 0; i < 4; i++) linha(linhas, q[i], q[(i + 1) % 4], c);
}

// ---------------------------------------------------------------------------
// Um visualizador por tipo (o nome é o do gabarito, igual ao da amostra)
// ---------------------------------------------------------------------------

const MAX_FAIXAS = 96;

const VIS = {
  matrix: {
    nome: "Multiplicação de matrizes C = A·B",
    novo: () => ({ linhas: new Map(), min: Infinity, max: -Infinity, lado: 0, aCada: 1 }),
    somar(e, a) {
      e.lado = a.lado;
      e.aCada = a.a_cada;
      const faixa = Math.floor((a.linha * Math.min(MAX_FAIXAS, a.lado)) / a.lado);
      e.linhas.set(faixa, { linha: a.linha, valores: a.valores });
      for (const v of a.valores) {
        if (v > e.max) e.max = v;
        if (v < e.min) e.min = v;
      }
      e.amostradas = (e.amostradas || 0) + 1;
      e.ultima = a.linha;
    },
    geometria(e, recurso) {
      const caixas = [];
      const linhas = [];
      base(linhas);
      const faixas = Math.min(MAX_FAIXAS, e.lado || 1);
      for (const [f, { valores }] of e.linhas) {
        const z = -1 + ((f + 0.5) * 2) / faixas;
        const cols = valores.length;
        // altura relativa ao intervalo visto: entradas de C costumam ser
        // parecidas entre si, e a variação é o que a tela precisa mostrar
        const faixaValores = e.max - e.min || 1;
        valores.forEach((v, j) => {
          const t = lim((v - e.min) / faixaValores);
          caixa(caixas, -1 + ((j + 0.5) * 2) / cols, 0, z, (2 / cols) * 0.86, 0.04 + t * 0.86, (2 / faixas) * 0.86, tom(corDo(recurso), t));
        });
      }
      return { caixas, linhas };
    },
    legenda: (e, recurso) => [
      `lado ${e.lado} · cada fileira é uma linha de C que o motor ${recurso === "GPU" ? "da GPU devolveu" : "acabou de calcular"}`,
      `altura = valor da entrada de C entre o menor (${compacto(e.min)}) e o maior (${compacto(e.max)}) visto até agora`,
      `${e.linhas.size} fileira(s) na tela, ${e.amostradas} linha(s) amostrada(s) de ${e.lado}` +
        (e.aCada > 1 ? ` · uma coluna a cada ${e.aCada}` : "") + " · as linhas entre amostras não vêm para a tela",
    ],
  },

  knapsack: {
    nome: "Mochila (programação dinâmica)",
    novo: () => ({ linhas: [], max: 1 }),
    somar(e, a) {
      e.itens = a.itens;
      e.capacidade = a.capacidade;
      e.aCada = a.a_cada;
      e.linhas.push({ item: a.item, melhor: a.melhor });
      if (e.linhas.length > 64) e.linhas.shift();
      for (const v of a.melhor) if (v > e.max) e.max = v;
      e.ultimo = a;
    },
    geometria(e, recurso) {
      const caixas = [];
      const linhas = [];
      base(linhas);
      const n = Math.max(1, e.itens || 1);
      for (const { item, melhor } of e.linhas) {
        const z = -1 + ((item + 0.5) * 2) / n;
        melhor.forEach((v, j) => {
          const t = lim(v / e.max);
          caixa(caixas, -1 + ((j + 0.5) * 2) / melhor.length, 0, z, (2 / melhor.length) * 0.8, t * 0.9, Math.max(0.01, (2 / n) * 0.8), tom(corDo(recurso), t));
        });
      }
      return { caixas, linhas };
    },
    legenda: (e) => [
      `${e.itens} itens, capacidade ${fmt(e.capacidade)} · cada fileira é a tabela do ótimo depois de um item`,
      `x = capacidade (uma a cada ${e.aCada}), altura = melhor valor possível com os itens vistos`,
      e.ultimo ? `último item: peso ${e.ultimo.peso}, valor ${e.ultimo.valor}` : "",
    ],
  },

  diffusion: {
    nome: "Difusão numa grade",
    novo: () => ({}),
    somar(e, a) {
      Object.assign(e, { lado: a.lado, passo: a.passo, passos: a.passos, grade: a.grade, n: a.lado_amostra, aCada: a.a_cada });
    },
    geometria(e, recurso) {
      const malha = [];
      const linhas = [];
      if (!e.grade) return { malha, linhas };
      let min = Infinity;
      let max = -Infinity;
      for (const v of e.grade) { if (v < min) min = v; if (v > max) max = v; }
      const faixa = max - min || 1;
      const n = e.n;
      const p = (i, j) => {
        const v = e.grade[i * n + j];
        return [-1 + (j * 2) / (n - 1), ((v - min) / faixa) * 0.7, -1 + (i * 2) / (n - 1)];
      };
      for (let i = 0; i < n - 1; i++) {
        for (let j = 0; j < n - 1; j++) {
          const a = p(i, j), b = p(i, j + 1), c = p(i + 1, j), d = p(i + 1, j + 1);
          const t = lim((a[1] + b[1] + c[1] + d[1]) / 4 / 0.7);
          triangulo(malha, a, c, b, tom(corDo(recurso), t));
          triangulo(malha, b, c, d, tom(corDo(recurso), t));
        }
      }
      base(linhas);
      return { malha, linhas };
    },
    legenda: (e) => [
      `grade ${e.lado}×${e.lado}, passo ${e.passo} de ${e.passos} · superfície = valor de cada célula agora`,
      e.aCada > 1 ? `um ponto a cada ${e.aCada} linhas e colunas (${e.n}×${e.n} na tela)` : "a grade inteira",
    ],
  },

  "ai-training": {
    nome: "Treino de IA (solubilidade de moléculas reais)",
    novo: () => ({ perdas: [] }),
    somar(e, a) {
      e.passo = a.passo;
      e.passos = a.passos;
      e.pesos = a.pesos_saida;
      e.perdas.push([a.passo, a.perda_q12]);
    },
    geometria(e, recurso) {
      const caixas = [];
      const linhas = [];
      base(linhas);
      const pesos = e.pesos || [];
      const maxAbs = Math.max(1, ...pesos.map(Math.abs));
      pesos.forEach((w, j) => {
        const t = lim(Math.abs(w) / maxAbs);
        const c = w >= 0 ? tom(corDo(recurso), 0.4 + 0.6 * t) : tom(APAGADO, 0.6 + 0.4 * t);
        caixa(caixas, -0.9 + ((j + 0.5) * 1.8) / pesos.length, 0, 0.55, (1.8 / pesos.length) * 0.7, t * 0.8, 0.12, c);
      });
      const maxPerda = Math.max(1, ...e.perdas.map((p) => p[1]));
      for (let k = 1; k < e.perdas.length; k++) {
        const [pa, la] = e.perdas[k - 1];
        const [pb, lb] = e.perdas[k];
        const x = (s) => -0.9 + (s / Math.max(1, e.passos)) * 1.8;
        linha(linhas, [x(pa), (la / maxPerda) * 0.8, -0.6], [x(pb), (lb / maxPerda) * 0.8, -0.6], [0.93, 0.93, 0.9, 1]);
      }
      return { caixas, linhas };
    },
    legenda: (e) => [
      `passo ${e.passo} de ${e.passos} · na frente: os ${e.pesos?.length ?? 0} pesos da camada oculta para a saída (claros positivos, escuros negativos)`,
      `ao fundo: a perda do lote em cada passo amostrado (${e.perdas.length} ponto(s)) — ela cai quando o modelo aprende`,
    ],
  },

  "population-genetics": {
    nome: "Genética de populações (Wright-Fisher)",
    novo: () => ({ geracoes: [] }),
    somar(e, a) {
      e.total = a.geracoes;
      e.copias = a.copias;
      e.geracoes.push({ g: a.geracao, f: a.contagens.map((k) => k / a.copias) });
    },
    geometria(e, recurso) {
      const linhas = [];
      const pontos = [];
      base(linhas);
      const loci = e.geracoes[0]?.f.length || 0;
      const x = (g) => -1 + (g / Math.max(1, e.total)) * 2;
      for (let l = 0; l < loci; l++) {
        const z = -1 + ((l + 0.5) * 2) / loci;
        for (let k = 0; k < e.geracoes.length; k++) {
          const { g, f } = e.geracoes[k];
          const p = [x(g), f[l] * 0.9, z];
          ponto(pontos, p, tom(corDo(recurso), 0.5 + 0.5 * f[l]));
          if (k > 0) {
            const ant = e.geracoes[k - 1];
            linha(linhas, [x(ant.g), ant.f[l] * 0.9, z], p, tom(corDo(recurso), 0.7));
          }
        }
      }
      return { linhas, pontos, tamanhoPonto: 5 };
    },
    legenda: (e) => {
      const u = e.geracoes[e.geracoes.length - 1];
      const media = u ? u.f.reduce((s, x) => s + x, 0) / u.f.length : 0;
      return [
        `x = geração (${u?.g ?? 0} de ${e.total}), altura = frequência do alelo A, cada fileira é um locus (${u?.f.length ?? 0})`,
        `frequência média agora ${fmt(media, 3)} · ${e.geracoes.length} geração(ões) amostrada(s); entre elas, a linha só liga os pontos`,
        `${e.copias / 2} indivíduos (${e.copias} cópias do gene)`,
      ];
    },
  },

  "crop-breeding": {
    nome: "Melhoramento de plantas (seleção por fenótipo)",
    novo: () => ({ geracoes: [] }),
    somar(e, a) {
      e.total = a.geracoes;
      e.plantas = a.plantas;
      e.aCada = a.a_cada;
      e.geracoes.push({ t: a.geracao, pontos: a.pontos });
      if (e.geracoes.length > 24) e.geracoes.shift();
    },
    geometria(e, recurso) {
      const pontos = [];
      const linhas = [];
      base(linhas);
      let gMin = Infinity, gMax = -Infinity, fMin = Infinity, fMax = -Infinity;
      for (const { pontos: ps } of e.geracoes) for (const [g, f] of ps) {
        if (g < gMin) gMin = g; if (g > gMax) gMax = g; if (f < fMin) fMin = f; if (f > fMax) fMax = f;
      }
      const gx = (g) => -1 + ((g - gMin) / (gMax - gMin || 1)) * 2;
      const fy = (f) => ((f - fMin) / (fMax - fMin || 1)) * 0.9;
      for (const { t, pontos: ps } of e.geracoes) {
        const z = -1 + (t / Math.max(1, e.total)) * 2;
        for (const [g, f, sel] of ps) ponto(pontos, [gx(g), fy(f), z], sel ? tom(corDo(recurso), 1) : tom(APAGADO, 0.8, 0.8));
      }
      return { pontos, linhas, tamanhoPonto: 4 };
    },
    legenda: (e) => {
      const u = e.geracoes[e.geracoes.length - 1];
      const sel = u ? u.pontos.filter((p) => p[2]).length : 0;
      return [
        `${e.plantas} plantas · x = valor genético, altura = fenótipo (genético + ambiente), profundidade = geração (${u?.t ?? 0} de ${e.total})`,
        `pontos claros: as selecionadas para cruzar (${sel} na última amostra)` + (e.aCada > 1 ? ` · uma planta a cada ${e.aCada}` : ""),
      ];
    },
  },

  routing: {
    nome: "Rotas de entrega (2-opt)",
    novo: () => ({ rotas: [], cidades: null }),
    somar(e, a, ctx) {
      e.passos = a.passos;
      e.inicial = a.inicial;
      e.rotas.push({ passada: a.passada, comprimento: a.comprimento, rota: a.rota });
      if (e.rotas.length > 10) e.rotas.shift();
      const instancia = ctx.parametros?.[0];
      const n = ctx.tamanho;
      if (!e.cidades && !e.pedindo && instancia !== undefined && n) {
        e.pedindo = true;
        obter(`/ciencia/rotas/${instancia}/${n}`).then((r) => {
          if (r.ok) {
            e.cidades = r.dados.cidades;
            e.ladoMapa = r.dados.lado;
            e.redesenhar?.();
          }
        });
      }
    },
    geometria(e, recurso) {
      const linhas = [];
      const pontos = [];
      if (!e.cidades) return { linhas, pontos };
      const L = e.ladoMapa || 1;
      const xz = (c) => [-1 + (c[0] / L) * 2, -1 + (c[1] / L) * 2];
      for (const c of e.cidades) {
        const [x, z] = xz(c);
        ponto(pontos, [x, 0, z], [0.7, 0.7, 0.72, 1]);
      }
      e.rotas.forEach(({ rota, passada }, k) => {
        const ultima = k === e.rotas.length - 1;
        const y = (passada / Math.max(1, e.passos)) * 0.8;
        const c = ultima ? tom(corDo(recurso), 1) : tom(APAGADO, 0.5 + (0.5 * k) / e.rotas.length, 0.6);
        for (let i = 0; i < rota.length; i++) {
          const a = e.cidades[rota[i]];
          const b = e.cidades[rota[(i + 1) % rota.length]];
          if (!a || !b) continue;
          const [ax, az] = xz(a);
          const [bx, bz] = xz(b);
          linha(linhas, [ax, y, az], [bx, y, bz], c);
        }
      });
      return { linhas, pontos, tamanhoPonto: 5 };
    },
    legenda: (e) => {
      const u = e.rotas[e.rotas.length - 1];
      return [
        `${e.cidades?.length ?? "…"} cidades (coordenadas reais da instância) · cada camada é a rota depois de uma passada; mais alta = mais recente`,
        u ? `passada ${u.passada} de até ${e.passos}: comprimento ${fmt(u.comprimento)} (partida ${fmt(e.inicial)}, ${fmt((1 - u.comprimento / e.inicial) * 100, 1)}% menor)` : "",
      ];
    },
  },

  "molecular-screening": {
    nome: "Triagem de moléculas (AqSolDB)",
    novo: () => ({ moleculas: [], atual: null }),
    somar(e, a) {
      e.moleculas.push(a);
      if (e.moleculas.length > 600) e.moleculas.shift();
      if (!e.pedindo) {
        e.pedindo = true;
        obter(`/ciencia/molecula/${a.indice}`).then((r) => {
          e.pedindo = false;
          if (!r.ok) return;
          const grafo = lerSmiles(r.dados.smiles);
          e.atual = { ...r.dados, grafo, pos: arranjar(grafo, r.dados.smiles), amostra: a };
          e.redesenhar?.();
        });
      }
    },
    geometria(e, recurso) {
      const pontos = [];
      const linhas = [];
      base(linhas);
      const notaMax = Math.max(1, ...e.moleculas.map((m) => m.nota));
      const n = e.moleculas.length;
      e.moleculas.forEach((m, k) => {
        const x = lim((m.previsto_mili / 1000 + 12) / 16) * 2 - 1;
        const z = -1 + ((k + 0.5) * 2) / Math.max(n, 1);
        ponto(pontos, [x, (m.nota / notaMax) * 0.6, z], m.aprovada ? tom(corDo(recurso), 1) : tom(APAGADO, 0.7, 0.7));
      });
      // a molécula em avaliação, em cima: grafo real do SMILES, arranjo plano
      const at = e.atual;
      if (at?.pos?.length) {
        let mx = 0;
        for (const { x, y } of at.pos) mx = Math.max(mx, Math.abs(x), Math.abs(y));
        const s = 0.55 / (mx || 1);
        const p = (i) => [at.pos[i].x * s, 1.05, at.pos[i].y * s];
        for (const l of at.grafo.ligacoes) linha(linhas, p(l.a), p(l.b), [0.8, 0.8, 0.82, 1]);
        at.grafo.atomos.forEach((a, i) => ponto(pontos, p(i), a.el === "C" || a.el === "c" ? [0.75, 0.75, 0.75, 1] : [1, 1, 1, 1]));
      }
      return { pontos, linhas, tamanhoPonto: 5 };
    },
    legenda: (e) => {
      const at = e.atual;
      const ap = e.moleculas.filter((m) => m.aprovada).length;
      return [
        `embaixo: ${e.moleculas.length} molécula(s) triada(s) · x = log S previsto pelo modelo, altura = nota, profundidade = ordem; claras passaram nos 7 filtros (${ap})`,
        at
          ? `em cima: ${at.nome || at.id} (${at.formula}) · log S medido ${fmt(at.logs_medido_mili / 1000, 2)}, previsto ${fmt(at.amostra.previsto_mili / 1000, 2)}`
          : "em cima: a molécula em avaliação (carregando o SMILES)",
        "estrutura desenhada do SMILES real, com arranjo plano por forças · 3D aproximado na tela Moléculas; conformação otimizada: PENDENTE",
      ];
    },
  },
};

/** Tipos de trabalho sem motor: nada é desenhado. */
export const SEM_MOTOR = ["materiais", "energia", "meio ambiente", "regeneração"];

// ---------------------------------------------------------------------------
// A cena: guarda o estado de cada tarefa que mandou amostra e desenha a em foco
// ---------------------------------------------------------------------------

export class Cena {
  /**
   * @param {HTMLCanvasElement} canvas
   * @param {(info: object) => void} aoMudar recebe o resumo do que está na tela
   */
  constructor(canvas, aoMudar) {
    this.motor = new Motor(canvas);
    this.aoMudar = aoMudar;
    this.tarefas = new Map();
    this.foco = null;
    this.seguir = true;
    this.pendente = false;
  }

  get ok() {
    return this.motor.ok;
  }

  /** Uma amostra: { contexto, amostra }. */
  receber({ contexto, amostra }) {
    const vis = VIS[amostra?.tipo];
    if (!vis || !contexto) return;
    const chave = `${contexto.recurso}:${contexto.tarefa}`;
    let t = this.tarefas.get(chave);
    if (!t || t.tipo !== amostra.tipo) {
      t = { chave, tipo: amostra.tipo, contexto, estado: vis.novo(), amostras: 0, primeira: Date.now(), verificacao: null };
      t.estado.redesenhar = () => {
        if (this.foco === chave) this.redesenhar();
      };
      this.tarefas.set(chave, t);
      // esquece as tarefas antigas
      if (this.tarefas.size > 24) {
        const velha = [...this.tarefas.values()].sort((a, b) => a.ultima - b.ultima)[0];
        if (velha && velha.chave !== this.foco) this.tarefas.delete(velha.chave);
      }
    }
    t.contexto = contexto;
    t.ultima = Date.now();
    t.amostras++;
    vis.somar(t.estado, amostra, contexto);
    // seguindo: fica na tarefa em foco enquanto ela manda amostra; só troca
    // quando ela para (terminou ou ficou 3 s quieta)
    if (!this.foco || (this.seguir && this.foco !== chave && !this.ativa(this.foco))) this.foco = chave;
    if (this.foco === chave) this.redesenhar();
  }

  /** A tarefa ainda está mandando amostra e não terminou. */
  ativa(chave) {
    const t = this.tarefas.get(chave);
    return !!t && !t.encerrada && Date.now() - t.ultima < 3000;
  }

  /** Um passo de tarefa do ULTRAX (a verificação aparece na cena). */
  passoDeTarefa(ev) {
    for (const recurso of ["CPU", "GPU"]) {
      const t = this.tarefas.get(`${recurso}:${ev.tarefa}`);
      if (!t) continue;
      if (/VERIFICATION|SETTLEMENT|RESULT SUBMITTED|EXPIRED|CANCELLED|ABANDONED/.test(ev.evento)) {
        t.verificacao = { evento: ev.evento, detalhe: ev.detalhe };
        if (/RESULT SUBMITTED|SETTLEMENT|EXPIRED|CANCELLED|ABANDONED|FAILED/.test(ev.evento)) t.encerrada = true;
        if (this.foco === t.chave) this.redesenhar();
      }
    }
  }

  focar(chave) {
    if (!this.tarefas.has(chave)) return;
    this.foco = chave;
    this.seguir = false;
    this.redesenhar();
  }

  seguirAMaisRecente() {
    this.seguir = true;
    const recente = [...this.tarefas.values()].sort((a, b) => b.ultima - a.ultima)[0];
    if (recente) {
      this.foco = recente.chave;
      this.redesenhar();
    }
  }

  lista() {
    return [...this.tarefas.values()].sort((a, b) => b.ultima - a.ultima);
  }

  /** Desenha a tarefa em foco no próximo quadro (vários pedidos viram um). */
  redesenhar() {
    if (this.pendente) return;
    this.pendente = true;
    requestAnimationFrame(() => {
      this.pendente = false;
      const t = this.tarefas.get(this.foco);
      if (!t) return;
      const vis = VIS[t.tipo];
      this.motor.mostrar(vis.geometria(t.estado, t.contexto.recurso));
      this.aoMudar?.({
        chave: t.chave,
        nome: vis.nome,
        tipo: t.tipo,
        contexto: t.contexto,
        amostras: t.amostras,
        legenda: vis.legenda(t.estado, t.contexto.recurso).filter(Boolean),
        verificacao: t.verificacao,
        seguindo: this.seguir,
      });
    });
  }
}
