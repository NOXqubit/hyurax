# Segurança

O Auron está em **pré-testnet**. Não existe mainnet e nenhum AUR tem valor.
Mesmo assim, falha de segurança é tratada com seriedade: é agora que ela custa
menos.

## O que é bem-vindo

Tentar quebrar o consenso, a rede e a validação faz parte do projeto:

- bloco ou transação inválida que um nó aceite;
- divergência entre o gabarito Python e o nó Rust (um vetor que deveria bater
  e não bate);
- derrubar ou travar um nó com mensagens de rede;
- gasto duplo, reorganização indevida ou emissão acima da regra;
- qualquer caso em que a auto-reanimação não reconstrua a cadeia.

Ataque só **nós seus** ou a testnet pública do projeto, quando ela existir. Não
ataque computadores de outras pessoas.

## Como relatar

- **Falha grave** (aceitar bloco inválido, emitir moeda, derrubar a rede): use
  o relato privado do GitHub, em *Security → Report a vulnerability*. Não abra
  issue pública antes da correção.
- **O resto:** abra uma issue normal.

Inclua o passo a passo para reproduzir e, se der, um teste que falha.

## O que o Auron ainda não tem

Está escrito para ninguém confiar no que não existe:

- **cifra da conexão:** existe (Noise XX), mas ainda não passou por revisão
  de fora;
- **carteira com senha:** existe (Argon2id + ChaCha20-Poly1305); carteiras
  criadas antes de 13/09/2026 guardam a chave em texto e precisam de
  `auron-no carteira cifrar`;
- **auditoria externa:** nenhuma foi feita.

Não há recompensa em dinheiro por falha encontrada. Quem relatar recebe crédito
público no registro de mudanças, se quiser.
