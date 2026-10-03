//! A malha dentro do transporte (`docs/HYURAX-MALHA.md`): verificação de
//! alcance, pontes (relay) entre nós que não se alcançam e descoberta na rede
//! local. O formato das mensagens mora em `crate::malha`.
//!
//! **Alcance.** O nó pede a um par que tente abrir uma conexão de volta para
//! ele, no IP que o par vê, levando um token que só foi dito dentro da
//! conexão cifrada. Se o token chega, o nó é alcançável dali. Visto de um par
//! da internet, isso responde "alcançável de fora"; de um par da rede local,
//! só "alcançável na rede local".
//!
//! **Ponte.** Um nó que ninguém alcança (atrás de NAT, ou sem escutar) pede a
//! um par alcançável que guarde uma vaga: abre um socket de reserva até ele,
//! com um token dito dentro da cifra, e anuncia "me alcance por esta ponte".
//! Quem quer falar com ele disca para a ponte pedindo um circuito até a
//! identidade dele; a ponte junta os dois sockets e só copia bytes. Os dois
//! nós fazem o aperto Noise XX de ponta a ponta por dentro: a ponte não lê,
//! não altera e não se passa por nenhum dos dois.
//!
//! **Vizinhos.** Anúncio por UDP na rede local: dois nós no mesmo Wi-Fi se
//! acham sem internet e sem semente.
//!
//! Tetos: reservas, circuitos e bytes por circuito, para a ponte não virar
//! amplificador de ataque nem conta de luz de ninguém.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyurax_wire::Message;

use super::{Rede, VagaDeEntrada, agora_seg};
use crate::cifra::Papel;
use crate::malha::{AnuncioVizinho, MAX_PONTES, MensagemMalha, Prefixo, TIPO_MALHA, ip_local, ip_sem_saida};

/// Reservas guardadas por esta ponte, no total e por identidade.
const MAX_RESERVAS: usize = 64;
const RESERVAS_POR_NO: usize = 2;
/// Uma reserva sem uso fecha depois disto (o nó abre outra).
const RESERVA_DURA_S: u64 = 600;
/// Circuitos ao mesmo tempo, no total e até a mesma identidade.
const MAX_CIRCUITOS: usize = 16;
const CIRCUITOS_POR_ALVO: usize = 4;
/// Bytes por sentido de um circuito, e silêncio que derruba.
const BYTES_POR_CIRCUITO: u64 = 256 * 1024 * 1024;
const SILENCIO_DO_CIRCUITO: Duration = Duration::from_secs(300);
/// Pontes conhecidas (alvo -> ponte).
const MAX_PONTES_CONHECIDAS: usize = 1000;
/// Uma verificação de alcance por IP a cada 5 minutos (quem confere).
const VERIFICACAO_POR_IP_S: u64 = 300;
/// De quanto em quanto tempo o nó volta a conferir o próprio alcance.
const REVERIFICAR_S: u64 = 600;
/// Intervalo do anúncio na rede local.
const ANUNCIO_A_CADA: Duration = Duration::from_secs(10);
const PRAZO_CONTROLE: Duration = Duration::from_secs(5);

/// Token de reserva -> (dono, expira).
type TokensDeReserva = HashMap<[u8; 16], ([u8; 32], u64)>;
/// Dono -> sockets de reserva, com o instante em que chegaram.
type Reservas = HashMap<[u8; 32], Vec<(TcpStream, u64)>>;

/// Estado do alcance deste nó.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Estado {
    #[default]
    Desconhecido,
    /// Um par da internet conseguiu abrir conexão até aqui.
    DeFora,
    /// Só conferido por par da rede local.
    SoNaRedeLocal,
    /// Um par da internet tentou e não conseguiu.
    Nao,
}

#[derive(Default)]
pub(super) struct Alcance {
    estado: Estado,
    /// O endereço que um par da internet viu deste nó.
    externo: Option<SocketAddr>,
    /// Tokens pedidos e ainda não chegados: token -> (expira, pedido a par da internet?).
    tokens: HashMap<[u8; 16], u64>,
}

