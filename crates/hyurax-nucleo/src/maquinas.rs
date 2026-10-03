//! As outras máquinas do dono, no mesmo painel.
//!
//! Cada Hyurax publica um resumo público em `/api/v1/resumo`. Quando o dono
//! liga "ver no celular" numa máquina, esse endereço responde para a rede
//! local, só para leitura. Aqui o programa pergunta o resumo das máquinas que
//! o dono listou (`IP:PORTA`) e mostra o total num lugar só.
//!
//! Isto **não** é controle remoto: só lê. Comando continua valendo só de
//! dentro de cada computador.
//!
//! **Autenticação.** Cada pergunta leva um desafio aleatório, e a resposta
//! volta assinada (Ed25519) pela chave de worker da máquina que respondeu,
//! sobre o desafio e os bytes exatos do resumo. Na primeira resposta válida
//! a chave fica guardada (`maquinas-conhecidas.txt`); depois disso, um
//! aparelho que tome o IP da máquina na rede local não consegue mostrar
//! números falsos, nem repetir uma resposta velha. Para aceitar uma chave
//! nova (reinstalação), o dono tira a máquina da lista e põe de novo.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use hyurax_crypto::PUBKEY_LEN;
use serde_json::Value;

use crate::util::{de_hex, hex};

/// Onde ficam as chaves já vistas de cada máquina, na pasta de configuração.
pub const ARQUIVO_DAS_CONHECIDAS: &str = "maquinas-conhecidas.txt";
/// Cabeçalho com a chave pública de quem assinou o resumo.
pub const CABECALHO_CHAVE: &str = "X-Hyurax-Assinado-Por";
/// Cabeçalho com a assinatura do resumo.
pub const CABECALHO_ASSINATURA: &str = "X-Hyurax-Assinatura";
/// Etiqueta de domínio da assinatura do resumo.
const ETIQUETA: &[u8] = b"HYURAX-RESUMO-v1";

/// Os bytes assinados: etiqueta, desafio e o corpo exato da resposta.
pub fn mensagem(desafio: &[u8; 32], corpo: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(ETIQUETA.len().saturating_add(32).saturating_add(corpo.len()));
    m.extend_from_slice(ETIQUETA);
    m.extend_from_slice(desafio);
    m.extend_from_slice(corpo);
    m
}

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
    /// A chave pública (hex) que assinou, quando a assinatura conferiu.
    pub chave: String,
}

/// A lista e a última olhada em cada máquina.
#[derive(Default)]
pub struct Maquinas {
    /// A lista de `IP:PORTA`.
    pub lista: Mutex<Vec<String>>,
    /// A última olhada.
    pub vistas: Mutex<Vec<Vista>>,
    /// A chave já vista de cada máquina da lista.
    conhecidas: Mutex<BTreeMap<String, [u8; PUBKEY_LEN]>>,
    /// Onde as chaves conhecidas ficam gravadas (nenhum: só na memória).
    arquivo: Option<PathBuf>,
}

impl Maquinas {
    /// Com as chaves conhecidas gravadas neste arquivo.
    pub fn com_arquivo(arquivo: PathBuf) -> Self {
        let conhecidas = std::fs::read_to_string(&arquivo).map(|t| ler_conhecidas(&t)).unwrap_or_default();
        Self { conhecidas: Mutex::new(conhecidas), arquivo: Some(arquivo), ..Self::default() }
    }

    fn gravar_conhecidas(&self) {
        let (Some(arquivo), Ok(c)) = (&self.arquivo, self.conhecidas.lock()) else { return };
        let mut texto = String::from("# Hyurax: a chave de cada máquina do painel, vista na primeira resposta.\n# Apagar uma linha faz o programa aceitar a próxima chave daquela máquina.\n");
        for (alvo, chave) in c.iter() {
            texto.push_str(&format!("{alvo} {}\n", hex(chave)));
        }
        let _ = crate::arquivos::gravar_atomico(arquivo, texto.as_bytes());
    }

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
        // quem saiu da lista esquece a chave: voltar com ela aceita uma nova
        let mudou = self.conhecidas.lock().is_ok_and(|mut c| {
            let antes = c.len();
            c.retain(|alvo, _| novas.contains(alvo));
            c.len() != antes
        });
        if mudou {
            self.gravar_conhecidas();
        }
        Ok(novas)
    }

    /// Olha todas uma vez.
    pub fn olhar_todas(&self, agora: u64) {
        let alvos: Vec<String> = self.lista.lock().map(|l| l.clone()).unwrap_or_default();
        let mut aprendeu = false;
        let mut vistas = Vec::with_capacity(alvos.len());
        for alvo in &alvos {
            let fixada = self.conhecidas.lock().ok().and_then(|c| c.get(alvo).copied());
            let (vista, chave) = olhar(alvo, agora, fixada.as_ref());
            if fixada.is_none()
                && let Some(chave) = chave
                && let Ok(mut c) = self.conhecidas.lock()
            {
                c.insert(alvo.clone(), chave);
                aprendeu = true;
            }
            vistas.push(vista);
        }
        if aprendeu {
            self.gravar_conhecidas();
        }
        if let Ok(mut v) = self.vistas.lock() {
            *v = vistas;
        }
    }
}

