//! As outras máquinas do dono, no mesmo painel.
//!
//! Cada Hyurax publica um resumo público em `/api/v1/resumo`. Quando o dono
//! liga "ver no celular" numa máquina, esse endereço responde para a rede
//! local, só para leitura. Aqui o programa pergunta o resumo das máquinas que
//! o dono listou (`IP:PORTA`) e mostra o total num lugar só.
//!
//! Isto **não** é controle remoto: só lê. Comando continua valendo só de
//! dentro de cada computador.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

/// Quantas máquinas a lista aceita.
pub const MAXIMO: usize = 8;
/// Teto do que o programa lê de uma resposta: o resumo tem menos de 2 KiB.
const RESPOSTA_MAXIMA: usize = 64 * 1024;
/// De quanto em quanto tempo as máquinas são perguntadas.
pub const INTERVALO: Duration = Duration::from_secs(6);

/// O que se sabe de uma máquina da lista, na última olhada.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Vista {
    /// `IP:PORTA`.
    pub alvo: String,
    /// Quando esta leitura foi feita (unix). Zero: nunca respondeu ainda.
    pub quando: u64,
    /// Respondeu com um resumo válido.
    pub ok: bool,
    /// O que deu errado, quando não respondeu.
    pub erro: String,
    /// O resumo dela, como veio (já conferido: é JSON de um Hyurax).
    pub resumo: Value,
}

/// A lista e a última olhada em cada máquina.
#[derive(Default)]
pub struct Maquinas {
    /// A lista de `IP:PORTA`.
    pub lista: Mutex<Vec<String>>,
    /// A última olhada.
    pub vistas: Mutex<Vec<Vista>>,
}

impl Maquinas {
    /// Troca a lista. A olhada de quem saiu some na hora.
    ///
    /// # Errors
    /// Endereço inválido ou lista grande demais.
    pub fn trocar(&self, texto: &str) -> Result<Vec<String>, String> {
        let novas: Vec<String> =
            texto.split([',', '\n', ' ', ';']).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
        if novas.len() > MAXIMO {
            return Err(format!("no máximo {MAXIMO} máquinas"));
        }
        if let Some(ruim) = novas.iter().find(|s| !crate::sementes::valida(s)) {
            return Err(format!("\"{ruim}\" não é IP:PORTA (exemplo: 192.168.0.12:8800)"));
        }
        if let Ok(mut l) = self.lista.lock() {
            l.clone_from(&novas);
        }
        if let Ok(mut v) = self.vistas.lock() {
            v.retain(|vista| novas.contains(&vista.alvo));
        }
        Ok(novas)
    }

    /// Olha todas uma vez.
    pub fn olhar_todas(&self, agora: u64) {
        let alvos: Vec<String> = self.lista.lock().map(|l| l.clone()).unwrap_or_default();
        let vistas: Vec<Vista> = alvos.iter().map(|a| olhar(a, agora)).collect();
        if let Ok(mut v) = self.vistas.lock() {
            *v = vistas;
        }
    }
}

/// Pergunta o resumo de uma máquina. Nunca demora muito e nunca falha: o pior
/// caso é uma `Vista` que diz o que deu errado.
pub fn olhar(alvo: &str, agora: u64) -> Vista {
    let caiu = |erro: String| Vista { alvo: alvo.to_string(), quando: agora, ok: false, erro, resumo: Value::Null };
    match buscar(alvo).and_then(|corpo| ler(&corpo)) {
        Ok(resumo) => Vista { alvo: alvo.to_string(), quando: agora, ok: true, erro: String::new(), resumo },
        Err(e) => caiu(e),
    }
}

/// Confere que o corpo é o resumo de um Hyurax destrancado.
fn ler(corpo: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(corpo).map_err(|_| "respondeu, mas não parece ser um Hyurax".to_string())?;
    if v.get("produto").and_then(Value::as_str).is_none_or(|p| !p.starts_with("Hyurax")) {
        return Err("respondeu, mas não parece ser um Hyurax 1.0".into());
    }
    if v.get("trancado").and_then(Value::as_bool) == Some(true) {
        return Err("essa máquina está trancada: destrave nela para o painel ler".into());
    }
    Ok(v)
}

/// GET `/api/v1/resumo` na unha: o outro lado é HTTP simples na rede local.
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
    let pedido = format!("GET /api/v1/resumo HTTP/1.1\r\nHost: {alvo}\r\nUser-Agent: Hyurax\r\nConnection: close\r\n\r\n");
    s.write_all(pedido.as_bytes()).map_err(|e| e.to_string())?;
    let mut bruto = Vec::new();
    let mut pedaco = [0u8; 8192];
    loop {
        let n = s.read(&mut pedaco).map_err(|_| "a resposta parou no meio".to_string())?;
        if n == 0 {
            break;
        }
        bruto.extend_from_slice(pedaco.get(..n).unwrap_or_default());
        if bruto.len() > RESPOSTA_MAXIMA {
            return Err("resposta grande demais para ser um resumo".into());
        }
    }
    let texto = String::from_utf8_lossy(&bruto).into_owned();
    let (cabecalho, corpo) = texto.split_once("\r\n\r\n").ok_or("resposta sem corpo")?;
    if cabecalho.contains(" 403 ") {
        return Err("recusou: ligue \"ver no celular\" no Hyurax dessa máquina".into());
    }
    if !cabecalho.starts_with("HTTP/1.1 200") {
        return Err(format!("respondeu \"{}\"", cabecalho.lines().next().unwrap_or("").trim()));
    }
    Ok(corpo.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn so_aceita_resumo_de_hyurax_destrancado() {
        assert!(ler(r#"{"produto":"Hyurax / Ultrax","versao":"1.0.0","altura":42}"#).is_ok());
        assert!(ler(r#"{"ola":1}"#).is_err());
        assert!(ler("não é json").is_err());
        assert!(ler(r#"{"produto":"Hyurax / Ultrax","trancado":true}"#).unwrap_err().contains("trancada"));
    }

    #[test]
    fn lista_so_aceita_ip_e_porta() {
        let m = Maquinas::default();
        assert_eq!(m.trocar("192.168.0.9:8800, 10.0.0.2:8800").unwrap().len(), 2);
        assert!(m.trocar("192.168.0.9").is_err());
        assert!(m.trocar(&"1.2.3.4:1,".repeat(MAXIMO + 1)).is_err());
    }
}
