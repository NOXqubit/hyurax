// Tema do app: os mesmos tokens do programa do PC (Cloud Design 2.1,
// gerados em src/tema/tokens.ts), claro ou escuro conforme o aparelho.

import { useColorScheme } from "react-native";

import { claro, escuro, medidas, type Tema } from "../tema/tokens";

export { medidas };
export type { Tema };

export function useTema(): Tema & { escuro: boolean } {
  const modo = useColorScheme();
  return modo === "dark" ? { ...escuro, escuro: true } : { ...claro, escuro: false };
}
