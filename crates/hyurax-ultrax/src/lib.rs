// ✝ Gálatas 6:4 — “Mas prove cada um a sua própria obra.”
//! Hyurax — ULTRAX, o motor de trabalho útil. **Fora do consenso.**
//!
//! Biblioteca pura: não abre socket nem arquivo. Quem executa, guarda e mostra
//! é o `hyurax-no`; quem leva tarefa de um nó a outro será o `hyurax-net`. O
//! desenho completo está em `docs/ULTRAX.md`.
//!
//! As peças, separadas de propósito:
//!
//! - [`ia`]: o treino de rede neural sobre moléculas reais (AqSolDB);
//! - [`trabalho`]: os tipos de trabalho. Cada um sabe executar, verificar e
//!   contar as operações que fez. Matriz, mochila e difusão são a tradução de
//!   `reference/hyurax/utrax.py`, conferida contra `vectors/utrax.json`;
//! - [`tarefa`]: a tarefa e o ciclo de vida, só com transições permitidas;
//! - [`prova`]: o registro de prova assinado pelo worker;
//! - [`validador`]: compara resultados de workers diferentes e dá parecer;
//! - [`reputacao`]: o histórico de cada worker;
//! - [`pontuacao`]: o Work Score, uma medida de contribuição;
//! - os motores científicos, fora do rodízio LAB e só por JOB: [`genetica`],
//!   [`melhoramento`], [`rotas`] e [`triagem`] (`docs/COMPUTACAO-CIENTIFICA.md`).
//!
//! **Work Score não é dinheiro.** Nada aqui converte operação em HYX. Uma
//! recompensa, se um dia existir, depende de regra econômica publicada, fora
//! deste crate.

#![forbid(unsafe_code)]

/// Monta um rótulo de domínio do ULTRAX na compilação: `HYURAX-UTRAX-<sufixo>`.
///
/// O prefixo `UTRAX` é o dos rótulos que já existem no gabarito; ULTRAX é o
/// nome do produto, e trocar o rótulo mudaria os vetores.
macro_rules! dominio {
    ($sufixo:literal) => {
        concat!(hyurax_identidade::raiz!(), "-UTRAX-", $sufixo).as_bytes()
    };
}

pub mod genetica;
pub mod ia;
pub mod melhoramento;
pub mod pontuacao;
pub mod prova;
pub mod reputacao;
pub mod rotas;
pub mod tarefa;
pub mod trabalho;
pub mod triagem;
pub mod validador;
