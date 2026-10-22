# Segurança

O Hyurax está em **rede de teste pública**: a semente
`wss://hyurax-semente.onrender.com/p2p` está no ar desde 04/10/2026. Não existe
mainnet e nenhum HYX tem valor. Mesmo assim, falha de segurança é tratada com
seriedade: é agora que ela custa menos.

A **temporada de ataques** tem regras, laboratório, a lista do que já está
coberto e o placar em [docs/TEMPORADA-DE-ATAQUES.md](docs/TEMPORADA-DE-ATAQUES.md).

## O que é bem-vindo

Tentar quebrar o consenso, a rede e a validação faz parte do projeto:

- bloco ou transação inválida que um nó aceite;
- divergência entre o gabarito Python e o nó Rust (um vetor que deveria bater
  e não bate);
- derrubar ou travar um nó com mensagens de rede;
- gasto duplo, reorganização indevida ou emissão acima da regra;
- resultado falso que a conferência do ULTRAX aceite;
- qualquer caso em que a auto-reanimação não reconstrua a cadeia.

Ataque só **nós seus**, ou a rede de teste pública **pelo protocolo**. Não
inunde a semente pública com volume (ela é de todos) e não ataque computadores
de outras pessoas.

## Como relatar

- **Falha grave** (aceitar bloco inválido, emitir moeda, derrubar a rede): use
  o relato privado do GitHub, em *Security → Report a vulnerability*. Se o
  botão não aparecer, abra uma issue com o título "Pedido de canal privado",
  **sem nenhum detalhe da falha**, e a resposta vem com um caminho privado.
  Não publique o passo a passo antes da correção.
- **O resto:** abra uma issue normal.

Inclua o passo a passo para reproduzir, a versão (`hyurax-no versao`) e, se
der, um teste que falha.

## O que o Hyurax ainda não tem

Está escrito para ninguém confiar no que não existe:

- **cifra da conexão:** existe (Noise XX), mas ainda não passou por revisão
  de fora;
- **carteira com senha:** existe (Argon2id + ChaCha20-Poly1305); carteiras
  criadas antes de 13/09/2026 guardam a chave em texto e precisam de
  `hyurax-no carteira cifrar`;
- **auditoria externa:** nenhuma foi feita. A única rodada até aqui foi a
  interna, de 22/10/2026 (fuzzing e a porta WebSocket), descrita no guia da
  temporada.

Não há recompensa em dinheiro nem em HYX por falha encontrada. Quem relatar
recebe crédito público no placar da temporada e nas notas de versão, se quiser.
