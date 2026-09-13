// ✝ Neemias 4:17 — “Com uma das mãos faziam a obra, e com a outra seguravam a arma.”
//! O transporte: aceita conexões, disca para pares, faz o aperto de mão,
//! mantém uma thread de leitura e uma de escrita por par, e **descobre** novos
//! pares sozinho a partir dos que já conhece.
//!
//! Sem async e sem biblioteca de rede: `std::net` mais threads. Uma thread por
//! par escala mal para milhares de conexões, e é de propósito — a testnet cabe
//! nisso, e o código fica simples de auditar.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use auron_wire::{EnderecoDeRede, MAX_ADDRS, Message, Ponta, encode_frame};

use crate::conexao::{Conexao, NetError};
use crate::no::No;

const TIMEOUT: Duration = Duration::from_millis(400);
const INTERVALO_MANUTENCAO: Duration = Duration::from_secs(2);
/// Quantos pares o nó tenta manter conectados.
const ALVO_PARES: usize = 8;
/// Quantos endereços anunciar num `ADDRS`.
const MAX_ANUNCIO: usize = 64;

fn agora_seg() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Nonce de sessão: relógio em nanos misturado a um contador, para dois apertos
/// de mão nunca saírem iguais. Não é segredo; só serve de prova de vivacidade.
fn nonce_sessao() -> u64 {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    nanos ^ CONTADOR.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
}

/// `SocketAddr` num endereço de rede, com a porta de ESCUTA do par (não a
/// efêmera da conexão). Só IPv4 e IPv6.
fn para_endereco(ip: std::net::IpAddr, porta_escuta: u16) -> EnderecoDeRede {
    match ip {
        std::net::IpAddr::V4(v4) => EnderecoDeRede { familia: 4, ip: v4.octets().to_vec(), porta: porta_escuta, visto_em: agora_seg() },
        std::net::IpAddr::V6(v6) => EnderecoDeRede { familia: 6, ip: v6.octets().to_vec(), porta: porta_escuta, visto_em: agora_seg() },
    }
}

/// Texto `ip:porta` de um endereço de rede, ou `None` se malformado.
fn endereco_para_str(e: &EnderecoDeRede) -> Option<String> {
    match (e.familia, e.ip.len()) {
        (4, 4) => {
            let o = &e.ip;
            Some(format!("{}.{}.{}.{}:{}", o.first()?, o.get(1)?, o.get(2)?, o.get(3)?, e.porta))
        }
        (6, 16) => {
            let mut b = [0u8; 16];
            b.copy_from_slice(&e.ip);
            Some(format!("[{}]:{}", std::net::Ipv6Addr::from(b), e.porta))
        }
        _ => None,
    }
}

