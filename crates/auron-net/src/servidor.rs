// ✝ Neemias 4:17 — “Com uma das mãos faziam a obra, e com a outra seguravam a arma.”
//! O transporte: aceita conexões, disca para pares, faz o aperto de mão e
//! mantém uma thread de leitura e uma de escrita por par.
//!
//! Sem async e sem biblioteca de rede: `std::net` mais threads. Uma thread por
//! par escala mal para milhares de conexões, e é de propósito — a testnet cabe
//! nisso, e o código fica simples de auditar. Trocar por um reator vem quando a
//! rede crescer, sem mexer nas regras.

use std::collections::HashMap;
use std::io::Write;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use auron_wire::{Message, Ponta, encode_frame};

use crate::conexao::{Conexao, NetError};
use crate::no::No;

const TIMEOUT: Duration = Duration::from_millis(400);

/// Nonce de sessão: relógio em nanos misturado a um contador, para dois apertos
/// de mão nunca saírem iguais. Não é segredo; só serve de prova de vivacidade.
fn nonce_sessao() -> u64 {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    nanos ^ CONTADOR.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
}

/// A rede vista por um nó: o estado compartilhado e os pares conectados.
pub struct Rede {
    /// O nó (cadeia e mempool), atrás de um cadeado.
    pub no: Arc<Mutex<No>>,
    magic: [u8; 4],
    pares: Mutex<HashMap<u64, Sender<Message>>>,
    proximo_id: AtomicU64,
    parar: AtomicBool,
}

impl Rede {
    /// Cria a rede em torno de um nó.
    pub fn nova(no: No) -> Arc<Self> {
        let magic = no.chain.params.magic;
        Arc::new(Self {
            no: Arc::new(Mutex::new(no)),
            magic,
            pares: Mutex::new(HashMap::new()),
            proximo_id: AtomicU64::new(1),
            parar: AtomicBool::new(false),
        })
    }

    /// Quantos pares estão conectados agora.
    pub fn pares_conectados(&self) -> usize {
        self.pares.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Manda todos os pares pararem. As threads saem no próximo tique.
    pub fn desligar(&self) {
        self.parar.store(true, Ordering::Relaxed);
    }

    fn parando(&self) -> bool {
        self.parar.load(Ordering::Relaxed)
    }

    fn ponta_local(&self) -> Ponta {
        let no = self.no.lock();
        let (altura, trabalho) = match no {
            Ok(n) => (n.chain.height(), n.chain.total_work().to_be32().unwrap_or([0xff; 32])),
            Err(_) => (0, [0u8; 32]),
        };
        Ponta { protocolo: auron_wire::PROTOCOL_VERSION, magic: self.magic, altura, trabalho, nonce: nonce_sessao() }
    }

    fn registrar(&self, saida: Sender<Message>) -> u64 {
        let id = self.proximo_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut pares) = self.pares.lock() {
            pares.insert(id, saida);
        }
        id
    }

    fn remover(&self, id: u64) {
        if let Ok(mut pares) = self.pares.lock() {
            pares.remove(&id);
        }
    }

    /// Injeta na rede um bloco criado por este nó (minerado localmente): aplica
    /// na cadeia e, se avançar, anuncia a todos os pares. `Ok(false)` quando o
    /// bloco não avançou a ponta.
    pub fn submeter_bloco(self: &Arc<Self>, bloco: auron_block::Block) -> Result<bool, crate::Malicia> {
        let avancou = {
            let mut no = self.no.lock().map_err(|_| crate::Malicia("nó travado".into()))?;
            no.aceitar_bloco(bloco.clone())?
        };
        if avancou {
            self.difundir(u64::MAX, &Message::Block(Box::new(bloco)));
        }
        Ok(avancou)
    }

    /// Injeta uma transação criada por este nó: põe no mempool e, se for nova,
    /// espalha para os pares.
    pub fn submeter_tx(self: &Arc<Self>, tx: auron_tx::Transfer) -> Result<bool, crate::Malicia> {
        let nova = {
            let mut no = self.no.lock().map_err(|_| crate::Malicia("nó travado".into()))?;
            no.adicionar_tx(tx.clone())?
        };
        if nova {
            self.difundir(u64::MAX, &Message::Tx(Box::new(tx)));
        }
        Ok(nova)
    }

    /// Manda uma mensagem a todos os pares, menos o de origem.
    /// `exceto = u64::MAX` alcança todos (nenhum par tem esse id).
    fn difundir(&self, exceto: u64, msg: &Message) {
        if let Ok(pares) = self.pares.lock() {
            for (id, saida) in pares.iter() {
                if *id != exceto {
                    let _ = saida.send(msg.clone());
                }
            }
        }
    }

