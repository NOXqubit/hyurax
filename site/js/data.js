// Estado real de cada item do caminho. Os textos ficam nos arquivos de idioma (cam.itens.*).
// implementado = existe em código e passa nos testes; desenvolvimento = em construção;
// pesquisa = hipótese em estudo; planejado = arquitetura escrita, sem código.
export const ESTADOS = ["implementado", "desenvolvimento", "pesquisa", "planejado"];

export const RAMOS = [
  { id: "blockchain", itens: [["codec", "implementado"], ["cripto", "implementado"], ["dinheiro", "implementado"], ["tx", "implementado"], ["cadeia", "implementado"], ["redep2p", "desenvolvimento"], ["testnet", "desenvolvimento"], ["auditoria", "planejado"], ["mainnet", "planejado"]] },
  { id: "compute", itens: [["freivalds", "implementado"], ["mercado", "desenvolvimento"], ["tarefas_ia", "planejado"], ["usefulpow", "implementado"]] },
  { id: "communication", itens: [["packet", "planejado"], ["resonance", "planejado"], ["mesh", "planejado"], ["radio", "pesquisa"]] },
  { id: "storage", itens: [["frag", "pesquisa"], ["arquivos", "planejado"], ["erasure", "pesquisa"]] },
  { id: "ai", itens: [["agentes", "planejado"], ["politica", "planejado"], ["m2m", "planejado"]] },
  { id: "economy", itens: [["direct", "planejado"], ["offline", "planejado"], ["recursos", "planejado"], ["flux", "planejado"]] },
];

export const CORES = { implementado: "#eef0f2", desenvolvimento: "#efcb92", pesquisa: "#93b6cc", planejado: "#6b6e76" };

export const BITCOIN = "bc1qkp7d90t9tnmuv2rwq742pwc8pnet28a59zdzt7";
