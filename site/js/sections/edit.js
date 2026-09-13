// A APRESENTAÇÃO da Auron: um vídeo só, educativo, em tela cheia.
//
// Uma narração acompanha, a legenda entra em blocos curtos no ritmo da fala, a
// trilha própria fica baixa ao fundo e os cortes caem no tempo da batida.
//
// Na tela, só os objetos 3D de explicação: a rede de pontos do site some durante
// o vídeo, porque era a parte pesada e competia com o que está sendo explicado.
//
// O mundo 3D é desenhado DENTRO desta tela 2D. Isso serve para duas coisas:
// gravar exatamente o que se vê, e desenhar num tamanho menor que o da janela
// quando a máquina é fraca, sem mexer no resto do site.

// Cada cena tem um mínimo de compassos; a troca só acontece quando a narração
// termina E o compasso fecha, para o corte cair na batida.
const CENAS = [
  { chave: "oque", compassos: 3, mundo: "rede", zoom: [2.2, 1.15], objeto: "nos" },
  { chave: "proposito", compassos: 3, mundo: "rede", zoom: [1.12, 1.0], objeto: "calculo" },
  { chave: "onde", compassos: 3, mundo: "global", zoom: [1.1, 1.0], global: [0.08, 1], objeto: "ciencia" },
  { chave: "moeda", compassos: 3, mundo: "cadeia", zoom: [1.15, 1.02], objeto: "cadeia3d" },
  { chave: "trabalho", compassos: 3, mundo: "nucleo", zoom: [1.1, 1.0], objeto: "freivalds", tag: "simulacao" },
  { chave: "dados", compassos: 3, mundo: "malha", zoom: [1.15, 1.0], objeto: "pacote", tag: "planejado" },
  { chave: "fragmento", compassos: 3, mundo: "grade", zoom: [1.15, 1.0], objeto: "fragmentos", tag: "pesquisa" },
  { chave: "engenharia", compassos: 3, mundo: "nucleo", zoom: [1.1, 1.0], objeto: "dois" },
  { chave: "verdade", compassos: 3, mundo: "nucleo", zoom: [1.08, 1.0], tres: true },
  { chave: "ajuda", compassos: 3, mundo: "logo", zoom: [1.25, 1.0], objeto: "gente", marca: true },
];

/**
 * Caminho do arquivo de narração de uma cena, no idioma escolhido. O `.mp3` vem
 * primeiro: é o formato de uma voz gravada por gente ou de um serviço de voz.
 * O `.wav` é o gerado pela voz do Windows (`tools/build_narracao.py`).
 */
const arquivoDeVoz = (lingua, indice, formato = "mp3") => `assets/voz/${lingua}/${String(indice + 1).padStart(2, "0")}.${formato}`;

// Nomes de voz feminina por idioma. A lista não é exaustiva: é o que aparece
// nos aparelhos mais comuns. Sem nenhuma delas, vale a primeira voz do idioma.
const VOZES_FEMININAS = /maria|francisca|luciana|joana|ines|catarina|helena|zira|hazel|samantha|victoria|susan|karen|moira|tessa|fiona|monica|paulina|marisol|sabina|esperanza|kyoko|haruka|nanami|ayumi|female|mulher|feminin/i;

import PRONUNCIA from "../../locales/pronuncia.js";

const ease = (x) => 1 - Math.pow(1 - x, 3);
const faixa = (x, a, b) => Math.min(1, Math.max(0, (x - a) / (b - a)));

// Legenda no padrão de TV: no máximo duas linhas e cerca de 42 caracteres por
// linha, trocando de bloco no ritmo da fala.
const MAX_BLOCO = 84;

