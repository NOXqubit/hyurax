//! Cifra da conexão (seção 21.3): `Noise_XX_25519_ChaChaPoly_BLAKE2s` sobre TCP.
//!
//! - **XX**: os dois lados provam a própria chave estática sem precisar
//!   conhecer a do outro antes. Combina com descoberta aberta de pares.
//! - **Identidade de nó**: a chave estática X25519. É persistente (o nó guarda
//!   num arquivo) e separada da chave da carteira.
//! - **Prólogo** `AURON-WIRE-v2 || magic`: os dois lados precisam concordar
//!   com ele, então um nó de outra rede falha já no aperto de mão.
//!
//! Depois do aperto de mão, cada quadro do `auron-wire` é cortado em pedaços de
//! até 65 519 bytes, e cada pedaço vai cifrado e autenticado com um nonce que
//! só cresce, prefixado pelo tamanho em `u16`. Um byte alterado no caminho faz
//! a autenticação falhar e a conexão cair.
//!
//! O que a cifra NÃO faz: esconder que existe uma conexão, nem o tamanho
//! aproximado do que passa. Ela impede ler e alterar o conteúdo.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use snow::params::{CipherChoice, DHChoice, HashChoice, NoiseParams};
use snow::resolvers::{CryptoResolver, DefaultResolver};
use snow::types::{Cipher, Dh, Hash, Random};

use crate::conexao::NetError;
use crate::entropia;

/// O padrão Noise da versão 2 do protocolo.
pub const PADRAO_NOISE: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
/// Começo do prólogo; a magic da rede vem logo depois.
pub const PROLOGO: &[u8] = b"AURON-WIRE-v2";
/// Maior mensagem Noise.
pub const MAX_MENSAGEM: usize = 65_535;
/// Tamanho da etiqueta de autenticação.
pub const ETIQUETA: usize = 16;
/// Maior pedaço de texto claro por mensagem cifrada.
pub const MAX_PEDACO: usize = MAX_MENSAGEM - ETIQUETA;
/// Maior mensagem aceita durante o aperto de mão. As do XX têm menos de 100
/// bytes; recusar acima disso derruba lixo sem esperar o prazo.
const MAX_APERTO: usize = 1024;

// ------------------------------------------------------------------ gerador

/// Gerador aleatório para o `snow`, alimentado pela entropia do sistema.
struct Gerador;

impl Random for Gerador {
    fn try_fill_bytes(&mut self, destino: &mut [u8]) -> Result<(), snow::Error> {
        entropia::preencher(destino).map_err(|_| snow::Error::Rng)
    }
}

/// O resolvedor padrão do `snow`, com o nosso gerador no lugar do `getrandom`.
struct Resolvedor(DefaultResolver);

impl CryptoResolver for Resolvedor {
    fn resolve_rng(&self) -> Option<Box<dyn Random>> {
        Some(Box::new(Gerador))
    }
    fn resolve_dh(&self, escolha: &DHChoice) -> Option<Box<dyn Dh>> {
        self.0.resolve_dh(escolha)
    }
    fn resolve_hash(&self, escolha: &HashChoice) -> Option<Box<dyn Hash>> {
        self.0.resolve_hash(escolha)
    }
    fn resolve_cipher(&self, escolha: &CipherChoice) -> Option<Box<dyn Cipher>> {
        self.0.resolve_cipher(escolha)
    }
}

fn erro(contexto: &str, e: snow::Error) -> NetError {
    NetError::Cifra(format!("{contexto}: {e:?}"))
}

fn parametros() -> Result<NoiseParams, NetError> {
    PADRAO_NOISE.parse().map_err(|e| erro("padrão Noise", e))
}

// --------------------------------------------------------------- identidade

/// Identidade de um nó: a chave estática X25519 usada na cifra.
#[derive(Clone)]
pub struct Identidade {
    segredo: [u8; 32],
    publica: [u8; 32],
}

impl std::fmt::Debug for Identidade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // O segredo nunca aparece em log.
        f.debug_struct("Identidade").field("publica", &self.publica).finish_non_exhaustive()
    }
}

impl Identidade {
    /// Identidade nova, com segredo tirado da entropia do sistema.
    ///
    /// # Errors
    /// Quando o sistema não entrega entropia.
    pub fn nova() -> Result<Self, NetError> {
        let mut segredo = [0u8; 32];
        entropia::preencher(&mut segredo).map_err(NetError::Cifra)?;
        Self::de_segredo(segredo)
    }

    /// Recria a identidade a partir de um segredo guardado.
    ///
    /// # Errors
    /// Quando a curva não está disponível (não acontece com as features do projeto).
    pub fn de_segredo(segredo: [u8; 32]) -> Result<Self, NetError> {
        let mut dh = Resolvedor(DefaultResolver)
            .resolve_dh(&DHChoice::Curve25519)
            .ok_or_else(|| NetError::Cifra("Curve25519 indisponível".into()))?;
        dh.set(&segredo);
        let publica: [u8; 32] = dh
            .pubkey()
            .try_into()
            .map_err(|_| NetError::Cifra("chave pública com tamanho errado".into()))?;
        Ok(Self { segredo, publica })
    }

