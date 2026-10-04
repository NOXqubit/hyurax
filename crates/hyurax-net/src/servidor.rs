// ✝ Neemias 4:17 — “Com uma das mãos faziam a obra, e com a outra seguravam a arma.”
//! O transporte: aceita conexões, disca para pares, faz o aperto de mão,
//! mantém uma thread de leitura e uma de escrita por par, e **descobre** novos
//! pares sozinho a partir dos que já conhece.
//!
//! Sem async e sem biblioteca de rede: `std::net` mais threads. Uma thread por
//! par escala mal para milhares de conexões, e é de propósito — a testnet cabe
//! nisso, e o código fica simples de auditar.
//!
//! Defesas (porta aberta para a internet):
//! - **entrada:** teto de conexões ao mesmo tempo, no total e por IP,
//!   conferido antes de abrir a thread; IP banido nem é atendido;
//! - **prazos:** o aperto de mão e cada mensagem começada têm prazo para
//!   terminar, também contra quem goteja um byte por vez; toda escrita tem
//!   prazo (quem para de ler cai);
//! - **fila de saída** de cada par com teto de mensagens e de bytes: quem pede
//!   mais do que lê é derrubado, e um anúncio que não cabe é descartado só
//!   para ele;
//! - **livro de endereços** com teto, sem endereço inválido, e cada endereço
//!   com espera crescente entre discagens (endereço aprendido não vira
//!   conexão automática a cada volta, spec 21.6);
//! - **banimento** por uma hora, da identidade e do IP, depois de malícia;
//! - o trabalho que um par **anuncia** no aperto de mão só vale enquanto ele
//!   está conectado.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hyurax_wire::{EnderecoDeRede, MAX_ADDRS, Message, Ponta, encode_frame};

use crate::cifra::{Identidade, Papel};
use crate::conexao::{Conexao, Escritor, NetError};
use crate::no::No;

mod pacote;
mod ponte;
mod websocket;
pub use pacote::ResultadoDoPacote;
pub use ponte::EstadoDaMalha;

const TIMEOUT: Duration = Duration::from_millis(400);
/// Tempo máximo para o par completar o aperto de mão.
const PRAZO_APERTO: Duration = Duration::from_secs(10);
/// Tempo máximo para uma mensagem começada terminar de chegar (o maior
/// quadro, 2 MiB, passa com folga numa ligação de 20 KiB/s).
const PRAZO_QUADRO: Duration = Duration::from_secs(120);
const INTERVALO_MANUTENCAO: Duration = Duration::from_secs(2);
/// Quantos pares de SAÍDA o nó tenta manter. As entradas não contam: quem
/// conecta muitas vezes neste nó não o impede de escolher os próprios pares.
const ALVO_SAIDAS: usize = 8;
/// Conexões de entrada ao mesmo tempo, no total e por IP. O próprio
/// computador (loopback) não tem teto por IP: vários nós numa máquina.
const MAX_ENTRADAS: usize = 32;
const MAX_ENTRADAS_POR_IP: usize = 4;
/// Teto de conexões de entrada vindas da mesma faixa de endereços (IPv4 /24,
/// IPv6 /48). Quem aluga muitos IPs costuma recebê-los vizinhos: isto encarece
/// encher as vagas com identidades falsas, sem impedir (nenhum mecanismo
/// isolado resolve Sybil). Rede local e loopback ficam de fora.
const MAX_ENTRADAS_POR_FAIXA: usize = 8;
/// Quantas transações do mempool entregar a um par que acabou de conectar.
const MAX_MEMPOOL_NA_ENTRADA: usize = 1000;
/// Quantos endereços anunciar num `ADDRS`.
const MAX_ANUNCIO: usize = 64;
/// Teto do livro de endereços.
const MAX_LIVRO: usize = 2000;
/// Discagens por volta da manutenção, cada uma com prazo.
const DISCAGENS_POR_VOLTA: usize = 4;
const PRAZO_DISCAGEM: Duration = Duration::from_secs(3);
/// Espera mínima entre duas discagens ao mesmo endereço, e a máxima.
const ESPERA_MINIMA_S: u64 = 30;
const ESPERA_MAXIMA_S: u64 = 3600;
/// Falhas seguidas que tiram do livro um endereço aprendido.
const FALHAS_PARA_ESQUECER: u32 = 8;
/// Fila de saída de cada par.
const FILA_MENSAGENS: usize = 1024;
const FILA_BYTES: usize = 32 * 1024 * 1024;
/// Quanto tempo dura um banimento.
const BANIMENTO_S: u64 = 3600;

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
fn para_endereco(ip: IpAddr, porta_escuta: u16) -> EnderecoDeRede {
    match ip {
        IpAddr::V4(v4) => EnderecoDeRede { familia: 4, ip: v4.octets().to_vec(), porta: porta_escuta, visto_em: agora_seg() },
        IpAddr::V6(v6) => EnderecoDeRede { familia: 6, ip: v6.octets().to_vec(), porta: porta_escuta, visto_em: agora_seg() },
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

/// Endereço que vale a pena guardar: porta e IP que alguém pode discar.
/// `ws://host[:porta]/caminho` ou `wss://…` bem formado (semente atrás de
/// HTTPS, como no Render).
#[must_use]
pub fn endereco_websocket_valido(texto: &str) -> bool {
    websocket::ler_url(texto).is_some()
}

fn endereco_util(texto: &str) -> bool {
    if texto.starts_with("ws://") || texto.starts_with("wss://") {
        return websocket::ler_url(texto).is_some();
    }
    let Ok(sa) = texto.parse::<SocketAddr>() else { return false };
    if sa.port() == 0 {
        return false;
    }
    match sa.ip() {
        IpAddr::V4(v4) => !(v4.is_unspecified() || v4.is_broadcast() || v4.is_multicast() || v4.is_documentation()),
        IpAddr::V6(v6) => !(v6.is_unspecified() || v6.is_multicast()),
    }
}

/// Tipo de mensagem de rede que leva o ULTRAX (trabalho útil entre nós, fora
/// do consenso). Quem não registrou tratador ignora, como toda mensagem de
/// tipo desconhecido.
pub const TIPO_ULTRAX: u16 = 0x5558;

/// Uma conexão viva com uma identidade: quem discou, a geração (para a
/// limpeza não apagar a ligação que a substituiu) e o socket para fechá-la.
struct Ligacao {
    quem_discou: [u8; 32],
    geracao: u64,
    fechar: Option<TcpStream>,
}

/// Quem recebe as mensagens do ULTRAX: `(id do par, identidade do par, corpo)`.
pub type TratadorUltrax = Arc<dyn Fn(u64, [u8; 32], &[u8]) + Send + Sync>;

/// A fila de saída de um par: quadros já montados, com teto de mensagens
/// (o canal) e de bytes (o contador, que a thread de escrita desconta).
#[derive(Clone)]
struct Saida {
    fila: SyncSender<Vec<u8>>,
    bytes: Arc<AtomicUsize>,
}

/// Por que um quadro não entrou na fila.
enum Recusa {
    /// A fila está no teto: o par não lê no ritmo do que recebe.
    Cheia,
    /// A conexão já acabou.
    Fechada,
}

impl Saida {
    fn mandar(&self, quadro: Vec<u8>) -> Result<(), Recusa> {
        let n = quadro.len();
        if self.bytes.fetch_add(n, Ordering::SeqCst).saturating_add(n) > FILA_BYTES {
            self.bytes.fetch_sub(n, Ordering::SeqCst);
            return Err(Recusa::Cheia);
        }
        match self.fila.try_send(quadro) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.bytes.fetch_sub(n, Ordering::SeqCst);
                Err(match e {
                    TrySendError::Full(_) => Recusa::Cheia,
                    TrySendError::Disconnected(_) => Recusa::Fechada,
                })
            }
        }
    }
}

