// ✝ Provérbios 24:3 — “Com a sabedoria se edifica a casa, e com o entendimento ela se estabelece.”
//! Núcleo do Hyurax / Ultrax 1.0.
//!
//! Tudo o que o programa faz, sem nada de tela: o nó na rede de teste, a
//! carteira, a mineração, o ULTRAX (trabalho útil), os JOBs de computação
//! científica, as métricas da máquina, o barramento de eventos e a API local
//! que a interface consome.
//!
//! Quem usa:
//! - `hyurax-app` (o programa com janela), que liga o núcleo e mostra a
//!   interface de `hyurax-interface`;
//! - `hyurax-no` (o programa de terminal).
//!
//! Mapa dos módulos:
//!
//! | módulo | o que faz |
//! |---|---|
//! | [`pastas`] | onde ficam configuração e dados do usuário |
//! | [`atualizacao`] | atualização segura: manifesto assinado pela chave de lançamento |
//! | [`instalacao`] | a pasta do programa e o manifesto do instalador (Windows) |
//! | [`config`] | rede, portas e sementes |
//! | [`ajustes`] | as escolhas do dono, em `ajustes.txt` |
//! | [`cadeia`] | abrir, subir, sincronizar e gravar a cadeia |
//! | [`carteira`] | arquivo cifrado, endereço, envio, segundo fator |
//! | [`mineracao`] | a mineração dos blocos (Argon2id + prova útil) |
//! | [`ultrax`] | o worker de trabalho útil: fila, linhas, GPU, LAB, prova |
//! | [`ciencia`] | JOBs científicos: agendador, consenso entre nós, relatório |
//! | [`metricas`] | CPU, RAM e GPU da máquina, com a origem de cada número |
//! | [`barramento`] | os eventos, em ordem, para a tela e o registro |
//! | [`api`] | o servidor local (HTTP e fluxo de eventos) |
//! | [`servico`] | o núcleo ligado: junta tudo e cuida do ciclo de vida |

pub mod ajustes;
pub mod api;
pub mod arquivos;
pub mod atualizacao;
pub mod barramento;
pub mod cadeia;
pub mod carteira;
pub mod ciencia;
pub mod config;
pub mod identidade;
pub mod instalacao;
pub mod maquinas;
pub mod metricas;
pub mod mineracao;
pub mod pastas;
pub mod sementes;
pub mod servico;
pub mod termos;
pub mod ultrax;
pub mod util;

/// A versão do programa (a mesma em todos os crates do workspace).
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Nome do produto, para títulos e registros.
pub const PRODUTO: &str = "Hyurax / Ultrax";
