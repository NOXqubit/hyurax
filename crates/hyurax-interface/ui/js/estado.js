// Hyurax / Ultrax — o estado da tela, alimentado pelo fluxo de eventos do
// núcleo (/api/v1/fluxo, Server-Sent Events).
//
// Nada aqui é inventado: o estado inteiro chega uma vez por segundo
// ("estado"), e o que acontece entre um e outro chega como evento
// ("registro", "tarefa", "ciencia", "amostra", "bloco"). Se o fluxo cai, a
// tela diz que caiu; o navegador reconecta sozinho.

const ouvintes = new Map();

export const estado = {
  /** O último estado inteiro (null até chegar o primeiro). */
  atual: null,
  /** Conectado ao fluxo. */
  conectado: false,
  /** Quando chegou o último evento (ms). */
  ultimoEvento: 0,
  /** Passos de tarefa recentes do ULTRAX, do mais novo para o mais velho. */
  tarefas: [],
  /** Eventos recentes da ciência. */
  ciencia: [],
};

/** Ouve um tipo de evento ("estado", "amostra", "tarefa", "ciencia", "bloco", "registro", "conexao"). */
export function ouvir(tipo, fn) {
  if (!ouvintes.has(tipo)) ouvintes.set(tipo, new Set());
  ouvintes.get(tipo).add(fn);
  return () => ouvintes.get(tipo)?.delete(fn);
}

/** Avisa quem ouve um tipo (também usado pelo backend de GPU da própria página). */
export function avisar(tipo, dados) {
  for (const fn of ouvintes.get(tipo) || []) {
    try {
      fn(dados);
    } catch (e) {
      console.error(`ouvinte de ${tipo}:`, e);
    }
  }
}

function guardar(lista, item, max) {
  lista.unshift(item);
  if (lista.length > max) lista.length = max;
}

/** Liga o fluxo. */
export function conectar() {
  const fonte = new EventSource("/api/v1/fluxo");
  const ler = (e) => {
    estado.ultimoEvento = Date.now();
    try {
      return JSON.parse(e.data);
    } catch {
      return null;
    }
  };
  fonte.addEventListener("open", () => {
    estado.conectado = true;
    avisar("conexao", true);
  });
  fonte.addEventListener("error", () => {
    estado.conectado = false;
    avisar("conexao", false);
  });
  fonte.addEventListener("estado", (e) => {
    const d = ler(e);
    if (!d) return;
    estado.atual = d;
    if (!estado.conectado) {
      estado.conectado = true;
      avisar("conexao", true);
    }
    avisar("estado", d);
  });
  for (const tipo of ["registro", "bloco", "amostra"]) {
    fonte.addEventListener(tipo, (e) => {
      const d = ler(e);
      if (d) avisar(tipo, d);
    });
  }
  fonte.addEventListener("tarefa", (e) => {
    const d = ler(e);
    if (!d) return;
    d.quando = Date.now();
    guardar(estado.tarefas, d, 300);
    avisar("tarefa", d);
  });
  fonte.addEventListener("ciencia", (e) => {
    const d = ler(e);
    if (!d) return;
    guardar(estado.ciencia, d, 300);
    avisar("ciencia", d);
  });
  // sem nenhum evento por 5 s, a conexão está morta mesmo sem erro
  setInterval(() => {
    if (estado.conectado && Date.now() - estado.ultimoEvento > 5000) {
      estado.conectado = false;
      avisar("conexao", false);
    }
  }, 1000);
  return fonte;
}
