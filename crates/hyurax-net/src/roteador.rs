// ✝ Isaías 22:22 — “Abrirá, e ninguém fechará; fechará, e ninguém abrirá.”
//! Abrir a porta do nó no roteador de casa, sozinho (`docs/HYURAX-MALHA.md`,
//! etapa M2), para o nó virar ponto de entrada da rede sem o dono mexer em
//! nada.
//!
//! Dois protocolos que os roteadores domésticos falam, escritos aqui com a
//! biblioteca padrão (sem dependência nova):
//!
//! - **UPnP IGD**: procura o roteador por SSDP (UDP multicast
//!   239.255.255.250:1900), lê a descrição dele por HTTP e pede o mapeamento
//!   com SOAP (`AddPortMapping`), além do IP externo (`GetExternalIPAddress`).
//! - **NAT-PMP** (RFC 6886): dois pacotes UDP para a porta 5351 do roteador.
//!
//! O IP externo que o roteador diz ter revela o **CGNAT**: se ele mesmo está
//! atrás da operadora (100.64.0.0/10 ou rede privada), abrir a porta nele não
//! adianta, e a tela diz isso em vez de fingir que deu certo.
//!
//! Tudo aqui é melhor esforço: roteador sem UPnP, ou com UPnP desligado, só
//! devolve erro, e o nó segue como antes.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use crate::malha::ip_sem_saida;

const SSDP: &str = "239.255.255.250:1900";
const PRAZO: Duration = Duration::from_secs(3);
/// Teto do que se lê de uma resposta HTTP do roteador.
const RESPOSTA_MAX: usize = 256 * 1024;
/// Quanto dura o mapeamento pedido; o nó renova antes.
pub const DURACAO_S: u32 = 3600;

/// O mapeamento conseguido.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapeamento {
    /// `UPnP` ou `NAT-PMP`.
    pub metodo: &'static str,
    /// O IP que o roteador diz ter do lado de fora, quando disse.
    pub ip_externo: Option<IpAddr>,
    /// A porta aberta do lado de fora.
    pub porta_externa: u16,
    /// O roteador também está atrás da operadora: a porta aberta nele não
    /// chega à internet.
    pub cgnat: bool,
    /// Para renovar ou desfazer (UPnP): URL de controle e tipo do serviço.
    pub controle: Option<(String, String)>,
}

/// Tenta abrir `porta` (TCP) no roteador: UPnP primeiro, depois NAT-PMP.
///
/// # Errors
/// Nenhum roteador respondeu, ou ele recusou.
pub fn abrir(porta: u16) -> Result<Mapeamento, String> {
    let upnp = descobrir_upnp().and_then(|loc| mapear_upnp(&loc, porta));
    match upnp {
        Ok(m) => Ok(m),
        Err(e_upnp) => {
            let gateway = gateway_provavel().ok_or_else(|| format!("UPnP: {e_upnp}; NAT-PMP: sem gateway"))?;
            mapear_natpmp(gateway, porta).map_err(|e| format!("UPnP: {e_upnp}; NAT-PMP: {e}"))
        }
    }
}

/// Desfaz o mapeamento (melhor esforço, ao fechar).
pub fn fechar(m: &Mapeamento) {
    if let Some((controle, servico)) = &m.controle {
        let corpo = format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{}</NewExternalPort><NewProtocol>TCP</NewProtocol>",
            m.porta_externa
        );
        let _ = soap(controle, servico, "DeletePortMapping", &corpo);
    } else if let Some(g) = gateway_provavel() {
        // NAT-PMP: duração 0 apaga
        let _ = natpmp_mapear(g, m.porta_externa, 0);
    }
}

// ------------------------------------------------------------------ UPnP

/// Procura o roteador por SSDP. Devolve o endereço da descrição (`LOCATION`).
///
/// # Errors
/// Ninguém respondeu no prazo.
pub fn descobrir_upnp() -> Result<String, String> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_millis(500))).map_err(|e| e.to_string())?;
    for st in ["urn:schemas-upnp-org:device:InternetGatewayDevice:1", "urn:schemas-upnp-org:service:WANIPConnection:1"] {
        let pedido = format!("M-SEARCH * HTTP/1.1\r\nHOST: {SSDP}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {st}\r\n\r\n");
        let _ = s.send_to(pedido.as_bytes(), SSDP);
    }
    let ate = Instant::now().checked_add(PRAZO).unwrap_or_else(Instant::now);
    let mut buf = [0u8; 2048];
    while Instant::now() < ate {
        if let Ok((n, _)) = s.recv_from(&mut buf)
            && let Some(loc) = location_de(&String::from_utf8_lossy(buf.get(..n).unwrap_or_default()))
        {
            return Ok(loc);
        }
    }
    Err("nenhum roteador respondeu ao UPnP (desligado no roteador, ou sem UPnP)".into())
}

