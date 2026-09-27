// ✝ Provérbios 16:9 — “O coração do homem planeja o seu caminho.”
//! Rotas (caixeiro-viajante): 2-opt a partir de uma partida sorteada pela
//! semente da unidade, numa instância que vem dos parâmetros.
//!
//! **Esqueleto.** A interface é a que `trabalho.rs` chama; o motor ainda não
//! está implementado e toda especificação deste tipo é recusada com
//! [`ErroDeTrabalho::SemMotor`]. O desenho está em
//! `docs/COMPUTACAO-CIENTIFICA.md`, seção 2.3.

use crate::trabalho::{ErroDeTrabalho, Especificacao, Execucao, Recusa};

/// Quantos parâmetros extras a especificação leva.
pub const PARAMETROS: usize = 1;
/// Faixa aceita de `tamanho`.
pub const TAMANHO: (u32, u32) = (4, 2_000);
/// Faixa aceita de `passos`.
pub const PASSOS: (u32, u32) = (1, 10_000);

/// Confere as faixas dos parâmetros.
pub fn validar(_tamanho: u32, _passos: u32, _parametros: &[u32]) -> Result<(), ErroDeTrabalho> {
    Err(ErroDeTrabalho::SemMotor("otimização de rotas"))
}

/// Operações da execução, pelo modelo de custo do motor.
pub fn operacoes(_esp: &Especificacao) -> u64 {
    0
}

/// Operações da verificação.
pub fn operacoes_de_verificacao(esp: &Especificacao) -> u64 {
    operacoes(esp)
}

/// Memória no pico, execução e verificação somadas, em bytes.
pub fn memoria_bytes(_esp: &Especificacao) -> u64 {
    0
}

/// Texto curto para o painel.
pub fn resumo(esp: &Especificacao) -> String {
    format!("otimização de rotas, tamanho {}", esp.tamanho())
}

/// Executa a unidade.
pub fn executar(_esp: &Especificacao, _semente: &[u8], _continuar: &mut dyn FnMut(u64) -> bool) -> Result<Execucao, ErroDeTrabalho> {
    Err(ErroDeTrabalho::SemMotor("otimização de rotas"))
}

/// Confere um resultado. Mesma convenção de [`crate::trabalho::verificar_controlado`].
pub fn verificar(
    _esp: &Especificacao,
    _semente: &[u8],
    _resultado: &[u8],
    _continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    Err(ErroDeTrabalho::SemMotor("otimização de rotas"))
}
