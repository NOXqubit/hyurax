//! Como o nó sobe: a rede, as pastas, a porta e as sementes.

use hyurax_consensus::ParametrosRede;

use crate::pastas::Pastas;
use crate::sementes;

/// A rede de teste pública. É a única rede pública que existe.
pub const REDE_PADRAO: ParametrosRede = ParametrosRede::TESTNET;

/// Porta padrão do nó (P2P) e do painel local.
pub const PORTA_P2P: u16 = 8790;
/// Porta padrão do painel local (API e interface).
pub const PORTA_PAINEL: u16 = 8800;

/// Nós semente da rede de teste pública, embutidos no programa. Vazio enquanto
/// nenhum semente estiver no ar: a lista só ganha um endereço depois de ele
/// responder de verdade (ver `docs/rede/NO-SEMENTE.md`).
pub const SEMENTES_TESTNET: &[&str] = &[];

/// As redes que o programa aceita: a de teste pública e a local, para testes.
/// Não existe rede principal (mainnet).
///
/// # Errors
/// Nome desconhecido, ou `mainnet`.
pub fn rede_do_nome(nome: &str) -> Result<ParametrosRede, String> {
    match nome {
        "testnet" => Ok(ParametrosRede::TESTNET),
        "regtest" => Ok(ParametrosRede::REGTEST),
        "mainnet" => Err("a rede principal (mainnet) não existe: o Hyurax 1.0 roda na rede de teste (testnet)".into()),
        outro => Err(format!("rede desconhecida: {outro} (use testnet ou regtest)")),
    }
}

/// O que o nó precisa para subir.
#[derive(Clone, Debug)]
pub struct ConfigDoNo {
    /// Parâmetros de consenso da rede.
    pub rede: ParametrosRede,
    /// Onde guardar configuração e dados.
    pub pastas: Pastas,
    /// Porta para os outros nós se conectarem; 0 = não escutar.
    pub porta: u16,
    /// Sementes (`host:porta`) para conectar.
    pub sementes: Vec<String>,
    /// Não usar a lista embutida nem a publicada.
    pub sem_sementes_padrao: bool,
    /// Malha (docs/HYURAX-MALHA.md): anunciar na rede local e abrir a porta
    /// no roteador por UPnP/NAT-PMP. Só vale com porta de escuta.
    pub malha: bool,
}

impl ConfigDoNo {
    /// Configuração com as sementes completadas: primeiro o arquivo do dono da
    /// máquina (`sementes.txt`), depois a lista embutida da testnet. A lista
    /// publicada na internet é consultada depois de o nó subir.
    pub fn nova(rede: ParametrosRede, pastas: Pastas, porta: u16, sementes: Vec<String>, sem_sementes_padrao: bool) -> Self {
        let mut c = Self { rede, pastas, porta, sementes, sem_sementes_padrao, malha: true };
        if c.sementes.is_empty() && !c.sem_sementes_padrao {
            c.sementes = sementes::do_arquivo(&c.pastas.config);
            if c.sementes.is_empty() && c.rede.nome == ParametrosRede::TESTNET.nome {
                c.sementes = SEMENTES_TESTNET.iter().map(|s| (*s).to_string()).collect();
            }
        }
        c
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn so_testnet_e_regtest() {
        assert_eq!(rede_do_nome("testnet").map(|r| r.nome), Ok(ParametrosRede::TESTNET.nome));
        assert_eq!(rede_do_nome("regtest").map(|r| r.nome), Ok(ParametrosRede::REGTEST.nome));
        assert!(rede_do_nome("mainnet").is_err());
        assert!(rede_do_nome("qualquer").is_err());
    }
}