    /// Escuta em `addr` e devolve a porta real (útil quando `addr` pede porta 0).
    /// Cada conexão aceita ganha suas threads.
    pub fn escutar(self: &Arc<Self>, addr: impl ToSocketAddrs) -> std::io::Result<u16> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let porta = listener.local_addr()?.port();
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            for fluxo in listener.incoming() {
                if rede.parando() {
                    break;
                }
                match fluxo {
                    Ok(stream) => {
                        let r = Arc::clone(&rede);
                        std::thread::spawn(move || {
                            let _ = r.servir(stream, false);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(porta)
    }

    /// Disca para um par e sobe as threads da conexão.
    pub fn conectar(self: &Arc<Self>, addr: impl ToSocketAddrs) -> std::io::Result<()> {
        let stream = TcpStream::connect(addr)?;
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = rede.servir(stream, true);
        });
        Ok(())
    }

    /// O aperto de mão e depois o laço de mensagens. `discou` diz se este lado
    /// abriu a conexão (manda `HELLO` primeiro).
    fn servir(self: &Arc<Self>, stream: TcpStream, discou: bool) -> Result<(), NetError> {
        stream.set_read_timeout(Some(TIMEOUT))?;
        let mut conexao = Conexao::nova(stream, self.magic);
        let par_ponta = self.aperto_de_mao(&mut conexao, discou)?;

        let stream_escrita = conexao.clonar_stream()?;
        let (saida, entrada) = channel::<Message>();
        let id = self.registrar(saida.clone());

        // Se o par tem mais trabalho que eu, começo a sincronizar.
        if let Ok(no) = self.no.lock() {
            let meu = no.chain.total_work().to_be32().unwrap_or([0xff; 32]);
            if par_ponta.trabalho > meu {
                let _ = saida.send(no.pedir_sincronizacao());
            }
        }

        // Thread de escrita: só ela toca o socket para enviar.
        let magic = self.magic;
        let rede_escrita = Arc::clone(self);
        let escritora = std::thread::spawn(move || {
            escrever_laco(stream_escrita, magic, &entrada, &rede_escrita);
        });

        let resultado = self.ler_laco(&mut conexao, id, &saida);

        self.remover(id);
        conexao.fechar();
        drop(saida);
        let _ = escritora.join();
        resultado
    }

    fn aperto_de_mao(&self, conexao: &mut Conexao, discou: bool) -> Result<Ponta, NetError> {
        let confere = |p: &Ponta| -> Result<(), NetError> {
            if p.protocolo != auron_wire::PROTOCOL_VERSION {
                return Err(NetError::Handshake(format!("protocolo {}", p.protocolo)));
            }
            if p.magic != self.magic {
                return Err(NetError::Handshake("magic de outra rede".into()));
            }
            Ok(())
        };

        if discou {
            let meu = self.ponta_local();
            conexao.enviar(&Message::Hello(meu))?;
            match conexao.receber()? {
                Message::HelloAck { ponta, eco } => {
                    confere(&ponta)?;
                    if eco != meu.nonce {
                        return Err(NetError::Handshake("eco de nonce errado".into()));
                    }
                    Ok(ponta)
                }
                _ => Err(NetError::Handshake("esperava HELLO_ACK".into())),
            }
        } else {
            match conexao.receber()? {
                Message::Hello(ponta) => {
                    confere(&ponta)?;
                    let meu = self.ponta_local();
                    conexao.enviar(&Message::HelloAck { ponta: meu, eco: ponta.nonce })?;
                    Ok(ponta)
                }
                _ => Err(NetError::Handshake("esperava HELLO".into())),
            }
        }
    }

    fn ler_laco(&self, conexao: &mut Conexao, id: u64, saida: &Sender<Message>) -> Result<(), NetError> {
        loop {
            if self.parando() {
                return Ok(());
            }
            let msg = match conexao.receber() {
                Ok(m) => m,
                Err(NetError::Io(e))
                    if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) =>
                {
                    continue;
                }
                Err(e) => return Err(e), // par sumiu ou falou errado: desconecta
            };

            let reacao = {
                let mut no = self.no.lock().map_err(|_| NetError::Handshake("nó travado".into()))?;
                no.tratar(msg)
            };
            match reacao {
                Ok(r) => {
                    for resposta in r.respostas {
                        if saida.send(resposta).is_err() {
                            return Ok(());
                        }
                    }
                    for anuncio in r.difundir {
                        self.difundir(id, &anuncio);
                    }
                }
                // Par malicioso: cai fora. É a base da pontuação por
                // comportamento; medir bans finos vem com os ataques.
                Err(malicia) => return Err(NetError::Handshake(format!("par malicioso: {malicia}"))),
            }
        }
    }
}

fn escrever_laco(mut stream: TcpStream, magic: [u8; 4], entrada: &Receiver<Message>, rede: &Arc<Rede>) {
    loop {
        match entrada.recv_timeout(TIMEOUT) {
            Ok(msg) => match encode_frame(&magic, &msg) {
                Ok(quadro) => {
                    if stream.write_all(&quadro).is_err() || stream.flush().is_err() {
                        return;
                    }
                }
                Err(_) => return,
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if rede.parando() {
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}
