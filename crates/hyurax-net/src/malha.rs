// ✝ Eclesiastes 4:12 — “O cordão de três dobras não se quebra tão depressa.”
//! Malha: os nós se encontram e se alcançam sem servidor central
//! (`docs/HYURAX-MALHA.md`).
//!
//! Tradução de `reference/hyurax/malha.py`, conferida por
//! `vectors/malha.json`; o gabarito descreve cada campo. Fora do consenso.
//!
//! - [`MensagemMalha`]: dentro da conexão cifrada, tipo [`TIPO_MALHA`].
//! - [`Prefixo`]: os primeiros bytes de uma conexão TCP nova, antes da cifra
//!   (verificação de alcance, reserva de ponte, pedido de circuito).
//! - [`AnuncioVizinho`]: o datagrama de descoberta na rede local.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use hyurax_codec::{CodecError, Reader, Writer};

/// Tipo da mensagem de rede da malha ("MA").
pub const TIPO_MALHA: u16 = 0x4D41;
/// Versão do corpo.
pub const VERSAO: u8 = 1;
/// Porta UDP da descoberta na rede local.
pub const PORTA_VIZINHOS: u16 = 8792;
/// Maior lista de pontes numa mensagem.
pub const MAX_PONTES: usize = 32;
/// Prefixo da verificação de alcance.
pub const PREFIXO_VERIFICAR: &[u8; 4] = b"HXV1";
/// Prefixo do socket de reserva.
pub const PREFIXO_RESERVA: &[u8; 4] = b"HXR1";
/// Prefixo do pedido de circuito.
pub const PREFIXO_CIRCUITO: &[u8; 4] = b"HXC1";
/// Prefixo do anúncio na rede local.
pub const PREFIXO_VIZINHO: &[u8; 4] = b"HXD1";
/// Prefixo do pacote do Éter.
pub const PREFIXO_PACOTE: &[u8; 4] = b"HXP1";
/// Quadros num pacote.
pub const MAX_QUADROS_NO_PACOTE: usize = 4096;
/// Tamanho do anúncio na rede local.
pub const TAMANHO_ANUNCIO: usize = 42;

/// Mensagem malformada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroMalha {
    /// Bytes que não fecham.
    Codec(CodecError),
    /// Campo fora da faixa, versão ou subtipo desconhecidos.
    Invalida(&'static str),
}

impl std::fmt::Display for ErroMalha {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "{e}"),
            Self::Invalida(m) => f.write_str(m),
        }
    }
}

impl From<CodecError> for ErroMalha {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

/// Uma mensagem da malha, dentro da conexão cifrada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MensagemMalha {
    /// "Tente me alcançar no IP que você vê de mim, nesta porta, com este token."
    PedirAlcance {
        /// Porta de escuta de quem pede.
        porta: u16,
        /// Token a mandar pela conexão de verificação.
        token: [u8; 16],
    },
    /// O endereço que quem conferiu viu, e se abriu a conexão.
    Alcance {
        /// IP e porta vistos.
        endereco: SocketAddr,
        /// A conexão de verificação abriu.
        tentou: bool,
    },
    /// "Guarde uma vaga de ponte para mim."
    Reservar {
        /// O token do socket de reserva que vem a seguir.
        token: [u8; 16],
    },
    /// Resposta ao pedido de reserva.
    Reserva {
        /// O token pedido.
        token: [u8; 16],
        /// Se a ponte aceitou.
        aceita: bool,
    },
    /// Nós alcançáveis por pontes: `(alvo, ponte)`.
    Pontes(Vec<([u8; 32], SocketAddr)>),
}

fn escrever_endereco(w: &mut Writer, e: &SocketAddr) {
    match e.ip() {
        IpAddr::V4(v4) => {
            w.u8(4);
            w.fixed(&v4.octets());
        }
        IpAddr::V6(v6) => {
            w.u8(6);
            w.fixed(&v6.octets());
        }
    }
    w.u16(e.port());
}

fn ler_endereco(r: &mut Reader<'_>) -> Result<SocketAddr, ErroMalha> {
    let ip = match r.u8()? {
        4 => IpAddr::V4(Ipv4Addr::from(r.fixed::<4>()?)),
        6 => IpAddr::V6(Ipv6Addr::from(r.fixed::<16>()?)),
        _ => return Err(ErroMalha::Invalida("família de endereço desconhecida")),
    };
    Ok(SocketAddr::new(ip, r.u16()?))
}

fn ler_marca(r: &mut Reader<'_>) -> Result<bool, ErroMalha> {
    match r.u8()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ErroMalha::Invalida("marca inválida")),
    }
}

