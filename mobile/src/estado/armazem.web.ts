// SÓ PARA DESENVOLVIMENTO: ver as telas no navegador. Guarda em
// sessionStorage, que some ao fechar a aba. O app de verdade nunca usa isto.

export const ler = async (chave: string): Promise<string | null> => sessionStorage.getItem(chave);
export const gravar = async (chave: string, valor: string): Promise<void> => sessionStorage.setItem(chave, valor);
export const apagar = async (chave: string): Promise<void> => sessionStorage.removeItem(chave);
