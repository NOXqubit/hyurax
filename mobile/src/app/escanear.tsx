import { CameraView, useCameraPermissions } from "expo-camera";
import { router } from "expo-router";
import { useRef } from "react";
import { View } from "react-native";

import { Botao, Tela, Texto, Titulo } from "../ui/componentes";

export default function Escanear() {
  const [permissao, pedir] = useCameraPermissions();
  const lido = useRef(false);

  if (!permissao) return <View style={{ flex: 1 }} />;
  if (!permissao.granted) {
    return (
      <Tela>
        <Titulo sub="A câmera serve só para ler o QR do endereço. Nada é gravado nem sai do aparelho.">Câmera</Titulo>
        <Botao titulo="Permitir a câmera" grande aoTocar={pedir} />
        {!permissao.canAskAgain ? <Texto fraco>A permissão foi negada antes. Libere em Configurações → Apps → Hyurax.</Texto> : null}
      </Tela>
    );
  }
  return (
    <View style={{ flex: 1, backgroundColor: "#000" }}>
      <CameraView
        style={{ flex: 1 }}
        facing="back"
        barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
        onBarcodeScanned={({ data }) => {
          if (lido.current) return;
          lido.current = true;
          // aceita "hyurax:thyx1…" e "thyx1…"
          const destino = String(data).replace(/^hyurax:/i, "").split("?")[0].trim();
          router.dismissTo({ pathname: "/enviar", params: { destino } });
        }}
      />
    </View>
  );
}
