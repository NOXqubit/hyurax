// ✝ Lucas 16:10 — “Quem é fiel no mínimo, também é fiel no muito.”
//! HYURAX Compute (V1): a plataforma central do mercado de computação.
//!
//! Liga quem precisa de computação (o cliente) a quem tem máquina parada
//! (o fornecedor), com o caminho inteiro do documento da V1:
//!
//! ```text
//! cliente → web → API → contas / cobrança / mercado / tarefas
//!         → agendador → registro de nós → agente → motor isolado
//!         → resultado → verificação → cobrança → repasse
//! ```
//!
//! O desenho, seção por seção, está em `docs/V1-PLATAFORMA.md`. As peças:
//!
//! - [`regras`]: preço, liquidação, reputação e assinatura do agente,
//!   especificados em `reference/hyurax/plataforma.py` e conferidos por vetor;
//! - [`dinheiro`]: moeda, contas e o livro em partida dobrada;
//! - [`eventos`], [`diario`] e [`estado`]: tudo o que muda vira evento num
//!   diário com hash encadeado; o estado é o diário aplicado em ordem;
//! - [`auth`]: senha (Argon2id), sessão, chave de API e códigos;
//! - [`servico`]: as operações (contas, nós, agendador, tarefas, dinheiro,
//!   administração, antifraude);
//! - [`http`] e [`rotas`]: o servidor HTTP e a API `/api/v1/`;
//! - [`web`]: os painéis do cliente, do fornecedor e do administrador.
//!
//! **Sem token, sem blockchain, sem mineração no caminho do dinheiro** (seção
//! 20): a V1 cobra em real ou dólar, por saldo pré-pago.

#![forbid(unsafe_code)]

pub mod auth;
pub mod diario;
pub mod dinheiro;
pub mod estado;
pub mod eventos;
pub mod http;
pub mod modelo;
pub mod regras;
pub mod rotas;
pub mod servico;
pub mod util;
pub mod web;

pub use servico::{Erro, Opcoes, Plataforma};

/// Versão do programa.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");
