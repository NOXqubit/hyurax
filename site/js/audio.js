// Trilha original da Auron, sintetizada no navegador com Web Audio.
//
// Por que sintetizada e nao um arquivo de musica: musica de terceiro exige
// licenca, e um MP3 de uma faixa conhecida num site publico e uso indevido.
// Isto aqui e composicao propria do projeto, gerada por codigo: nao carrega
// arquivo nenhum, nao depende de ninguem, e o projeto pode usar onde quiser.
//
// Clima: phonk lento ("slowed"), 62 batidas por minuto, la menor. Grave 808,
// cowbell desafinado, chimbal, almofada de cordas ao fundo e chiado de vinil.
// Nada disso e amostra gravada: sao osciladores e ruido.

const BPM = 62;
const BATIDA = 60 / BPM;          // 0,968 s
const COMPASSO = BATIDA * 4;

// La menor: i - VI - VII - v. Notas em hertz, oitavas graves.
const HARMONIA = [
  { baixo: 55.00, acorde: [110.00, 130.81, 164.81] },   // Am
  { baixo: 43.65, acorde: [87.31, 130.81, 174.61] },    // F
  { baixo: 49.00, acorde: [98.00, 146.83, 196.00] },    // G
  { baixo: 41.20, acorde: [82.41, 123.47, 164.81] },    // Em
];
// Melodia de cowbell, em semicolcheias dentro do compasso (posicao, nota).
const MELODIA = [
  [0, 440.00], [1.5, 523.25], [2, 392.00], [3, 329.63],
  [4, 440.00], [5.5, 587.33], [6, 523.25], [7, 392.00],
  [8, 329.63], [9.5, 392.00], [10, 440.00], [11, 523.25],
  [12, 587.33], [13.5, 523.25], [14, 440.00], [15, 392.00],
];

function ruido(ctx, segundos) {
  const buffer = ctx.createBuffer(1, Math.floor(ctx.sampleRate * segundos), ctx.sampleRate);
  const dados = buffer.getChannelData(0);
  for (let i = 0; i < dados.length; i++) dados[i] = Math.random() * 2 - 1;
  return buffer;
}

function respostaDeSala(ctx, segundos = 2.6, decaimento = 3.2) {
  const n = Math.floor(ctx.sampleRate * segundos);
  const buffer = ctx.createBuffer(2, n, ctx.sampleRate);
  for (let canal = 0; canal < 2; canal++) {
    const dados = buffer.getChannelData(canal);
    for (let i = 0; i < n; i++) {
      dados[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / n, decaimento);
    }
  }
  return buffer;
}

function curvaDeSaturacao(quantidade = 12) {
  const n = 1024, curva = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const x = (i * 2) / n - 1;
    curva[i] = ((1 + quantidade) * x) / (1 + quantidade * Math.abs(x));
  }
  return curva;
}