/// A rede vista por um nó: o estado compartilhado, os pares conectados e o
/// livro de endereços conhecidos.
pub struct Rede {
    /// O nó (cadeia e mempool), atrás de um cadeado.
    pub no: Arc<Mutex<No>>,
    magic: [u8; 4],
    pares: Mutex<HashMap<u64, Sender<Message>>>,
    proximo_id: AtomicU64,
    parar: AtomicBool,
    escuta: AtomicU16,
    /// Endereços conhecidos, `"ip:porta"` -> visto em (segundos Unix).
    livro: Mutex<BTreeMap<String, u64>>,
    /// Endereços de escuta dos pares conectados agora — para não reconectar.
    conectados: Mutex<BTreeMap<String, ()>>,
    manutencao_ligada: AtomicBool,
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
            escuta: AtomicU16::new(0),
            livro: Mutex::new(BTreeMap::new()),
            conectados: Mutex::new(BTreeMap::new()),
            manutencao_ligada: AtomicBool::new(false),
        })
    }

    /// Quantos pares estão conectados agora.
    pub fn pares_conectados(&self) -> usize {
        self.pares.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Quantos endereços o nó conhece no livro.
    pub fn enderecos_conhecidos(&self) -> usize {
        self.livro.lock().map(|l| l.len()).unwrap_or(0)
    }

    /// Semeia o livro com um endereço de arranque (a "semente de descoberta").
    pub fn semear(&self, addr: &str) {
        if let Ok(mut livro) = self.livro.lock() {
            livro.insert(addr.to_string(), agora_seg());
        }
    }

    /// Manda todos os pares pararem. As threads saem no próximo tique.
    pub fn desligar(&self) {
        self.parar.store(true, Ordering::Relaxed);
    }

    fn parando(&self) -> bool {
        self.parar.load(Ordering::Relaxed)
    }

    fn meu_addr_loopback(&self) -> Option<String> {
        let porta = self.escuta.load(Ordering::Relaxed);
        (porta != 0).then(|| format!("127.0.0.1:{porta}"))
    }

    fn ponta_local(&self) -> Ponta {
        let (altura, trabalho) = match self.no.lock() {
            Ok(n) => (n.chain.height(), n.chain.total_work().to_be32().unwrap_or([0xff; 32])),
            Err(_) => (0, [0u8; 32]),
        };
        Ponta {
            protocolo: auron_wire::PROTOCOL_VERSION,
            magic: self.magic,
            altura,
            trabalho,
            nonce: nonce_sessao(),
            porta_escuta: self.escuta.load(Ordering::Relaxed),
        }
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

    fn aprender(&self, addr: String) {
        // Nunca aprende o próprio endereço de loopback.
        if self.meu_addr_loopback().is_some_and(|meu| meu == addr) {
            return;
        }
        if let Ok(mut livro) = self.livro.lock() {
            livro.insert(addr, agora_seg());
        }
    }

    fn enderecos_para_anunciar(&self) -> Vec<EnderecoDeRede> {
        let livro = match self.livro.lock() {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        };
        livro
            .iter()
            .take(MAX_ANUNCIO.min(MAX_ADDRS as usize))
            .filter_map(|(addr, visto)| {
                let sa: SocketAddr = addr.parse().ok()?;
                let mut e = para_endereco(sa.ip(), sa.port());
                e.visto_em = *visto;
                Some(e)
            })
            .collect()
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

    /// Injeta na rede um bloco criado por este nó: aplica na cadeia e, se
    /// avançar, anuncia a todos os pares.
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

    /// Escuta em `addr` e devolve a porta real (útil quando `addr` pede porta 0).
    pub fn escutar(self: &Arc<Self>, addr: impl ToSocketAddrs) -> std::io::Result<u16> {
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let porta = listener.local_addr()?.port();
        self.escuta.store(porta, Ordering::Relaxed);
        self.garantir_manutencao();
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
                            let _ = r.servir(stream);
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
        self.garantir_manutencao();
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = rede.servir(stream);
        });
        Ok(())
    }

    /// Sobe a thread de manutenção uma única vez: ela pede endereços aos pares
    /// e disca para candidatos do livro até chegar a `ALVO_PARES`. É a base da
    /// retomada automática da seção 22.
    fn garantir_manutencao(self: &Arc<Self>) {
        if self.manutencao_ligada.swap(true, Ordering::Relaxed) {
            return;
        }
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            while !rede.parando() {
                std::thread::sleep(INTERVALO_MANUTENCAO);
                if rede.parando() {
                    break;
                }
                // Pede a todos os pares o que eles conhecem.
                rede.difundir(u64::MAX, &Message::GetAddrs);
                // Disca para candidatos que ainda não são pares.
                if rede.pares_conectados() < ALVO_PARES {
                    for addr in rede.candidatos() {
                        if rede.pares_conectados() >= ALVO_PARES {
                            break;
                        }
                        let _ = rede.conectar(addr.as_str());
                    }
                }
            }
        });
    }

    fn candidatos(&self) -> Vec<String> {
        let conectados = self.conectados.lock().map(|c| c.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
        let meu = self.meu_addr_loopback();
        self.livro
            .lock()
            .map(|livro| {
                livro
                    .keys()
                    .filter(|a| !conectados.contains(a) && meu.as_ref() != Some(*a))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// O aperto de mão e depois o laço de mensagens.
    fn servir(self: &Arc<Self>, stream: TcpStream) -> Result<(), NetError> {
        stream.set_read_timeout(Some(TIMEOUT))?;
        // Os dois lados rodam o mesmo laço; o aperto de mão resolve quem fala
        // primeiro (quem discou manda HELLO após um timeout curto de escuta).
        let ip_par = stream.peer_addr().ok().map(|s| s.ip());
        let mut conexao = Conexao::nova(stream, self.magic);
        let par_ponta = self.aperto_de_mao(&mut conexao)?;

        // Aprende onde o par escuta, e marca como conectado para não rediscar.
        let addr_escuta = match (ip_par, par_ponta.porta_escuta) {
            (Some(ip), porta) if porta != 0 => Some(para_endereco(ip, porta)),
            _ => None,
        };
        let addr_str = addr_escuta.as_ref().and_then(endereco_para_str);
        if let Some(a) = &addr_str {
            self.aprender(a.clone());
            if let Ok(mut c) = self.conectados.lock() {
                c.insert(a.clone(), ());
            }
        }

        let stream_escrita = conexao.clonar_stream()?;
        let (saida, entrada) = channel::<Message>();
        let id = self.registrar(saida.clone());

        if let Ok(no) = self.no.lock() {
            let meu = no.chain.total_work().to_be32().unwrap_or([0xff; 32]);
            if par_ponta.trabalho > meu {
                let _ = saida.send(no.pedir_sincronizacao());
            }
        }
        // Já pede endereços de cara, para a descoberta andar rápido.
        let _ = saida.send(Message::GetAddrs);

        let magic = self.magic;
        let rede_escrita = Arc::clone(self);
        let escritora = std::thread::spawn(move || {
            escrever_laco(stream_escrita, magic, &entrada, &rede_escrita);
        });

        let resultado = self.ler_laco(&mut conexao, id, &saida);

        self.remover(id);
        if let (Some(a), Ok(mut c)) = (&addr_str, self.conectados.lock()) {
            c.remove(a);
        }
        conexao.fechar();
        drop(saida);
        let _ = escritora.join();
        resultado
    }

    fn aperto_de_mao(&self, conexao: &mut Conexao) -> Result<Ponta, NetError> {
        let confere = |p: &Ponta| -> Result<(), NetError> {
            if p.protocolo != auron_wire::PROTOCOL_VERSION {
                return Err(NetError::Handshake(format!("protocolo {}", p.protocolo)));
            }
            if p.magic != self.magic {
                return Err(NetError::Handshake("magic de outra rede".into()));
            }
            Ok(())
        };

        // O primeiro a falar manda HELLO. Como os dois lados podem discar, cada
        // lado tenta ler primeiro; num timeout curto, assume que é ele quem abre.
        // Para simplificar e casar com os testes, quem recebe a conexão espera o
        // HELLO, e quem disca manda. Aqui detectamos por quem chega primeiro:
        // tentamos receber; se vier HELLO, respondemos ACK (lado servidor); se
        // der timeout, mandamos HELLO (lado cliente) e esperamos o ACK.
        match conexao.receber() {
            Ok(Message::Hello(ponta)) => {
                confere(&ponta)?;
                conexao.enviar(&Message::HelloAck { ponta: self.ponta_local(), eco: ponta.nonce })?;
                Ok(ponta)
            }
            Ok(_) => Err(NetError::Handshake("esperava HELLO".into())),
            Err(NetError::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
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
            }
            Err(e) => Err(e),
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
                Err(e) => return Err(e),
            };

            // A descoberta de pares é transporte, não consenso: tratada aqui,
            // com o livro, antes de chegar ao nó.
            match msg {
                Message::GetAddrs => {
                    let _ = saida.send(Message::Addrs(self.enderecos_para_anunciar()));
                    continue;
                }
                Message::Addrs(lista) => {
                    for e in lista.iter().take(MAX_ADDRS as usize) {
                        if let Some(a) = endereco_para_str(e) {
                            self.aprender(a);
                        }
                    }
                    continue;
                }
                _ => {}
            }

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
