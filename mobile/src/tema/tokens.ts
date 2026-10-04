// Cloud Design 2.0 2.0.0 — GERADO por scripts/gerar-tokens.py a partir de design/cloud-design-2.json. Não edite à mão.

export const escuro = {
  vazio: "#040405",
  fundo: "#0A0B0D",
  lateral: "#0D0E10",
  painel: "#111214",
  painel2: "#17181B",
  painel3: "#1E2024",
  aco: "#24262A",
  tinta: "#EDEBE6",
  tinta2: "#C4C7CC",
  tinta3: "#8D9199",
  fio: "rgba(237, 235, 230, 0.08)",
  fioForte: "rgba(237, 235, 230, 0.20)",
  luz: "#F2D9A8",
  luzForte: "#FFF4E0",
  luzFundo: "rgba(242, 217, 168, 0.10)",
  sobreLuz: "#141210",
  ok: "#F2D9A8",
  atencao: "#E3A15E",
  falha: "#EE7363",
  info: "#A9BCDB",
  gpu: "#9FBDF2",
} as const;

export type Tema = { readonly [K in keyof typeof escuro]: string };

export const claro: Tema = {
  vazio: "#FFFFFF",
  fundo: "#F2F2F0",
  lateral: "#FAFAF9",
  painel: "#FFFFFF",
  painel2: "#F4F4F2",
  painel3: "#E9E9E6",
  aco: "#D6D6D2",
  tinta: "#0B0C0E",
  tinta2: "#3A3D43",
  tinta3: "#5F636B",
  fio: "rgba(11, 12, 14, 0.09)",
  fioForte: "rgba(11, 12, 14, 0.22)",
  luz: "#7A5715",
  luzForte: "#5C410F",
  luzFundo: "rgba(122, 87, 21, 0.10)",
  sobreLuz: "#FFFFFF",
  ok: "#7A5715",
  atencao: "#9A5418",
  falha: "#B2382A",
  info: "#38598A",
  gpu: "#2F5DA8",
};

export const medidas = {
  raio: 12,
  raioP: 8,
  raioPilula: 999,
  espaco1: 4,
  espaco2: 8,
  espaco3: 12,
  espaco4: 16,
  espaco5: 24,
  espaco6: 32,
  tRotulo: 11,
  tPequeno: 12.5,
  tCorpo: 14,
  tDestaque: 16,
  tTitulo: 22,
  tTela: 28,
  tNumero: 34,
} as const;

export const fontes = {
  texto: "Archivo",
  mono: "IBM Plex Mono",
} as const;
