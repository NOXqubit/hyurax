//! O que os testes de ponta a ponta usam: um servidor de verdade na porta
//! 0, um cliente HTTP mínimo e um agente simulado que executa os motores do
//! ULTRAX de verdade e assina como o programa do fornecedor.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{ed25519_public_key, sha512};
use hyurax_plataforma::{Opcoes, Plataforma, regras};
use hyurax_ultrax::prova::RegistroDeProva;
use hyurax_ultrax::trabalho::{self, Especificacao};
use serde_json::{Value, json};

/// Um servidor de teste: a plataforma, o endereço e o relógio adiantável.
pub struct Servidor {
    pub p: Arc<Plataforma>,
    pub endereco: String,
    pub adianto: Arc<AtomicU64>,
    pub pasta: PathBuf,
}

pub fn pasta(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("hyx-plat-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

pub fn relogio(adianto: &Arc<AtomicU64>) -> Arc<dyn Fn() -> u64 + Send + Sync> {
    let a2 = Arc::clone(adianto);
    Arc::new(move || hyurax_plataforma::util::agora_ms() + a2.load(Ordering::Relaxed))
}

pub fn servidor(nome: &str, modo_teste: bool) -> Servidor {
    let pasta = pasta(nome);
    let adianto = Arc::new(AtomicU64::new(0));
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let endereco = ouvinte.local_addr().unwrap().to_string();
    let p = Plataforma::abrir_com_relogio(&pasta, Opcoes { modo_teste, https: false, endereco_publico: format!("http://{endereco}") }, relogio(&adianto)).unwrap().plataforma;
    let p2 = Arc::clone(&p);
    std::thread::spawn(move || hyurax_plataforma::http::servir_em(p2, ouvinte, 0));
    Servidor { p, endereco, adianto, pasta }
}

/// Resposta crua.
pub struct Resp {
    pub status: u16,
    pub cabecalhos: Vec<(String, String)>,
    pub corpo: Vec<u8>,
}

impl Resp {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.corpo).unwrap_or(Value::Null)
    }
    pub fn texto(&self) -> String {
        String::from_utf8_lossy(&self.corpo).into_owned()
    }
    pub fn cookie(&self) -> Option<String> {
        self.cabecalhos.iter().find(|(k, _)| k.eq_ignore_ascii_case("set-cookie")).and_then(|(_, v)| v.split(';').next().map(str::to_string))
    }
}

pub fn pedir(endereco: &str, metodo: &str, caminho: &str, cabecalhos: &[(&str, String)], corpo: &[u8]) -> Resp {
    let mut s = TcpStream::connect(endereco).unwrap();
    let mut cab = format!("{metodo} {caminho} HTTP/1.1\r\nHost: {endereco}\r\nContent-Length: {}\r\n", corpo.len());
    for (k, v) in cabecalhos {
        cab.push_str(&format!("{k}: {v}\r\n"));
    }
    cab.push_str("\r\n");
    s.write_all(cab.as_bytes()).unwrap();
    let _ = s.write_all(corpo);
    let mut tudo = Vec::new();
    let _ = s.read_to_end(&mut tudo);
    let fim = tudo.windows(4).position(|j| j == b"\r\n\r\n").unwrap_or_else(|| panic!("resposta sem cabeçalho: {}", String::from_utf8_lossy(&tudo)));
    let texto = String::from_utf8_lossy(&tudo[..fim]).to_string();
    let mut linhas = texto.split("\r\n");
    let status: u16 = linhas.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
    let cabecalhos = linhas.filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
    Resp { status, cabecalhos, corpo: tudo[fim + 4..].to_vec() }
}

/// Um navegador: guarda o cookie e manda o cabeçalho contra CSRF.
pub struct Navegador {
    pub endereco: String,
    pub cookie: Option<String>,
}

