// AURON EDIT: sequencia cinematografica em tela cheia, cortada no tempo da
// trilha original do projeto (js/audio.js).
//
// A troca entre cenas imita o corte de personagem do GTA V: a camera sai de
// dentro de um no, dispara para tras, e a proxima cena entra num estalo de
// luz. Tudo e desenhado numa tela 2D que recebe o quadro do mundo 3D por
// cima — e e essa mesma tela que a gravacao captura, entao o que voce ve e
// exatamente o que sai no arquivo.
import { el } from "../ui.js";

// Cada cena dura um numero de compassos. 18 compassos a 62 BPM = ~70 s.
const CENAS = [
  { chave: "no", compassos: 2, mundo: "rede", zoom: [3.4, 1.35] },
  { chave: "rede", compassos: 2, mundo: "rede", zoom: [1.25, 1.0] },
  { chave: "cadeia", compassos: 2, mundo: "cadeia", zoom: [1.3, 1.05] },
  { chave: "fragmento", compassos: 2, mundo: "grade", zoom: [1.35, 1.0], tag: "pesquisa" },
  { chave: "trabalho", compassos: 2, mundo: "nucleo", zoom: [1.2, 1.02], tag: "simulacao" },
  { chave: "malha", compassos: 2, mundo: "malha", zoom: [1.3, 1.0], tag: "planejado" },
  { chave: "escala", compassos: 2, mundo: "global", zoom: [1.1, 1.0], global: [0.05, 1] },
  { chave: "verdade", compassos: 2, mundo: "nucleo", zoom: [1.15, 1.0], tres: true },
  { chave: "marca", compassos: 2, mundo: "logo", zoom: [1.5, 1.0], marca: true },
];

const ease = (x) => 1 - Math.pow(1 - x, 3);

/** Escreve centralizado, encolhendo e quebrando em duas linhas se não couber. */
function escreveCabendo(ctx, texto, x, y, tamanho, fonte, maxLargura) {
  if (!texto) return;
  let tam = tamanho;
  const monta = (t) => `${fonte} ${t}px Archivo, Arial`;
  ctx.font = monta(tam);
  if (ctx.measureText(texto).width <= maxLargura) { ctx.fillText(texto, x, y); return; }

  // primeiro tenta encolher até 72% do tamanho
  while (tam > tamanho * 0.72) {
    tam -= tamanho * 0.04;
    ctx.font = monta(tam);
    if (ctx.measureText(texto).width <= maxLargura) { ctx.fillText(texto, x, y); return; }
  }
  // ainda não coube: parte em duas linhas no espaço mais perto do meio
  const palavras = texto.split(" ");
  let corte = Math.round(palavras.length / 2);
  if (palavras.length < 2) { ctx.fillText(texto, x, y); return; }
  const linhas = [palavras.slice(0, corte).join(" "), palavras.slice(corte).join(" ")];
  while (tam > tamanho * 0.5 && linhas.some((l) => ctx.measureText(l).width > maxLargura)) {
    tam -= tamanho * 0.05;
    ctx.font = monta(tam);
  }
  ctx.fillText(linhas[0], x, y - tam * 0.52);
  ctx.fillText(linhas[1], x, y + tam * 0.52);
}

