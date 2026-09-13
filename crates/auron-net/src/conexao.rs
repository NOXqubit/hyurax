// ✝ Eclesiastes 4:9 — “Melhor é serem dois do que um, porque têm melhor paga do seu trabalho.”
//! Uma conexão com outro nó: lê e escreve quadros num `TcpStream`.
//!
//! O enquadramento é o da seção 21 (`auron-wire`). Aqui só entra a parte de
//! rede: juntar bytes do socket até fechar um quadro, e mandar bytes. As regras
//! de o que é um quadro válido ficam no `auron-wire`; o transporte não decide
//! consenso.
//!
//! Com cifra (versão 2 do protocolo, ver `cifra.rs`), o que viaja no socket são
//! pedaços `[u16 tamanho][texto cifrado]`. A leitura decifra cada pedaço e junta
//! o texto claro no mesmo buffer de antes; a escrita corta o quadro e cifra.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use auron_wire::{Message, WireError, decode_frame, encode_frame};

use crate::cifra::{self, ETIQUETA, Identidade, MAX_MENSAGEM, MAX_PEDACO, Papel};

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
    /// A cifra falhou: aperto Noise recusado ou pedaço que não autentica.
    Cifra(String),
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "erro de rede: {e}"),
            Self::Wire(e) => write!(f, "quadro inválido: {e}"),
            Self::BufferCheio => f.write_str("par encheu o buffer sem fechar um quadro"),
            Self::Handshake(m) => write!(f, "aperto de mão falhou: {m}"),
            Self::Cifra(m) => write!(f, "cifra: {m}"),
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

/// A metade de escrita da cifra, compartilhada entre quem escreve.
///
/// O nonce e a escrita no socket ficam atrás do mesmo cadeado: o que sai
/// primeiro leva o nonce menor, e o outro lado lê na mesma ordem.
#[derive(Clone)]
struct CifraEscrita {
    sessao: Arc<cifra::Sessao>,
    nonce: Arc<Mutex<u64>>,
}

impl CifraEscrita {
    fn escrever(&self, stream: &mut TcpStream, dados: &[u8]) -> Result<(), NetError> {
        let mut nonce = self.nonce.lock().map_err(|_| NetError::Cifra("cadeado da escrita envenenado".into()))?;
        let mut saida = vec![0u8; MAX_MENSAGEM];
        for pedaco in dados.chunks(MAX_PEDACO) {
            let n = self
                .sessao
                .transporte
                .write_message(*nonce, pedaco, &mut saida)
                .map_err(|e| NetError::Cifra(format!("cifrar: {e:?}")))?;
            *nonce = nonce.checked_add(1).ok_or_else(|| NetError::Cifra("nonces esgotados".into()))?;
            let tamanho = u16::try_from(n).map_err(|_| NetError::Cifra("pedaço cifrado grande demais".into()))?;
            stream.write_all(&tamanho.to_be_bytes())?;
            stream.write_all(saida.get(..n).unwrap_or_default())?;
        }
        stream.flush()?;
        Ok(())
    }
}

/// A metade de leitura: texto cifrado ainda não aberto e o nonce esperado.
struct CifraLeitura {
    sessao: Arc<cifra::Sessao>,
    nonce: u64,
    cru: Vec<u8>,
}

/// Conexão com um par, sobre um `TcpStream`.
pub struct Conexao {
    stream: TcpStream,
    magic: [u8; 4],
    buffer: Vec<u8>,
    escrita: Option<CifraEscrita>,
    leitura: Option<CifraLeitura>,
}

/// Quem escreve numa conexão a partir de outra thread.
pub struct Escritor {
    stream: TcpStream,
    magic: [u8; 4],
    escrita: Option<CifraEscrita>,
}

impl Escritor {
    /// Manda uma mensagem, enquadrada e cifrada se a conexão tiver cifra.
    ///
    /// # Errors
    /// Socket fechado ou mensagem que não cabe num quadro.
    pub fn enviar(&mut self, msg: &Message) -> Result<(), NetError> {
        let quadro = encode_frame(&self.magic, msg)?;
        escrever(&mut self.stream, self.escrita.as_ref(), &quadro)
    }
}

fn escrever(stream: &mut TcpStream, escrita: Option<&CifraEscrita>, dados: &[u8]) -> Result<(), NetError> {
    match escrita {
        Some(c) => c.escrever(stream, dados),
        None => {
            stream.write_all(dados)?;
            stream.flush()?;
            Ok(())
        }
    }
}

impl Conexao {
    /// Envolve um socket já aberto, sem cifra (versão 1 do protocolo).
    pub fn nova(stream: TcpStream, magic: [u8; 4]) -> Self {
        Self { stream, magic, buffer: Vec::new(), escrita: None, leitura: None }
    }

