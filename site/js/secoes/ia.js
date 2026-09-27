// Hyurax — capítulo da IA: moléculas de verdade, desenhadas do SMILES.
//
// Oito moléculas da base que a IA do programa usa (AqSolDB, CC0), com a
// solubilidade medida em laboratório. O desenho é o grafo real de cada uma
// (moleculas.js, o mesmo código do programa). Aqui não há previsão: a
// previsão é do modelo treinado no computador de quem roda o programa.

import { desenharMolecula } from "./moleculas.js";

// id, nome, fórmula, SMILES, massa molar (mg/mol) e log S medido (× 1000),
// exatamente como estão em crates/hyurax-ultrax/dados/moleculas.tsv.
const MOLECULAS = [
  ["B-205", "cânfora", "C10H16O", "CC1(C)C2CCC1(C)C(=O)C2", 152237, -1978],
  ["C-791", "sulpirida", "C15H23N3O4S", "CCN1CCCC1CNC(=O)C2=C(OC)C=CC(=C2)[S](N)(=O)=O", 341433, -2880],
  ["E-411", "17α-hidroxiprogesterona", "C21H30O3", "CC(=O)C3(O)CCC4C2CCC1=CC(=O)CCC1(C)C2CCC34C", 330468, -4710],
  ["B-945", "isoleucina", "C6H13NO2", "CC[C@H](C)[C@@H](N)C(O)=O", 131175, -788],
  ["E-878", "vasicinona", "C11H12N2O2", "c1ccc2C(O)N3CCC(O)C3=Nc2c1", 204229, -2070],
  ["C-307", "pteridina", "C6H4N4", "C1=NC2=C(N=C1)N=CN=C2", 132126, 20],
  ["B-3779", "ácido cúmico", "C10H12O2", "CC(C)c1ccc(cc1)C(O)=O", 164204, -3036],
  ["B-343", "1-bromonaftaleno", "C10H7Br", "Brc1cccc2ccccc12", 207070, -4347],
];

// log S (mol/L) e massa molar dão a solubilidade em g/L, que se lê melhor.
function gramas(logsMili, massaMili, idioma) {
  const gl = Math.pow(10, logsMili / 1000) * (massaMili / 1000);
  const n = (v, c) => v.toLocaleString(idioma, { maximumFractionDigits: c, minimumFractionDigits: c });
  if (gl >= 100) return `${n(gl, 0)} g/L`;
  if (gl >= 1) return `${n(gl, 1)} g/L`;
  if (gl >= 0.001) return `${n(gl * 1000, 1)} mg/L`;
  return `${n(gl * 1e6, 1)} µg/L`;
}

export function iniciarIa({ t, idioma }) {
  const canvas = document.getElementById("mol-desenho");
  if (!canvas) return { redesenhar() {} };
  let vez = 0;
  const cores = () => {
    const css = getComputedStyle(canvas);
    return { tinta: css.color || "#000", fraca: "rgba(0,0,0,.5)", fio: "rgba(0,0,0,.2)", fonte: getComputedStyle(document.body).fontFamily };
  };
  function mostrar() {
    const [id, nome, formula, smiles, massa, logs] = MOLECULAS[vez % MOLECULAS.length];
    document.getElementById("mol-nome").textContent = nome;
    document.getElementById("mol-formula").textContent = `${formula} · ${(massa / 1000).toLocaleString(idioma(), { maximumFractionDigits: 1 })} g/mol · AqSolDB ${id}`;
    const s = (logs / 1000).toLocaleString(idioma(), { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    document.getElementById("mol-medida").textContent = t("ia.medida", { s, g: gramas(logs, massa, idioma()) });
    canvas.setAttribute("aria-label", `${nome}: ${smiles}`);
    desenharMolecula(canvas, smiles, cores());
  }
  document.getElementById("mol-outra")?.addEventListener("click", () => {
    vez += 1;
    mostrar();
  });
  addEventListener("resize", mostrar);
  mostrar();
  return { redesenhar: mostrar };
}