/// Uma vaga de conexão de entrada; solta sozinha quando a conexão acaba.
struct VagaDeEntrada {
    rede: Arc<Rede>,
    ip: IpAddr,
}

impl Drop for VagaDeEntrada {
    fn drop(&mut self) {
        if let Ok(mut e) = self.rede.entradas.lock()
            && let Some(n) = e.get_mut(&self.ip)
        {
            *n = n.saturating_sub(1);
            if *n == 0 {
                e.remove(&self.ip);
            }
        }
    }
}

/// A rede vista por um nó: o estado compartilhado, os pares conectados e o
/// livro de endereços conhecidos.
pub struct Rede {
    /// O nó (cadeia e mempool), atrás de um cadeado.
    pub no: Arc<Mutex<No>>,
    magic: [u8; 4],
    pares: Mutex<HashMap<u64, Saida>>,
    proximo_id: AtomicU64,
    parar: AtomicBool,
    escuta: AtomicU16,
    /// Endereços conhecidos, `"ip:porta"` -> visto em (segundos Unix).
    livro: Mutex<BTreeMap<String, u64>>,
    /// Sementes do dono: nunca saem do livro.
    sementes: Mutex<BTreeSet<String>>,
    /// Discagens por endereço: (falhas seguidas, próxima tentativa em segundos Unix).
    discagens: Mutex<BTreeMap<String, (u32, u64)>>,
    /// Endereços de escuta dos pares conectados agora — para não reconectar.
    conectados: Mutex<BTreeMap<String, ()>>,
    manutencao_ligada: AtomicBool,
    /// Chave estática da cifra: como os outros nós reconhecem este.
    identidade: Identidade,
    /// Identidades com conexão aberta agora: uma conexão por nó, no máximo.
    identidades_conectadas: Mutex<BTreeMap<[u8; 32], Ligacao>>,
    /// Contador das ligações (para a limpeza não apagar a ligação que
    /// substituiu a sua).
    geracoes: AtomicU64,
    /// O trabalho acumulado que cada par conectado anunciou no aperto de mão.
    trabalho_dos_pares: Mutex<HashMap<u64, [u8; 32]>>,
    /// Quem trata as mensagens do ULTRAX, se alguém registrou.
    ultrax: Mutex<Option<TratadorUltrax>>,
    /// Quem trata as outras extensões (a nuvem), por tipo de mensagem.
    extensoes: Mutex<HashMap<u16, TratadorUltrax>>,
    /// Identidade (chave estática da cifra) de cada par conectado, pelo id.
    identidade_do_par: Mutex<HashMap<u64, [u8; 32]>>,
    /// Conexões de entrada abertas, por IP.
    entradas: Mutex<HashMap<IpAddr, usize>>,
    /// Pares de saída conectados agora.
    saidas: AtomicUsize,
    /// Banidos até (segundos Unix): identidades e IPs.
    banidas: Mutex<BTreeMap<[u8; 32], u64>>,
    ips_banidos: Mutex<BTreeMap<IpAddr, u64>>,
    /// Alcance, pontes e vizinhos (`servidor/ponte.rs`).
    malha: ponte::Malha,
}

