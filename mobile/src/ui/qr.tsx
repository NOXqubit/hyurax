// QR Code desenhado em SVG com o gerador que o programa do PC já usa
// (Kazuhiko Arase, licença MIT, em src/vendor).

import { useMemo } from "react";
import Svg, { Path, Rect } from "react-native-svg";

// eslint-disable-next-line @typescript-eslint/no-require-imports
const qrcode: (tipo: number, correcao: string) => any = require("../vendor/qrcode.js");

export function QR({ valor, tamanho = 220 }: { valor: string; tamanho?: number }) {
  const { n, caminho } = useMemo(() => {
    const q = qrcode(0, "M");
    q.addData(valor);
    q.make();
    const n: number = q.getModuleCount();
    let d = "";
    for (let l = 0; l < n; l++) for (let c = 0; c < n; c++) if (q.isDark(l, c)) d += `M${c} ${l}h1v1h-1z`;
    return { n, caminho: d };
  }, [valor]);
  const margem = 2;
  return (
    <Svg width={tamanho} height={tamanho} viewBox={`${-margem} ${-margem} ${n + 2 * margem} ${n + 2 * margem}`}>
      <Rect x={-margem} y={-margem} width={n + 2 * margem} height={n + 2 * margem} fill="#FFFFFF" />
      <Path d={caminho} fill="#000000" />
    </Svg>
  );
}