    /// A chave pública: é assim que os outros nós reconhecem este.
    pub const fn publica(&self) -> [u8; 32] {
        self.publica
    }

    /// O segredo, para gravar no arquivo de identidade do nó.
    pub const fn segredo(&self) -> &[u8; 32] {
        &self.segredo
    }
}

// ------------------------------------------------------------ aperto de mão

/// Quem abriu a conexão. Quem discou inicia o Noise e fala primeiro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Papel {
    /// Abriu a conexão: inicia.
    Discou,
    /// Aceitou a conexão: responde.
    Recebeu,
}

/// Resultado do aperto de mão: a sessão cifrada e a chave estática do par.
pub struct Sessao {
    /// Estado de transporte; cada direção tem o próprio nonce, contado fora.
    pub transporte: snow::StatelessTransportState,
    /// Chave estática que o par provou ter.
    pub chave_do_par: [u8; 32],
}

fn eh_espera(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
}

fn ler_exato(stream: &mut TcpStream, destino: &mut [u8], ate: Instant) -> Result<(), NetError> {
    let mut lido = 0usize;
    while lido < destino.len() {
        let resto = destino.get_mut(lido..).ok_or_else(|| NetError::Cifra("leitura fora da faixa".into()))?;
        match stream.read(resto) {
            Ok(0) => return Err(NetError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))),
            Ok(n) => lido = lido.saturating_add(n),
            Err(e) if eh_espera(&e) && Instant::now() < ate => {}
            Err(e) => return Err(NetError::Io(e)),
        }
    }
    Ok(())
}

fn mandar(stream: &mut TcpStream, mensagem: &[u8]) -> Result<(), NetError> {
    let tamanho = u16::try_from(mensagem.len()).map_err(|_| NetError::Cifra("mensagem de aperto grande demais".into()))?;
    stream.write_all(&tamanho.to_be_bytes())?;
    stream.write_all(mensagem)?;
    stream.flush()?;
    Ok(())
}

fn receber(stream: &mut TcpStream, ate: Instant) -> Result<Vec<u8>, NetError> {
    let mut cab = [0u8; 2];
    ler_exato(stream, &mut cab, ate)?;
    let tamanho = usize::from(u16::from_be_bytes(cab));
    if tamanho > MAX_APERTO {
        return Err(NetError::Cifra(format!("mensagem de aperto com {tamanho} bytes")));
    }
    let mut corpo = vec![0u8; tamanho];
    ler_exato(stream, &mut corpo, ate)?;
    Ok(corpo)
}

/// Faz o aperto de mão Noise XX num socket recém-aberto.
///
/// # Errors
/// Socket fechado, prazo estourado, mensagem malformada ou autenticação que
/// não fecha (outra rede, par que não tem a chave que diz ter, alteração no
/// caminho).
pub fn apertar_mao(
    stream: &mut TcpStream,
    magic: &[u8; 4],
    identidade: &Identidade,
    papel: Papel,
    prazo: Duration,
) -> Result<Sessao, NetError> {
    let ate = Instant::now().checked_add(prazo).unwrap_or_else(Instant::now);
    let mut prologo = PROLOGO.to_vec();
    prologo.extend_from_slice(magic);
    let construtor = snow::Builder::with_resolver(parametros()?, Box::new(Resolvedor(DefaultResolver)))
        .local_private_key(&identidade.segredo)
        .map_err(|e| erro("chave local", e))?
        .prologue(&prologo)
        .map_err(|e| erro("prólogo", e))?;
    let mut estado = match papel {
        Papel::Discou => construtor.build_initiator(),
        Papel::Recebeu => construtor.build_responder(),
    }
    .map_err(|e| erro("montar o aperto de mão", e))?;

    let mut saida = [0u8; MAX_APERTO];
    let mut entrada = [0u8; MAX_APERTO];
    // XX: -> e ; <- e, ee, s, es ; -> s, se
    let minha_vez_primeiro = papel == Papel::Discou;
    for passo in 0..3u8 {
        let eu_escrevo = (passo % 2 == 0) == minha_vez_primeiro;
        if eu_escrevo {
            let n = estado.write_message(&[], &mut saida).map_err(|e| erro("escrever aperto", e))?;
            mandar(stream, saida.get(..n).unwrap_or_default())?;
        } else {
            let msg = receber(stream, ate)?;
            estado.read_message(&msg, &mut entrada).map_err(|e| erro("aperto de mão recusado", e))?;
        }
    }
    let chave_do_par: [u8; 32] = estado
        .get_remote_static()
        .and_then(|c| c.try_into().ok())
        .ok_or_else(|| NetError::Cifra("par sem chave estática".into()))?;
    let transporte = estado.into_stateless_transport_mode().map_err(|e| erro("modo de transporte", e))?;
    Ok(Sessao { transporte, chave_do_par })
}
