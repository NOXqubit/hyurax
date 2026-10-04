// Início: o saldo, receber e enviar, e o que aconteceu.

import { Redirect, router } from "expo-router";
import { ActivityIndicator, Pressable, RefreshControl, ScrollView, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import Svg, { Path } from "react-native-svg";

import { useCarteira } from "../estado/carteira";
import { textoDeUnidades } from "../nucleo/valor";
import { Aviso, Botao, Cartao, Marca, SeloTeste, Texto } from "../ui/componentes";
import { medidas, useTema } from "../ui/tema";

function quando(seg: number): string {
  if (!seg) return "esperando bloco";
  const s = Math.max(0, Date.now() / 1000 - seg);
  if (s < 60) return "agora";
  if (s < 3600) return `há ${Math.floor(s / 60)} min`;
  if (s < 86400) return `há ${Math.floor(s / 3600)} h`;
  return new Date(seg * 1000).toLocaleDateString("pt-BR");
}

function Seta({ entrada, cor }: { entrada: boolean; cor: string }) {
  return (
    <Svg width={20} height={20} viewBox="0 0 24 24" fill="none" stroke={cor} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
      <Path d={entrada ? "M12 5v14M6 13l6 6 6-6" : "M12 19V5M6 11l6-6 6 6"} />
    </Svg>
  );
}

export default function Inicio() {
  const t = useTema();
  const c = useCarteira();
  if (c.situacao === "carregando") {
    return (
      <View style={{ flex: 1, alignItems: "center", justifyContent: "center", backgroundColor: t.fundo }}>
        <ActivityIndicator color={t.luz} />
      </View>
    );
  }
  if (c.situacao === "sem-carteira") return <Redirect href="/boas-vindas" />;

  const saldo = c.conta ? textoDeUnidades(c.conta.saldo) : "—";
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: t.fundo }}>
      <ScrollView
        contentContainerStyle={{ padding: 20, gap: 16, paddingBottom: 40 }}
        refreshControl={<RefreshControl refreshing={c.atualizando} onRefresh={c.atualizar} tintColor={t.luz} />}
      >
        <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between" }}>
          <View style={{ flexDirection: "row", alignItems: "center", gap: 12 }}>
            <Marca tamanho={40} />
            <View>
              <Text style={{ color: t.tinta, fontSize: 20, fontWeight: "700" }}>Hyurax</Text>
              <Text style={{ color: t.tinta3, fontSize: 13 }}>
                {c.info ? `${c.info.pares} conexões do nó · bloco ${c.info.altura.toLocaleString("pt-BR")}` : c.noEndereco ? "conectando…" : "sem nó"}
              </Text>
            </View>
          </View>
          <Pressable accessibilityLabel="Ajustes" onPress={() => router.push("/ajustes")} hitSlop={12} style={{ padding: 8 }}>
            <Svg width={24} height={24} viewBox="0 0 24 24" fill="none" stroke={t.tinta} strokeWidth={1.8} strokeLinecap="round">
              <Path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
              <Path d="M16 5a2 2 0 1 1 0 4 2 2 0 0 1 0-4zM10 15a2 2 0 1 1 0 4 2 2 0 0 1 0-4z" />
            </Svg>
          </Pressable>
        </View>

        <View style={{ backgroundColor: "#121626", borderRadius: medidas.raioGrande, padding: 24, gap: 14 }}>
          <SeloTeste rede={c.rede} sobreEscuro />
          <Text style={{ color: "rgba(245,245,247,0.7)", fontSize: 14 }}>Saldo</Text>
          <Text adjustsFontSizeToFit numberOfLines={1} style={{ color: "#F5F5F7", fontSize: 46, fontWeight: "700", letterSpacing: -1 }}>
            {saldo} <Text style={{ color: "#F2B544", fontSize: 20 }}>HYX</Text>
          </Text>
          {c.conta && c.conta.imaturo > 0n ? (
            <Text style={{ color: "rgba(245,245,247,0.6)", fontSize: 13 }}>+ {textoDeUnidades(c.conta.imaturo)} HYX liberando (recompensa recente)</Text>
          ) : null}
          <View style={{ flexDirection: "row", gap: 10, marginTop: 6 }}>
            <View style={{ flex: 1 }}>
              <Botao titulo="Receber" tipo="leve" aoTocar={() => router.push("/receber")} />
            </View>
            <View style={{ flex: 1 }}>
              <Botao titulo="Enviar" tipo="marca" aoTocar={() => router.push("/enviar")} desligado={!c.info} />
            </View>
          </View>
        </View>

        {!c.noEndereco ? (
          <Cartao>
            <Text style={{ color: t.tinta, fontSize: 17, fontWeight: "600" }}>Conecte a um nó</Text>
            <Texto fraco>
              O celular não guarda a cadeia: ele pergunta o saldo a um nó Hyurax e entrega a ele as transações já assinadas aqui. Use o programa do seu PC (Ajustes → Carteiras de celular).
            </Texto>
            <Botao titulo="Escolher o nó" tipo="leve" aoTocar={() => router.push("/ajustes")} />
          </Cartao>
        ) : null}
        {c.erro ? <Aviso tipo="falha">{c.erro}</Aviso> : null}

        <Cartao>
          <Text style={{ color: t.tinta, fontSize: 17, fontWeight: "600" }}>Movimentos</Text>
          {c.movimentos.length === 0 ? (
            <Texto fraco>Nada ainda. Quando você receber ou enviar HYX, aparece aqui.</Texto>
          ) : (
            c.movimentos.map((m) => (
              <View key={m.txid + m.altura} style={{ flexDirection: "row", alignItems: "center", gap: 12, paddingVertical: 6 }}>
                <View style={{ width: 36, height: 36, borderRadius: 18, alignItems: "center", justifyContent: "center", backgroundColor: m.entrada ? t.okFundo : t.painel3 }}>
                  <Seta entrada={m.entrada} cor={m.entrada ? t.ok : t.tinta2} />
                </View>
                <View style={{ flex: 1, minWidth: 0 }}>
                  <Text style={{ color: t.tinta, fontSize: 15, fontWeight: "500" }}>
                    {m.tipo === "recompensa" ? "Recompensa de bloco" : m.entrada ? "Recebido" : "Enviado"}
                    {m.pendente ? "  ·  esperando" : ""}
                  </Text>
                  <Text numberOfLines={1} ellipsizeMode="middle" style={{ color: t.tinta3, fontSize: 12.5 }}>
                    {m.outro || "mineração"} · {quando(m.quando)}
                  </Text>
                </View>
                <Text style={{ color: m.entrada ? t.ok : t.tinta, fontSize: 15, fontWeight: "600", fontVariant: ["tabular-nums"] }}>
                  {m.entrada ? "+" : "−"}
                  {textoDeUnidades(m.valor)}
                </Text>
              </View>
            ))
          )}
        </Cartao>
      </ScrollView>
    </SafeAreaView>
  );
}
