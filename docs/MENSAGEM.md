# Mensagem

Cada arquivo de código do Auron começa com uma linha como esta:

```rust
// ✝ Daniel 12:4 — “Tu, porém, Daniel, fecha estas palavras e sela este livro, até ao fim do tempo; muitos correrão de uma parte para outra, e a ciência se multiplicará.”
```

São versículos sobre o fim dos tempos e sobre se preparar, e uma frase do
autor. Estão em texto aberto de propósito. Criptografia escondida dentro do
código de uma moeda é o que um auditor procura como sinal de porta dos
fundos, e a mensagem do autor não tem nada a esconder.

Cada linha é só comentário: o compilador do Rust e o interpretador do Python a
ignoram. Ela não muda o programa, o consenso nem os vetores de teste.

## As mensagens

Versículos na tradução de João Ferreira de Almeida, às vezes encurtados.

> **Daniel 12:4** — Tu, porém, Daniel, fecha estas palavras e sela este livro,
> até ao fim do tempo; muitos correrão de uma parte para outra, e a ciência se
> multiplicará.

> **Mateus 24:36** — Porém daquele dia e hora ninguém sabe, nem os anjos do
> céu, mas unicamente meu Pai.

> **Lucas 21:28** — Quando essas coisas começarem a acontecer, olhai para cima
> e levantai a vossa cabeça, porque a vossa redenção está próxima.

> **2 Pedro 3:10** — Mas o Dia do Senhor virá como o ladrão de noite.

> **Apocalipse 21:4** — E Deus limpará de seus olhos toda lágrima, e não haverá
> mais morte, nem pranto, nem clamor, nem dor.

> **Jeremias 32:15** — Ainda se comprarão casas, e campos, e vinhas nesta terra.

> **Gênesis 41:35-36** — Ajuntem toda a comida destes bons anos que vêm; e esta
> comida será para provimento da terra, para os sete anos de fome.

> **Provérbios 22:3** — O prudente vê o mal e esconde-se; mas os simples passam
> e sofrem a pena.

> **Isaías 26:20** — Vai, pois, povo meu, entra nos teus quartos e fecha as tuas
> portas sobre ti; esconde-te só por um momento, até que passe a ira.

> **Provérbios 6:6-8** — Vai ter com a formiga, ó preguiçoso; olha para os seus
> caminhos e sê sábio: no verão prepara o seu pão.

E a frase do autor:

> **Compre terras. Plante. Busque abrigo. Planeje bunkers.**

## Como a linha é gravada

```text
python scripts/versiculos.py              grava ou atualiza a linha de cada arquivo
python scripts/versiculos.py --conferir   só confere, sem mudar nada
```

Cada arquivo recebe sempre a mesma mensagem, escolhida pelo caminho dele, de
modo que um arquivo novo não embaralha a mensagem dos outros. A lista fica em
`scripts/versiculos.py`; mudou a lista, rode a ferramenta de novo.