impl Navegador {
    pub fn novo(s: &Servidor) -> Self {
        Self { endereco: s.endereco.clone(), cookie: None }
    }
    fn cab(&self) -> Vec<(&'static str, String)> {
        let mut c = vec![("X-Hyx-Pedido", "1".to_string()), ("Content-Type", "application/json".to_string())];
        if let Some(k) = &self.cookie {
            c.push(("Cookie", k.clone()));
        }
        c
    }
    pub fn get(&self, caminho: &str) -> Resp {
        pedir(&self.endereco, "GET", caminho, &self.cab(), b"")
    }
    pub fn post(&mut self, caminho: &str, corpo: Value) -> Resp {
        let r = pedir(&self.endereco, "POST", caminho, &self.cab(), corpo.to_string().as_bytes());
        if let Some(c) = r.cookie() {
            self.cookie = Some(c);
        }
        r
    }
    pub fn cadastrar(&mut self, email: &str, cliente: bool, fornecedor: bool) -> Value {
        let r = self.post(
            "/api/v1/auth/cadastro",
            json!({ "email": email, "nome": "Pessoa de Teste", "senha": "ponte-azul-77-rio", "cliente": cliente, "fornecedor": fornecedor, "aceite": true, "moeda": "BRL", "pais": "BR" }),
        );
        assert_eq!(r.status, 201, "{}", r.texto());
        r.json()
    }
}

/// Um agente simulado: chave de worker própria, assina cada pedido e executa
/// as unidades com os motores de verdade.
pub struct Agente {
    pub endereco: String,
    pub segredo: [u8; 32],
    pub chave: [u8; 32],
    pub relogio: Arc<AtomicU64>,
    pub no: u64,
    pub estraga: bool,
}

impl Agente {
    pub fn novo(s: &Servidor, semente: u8) -> Self {
        let segredo: [u8; 32] = sha512(&[semente; 8])[..32].try_into().unwrap();
        Self { endereco: s.endereco.clone(), chave: ed25519_public_key(&segredo), segredo, relogio: Arc::clone(&s.adianto), no: 0, estraga: false }
    }
    pub fn agora(&self) -> u64 {
        hyurax_plataforma::util::agora_ms() + self.relogio.load(Ordering::Relaxed)
    }
    pub fn cabecalhos(&self, metodo: &str, caminho: &str, corpo: &[u8]) -> Vec<(&'static str, String)> {
        let t = self.agora();
        let a = regras::assinar_pedido(&self.segredo, metodo, caminho, t, corpo);
        vec![
            ("X-Hyx-No", hyurax_plataforma::util::hex(&self.chave)),
            ("X-Hyx-Tempo", t.to_string()),
            ("X-Hyx-Assinatura", hyurax_plataforma::util::hex(&a)),
            ("Content-Type", "application/json".to_string()),
        ]
    }
    pub fn assinado(&self, metodo: &str, caminho: &str, corpo: &[u8]) -> Resp {
        let cab = self.cabecalhos(metodo, caminho, corpo);
        pedir(&self.endereco, metodo, caminho, &cab, corpo)
    }
    pub fn registrar(&mut self, codigo: &str, nome: &str) -> Value {
        let corpo = json!({ "codigo": codigo, "nome": nome, "versao": "1.5.0",
            "hardware": { "cpu_modelo": format!("CPU de teste {nome}"), "nucleos": 4, "ram_mib": 8192, "gpu_modelo": "", "gpu_vram_mib": 0, "disco_livre_mib": 100000, "so": "teste" },
            "limites": { "linhas": 2, "ram_mib": 4096, "gpu": false } });
        let r = self.assinado("POST", "/api/v1/agente/registrar", corpo.to_string().as_bytes());
        assert_eq!(r.status, 200, "{}", r.texto());
        let j = r.json();
        self.no = j["no"].as_u64().unwrap();
        j
    }
    pub fn pulso(&self, progresso: Value) -> Value {
        let corpo = json!({ "versao": "1.5.0", "livres": 2, "ram_livre_mib": 4096, "tipos": [1,2,3,4,5,6,7,8], "gpu": false, "pausado": false,
            "progresso": progresso, "telemetria": { "cpu": 12.5, "ram_usada_mib": 3000, "temperatura": 51.0, "potencia_w": 30.0, "rede_bytes": 1000 } });
        let r = self.assinado("POST", "/api/v1/agente/pulso", corpo.to_string().as_bytes());
        assert_eq!(r.status, 200, "{}", r.texto());
        r.json()
    }
    /// Executa uma atribuição e devolve o corpo binário da entrega.
    pub fn envelope(&self, a: &Value) -> Vec<u8> {
        let id = a["id"].as_u64().unwrap();
        let esp_b = hex::decode(a["especificacao"].as_str().unwrap()).unwrap();
        let mut r = Reader::new(&esp_b);
        let esp = Especificacao::decodificar(&mut r).unwrap();
        let semente = hex::decode(a["semente"].as_str().unwrap()).unwrap();
        let inicio = self.agora();
        let ex = trabalho::executar(&esp, &semente, &mut |_| true).unwrap();
        let reg = RegistroDeProva {
            tarefa: sha512(&id.to_be_bytes()),
            entrada: trabalho::hash_da_entrada(&esp, &semente).unwrap(),
            especificacao: esp,
            metodo: esp.tipo().metodo(),
            resultado: trabalho::hash_do_resultado(&ex.resultado),
            operacoes: ex.operacoes,
            worker: self.chave,
            inicio_ms: inicio,
            fim_ms: self.agora(),
        };
        let assinatura = reg.assinar(&self.segredo).unwrap();
        let mut w = Writer::new();
        w.u64(id);
        w.var_bytes(&reg.codificar()).unwrap();
        w.fixed(&assinatura);
        w.u64(5);
        w.u8(0);
        w.var_bytes(&ex.resultado).unwrap();
        w.into_bytes()
    }

