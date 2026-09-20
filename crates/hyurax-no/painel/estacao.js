// Estação 3D do minerador: a mesa, o PC que minera e a corrente de blocos.
//
// Tudo reage ao estado real do nó:
// - a tela do monitor mostra altura, ritmo, tentativas e os últimos hashes;
// - a torre acende e as ventoinhas giram conforme o ritmo da mineração;
// - cada bloco novo entra na corrente: os meus saem da torre (cubo sólido),
//   os de outros nós chegam de longe (cubo aramado).

import * as THREE from "./three.module.min.js";

const BRANCO = 0xf3f3f1;
const NA_CORRENTE = 12;
const LADO = 0.26;

// Cada qualidade é um acordo entre desenho bonito e máquina livre. "auto"
// decide pelo que o navegador informa: pouca memória ou poucos núcleos caem
// para baixa. O que muda é só o desenho; a mineração não muda em nada.
const QUALIDADES = {
  alta: { dpr: 1.5, quadro: 33, suave: true, girar: true },
  media: { dpr: 1.0, quadro: 50, suave: true, girar: true },
  baixa: { dpr: 0.75, quadro: 100, suave: false, girar: false },
};

function decidirQualidade(pedida) {
  if (pedida && pedida !== "auto") return QUALIDADES[pedida] || QUALIDADES.media;
  const nucleos = navigator.hardwareConcurrency || 2;
  const memoria = navigator.deviceMemory || 4;
  if (nucleos <= 2 || memoria <= 2) return QUALIDADES.baixa;
  if (nucleos <= 4 || memoria <= 4) return QUALIDADES.media;
  return QUALIDADES.alta;
}