/// O cabeçalho `LOCATION` de uma resposta SSDP.
pub fn location_de(resposta: &str) -> Option<String> {
    resposta.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case("location").then(|| v.trim().to_string())
    })
}

/// `http://host:porta/caminho` em (host:porta, caminho).
fn partes_da_url(url: &str) -> Option<(String, String)> {
    let resto = url.strip_prefix("http://")?;
    let (hostporta, caminho) = resto.split_once('/').map_or((resto, "/".to_string()), |(h, c)| (h, format!("/{c}")));
    let hostporta = if hostporta.contains(':') { hostporta.to_string() } else { format!("{hostporta}:80") };
    Some((hostporta, caminho))
}

fn http(metodo: &str, url: &str, cabecalhos: &[(&str, &str)], corpo: &str) -> Result<(u16, String), String> {
    let (hostporta, caminho) = partes_da_url(url).ok_or("URL do roteador inválida")?;
    let endereco = hostporta.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("sem endereço")?;
    // só fala HTTP com a rede local: o roteador nunca está na internet
    if !crate::malha::ip_local(endereco.ip()) {
        return Err("o roteador anunciou um endereço fora da rede local".into());
    }
    let mut s = TcpStream::connect_timeout(&endereco, PRAZO).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(PRAZO)).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(PRAZO)).map_err(|e| e.to_string())?;
    let mut pedido = format!("{metodo} {caminho} HTTP/1.1\r\nHost: {hostporta}\r\nConnection: close\r\nContent-Length: {}\r\n", corpo.len());
    for (k, v) in cabecalhos {
        pedido.push_str(&format!("{k}: {v}\r\n"));
    }
    pedido.push_str("\r\n");
    pedido.push_str(corpo);
    s.write_all(pedido.as_bytes()).map_err(|e| e.to_string())?;
    let mut bruto = Vec::new();
    let mut pedaco = [0u8; 4096];
    loop {
        match s.read(&mut pedaco) {
            Ok(0) => break,
            Ok(n) => {
                bruto.extend_from_slice(pedaco.get(..n).unwrap_or_default());
                if bruto.len() > RESPOSTA_MAX {
                    return Err("resposta do roteador grande demais".into());
                }
            }
            Err(e) if bruto.is_empty() => return Err(e.to_string()),
            Err(_) => break,
        }
    }
    let texto = String::from_utf8_lossy(&bruto).into_owned();
    let status = texto.split(' ').nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
    let corpo = texto.split_once("\r\n\r\n").map(|(_, c)| c.to_string()).unwrap_or_default();
    Ok((status, corpo))
}

fn entre<'a>(texto: &'a str, abre: &str, fecha: &str) -> Option<&'a str> {
    let i = texto.find(abre)?.checked_add(abre.len())?;
    let resto = texto.get(i..)?;
    let f = resto.find(fecha)?;
    resto.get(..f)
}

/// Na descrição do roteador, o serviço de conexão WAN e a URL de controle.
pub fn servico_wan(descricao: &str, location: &str) -> Option<(String, String)> {
    for servico in descricao.split("<service>").skip(1) {
        let tipo = entre(servico, "<serviceType>", "</serviceType>")?.trim();
        if tipo.contains("WANIPConnection") || tipo.contains("WANPPPConnection") {
            let controle = entre(servico, "<controlURL>", "</controlURL>")?.trim();
            let url = if controle.starts_with("http://") {
                controle.to_string()
            } else {
                let (hostporta, _) = partes_da_url(location)?;
                let barra = if controle.starts_with('/') { "" } else { "/" };
                format!("http://{hostporta}{barra}{controle}")
            };
            return Some((url, tipo.to_string()));
        }
    }
    None
}