impl MensagemMalha {
    /// Os bytes do corpo.
    ///
    /// # Errors
    /// Lista de pontes acima do teto.
    pub fn codificar(&self) -> Result<Vec<u8>, ErroMalha> {
        let mut w = Writer::new();
        w.u8(VERSAO);
        match self {
            Self::PedirAlcance { porta, token } => {
                w.u8(1);
                w.u16(*porta);
                w.fixed(token);
            }
            Self::Alcance { endereco, tentou } => {
                w.u8(2);
                escrever_endereco(&mut w, endereco);
                w.u8(u8::from(*tentou));
            }
            Self::Reservar { token } => {
                w.u8(3);
                w.fixed(token);
            }
            Self::Reserva { token, aceita } => {
                w.u8(4);
                w.fixed(token);
                w.u8(u8::from(*aceita));
            }
            Self::Pontes(lista) => {
                if lista.len() > MAX_PONTES {
                    return Err(ErroMalha::Invalida("pontes demais"));
                }
                w.u8(5);
                w.u8(u8::try_from(lista.len()).map_err(|_| ErroMalha::Invalida("pontes demais"))?);
                for (alvo, ponte) in lista {
                    w.fixed(alvo);
                    escrever_endereco(&mut w, ponte);
                }
            }
        }
        Ok(w.into_bytes())
    }

    /// Lê um corpo, sem sobra.
    ///
    /// # Errors
    /// Qualquer coisa que não seja exatamente uma mensagem válida.
    pub fn decodificar(dados: &[u8]) -> Result<Self, ErroMalha> {
        let mut r = Reader::new(dados);
        if r.u8()? != VERSAO {
            return Err(ErroMalha::Invalida("versão de mensagem da malha desconhecida"));
        }
        let m = match r.u8()? {
            1 => Self::PedirAlcance { porta: r.u16()?, token: r.fixed()? },
            2 => {
                let endereco = ler_endereco(&mut r)?;
                Self::Alcance { endereco, tentou: ler_marca(&mut r)? }
            }
            3 => Self::Reservar { token: r.fixed()? },
            4 => {
                let token = r.fixed()?;
                Self::Reserva { token, aceita: ler_marca(&mut r)? }
            }
            5 => {
                let n = usize::from(r.u8()?);
                if n > MAX_PONTES {
                    return Err(ErroMalha::Invalida("pontes demais"));
                }
                let mut lista = Vec::with_capacity(n);
                for _ in 0..n {
                    let alvo = r.fixed::<32>()?;
                    lista.push((alvo, ler_endereco(&mut r)?));
                }
                Self::Pontes(lista)
            }
            _ => return Err(ErroMalha::Invalida("subtipo de mensagem da malha desconhecido")),
        };
        r.finish()?;
        Ok(m)
    }
}

/// O prefixo de uma conexão TCP nova, antes da cifra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prefixo {
    /// Verificação de alcance, com o token que o nó pediu.
    Verificar([u8; 16]),
    /// Socket de reserva de ponte.
    Reserva([u8; 16]),
    /// Pedido de circuito até o alvo.
    Circuito([u8; 32]),
}

impl Prefixo {
    /// Os bytes do prefixo.
    pub fn bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(36);
        match self {
            Self::Verificar(t) => {
                v.extend_from_slice(PREFIXO_VERIFICAR);
                v.extend_from_slice(t);
            }
            Self::Reserva(t) => {
                v.extend_from_slice(PREFIXO_RESERVA);
                v.extend_from_slice(t);
            }
            Self::Circuito(a) => {
                v.extend_from_slice(PREFIXO_CIRCUITO);
                v.extend_from_slice(a);
            }
        }
        v
    }

    /// Quantos bytes seguem o cabeçalho de 4, ou `None` se o cabeçalho não é
    /// de prefixo da malha.
    pub fn tamanho_do_resto(cab: &[u8; 4]) -> Option<usize> {
        match cab {
            c if c == PREFIXO_VERIFICAR || c == PREFIXO_RESERVA => Some(16),
            c if c == PREFIXO_CIRCUITO => Some(32),
            _ => None,
        }
    }

    /// Lê um prefixo inteiro (cabeçalho e resto).
    ///
    /// # Errors
    /// Cabeçalho desconhecido ou tamanho errado.
    pub fn ler(dados: &[u8]) -> Result<Self, ErroMalha> {
        let (cab, resto) = dados.split_first_chunk::<4>().ok_or(ErroMalha::Invalida("prefixo curto"))?;
        let errado = ErroMalha::Invalida("prefixo com tamanho errado");
        match cab {
            c if c == PREFIXO_VERIFICAR => Ok(Self::Verificar(resto.try_into().map_err(|_| errado)?)),
            c if c == PREFIXO_RESERVA => Ok(Self::Reserva(resto.try_into().map_err(|_| errado)?)),
            c if c == PREFIXO_CIRCUITO => Ok(Self::Circuito(resto.try_into().map_err(|_| errado)?)),
            _ => Err(ErroMalha::Invalida("prefixo desconhecido")),
        }
    }
}

/// O anúncio de um nó na rede local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnuncioVizinho {
    /// A rede (magic).
    pub magic: [u8; 4],
    /// Porta de escuta TCP.
    pub porta: u16,
    /// Identidade de rede (chave da cifra).
    pub identidade: [u8; 32],
}