export function iniciar({ t, audio, mundo, calmo, aoMudarIdioma, restaurarMundo }) {
  const abrirBotao = document.getElementById("abre-edit");
  if (!abrirBotao) return;
  const palco = document.getElementById("palco-edit");
  const tela = document.getElementById("tela-edit");
  const ctx = tela.getContext("2d");
  const botaoGravar = document.getElementById("edit-gravar");
  const botaoFechar = document.getElementById("edit-fechar");
  const aviso = document.getElementById("edit-aviso");
  const mundoCanvas = document.getElementById("mundo");

  let rodando = false, inicio = 0, raf = 0, pulso = 0, cenaAtual = -1;
  let gravador = null, pedacos = [], gravando = false;
  let foco = null;
  const compasso = audio ? audio.compasso : 3.871;
  const total = CENAS.reduce((s, c) => s + c.compassos, 0) * compasso;

  function medir() {
    const dpr = Math.min(window.devicePixelRatio || 1, gravando ? 1 : 1.5);
    tela.width = Math.round(window.innerWidth * dpr);
    tela.height = Math.round(window.innerHeight * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  function cenaEm(tempo) {
    let acumulado = 0;
    for (let i = 0; i < CENAS.length; i++) {
      const dur = CENAS[i].compassos * compasso;
      if (tempo < acumulado + dur) {
        return { i, cena: CENAS[i], p: (tempo - acumulado) / dur, inicio: acumulado };
      }
      acumulado += dur;
    }
    const ultima = CENAS.length - 1;
    return { i: ultima, cena: CENAS[ultima], p: 1, inicio: acumulado };
  }

  function texto(chave, indice) {
    const lista = t("edit.cenas");
    return lista[indice] ? lista[indice][chave] : "";
  }

  // ------------------------------------------------------------- desenho
  function quadro(agora) {
    if (!rodando) return;
    const tempo = (agora - inicio) / 1000;
    if (tempo >= total) { encerrar(); return; }

    const w = tela.width / (tela.width / window.innerWidth);
    const larg = window.innerWidth, alt = window.innerHeight;
    const { i, cena, p } = cenaEm(tempo);
    void w;

    if (i !== cenaAtual) {
      cenaAtual = i;
      mundo.definirModo(cena.mundo);
      if (audio) { audio.impacto(); if (i < CENAS.length - 1) setTimeout(() => audio.subida(compasso * 0.9), (cena.compassos - 1) * compasso * 1000); }
    }
    if (cena.global) {
      const [a, b] = cena.global;
      mundo.definirZoom(a + (b - a) * p);
    }

    // corte: estalo de luz, tremor e separacao de cor logo no primeiro quarto
    const desdeCorte = p * cena.compassos * compasso;
    const estalo = Math.max(0, 1 - desdeCorte / 0.3);
    const separa = Math.max(0, 1 - desdeCorte / 0.9) * 14;
    const tremor = estalo * 10;

    // zoom da cena: comeca fechado e abre (o "sai de dentro do no")
    const [z0, z1] = cena.zoom;
    const escala = z0 + (z1 - z0) * ease(Math.min(1, p * 1.25));
    const bump = 1 + pulso * 0.012;

    ctx.save();
    ctx.fillStyle = "#030303";
    ctx.fillRect(0, 0, larg, alt);

    // o quadro do mundo 3D entra escalado, com leve tremor no corte
    const dx = (Math.random() - 0.5) * tremor, dy = (Math.random() - 0.5) * tremor;
    const k = escala * bump;
    const lw = larg * k, lh = alt * k;
    const ox = (larg - lw) / 2 + dx, oy = (alt - lh) / 2 + dy;
    try {
      if (separa > 0.4) {
        ctx.globalCompositeOperation = "lighter";
        ctx.globalAlpha = 0.85;
        ctx.drawImage(mundoCanvas, ox - separa, oy, lw, lh);
        ctx.drawImage(mundoCanvas, ox + separa, oy, lw, lh);
        ctx.globalAlpha = 1;
        ctx.globalCompositeOperation = "source-over";
      } else {
        ctx.drawImage(mundoCanvas, ox, oy, lw, lh);
      }
    } catch { /* o mundo ainda nao desenhou nada */ }

    // vinheta
    const vinheta = ctx.createRadialGradient(larg / 2, alt * 0.45, alt * 0.25, larg / 2, alt * 0.5, alt * 0.95);
    vinheta.addColorStop(0, "rgba(3,3,3,0)");
    vinheta.addColorStop(1, "rgba(3,3,3,0.82)");
    ctx.fillStyle = vinheta;
    ctx.fillRect(0, 0, larg, alt);

    desenharTexto(i, cena, p, larg, alt, desdeCorte);

    // riscas horizontais discretas
    ctx.globalAlpha = 0.05;
    ctx.fillStyle = "#eceae6";
    for (let y = (tempo * 40) % 4; y < alt; y += 4) ctx.fillRect(0, y, larg, 1);
    ctx.globalAlpha = 1;

    // tarjas de cinema
    const tarja = alt * 0.085 + estalo * 6;
    ctx.fillStyle = "#030303";
    ctx.fillRect(0, 0, larg, tarja);
    ctx.fillRect(0, alt - tarja, larg, tarja);

    // estalo de luz do corte
    if (estalo > 0) {
      ctx.fillStyle = `rgba(232,236,242,${(estalo * 0.3).toFixed(3)})`;
      ctx.fillRect(0, 0, larg, alt);
    }

    // linha de tempo, fina, no pe
    ctx.fillStyle = "rgba(236,234,230,0.25)";
    ctx.fillRect(0, alt - tarja - 2, larg * (tempo / total), 2);

    ctx.restore();
    pulso *= 0.9;
    raf = requestAnimationFrame(quadro);
  }

  function desenharTexto(i, cena, p, larg, alt, desdeCorte) {
    const entrada = Math.min(1, desdeCorte / 0.5);
    const saida = Math.min(1, (1 - p) * 6);
    const alfa = ease(entrada) * Math.min(1, saida);
    if (alfa <= 0.01) return;

    const base = Math.min(larg, alt * 1.6);
    ctx.textAlign = "center";
    ctx.globalAlpha = alfa;

    if (cena.marca) {
      const tam = base * 0.115;
      ctx.fillStyle = "#eceae6";
      escreveCabendo(ctx, "AURON", larg / 2, alt * 0.5, tam, "900", larg * 0.86);
      ctx.font = `500 ${tam * 0.2}px Michroma, Archivo, Arial`;
      ctx.fillStyle = "#f3e2c4";
      ctx.fillText("BUILD THE INFRASTRUCTURE.", larg / 2, alt * 0.5 + tam * 0.62);
      ctx.font = `400 ${tam * 0.17}px Archivo, Arial`;
      ctx.fillStyle = "#8e9199";
      ctx.fillText(texto("sub", i), larg / 2, alt * 0.5 + tam * 1.05);
    } else if (cena.tres) {
      const linhas = texto("linhas", i) || [];
      const tam = base * 0.05;
      linhas.forEach((linha, k) => {
        const chega = Math.min(1, Math.max(0, (desdeCorte - k * 0.55) / 0.4));
        ctx.globalAlpha = alfa * ease(chega);
        ctx.fillStyle = k === linhas.length - 1 ? "#f3e2c4" : "#eceae6";
        escreveCabendo(ctx, linha, larg / 2, alt * 0.42 + k * tam * 1.6, tam, "700", larg * 0.86);
      });
      ctx.globalAlpha = alfa;
    } else {
      const tam = base * 0.072;
      ctx.fillStyle = "#eceae6";
      const deslize = (1 - ease(entrada)) * tam * 0.5;
      escreveCabendo(ctx, texto("titulo", i), larg / 2, alt * 0.52 + deslize, tam, "900", larg * 0.86);

      ctx.fillStyle = "#95989f";
      escreveCabendo(ctx, texto("sub", i), larg / 2, alt * 0.52 + tam * 0.62, tam * 0.26, "400", larg * 0.8);
    }

    if (cena.tag) {
      const rotulo = { simulacao: "SIMULATION", pesquisa: "RESEARCH", planejado: t("estado.planejado").toUpperCase() }[cena.tag];
      const tam = base * 0.016;
      ctx.font = `500 ${tam}px "IBM Plex Mono", monospace`;
      ctx.textAlign = "left";
      ctx.fillStyle = "#f3e2c4";
      ctx.fillText(rotulo, larg * 0.06, alt * 0.14);
      ctx.textAlign = "center";
    }
    ctx.globalAlpha = 1;
  }

  // ------------------------------------------------------------- controle
  async function abrir() {
    foco = document.activeElement;
    palco.hidden = false;
    document.body.style.overflow = "hidden";
    medir();
    cenaAtual = -1;
    aviso.textContent = "";

    if (audio) {
      const pronto = await audio.iniciar();
      if (pronto) audio.tocar({ volume: 0.85, batida: () => { pulso = 1; } });
    }
    // as fontes precisam estar carregadas antes de desenhar na tela 2D
    try { await Promise.all([document.fonts.load("900 80px Archivo"), document.fonts.load("500 20px Michroma")]); } catch { /* segue com a fonte de reserva */ }

    rodando = true;
    inicio = performance.now();
    botaoFechar.focus();
    raf = requestAnimationFrame(quadro);
  }

  function encerrar() {
    rodando = false;
    cancelAnimationFrame(raf);
    if (gravando) pararGravacao();
    if (audio) audio.parar({ suave: true });
    palco.hidden = true;
    document.body.style.overflow = "";
    restaurarMundo();
    mundo.definirZoom(1);
    if (foco) foco.focus();
  }

  // ------------------------------------------------------------- gravacao
  function tipoSuportado() {
    for (const tipo of ["video/webm;codecs=vp9,opus", "video/webm;codecs=vp8,opus", "video/webm"]) {
      if (window.MediaRecorder && MediaRecorder.isTypeSupported(tipo)) return tipo;
    }
    return null;
  }

  async function comecarGravacao() {
    const tipo = tipoSuportado();
    if (!tipo || !tela.captureStream) { aviso.textContent = t("edit.sem_gravacao"); return; }
    const fluxo = tela.captureStream(30);
    const trilha = audio ? audio.trilhaDeGravacao() : null;
    if (trilha) fluxo.addTrack(trilha);
    pedacos = [];
    gravador = new MediaRecorder(fluxo, { mimeType: tipo, videoBitsPerSecond: 4_000_000 });
    gravador.ondataavailable = (e) => { if (e.data.size) pedacos.push(e.data); };
    gravador.onstop = salvar;
    gravador.start(1000);
    gravando = true;
    medir();
    botaoGravar.textContent = t("edit.gravando");
    aviso.textContent = t("edit.gravando_aviso");
    // recomeca a sequencia, para o arquivo sair do inicio
    cenaAtual = -1;
    inicio = performance.now();
  }

  function pararGravacao() {
    if (!gravador || gravador.state === "inactive") return;
    gravador.stop();
    gravando = false;
    botaoGravar.textContent = t("edit.gravar");
  }

  async function salvar() {
    const blob = new Blob(pedacos, { type: "video/webm" });
    pedacos = [];
    const nome = "auron-edit.webm";
    aviso.textContent = t("edit.salvando", { mb: (blob.size / 1048576).toFixed(1) });
    const claude = globalThis.claude;
    if (claude && typeof claude.use === "function") {
      const downloads = await claude.use("downloads").catch(() => null);
      if (downloads) {
        try {
          await downloads.save({ filename: nome, data: blob });
          aviso.textContent = t("aberto.salvo", { f: nome });
          return;
        } catch (e) {
          aviso.textContent = e && e.code === "declined" ? t("aberto.cancelado") : t("aberto.indisponivel");
          return;
        }
      }
    }
    const url = URL.createObjectURL(blob);
    const a = Object.assign(document.createElement("a"), { href: url, download: nome, rel: "noopener" });
    document.body.append(a); a.click(); a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 30000);
    aviso.textContent = t("aberto.salvo", { f: nome });
  }

  abrirBotao.addEventListener("click", abrir);
  botaoFechar.addEventListener("click", encerrar);
  botaoGravar.addEventListener("click", () => (gravando ? pararGravacao() : comecarGravacao()));
  palco.addEventListener("keydown", (e) => { if (e.key === "Escape") encerrar(); });
  window.addEventListener("resize", () => { if (rodando) medir(); });

  if (calmo) abrirBotao.title = t("edit.aviso_movimento");
  aoMudarIdioma(() => {
    botaoGravar.textContent = gravando ? t("edit.gravando") : t("edit.gravar");
  });
  botaoGravar.textContent = t("edit.gravar");
}