impl Rede {
    /// Cria a rede em torno de um nó, com uma identidade nova (que muda a
    /// cada execução). Para identidade persistente, ver [`Rede::com_identidade`].
    ///
    /// # Errors
    /// Quando o sistema não entrega entropia para a chave.
    pub fn nova(no: No) -> Result<Arc<Self>, NetError> {
        Ok(Self::com_identidade(no, Identidade::nova()?))
    }

    /// Cria a rede em torno de um nó, com a identidade dada.
    pub fn com_identidade(no: No, identidade: Identidade) -> Arc<Self> {
        crate::entropia::aquecer();
        let magic = no.chain.params.magic;
        Arc::new(Self {
            no: Arc::new(Mutex::new(no)),
            magic,
            pares: Mutex::new(HashMap::new()),
            proximo_id: AtomicU64::new(1),
            parar: AtomicBool::new(false),
            escuta: AtomicU16::new(0),
            livro: Mutex::new(BTreeMap::new()),
            sementes: Mutex::new(BTreeSet::new()),
            discagens: Mutex::new(BTreeMap::new()),
            conectados: Mutex::new(BTreeMap::new()),
            manutencao_ligada: AtomicBool::new(false),
            identidade,
            identidades_conectadas: Mutex::new(BTreeMap::new()),
            geracoes: AtomicU64::new(1),
            trabalho_dos_pares: Mutex::new(HashMap::new()),
            ultrax: Mutex::new(None),
            extensoes: Mutex::new(HashMap::new()),
            identidade_do_par: Mutex::new(HashMap::new()),
            entradas: Mutex::new(HashMap::new()),
            saidas: AtomicUsize::new(0),
            banidas: Mutex::new(BTreeMap::new()),
            ips_banidos: Mutex::new(BTreeMap::new()),
            malha: ponte::Malha::default(),
        })
    }

    fn quadro(&self, msg: &Message) -> Option<Vec<u8>> {
        encode_frame(&self.magic, msg).ok()
    }

    /// Registra quem trata as mensagens do ULTRAX.
    pub fn ao_receber_ultrax(&self, tratador: TratadorUltrax) {
        if let Ok(mut t) = self.ultrax.lock() {
            *t = Some(tratador);
        }
    }

    /// Manda uma mensagem do ULTRAX a um par. `false` se ele não está mais
    /// conectado, ou não está lendo.
    pub fn enviar_ultrax(&self, par: u64, corpo: Vec<u8>) -> bool {
        let Some(quadro) = self.quadro(&Message::Desconhecida { tipo: TIPO_ULTRAX, corpo }) else { return false };
        self.pares.lock().ok().and_then(|p| p.get(&par).map(|s| s.mandar(quadro).is_ok())).unwrap_or(false)
    }

    /// Manda uma mensagem do ULTRAX a todos os pares. Devolve a quantos.
    pub fn difundir_ultrax(&self, corpo: &[u8]) -> usize {
        let Some(quadro) = self.quadro(&Message::Desconhecida { tipo: TIPO_ULTRAX, corpo: corpo.to_vec() }) else { return 0 };
        self.pares.lock().map_or(0, |p| p.values().filter(|s| s.mandar(quadro.clone()).is_ok()).count())
    }

    /// Registra quem trata as mensagens de rede de `tipo` (uma extensão fora
    /// do consenso, como a nuvem). Os tipos da rede, da malha e do ULTRAX
    /// não podem ser tomados.
    pub fn ao_receber_tipo(&self, tipo: u16, tratador: TratadorUltrax) {
        if tipo == TIPO_ULTRAX || tipo == crate::malha::TIPO_MALHA {
            return;
        }
        if let Ok(mut t) = self.extensoes.lock() {
            t.insert(tipo, tratador);
        }
    }

    /// Manda uma mensagem de extensão a um par. `false` se ele não está mais
    /// conectado, ou a fila dele está cheia.
    pub fn enviar_tipo(&self, par: u64, tipo: u16, corpo: Vec<u8>) -> bool {
        let Some(quadro) = self.quadro(&Message::Desconhecida { tipo, corpo }) else { return false };
        self.pares.lock().ok().and_then(|p| p.get(&par).map(|s| s.mandar(quadro).is_ok())).unwrap_or(false)
    }

    /// Manda uma mensagem de extensão a todos os pares, menos `exceto`.
    /// Devolve a quantos.
    pub fn difundir_tipo(&self, tipo: u16, corpo: &[u8], exceto: Option<u64>) -> usize {
        let Some(quadro) = self.quadro(&Message::Desconhecida { tipo, corpo: corpo.to_vec() }) else { return 0 };
        self.pares.lock().map_or(0, |p| p.iter().filter(|(id, _)| Some(**id) != exceto).filter(|(_, s)| s.mandar(quadro.clone()).is_ok()).count())
    }

    /// Os ids dos pares conectados agora, com a identidade de cada um.
    pub fn pares_com_identidade(&self) -> Vec<(u64, [u8; 32])> {
        self.identidade_do_par.lock().map(|m| m.iter().map(|(k, v)| (*k, *v)).collect()).unwrap_or_default()
    }

