// O herói da abertura: o "A" da Auron como peça usinada de metal.
//
// A primeira tela mostra o A em SVG (desenha sem esperar o Three.js). Quando o
// mundo 3D fica pronto, esta peça é desenhada POR BAIXO do SVG, na mesma pose e
// no mesmo enquadramento: câmera de teleobjetiva (FOV 18°) e escala casada com a
// caixa do elemento. O SVG some e a peça gira 22°: o que parecia um logo plano
// mostra que tem espessura, chanfro e reflexo. Um filete de luz corre pela face
// uma vez só. Depois, a peça fica parada: parado significa confiante.
//
// Custo: 2 malhas, 2 draw calls, matcap gerada em canvas (sem PMREM, sem luz).

import * as THREE from "../../vendor/three.module.min.js";

const FOV = 18, DISTANCIA = 400, GIRO = THREE.MathUtils.degToRad(-22);

// Os dois caminhos do logo.svg, em coordenadas de 0 a 100, com Y invertido.
function formaCorpo() {
  const s = new THREE.Shape();
  const p = (x, y) => [x, 100 - y];
  s.moveTo(...p(50, 6)); s.lineTo(...p(95, 92)); s.lineTo(...p(76, 92));
  s.lineTo(...p(50, 42)); s.lineTo(...p(24, 92)); s.lineTo(...p(5, 92)); s.closePath();
  return s;
}
function formaLamina() {
  const s = new THREE.Shape();
  const p = (x, y) => [x, 100 - y];
  s.moveTo(...p(8, 90));
  s.bezierCurveTo(...p(30, 75), ...p(51, 64), ...p(75, 57));
  s.lineTo(...p(78.5, 65.5));
  s.bezierCurveTo(...p(56, 69.5), ...p(33, 78), ...p(8, 90));
  return s;
}