/** Corta a fala em blocos de legenda: frase inteira quando cabe; senão, vírgula; senão, palavras. */
export function blocosDeLegenda(texto, max = MAX_BLOCO) {
  const blocos = [];
  const empurraPalavras = (trecho) => {
    const palavras = trecho.split(/\s+/).filter(Boolean);
    const partes = Math.ceil(trecho.length / max);
    const alvo = Math.ceil(trecho.length / partes);
    let atual = "";
    for (const p of palavras) {
      const tentativa = atual ? `${atual} ${p}` : p;
      if (tentativa.length > alvo && atual) { blocos.push(atual); atual = p; } else atual = tentativa;
    }
    if (atual) blocos.push(atual);
  };
  for (const frase of String(texto || "").split(/(?<=[.!?:;])\s+/)) {
    if (!frase) continue;
    if (frase.length <= max) { blocos.push(frase); continue; }
    let atual = "";
    for (const parte of frase.split(/(?<=,)\s+/)) {
      const tentativa = atual ? `${atual} ${parte}` : parte;
      if (tentativa.length <= max) { atual = tentativa; continue; }
      if (atual) blocos.push(atual);
      if (parte.length <= max) atual = parte;
      else { empurraPalavras(parte); atual = ""; }
    }
    if (atual) blocos.push(atual);
  }
  return blocos;
}

/** Quebra um bloco em até duas linhas equilibradas, cortando no espaço mais perto do meio. */
function duasLinhas(ctx, bloco, maxLargura) {
  if (ctx.measureText(bloco).width <= maxLargura && bloco.length <= 42) return [bloco];
  const meio = Math.floor(bloco.length / 2);
  let corte = -1;
  for (let d = 0; d < meio; d++) {
    if (bloco[meio + d] === " ") { corte = meio + d; break; }
    if (bloco[meio - d] === " ") { corte = meio - d; break; }
  }
  if (corte < 0) return [bloco];
  return [bloco.slice(0, corte), bloco.slice(corte + 1)];
}

