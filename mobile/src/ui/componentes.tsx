// Peças da tela no visual do programa do PC: cartões claros, botões em
// pílula, o cartão escuro do saldo e o âmbar da marca.

import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View, type TextInputProps, type ViewStyle } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import Svg, { Path, Rect } from "react-native-svg";

import { medidas, useTema } from "./tema";

export function Tela({ children, rolar = true }: { children: React.ReactNode; rolar?: boolean }) {
  const t = useTema();
  const conteudo = <View style={{ gap: 16, padding: 20, paddingBottom: 40 }}>{children}</View>;
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: t.fundo }} edges={["bottom", "left", "right"]}>
      {rolar ? <ScrollView keyboardShouldPersistTaps="handled">{conteudo}</ScrollView> : conteudo}
    </SafeAreaView>
  );
}

export function Titulo({ children, sub }: { children: React.ReactNode; sub?: string }) {
  const t = useTema();
  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: t.tinta, fontSize: 30, fontWeight: "700", letterSpacing: -0.6 }}>{children}</Text>
      {sub ? <Text style={{ color: t.tinta3, fontSize: 15, lineHeight: 22 }}>{sub}</Text> : null}
    </View>
  );
}

export function Cartao({ children, style }: { children: React.ReactNode; style?: ViewStyle }) {
  const t = useTema();
  return <View style={[{ backgroundColor: t.painel, borderRadius: medidas.raio, padding: 20, gap: 12 }, style]}>{children}</View>;
}

export function Texto({ children, fraco, pequeno, mono, style }: { children: React.ReactNode; fraco?: boolean; pequeno?: boolean; mono?: boolean; style?: object }) {
  const t = useTema();
  return (
    <Text
      selectable={mono}
      style={[{ color: fraco ? t.tinta3 : t.tinta2, fontSize: pequeno ? 13 : 15, lineHeight: pequeno ? 19 : 22, fontFamily: mono ? "monospace" : undefined }, style]}
    >
      {children}
    </Text>
  );
}

type BotaoProps = { titulo: string; aoTocar: () => void; tipo?: "principal" | "leve" | "perigo" | "marca"; ocupado?: boolean; desligado?: boolean; grande?: boolean };

export function Botao({ titulo, aoTocar, tipo = "principal", ocupado, desligado, grande }: BotaoProps) {
  const t = useTema();
  const fundo = tipo === "principal" ? t.tinta : tipo === "perigo" ? t.falhaFundo : tipo === "marca" ? t.luz : t.painel3;
  const cor = tipo === "principal" ? t.fundo : tipo === "perigo" ? t.falha : tipo === "marca" ? t.sobreLuz : t.tinta;
  const parado = desligado || ocupado;
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ disabled: !!parado, busy: !!ocupado }}
      disabled={parado}
      onPress={aoTocar}
      style={({ pressed }) => ({
        minHeight: grande ? 56 : medidas.alvo + 4,
        paddingHorizontal: 22,
        borderRadius: medidas.raioPilula,
        backgroundColor: fundo,
        alignItems: "center",
        justifyContent: "center",
        opacity: parado ? 0.45 : pressed ? 0.8 : 1,
        flexDirection: "row",
        gap: 10,
      })}
    >
      {ocupado ? <ActivityIndicator color={cor} /> : null}
      <Text style={{ color: cor, fontSize: grande ? 17 : 15, fontWeight: "650" as any }}>{titulo}</Text>
    </Pressable>
  );
}

export function Campo({ rotulo, nota, erro, ...props }: TextInputProps & { rotulo: string; nota?: string; erro?: string | null }) {
  const t = useTema();
  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: t.tinta3, fontSize: 13 }}>{rotulo}</Text>
      <TextInput
        placeholderTextColor={t.tinta3}
        {...props}
        style={[
          { minHeight: 50, borderRadius: medidas.raioP + 4, paddingHorizontal: 16, paddingVertical: 12, backgroundColor: t.painel2, color: t.tinta, fontSize: 16, borderWidth: erro ? 1.5 : StyleSheet.hairlineWidth, borderColor: erro ? t.falha : t.fioForte },
          props.multiline ? { minHeight: 120, textAlignVertical: "top" } : null,
          props.style,
        ]}
      />
      {erro ? <Text style={{ color: t.falha, fontSize: 12.5, lineHeight: 18 }}>{erro}</Text> : nota ? <Text style={{ color: t.tinta3, fontSize: 12.5, lineHeight: 18 }}>{nota}</Text> : null}
    </View>
  );
}

export function Aviso({ children, tipo = "info" }: { children: React.ReactNode; tipo?: "info" | "atencao" | "falha" | "ok" }) {
  const t = useTema();
  const [fundo, cor] = { info: [t.infoFundo, t.info], atencao: [t.atencaoFundo, t.atencao], falha: [t.falhaFundo, t.falha], ok: [t.okFundo, t.ok] }[tipo];
  return (
    <View style={{ backgroundColor: fundo, borderRadius: medidas.raioP + 2, padding: 14 }}>
      <Text style={{ color: cor, fontSize: 14, lineHeight: 20 }}>{children}</Text>
    </View>
  );
}

/** Selo fixo de rede de teste: aparece em toda tela que mostra valor. */
export function SeloTeste({ rede, sobreEscuro }: { rede: string; sobreEscuro?: boolean }) {
  const t = useTema();
  const teste = !rede.endsWith("mainnet");
  if (!teste) return null;
  return (
    <View style={{ alignSelf: "flex-start", backgroundColor: sobreEscuro ? "rgba(242,181,68,0.16)" : t.luzFundo, borderRadius: 999, paddingHorizontal: 10, paddingVertical: 4 }}>
      <Text style={{ color: sobreEscuro ? "#F2B544" : t.luzTexto, fontSize: 12, fontWeight: "700", letterSpacing: 0.4 }}>REDE DE TESTE · SEM VALOR REAL</Text>
    </View>
  );
}

/** A marca: o H com a travessa em subida. */
export function Marca({ tamanho = 56 }: { tamanho?: number }) {
  return (
    <Svg width={tamanho} height={tamanho} viewBox="0 0 64 64">
      <Rect width={64} height={64} rx={15} fill="#121626" />
      <Path d="M18 39.5 46 24.5" stroke="#F2B544" strokeWidth={8} />
      <Rect x={15} y={15} width={9.5} height={35} rx={4.75} fill="#F5F5F7" />
      <Rect x={39.5} y={12} width={9.5} height={35} rx={4.75} fill="#F5F5F7" />
    </Svg>
  );
}

export function Linha({ rotulo, valor, mono }: { rotulo: string; valor: React.ReactNode; mono?: boolean }) {
  const t = useTema();
  if (mono) {
    // endereço e txid: o rótulo em cima e o valor inteiro embaixo, quebrando
    return (
      <View style={{ gap: 4 }}>
        <Text style={{ color: t.tinta3, fontSize: 14 }}>{rotulo}</Text>
        <Text selectable style={{ color: t.tinta, fontSize: 13.5, lineHeight: 20, fontFamily: "monospace" }}>
          {valor}
        </Text>
      </View>
    );
  }
  return (
    <View style={{ flexDirection: "row", justifyContent: "space-between", gap: 16, alignItems: "baseline" }}>
      <Text style={{ color: t.tinta3, fontSize: 14 }}>{rotulo}</Text>
      <Text selectable style={{ color: t.tinta, fontSize: 14.5, flexShrink: 1, textAlign: "right", fontFamily: mono ? "monospace" : undefined }}>
        {valor}
      </Text>
    </View>
  );
}