export function criarEstacao(canvas, { calmo = false, qualidade = "auto" } = {}) {
  const q = decidirQualidade(qualidade);
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: q.suave, powerPreference: "low-power" });
  renderer.setPixelRatio(Math.min(devicePixelRatio || 1, q.dpr));
  renderer.setClearColor(0x0a0a0a, 1);
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  const cena = new THREE.Scene();
  cena.fog = new THREE.Fog(0x0a0a0a, 6, 13);
  const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 50);
  const alvo = new THREE.Vector3(0.8, 1.4, -0.35);

  // ---------- luz ----------
  cena.add(new THREE.HemisphereLight(0xffffff, 0x111111, 0.55));
  const chave = new THREE.DirectionalLight(0xffffff, 1.4);
  chave.position.set(-3, 5, 4);
  cena.add(chave);
  const brilhoTela = new THREE.PointLight(0xffffff, 1.2, 3.2, 2);
  brilhoTela.position.set(0, 1.25, 0.55);
  cena.add(brilhoTela);
  const brilhoTorre = new THREE.PointLight(0xffffff, 0, 2.2, 2);
  brilhoTorre.position.set(1.62, 0.55, 0.75);
  cena.add(brilhoTorre);

  // ---------- materiais ----------
  const fosco = (tom, rugoso = 0.85, metal = 0.1) => new THREE.MeshStandardMaterial({ color: tom, roughness: rugoso, metalness: metal });
  const mCorpo = fosco(0x1b1b1b, 0.55, 0.4);
  const mMesa = fosco(0x151515, 0.7, 0.2);
  const mClaro = fosco(0x8a8a88, 0.5, 0.5);
  const mLed = new THREE.MeshStandardMaterial({ color: 0x222222, emissive: BRANCO, emissiveIntensity: 0.1 });

  const caixa = (l, a, p, m, x, y, z) => {
    const o = new THREE.Mesh(new THREE.BoxGeometry(l, a, p), m);
    o.position.set(x, y, z);
    cena.add(o);
    return o;
  };

  // ---------- chão ----------
  const grade = new THREE.GridHelper(16, 32, 0x2a2a2a, 0x161616);
  cena.add(grade);

  // ---------- mesa ----------
  caixa(2.5, 0.05, 1.05, mMesa, 0.1, 0.76, 0);
  for (const [x, z] of [[-1.08, -0.45], [1.28, -0.45], [-1.08, 0.45], [1.28, 0.45]]) caixa(0.05, 0.74, 0.05, mCorpo, x, 0.37, z);

  // ---------- monitor ----------
  caixa(0.42, 0.02, 0.24, mCorpo, 0, 0.795, -0.18);
  caixa(0.05, 0.36, 0.04, mCorpo, 0, 0.98, -0.22);
  caixa(1.32, 0.78, 0.045, mCorpo, 0, 1.36, -0.19);
  const telaCanvas = document.createElement("canvas");
  telaCanvas.width = 1024;
  telaCanvas.height = 576;
  const telaTex = new THREE.CanvasTexture(telaCanvas);
  telaTex.colorSpace = THREE.SRGBColorSpace;
  telaTex.anisotropy = 4;
  const tela = new THREE.Mesh(new THREE.PlaneGeometry(1.26, 0.709), new THREE.MeshBasicMaterial({ map: telaTex, toneMapped: false }));
  tela.position.set(0, 1.36, -0.166);
  cena.add(tela);

  // ---------- teclado, mouse, caneca ----------
  caixa(0.86, 0.025, 0.26, mCorpo, -0.05, 0.795, 0.22);
  const teclas = new THREE.InstancedMesh(new THREE.BoxGeometry(0.048, 0.012, 0.045), mClaro, 14 * 4);
  let k = 0;
  const mat4 = new THREE.Matrix4();
  for (let fila = 0; fila < 4; fila++) {
    for (let col = 0; col < 14; col++) {
      mat4.makeTranslation(-0.05 - 0.39 + col * 0.06, 0.814, 0.14 + fila * 0.055);
      teclas.setMatrixAt(k++, mat4);
    }
  }
  cena.add(teclas);
  caixa(0.07, 0.025, 0.11, mCorpo, 0.52, 0.795, 0.24);
  const caneca = new THREE.Mesh(new THREE.CylinderGeometry(0.06, 0.055, 0.13, 20, 1, true), fosco(0xd8d8d4, 0.35, 0.05));
  caneca.material.side = THREE.DoubleSide;
  caneca.position.set(-0.85, 0.85, 0.12);
  cena.add(caneca);
  const alca = new THREE.Mesh(new THREE.TorusGeometry(0.035, 0.009, 8, 16, Math.PI), caneca.material);
  alca.rotation.z = -Math.PI / 2;
  alca.position.set(-0.79, 0.85, 0.12);
  cena.add(alca);

  // ---------- torre (o PC que minera) ----------
  const TX = 1.62;
  caixa(0.46, 1.0, 0.98, mCorpo, TX, 0.5, 0);
  // vidro lateral
  const vidro = new THREE.Mesh(
    new THREE.PlaneGeometry(0.86, 0.86),
    new THREE.MeshStandardMaterial({ color: 0x0c0c0c, roughness: 0.1, metalness: 0.6, transparent: true, opacity: 0.75 }),
  );
  vidro.rotation.y = -Math.PI / 2;
  vidro.position.set(TX - 0.232, 0.5, 0);
  cena.add(vidro);
  // faixa de luz na frente: acende com o ritmo
  const faixa = caixa(0.012, 0.86, 0.02, mLed, TX - 0.12, 0.5, 0.492);
  const faixa2 = caixa(0.012, 0.86, 0.02, mLed, TX + 0.12, 0.5, 0.492);
  // ventoinhas na frente
  const ventoinhas = [];
  for (const y of [0.72, 0.3]) {
    const aro = new THREE.Mesh(new THREE.TorusGeometry(0.1, 0.008, 8, 32), mClaro);
    aro.position.set(TX, y, 0.495);
    cena.add(aro);
    const helice = new THREE.Group();
    for (let i = 0; i < 7; i++) {
      const pa = new THREE.Mesh(new THREE.BoxGeometry(0.085, 0.022, 0.004), mClaro);
      pa.position.x = 0.045;
      const suporte = new THREE.Group();
      suporte.rotation.z = (i / 7) * Math.PI * 2;
      pa.rotation.x = 0.4;
      suporte.add(pa);
      helice.add(suporte);
    }
    helice.position.set(TX, y, 0.5);
    cena.add(helice);
    ventoinhas.push(helice);
  }
  // cabo da torre até a mesa, só um fio
  const cabo = new THREE.Mesh(
    new THREE.TubeGeometry(new THREE.CatmullRomCurve3([new THREE.Vector3(TX - 0.2, 0.9, -0.4), new THREE.Vector3(1.0, 1.2, -0.45), new THREE.Vector3(0.2, 1.05, -0.3)]), 20, 0.008, 6),
    mCorpo,
  );
  cena.add(cabo);

  // ---------- a corrente de blocos ----------
  const mMeu = new THREE.MeshStandardMaterial({ color: BRANCO, roughness: 0.35, metalness: 0.2, emissive: BRANCO, emissiveIntensity: 0.08 });
  const mOutro = new THREE.MeshBasicMaterial({ color: BRANCO, wireframe: true, transparent: true, opacity: 0.55 });
  const geoCubo = new THREE.BoxGeometry(LADO, LADO, LADO);
  const corrente = []; // { hash, altura, meu, grupo, de: Vector3|null, t }
  const elos = new THREE.LineSegments(new THREE.BufferGeometry(), new THREE.LineBasicMaterial({ color: 0x777777 }));
  cena.add(elos);

  // Posição do i-ésimo bloco (0 = mais antigo): um arco atrás da mesa, subindo para a direita.
  const lugar = (i) => {
    const f = i / (NA_CORRENTE - 1);
    return new THREE.Vector3(-2.3 + f * 3.9, 2.2 + Math.sin(f * Math.PI) * 0.18, -1.0 - Math.cos(f * Math.PI) * 0.35);
  };

  function etiqueta(texto) {
    const c = document.createElement("canvas");
    c.width = 256;
    c.height = 64;
    const g = c.getContext("2d");
    g.fillStyle = "#f3f3f1";
    g.font = "500 34px Consolas, 'Cascadia Mono', monospace";
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.fillText(texto, 128, 32);
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    const s = new THREE.Sprite(new THREE.SpriteMaterial({ map: tex, transparent: true, depthWrite: false }));
    s.scale.set(0.5, 0.125, 1);
    s.position.y = LADO * 0.95;
    return s;
  }

  function novoCubo(b) {
    const grupo = new THREE.Group();
    const cubo = new THREE.Mesh(geoCubo, b.meu ? mMeu : mOutro);
    grupo.add(cubo);
    if (b.meu) {
      // Aresta viva no cubo sólido, para ler a forma de longe.
      grupo.add(new THREE.LineSegments(new THREE.EdgesGeometry(geoCubo), new THREE.LineBasicMaterial({ color: 0x0a0a0a })));
    }
    grupo.add(etiqueta(`#${b.altura}`));
    cena.add(grupo);
    return grupo;
  }

  function sincronizarCorrente(blocos, primeiraVez) {
    const ultimos = blocos.slice(-NA_CORRENTE);
    const hashes = new Set(ultimos.map((b) => b.hash));
    // Sai o que caiu da janela (ou foi trocado numa reorganização).
    for (let i = corrente.length - 1; i >= 0; i--) {
      if (!hashes.has(corrente[i].hash)) {
        cena.remove(corrente[i].grupo);
        // A etiqueta tem textura própria: libera, senão a memória cresce a cada bloco.
        corrente[i].grupo.traverse((o) => {
          if (o.isSprite) { o.material.map?.dispose(); o.material.dispose(); }
          if (o.isLineSegments) { o.geometry.dispose(); o.material.dispose(); }
        });
        corrente.splice(i, 1);
      }
    }
    const tem = new Set(corrente.map((c) => c.hash));
    for (const b of ultimos) {
      if (tem.has(b.hash)) continue;
      const grupo = novoCubo(b);
      // Meu bloco nasce na torre; o de outro nó chega de longe, pela direita.
      const de = primeiraVez || calmo ? null : b.meu ? new THREE.Vector3(TX, 1.05, 0) : new THREE.Vector3(6, 3.2, -4);
      corrente.push({ hash: b.hash, altura: b.altura, meu: b.meu, grupo, de, t: 0 });
    }
    corrente.sort((a, b) => a.altura - b.altura);
    corrente.forEach((c, i) => {
      c.lugar = lugar(i + (NA_CORRENTE - corrente.length));
      if (!c.de && !c.pos) c.grupo.position.copy(c.lugar);
    });
  }

  // ---------- tela do monitor ----------
  let estado = null;
  function pintarTela(agora) {
    const g = telaCanvas.getContext("2d");
    const W = telaCanvas.width, H = telaCanvas.height;
    g.fillStyle = "#050505";
    g.fillRect(0, 0, W, H);
    // linhas de varredura bem leves
    g.fillStyle = "rgba(255,255,255,0.025)";
    for (let y = 0; y < H; y += 4) g.fillRect(0, y, W, 1);
    g.fillStyle = "#f3f3f1";
    g.font = "700 30px Bahnschrift, 'Arial Narrow', sans-serif";
    g.textBaseline = "top";
    g.fillText("HYURAX", 40, 32);
    g.font = "18px Consolas, monospace";
    g.fillStyle = "rgba(243,243,241,0.5)";
    g.fillText(`MINERADOR · ${estado ? estado.rede.replace(/^hyurax-/, "").toUpperCase() : "…"}`, 170, 40);
    if (!estado) return;
    const minerando = estado.minerando;
    const piscar = Math.floor(agora / 500) % 2 === 0;
    g.textAlign = "right";
    g.fillStyle = "#f3f3f1";
    g.fillText(minerando ? `${piscar ? "●" : "○"} MINERANDO` : "○ PARADO", W - 40, 40);
    g.textAlign = "left";

    g.fillStyle = "rgba(243,243,241,0.45)";
    g.font = "16px Consolas, monospace";
    g.fillText(minerando ? "PROCURANDO O BLOCO" : "ALTURA DA CADEIA", 40, 108);
    g.fillStyle = "#f3f3f1";
    g.font = "700 118px Bahnschrift, 'Arial Narrow', sans-serif";
    g.fillText(String(minerando ? estado.rodada_altura || estado.altura + 1 : estado.altura), 34, 128);

    // três leituras
    const col = [["RITMO", `${estado.ritmo.toFixed(1)} t/s`], ["TENTATIVAS", estado.tentativas.toLocaleString("pt-BR")], ["MEUS BLOCOS", String(estado.meus_cadeia)]];
    col.forEach(([r, v], i) => {
      const x = 40 + i * 230;
      g.fillStyle = "rgba(243,243,241,0.45)";
      g.font = "15px Consolas, monospace";
      g.fillText(r, x, 270);
      g.fillStyle = "#f3f3f1";
      g.font = "28px Consolas, monospace";
      g.fillText(v, x, 292);
    });

    // barra de varredura: o nonce sendo procurado
    const bx = 40, by = 352, bw = 680, bh = 10;
    g.strokeStyle = "rgba(243,243,241,0.35)";
    g.strokeRect(bx + 0.5, by + 0.5, bw, bh);
    if (minerando) {
      const f = (agora / 1400) % 1;
      g.fillStyle = "#f3f3f1";
      g.fillRect(bx + f * (bw - 120), by, 120, bh);
    }

    // últimos blocos: hashes de verdade
    g.font = "16px Consolas, monospace";
    const ultimos = [...estado.blocos].reverse().slice(0, 6);
    ultimos.forEach((b, i) => {
      const y = 392 + i * 26;
      g.fillStyle = i === 0 ? "#f3f3f1" : "rgba(243,243,241,0.5)";
      g.fillText(`${b.meu ? "■" : "□"} #${String(b.altura).padEnd(6)} ${b.hash.slice(0, 40)}…`, 40, y);
    });

    // painel lateral da tela: carteira
    g.fillStyle = "rgba(243,243,241,0.06)";
    g.fillRect(760, 108, 224, 250);
    g.fillStyle = "rgba(243,243,241,0.45)";
    g.font = "15px Consolas, monospace";
    g.fillText("SALDO", 780, 124);
    g.fillStyle = "#f3f3f1";
    g.font = "700 40px Bahnschrift, 'Arial Narrow', sans-serif";
    g.fillText(estado.saldo.split(".")[0], 780, 146);
    g.font = "18px Consolas, monospace";
    g.fillText(`.${estado.saldo.split(".")[1] || "0"} HYUR`, 780, 194);
    g.fillStyle = "rgba(243,243,241,0.45)";
    g.font = "15px Consolas, monospace";
    g.fillText("PARES", 780, 240);
    g.fillText("MEMPOOL", 880, 240);
    g.fillStyle = "#f3f3f1";
    g.font = "28px Consolas, monospace";
    g.fillText(String(estado.pares), 780, 262);
    g.fillText(String(estado.mempool), 880, 262);
    // cursor
    if (piscar) g.fillRect(40, H - 28, 12, 3);
  }

  // ---------- câmera: gira devagar, e o mouse arrasta ----------
  let az = 0.3, el = 0.24, raio = 3.7, alvoAz = az, alvoEl = el;
  let arrastando = null;
  canvas.addEventListener("pointerdown", (ev) => { arrastando = { x: ev.clientX, y: ev.clientY, az: alvoAz, el: alvoEl }; canvas.setPointerCapture(ev.pointerId); });
  canvas.addEventListener("pointermove", (ev) => {
    if (!arrastando) return;
    alvoAz = arrastando.az - (ev.clientX - arrastando.x) * 0.006;
    alvoEl = Math.min(0.9, Math.max(0.05, arrastando.el + (ev.clientY - arrastando.y) * 0.004));
  });
  canvas.addEventListener("pointerup", () => { arrastando = null; });
  canvas.addEventListener("wheel", (ev) => { ev.preventDefault(); raio = Math.min(7, Math.max(3, raio + ev.deltaY * 0.002)); }, { passive: false });

  function ajustar() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    if (!w || !h) return;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    // Tela estreita (celular): afasta para caber a mesa inteira.
    camera.fov = w / h < 1.2 ? 52 : 38;
    camera.updateProjectionMatrix();
  }
  new ResizeObserver(ajustar).observe(canvas);
  ajustar();

  // ---------- laço ----------
  let anterior = performance.now(), ultimoQuadro = 0, ultimaTela = 0, girar = 0;
  function quadro(agora) {
    requestAnimationFrame(quadro);
    if (document.hidden || agora - ultimoQuadro < q.quadro) return; // o suficiente, sem torrar CPU
    ultimoQuadro = agora;
    const dt = Math.min(0.1, (agora - anterior) / 1000);
    anterior = agora;

    const minerando = estado?.minerando;
    const ritmo = estado?.ritmo || 0;
    if (!calmo && !arrastando) alvoAz += dt * 0.035 * Math.sin(agora / 9000 + 1.2);
    az += (alvoAz - az) * 0.08;
    el += (alvoEl - el) * 0.08;
    camera.position.set(alvo.x + raio * Math.sin(az) * Math.cos(el), alvo.y + raio * Math.sin(el), alvo.z + raio * Math.cos(az) * Math.cos(el));
    camera.lookAt(alvo);

    // Torre: ventoinhas e luz seguem o ritmo.
    const velocidade = minerando ? 6 + Math.min(ritmo, 40) * 0.6 : 0.4;
    girar += dt * velocidade * (calmo ? 0.2 : 1);
    ventoinhas.forEach((v, i) => (v.rotation.z = girar * (i ? -1 : 1)));
    const pulso = minerando ? 0.55 + 0.45 * Math.abs(Math.sin(agora / (260 - Math.min(ritmo, 40) * 4))) : 0.08;
    mLed.emissiveIntensity = pulso;
    brilhoTorre.intensity = minerando ? pulso * 1.6 : 0;
    faixa.visible = faixa2.visible = true;

    // Blocos voando para o seu lugar na corrente.
    for (const c of corrente) {
      if (c.de) {
        c.t = Math.min(1, c.t + dt / 1.6);
        const e = 1 - Math.pow(1 - c.t, 3);
        const meio = c.de.clone().lerp(c.lugar, 0.5).add(new THREE.Vector3(0, 1.1, 0.4));
        const a = c.de.clone().lerp(meio, e), b = meio.clone().lerp(c.lugar, e);
        c.grupo.position.copy(a.lerp(b, e));
        c.grupo.rotation.y = (1 - e) * Math.PI * 2;
        if (c.t >= 1) c.de = null;
      } else {
        c.grupo.position.lerp(c.lugar, 0.1);
        if (!calmo && q.girar) c.grupo.rotation.y += dt * (c.meu ? 0.35 : 0.15);
      }
    }
    // Elos entre blocos vizinhos.
    const pts = [];
    for (let i = 1; i < corrente.length; i++) pts.push(corrente[i - 1].grupo.position, corrente[i].grupo.position);
    elos.geometry.setFromPoints(pts);

    if (agora - ultimaTela > 120) {
      pintarTela(agora);
      telaTex.needsUpdate = true;
      ultimaTela = agora;
    }
    renderer.render(cena, camera);
  }
  requestAnimationFrame(quadro);

  let primeira = true;
  return {
    atualizar(e) {
      estado = e;
      sincronizarCorrente(e.blocos, primeira);
      primeira = false;
    },
  };
}