impl AnuncioVizinho {
    /// Os 42 bytes do datagrama.
    pub fn bytes(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.fixed(PREFIXO_VIZINHO);
        w.fixed(&self.magic);
        w.u16(self.porta);
        w.fixed(&self.identidade);
        w.into_bytes()
    }

    /// Lê um datagrama.
    ///
    /// # Errors
    /// Não é anúncio, tamanho errado ou porta zero.
    pub fn ler(dados: &[u8]) -> Result<Self, ErroMalha> {
        let mut r = Reader::new(dados);
        if &r.fixed::<4>()? != PREFIXO_VIZINHO {
            return Err(ErroMalha::Invalida("não é anúncio de vizinho"));
        }
        let a = Self { magic: r.fixed()?, porta: r.u16()?, identidade: r.fixed()? };
        r.finish()?;
        if a.porta == 0 {
            return Err(ErroMalha::Invalida("porta zero"));
        }
        Ok(a)
    }
}

/// O pacote do Éter: quadros de rede (transações e blocos) num arquivo que
/// atravessa pendrive, Bluetooth, som ou rádio (`docs/HYURAX-ETER.md`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pacote {
    /// A rede (magic) dos quadros.
    pub magic: [u8; 4],
    /// Os quadros, cada um inteiro, como chegariam por um par.
    pub quadros: Vec<Vec<u8>>,
}

impl Pacote {
    /// Os bytes do pacote.
    ///
    /// # Errors
    /// Quadros demais, ou quadro grande demais.
    pub fn bytes(&self) -> Result<Vec<u8>, ErroMalha> {
        if self.quadros.len() > MAX_QUADROS_NO_PACOTE {
            return Err(ErroMalha::Invalida("quadros demais no pacote"));
        }
        let mut w = Writer::new();
        w.fixed(PREFIXO_PACOTE);
        w.fixed(&self.magic);
        w.u32(u32::try_from(self.quadros.len()).map_err(|_| ErroMalha::Invalida("quadros demais no pacote"))?);
        for q in &self.quadros {
            w.var_bytes(q)?;
        }
        Ok(w.into_bytes())
    }

    /// Lê um pacote.
    ///
    /// # Errors
    /// Não é pacote, quadros demais ou bytes que não fecham.
    pub fn ler(dados: &[u8]) -> Result<Self, ErroMalha> {
        let mut r = Reader::new(dados);
        if &r.fixed::<4>()? != PREFIXO_PACOTE {
            return Err(ErroMalha::Invalida("não é pacote do Éter"));
        }
        let magic = r.fixed()?;
        let n = usize::try_from(r.u32()?).map_err(|_| ErroMalha::Invalida("quadros demais no pacote"))?;
        if n > MAX_QUADROS_NO_PACOTE {
            return Err(ErroMalha::Invalida("quadros demais no pacote"));
        }
        let mut quadros = Vec::new();
        for _ in 0..n {
            quadros.push(r.var_bytes()?.to_vec());
        }
        r.finish()?;
        Ok(Self { magic, quadros })
    }
}

/// Endereço de rede local (de onde um anúncio de vizinho é aceito).
pub fn ip_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        IpAddr::V6(v6) => {
            let [o0, o1, ..] = v6.octets();
            v6.is_loopback() || (o0 & 0xfe) == 0xfc || (o0 == 0xfe && (o1 & 0xc0) == 0x80)
        }
    }
}

/// Endereço que só existe atrás de NAT de operadora (CGNAT, 100.64.0.0/10)
/// ou de rede privada: um nó que se vê com ele não é alcançável de fora.
pub fn ip_sem_saida(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || (a == 100 && (b & 0xc0) == 64)
        }
        IpAddr::V6(v6) => ip_local(IpAddr::V6(v6)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn cgnat_e_rede_local() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(ip_sem_saida(ip("100.64.0.1")) && ip_sem_saida(ip("100.127.255.254")));
        assert!(!ip_sem_saida(ip("100.128.0.1")) && !ip_sem_saida(ip("177.195.137.243")));
        assert!(ip_sem_saida(ip("10.43.128.1")) && ip_sem_saida(ip("192.168.0.9")));
        assert!(ip_local(ip("192.168.0.9")) && ip_local(ip("fe80::1")) && !ip_local(ip("8.8.8.8")));
    }

    #[test]
    fn prefixo_nunca_confunde_com_o_noise() {
        // o primeiro quadro do Noise XX começa com o tamanho 32 (0x00 0x20)
        assert_eq!(Prefixo::tamanho_do_resto(&[0, 0x20, 1, 2]), None);
        assert_eq!(Prefixo::tamanho_do_resto(PREFIXO_CIRCUITO), Some(32));
        let p = Prefixo::Reserva([3; 16]);
        assert_eq!(Prefixo::ler(&p.bytes()).unwrap(), p);
    }
}