fn ler_conhecidas(texto: &str) -> BTreeMap<String, [u8; PUBKEY_LEN]> {
    texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (alvo, chave) = l.split_once(' ')?;
            Some((alvo.to_string(), de_hex::<PUBKEY_LEN>(chave.trim())?))
        })
        .take(MAXIMO)
        .collect()
}

/// Pergunta o resumo de uma máquina. Nunca demora muito e nunca falha: o pior
/// caso é uma `Vista` que diz o que deu errado. Devolve também a chave que
/// assinou, quando a assinatura conferiu (para fixar na primeira vez).
pub fn olhar(alvo: &str, agora: u64, fixada: Option<&[u8; PUBKEY_LEN]>) -> (Vista, Option<[u8; PUBKEY_LEN]>) {
    let caiu = |erro: String| Vista { alvo: alvo.to_string(), quando: agora, ok: false, erro, resumo: Value::Null, chave: String::new() };
    let desafio = match hyurax_net::entropia::entropia_do_sistema() {
        Ok(d) => d,
        Err(e) => return (caiu(format!("sem gerador aleatório: {e}")), None),
    };
    let resposta = buscar(alvo, &desafio).and_then(|(cabecalho, corpo)| {
        let chave = conferir(&cabecalho, &desafio, corpo.as_bytes(), fixada)?;
        Ok((ler(&corpo)?, chave))
    });
    match resposta {
        Ok((resumo, chave)) => {
            (Vista { alvo: alvo.to_string(), quando: agora, ok: true, erro: String::new(), resumo, chave: hex(&chave) }, Some(chave))
        }
        Err(e) => (caiu(e), None),
    }
}

/// O valor de um cabeçalho da resposta (nome sem diferença de caixa).
fn cabecalho_de<'a>(cabecalho: &'a str, nome: &str) -> Option<&'a str> {
    cabecalho.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(nome).then(|| v.trim())
    })
}

