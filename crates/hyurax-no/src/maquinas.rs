//! Somar as suas outras máquinas no mesmo painel.
//!
//! Cada Hyurax já publica o próprio estado em `/api/estado`. Quando o dono liga
//! "ver no celular" numa máquina, esse endereço responde para o resto da rede
//! local — só leitura. Aqui o painel usa isso: você lista as suas outras
//! máquinas por `IP:PORTA` e este programa pergunta o estado delas de tempo em
//! tempo, para mostrar o total (ritmo, núcleos, energia) num lugar só.
//!
//! O que isto **não** é: controle remoto. Este programa só lê. Ligar e desligar
//! a mineração continua sendo no computador que minera, porque comando só vale
//! de dentro dele.
//!
//! É também a base do sistema de planos: quando existir máquina alugada da rede,
//! ela entra nesta mesma lista e soma no mesmo total.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Quantas máquinas a lista aceita.
pub(crate) const MAXIMO: usize = 8;
/// Teto do que o programa lê de uma resposta: o estado tem alguns KiB.
const RESPOSTA_MAXIMA: usize = 512 * 1024;

/// O que se sabe de uma máquina da lista, na última olhada.
pub(crate) struct Vista {
    pub alvo: String,
    /// Quando esta leitura foi feita (unix). Zero: nunca respondeu ainda.
    pub quando: u64,
    pub ok: bool,
    pub erro: String,
    pub minerando: bool,
    pub ritmo: f64,
    pub linhas: u32,
    pub nucleos: u32,
    pub altura: u64,
    pub watts: f64,
    pub tentativas: u64,
    pub versao: String,
    pub endereco: String,
    pub saldo: String,
    pub rede: String,
}

impl Vista {
    fn caiu(alvo: &str, quando: u64, erro: String) -> Self {
        Self {
            alvo: alvo.to_string(),
            quando,
            ok: false,
            erro,
            minerando: false,
            ritmo: 0.0,
            linhas: 0,
            nucleos: 0,
            altura: 0,
            watts: 0.0,
            tentativas: 0,
            versao: String::new(),
            endereco: String::new(),
            saldo: String::new(),
            rede: String::new(),
        }
    }
}

/// O valor de um campo de primeiro nível, como texto cru.
fn cru<'a>(json: &'a str, campo: &str) -> Option<&'a str> {
    let depois = json.split(&format!("\"{campo}\":")).nth(1)?;
    let fim = depois.find([',', '}']).unwrap_or(depois.len());
    depois.get(..fim).map(str::trim)
}

fn numero(json: &str, campo: &str) -> Option<f64> {
    cru(json, campo)?.parse().ok()
}

fn inteiro(json: &str, campo: &str) -> Option<u64> {
    numero(json, campo).filter(|n| *n >= 0.0).map(|n| n as u64)
}

fn booleano(json: &str, campo: &str) -> bool {
    cru(json, campo) == Some("true")
}

/// Texto simples entre aspas. Recusa o que tenha escape: nenhum campo lido aqui
/// precisa disso, e assim nada estranho chega ao painel.
fn texto(json: &str, campo: &str) -> String {
    let Some(bruto) = cru(json, campo) else { return String::new() };
    let sem_aspas = bruto.trim_matches('"');
    if sem_aspas.contains('\\') || sem_aspas.len() > 96 {
        return String::new();
    }
    sem_aspas.chars().filter(|c| !c.is_control()).collect()
}

/// Pergunta o estado de uma máquina. Nunca lança e nunca demora muito: o pior
/// caso é uma `Vista` que diz o que deu errado.
pub(crate) fn olhar(alvo: &str, agora: u64) -> Vista {
    match buscar(alvo) {
        Ok(json) => ler(alvo, agora, &json),
        Err(e) => Vista::caiu(alvo, agora, e),
    }
}

fn ler(alvo: &str, agora: u64, json: &str) -> Vista {
    if !json.contains("\"rede\":") {
        return Vista::caiu(alvo, agora, "respondeu, mas não parece ser um Hyurax".into());
    }
    if booleano(json, "trancado") {
        return Vista::caiu(alvo, agora, "essa máquina está trancada: destrave nela para o painel ler".into());
    }
    Vista {
        alvo: alvo.to_string(),
        quando: agora,
        ok: true,
        erro: String::new(),
        minerando: booleano(json, "minerando"),
        ritmo: numero(json, "ritmo").unwrap_or(0.0),
        linhas: inteiro(json, "linhas").unwrap_or(0) as u32,
        nucleos: inteiro(json, "nucleos").unwrap_or(0) as u32,
        altura: inteiro(json, "altura").unwrap_or(0),
        watts: numero(json, "watts").unwrap_or(0.0),
        tentativas: inteiro(json, "tentativas").unwrap_or(0),
        versao: texto(json, "versao"),
        endereco: texto(json, "endereco"),
        saldo: texto(json, "saldo"),
        rede: texto(json, "rede"),
    }
}

