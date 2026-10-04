import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { ProvedorDaCarteira } from "../estado/carteira";
import { useTema } from "../ui/tema";

export default function Raiz() {
  const t = useTema();
  return (
    <SafeAreaProvider>
      <ProvedorDaCarteira>
        <StatusBar style={t.escuro ? "light" : "dark"} />
        <Stack
          screenOptions={{
            headerStyle: { backgroundColor: t.fundo },
            headerTintColor: t.tinta,
            headerShadowVisible: false,
            headerTitleStyle: { fontWeight: "600" },
            contentStyle: { backgroundColor: t.fundo },
            headerBackTitle: "Voltar",
          }}
        >
          <Stack.Screen name="index" options={{ title: "Hyurax", headerShown: false }} />
          <Stack.Screen name="boas-vindas" options={{ headerShown: false }} />
          <Stack.Screen name="criar" options={{ title: "Nova carteira" }} />
          <Stack.Screen name="importar" options={{ title: "Importar carteira" }} />
          <Stack.Screen name="receber" options={{ title: "Receber" }} />
          <Stack.Screen name="enviar" options={{ title: "Enviar" }} />
          <Stack.Screen name="escanear" options={{ title: "Escanear QR", presentation: "modal" }} />
          <Stack.Screen name="ajustes" options={{ title: "Ajustes" }} />
          <Stack.Screen name="backup" options={{ title: "Cópia de segurança" }} />
        </Stack>
      </ProvedorDaCarteira>
    </SafeAreaProvider>
  );
}