    /// Se a cadeia deste nó já tem pelo menos o trabalho que os pares
    /// conectados anunciaram. Sem nenhum par, é `false`.
    pub fn alcancou_os_pares(&self) -> bool {
        let visto = self.trabalho_dos_pares.lock().ok().and_then(|t| t.values().max().copied()).unwrap_or([0u8; 32]);
        if visto == [0u8; 32] {
            return false;
        }
        let meu = self.no.lock().map_or([0u8; 32], |n| n.chain.total_work().to_be32().unwrap_or([0xff; 32]));
        meu >= visto
    }

    /// Chave pública da identidade deste nó.
    pub fn identidade_publica(&self) -> [u8; 32] {
        self.identidade.publica()
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
    /// Semente nunca sai do livro.
    pub fn semear(&self, addr: &str) {
        if let Ok(mut s) = self.sementes.lock() {
            s.insert(addr.to_string());
        }
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

    /// O nó foi desligado ([`Rede::desligar`]).
    pub fn parado(&self) -> bool {
        self.parando()
    }

    /// Bane uma identidade e um IP (que não seja deste computador) por uma hora.
    fn banir(&self, chave: [u8; 32], ip: Option<IpAddr>) {
        let ate = agora_seg().saturating_add(BANIMENTO_S);
        if let Ok(mut b) = self.banidas.lock() {
            b.retain(|_, fim| *fim > agora_seg());
            b.insert(chave, ate);
        }
        if let Some(ip) = ip.filter(|i| !i.is_loopback())
            && let Ok(mut b) = self.ips_banidos.lock()
        {
            b.retain(|_, fim| *fim > agora_seg());
            b.insert(ip, ate);
        }
    }

    fn ip_banido(&self, ip: IpAddr) -> bool {
        self.ips_banidos.lock().is_ok_and(|b| b.get(&ip).is_some_and(|fim| *fim > agora_seg()))
    }

    fn identidade_banida(&self, chave: &[u8; 32]) -> bool {
        self.banidas.lock().is_ok_and(|b| b.get(chave).is_some_and(|fim| *fim > agora_seg()))
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
            protocolo: hyurax_wire::PROTOCOL_VERSION,
            magic: self.magic,
            altura,
            trabalho,
            nonce: nonce_sessao(),
            porta_escuta: self.escuta.load(Ordering::Relaxed),
        }
    }

    fn registrar(&self, saida: Saida) -> u64 {
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
        if let Ok(mut t) = self.trabalho_dos_pares.lock() {
            t.remove(&id);
        }
    }

    fn aprender(&self, addr: String) {
        // Nunca aprende o próprio endereço de loopback, nem endereço inútil.
        if self.meu_addr_loopback().is_some_and(|meu| meu == addr) || !endereco_util(&addr) {
            return;
        }
        let Ok(mut livro) = self.livro.lock() else { return };
        if !livro.contains_key(&addr) && livro.len() >= MAX_LIVRO {
            // cheio: sai o endereço visto há mais tempo (semente nunca sai)
            let sementes = self.sementes.lock().map(|s| s.clone()).unwrap_or_default();
            let velho = livro.iter().filter(|(a, _)| !sementes.contains(*a)).min_by_key(|(_, v)| **v).map(|(a, _)| a.clone());
            match velho {
                Some(v) => {
                    livro.remove(&v);
                }
                None => return,
            }
        }
        livro.insert(addr, agora_seg());
    }

    fn enderecos_para_anunciar(&self) -> Vec<EnderecoDeRede> {
        let livro = match self.livro.lock() {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        };
        // os vistos mais recentemente primeiro
        let mut todos: Vec<(&String, &u64)> = livro.iter().collect();
        todos.sort_by(|a, b| b.1.cmp(a.1));
        todos
            .into_iter()
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
    /// `exceto = u64::MAX` alcança todos (nenhum par tem esse id). Par cuja
    /// fila está cheia perde só este anúncio.
    fn difundir(&self, exceto: u64, msg: &Message) {
        let Some(quadro) = self.quadro(msg) else { return };
        if let Ok(pares) = self.pares.lock() {
            for (id, saida) in pares.iter() {
                if *id != exceto {
                    let _ = saida.mandar(quadro.clone());
                }
            }
        }
    }

    /// Injeta na rede um bloco criado por este nó: aplica na cadeia e, se
    /// avançar, anuncia a todos os pares.
    pub fn submeter_bloco(self: &Arc<Self>, bloco: hyurax_block::Block) -> Result<bool, crate::Malicia> {
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
    pub fn submeter_tx(self: &Arc<Self>, tx: hyurax_tx::Transfer) -> Result<bool, crate::Malicia> {
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
                        // o teto e o banimento valem antes de abrir a thread
                        let Some(vaga) = rede.vaga_de_entrada(&stream) else { continue };
                        let r = Arc::clone(&rede);
                        // prefixo da malha (ponte, alcance) ou aperto Noise
                        std::thread::spawn(move || r.atender_entrada(stream, vaga));
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

    fn vaga_de_entrada(self: &Arc<Self>, stream: &TcpStream) -> Option<VagaDeEntrada> {
        let ip = stream.peer_addr().ok()?.ip();
        if self.ip_banido(ip) {
            return None;
        }
        let mut entradas = self.entradas.lock().ok()?;
        let total: usize = entradas.values().sum();
        let deste_ip = entradas.get(&ip).copied().unwrap_or(0);
        if total >= MAX_ENTRADAS || (!ip.is_loopback() && deste_ip >= MAX_ENTRADAS_POR_IP) {
            return None;
        }
        if let Some(f) = faixa(ip) {
            let da_faixa: usize = entradas.iter().filter(|(outro, _)| faixa(**outro) == Some(f)).map(|(_, n)| *n).sum();
            if da_faixa >= MAX_ENTRADAS_POR_FAIXA {
                return None;
            }
        }
        entradas.insert(ip, deste_ip.saturating_add(1));
        drop(entradas);
        Some(VagaDeEntrada { rede: Arc::clone(self), ip })
    }

    /// Disca para um par (com prazo) e sobe as threads da conexão.
    ///
    /// # Errors
    /// Endereço que não resolve, ou nenhum dos endereços atende no prazo.
    pub fn conectar(self: &Arc<Self>, addr: impl ToSocketAddrs) -> std::io::Result<()> {
        // vai também para o livro: se este aperto de mão falhar (prazo, par
        // ocupado), a manutenção tenta de novo com a espera crescente
        let enderecos: Vec<SocketAddr> = addr.to_socket_addrs()?.collect();
        for sa in &enderecos {
            self.aprender(sa.to_string());
        }
        self.discar(enderecos.as_slice(), None)
    }

    /// Conecta num endereço escrito: `IP:porta`, `nome:porta` ou uma semente
    /// por WebSocket (`wss://nome/p2p`). É o que as sementes usam.
    ///
    /// # Errors
    /// Endereço que não resolve ou não atende (no WebSocket, a discagem segue
    /// numa linha própria e o erro só aparece no registro de discagens).
    pub fn conectar_texto(self: &Arc<Self>, texto: &str) -> std::io::Result<()> {
        if texto.starts_with("ws://") || texto.starts_with("wss://") {
            if websocket::ler_url(texto).is_none() {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "endereço WebSocket inválido"));
            }
            self.aprender(texto.to_string());
            self.discar_ws(texto.to_string());
            return Ok(());
        }
        self.conectar(texto)
    }

    fn discar_texto(self: &Arc<Self>, addr: &str) -> bool {
        if addr.starts_with("ws://") || addr.starts_with("wss://") {
            // anota o resultado sozinho quando terminar
            self.discar_ws(addr.to_string());
            return true;
        }
        let ok = self.discar(addr, Some(addr.to_string())).is_ok();
        self.anotar_discagem(addr, ok);
        ok
    }

    fn discar(self: &Arc<Self>, addr: impl ToSocketAddrs, rotulo: Option<String>) -> std::io::Result<()> {
        let mut ultimo = std::io::Error::from(std::io::ErrorKind::AddrNotAvailable);
        for sa in addr.to_socket_addrs()? {
            if self.ip_banido(sa.ip()) {
                continue;
            }
            match TcpStream::connect_timeout(&sa, PRAZO_DISCAGEM) {
                Ok(stream) => {
                    self.garantir_manutencao();
                    let rede = Arc::clone(self);
                    std::thread::spawn(move || {
                        let _ = rede.servir(stream, Papel::Discou, rotulo, false, None);
                    });
                    return Ok(());
                }
                Err(e) => ultimo = e,
            }
        }
        Err(ultimo)
    }

    /// Sobe a thread de manutenção uma única vez: ela pede endereços aos pares
    /// e disca para candidatos do livro até chegar a `ALVO_SAIDAS`. É a base da
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
                // Disca para alguns candidatos que ainda não são pares.
                if rede.saidas.load(Ordering::Relaxed) < ALVO_SAIDAS {
                    for addr in rede.candidatos() {
                        if rede.saidas.load(Ordering::Relaxed) >= ALVO_SAIDAS || rede.parando() {
                            break;
                        }
                        rede.discar_texto(&addr);
                    }
                }
                rede.cuidar_da_malha();
            }
        });
    }

    /// Anota o resultado de uma discagem: espera crescente para o próximo
    /// discar no mesmo endereço, e endereço aprendido que falha demais sai.
    fn anotar_discagem(&self, addr: &str, ok: bool) {
        let agora = agora_seg();
        let falhas = {
            let Ok(mut d) = self.discagens.lock() else { return };
            let e = d.entry(addr.to_string()).or_insert((0, 0));
            e.0 = if ok { 0 } else { e.0.saturating_add(1) };
            let espera = ESPERA_MINIMA_S.saturating_mul(1u64 << e.0.min(7)).min(ESPERA_MAXIMA_S);
            e.1 = agora.saturating_add(espera);
            e.0
        };
        let semente = self.sementes.lock().is_ok_and(|s| s.contains(addr));
        if falhas >= FALHAS_PARA_ESQUECER && !semente {
            if let Ok(mut l) = self.livro.lock() {
                l.remove(addr);
            }
            if let Ok(mut d) = self.discagens.lock() {
                d.remove(addr);
            }
        }
    }

    fn candidatos(&self) -> Vec<String> {
        let conectados = self.conectados.lock().map(|c| c.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
        let meu = self.meu_addr_loopback();
        let agora = agora_seg();
        let discagens = self.discagens.lock().map(|d| d.clone()).unwrap_or_default();
        self.livro
            .lock()
            .map(|livro| {
                livro
                    .keys()
                    .filter(|a| !conectados.contains(a) && meu.as_ref() != Some(*a))
                    .filter(|a| discagens.get(*a).is_none_or(|(_, proxima)| *proxima <= agora))
                    .take(DISCAGENS_POR_VOLTA)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// O aperto de mão e depois o laço de mensagens. `rotulo`: o endereço
    /// como foi discado (pode ser um nome), marcado como conectado também.
    /// `via_ponte`: o socket é um circuito por uma ponte (o IP do outro lado é
    /// o da ponte, não o do par); `esperado`: a identidade que tem de
    /// responder.
    fn servir(
        self: &Arc<Self>,
        stream: TcpStream,
        papel: Papel,
        rotulo: Option<String>,
        via_ponte: bool,
        esperado: Option<[u8; 32]>,
    ) -> Result<(), NetError> {
        // No Windows o socket aceito herda o modo não bloqueante do listener; no
        // Linux, não. Fixar o modo aqui faz os dois sistemas se comportarem igual.
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        let sa_par = if via_ponte { None } else { stream.peer_addr().ok() };
        let ip_par = sa_par.map(|s| s.ip());
        // Primeiro a cifra (Noise XX); o HELLO já viaja cifrado.
        let (mut conexao, chave_do_par) =
            Conexao::com_cifra(stream, self.magic, &self.identidade, papel, PRAZO_APERTO)?;
        if self.identidade_banida(&chave_do_par) {
            conexao.fechar();
            return Err(NetError::Handshake("identidade banida por malícia".into()));
        }
        if esperado.is_some_and(|e| e != chave_do_par) {
            conexao.fechar();
            return Err(NetError::Handshake("pela ponte respondeu outro nó".into()));
        }
        // A identidade provada na cifra evita conexão duplicada com o mesmo nó
        // (os dois discando ao mesmo tempo, ou o mesmo nó por dois endereços) e
        // conexão consigo mesmo.
        if chave_do_par == self.identidade.publica() {
            conexao.fechar();
            return Err(NetError::Handshake("conexão comigo mesmo".into()));
        }
        // Os dois discando ao mesmo tempo: cada lado recusava a do outro e as
        // duas caíam (e de novo, na mesma batida). Agora fica a conexão que
        // foi discada pela identidade menor; os dois lados chegam à mesma
        // escolha sem conversar, e a outra é fechada.
        let quem_discou = if papel == Papel::Discou { self.identidade.publica() } else { chave_do_par };
        let geracao = self.geracoes.fetch_add(1, Ordering::Relaxed);
        {
            let mut ids = self.identidades_conectadas.lock().map_err(|_| NetError::Handshake("cadeado envenenado".into()))?;
            if let Some(velha) = ids.get(&chave_do_par)
                && velha.quem_discou <= quem_discou
            {
                drop(ids);
                conexao.fechar();
                return Err(NetError::Handshake("já existe conexão com este nó".into()));
            }
            if let Some(velha) = ids.remove(&chave_do_par)
                && let Some(s) = velha.fechar
            {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
            ids.insert(chave_do_par, Ligacao { quem_discou, geracao, fechar: conexao.clonar_stream().ok() });
        }
        if papel == Papel::Discou {
            self.saidas.fetch_add(1, Ordering::Relaxed);
        }
        let resultado = self.servir_cifrado(&mut conexao, papel, sa_par, chave_do_par, rotulo);
        if papel == Papel::Discou {
            self.saidas.fetch_sub(1, Ordering::Relaxed);
        }
        if let Ok(mut ids) = self.identidades_conectadas.lock()
            && ids.get(&chave_do_par).is_some_and(|l| l.geracao == geracao)
        {
            ids.remove(&chave_do_par);
        }
        // malícia provada: a identidade e o IP ficam de fora por uma hora
        if let Err(NetError::Handshake(m)) = &resultado
            && m.starts_with("par malicioso")
        {
            self.banir(chave_do_par, ip_par);
        }
        conexao.fechar();
        resultado
    }

    /// O resto da conexão, já cifrada e com identidade única.
    fn servir_cifrado(
        self: &Arc<Self>,
        conexao: &mut Conexao,
        papel: Papel,
        sa_par: Option<SocketAddr>,
        chave_do_par: [u8; 32],
        rotulo: Option<String>,
    ) -> Result<(), NetError> {
        let ip_par = sa_par.map(|s| s.ip());
        let par_ponta = self.aperto_de_mao(conexao, papel)?;

        // Aprende onde o par escuta, e marca como conectado para não rediscar.
        let addr_escuta = match (ip_par, par_ponta.porta_escuta) {
            (Some(ip), porta) if porta != 0 => Some(para_endereco(ip, porta)),
            _ => None,
        };
        let addr_str = addr_escuta.as_ref().and_then(endereco_para_str);
        let marcados: Vec<String> = addr_str.iter().chain(rotulo.iter()).cloned().collect();
        if let Some(a) = &addr_str {
            self.aprender(a.clone());
        }
        if let Ok(mut c) = self.conectados.lock() {
            for a in &marcados {
                c.insert(a.clone(), ());
            }
        }

        let escritor = conexao.escritor()?;
        let (fila, entrada) = sync_channel::<Vec<u8>>(FILA_MENSAGENS);
        let bytes = Arc::new(AtomicUsize::new(0));
        let saida = Saida { fila, bytes: Arc::clone(&bytes) };
        let id = self.registrar(saida.clone());
        if let Ok(mut m) = self.identidade_do_par.lock() {
            m.insert(id, chave_do_par);
        }
        if let Ok(mut t) = self.trabalho_dos_pares.lock() {
            t.insert(id, par_ponta.trabalho);
        }
        // para a malha: o IP de quem falou direto, e onde estão as pontes
        // possíveis (os pares que este nó discou e que atenderam)
        if let (Some(sa), Ok(mut m)) = (sa_par, self.malha.ip_do_par.lock()) {
            m.insert(id, sa.ip());
        }
        if papel == Papel::Discou
            && let (Some(sa), Ok(mut d)) = (sa_par, self.malha.discado.lock())
        {
            d.insert(id, sa);
        }

        let mut iniciais = Vec::new();
        if let Ok(no) = self.no.lock() {
            let meu = no.chain.total_work().to_be32().unwrap_or([0xff; 32]);
            if par_ponta.trabalho > meu {
                iniciais.push(no.pedir_sincronizacao());
            }
            // Já pede endereços de cara, para a descoberta andar rápido.
            iniciais.push(Message::GetAddrs);
            // E entrega o que espera no mempool: um minerador que acabou de
            // entrar precisa das transações que chegaram antes dele. Com teto,
            // para uma conexão nova não virar enxurrada.
            for tx in no.mempool_ordenado().into_iter().take(MAX_MEMPOOL_NA_ENTRADA) {
                iniciais.push(Message::Tx(Box::new(tx)));
            }
        }
        for m in &iniciais {
            if let Some(q) = self.quadro(m)
                && saida.mandar(q).is_err()
            {
                break;
            }
        }

        let rede_escrita = Arc::clone(self);
        let escritora = std::thread::spawn(move || {
            escrever_laco(escritor, &entrada, &rede_escrita, &bytes);
        });

        let resultado = self.ler_laco(conexao, id, &saida);

        self.remover(id);
        if let Ok(mut m) = self.identidade_do_par.lock() {
            m.remove(&id);
        }
        if let Ok(mut m) = self.malha.ip_do_par.lock() {
            m.remove(&id);
        }
        if let Ok(mut d) = self.malha.discado.lock() {
            d.remove(&id);
        }
        if let Ok(mut c) = self.conectados.lock() {
            for a in &marcados {
                c.remove(a);
            }
        }
        conexao.fechar();
        drop(saida);
        let _ = escritora.join();
        resultado
    }

    fn aperto_de_mao(&self, conexao: &mut Conexao, papel: Papel) -> Result<Ponta, NetError> {
        let confere = |p: &Ponta| -> Result<(), NetError> {
            if p.protocolo != hyurax_wire::PROTOCOL_VERSION {
                return Err(NetError::Handshake(format!("protocolo {}", p.protocolo)));
            }
            if p.magic != self.magic {
                return Err(NetError::Handshake("magic de outra rede".into()));
            }
            Ok(())
        };

        // Papéis fixos: quem discou manda HELLO na hora; quem recebeu espera o
        // HELLO. Adivinhar o papel por tempo de espera fazia os dois lados, às
        // vezes, mandarem HELLO juntos (visto no Linux do GitHub Actions).
        match papel {
            Papel::Discou => {
                let meu = self.ponta_local();
                conexao.enviar(&Message::Hello(meu))?;
                match receber_com_prazo(conexao, PRAZO_APERTO)? {
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
            Papel::Recebeu => match receber_com_prazo(conexao, PRAZO_APERTO)? {
                Message::Hello(ponta) => {
                    confere(&ponta)?;
                    conexao.enviar(&Message::HelloAck { ponta: self.ponta_local(), eco: ponta.nonce })?;
                    Ok(ponta)
                }
                _ => Err(NetError::Handshake("esperava HELLO".into())),
            },
        }
    }

    fn ler_laco(self: &Arc<Self>, conexao: &mut Conexao, id: u64, saida: &Saida) -> Result<(), NetError> {
        // responder: o que não cabe na fila derruba o par (ele pede mais do que lê)
        let responder = |msg: &Message| -> Result<(), NetError> {
            let Some(q) = self.quadro(msg) else { return Ok(()) };
            match saida.mandar(q) {
                Ok(()) => Ok(()),
                Err(Recusa::Cheia) => Err(NetError::Handshake("par não lê o que pede: fila de saída cheia".into())),
                Err(Recusa::Fechada) => Err(NetError::Handshake("conexão fechada".into())),
            }
        };
        loop {
            if self.parando() {
                return Ok(());
            }
            let ate = Instant::now().checked_add(PRAZO_QUADRO);
            let msg = match conexao.receber_ate(ate) {
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
                    responder(&Message::Addrs(self.enderecos_para_anunciar()))?;
                    // e os nós que só se alcançam por ponte
                    if let Some(p) = self.pontes_para_anunciar() {
                        responder(&p)?;
                    }
                    continue;
                }
                Message::Desconhecida { tipo: crate::malha::TIPO_MALHA, corpo } => {
                    self.tratar_malha(id, &corpo);
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
                // O ULTRAX também é transporte: vai para quem registrou, fora do nó.
                Message::Desconhecida { tipo: TIPO_ULTRAX, corpo } => {
                    let tratador = self.ultrax.lock().ok().and_then(|t| t.clone());
                    let par = self.identidade_do_par.lock().ok().and_then(|m| m.get(&id).copied()).unwrap_or([0; 32]);
                    if let Some(t) = tratador {
                        t(id, par, &corpo);
                    }
                    continue;
                }
                // as outras extensões (a nuvem): também transporte
                Message::Desconhecida { tipo, corpo } if self.extensoes.lock().is_ok_and(|e| e.contains_key(&tipo)) => {
                    let tratador = self.extensoes.lock().ok().and_then(|e| e.get(&tipo).cloned());
                    let par = self.identidade_do_par.lock().ok().and_then(|m| m.get(&id).copied()).unwrap_or([0; 32]);
                    if let Some(t) = tratador {
                        t(id, par, &corpo);
                    }
                    continue;
                }
                _ => {}
            }

            // O Argon2id de um bloco novo é conferido aqui, sem a trava do nó:
            // enquanto ele roda, os outros pares continuam sendo atendidos. Um
            // bloco que já tenho não custa nada; um forjado derruba o par.
            let pow = match &msg {
                Message::Block(bloco) => {
                    let (params, ja_tenho) = {
                        let no = self.no.lock().map_err(|_| NetError::Handshake("nó travado".into()))?;
                        (no.chain.params, no.chain.altura_de(&bloco.block_hash()).is_some())
                    };
                    if ja_tenho {
                        None
                    } else {
                        let recibo = hyurax_chain::conferir_pow(&params, &bloco.header)
                            .map_err(|e| NetError::Handshake(format!("par malicioso: bloco recusado: {e}")))?;
                        Some(recibo)
                    }
                }
                _ => None,
            };
            let reacao = {
                let mut no = self.no.lock().map_err(|_| NetError::Handshake("nó travado".into()))?;
                no.tratar_com_pow(msg, pow)
            };
            match reacao {
                Ok(r) => {
                    for resposta in &r.respostas {
                        responder(resposta)?;
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

/// A faixa de um endereço público (IPv4 /24, IPv6 /48), para o teto por
/// faixa. Loopback, rede local e link-local não têm faixa (sem teto).
fn faixa(ip: IpAddr) -> Option<[u8; 7]> {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() {
                return None;
            }
            let [a, b, c, _] = v4.octets();
            Some([4, a, b, c, 0, 0, 0])
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return faixa(IpAddr::V4(v4));
            }
            let [o0, o1, o2, o3, o4, o5, ..] = v6.octets();
            // loopback, local única (fc00::/7) e link-local (fe80::/10)
            if v6.is_loopback() || v6.is_unspecified() || (o0 & 0xfe) == 0xfc || (o0 == 0xfe && (o1 & 0xc0) == 0x80) {
                return None;
            }
            Some([6, o0, o1, o2, o3, o4, o5])
        }
    }
}

/// Espera uma mensagem inteira por até `prazo`, tolerando os timeouts curtos
/// de leitura do socket. Um par que fica mudo, ou goteja, além do prazo é
/// derrubado.
fn receber_com_prazo(conexao: &mut Conexao, prazo: Duration) -> Result<Message, NetError> {
    let ate = Instant::now().checked_add(prazo);
    loop {
        match conexao.receber_ate(ate) {
            Err(NetError::Io(e))
                if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
                    && ate.is_some_and(|a| Instant::now() < a) => {}
            outro => return outro,
        }
    }
}

fn escrever_laco(mut escritor: Escritor, entrada: &Receiver<Vec<u8>>, rede: &Arc<Rede>, bytes: &AtomicUsize) {
    loop {
        match entrada.recv_timeout(TIMEOUT) {
            Ok(quadro) => {
                let falhou = escritor.enviar_quadro(&quadro).is_err();
                bytes.fetch_sub(quadro.len(), Ordering::SeqCst);
                if falhou {
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if rede.parando() {
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[cfg(test)]
mod testes {
    #[test]
    #[allow(clippy::unwrap_used)]
    fn faixa_agrupa_vizinhos_publicos_e_ignora_a_rede_local() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert_eq!(faixa(ip("200.10.20.1")), faixa(ip("200.10.20.254")));
        assert_ne!(faixa(ip("200.10.20.1")), faixa(ip("200.10.21.1")));
        assert_eq!(faixa(ip("192.168.0.5")), None);
        assert_eq!(faixa(ip("10.1.2.3")), None);
        assert_eq!(faixa(ip("127.0.0.1")), None);
        assert_eq!(faixa(ip("2001:db8:1:2::1")), faixa(ip("2001:db8:1:ffff::9")));
        assert_ne!(faixa(ip("2001:db8:1::1")), faixa(ip("2001:db8:2::1")));
        assert_eq!(faixa(ip("fe80::1")), None);
        assert_eq!(faixa(ip("::ffff:200.10.20.7")), faixa(ip("200.10.20.1")));
    }

    use super::*;

    #[test]
    fn so_endereco_util_entra_no_livro() {
        assert!(endereco_util("8.8.8.8:8790"));
        assert!(endereco_util("192.168.0.10:8790"));
        assert!(endereco_util("127.0.0.1:8790"));
        for ruim in ["0.0.0.0:8790", "8.8.8.8:0", "255.255.255.255:8790", "224.0.0.1:8790", "192.0.2.1:8790", "nome:8790", "lixo"] {
            assert!(!endereco_util(ruim), "{ruim}");
        }
    }
}