export function criarAudio() {
  const Ctx = window.AudioContext || window.webkitAudioContext;
  if (!Ctx) return null;

  let ctx = null, mestre = null, sala = null, duque = null, chiado = null;
  let destinoGravacao = null;
  let rodando = false, relogio = 0, proximoCompasso = 0, compasso = 0;
  let volumeAlvo = 0.8, aoBatida = null, aoCompasso = null;
  let ruidoCurto = null, saturacao = null;

  function montar() {
    if (ctx) return;
    ctx = new Ctx();
    saturacao = curvaDeSaturacao();
    ruidoCurto = ruido(ctx, 2);

    mestre = ctx.createGain();
    mestre.gain.value = 0;
    const compressor = ctx.createDynamicsCompressor();
    compressor.threshold.value = -14;
    compressor.ratio.value = 4;
    compressor.attack.value = 0.006;
    compressor.release.value = 0.25;
    mestre.connect(compressor).connect(ctx.destination);

    destinoGravacao = ctx.createMediaStreamDestination();
    compressor.connect(destinoGravacao);

    // Sala: reverberacao longa, como toda faixa lenta tem.
    sala = ctx.createConvolver();
    sala.buffer = respostaDeSala(ctx);
    const envioSala = ctx.createGain();
    envioSala.gain.value = 0.9;
    sala.connect(envioSala).connect(mestre);

    // Barramento que abaixa a cada bumbo: o "pump" do genero.
    duque = ctx.createGain();
    duque.gain.value = 1;
    duque.connect(mestre);
    duque.connect(sala);

    // Chiado de vinil contínuo, bem baixo.
    const fonte = ctx.createBufferSource();
    fonte.buffer = ruido(ctx, 4);
    fonte.loop = true;
    const filtroChiado = ctx.createBiquadFilter();
    filtroChiado.type = "bandpass";
    filtroChiado.frequency.value = 2600;
    filtroChiado.Q.value = 0.7;
    chiado = ctx.createGain();
    chiado.gain.value = 0.012;
    fonte.connect(filtroChiado).connect(chiado).connect(mestre);
    fonte.start();
  }

  // ----------------------------------------------------------- instrumentos
  function bumbo(t) {
    const osc = ctx.createOscillator();
    const g = ctx.createGain();
    const forma = ctx.createWaveShaper();
    forma.curve = saturacao;
    osc.type = "sine";
    osc.frequency.setValueAtTime(120, t);
    osc.frequency.exponentialRampToValueAtTime(38, t + 0.08);
    osc.frequency.exponentialRampToValueAtTime(31, t + 0.9);
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(1.0, t + 0.012);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 1.15);
    osc.connect(forma).connect(g).connect(duque);
    osc.start(t);
    osc.stop(t + 1.2);

    // abaixa o resto por um instante
    duque.gain.cancelScheduledValues(t);
    duque.gain.setValueAtTime(0.42, t);
    duque.gain.linearRampToValueAtTime(1, t + BATIDA * 0.75);
  }

  function caixa(t) {
    const fonte = ctx.createBufferSource();
    fonte.buffer = ruidoCurto;
    const filtro = ctx.createBiquadFilter();
    filtro.type = "bandpass";
    filtro.frequency.value = 1900;
    filtro.Q.value = 1.1;
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.55, t);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.22);
    fonte.connect(filtro).connect(g).connect(duque);
    fonte.start(t);
    fonte.stop(t + 0.3);
  }

  function chimbal(t, aberto = false) {
    const fonte = ctx.createBufferSource();
    fonte.buffer = ruidoCurto;
    const filtro = ctx.createBiquadFilter();
    filtro.type = "highpass";
    filtro.frequency.value = 7200;
    const g = ctx.createGain();
    const dur = aberto ? 0.26 : 0.05;
    g.gain.setValueAtTime(aberto ? 0.18 : 0.12, t);
    g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
    fonte.connect(filtro).connect(g).connect(duque);
    fonte.start(t);
    fonte.stop(t + dur + 0.05);
  }

  function cowbell(t, frequencia, forte = 1) {
    // Dois quadrados desafinados num passa-banda: o cowbell do phonk.
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.22 * forte, t + 0.01);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.42);
    const filtro = ctx.createBiquadFilter();
    filtro.type = "bandpass";
    filtro.frequency.value = frequencia * 1.6;
    filtro.Q.value = 2.4;
    for (const razao of [1, 1.5031]) {
      const osc = ctx.createOscillator();
      osc.type = "square";
      osc.frequency.value = frequencia * razao;
      osc.connect(filtro);
      osc.start(t);
      osc.stop(t + 0.45);
    }
    filtro.connect(g);
    g.connect(duque);
    const envio = ctx.createGain();
    envio.gain.value = 0.5;
    g.connect(envio).connect(sala);
  }

  function almofada(t, notas) {
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.linearRampToValueAtTime(0.075, t + COMPASSO * 0.35);
    g.gain.linearRampToValueAtTime(0.0001, t + COMPASSO * 1.02);
    const filtro = ctx.createBiquadFilter();
    filtro.type = "lowpass";
    filtro.frequency.setValueAtTime(420, t);
    filtro.frequency.linearRampToValueAtTime(900, t + COMPASSO);
    for (const nota of notas) {
      for (const desafino of [-6, 6]) {
        const osc = ctx.createOscillator();
        osc.type = "sawtooth";
        osc.frequency.value = nota * 2;
        osc.detune.value = desafino;
        osc.connect(filtro);
        osc.start(t);
        osc.stop(t + COMPASSO * 1.05);
      }
    }
    filtro.connect(g).connect(sala);
    const seco = ctx.createGain();
    seco.gain.value = 0.5;
    g.connect(seco).connect(duque);
  }

  function baixo(t, frequencia) {
    const osc = ctx.createOscillator();
    const g = ctx.createGain();
    osc.type = "triangle";
    osc.frequency.value = frequencia;
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.5, t + 0.05);
    g.gain.setValueAtTime(0.5, t + COMPASSO * 0.7);
    g.gain.exponentialRampToValueAtTime(0.0001, t + COMPASSO * 0.98);
    osc.connect(g).connect(duque);
    osc.start(t);
    osc.stop(t + COMPASSO);
  }

  /** Chiado que sobe, para anunciar um corte. */
  function subida(segundos = COMPASSO) {
    if (!ctx || !rodando) return;
    const t = ctx.currentTime;
    const fonte = ctx.createBufferSource();
    fonte.buffer = ruido(ctx, segundos + 0.2);
    const filtro = ctx.createBiquadFilter();
    filtro.type = "bandpass";
    filtro.Q.value = 1.4;
    filtro.frequency.setValueAtTime(300, t);
    filtro.frequency.exponentialRampToValueAtTime(7000, t + segundos);
    const g = ctx.createGain();
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.28, t + segundos * 0.9);
    g.gain.exponentialRampToValueAtTime(0.0001, t + segundos + 0.15);
    fonte.connect(filtro).connect(g).connect(mestre);
    const envio = ctx.createGain();
    envio.gain.value = 0.6;
    g.connect(envio).connect(sala);
    fonte.start(t);
    fonte.stop(t + segundos + 0.2);
  }

  /** Impacto grave, para o primeiro quadro de uma cena. */
  function impacto() {
    if (!ctx || !rodando) return;
    const t = ctx.currentTime;
    const osc = ctx.createOscillator();
    const g = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(90, t);
    osc.frequency.exponentialRampToValueAtTime(28, t + 1.4);
    g.gain.setValueAtTime(0.9, t);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 1.6);
    osc.connect(g).connect(mestre);
    const envio = ctx.createGain();
    envio.gain.value = 0.8;
    g.connect(envio).connect(sala);
    osc.start(t);
    osc.stop(t + 1.7);
  }

  // ------------------------------------------------------------- sequenciador
  function agendarCompasso(t, indice) {
    const passo = HARMONIA[indice % HARMONIA.length];
    const semicolcheia = BATIDA / 4;

    baixo(t, passo.baixo);
    almofada(t, passo.acorde);
    bumbo(t);
    bumbo(t + BATIDA * 2);
    if (indice % 2 === 1) bumbo(t + BATIDA * 3.5);
    caixa(t + BATIDA);
    caixa(t + BATIDA * 3);

    for (let i = 0; i < 8; i++) {
      chimbal(t + i * BATIDA * 0.5, i === 7 && indice % 4 === 3);
    }
    if (indice % 4 === 3) {
      for (let i = 0; i < 6; i++) chimbal(t + BATIDA * 3 + i * semicolcheia * 0.66);
    }
    for (const [posicao, nota] of MELODIA) {
      cowbell(t + posicao * semicolcheia, nota, posicao % 4 === 0 ? 1 : 0.7);
    }
    if (aoCompasso) aoCompasso(indice, t);
    if (aoBatida) for (let b = 0; b < 4; b++) aoBatida(indice * 4 + b, t + b * BATIDA);
  }

  let temporizador = 0;
  function bombear() {
    if (!rodando) return;
    while (proximoCompasso < ctx.currentTime + 0.35) {
      agendarCompasso(proximoCompasso, compasso);
      proximoCompasso += COMPASSO;
      compasso++;
    }
  }

  async function iniciar() {
    montar();
    if (ctx.state === "suspended") await ctx.resume();
    return ctx.state === "running";
  }

  function tocar({ volume = 0.8, batida = null, compassoCb = null } = {}) {
    volumeAlvo = volume;
    aoBatida = batida;
    aoCompasso = compassoCb;
    if (!ctx) return;
    if (!rodando) {
      rodando = true;
      relogio = ctx.currentTime;
      proximoCompasso = relogio + 0.08;
      compasso = 0;
      temporizador = setInterval(bombear, 60);
      bombear();
    }
    mestre.gain.cancelScheduledValues(ctx.currentTime);
    mestre.gain.setValueAtTime(mestre.gain.value, ctx.currentTime);
    mestre.gain.linearRampToValueAtTime(volumeAlvo, ctx.currentTime + 1.2);
  }

  function parar({ suave = true } = {}) {
    if (!ctx || !rodando) return;
    const t = ctx.currentTime;
    mestre.gain.cancelScheduledValues(t);
    mestre.gain.setValueAtTime(mestre.gain.value, t);
    mestre.gain.linearRampToValueAtTime(0, t + (suave ? 1.4 : 0.08));
    clearTimeout(temporizador);
    setTimeout(() => {
      clearInterval(temporizador);
      rodando = false;
    }, suave ? 1500 : 120);
  }

  function volume(v) {
    volumeAlvo = v;
    if (!ctx) return;
    mestre.gain.cancelScheduledValues(ctx.currentTime);
    mestre.gain.setValueAtTime(mestre.gain.value, ctx.currentTime);
    mestre.gain.linearRampToValueAtTime(v, ctx.currentTime + 0.6);
  }

  return {
    iniciar, tocar, parar, volume, subida, impacto,
    get tocando() { return rodando; },
    get compasso() { return COMPASSO; },
    get batida() { return BATIDA; },
    get bpm() { return BPM; },
    tempo: () => (ctx ? ctx.currentTime : 0),
    trilhaDeGravacao: () => (destinoGravacao ? destinoGravacao.stream.getAudioTracks()[0] : null),
  };
}
