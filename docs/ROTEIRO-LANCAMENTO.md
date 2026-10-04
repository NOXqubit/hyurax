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

| 20/09/2026 | Mempool que só aceita o que dá para minerar em sequência; saldo não derruba mais par honesto; versão alinhada com a etiqueta `v0.2.0-teste.1` |
| 26/09/2026 | Semana 6, parte que não depende de servidor pago: lista de sementes publicada no repositório (`rede/sementes-testnet.txt`), que o nó busca sozinho; script para o PC do autor servir de semente; ensaio com três processos (bloco e transferência atravessando a rede); passo a passo em `docs/LANCAR-A-REDE.md` |
| 26/09/2026 | Ticker HYUR vira HYX (só exibição); documento do HYX Index registrado em `docs/HYX-INDEX.md` |
| 26/09/2026 | ULTRAX, etapas 0 e 1: arquitetura e plano de migração em `docs/ULTRAX.md`; crate `hyurax-ultrax` com matriz, mochila e difusão iguais ao gabarito byte a byte, ciclo de vida, registro de prova assinado, validador, reputação e Work Score |
| 26/09/2026 | ULTRAX, etapa 2: worker LAB no programa (`hyurax-no ultrax lab` e `/api/ultrax`), com limite de CPU e memória, prazo, cancelamento, tarefas-desafio contra o gabarito, histórico assinado, telemetria e `hyurax-no ultrax auditar`; revisado por cinco lentes, com os achados corrigidos |
| 26/09/2026 | ULTRAX, etapa 3: a tela do ULTRAX no lugar da estação 3D (trabalho ativo com o progresso real, ciclo de vida, verificação, Work Score, reputação, histórico, telemetria e o selo LAB fixo); mineração com painel próprio |
| 26/09/2026 | ULTRAX, etapa 4: treino de rede neural sobre 2.048 moléculas reais (AqSolDB, CC0), especificado em Python e igual no Rust por vetor; painel com a molécula desenhada do SMILES, solubilidade medida × prevista e a curva do treino |
| 26/09/2026 | A GPU trabalha no ULTRAX, inclusive a integrada, pelo WebGL2 da janela e com limitador próprio; a CPU confere cada resultado (Freivalds) antes de contar |
| 26/09/2026 | Endereço com dígito verificador (Bech32m, `thyx1…`); painel sem a espera de 300 ms por resposta |
| 26/09/2026 | Redesign do programa (navegação por seção, visão geral, fontes da marca); instalador com os termos de uso, sem administrador e com desinstalação pelo Windows; termos também na primeira abertura |
| 26/09/2026 | Redesign do site (capítulos ULTRAX e IA com moléculas reais); fontes servidas pelo próprio site, sem nenhum terceiro; política de privacidade e cookies (LGPD), termos de uso, `security.txt` e CSP em todas as páginas |
| 26/09/2026 | Versão `v0.3.0-teste.1` preparada: instalador na Release, notas em `docs/historico/NOTAS-v0.3.0-teste.1.md` |
| 27/09/2026 | Computação científica: JOBs com unidades conferidas, motores de genética, plantas, rotas e triagem de 8.289 moléculas, visão 3D por eventos reais, ULTRA BENCHMARK e cálculo entre nós com compromisso, maioria e conferência local (ensaio com 3 processos: 30/30 em consenso 3/3) |
| 27/09/2026 | Termos de uso versão 2 (computação científica); versão `v0.4.0-teste.1` preparada, notas em `docs/historico/NOTAS-v0.4.0-teste.1.md` |
| 02/10/2026 | **Hyurax / Ultrax 1.0** (programa oficial, rede de TESTE): núcleo separado da tela, API v1 com fluxo de eventos, painel novo, métricas reais com a origem de cada número, cena 3D feita das amostras reais dos motores, GPU calculando unidades de JOB, instalador com manifesto e teste de instalação limpa; termos versão 3. Notas em `docs/NOTAS-1.0.md` |
| 04/10/2026 | Preparação da mainnet medida em [MAINNET.md](MAINNET.md) (o que trava é a semente pública); aplicativo do celular (carteira) com a chave só no aparelho e a API das carteiras leves no nó ([APP-CELULAR.md](APP-CELULAR.md)) |

Próximo: **o primeiro nó respondendo de fora**. O PC do autor serve para começar (`scripts/no-sempre-ligado.ps1` + porta 8790 no roteador + teste pelo 4G do celular). Os passos 1, 2 e 3 de `docs/LANCAR-A-REDE.md` são do autor: renomear e publicar o repositório, criar a etiqueta, e deixar o nó ligado.

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
- [ ] aviso público de que HYX não é investimento e não tem valor garantido.

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
