// ✝ Eclesiastes 4:9 — “Melhor é serem dois do que um, porque têm melhor paga do seu trabalho.”
//! Uma conexão com outro nó: lê e escreve quadros num `TcpStream`.
//!
//! O enquadramento é o da seção 21 (`auron-wire`). Aqui só entra a parte de
//! rede: juntar bytes do socket até fechar um quadro, e mandar bytes. As regras
//! de o que é um quadro válido ficam no `auron-wire`; o transporte não decide
//! consenso.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

use auron_wire::{Message, WireError, decode_frame, encode_frame};

/// Teto do buffer de recepção: um pouco acima do maior quadro possível
/// (`MAX_FRAME_BODY` + cabeçalho). Passar disso é um par empurrando lixo, e a
/// conexão cai antes de a memória crescer sem limite.
const MAX_BUFFER: usize = (auron_wire::MAX_FRAME_BODY as usize) + 64;

/// Erro de conexão.
#[derive(Debug)]
pub enum NetError {
    /// Falha de socket (fechou, timeout, recusado).
    Io(std::io::Error),
    /// Quadro inválido: o par falou errado. Motivo para desconectar.
    Wire(WireError),
    /// O par encheu o buffer sem fechar um quadro.
    BufferCheio,
    /// O aperto de mão falhou (magic, versão ou eco errados).
    Handshake(String),
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "erro de rede: {e}"),
            Self::Wire(e) => write!(f, "quadro inválido: {e}"),
            Self::BufferCheio => f.write_str("par encheu o buffer sem fechar um quadro"),
            Self::Handshake(m) => write!(f, "aperto de mão falhou: {m}"),
        }
    }
}

impl std::error::Error for NetError {}

impl From<std::io::Error> for NetError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<WireError> for NetError {
    fn from(e: WireError) -> Self {
        Self::Wire(e)
    }
}

/// Conexão com um par, sobre um `TcpStream`.
pub struct Conexao {
    stream: TcpStream,
    magic: [u8; 4],
    buffer: Vec<u8>,
}

impl Conexao {
    /// Envolve um socket já aberto.
    pub fn nova(stream: TcpStream, magic: [u8; 4]) -> Self {
        Self { stream, magic, buffer: Vec::new() }
    }

    /// Endereço do outro lado, para registro e log.
    pub fn par(&self) -> std::io::Result<std::net::SocketAddr> {
        self.stream.peer_addr()
    }

    /// Clona o socket para uma thread de escrita separada.
    pub fn clonar_stream(&self) -> std::io::Result<TcpStream> {
        self.stream.try_clone()
    }

    /// Tempo máximo esperando bytes numa leitura. `None` bloqueia sem limite.
    pub fn definir_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.stream.set_read_timeout(t)
    }

    /// Manda uma mensagem, enquadrada.
    pub fn enviar(&mut self, msg: &Message) -> Result<(), NetError> {
        let quadro = encode_frame(&self.magic, msg)?;
        self.stream.write_all(&quadro)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Bloqueia até receber uma mensagem inteira, ou erro.
    ///
    /// Junta o que o socket entrega até `decode_frame` fechar um quadro. Um
    /// `WouldBlock`/`TimedOut` no meio de um quadro é repassado como erro de
    /// I/O: quem chama decide se tenta de novo.
    pub fn receber(&mut self) -> Result<Message, NetError> {
        loop {
            // Tenta fechar um quadro com o que já está no buffer.
            match decode_frame(&self.magic, &self.buffer) {
                Ok(lido) => {
                    let consumido = self.buffer.len().saturating_sub(lido.resto.len());
                    let msg = lido.message;
                    self.buffer.drain(..consumido);
                    return Ok(msg);
                }
                Err(WireError::QuadroIncompleto) => {}
                Err(e) => return Err(NetError::Wire(e)),
            }
            if self.buffer.len() > MAX_BUFFER {
                return Err(NetError::BufferCheio);
            }
            let mut pedaco = [0u8; 8192];
            let lido = self.stream.read(&mut pedaco)?;
            if lido == 0 {
                return Err(NetError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)));
            }
            self.buffer.extend_from_slice(pedaco.get(..lido).unwrap_or(&[]));
        }
    }

    /// Fecha os dois sentidos do socket.
    pub fn fechar(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}