/// O estado da malha dentro da [`Rede`].
#[derive(Default)]
pub(super) struct Malha {
    alcance: Mutex<Alcance>,
    ultimo_pedido_alcance: AtomicU64,
    /// Quem confere: último pedido por IP.
    conferidos: Mutex<HashMap<IpAddr, u64>>,
    /// Alvo -> (ponte, visto em).
    pontes: Mutex<BTreeMap<[u8; 32], (SocketAddr, u64)>>,
    /// Alvo -> próxima tentativa por ponte.
    tentativas: Mutex<BTreeMap<[u8; 32], (u32, u64)>>,
    /// (ponte) token -> (identidade, expira).
    tokens_reserva: Mutex<TokensDeReserva>,
    /// (ponte) identidade -> sockets de reserva, com o instante.
    reservas: Mutex<Reservas>,
    circuitos: AtomicUsize,
    circuitos_por_alvo: Mutex<HashMap<[u8; 32], usize>>,
    /// (atrás de NAT) token pedido -> (ponte, expira).
    pedidos_reserva: Mutex<HashMap<[u8; 16], (SocketAddr, u64)>>,
    minhas_reservas: AtomicUsize,
    minhas_pontes: Mutex<BTreeSet<SocketAddr>>,
    /// Identidades vistas na rede local.
    vizinhos: Mutex<BTreeSet<[u8; 32]>>,
    /// O que aconteceu no roteador (UPnP/NAT-PMP), em texto.
    roteador: Mutex<Option<String>>,
    /// IP de cada par direto e o endereço discado (saídas diretas).
    pub(super) ip_do_par: Mutex<HashMap<u64, IpAddr>>,
    pub(super) discado: Mutex<HashMap<u64, SocketAddr>>,
}

/// O que a tela mostra da malha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstadoDaMalha {
    /// `desconhecido`, `de fora`, `só na rede local` ou `não`.
    pub alcance: &'static str,
    /// O endereço que um par da internet viu deste nó.
    pub externo: Option<String>,
    /// Nós vistos na rede local.
    pub vizinhos: usize,
    /// Rotas por ponte conhecidas.
    pub pontes_conhecidas: usize,
    /// Reservas que este nó mantém em pontes (quando ninguém o alcança).
    pub reservas_minhas: usize,
    /// Circuitos que este nó está repassando como ponte.
    pub circuitos: usize,
    /// O que aconteceu no roteador.
    pub roteador: Option<String>,
}

