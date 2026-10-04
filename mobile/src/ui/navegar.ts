import { router } from "expo-router";

/** Volta ao início limpando a pilha (depois de criar ou importar a carteira). */
export function irParaInicio(): void {
  if (router.canDismiss()) router.dismissAll();
  router.replace("/");
}
