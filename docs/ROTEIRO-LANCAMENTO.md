# Roteiro de lançamento: 12 semanas

Escrito em 13/09/2026. Prazo: **13/12/2026**.

## Decisões tomadas

O autor delegou as escolhas técnicas ao arquiteto em 13/09/2026, com prazo de
três meses. O que ficou decidido:

| Assunto | Decisão | Motivo |
|---|---|---|
| Ordem | GitHub → testnet pública → mainnet | Erro de mainnet não tem volta; a testnet é onde se descobre a falha barata |
| Licença | MIT ou Apache-2.0, à escolha de quem usa (já estava no repositório) | Padrão do ecossistema Rust; empresas e pessoas podem usar e contribuir |
| Cifra da conexão | Noise `XX` | Autentica os dois lados sem conhecer o par antes; combina com descoberta aberta |
| Identidade de nó | Persistente, num arquivo próprio, separado da carteira | Permite banir um nó malicioso de verdade; o custo de privacidade é declarado na spec |
| Doação | Endereço Bitcoin já conferido, com o aviso "não dá direito a nada" | Doação não é investimento |
| Indicação | Placar de contribuição **sem recompensa em dinheiro ou token** (testadores trazidos, falhas achadas, nós no ar) | "Indique e ganhe" tem cara de pirâmide e de promessa de retorno |

## O que conta como "mainnet" neste prazo

A mainnet **só sai se os critérios da semana 11 forem cumpridos**. Se não
forem, o que se entrega em 13/12 é a testnet pública estável, com a data da
mainnet anunciada depois. Mudar a data é aceitável; lançar com falha conhecida
não é.

## Andamento

| Data | O que ficou pronto |
|---|---|
| 13/09/2026 | Semana 1: repositório público, testes no GitHub, `SECURITY.md` e doação |
| 13/09/2026 | Semanas 2 e 3: cifra Noise XX, identidade de nó e protocolo versão 2 |
| 13/09/2026 | Semana 4: carteira com senha e comando `enviar` |
| 13/09/2026 | Semana 5: guia `docs/RODAR-UM-NO.md` e programas prontos pelo GitHub |

| 14/09/2026 | Redesign do site (abertura com o A de metal, capítulos 01, 03, 04 e 08, atos) |
| 14/09/2026 | Semana 6, parte que não depende de servidor: sementes embutidas, `--exportar`, kit `deploy/` e guia `docs/NO-SEMENTE.md` |

Próximo: **subir o primeiro nó semente**. Precisa de uma máquina ligada 24 h, e a conta no provedor é do autor (guia em `docs/NO-SEMENTE.md`).

## Semanas

| Semana | Entrega | Pronto quando |
|---|---|---|
| 1 | Repositório público no GitHub; testes automáticos; `SECURITY.md`; doação no README | Os testes passam no GitHub, não só na máquina local |
| 2–3 | Cifra Noise XX na conexão; identidade de nó; protocolo sobe de versão | Os cenários de rede (ataques, reorganização e auto-reanimação) passam cifrados; um intermediário não lê nem altera o tráfego sem ser detectado |
| 4 | Carteira com senha (Argon2id para derivar a chave, cifra autenticada) | Arquivo roubado sem a senha não gasta nada |
| 5 | Guia "rode um nó em 5 minutos"; binários prontos para Linux, Windows e Android (Termux) | Uma pessoa de fora sobe um nó seguindo só o guia |
| 6 | **Testnet pública no ar**, com 2 ou 3 nós semente sempre ligados e um explorador de blocos simples | Nós de pessoas diferentes sincronizam pela internet |
| 7–9 | **Temporada de ataques** aberta; correções; placar de contribuição | Toda falha grave relatada tem teste que a trava |
| 10 | Parâmetros de consenso congelados; spec versionada; gênese da mainnet definida | Nenhuma mudança de consenso pendente |
| 11 | Revisão final e critérios de lançamento (abaixo) | Todos os critérios cumpridos, por escrito |
| 12 | Mainnet, ou testnet estável com a data da mainnet anunciada | — |

## Critérios para lançar a mainnet

- [ ] testnet pública rodando pelo menos 4 semanas sem falha grave em aberto;
- [ ] nenhuma falha de consenso relatada sem correção e teste;
- [ ] cifra da conexão e carteira com senha em uso na testnet;
- [ ] pelo menos 5 nós de pessoas diferentes rodaram a testnet;
- [ ] Python e Rust concordando em todos os vetores;
- [ ] spec congelada, com o hash do documento publicado;
- [ ] aviso público de que AUR não é investimento e não tem valor garantido.

## Divulgação (TikTok, Instagram, Facebook e X)

Mostrar o que existe de verdade:

1. "Meu celular minerando blocos": tela do Termux e o número real do `medir`.
2. "Tentei hackear minha própria blockchain": os ataques que o nó repeliu.
3. "Derrubei a rede e ela voltou sozinha": a auto-reanimação com dois terminais.
4. "Python e Rust acharam o mesmo nonce": a validação byte a byte.
5. Diário do dev brasileiro construindo uma blockchain num Atom de 2012.

Regras que valem em todo post:

- música sem direitos de terceiros;
- voz sintética declarada;
- sem rosto inventado;
- nunca "invista", "vai valorizar" ou "ganhe dinheiro". O convite é
  **rode, teste, ataque, contribua**.

## Riscos

| Risco | O que fazer |
|---|---|
| Ninguém de fora rodar nós | Nós semente próprios e o guia de 5 minutos; divulgar o desafio de ataque |
| Máquina de desenvolvimento lenta (Atom) | Os testes pesados rodam no GitHub; o PC Athlon vira segundo nó quando voltar a dar vídeo |
| Falha grave na semana 10 ou 11 | Adiar a mainnet; não é fracasso, é o processo funcionando |
| O endereço de doação é de uma corretora | Funciona, mas a corretora pode trocar ou bloquear. Antes da mainnet, migrar para uma carteira em que só o autor tenha a chave |