fn token_novo() -> [u8; 16] {
    let mut t = [0u8; 16];
    // sem entropia, o token zero é recusado adiante (nada é pedido)
    let _ = crate::entropia::preencher(&mut t);
    t
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn ler_exato_com_prazo(s: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<()> {
    s.set_read_timeout(Some(PRAZO_CONTROLE))?;
    s.read_exact(buf)
}

impl Rede {
    // ------------------------------------------------------------ estado

    /// O estado da malha, para a tela.
    pub fn estado_da_malha(&self) -> EstadoDaMalha {
        let (alcance, externo) = self.malha.alcance.lock().map_or(("desconhecido", None), |a| {
            let texto = match a.estado {
                Estado::Desconhecido => "desconhecido",
                Estado::DeFora => "de fora",
                Estado::SoNaRedeLocal => "só na rede local",
                Estado::Nao => "não",
            };
            (texto, a.externo.map(|e| e.to_string()))
        });
        EstadoDaMalha {
            alcance,
            externo,
            vizinhos: self.malha.vizinhos.lock().map_or(0, |v| v.len()),
            pontes_conhecidas: self.malha.pontes.lock().map_or(0, |p| p.len()),
            reservas_minhas: self.malha.minhas_reservas.load(Ordering::Relaxed),
            circuitos: self.malha.circuitos.load(Ordering::Relaxed),
            roteador: self.malha.roteador.lock().ok().and_then(|r| r.clone()),
        }
    }

    /// Anota o que aconteceu no roteador (quem abre a porta é o núcleo).
    pub fn anotar_roteador(&self, texto: String) {
        if let Ok(mut r) = self.malha.roteador.lock() {
            *r = Some(texto);
        }
    }

    fn precisa_de_ponte(&self) -> bool {
        self.escuta.load(Ordering::Relaxed) == 0 || self.malha.alcance.lock().is_ok_and(|a| a.estado == Estado::Nao)
    }

    fn enviar_malha(&self, id: u64, m: &MensagemMalha) -> bool {
        let Ok(corpo) = m.codificar() else { return false };
        let Some(quadro) = self.quadro(&Message::Desconhecida { tipo: TIPO_MALHA, corpo }) else { return false };
        self.pares.lock().ok().and_then(|p| p.get(&id).map(|s| s.mandar(quadro).is_ok())).unwrap_or(false)
    }

    fn difundir_malha(&self, m: &MensagemMalha) {
        if let Ok(corpo) = m.codificar() {
            self.difundir(u64::MAX, &Message::Desconhecida { tipo: TIPO_MALHA, corpo });
        }
    }

    /// As rotas por ponte para anunciar (as minhas primeiro).
    pub(super) fn pontes_para_anunciar(&self) -> Option<Message> {
        let eu = self.identidade.publica();
        let mut lista: Vec<([u8; 32], SocketAddr)> =
            self.malha.minhas_pontes.lock().map(|m| m.iter().map(|p| (eu, *p)).collect()).unwrap_or_default();
        if let Ok(p) = self.malha.pontes.lock() {
            let mut outras: Vec<_> = p.iter().map(|(a, (ponte, visto))| (*visto, *a, *ponte)).collect();
            outras.sort_by_key(|x| std::cmp::Reverse(x.0));
            lista.extend(outras.into_iter().map(|(_, a, ponte)| (a, ponte)));
        }
        lista.truncate(MAX_PONTES);
        if lista.is_empty() {
            return None;
        }
        let corpo = MensagemMalha::Pontes(lista).codificar().ok()?;
        Some(Message::Desconhecida { tipo: TIPO_MALHA, corpo })
    }

    // ------------------------------------------------- mensagens cifradas

    /// Uma mensagem da malha chegou do par `id`. Mensagem malformada é
    /// ignorada (é transporte opcional, como o ULTRAX).
    pub(super) fn tratar_malha(self: &Arc<Self>, id: u64, corpo: &[u8]) {
        let Ok(m) = MensagemMalha::decodificar(corpo) else { return };
        let agora = agora_seg();
        match m {
            MensagemMalha::PedirAlcance { porta, token } => {
                let Some(ip) = self.malha.ip_do_par.lock().ok().and_then(|m| m.get(&id).copied()) else { return };
                let liberado = self.malha.conferidos.lock().is_ok_and(|mut c| {
                    c.retain(|_, t| agora.saturating_sub(*t) < VERIFICACAO_POR_IP_S);
                    if c.contains_key(&ip) && !ip.is_loopback() {
                        false
                    } else {
                        c.insert(ip, agora);
                        true
                    }
                });
                if !liberado || porta == 0 {
                    return;
                }
                let rede = Arc::clone(self);
                std::thread::spawn(move || {
                    let alvo = SocketAddr::new(ip, porta);
                    let tentou = TcpStream::connect_timeout(&alvo, PRAZO_CONTROLE)
                        .and_then(|mut s| s.write_all(&Prefixo::Verificar(token).bytes()))
                        .is_ok();
                    rede.enviar_malha(id, &MensagemMalha::Alcance { endereco: alvo, tentou });
                });
            }
            MensagemMalha::Alcance { endereco, tentou } => {
                // quem conferiu da internet viu um IP público: o resultado vale
                // para "de fora"; da rede local, só para a rede local
                let de_fora = !ip_sem_saida(endereco.ip());
                if de_fora && let Ok(mut a) = self.malha.alcance.lock() {
                    a.externo = Some(endereco);
                }
                // o token pode chegar um pouco depois da resposta: dá 5 s.
                // Conferido por par da internet e o token não veio: não alcançável.
                if de_fora {
                    let rede = Arc::clone(self);
                    std::thread::spawn(move || {
                        std::thread::sleep(if tentou { Duration::from_secs(5) } else { Duration::ZERO });
                        if let Ok(mut a) = rede.malha.alcance.lock()
                            && a.estado != Estado::DeFora
                        {
                            a.estado = Estado::Nao;
                        }
                    });
                }
            }
            MensagemMalha::Reservar { token } => {
                let Some(dono) = self.identidade_do_par.lock().ok().and_then(|m| m.get(&id).copied()) else { return };
                let aceita = token != [0u8; 16] && self.vaga_de_reserva(&dono);
                if aceita && let Ok(mut t) = self.malha.tokens_reserva.lock() {
                    t.retain(|_, (_, expira)| *expira > agora);
                    t.insert(token, (dono, agora.saturating_add(60)));
                }
                self.enviar_malha(id, &MensagemMalha::Reserva { token, aceita });
            }
            MensagemMalha::Reserva { token, aceita } => {
                let ponte = self.malha.pedidos_reserva.lock().ok().and_then(|mut p| p.remove(&token)).map(|(ponte, _)| ponte);
                if let (true, Some(ponte)) = (aceita, ponte) {
                    let rede = Arc::clone(self);
                    std::thread::spawn(move || rede.manter_reserva(ponte, token));
                }
            }
            MensagemMalha::Pontes(lista) => {
                let eu = self.identidade.publica();
                if let Ok(mut p) = self.malha.pontes.lock() {
                    for (alvo, ponte) in lista {
                        if alvo == eu || ponte.port() == 0 {
                            continue;
                        }
                        if !p.contains_key(&alvo) && p.len() >= MAX_PONTES_CONHECIDAS {
                            let velho = p.iter().min_by_key(|(_, (_, v))| *v).map(|(a, _)| *a);
                            if let Some(v) = velho {
                                p.remove(&v);
                            }
                        }
                        p.insert(alvo, (ponte, agora));
                    }
                }
            }
        }
    }

    fn vaga_de_reserva(&self, dono: &[u8; 32]) -> bool {
        let pendentes = self.malha.tokens_reserva.lock().map_or(0, |t| t.values().filter(|(d, _)| d == dono).count());
        self.malha.reservas.lock().is_ok_and(|r| {
            let total: usize = r.values().map(Vec::len).sum();
            let deste = r.get(dono).map_or(0, Vec::len).saturating_add(pendentes);
            total < MAX_RESERVAS && deste < RESERVAS_POR_NO
        })
    }

    // ------------------------------------------- conexões novas (prefixo)

    /// Uma conexão de entrada: um prefixo da malha ("HX..."), ou o aperto de
    /// mão Noise de sempre.
    pub(super) fn atender_entrada(self: &Arc<Self>, mut stream: TcpStream, vaga: VagaDeEntrada) {
        let mut cab = [0u8; 4];
        let prefixo = stream.set_nonblocking(false).is_ok()
            && stream.set_read_timeout(Some(PRAZO_CONTROLE)).is_ok()
            && espiar(&stream, &mut cab)
            && Prefixo::tamanho_do_resto(&cab).is_some();
        if !prefixo {
            let _vaga = vaga;
            let _ = self.servir(stream, Papel::Recebeu, None, false, None);
            return;
        }
        let tamanho = Prefixo::tamanho_do_resto(&cab).unwrap_or(0).saturating_add(4);
        let mut bruto = vec![0u8; tamanho];
        if ler_exato_com_prazo(&mut stream, &mut bruto).is_err() {
            return;
        }
        match Prefixo::ler(&bruto) {
            Ok(Prefixo::Verificar(token)) => self.verificacao_chegou(&token, stream.peer_addr().ok().map(|s| s.ip())),
            Ok(Prefixo::Reserva(token)) => {
                // a reserva não ocupa vaga de entrada: tem teto próprio
                drop(vaga);
                self.guardar_reserva(token, stream);
            }
            Ok(Prefixo::Circuito(alvo)) => {
                let _vaga = vaga;
                self.abrir_circuito(alvo, stream);
            }
            Err(_) => {}
        }
    }

    fn verificacao_chegou(&self, token: &[u8; 16], de: Option<IpAddr>) {
        if let Ok(mut a) = self.malha.alcance.lock()
            && a.tokens.remove(token).is_some()
        {
            let de_fora = de.is_some_and(|ip| !ip_sem_saida(ip));
            if de_fora {
                a.estado = Estado::DeFora;
            } else if a.estado != Estado::DeFora {
                a.estado = Estado::SoNaRedeLocal;
            }
        }
    }

    fn guardar_reserva(&self, token: [u8; 16], stream: TcpStream) {
        let agora = agora_seg();
        let dono = self.malha.tokens_reserva.lock().ok().and_then(|mut t| t.remove(&token)).filter(|(_, expira)| *expira > agora);
        let Some((dono, _)) = dono else { return };
        if let Ok(mut r) = self.malha.reservas.lock() {
            let total: usize = r.values().map(Vec::len).sum();
            let deste = r.entry(dono).or_default();
            if total < MAX_RESERVAS && deste.len() < RESERVAS_POR_NO {
                deste.push((stream, agora));
            }
        }
    }

    fn abrir_circuito(&self, alvo: [u8; 32], mut a: TcpStream) {
        let lotado = self.malha.circuitos.load(Ordering::Relaxed) >= MAX_CIRCUITOS
            || self.malha.circuitos_por_alvo.lock().is_ok_and(|c| c.get(&alvo).copied().unwrap_or(0) >= CIRCUITOS_POR_ALVO);
        let mut b = None;
        if !lotado {
            while let Some(mut candidato) = self.malha.reservas.lock().ok().and_then(|mut r| r.get_mut(&alvo).and_then(Vec::pop)).map(|(s, _)| s) {
                // a reserva pode ter morrido: o primeiro byte diz se ainda vive
                if candidato.set_write_timeout(Some(PRAZO_CONTROLE)).is_ok() && candidato.write_all(&[1]).is_ok() {
                    b = Some(candidato);
                    break;
                }
            }
        }
        let Some(b) = b else {
            let _ = a.write_all(&[0]);
            return;
        };
        if a.write_all(&[1]).is_err() {
            let _ = b.shutdown(Shutdown::Both);
            return;
        }
        self.malha.circuitos.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut c) = self.malha.circuitos_por_alvo.lock() {
            let n = c.entry(alvo).or_insert(0);
            *n = n.saturating_add(1);
        }
        espelhar(a, b);
        self.malha.circuitos.fetch_sub(1, Ordering::Relaxed);
        if let Ok(mut c) = self.malha.circuitos_por_alvo.lock()
            && let Some(n) = c.get_mut(&alvo)
        {
            *n = n.saturating_sub(1);
            if *n == 0 {
                c.remove(&alvo);
            }
        }
    }

    // -------------------------------------------------- lado sem alcance

    /// Mantém um socket de reserva aberto numa ponte até um circuito chegar
    /// (aí ele vira a conexão com quem discou) ou a ponte fechar.
    fn manter_reserva(self: &Arc<Self>, ponte: SocketAddr, token: [u8; 16]) {
        let Ok(mut s) = TcpStream::connect_timeout(&ponte, PRAZO_CONTROLE) else { return };
        if s.write_all(&Prefixo::Reserva(token).bytes()).is_err() || s.set_read_timeout(Some(Duration::from_secs(1))).is_err() {
            return;
        }
        self.malha.minhas_reservas.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut m) = self.malha.minhas_pontes.lock() {
            m.insert(ponte);
        }
        // "me alcance por esta ponte"
        self.difundir_malha(&MensagemMalha::Pontes(vec![(self.identidade.publica(), ponte)]));
        let mut sinal = [0u8; 1];
        let chegou = loop {
            if self.parando() {
                break false;
            }
            match s.read(&mut sinal) {
                Ok(1) => break sinal == [1],
                Ok(_) => break false,
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => break false,
            }
        };
        self.malha.minhas_reservas.fetch_sub(1, Ordering::Relaxed);
        if let Ok(mut m) = self.malha.minhas_pontes.lock() {
            m.remove(&ponte);
        }
        if chegou {
            let _ = self.servir(s, Papel::Recebeu, None, true, None);
        }
    }

    /// Disca para `alvo` através de `ponte`.
    ///
    /// # Errors
    /// Ponte fora do ar, ou sem reserva para o alvo.
    pub fn conectar_por_ponte(self: &Arc<Self>, ponte: SocketAddr, alvo: [u8; 32]) -> std::io::Result<()> {
        let mut s = TcpStream::connect_timeout(&ponte, PRAZO_CONTROLE)?;
        s.write_all(&Prefixo::Circuito(alvo).bytes())?;
        let mut r = [0u8; 1];
        ler_exato_com_prazo(&mut s, &mut r)?;
        if r != [1] {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "a ponte não tem reserva para esse nó"));
        }
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = rede.servir(s, Papel::Discou, Some(format!("ponte:{}", hex(&alvo))), true, Some(alvo));
        });
        Ok(())
    }

    // ----------------------------------------------------- manutenção

    /// Uma volta da manutenção da malha (a cada 2 s, junto com a da rede).
    pub(super) fn cuidar_da_malha(self: &Arc<Self>) {
        let agora = agora_seg();
        self.limpar_reservas_velhas(agora);
        let pares: Vec<u64> = self.malha.ip_do_par.lock().map(|m| m.keys().copied().collect()).unwrap_or_default();
        if pares.is_empty() {
            return;
        }
        // 1. alcance: pede a um par que tente voltar até aqui
        let porta = self.escuta.load(Ordering::Relaxed);
        let estado = self.malha.alcance.lock().map_or(Estado::Desconhecido, |a| a.estado);
        let espera = if estado == Estado::Desconhecido { 20 } else { REVERIFICAR_S };
        if porta != 0
            && estado != Estado::DeFora
            && agora.saturating_sub(self.malha.ultimo_pedido_alcance.load(Ordering::Relaxed)) >= espera
        {
            self.malha.ultimo_pedido_alcance.store(agora, Ordering::Relaxed);
            // de preferência um par da internet (o resultado vale para "de fora")
            let ips = self.malha.ip_do_par.lock().map(|m| m.clone()).unwrap_or_default();
            let escolhido = pares.iter().copied().max_by_key(|id| ips.get(id).is_some_and(|ip| !ip_sem_saida(*ip)));
            let token = token_novo();
            if let (Some(id), true) = (escolhido, token != [0u8; 16]) {
                if let Ok(mut a) = self.malha.alcance.lock() {
                    a.tokens.retain(|_, expira| *expira > agora);
                    a.tokens.insert(token, agora.saturating_add(60));
                }
                self.enviar_malha(id, &MensagemMalha::PedirAlcance { porta, token });
            }
        }
        // 2. sem alcance: mantém uma reserva numa ponte alcançável (pedido
        // sem resposta em 30 s não segura a próxima tentativa)
        let pedidos = self.malha.pedidos_reserva.lock().map_or(0, |mut p| {
            p.retain(|_, (_, expira)| *expira > agora);
            p.len()
        });
        if self.precisa_de_ponte() && self.malha.minhas_reservas.load(Ordering::Relaxed) == 0 && pedidos == 0 {
            let discados = self.malha.discado.lock().map(|d| d.clone()).unwrap_or_default();
            if let Some((id, ponte)) = discados.into_iter().next() {
                let token = token_novo();
                if token != [0u8; 16] {
                    if let Ok(mut p) = self.malha.pedidos_reserva.lock() {
                        p.insert(token, (ponte, agora.saturating_add(30)));
                    }
                    if !self.enviar_malha(id, &MensagemMalha::Reservar { token })
                        && let Ok(mut p) = self.malha.pedidos_reserva.lock()
                    {
                        p.remove(&token);
                    }
                }
            }
        }
        // 3. pares só alcançáveis por ponte: um por volta
        if self.saidas.load(Ordering::Relaxed) < super::ALVO_SAIDAS {
            self.discar_um_por_ponte(agora);
        }
    }

    fn discar_um_por_ponte(self: &Arc<Self>, agora: u64) {
        let conectados = self.identidades_conectadas.lock().map(|c| c.keys().copied().collect::<BTreeSet<_>>()).unwrap_or_default();
        let eu = self.identidade.publica();
        let tentativas = self.malha.tentativas.lock().map(|t| t.clone()).unwrap_or_default();
        let candidato = self.malha.pontes.lock().ok().and_then(|p| {
            p.iter()
                .filter(|(alvo, _)| **alvo != eu && !conectados.contains(*alvo) && !self.identidade_banida(alvo))
                .filter(|(alvo, _)| tentativas.get(*alvo).is_none_or(|(_, proxima)| *proxima <= agora))
                .map(|(a, (ponte, _))| (*a, *ponte))
                .next()
        });
        let Some((alvo, ponte)) = candidato else { return };
        let ok = self.conectar_por_ponte(ponte, alvo).is_ok();
        if let Ok(mut t) = self.malha.tentativas.lock() {
            let e = t.entry(alvo).or_insert((0, 0));
            e.0 = if ok { 0 } else { e.0.saturating_add(1) };
            e.1 = agora.saturating_add(30u64.saturating_mul(1u64 << e.0.min(7)));
            if e.0 >= 8 {
                t.remove(&alvo);
                if let Ok(mut p) = self.malha.pontes.lock() {
                    p.remove(&alvo);
                }
            }
        }
    }

    fn limpar_reservas_velhas(&self, agora: u64) {
        if let Ok(mut r) = self.malha.reservas.lock() {
            for lista in r.values_mut() {
                lista.retain(|(s, criada)| {
                    let viva = agora.saturating_sub(*criada) < RESERVA_DURA_S;
                    if !viva {
                        let _ = s.shutdown(Shutdown::Both);
                    }
                    viva
                });
            }
            r.retain(|_, l| !l.is_empty());
        }
    }

    // ----------------------------------------------------- rede local

    /// Liga a descoberta na rede local: anuncia este nó por UDP em
    /// `destinos` (na prática, a difusão 255.255.255.255) e ouve os anúncios
    /// na `porta_udp`. Se a porta já está em uso (outro nó nesta máquina),
    /// só anuncia.
    ///
    /// # Errors
    /// Sem socket UDP nenhum.
    pub fn ligar_vizinhos(self: &Arc<Self>, porta_udp: u16, destinos: Vec<SocketAddr>) -> std::io::Result<()> {
        let ouvinte = UdpSocket::bind(("0.0.0.0", porta_udp)).ok();
        let falante = match &ouvinte {
            Some(o) => o.try_clone()?,
            None => UdpSocket::bind(("0.0.0.0", 0))?,
        };
        falante.set_broadcast(true)?;
        let rede = Arc::clone(self);
        std::thread::spawn(move || {
            while !rede.parando() {
                let porta = rede.escuta.load(Ordering::Relaxed);
                if porta != 0 {
                    let a = AnuncioVizinho { magic: rede.magic, porta, identidade: rede.identidade.publica() };
                    for d in &destinos {
                        let _ = falante.send_to(&a.bytes(), d);
                    }
                }
                std::thread::sleep(ANUNCIO_A_CADA);
            }
        });
        if let Some(o) = ouvinte {
            o.set_read_timeout(Some(Duration::from_secs(1)))?;
            let rede = Arc::clone(self);
            std::thread::spawn(move || rede.ouvir_vizinhos(&o));
        }
        Ok(())
    }

    fn ouvir_vizinhos(self: &Arc<Self>, o: &UdpSocket) {
        let mut buf = [0u8; 128];
        let mut ultimo: HashMap<IpAddr, Instant> = HashMap::new();
        while !self.parando() {
            let Ok((n, de)) = o.recv_from(&mut buf) else { continue };
            // só da rede local, e um anúncio por segundo por IP
            if !ip_local(de.ip()) || ultimo.get(&de.ip()).is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
                continue;
            }
            ultimo.insert(de.ip(), Instant::now());
            if ultimo.len() > 1024 {
                ultimo.clear();
            }
            let Ok(a) = AnuncioVizinho::ler(buf.get(..n).unwrap_or_default()) else { continue };
            if a.magic != self.magic || a.identidade == self.identidade.publica() {
                continue;
            }
            // o IP é o de origem do datagrama, nunca um escrito dentro dele
            let addr = SocketAddr::new(de.ip(), a.porta).to_string();
            let novo = self.malha.vizinhos.lock().is_ok_and(|mut v| v.len() < 256 && v.insert(a.identidade));
            self.aprender(addr.clone());
            if novo && let Ok(mut d) = self.discagens.lock() {
                // vizinho novo: pode discar já, sem esperar a espera crescente
                d.remove(&addr);
            }
        }
    }
}