/** Matcap de estúdio: metal grafite, mancha quente, aro prata e faixa de luz. */
function matcap(tamanho) {
  const c = document.createElement("canvas");
  c.width = c.height = tamanho;
  const x = c.getContext("2d"), t = tamanho;
  let g = x.createRadialGradient(t * 0.45, t * 0.4, t * 0.05, t / 2, t / 2, t * 0.52);
  g.addColorStop(0, "#c9ccd1"); g.addColorStop(0.55, "#6c7077"); g.addColorStop(1, "#141518");
  x.fillStyle = g; x.fillRect(0, 0, t, t);
  const mancha = (cx, cy, r, cor) => {
    const m = x.createRadialGradient(cx * t, cy * t, 0, cx * t, cy * t, r * t);
    m.addColorStop(0, cor); m.addColorStop(1, "rgba(0,0,0,0)");
    x.fillStyle = m; x.fillRect(0, 0, t, t);
  };
  mancha(0.32, 0.28, 0.24, "rgba(255,243,223,0.9)");
  mancha(0.78, 0.66, 0.3, "rgba(214,216,220,0.55)");
  x.fillStyle = "rgba(253,253,251,0.65)"; x.fillRect(0, t * 0.34, t, t * 0.035);
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

const RAMPA = `
vec3 srgbLinear(vec3 c) { return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c)); }
vec3 rampaMetal(float t) {
  vec3 c0 = vec3(.969,.969,.961), c1 = vec3(.706,.722,.745), c2 = vec3(.992,.992,.984), c3 = vec3(.478,.494,.525), c4 = vec3(.839,.847,.863);
  vec3 c = t < .34 ? mix(c0, c1, t / .34) : t < .52 ? mix(c1, c2, (t - .34) / .18) : t < .76 ? mix(c2, c3, (t - .52) / .24) : mix(c3, c4, (t - .76) / .24);
  return srgbLinear(c);
}`;

/**
 * Material do metal. A face da frente repete o gradiente do SVG (mesma caixa,
 * mesma diagonal), então com a peça a 0° o 3D e o SVG ficam iguais. Laterais e
 * chanfro usam a matcap: é ali que o giro revela o volume.
 */
function material(tex, caixa, comum) {
  const m = new THREE.MeshMatcapMaterial({ matcap: tex, toneMapped: false });
  m.onBeforeCompile = (s) => {
    s.uniforms.uCaixa = { value: new THREE.Vector4(...caixa) };
    s.uniforms.uVarredura = comum.uVarredura;
    s.uniforms.uForca = comum.uForca;
    s.vertexShader = s.vertexShader
      .replace("#include <common>", "#include <common>\nuniform vec4 uCaixa; varying vec2 vB; varying float vFrente;")
      .replace("#include <begin_vertex>",
        "#include <begin_vertex>\n" +
        "vec2 svg = vec2(position.x + 50.0, 50.0 - position.y);\n" +
        "vB = (svg - uCaixa.xy) / (uCaixa.zw - uCaixa.xy);\n" +
        "vFrente = step(0.9, normal.z);");
    s.fragmentShader = s.fragmentShader
      .replace("#include <common>", "#include <common>\nuniform float uVarredura, uForca; varying vec2 vB; varying float vFrente;" + RAMPA)
      .replace("#include <opaque_fragment>",
        "outgoingLight = mix(outgoingLight, rampaMetal(clamp((vB.x + vB.y) * 0.5, 0.0, 1.0)), vFrente * 0.92);\n" +
        "float faixa = smoothstep(0.07, 0.0, abs((vB.x - vB.y) * 0.5 + 0.5 - uVarredura));\n" +
        "outgoingLight += srgbLinear(vec3(1.0, 0.957, 0.878)) * faixa * uForca * (0.35 + 0.65 * vFrente);\n" +
        "#include <opaque_fragment>");
  };
  return m;
}

export function criarHeroi(renderer, q) {
  const detalhe = q.nivel === "HIGH" ? 3 : q.nivel === "MEDIUM" ? 2 : 1;
  const curvas = q.nivel === "HIGH" ? 24 : q.nivel === "MEDIUM" ? 12 : 8;
  const cena = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(FOV, 1, 10, 2000);
  camera.position.set(0, 0, DISTANCIA);

  const tex = matcap(q.nivel === "HIGH" ? 256 : 128);
  const comum = { uVarredura: { value: -1 }, uForca: { value: 0 } };
  const bevel = { bevelEnabled: true, bevelThickness: 0.8, bevelSize: 0.6, bevelSegments: detalhe, curveSegments: curvas };

  const gCorpo = new THREE.ExtrudeGeometry(formaCorpo(), { depth: 9, ...bevel });
  gCorpo.translate(-50, -50, -4.5);
  const gLamina = new THREE.ExtrudeGeometry(formaLamina(), { depth: 10.5, ...bevel });
  gLamina.translate(-50, -50, -5.25);

  const peca = new THREE.Group();
  peca.add(new THREE.Mesh(gCorpo, material(tex, [5, 6, 95, 92], comum)));
  peca.add(new THREE.Mesh(gLamina, material(tex, [8, 57, 78.5, 90], comum)));
  cena.add(peca);

  let alvo = null, aoTrocar = null, estado = "espera", inicioGolpe = 0, visivel = false;
  const suave = (x) => 1 - Math.pow(1 - Math.min(1, Math.max(0, x)), 3);

  function alinhar() {
    const r = alvo.getBoundingClientRect();
    visivel = r.bottom > 0 && r.top < innerHeight && r.height > 0;
    if (!visivel) return;
    camera.aspect = innerWidth / innerHeight;
    camera.updateProjectionMatrix();
    // unidades de mundo por pixel CSS no plano z = 0
    const s = (2 * DISTANCIA * Math.tan(THREE.MathUtils.degToRad(FOV / 2))) / innerHeight;
    peca.scale.setScalar((r.height / 100) * s);
    peca.position.set((r.left + r.width / 2 - innerWidth / 2) * s, -(r.top + r.height / 2 - innerHeight / 2) * s, 0);
  }

  return {
    /** Registra o elemento do SVG e quem esconde o SVG quando o 3D assumir. */
    armar(elemento, trocar) { alvo = elemento; aoTrocar = trocar; estado = "pronto"; },
    get ativo() { return estado !== "espera"; },
    /** Chamado a cada quadro, depois da cena principal. */
    desenhar(agora) {
      if (!alvo) return;
      alinhar();
      if (!visivel) return;
      // o golpe só depois de a abertura em SVG terminar de entrar
      if (estado === "pronto" && agora > 1900) { estado = "golpe"; inicioGolpe = agora; }
      if (estado === "golpe" || estado === "parado") {
        const t = (agora - inicioGolpe) / 1000;
        if (t > 0.24 && aoTrocar) { aoTrocar(); aoTrocar = null; }
        peca.rotation.y = GIRO * suave(t / 0.968);
        comum.uVarredura.value = -0.2 + 1.4 * suave(t / 0.6);
        comum.uForca.value = t < 0.9 ? Math.sin(Math.min(1, t / 0.9) * Math.PI) * 1.4 : 0;
        if (t > 1.2) estado = "parado";
      }
      const antes = renderer.autoClear;
      renderer.autoClear = false;
      renderer.clearDepth();
      renderer.render(cena, camera);
      renderer.autoClear = antes;
    },
  };
}