    /// Executa uma atribuição e entrega. Devolve a resposta da plataforma.
    pub fn executar_e_entregar(&self, a: &Value) -> Value {
        let id = a["id"].as_u64().unwrap();
        let esp_b = hex::decode(a["especificacao"].as_str().unwrap()).unwrap();
        let mut r = Reader::new(&esp_b);
        let esp = Especificacao::decodificar(&mut r).unwrap();
        let semente = hex::decode(a["semente"].as_str().unwrap()).unwrap();
        let inicio = self.agora();
        let ex = trabalho::executar(&esp, &semente, &mut |_| true).unwrap();
        let mut resultado = ex.resultado;
        if self.estraga
            && let Some(b) = resultado.last_mut()
        {
            // um fornecedor desonesto: devolve outra coisa, assinando direitinho
            *b ^= 1;
        }
        let reg = RegistroDeProva {
            tarefa: sha512(&id.to_be_bytes()),
            entrada: trabalho::hash_da_entrada(&esp, &semente).unwrap(),
            especificacao: esp,
            metodo: esp.tipo().metodo(),
            resultado: trabalho::hash_do_resultado(&resultado),
            operacoes: ex.operacoes,
            worker: self.chave,
            inicio_ms: inicio,
            fim_ms: self.agora(),
        };
        let assinatura = reg.assinar(&self.segredo).unwrap();
        let mut w = Writer::new();
        w.u64(id);
        w.var_bytes(&reg.codificar()).unwrap();
        w.fixed(&assinatura);
        w.u64(5);
        w.u8(0);
        w.var_bytes(&resultado).unwrap();
        let r = self.assinado("POST", "/api/v1/agente/resultado", &w.into_bytes());
        assert!(r.status == 200 || r.status == 409, "{} {}", r.status, r.texto());
        r.json()
    }
    /// Um ciclo: pulsa, executa o que recebeu, entrega. Devolve quantas fez.
    pub fn ciclo(&self) -> usize {
        let j = self.pulso(json!([]));
        let lista = j["atribuicoes"].as_array().cloned().unwrap_or_default();
        for a in &lista {
            self.executar_e_entregar(a);
        }
        lista.len()
    }
}