/// Espia os primeiros 4 bytes sem tirá-los do socket.
fn espiar(s: &TcpStream, cab: &mut [u8; 4]) -> bool {
    let ate = Instant::now().checked_add(PRAZO_CONTROLE).unwrap_or_else(Instant::now);
    while Instant::now() < ate {
        match s.peek(cab) {
            Ok(4) => return true,
            Ok(0) => return false,
            Ok(_) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => return false,
        }
    }
    false
}

/// Copia bytes nos dois sentidos até um lado fechar, ficar mudo demais ou
/// passar do teto. Não interpreta nada: o que passa é Noise de ponta a ponta.
fn espelhar(a: TcpStream, b: TcpStream) {
    let copiar = |mut de: TcpStream, mut para: TcpStream| {
        let _ = de.set_read_timeout(Some(SILENCIO_DO_CIRCUITO));
        let _ = para.set_write_timeout(Some(Duration::from_secs(30)));
        let mut buf = [0u8; 16 * 1024];
        let mut total = 0u64;
        loop {
            match de.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    total = total.saturating_add(n as u64);
                    if total > BYTES_POR_CIRCUITO || para.write_all(buf.get(..n).unwrap_or_default()).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = de.shutdown(Shutdown::Both);
        let _ = para.shutdown(Shutdown::Both);
    };
    let (Ok(a2), Ok(b2)) = (a.try_clone(), b.try_clone()) else { return };
    let volta = std::thread::spawn(move || copiar(b2, a2));
    copiar(a, b);
    let _ = volta.join();
}
