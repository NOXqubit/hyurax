import { router } from "expo-router";
import { Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { Botao, Marca, Texto } from "../ui/componentes";
import { useTema } from "../ui/tema";

export default function BoasVindas() {
  const t = useTema();
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: t.fundo }}>
      <View style={{ flex: 1, padding: 24, justifyContent: "space-between" }}>
        <View style={{ gap: 20, marginTop: 48 }}>
          <Marca tamanho={72} />
          <Text style={{ color: t.tinta, fontSize: 38, fontWeight: "800", letterSpacing: -1, lineHeight: 44 }}>Sua carteira{"\n"}Hyurax.</Text>
          <Texto>
            A chave fica só neste aparelho. Para enviar, o celular assina aqui e entrega a transação pronta a um nó da rede; a chave nunca sai.
          </Texto>
          <View style={{ alignSelf: "flex-start", backgroundColor: t.luzFundo, borderRadius: 999, paddingHorizontal: 12, paddingVertical: 6 }}>
            <Text style={{ color: t.luzTexto, fontSize: 12.5, fontWeight: "700" }}>Rede de teste: o HYX ainda não tem valor</Text>
          </View>
        </View>
        <View style={{ gap: 12 }}>
          <Botao titulo="Criar carteira nova" grande aoTocar={() => router.push("/criar")} />
          <Botao titulo="Já tenho uma (importar do PC)" tipo="leve" grande aoTocar={() => router.push("/importar")} />
        </View>
      </View>
    </SafeAreaView>
  );
}
