// A APRESENTAÇÃO da Auron: um vídeo só, educativo, em tela cheia.
//
// Uma apresentadora sintética narra, a legenda acompanha, a trilha própria fica
// baixa ao fundo e os cortes caem no tempo da batida. A troca de cena imita a
// troca de personagem do GTA V: a câmera sai de dentro de um nó e abre.
//
// O mundo 3D é desenhado DENTRO desta tela 2D. Isso serve para duas coisas:
// gravar exatamente o que se vê, e desenhar num tamanho menor que o da janela
// quando a máquina é fraca, sem mexer no resto do site.

// Cada cena tem um mínimo de compassos; a troca só acontece quando a narração
// termina E o compasso fecha, para o corte cair na batida.
const CENAS = [
  { chave: "oque", compassos: 3, mundo: "rede", zoom: [3.2, 1.3] },
  { chave: "proposito", compassos: 3, mundo: "rede", zoom: [1.2, 1.0] },
  { chave: "onde", compassos: 3, mundo: "global", zoom: [1.1, 1.0], global: [0.08, 1] },
  { chave: "moeda", compassos: 3, mundo: "cadeia", zoom: [1.25, 1.02], objeto: "moeda" },
  { chave: "trabalho", compassos: 3, mundo: "nucleo", zoom: [1.15, 1.0], objeto: "tarefa", tag: "simulacao" },
  { chave: "dados", compassos: 3, mundo: "malha", zoom: [1.25, 1.0], objeto: "pacote", tag: "planejado" },
  { chave: "fragmento", compassos: 3, mundo: "grade", zoom: [1.3, 1.0], tag: "pesquisa" },
  { chave: "engenharia", compassos: 3, mundo: "nucleo", zoom: [1.15, 1.0], objeto: "blocos" },
  { chave: "verdade", compassos: 3, mundo: "nucleo", zoom: [1.1, 1.0], tres: true },
  { chave: "ajuda", compassos: 3, mundo: "logo", zoom: [1.4, 1.0], marca: true },
];

/** Caminho do arquivo de narração de uma cena, no idioma escolhido. */
const arquivoDeVoz = (lingua, indice) => `assets/voz/${lingua}/${String(indice + 1).padStart(2, "0")}.wav`;

// Nomes de voz feminina por idioma. A lista não é exaustiva: é o que aparece
// nos aparelhos mais comuns. Sem nenhuma delas, vale a primeira voz do idioma.
const VOZES_FEMININAS = /maria|francisca|luciana|joana|ines|catarina|helena|zira|hazel|samantha|victoria|susan|karen|moira|tessa|fiona|monica|paulina|marisol|sabina|esperanza|kyoko|haruka|nanami|ayumi|female|mulher|feminin/i;

const ease = (x) => 1 - Math.pow(1 - x, 3);

/** Escreve centralizado, encolhendo e quebrando em duas linhas se não couber. */
function escreveCabendo(ctx, texto, x, y, tamanho, peso, maxLargura) {
  if (!texto) return;
  let tam = tamanho;
  const monta = (t) => `${peso} ${t}px Archivo, Arial`;
  ctx.font = monta(tam);
  if (ctx.measureText(texto).width <= maxLargura) { ctx.fillText(texto, x, y); return; }
  while (tam > tamanho * 0.72) {
    tam -= tamanho * 0.04;
    ctx.font = monta(tam);
    if (ctx.measureText(texto).width <= maxLargura) { ctx.fillText(texto, x, y); return; }
  }
  const palavras = texto.split(" ");
  if (palavras.length < 2) { ctx.fillText(texto, x, y); return; }
  const corte = Math.round(palavras.length / 2);
  const linhas = [palavras.slice(0, corte).join(" "), palavras.slice(corte).join(" ")];
  while (tam > tamanho * 0.5 && linhas.some((l) => ctx.measureText(l).width > maxLargura)) {
    tam -= tamanho * 0.05;
    ctx.font = monta(tam);
  }
  ctx.fillText(linhas[0], x, y - tam * 0.52);
  ctx.fillText(linhas[1], x, y + tam * 0.52);
}

/** Quebra um texto em linhas que cabem na largura dada. */
function quebra(ctx, texto, maxLargura) {
  const palavras = texto.split(" ");
  const linhas = [];
  let atual = "";
  for (const palavra of palavras) {
    const tentativa = atual ? `${atual} ${palavra}` : palavra;
    if (ctx.measureText(tentativa).width > maxLargura && atual) {
      linhas.push(atual);
      atual = palavra;
    } else atual = tentativa;
  }
  if (atual) linhas.push(atual);
  return linhas;
}