fn soap(controle: &str, servico: &str, acao: &str, argumentos: &str) -> Result<String, String> {
    let corpo = format!(
        "<?xml version=\"1.0\"?>\r\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{acao} xmlns:u=\"{servico}\">{argumentos}</u:{acao}></s:Body></s:Envelope>"
    );
    let acao_soap = format!("\"{servico}#{acao}\"");
    let (status, resposta) = http(
        "POST",
        controle,
        &[("Content-Type", "text/xml; charset=\"utf-8\""), ("SOAPAction", acao_soap.as_str())],
        &corpo,
    )?;
    if status == 200 {
        Ok(resposta)
    } else {
        let motivo = entre(&resposta, "<errorDescription>", "</errorDescription>").unwrap_or("sem descrição");
        Err(format!("o roteador recusou {acao} ({status}: {motivo})"))
    }
}

/// O IP desta máquina na rede do roteador (por onde sairia um pacote a ele).
fn ip_local_para(destino: SocketAddr) -> Option<IpAddr> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(destino).ok()?;
    Some(s.local_addr().ok()?.ip())
}

/// Pede o mapeamento por UPnP, a partir da descrição em `location`.
///
/// # Errors
/// Descrição sem serviço WAN, ou o roteador recusou.
pub fn mapear_upnp(location: &str, porta: u16) -> Result<Mapeamento, String> {
    let (status, descricao) = http("GET", location, &[], "")?;
    if status != 200 {
        return Err(format!("descrição do roteador respondeu {status}"));
    }
    let (controle, servico) = servico_wan(&descricao, location).ok_or("o roteador não tem serviço de conexão WAN no UPnP")?;
    let (hostporta, _) = partes_da_url(&controle).ok_or("URL de controle inválida")?;
    let roteador = hostporta.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("sem endereço")?;
    let local = ip_local_para(roteador).ok_or("não sei o IP desta máquina na rede do roteador")?;
    let argumentos = format!(
        "<NewRemoteHost></NewRemoteHost><NewExternalPort>{porta}</NewExternalPort><NewProtocol>TCP</NewProtocol>\
         <NewInternalPort>{porta}</NewInternalPort><NewInternalClient>{local}</NewInternalClient><NewEnabled>1</NewEnabled>\
         <NewPortMappingDescription>Hyurax</NewPortMappingDescription><NewLeaseDuration>{DURACAO_S}</NewLeaseDuration>"
    );
    soap(&controle, &servico, "AddPortMapping", &argumentos)?;
    let ip_externo = soap(&controle, &servico, "GetExternalIPAddress", "")
        .ok()
        .and_then(|r| entre(&r, "<NewExternalIPAddress>", "</NewExternalIPAddress>").and_then(|ip| ip.trim().parse().ok()));
    Ok(Mapeamento {
        metodo: "UPnP",
        ip_externo,
        porta_externa: porta,
        cgnat: ip_externo.is_some_and(ip_sem_saida),
        controle: Some((controle, servico)),
    })
}

// --------------------------------------------------------------- NAT-PMP

/// O gateway mais provável: o `.1` da rede desta máquina (NAT-PMP não tem
/// descoberta; o UPnP, quando responde, já resolveu antes).
fn gateway_provavel() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    s.connect(("192.0.2.1", 9)).ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if ip.is_private() => {
            let [a, b, c, _] = ip.octets();
            Some(Ipv4Addr::new(a, b, c, 1))
        }
        _ => None,
    }
}

fn natpmp_pedir(gateway: Ipv4Addr, pedido: &[u8], opcode_resposta: u8) -> Result<Vec<u8>, String> {
    let s = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_millis(750))).map_err(|e| e.to_string())?;
    let destino = SocketAddr::new(IpAddr::V4(gateway), 5351);
    let mut buf = [0u8; 64];
    for _ in 0..3 {
        s.send_to(pedido, destino).map_err(|e| e.to_string())?;
        if let Ok((n, de)) = s.recv_from(&mut buf)
            && de == destino
        {
            let r = buf.get(..n).unwrap_or_default().to_vec();
            if r.get(1) != Some(&opcode_resposta) {
                continue;
            }
            let resultado = r.get(2..4).map(|b| u16::from_be_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]));
            return match resultado {
                Some(0) => Ok(r),
                Some(c) => Err(format!("o roteador recusou (código NAT-PMP {c})")),
                None => Err("resposta NAT-PMP curta".into()),
            };
        }
    }
    Err("o roteador não respondeu ao NAT-PMP".into())
}