/// GET `/api/estado` na unha: o painel do outro lado é HTTP simples na rede
/// local, e o projeto não carrega biblioteca de HTTP.
fn buscar(alvo: &str) -> Result<String, String> {
    let endereco: SocketAddr = alvo
        .to_socket_addrs()
        .map_err(|_| "não consegui resolver esse endereço".to_string())?
        .next()
        .ok_or("endereço sem resposta de DNS")?;
    let mut s = TcpStream::connect_timeout(&endereco, Duration::from_secs(2))
        .map_err(|_| "não respondeu (a máquina está ligada e com o Hyurax aberto?)".to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(4))).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(Duration::from_secs(2))).map_err(|e| e.to_string())?;
    let pedido = format!("GET /api/estado HTTP/1.1\r\nHost: {alvo}\r\nUser-Agent: Hyurax\r\nConnection: close\r\n\r\n");
    s.write_all(pedido.as_bytes()).map_err(|e| e.to_string())?;
    let mut bruto = Vec::new();
    let mut pedaco = [0u8; 8192];
    loop {
        let n = s.read(&mut pedaco).map_err(|_| "a resposta parou no meio".to_string())?;
        if n == 0 {
            break;
        }
        match pedaco.get(..n) {
            Some(parte) => bruto.extend_from_slice(parte),
            None => break,
        }
        if bruto.len() > RESPOSTA_MAXIMA {
            return Err("resposta grande demais para ser um estado".into());
        }
    }
    let texto = String::from_utf8_lossy(&bruto).into_owned();
    let (cabecalho, corpo) = texto.split_once("\r\n\r\n").ok_or("resposta sem corpo")?;
    if cabecalho.contains(" 403 ") {
        return Err("recusou: ligue \"ver no celular\" no Hyurax dessa máquina".into());
    }
    if !cabecalho.starts_with("HTTP/1.1 200") {
        let primeira = cabecalho.lines().next().unwrap_or("");
        return Err(format!("respondeu \"{}\"", primeira.trim()));
    }
    Ok(corpo.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    const EXEMPLO: &str = r#"{"rede":"hyurax-testnet","altura":42,"minerando":true,"linhas":3,"nucleos":4,
        "ritmo":18.75,"watts":36.0,"tentativas":9001,"versao":"0.1.0","endereco":"aabb","saldo":"12.50000000",
        "trabalho_util":{"familia":"matrizes","n":96}}"#;

    #[test]
    fn le_os_campos_que_importam() {
        let v = ler("192.168.0.9:8800", 100, EXEMPLO);
        assert!(v.ok && v.minerando);
        assert_eq!((v.altura, v.linhas, v.nucleos, v.tentativas), (42, 3, 4, 9001));
        assert!((v.ritmo - 18.75).abs() < 1e-9);
        assert!((v.watts - 36.0).abs() < 1e-9);
        assert_eq!((v.versao.as_str(), v.saldo.as_str(), v.rede.as_str()), ("0.1.0", "12.50000000", "hyurax-testnet"));
        assert_eq!(v.quando, 100);
    }

    #[test]
    fn resposta_que_nao_e_hyurax_ou_esta_trancada_nao_passa() {
        assert!(!ler("x:1", 1, "{\"ola\":1}").ok);
        let trancada = "{\"rede\":\"hyurax-testnet\",\"trancado\":true}";
        assert!(ler("x:1", 1, trancada).erro.contains("trancada"));
    }

    #[test]
    fn campo_que_falta_vira_zero_em_vez_de_travar() {
        let v = ler("x:1", 1, "{\"rede\":\"hyurax-testnet\"}");
        assert!(v.ok);
        assert_eq!((v.altura, v.ritmo, v.versao.as_str()), (0, 0.0, ""));
    }

    #[test]
    fn texto_com_escape_ou_comprido_demais_e_descartado() {
        assert_eq!(texto("{\"versao\":\"a\\u0041b\"}", "versao"), "");
        let comprido = format!("{{\"versao\":\"{}\"}}", "a".repeat(200));
        assert_eq!(texto(&comprido, "versao"), "");
    }
}