export function iniciar({ t, audio, mundo, idioma, movel, aoMudarIdioma, restaurarMundo }) {
  const abrirBotao = document.getElementById("abre-edit");
  if (!abrirBotao) return;
  const palco = document.getElementById("palco-edit");
  const tela = document.getElementById("tela-edit");
  const ctx = tela.getContext("2d", { alpha: false });
  const botaoGravar = document.getElementById("edit-gravar");
  const botaoVoz = document.getElementById("edit-voz");
  const botaoFechar = document.getElementById("edit-fechar");
  const aviso = document.getElementById("edit-aviso");
  const mundoCanvas = document.getElementById("mundo");
  const voz = "speechSynthesis" in window ? window.speechSynthesis : null;

  let rodando = false, raf = 0, pulso = 0, pulsoFala = 0;
  let cenaAtual = -1, inicioCena = 0, inicioTudo = 0, cortePedido = 0;
  let narrouAcabou = true, falaAtual = null, comVoz = !!voz;
  let gravador = null, pedacos = [], gravando = false, foco = null;
  let larg = 0, alt = 0, escala = 1, vinheta = null, ultimoQuadro = 0;
  const compasso = audio ? audio.compasso : 3.871;
  const alvoFPS = movel ? 24 : 30;

  // ---------------------------------------------------------------- tamanho
  function medir() {
    const larguraCSS = window.innerWidth, alturaCSS = window.innerHeight;
    // desenha no máximo em 1280 de largura: numa máquina fraca isso é a
    // diferença entre travar e rodar liso. O CSS estica de volta.
    const limite = gravando ? 1280 : movel ? 900 : 1180;
    escala = Math.min(1, limite / larguraCSS);
    larg = Math.round(larguraCSS * escala);
    alt = Math.round(alturaCSS * escala);
    tela.width = larg;
    tela.height = alt;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    vinheta = ctx.createRadialGradient(larg / 2, alt * 0.45, alt * 0.25, larg / 2, alt * 0.5, alt * 0.95);
    vinheta.addColorStop(0, "rgba(3,3,3,0)");
    vinheta.addColorStop(1, "rgba(3,3,3,0.82)");
  }

  // ------------------------------------------------------------------- voz
  function vozEscolhida() {
    if (!voz) return null;
    const lingua = idioma().toLowerCase(), base = lingua.slice(0, 2);
    const todas = voz.getVoices().filter((v) => v.lang.toLowerCase().replace("_", "-").startsWith(base));
    if (!todas.length) return null;
    return todas.find((v) => VOZES_FEMININAS.test(v.name)) || todas[0];
  }
  // A narração tem duas fontes, nesta ordem:
  // 1. arquivo de áudio gravado com a voz do projeto — toca igual em qualquer
  //    aparelho, entra na gravação do vídeo e dá o nível para mexer a boca;
  // 2. a voz do navegador, quando não existe arquivo naquele idioma.
  async function narrarCena(indice) {
    narrouAcabou = false;
    if (!comVoz) { narrouAcabou = true; return; }
    const texto = textoDaCena("fala", indice);
    if (audio) {
      const url = arquivoDeVoz(idioma(), indice);
      const tocou = await audio.narrar(url);
      if (tocou) { if (indice === cenaAtual) narrouAcabou = true; return; }
      // prepara a próxima enquanto esta fala
      const proxima = arquivoDeVoz(idioma(), indice + 1);
      if (indice + 1 < CENAS.length) audio.carregarVoz(proxima).catch(() => {});
    }
    falar(texto);
  }

  function falar(texto) {
    narrouAcabou = !comVoz;
    if (!comVoz || !voz) return;
    const v = vozEscolhida();
    if (!v) { narrouAcabou = true; return; }
    const fala = new SpeechSynthesisUtterance(texto);
    fala.voice = v;
    fala.lang = v.lang;
    fala.rate = 0.92;    // calma: é explicação, não propaganda
    fala.pitch = 1.12;   // um pouco acima, para soar sintética e clara
    fala.onboundary = () => { pulsoFala = 1; };
    fala.onend = () => { if (falaAtual === fala) narrouAcabou = true; };
    fala.onerror = () => { narrouAcabou = true; };
    falaAtual = fala;
    voz.cancel();
    voz.speak(fala);
  }
  function atualizarVoz() {
    if (!botaoVoz) return;
    const tem = !!vozEscolhida() || !!audio;
    botaoVoz.textContent = comVoz && tem ? t("edit.voz_on") : t("edit.voz_off");
    botaoVoz.setAttribute("aria-pressed", String(comVoz && tem));
    if (!tem && comVoz) aviso.textContent = t("edit.sem_voz");
  }
  if (voz) voz.addEventListener("voiceschanged", atualizarVoz);

  // ----------------------------------------------------------------- cenas
  function textoDaCena(chave, indice) {
    const lista = t("edit.cenas");
    return lista[indice] ? lista[indice][chave] : "";
  }
  function entrarNaCena(i) {
    cenaAtual = i;
    inicioCena = performance.now();
    cortePedido = 0;
    const cena = CENAS[i];
    mundo.definirModo(cena.mundo);
    mundo.objeto(cena.objeto || null);
    if (cena.global) mundo.definirZoom(cena.global[0]);
    if (audio) audio.impacto();
    narrarCena(i);
    // adianta o carregamento da próxima fala, para não haver silêncio no corte
    if (audio && i + 1 < CENAS.length) audio.carregarVoz(arquivoDeVoz(idioma(), i + 1)).catch(() => {});
  }

  function quadro(agora) {
    if (!rodando) return;
    raf = requestAnimationFrame(quadro);
    if (agora - ultimoQuadro < 1000 / alvoFPS - 1) return;   // limita os quadros
    const dt = (agora - ultimoQuadro) / 1000;
    ultimoQuadro = agora;

    const cena = CENAS[cenaAtual];
    const naCena = (agora - inicioCena) / 1000;
    const minimo = cena.compassos * compasso;

    // pronto para trocar: espera o compasso fechar, para o corte cair na batida
    if (!cortePedido && naCena >= minimo && narrouAcabou) {
      const desdeInicio = (agora - inicioTudo) / 1000;
      cortePedido = agora + (compasso - (desdeInicio % compasso)) * 1000;
      if (audio && cenaAtual < CENAS.length - 1) audio.subida(Math.min(1.2, (cortePedido - agora) / 1000));
    }
    if (cortePedido && agora >= cortePedido) {
      if (cenaAtual >= CENAS.length - 1) { encerrar(); return; }
      entrarNaCena(cenaAtual + 1);
      return;
    }
    if (cena.global) {
      const [a, b] = cena.global;
      mundo.definirZoom(a + (b - a) * Math.min(1, naCena / (minimo * 1.4)));
    }

    const estalo = Math.max(0, 1 - naCena / 0.3);
    const separa = Math.max(0, 1 - naCena / 0.8) * 10 * escala;
    const [z0, z1] = cena.zoom;
    const k = (z0 + (z1 - z0) * ease(Math.min(1, naCena / (minimo * 0.55)))) * (1 + pulso * 0.01);

    ctx.fillStyle = "#030303";
    ctx.fillRect(0, 0, larg, alt);

    const tremor = estalo * 8 * escala;
    const lw = larg * k, lh = alt * k;
    const ox = (larg - lw) / 2 + (Math.random() - 0.5) * tremor;
    const oy = (alt - lh) / 2 + (Math.random() - 0.5) * tremor;
    try {
      if (separa > 0.5) {
        // separação de cor só logo depois do corte: é o desenho mais caro
        ctx.globalCompositeOperation = "lighter";
        ctx.globalAlpha = 0.85;
        ctx.drawImage(mundoCanvas, ox - separa, oy, lw, lh);
        ctx.drawImage(mundoCanvas, ox + separa, oy, lw, lh);
        ctx.globalAlpha = 1;
        ctx.globalCompositeOperation = "source-over";
      } else {
        ctx.drawImage(mundoCanvas, ox, oy, lw, lh);
      }
    } catch { /* o mundo ainda não desenhou nada */ }

    ctx.fillStyle = vinheta;
    ctx.fillRect(0, 0, larg, alt);

    // a boca do modelo 3D segue o som da voz; sem arquivo, segue as sílabas
    mundo.definirBoca(audio && audio.falando() ? audio.nivelVoz() : pulsoFala * 0.7);
    if (mundo.hudEscala) mundo.hudEscala(k);

    desenharTitulo(cenaAtual, cena, naCena, minimo);
    desenharRotuloDaVoz();
    desenharLegenda(cenaAtual);

    const tarja = alt * 0.085 + estalo * 5;
    ctx.fillStyle = "#030303";
    ctx.fillRect(0, 0, larg, tarja);
    ctx.fillRect(0, alt - tarja, larg, tarja);

    if (estalo > 0) {
      ctx.fillStyle = `rgba(232,236,242,${(estalo * 0.3).toFixed(3)})`;
      ctx.fillRect(0, 0, larg, alt);
    }

    // marcas de cena no pé, em vez de barra de tempo: a duração varia com a fala
    const passo = larg * 0.012, base = larg / 2 - (CENAS.length * passo) / 2;
    for (let i = 0; i < CENAS.length; i++) {
      ctx.fillStyle = i <= cenaAtual ? "rgba(236,234,230,0.8)" : "rgba(236,234,230,0.22)";
      ctx.fillRect(base + i * passo, alt - tarja - 4 * escala, passo * 0.6, 2 * escala);
    }

    pulso *= 0.9;
    pulsoFala = Math.max(0, pulsoFala - dt * 3.2);
  }

  function desenharTitulo(i, cena, naCena, minimo) {
    const entrada = Math.min(1, naCena / 0.5);
    const saida = cortePedido ? 1 : Math.min(1, (minimo * 1.6 - naCena) * 2);
    const alfa = ease(entrada) * Math.max(0.15, Math.min(1, saida));
    const base = Math.min(larg, alt * 1.6);
    ctx.textAlign = "center";
    ctx.globalAlpha = alfa;

    if (cena.marca) {
      const tam = base * 0.1;
      ctx.fillStyle = "#eceae6";
      escreveCabendo(ctx, "AURON", larg / 2, alt * 0.42, tam, "900", larg * 0.86);
      ctx.font = `500 ${tam * 0.2}px Michroma, Archivo, Arial`;
      ctx.fillStyle = "#f3e2c4";
      ctx.fillText("BUILD THE INFRASTRUCTURE.", larg / 2, alt * 0.42 + tam * 0.6);
    } else if (cena.tres) {
      const linhas = textoDaCena("linhas", i) || [];
      const tam = base * 0.046;
      linhas.forEach((linha, quantos) => {
        const chega = Math.min(1, Math.max(0, (naCena - quantos * 0.7) / 0.4));
        ctx.globalAlpha = alfa * ease(chega);
        ctx.fillStyle = quantos === linhas.length - 1 ? "#f3e2c4" : "#eceae6";
        escreveCabendo(ctx, linha, larg / 2, alt * 0.33 + quantos * tam * 1.6, tam, "700", larg * 0.86);
      });
      ctx.globalAlpha = alfa;
    } else {
      const tam = base * 0.062;
      ctx.fillStyle = "#eceae6";
      escreveCabendo(ctx, textoDaCena("titulo", i), larg / 2, alt * 0.38, tam, "900", larg * 0.86);
      ctx.fillStyle = "#95989f";
      escreveCabendo(ctx, textoDaCena("sub", i), larg / 2, alt * 0.38 + tam * 0.62, tam * 0.26, "400", larg * 0.8);
    }

    if (cena.tag) {
      const rotulo = { simulacao: "SIMULATION", pesquisa: "RESEARCH", planejado: t("estado.planejado").toUpperCase() }[cena.tag];
      ctx.font = `500 ${base * 0.015}px "IBM Plex Mono", monospace`;
      ctx.textAlign = "left";
      ctx.fillStyle = "#f3e2c4";
      ctx.fillText(rotulo, larg * 0.06, alt * 0.145);
      ctx.textAlign = "center";
    }
    ctx.globalAlpha = 1;
  }

  // O rosto agora é um modelo 3D, desenhado pelo mundo. Aqui fica só o rótulo,
  // embaixo dele: quem ouve precisa saber que a voz é sintética.
  function desenharRotuloDaVoz() {
    const base = Math.min(larg, alt * 1.6);
    ctx.textAlign = "center";
    ctx.font = `500 ${Math.max(9, base * 0.014)}px "IBM Plex Mono", monospace`;
    ctx.fillStyle = "rgba(143,146,153,0.9)";
    ctx.fillText(t("edit.apresentadora"), larg * 0.216, alt * 0.73);
  }

  function desenharLegenda(i) {
    const fala = textoDaCena("fala", i);
    if (!fala) return;
    const tam = Math.max(12, Math.min(larg, alt * 1.7) * 0.021);
    ctx.font = `400 ${tam}px Archivo, Arial`;
    ctx.textAlign = "center";
    const linhas = quebra(ctx, fala, larg * 0.7);
    const altura = linhas.length * tam * 1.35;
    const topo = alt - alt * 0.085 - altura - tam * 1.2;
    ctx.fillStyle = "rgba(3,3,3,0.55)";
    ctx.fillRect(larg * 0.12, topo - tam, larg * 0.76, altura + tam * 1.4);
    ctx.fillStyle = "#eceae6";
    linhas.forEach((linha, k) => ctx.fillText(linha, larg / 2, topo + k * tam * 1.35 + tam * 0.2));
  }

  // -------------------------------------------------------------- controle
  async function abrir() {
    foco = document.activeElement;
    palco.hidden = false;
    document.body.style.overflow = "hidden";
    medir();
    aviso.textContent = "";
    if (audio) {
      const pronto = await audio.iniciar();
      // a trilha fica BAIXA: é fundo para a narração, não show de música
      if (pronto) audio.tocar({ volume: 0.22, batida: () => { pulso = 1; } });
    }
    try {
      await Promise.all([document.fonts.load("900 80px Archivo"), document.fonts.load("400 20px Archivo")]);
    } catch { /* segue com a fonte de reserva */ }
    atualizarVoz();
    mundo.assistente(true);
    rodando = true;
    inicioTudo = performance.now();
    ultimoQuadro = 0;
    entrarNaCena(0);
    botaoFechar.focus();
    raf = requestAnimationFrame(quadro);
  }

  function encerrar() {
    rodando = false;
    cancelAnimationFrame(raf);
    if (voz) voz.cancel();
    if (audio) audio.pararNarracao();
    mundo.assistente(false);
    mundo.definirBoca(0);
    if (gravando) pararGravacao();
    if (audio) audio.parar({ suave: true });
    palco.hidden = true;
    document.body.style.overflow = "";
    restaurarMundo();
    mundo.definirZoom(1);
    if (foco) foco.focus();
  }

  // -------------------------------------------------------------- gravação
  function tipoSuportado() {
    for (const tipo of ["video/webm;codecs=vp9,opus", "video/webm;codecs=vp8,opus", "video/webm"]) {
      if (window.MediaRecorder && MediaRecorder.isTypeSupported(tipo)) return tipo;
    }
    return null;
  }

  function comecarGravacao() {
    const tipo = tipoSuportado();
    if (!tipo || !tela.captureStream) { aviso.textContent = t("edit.sem_gravacao"); return; }
    const fluxo = tela.captureStream(30);
    const trilha = audio ? audio.trilhaDeGravacao() : null;
    if (trilha) fluxo.addTrack(trilha);
    pedacos = [];
    gravador = new MediaRecorder(fluxo, { mimeType: tipo, videoBitsPerSecond: 3_500_000 });
    gravador.ondataavailable = (e) => { if (e.data.size) pedacos.push(e.data); };
    gravador.onstop = salvar;
    gravador.start(1000);
    gravando = true;
    medir();
    botaoGravar.textContent = t("edit.gravando");
    aviso.textContent = t("edit.gravando_aviso");
    inicioTudo = performance.now();
    entrarNaCena(0);
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
    const nome = "auron-apresentacao.webm";
    aviso.textContent = t("edit.salvando", { mb: (blob.size / 1048576).toFixed(1) });
    const claude = globalThis.claude;
    if (claude && typeof claude.use === "function") {
      const downloads = await claude.use("downloads").catch(() => null);
      if (downloads) {
        try {
          await downloads.save({ filename: nome, data: blob });
          aviso.textContent = t("aberto.salvo", { f: nome });
        } catch (e) {
          aviso.textContent = e && e.code === "declined" ? t("aberto.cancelado") : t("aberto.indisponivel");
        }
        return;
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
  if (botaoVoz) {
    botaoVoz.addEventListener("click", () => {
      comVoz = !comVoz && !!voz;
      if (!comVoz) {
        if (voz) voz.cancel();
        if (audio) audio.pararNarracao();
        narrouAcabou = true;
      } else if (rodando) narrarCena(cenaAtual);
      atualizarVoz();
    });
  }
  palco.addEventListener("keydown", (e) => { if (e.key === "Escape") encerrar(); });
  window.addEventListener("resize", () => { if (rodando) medir(); });

  botaoGravar.textContent = t("edit.gravar");
  atualizarVoz();
  aoMudarIdioma(() => {
    botaoGravar.textContent = gravando ? t("edit.gravando") : t("edit.gravar");
    atualizarVoz();
  });
}