/** O texto que a voz lê: igual à legenda, com as palavras estrangeiras escritas como se falam. */
function textoParaVoz(texto, lingua) {
  let saida = String(texto || "");
  for (const [padrao, troca] of PRONUNCIA[lingua] || []) saida = saida.replace(new RegExp(padrao, "g"), troca);
  return saida;
}

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
  // Progresso da fala do navegador (0 a 1), pelo caractere que ela está lendo.
  let progressoFala = 0;
  // Legenda: blocos da cena atual, o bloco na tela e quando ele entrou.
  let blocos = [], blocoAtual = -1, trocaBloco = 0;
  // Se o primeiro .mp3 não existir, não adianta pedir os outros: vai direto ao .wav.
  let temMp3 = true;
  const vozDaCena = (lingua, indice) => {
    const wav = () => audio.carregarVoz(arquivoDeVoz(lingua, indice, "wav")).then(() => arquivoDeVoz(lingua, indice, "wav"));
    if (!temMp3) return wav();
    return audio.carregarVoz(arquivoDeVoz(lingua, indice, "mp3"))
      .then(() => arquivoDeVoz(lingua, indice, "mp3"))
      .catch(() => { temMp3 = false; return wav(); });
  };
  // Cada entrada em cena recebe um número. Uma narração que termina depois de
  // a cena já ter mudado carrega o número antigo e é ignorada — sem isso, uma
  // fala atrasada deixava o vídeo parado na cena para sempre.
  let geracao = 0;
  let gravador = null, pedacos = [], gravando = false, foco = null;
  let larg = 0, alt = 0, escala = 1, vinheta = null, ultimoQuadro = 0, quadros = 0;
  const compasso = audio ? audio.compasso : 3.871;
  // O vídeo desenha o quadro do mundo. Correr mais rápido que ele só repete
  // quadro e gasta bateria à toa.
  const alvoFPS = mundo.fps || (movel ? 24 : 30);

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
    const minha = ++geracao;
    narrouAcabou = false;
    if (!comVoz) { narrouAcabou = true; return; }
    const texto = textoDaCena("fala", indice);
    if (audio) {
      let tocou = false;
      try {
        const lingua = idioma();
        const fim = vozDaCena(lingua, indice).then((url) => audio.narrar(url)).catch(() => false);
        // se em três segundos a voz não estiver tocando, o arquivo não vai vir:
        // segue com a voz do navegador em vez de esperar calado
        const comecou = await Promise.race([
          fim.then(() => "fim"),
          new Promise((r) => setTimeout(() => r(audio.falando() ? "tocando" : "nada"), 3000)),
        ]);
        if (minha !== geracao) return;            // a cena já mudou: ignora
        if (comecou === "fim" && (await fim) !== false) { narrouAcabou = true; return; }
        if (comecou === "tocando") {
          fim.then(() => { if (minha === geracao) narrouAcabou = true; }).catch(() => {});
          return;
        }
      } catch { tocou = false; }
      if (minha !== geracao) return;
      void tocou;
    }
    falar(texto, minha);
  }

  function falar(texto, minha = geracao) {
    if (!comVoz || !voz) { narrouAcabou = true; return; }
    const v = vozEscolhida();
    if (!v) { narrouAcabou = true; return; }      // sem voz: só a legenda
    const falado = textoParaVoz(texto, idioma());
    const fala = new SpeechSynthesisUtterance(falado);
    progressoFala = 0;
    fala.voice = v;
    fala.lang = v.lang;
    fala.rate = 0.92;    // calma: é explicação, não propaganda
    fala.pitch = 1.05;
    fala.onboundary = (e) => {
      pulsoFala = 1;
      if (minha === geracao && falado.length) progressoFala = Math.min(1, (e.charIndex || 0) / falado.length);
    };
    fala.onend = () => { if (minha === geracao) narrouAcabou = true; };
    fala.onerror = () => { if (minha === geracao) narrouAcabou = true; };
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
    mundo.objeto(cena.objeto || null);
    blocos = blocosDeLegenda(textoDaCena("fala", i));
    blocoAtual = -1;
    progressoFala = 0;
    if (audio) audio.impacto();
    narrarCena(i);
    // adianta o carregamento da próxima fala, para não haver silêncio no corte
    if (audio && i + 1 < CENAS.length) {
      vozDaCena(idioma(), i + 1).catch(() => {});
    }
  }

  function quadro() {
    if (!rodando) return;
    raf = requestAnimationFrame(quadro);
    // Um relógio só. O horário que vem no quadro de animação e o do sistema
    // podem andar separados, e misturar os dois fazia a conta do tempo de cena
    // dar errado: ora pulava cenas, ora travava para sempre numa delas.
    const agora = performance.now();
    if (agora - ultimoQuadro < 1000 / alvoFPS - 1) return;   // limita os quadros
    const dt = (agora - ultimoQuadro) / 1000;
    ultimoQuadro = agora;
    quadros++;

    const cena = CENAS[cenaAtual];
    const naCena = (agora - inicioCena) / 1000;
    // Sem narração, a cena precisa durar o tempo de LER a legenda: umas duas
    // palavras e meia por segundo, arredondado para cima em compassos.
    const palavras = String(textoDaCena("fala", cenaAtual) || "").split(/\s+/).length;
    const paraLer = comVoz ? 0 : Math.ceil((palavras / 2.5) / compasso) * compasso;
    const minimo = Math.max(cena.compassos * compasso, paraLer);

    // Trava de segurança: se a narração não terminar (som bloqueado, arquivo
    // que não carrega, voz que nunca dispara o fim), a cena passa assim mesmo.
    // O vídeo nunca fica preso.
    if (!narrouAcabou && naCena > minimo + 45) narrouAcabou = true;

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
    // Entrada e saída em fade do preto: sem tremor, sem flash e sem separação
    // de cor, que eram as partes mais pesadas de desenhar.
    const entra = ease(faixa(naCena, 0, 0.45));
    const sai = cortePedido ? Math.min(1, Math.max(0, (cortePedido - agora) / 350)) : 1;
    const k = 1 + 0.03 * ease(Math.min(1, naCena / Math.max(1, minimo)));

    ctx.fillStyle = "#030303";
    ctx.fillRect(0, 0, larg, alt);

    const lw = larg * k, lh = alt * k;
    ctx.globalAlpha = entra * sai;
    try { ctx.drawImage(mundoCanvas, (larg - lw) / 2, (alt - lh) / 2, lw, lh); }
    catch { /* o mundo ainda não desenhou nada */ }
    ctx.globalAlpha = 1;

    ctx.fillStyle = vinheta;
    ctx.fillRect(0, 0, larg, alt);

    desenharTitulo(cenaAtual, cena, naCena, agora);
    desenharRotuloDaVoz();
    desenharLegenda(naCena, paraLer || minimo, agora, sai);

    const tarja = alt * 0.085;
    ctx.fillStyle = "#030303";
    ctx.fillRect(0, 0, larg, tarja);
    ctx.fillRect(0, alt - tarja, larg, tarja);

    // marcas de cena no pé, em vez de barra de tempo: a duração varia com a fala
    const passo = larg * 0.012, base = larg / 2 - (CENAS.length * passo) / 2;
    for (let i = 0; i < CENAS.length; i++) {
      ctx.fillStyle = i <= cenaAtual ? "rgba(236,234,230,0.8)" : "rgba(236,234,230,0.22)";
      ctx.fillRect(base + i * passo, alt - tarja - 4 * escala, passo * 0.6, 2 * escala);
    }

    pulso *= 0.9;
    pulsoFala = Math.max(0, pulsoFala - dt * 3.2);
  }

  function desenharTitulo(i, cena, naCena, agora) {
    // Um elemento de cada vez: título, depois o subtítulo, depois o selo.
    const entrada = faixa(naCena, 0.15, 0.75);
    const entradaSub = ease(faixa(naCena, 0.75, 1.3));
    const entradaSelo = ease(faixa(naCena, 1.1, 1.6));
    const sobe = (1 - ease(entrada)) * alt * 0.02;
    // O título fica de pé a cena inteira e só apaga nos últimos instantes,
    // quando o corte já está marcado. Antes ele desbotava no meio da fala.
    const saida = cortePedido ? Math.max(0, Math.min(1, (cortePedido - agora) / 400)) : 1;
    const alfa = ease(entrada) * saida;
    const base = Math.min(larg, alt * 1.6);
    ctx.textAlign = "center";
    ctx.globalAlpha = alfa;

    if (cena.marca) {
      const tam = base * 0.1;
      ctx.fillStyle = "#eceae6";
      escreveCabendo(ctx, "AURON", larg / 2, alt * 0.30, tam, "900", larg * 0.86);
      ctx.font = `500 ${tam * 0.2}px Michroma, Archivo, Arial`;
      ctx.fillStyle = "#f3e2c4";
      ctx.fillText("BUILD THE INFRASTRUCTURE.", larg / 2, alt * 0.30 + tam * 0.6);
    } else if (cena.tres) {
      const linhas = textoDaCena("linhas", i) || [];
      const tam = base * 0.046;
      linhas.forEach((linha, quantos) => {
        const chega = faixa(naCena, 0.2 + quantos * 0.8, 0.6 + quantos * 0.8);
        ctx.globalAlpha = alfa * ease(chega);
        ctx.fillStyle = quantos === linhas.length - 1 ? "#f3e2c4" : "#eceae6";
        escreveCabendo(ctx, linha, larg / 2, alt * 0.30 + quantos * tam * 1.6, tam, "700", larg * 0.86);
      });
      ctx.globalAlpha = alfa;
    } else {
      const tam = base * 0.056;
      ctx.fillStyle = "#eceae6";
      escreveCabendo(ctx, textoDaCena("titulo", i), larg / 2, alt * 0.26 + sobe, tam, "900", larg * 0.86);
      ctx.globalAlpha = alfa * entradaSub;
      ctx.fillStyle = "#a4a7ad";
      escreveCabendo(ctx, textoDaCena("sub", i), larg / 2, alt * 0.26 + tam * 0.66, tam * 0.3, "400", larg * 0.8);
      ctx.globalAlpha = alfa;
    }

    if (cena.tag) {
      ctx.globalAlpha = alfa * entradaSelo;
      const rotulo = { simulacao: "SIMULATION", pesquisa: "RESEARCH", planejado: t("estado.planejado").toUpperCase() }[cena.tag];
      ctx.font = `500 ${base * 0.015}px "IBM Plex Mono", monospace`;
      ctx.textAlign = "left";
      ctx.fillStyle = "#f3e2c4";
      ctx.fillText(rotulo, larg * 0.06, alt * 0.145);
      ctx.textAlign = "center";
    }
    ctx.globalAlpha = 1;
  }

  // Não há personagem na tela. Fica só o aviso, no canto, de que a voz é
  // sintética: quem ouve tem o direito de saber.
  function desenharRotuloDaVoz() {
    const base = Math.min(larg, alt * 1.6);
    ctx.textAlign = "right";
    ctx.font = `500 ${Math.max(9, base * 0.013)}px "IBM Plex Mono", monospace`;
    ctx.fillStyle = "rgba(143,146,153,0.75)";
    ctx.fillText(t("edit.apresentadora"), larg * 0.94, alt * 0.145);
    ctx.textAlign = "center";
  }

  /**
   * Quanto da fala já foi dito, de 0 a 1: pelo arquivo de áudio (tempo tocado),
   * pela voz do navegador (caractere lido) ou, sem voz, pelo tempo de leitura.
   */
  function progressoDaNarracao(naCena, duracaoLeitura) {
    if (comVoz && audio) {
      const p = audio.progressoVoz();
      if (p !== null) return p;
    }
    if (comVoz && voz && voz.speaking) return progressoFala;
    if (comVoz && !narrouAcabou) return 0;
    if (comVoz) return 1;
    return Math.min(1, Math.max(0, (naCena - 1.2) / Math.max(1, duracaoLeitura - 1.8)));
  }

  /** Legenda em blocos de no máximo duas linhas, trocando no ritmo da fala. */
  function desenharLegenda(naCena, duracaoLeitura, agora, sai) {
    if (!blocos.length || naCena < 1.0) return;
    const total = blocos.reduce((soma, b) => soma + b.length, 0);
    // um leve adiantamento: a legenda aparece junto com o começo da frase
    const p = Math.min(0.999, progressoDaNarracao(naCena, duracaoLeitura) + 0.015);
    let acumulado = 0, indice = blocos.length - 1;
    for (let k = 0; k < blocos.length; k++) {
      acumulado += blocos[k].length;
      if (acumulado / total > p) { indice = k; break; }
    }
    if (indice !== blocoAtual) { blocoAtual = indice; trocaBloco = agora; }
    const aparece = ease(Math.min(1, (agora - trocaBloco) / 160));

    const base = Math.min(larg, alt * 1.78);
    const tam = Math.max(13, Math.min(30, base * 0.022));
    ctx.font = `500 ${tam}px Archivo, Arial`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    const linhas = duasLinhas(ctx, blocos[indice], larg * 0.72);
    const alturaLinha = tam * 1.42;
    const fundo = alt - alt * 0.085 - tam * 1.4;
    ctx.globalAlpha = aparece * sai;
    linhas.forEach((linha, k) => {
      const y = fundo - (linhas.length - 1 - k) * alturaLinha;
      const w = ctx.measureText(linha).width;
      ctx.fillStyle = "rgba(3,3,3,0.62)";
      ctx.fillRect(larg / 2 - w / 2 - tam * 0.45, y - alturaLinha / 2, w + tam * 0.9, alturaLinha);
      ctx.fillStyle = "#f4f2ee";
      ctx.fillText(linha, larg / 2, y + tam * 0.04);
    });
    ctx.globalAlpha = 1;
    ctx.textBaseline = "alphabetic";
  }

  // -------------------------------------------------------------- controle
  function abrir() {
    if (rodando) return;                 // já está aberto: não abre de novo
    cancelAnimationFrame(raf);
    foco = document.activeElement;
    palco.hidden = false;
    document.body.style.overflow = "hidden";
    medir();
    aviso.textContent = "";
    atualizarVoz();

    // O vídeo começa AGORA. Som e fontes entram quando ficarem prontos, cada um
    // no seu tempo. Esperar por eles aqui era o que deixava a tela preta parada
    // quando o navegador demorava a liberar o áudio.
    rodando = true;
    if (mundo.somenteObjetos) mundo.somenteObjetos(true);
    inicioTudo = performance.now();
    ultimoQuadro = 0;
    entrarNaCena(0);
    botaoFechar.focus();
    raf = requestAnimationFrame(quadro);

    if (audio) {
      audio.iniciar().then((pronto) => {
        // a trilha fica BAIXA: é fundo para a narração, não show de música
        if (pronto && rodando) audio.tocar({ volume: 0.22, batida: () => { pulso = 1; } });
      }).catch(() => {});
    }
    if (document.fonts && document.fonts.load) {
      document.fonts.load("900 80px Archivo").catch(() => {});
      document.fonts.load("400 20px Archivo").catch(() => {});
    }
  }

  function encerrar() {
    rodando = false;
    cancelAnimationFrame(raf);
    if (voz) voz.cancel();
    if (audio) audio.pararNarracao();
    geracao++;                     // cancela qualquer narração pendente
    mundo.objeto(null);
    if (mundo.somenteObjetos) mundo.somenteObjetos(false);
    if (mundo.hudEscala) mundo.hudEscala(1);
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
    if (!tipo || !tela.captureStream) { aviso.textContent = t("edit.sem_gravacao"); gravando = false; return; }
    // o tamanho do quadro precisa estar definido ANTES de abrir o fluxo:
    // redimensionar depois deixa o vídeo gravado com o tamanho errado
    gravando = true;
    medir();
    const fluxo = tela.captureStream(30);
    const trilha = audio ? audio.trilhaDeGravacao() : null;
    if (trilha) fluxo.addTrack(trilha);
    pedacos = [];
    gravador = new MediaRecorder(fluxo, { mimeType: tipo, videoBitsPerSecond: 3_500_000 });
    gravador.ondataavailable = (e) => { if (e.data.size) pedacos.push(e.data); };
    gravador.onstop = salvar;
    gravador.start(1000);
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
  // ?debug=1&auto=1 abre o vídeo sozinho: serve para os testes automáticos.
  try {
    const busca = new URLSearchParams(location.search);
    if (busca.get("debug") && busca.get("auto")) setTimeout(abrir, 600);
  } catch { /* sem URL utilizável */ }
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
  // Durante a gravação o tamanho do quadro não pode mudar: o fluxo de vídeo
  // já foi aberto com o tamanho atual.
  window.addEventListener("resize", () => { if (rodando && !gravando) medir(); });

  botaoGravar.textContent = t("edit.gravar");
  atualizarVoz();
  // Estado do vídeo para depuração, só com ?debug=1 na URL.
  try {
    if (new URLSearchParams(location.search).get("debug")) {
      globalThis.__video = () => ({ cenaAtual, rodando, narrouAcabou, cortePedido, gravando, comVoz, quadros, naCena: (performance.now() - inicioCena) / 1000, minimo: CENAS[Math.max(0, cenaAtual)].compassos * compasso });
    }
  } catch { /* sem URL utilizável */ }
  aoMudarIdioma(() => {
    botaoGravar.textContent = gravando ? t("edit.gravando") : t("edit.gravar");
    atualizarVoz();
  });
}
