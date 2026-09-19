//! O Éter pelo Wi-Fi: datagramas UDP em difusão na rede local.
//!
//! Não precisa de internet, de servidor nem de saber o endereço do outro lado:
//! quem estiver na mesma rede (o roteador de casa, ou o roteador do celular
//! ligado sem chip nenhum) ouve. Cada quadro do Éter vai num datagrama só, e o
//! UDP já traz a sua soma de conferência; o resto (ordem, repetição, perda) é
//! trabalho do próprio Éter.
//!
//! O que isto não é: Wi-Fi sem roteador nenhum, de placa para placa, exige o
//! Wi-Fi Direct do sistema, que o Windows e o Termux não expõem a um programa
//! comum. O roteador do celular resolve o mesmo problema sem custo.

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::thread;
use std::time::Duration;

use crate::{EterError, Meio};

/// Porta padrão do Éter no Wi-Fi.
pub const PORTA_PADRAO: u16 = 8791;

/// Quadro seguro por datagrama: cabe num pacote Ethernet sem fragmentar o IP.
pub const MTU_PADRAO: usize = 1400;

/// Pausa entre datagramas: rajada sem pausa estoura o buffer do celular.
const PAUSA: Duration = Duration::from_micros(1500);

fn falha(e: impl core::fmt::Display) -> EterError {
    EterError::Meio(e.to_string())
}

/// Wi-Fi por difusão UDP.
pub struct MeioWifi {
    nome: String,
    socket: UdpSocket,
    destino: SocketAddr,
    mtu: usize,
    pausa: Duration,
}

impl MeioWifi {
    /// Escuta na `porta` e envia em difusão para a rede local inteira.
    pub fn difusao(porta: u16) -> Result<Self, EterError> {
        let destino = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, porta));
        Self::abrir("wi-fi", porta, destino)
    }

    /// Escuta na `porta` e envia só para `destino` (útil quando a rede bloqueia
    /// difusão, e nos testes).
    pub fn abrir(nome: &str, porta: u16, destino: SocketAddr) -> Result<Self, EterError> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, porta)).map_err(falha)?;
        socket.set_broadcast(true).map_err(falha)?;
        socket.set_nonblocking(true).map_err(falha)?;
        Ok(Self {
            nome: nome.to_owned(),
            socket,
            destino,
            mtu: MTU_PADRAO,
            pausa: PAUSA,
        })
    }

    /// A porta em que este lado escuta (útil quando se pediu a porta 0).
    pub fn porta_local(&self) -> Result<u16, EterError> {
        self.socket.local_addr().map(|a| a.port()).map_err(falha)
    }
}

impl Meio for MeioWifi {
    fn nome(&self) -> &str {
        &self.nome
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn vazao(&self) -> u64 {
        // Limitado pela pausa entre datagramas, não pelo rádio.
        let por_segundo = 1_000_000u64.checked_div(self.pausa.as_micros().max(1) as u64).unwrap_or(1);
        por_segundo.saturating_mul(self.mtu as u64)
    }

    fn enviar(&mut self, quadro: &[u8]) -> Result<(), EterError> {
        if quadro.len() > self.mtu {
            return Err(EterError::MeioPequenoDemais {
                mtu: self.mtu,
                preciso: quadro.len(),
            });
        }
        loop {
            match self.socket.send_to(quadro, self.destino) {
                Ok(_) => break,
                // Buffer de envio cheio: espera um pouco e tenta de novo.
                Err(e) if e.kind() == ErrorKind::WouldBlock => thread::sleep(self.pausa),
                Err(e) => return Err(falha(e)),
            }
        }
        thread::sleep(self.pausa);
        Ok(())
    }

    fn receber(&mut self) -> Result<Vec<Vec<u8>>, EterError> {
        let mut quadros = Vec::new();
        let mut buf = vec![0u8; 65_536];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((n, _)) => quadros.push(buf.get(..n).unwrap_or_default().to_vec()),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                // No Windows, um "porta inalcançável" de envio antigo aparece aqui.
                Err(e) if e.kind() == ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(falha(e)),
            }
        }
        Ok(quadros)
    }
}