/// Confere a assinatura do resumo e, se já havia chave fixada, que é a mesma.
fn conferir(cabecalho: &str, desafio: &[u8; 32], corpo: &[u8], fixada: Option<&[u8; PUBKEY_LEN]>) -> Result<[u8; PUBKEY_LEN], String> {
    let chave = cabecalho_de(cabecalho, CABECALHO_CHAVE).and_then(de_hex::<PUBKEY_LEN>);
    let assinatura = cabecalho_de(cabecalho, CABECALHO_ASSINATURA).and_then(de_hex::<64>);
    let (Some(chave), Some(assinatura)) = (chave, assinatura) else {
        return Err("respondeu sem assinatura: atualize o Hyurax dessa máquina".into());
    };
    if fixada.is_some_and(|f| *f != chave) {
        return Err("a chave dessa máquina mudou (outro aparelho com o mesmo IP?). Se você reinstalou o Hyurax nela, tire da lista e ponha de novo".into());
    }
    if !hyurax_crypto::ed25519_verify(&chave, &mensagem(desafio, corpo), &assinatura) {
        return Err("a assinatura do resumo não confere".into());
    }
    Ok(chave)
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
/// Devolve o cabeçalho e o corpo.
fn buscar(alvo: &str, desafio: &[u8; 32]) -> Result<(String, String), String> {
    let endereco: SocketAddr = alvo
        .to_socket_addrs()
        .map_err(|_| "não consegui resolver esse endereço".to_string())?
        .next()
        .ok_or("endereço sem resposta de DNS")?;
    let mut s = TcpStream::connect_timeout(&endereco, Duration::from_secs(2))
        .map_err(|_| "não respondeu (a máquina está ligada e com o Hyurax aberto?)".to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(4))).map_err(|e| e.to_string())?;
    s.set_write_timeout(Some(Duration::from_secs(2))).map_err(|e| e.to_string())?;
    let pedido = format!(
        "GET /api/v1/resumo?desafio={} HTTP/1.1\r\nHost: {alvo}\r\nUser-Agent: Hyurax\r\nConnection: close\r\n\r\n",
        hex(desafio)
    );
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
    // o corpo é conferido byte a byte pela assinatura: nada de conversão com perda
    let fim = bruto.windows(4).position(|w| w == b"\r\n\r\n").ok_or("resposta sem corpo")?;
    let cabecalho = String::from_utf8_lossy(bruto.get(..fim).unwrap_or_default()).into_owned();
    let corpo = String::from_utf8(bruto.get(fim.saturating_add(4)..).unwrap_or_default().to_vec())
        .map_err(|_| "o resumo não é texto UTF-8".to_string())?;
    if cabecalho.contains(" 403 ") {
        return Err("recusou: ligue \"ver no celular\" no Hyurax dessa máquina".into());
    }
    if !cabecalho.starts_with("HTTP/1.1 200") {
        return Err(format!("respondeu \"{}\"", cabecalho.lines().next().unwrap_or("").trim()));
    }
    Ok((cabecalho, corpo))
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
    fn assinatura_do_resumo_e_chave_fixada() {
        let segredo = [9u8; 32];
        let chave = hyurax_crypto::ed25519_public_key(&segredo);
        let desafio = [4u8; 32];
        let corpo = br#"{"produto":"Hyurax / Ultrax","altura":42}"#;
        let assinatura = hyurax_crypto::ed25519_sign(&segredo, &mensagem(&desafio, corpo));
        let cab = format!("HTTP/1.1 200 OK\r\nx-hyurax-assinado-por: {}\r\n{CABECALHO_ASSINATURA}: {}", hex(&chave), hex(&assinatura));
        // primeira vez: confere e devolve a chave para fixar
        assert_eq!(conferir(&cab, &desafio, corpo, None), Ok(chave));
        assert_eq!(conferir(&cab, &desafio, corpo, Some(&chave)), Ok(chave));
        // outro corpo, outro desafio (repetição de resposta velha), outra chave
        assert!(conferir(&cab, &desafio, br#"{"produto":"Hyurax / Ultrax","altura":43}"#, None).is_err());
        assert!(conferir(&cab, &[5u8; 32], corpo, None).is_err());
        assert!(conferir(&cab, &desafio, corpo, Some(&[1u8; 32])).unwrap_err().contains("mudou"));
        // sem assinatura
        assert!(conferir("HTTP/1.1 200 OK", &desafio, corpo, None).unwrap_err().contains("sem assinatura"));
    }

    #[test]
    fn chaves_conhecidas_sobrevivem_e_saem_com_a_maquina() {
        let pasta = std::env::temp_dir().join(format!("hyurax-maquinas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        let arquivo = pasta.join(ARQUIVO_DAS_CONHECIDAS);
        let m = Maquinas::com_arquivo(arquivo.clone());
        m.trocar("192.168.0.9:8800, 10.0.0.2:8800").unwrap();
        m.conhecidas.lock().unwrap().insert("192.168.0.9:8800".into(), [7u8; 32]);
        m.conhecidas.lock().unwrap().insert("10.0.0.2:8800".into(), [8u8; 32]);
        m.gravar_conhecidas();
        let de_novo = Maquinas::com_arquivo(arquivo.clone());
        assert_eq!(de_novo.conhecidas.lock().unwrap().len(), 2);
        // tirar da lista esquece a chave, também no disco
        de_novo.trocar("10.0.0.2:8800").unwrap();
        let terceira = Maquinas::com_arquivo(arquivo);
        assert_eq!(terceira.conhecidas.lock().unwrap().keys().cloned().collect::<Vec<_>>(), vec!["10.0.0.2:8800".to_string()]);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn lista_so_aceita_ip_e_porta() {
        let m = Maquinas::default();
        assert_eq!(m.trocar("192.168.0.9:8800, 10.0.0.2:8800").unwrap().len(), 2);
        assert!(m.trocar("192.168.0.9").is_err());
        assert!(m.trocar(&"1.2.3.4:1,".repeat(MAXIMO + 1)).is_err());
    }
}
