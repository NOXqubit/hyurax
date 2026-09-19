// O Éter: um arquivo vira pedaços que atravessam caminhos diferentes.
//
// Os pedaços e as provas são de verdade: cada pedaço tem o SHA-512 dele, e a
// raiz de Merkle de todos vai na frente (o "aviso"). Do outro lado, a montagem
// só termina se a raiz recalculada bater. Os caminhos são desenhados: Wi-Fi e
// pasta existem no código hoje; Bluetooth e rádio estão projetados e aparecem
// tracejados, sem carregar dado. O rádio leva só o aviso, como no projeto.

import { merkle, sha512, hex, curto } from "../simulation/engine.js";

const PEDACOS = 24;
const TAM = 64;
const MEIOS = [
  { id: "wifi", pronto: true, velocidade: 1.1 },
  { id: "pasta", pronto: true, velocidade: 2.8 },
  { id: "bluetooth", pronto: false },
  { id: "radio", pronto: false },
];
const COR_ETER = "169, 216, 255";
const COR_LUZ = "242, 217, 168";

export function iniciarEter({ t, calmo }) {
  const tela = document.getElementById("eter-tela");
  const botoes = document.getElementById("eter-meios");
  const status = document.getElementById("eter-status");
  if (!tela) return { redesenhar() {} };
  const ctx = tela.getContext("2d");
  const ativo = { wifi: true, pasta: true };

  // ---------- objeto de verdade ----------
  let pedacos = [], raizEsperada = "", chegou = [], voando = [], proximo = 0, vez = 0, fim = 0, avisoEm = 0, rodada = 0;

  async function novoObjeto() {
    rodada += 1;
    const dados = new Uint8Array(PEDACOS * TAM);
    for (let i = 0; i < dados.length; i++) dados[i] = (i * 31 + rodada * 17 + (i >> 6) * 7) & 0xff;
    pedacos = Array.from({ length: PEDACOS }, (_, i) => dados.slice(i * TAM, (i + 1) * TAM));
    raizEsperada = hex(await merkle(pedacos));
    chegou = new Array(PEDACOS).fill(null);
    voando = [];
    proximo = 0;
    fim = 0;
    avisoEm = performance.now();
    status.className = "veredito";
    status.textContent = t("eter.saindo", { n: PEDACOS, r: curto(raizEsperada, 4) });
  }

  // ---------- botões dos meios ----------
  function montarBotoes() {
    botoes.replaceChildren(...MEIOS.map((m) => {
      const b = document.createElement(m.pronto ? "button" : "span");
      b.className = "meio";
      if (m.pronto) {
        b.type = "button";
        b.setAttribute("aria-pressed", String(ativo[m.id]));
        b.addEventListener("click", () => { ativo[m.id] = !ativo[m.id]; montarBotoes(); });
      } else {
        b.dataset.projetado = "true";
      }
      const marcador = document.createElement("span"); marcador.className = "marcador";
      const nome = document.createElement("span"); nome.textContent = t(`eter.meio.${m.id}`);
      const sit = document.createElement("span"); sit.className = "situacao";
      sit.textContent = m.pronto ? t(ativo[m.id] ? "eter.ligado" : "eter.cortado") : t("eter.projetado");
      b.append(marcador, nome, sit);
      return b;
    }));
  }

  // ---------- geometria ----------
  let w = 0, h = 0, dpr = 1;
  function ajustar() {
    dpr = Math.min(window.devicePixelRatio || 1, 2);
    w = tela.clientWidth; h = tela.clientHeight;
    tela.width = Math.round(w * dpr); tela.height = Math.round(h * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  new ResizeObserver(ajustar).observe(tela);
  ajustar();

  const origem = () => ({ x: w * 0.08, y: h * 0.5 });
  const destino = () => ({ x: w * 0.78, y: h * 0.5 });
  const faixaY = (k) => h * (0.16 + k * 0.23);
  function ponto(k, f) {
    const o = origem(), d = destino(), y = faixaY(k);
    // Curva de Bézier cúbica: sai da origem, corre pela faixa, chega ao destino.
    const p0 = o, p1 = { x: w * 0.3, y }, p2 = { x: w * 0.56, y }, p3 = d;
    const u = 1 - f;
    return {
      x: u * u * u * p0.x + 3 * u * u * f * p1.x + 3 * u * f * f * p2.x + f * f * f * p3.x,
      y: u * u * u * p0.y + 3 * u * u * f * p1.y + 3 * u * f * f * p2.y + f * f * f * p3.y,
    };
  }

  function desenhar(agora) {
    ctx.clearRect(0, 0, w, h);
    ctx.font = `11px "IBM Plex Mono", ui-monospace, monospace`;
    // caminhos
    MEIOS.forEach((m, k) => {
      const ligado = m.pronto ? ativo[m.id] : false;
      ctx.beginPath();
      for (let s = 0; s <= 40; s++) { const p = ponto(k, s / 40); s ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y); }
      ctx.setLineDash(m.pronto ? [] : [4, 6]);
      ctx.strokeStyle = `rgba(${COR_ETER}, ${ligado ? 0.42 : m.pronto ? 0.1 : 0.22})`;
      ctx.lineWidth = ligado ? 1.4 : 1;
      ctx.stroke();
      ctx.setLineDash([]);
      const rot = ponto(k, 0.42);
      ctx.fillStyle = `rgba(${COR_ETER}, ${ligado ? 0.85 : 0.4})`;
      ctx.fillText(t(`eter.meio.${m.id}`), rot.x, rot.y - 7);
    });
    // aviso pelo rádio: um pulso só, no começo de cada objeto
    const fa = (agora - avisoEm) / 1800;
    if (fa >= 0 && fa <= 1) {
      const p = ponto(3, fa);
      ctx.fillStyle = `rgba(${COR_ETER}, 0.95)`;
      ctx.beginPath(); ctx.arc(p.x, p.y, 3.4, 0, Math.PI * 2); ctx.fill();
    }
    // pedaços voando
    for (const v of voando) {
      const f = Math.min(1, (agora - v.inicio) / (v.dur * 1000));
      const p = ponto(v.faixa, f);
      ctx.fillStyle = `rgba(${COR_LUZ}, 0.95)`;
      ctx.shadowColor = `rgba(${COR_LUZ}, 0.8)`; ctx.shadowBlur = 8;
      ctx.fillRect(p.x - 3.5, p.y - 3.5, 7, 7);
      ctx.shadowBlur = 0;
    }
    // origem e destino
    const o = origem(), d = destino();
    for (const [c, r] of [[o, 7], [d, 7]]) {
      ctx.fillStyle = `rgba(${COR_LUZ}, 1)`;
      ctx.shadowColor = `rgba(${COR_LUZ}, 0.7)`; ctx.shadowBlur = 16;
      ctx.beginPath(); ctx.arc(c.x, c.y, r, 0, Math.PI * 2); ctx.fill();
      ctx.shadowBlur = 0;
    }
    ctx.fillStyle = "rgba(178, 181, 186, 0.9)";
    ctx.fillText(t("eter.origem"), o.x - 12, o.y + 24);
    ctx.textAlign = "right";
    ctx.fillText(t("eter.destino"), d.x - 12, d.y + 24);
    ctx.textAlign = "left";
    // mosaico montando no destino
    const lado = Math.min(14, (w - d.x - 24) / 6.4);
    const x0 = d.x + 22, y0 = d.y - lado * 2;
    chegou.forEach((c, i) => {
      const cx = x0 + (i % 6) * (lado + 2), cy = y0 + Math.floor(i / 6) * (lado + 2);
      ctx.fillStyle = c ? `rgba(${COR_LUZ}, ${0.35 + ((i * 37) % 60) / 100})` : "rgba(236, 234, 228, 0.06)";
      ctx.fillRect(cx, cy, lado, lado);
    });
  }

  // ---------- andamento ----------
  let ultimoEnvio = 0;
  async function passo(agora) {
    const prontos = MEIOS.map((m, k) => ({ m, k })).filter(({ m }) => m.pronto && ativo[m.id]);
    if (proximo < PEDACOS && agora - ultimoEnvio > 150) {
      if (prontos.length) {
        const { m, k } = prontos[vez++ % prontos.length];
        voando.push({ i: proximo++, faixa: k, inicio: agora, dur: m.velocidade });
        ultimoEnvio = agora;
      } else if (!fim) {
        status.className = "veredito";
        status.textContent = t("eter.esperando", { f: PEDACOS - proximo });
      }
    }
    for (let x = voando.length - 1; x >= 0; x--) {
      const v = voando[x];
      if (agora - v.inicio >= v.dur * 1000) {
        // Cada pedaço é conferido na chegada contra o seu SHA-512.
        const confere = hex(await sha512(pedacos[v.i]));
        chegou[v.i] = confere;
        voando.splice(x, 1);
      }
    }
    if (!fim && chegou.every(Boolean)) {
      fim = agora;
      const raiz = hex(await merkle(pedacos));
      const ok = raiz === raizEsperada;
      status.className = "veredito " + (ok ? "ok" : "ruim");
      status.textContent = ok ? t("eter.montado", { n: PEDACOS, r: curto(raiz, 4) }) : t("eter.falhou");
    } else if (!fim && prontos.length && proximo > 0) {
      status.className = "veredito";
      status.textContent = t("eter.andando", { c: chegou.filter(Boolean).length, n: PEDACOS });
    }
    if (fim && agora - fim > 3200) await novoObjeto();
  }

  let visivel = false;
  new IntersectionObserver((es) => { visivel = es[0].isIntersecting; }, { threshold: 0.15 }).observe(tela);
  let ocupado = false;
  function laco(agora) {
    requestAnimationFrame(laco);
    if (!visivel || document.hidden || ocupado) return;
    ocupado = true;
    passo(agora).then(() => { desenhar(agora); ocupado = false; });
  }

  montarBotoes();
  novoObjeto().then(() => {
    if (calmo) {
      // Movimento reduzido: mostra o objeto já montado e o resultado.
      chegou = chegou.map(() => "ok");
      desenhar(performance.now() + 99999);
      status.className = "veredito ok";
      status.textContent = t("eter.montado", { n: PEDACOS, r: curto(raizEsperada, 4) });
    } else {
      requestAnimationFrame(laco);
    }
  });

  return {
    redesenhar() { montarBotoes(); desenhar(performance.now()); },
  };
}
