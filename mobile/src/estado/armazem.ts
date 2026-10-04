// Onde o app guarda o que é dele. No aparelho: SecureStore (no Android,
// cifrado pelo Keystore). A versão `.web.ts` existe só para ver as telas no
// navegador durante o desenvolvimento.

import * as SecureStore from "expo-secure-store";

export const ler = (chave: string): Promise<string | null> => SecureStore.getItemAsync(chave);
export const gravar = (chave: string, valor: string): Promise<void> => SecureStore.setItemAsync(chave, valor);
export const apagar = (chave: string): Promise<void> => SecureStore.deleteItemAsync(chave);
