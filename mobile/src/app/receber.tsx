import * as Clipboard from "expo-clipboard";
import { useState } from "react";
import { Share, Text, View } from "react-native";

import { useCarteira } from "../estado/carteira";
import { Botao, Cartao, SeloTeste, Tela, Texto, Titulo } from "../ui/componentes";
import { QR } from "../ui/qr";
import { useTema } from "../ui/tema";

export default function Receber() {
  const t = useTema();
  const c = useCarteira();
  const [copiado, setCopiado] = useState(false);
  const e = c.enderecoTexto ?? "";
  return (
    <Tela>
      <Titulo sub="Mostre o QR ou mande o endereço. Ele tem dígito verificador: um erro de digitação é recusado, em vez de mandar HYX para o lugar errado.">Receber HYX</Titulo>
      <Cartao style={{ alignItems: "center", gap: 18, paddingVertical: 28 }}>
        <SeloTeste rede={c.rede} />
        <View style={{ padding: 12, backgroundColor: "#FFFFFF", borderRadius: 20 }}>{e ? <QR valor={e} tamanho={232} /> : null}</View>
        <Text selectable style={{ color: t.tinta, fontFamily: "monospace", fontSize: 14, textAlign: "center", lineHeight: 21 }}>
          {e}
        </Text>
      </Cartao>
      <Botao
        titulo={copiado ? "Copiado" : "Copiar endereço"}
        aoTocar={async () => {
          await Clipboard.setStringAsync(e);
          setCopiado(true);
          setTimeout(() => setCopiado(false), 2000);
        }}
      />
      <Botao titulo="Compartilhar" tipo="leve" aoTocar={() => Share.share({ message: e })} />
      {!c.info ? <Texto fraco pequeno>Sem nó conectado, o endereço sai no formato da rede de teste (thyx1…).</Texto> : null}
    </Tela>
  );
}