fn natpmp_mapear(gateway: Ipv4Addr, porta: u16, duracao: u32) -> Result<u16, String> {
    let mut pedido = vec![0u8, 2, 0, 0];
    pedido.extend_from_slice(&porta.to_be_bytes());
    pedido.extend_from_slice(&porta.to_be_bytes());
    pedido.extend_from_slice(&duracao.to_be_bytes());
    let r = natpmp_pedir(gateway, &pedido, 130)?;
    let externa = r.get(10..12).map(|b| u16::from_be_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0)]));
    externa.ok_or_else(|| "resposta NAT-PMP curta".into())
}

/// Pede o mapeamento por NAT-PMP.
///
/// # Errors
/// O roteador não fala NAT-PMP, ou recusou.
pub fn mapear_natpmp(gateway: Ipv4Addr, porta: u16) -> Result<Mapeamento, String> {
    let externo = natpmp_pedir(gateway, &[0, 0], 128).ok().and_then(|r| {
        let o = r.get(8..12)?;
        Some(IpAddr::V4(Ipv4Addr::new(*o.first()?, *o.get(1)?, *o.get(2)?, *o.get(3)?)))
    });
    let porta_externa = natpmp_mapear(gateway, porta, DURACAO_S)?;
    Ok(Mapeamento { metodo: "NAT-PMP", ip_externo: externo, porta_externa, cgnat: externo.is_some_and(ip_sem_saida), controle: None })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn le_ssdp_e_descricao() {
        let ssdp = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nlocation: http://192.168.0.1:1900/igd.xml\r\nST: x\r\n\r\n";
        assert_eq!(location_de(ssdp).as_deref(), Some("http://192.168.0.1:1900/igd.xml"));
        let xml = "<root><service><serviceType>urn:x:Layer3</serviceType><controlURL>/l3</controlURL></service>\
                   <service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl/IPConn</controlURL></service></root>";
        let (url, tipo) = servico_wan(xml, "http://192.168.0.1:1900/igd.xml").unwrap();
        assert_eq!(url, "http://192.168.0.1:1900/ctl/IPConn");
        assert!(tipo.ends_with("WANIPConnection:1"));
    }

    /// Um roteador UPnP de mentira em 127.0.0.1: serve a descrição e aceita o
    /// mapeamento, dizendo um IP externo de CGNAT.
    #[test]
    fn mapeia_num_roteador_upnp_e_percebe_o_cgnat() {
        let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = ouvinte.local_addr().unwrap().port();
        let pedidos = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let p2 = std::sync::Arc::clone(&pedidos);
        std::thread::spawn(move || {
            for c in ouvinte.incoming().take(3) {
                let mut c = c.unwrap();
                c.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
                let mut buf = vec![0u8; 8192];
                let mut lido = 0;
                while let Ok(n) = c.read(&mut buf[lido..]) {
                    if n == 0 {
                        break;
                    }
                    lido += n;
                }
                let pedido = String::from_utf8_lossy(&buf[..lido]).into_owned();
                let corpo = if pedido.starts_with("GET") {
                    "<root><service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl</controlURL></service></root>".to_string()
                } else if pedido.contains("GetExternalIPAddress") {
                    "<s:Envelope><s:Body><NewExternalIPAddress>100.70.1.2</NewExternalIPAddress></s:Body></s:Envelope>".to_string()
                } else {
                    "<s:Envelope></s:Envelope>".to_string()
                };
                p2.lock().unwrap().push(pedido);
                let _ = c.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{corpo}", corpo.len()).as_bytes());
            }
        });
        let m = mapear_upnp(&format!("http://127.0.0.1:{porta}/igd.xml"), 8790).unwrap();
        assert_eq!(m.metodo, "UPnP");
        assert_eq!(m.ip_externo, Some("100.70.1.2".parse().unwrap()));
        assert!(m.cgnat, "100.70.x.x é CGNAT: a porta aberta no roteador não chega à internet");
        let todos = pedidos.lock().unwrap().join("\n");
        assert!(todos.contains("AddPortMapping") && todos.contains("<NewExternalPort>8790</NewExternalPort>"));
    }

    #[test]
    fn roteador_fora_da_rede_local_e_recusado() {
        assert!(http("GET", "http://8.8.8.8:80/x", &[], "").unwrap_err().contains("fora da rede local"));
    }
}