    /// Faz o aperto de mão Noise XX no socket e devolve a conexão cifrada, com
    /// a chave estática que o par provou ter.
    ///
    /// # Errors
    /// Ver [`cifra::apertar_mao`].
    pub fn com_cifra(
        mut stream: TcpStream,
        magic: [u8; 4],
        identidade: &Identidade,
        papel: Papel,
        prazo: Duration,
    ) -> Result<(Self, [u8; 32]), NetError> {
        let sessao = Arc::new(cifra::apertar_mao(&mut stream, &magic, identidade, papel, prazo)?);
        let chave = sessao.chave_do_par;
        let conexao = Self {
            stream,
            magic,
            buffer: Vec::new(),
            escrita: Some(CifraEscrita { sessao: Arc::clone(&sessao), nonce: Arc::new(Mutex::new(0)) }),
            leitura: Some(CifraLeitura { sessao, nonce: 0, cru: Vec::new() }),
        };
        Ok((conexao, chave))
    }

    /// Endereço do outro lado, para registro e log.
    pub fn par(&self) -> std::io::Result<std::net::SocketAddr> {
        self.stream.peer_addr()
    }

    /// Clona o socket cru (sem cifra). Serve a testes de ataque.
    pub fn clonar_stream(&self) -> std::io::Result<TcpStream> {
        self.stream.try_clone()
    }

    /// Um escritor para outra thread, que continua a numeração da cifra.
    ///
    /// # Errors
    /// Quando o socket não pode ser clonado.
    pub fn escritor(&self) -> std::io::Result<Escritor> {
        Ok(Escritor { stream: self.stream.try_clone()?, magic: self.magic, escrita: self.escrita.clone() })
    }

    /// Tempo máximo esperando bytes numa leitura. `None` bloqueia sem limite.
    pub fn definir_timeout(&self, t: Option<Duration>) -> std::io::Result<()> {
        self.stream.set_read_timeout(t)
    }

    /// Manda uma mensagem, enquadrada (e cifrada, se houver cifra).
    ///
    /// # Errors
    /// Socket fechado ou mensagem que não cabe num quadro.
    pub fn enviar(&mut self, msg: &Message) -> Result<(), NetError> {
        let quadro = encode_frame(&self.magic, msg)?;
        escrever(&mut self.stream, self.escrita.as_ref(), &quadro)
    }

    /// Manda bytes arbitrários pelo canal, cifrados se houver cifra. Existe
    /// para os testes de ataque: um par autenticado que manda lixo por dentro
    /// da cifra precisa ser derrubado do mesmo jeito.
    ///
    /// # Errors
    /// Socket fechado.
    pub fn enviar_cru(&mut self, dados: &[u8]) -> Result<(), NetError> {
        escrever(&mut self.stream, self.escrita.as_ref(), dados)
    }

    /// Bloqueia até receber uma mensagem inteira, ou erro.
    ///
    /// Junta o que o socket entrega até `decode_frame` fechar um quadro. Um
    /// `WouldBlock`/`TimedOut` no meio de um quadro é repassado como erro de
    /// I/O: quem chama decide se tenta de novo. Nada do que já chegou se perde.
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
            match self.leitura.as_mut() {
                None => {
                    let mut pedaco = [0u8; 8192];
                    let lido = self.stream.read(&mut pedaco)?;
                    if lido == 0 {
                        return Err(NetError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)));
                    }
                    self.buffer.extend_from_slice(pedaco.get(..lido).unwrap_or(&[]));
                }
                Some(c) => {
                    if abrir_pedaco(c, &mut self.buffer)? {
                        continue;
                    }
                    let mut pedaco = [0u8; 8192];
                    let lido = self.stream.read(&mut pedaco)?;
                    if lido == 0 {
                        return Err(NetError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)));
                    }
                    c.cru.extend_from_slice(pedaco.get(..lido).unwrap_or(&[]));
                }
            }
        }
    }

    /// Fecha os dois sentidos do socket.
    pub fn fechar(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// Se já chegou um pedaço cifrado inteiro, decifra e junta ao buffer.
/// Devolve `true` quando abriu um pedaço.
fn abrir_pedaco(c: &mut CifraLeitura, buffer: &mut Vec<u8>) -> Result<bool, NetError> {
    let Some((cab, resto)) = c.cru.split_first_chunk::<2>() else { return Ok(false) };
    let tamanho = usize::from(u16::from_be_bytes(*cab));
    if tamanho < ETIQUETA {
        return Err(NetError::Cifra(format!("pedaço cifrado com {tamanho} bytes")));
    }
    let Some(corpo) = resto.get(..tamanho) else { return Ok(false) };
    let mut claro = vec![0u8; tamanho];
    let n = c
        .sessao
        .transporte
        .read_message(c.nonce, corpo, &mut claro)
        .map_err(|_| NetError::Cifra("pedaço não autentica: alterado no caminho ou fora de ordem".into()))?;
    c.nonce = c.nonce.checked_add(1).ok_or_else(|| NetError::Cifra("nonces esgotados".into()))?;
    buffer.extend_from_slice(claro.get(..n).unwrap_or_default());
    c.cru.drain(..tamanho.saturating_add(2));
    Ok(true)
}
