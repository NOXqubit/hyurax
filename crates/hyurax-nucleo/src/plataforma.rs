// ✝ Eclesiastes 9:10 — “Tudo quanto te vier à mão para fazer, faze-o conforme as tuas forças.”
//! O agente da plataforma HYURAX Compute (V1): este computador como
//! fornecedor no mercado de computação.
//!
//! - **Vínculo:** o dono gera um código no painel da plataforma e cola aqui;
//!   o agente registra o computador com a chave do worker (a mesma que assina
//!   as provas do ULTRAX) e guarda o número do nó em `plataforma.txt`.
//! - **Pulso:** a cada 5 s o agente conta o que tem livre (núcleos e memória
//!   dentro dos limites que o dono escolheu), manda a telemetria e recebe o
//!   que parar e as unidades novas. Só de dentro para fora, por HTTPS: o
//!   computador não abre porta nenhuma.
//! - **Execução:** as unidades entram no mesmo worker do ULTRAX, com a mesma
//!   reserva de memória, o mesmo limite de CPU, prazo e prova assinada. Só
//!   rodam os motores que vêm no programa; o cliente manda parâmetros, nunca
//!   código, e o motor não abre arquivo nem rede (seção 3 do documento da V1).
//! - **Entrega:** o resultado, o registro de prova e a assinatura vão para a
//!   plataforma, que confere (Freivalds, concordância entre nós) e paga.
//! - **Parar:** pausar aqui ou no painel, ou o administrador mandar parar,
//!   cancela o que está rodando na hora.
//!
//! Cada pedido vai assinado (`HYURAX-PLATAFORMA-PEDIDO-v1`, a mesma regra de
//! `reference/hyurax/plataforma.py`, conferida contra `vectors/plataforma.json`).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::Duration;

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN, ed25519_public_key, ed25519_sign, sha512};
use hyurax_ultrax::tarefa::Estado;
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho};
use serde_json::{Value, json};

use crate::ciencia::Ciencia;
use crate::metricas::Metricas;
use crate::ultrax::{Agendador, DesfechoDeUnidade, PedidoDeUnidade, Ultrax};
use crate::util::{agora_ms, de_hex, hex};

/// Hexadecimal de tamanho livre.
fn bytes_de_hex(texto: &str) -> Option<Vec<u8>> {
    let t = texto.trim();
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len()).step_by(2).map(|i| t.get(i..i.saturating_add(2)).and_then(|par| u8::from_str_radix(par, 16).ok())).collect()
}

/// Arquivo da configuração do agente (pasta de configuração).
pub const ARQUIVO: &str = "plataforma.txt";
/// Domínio da assinatura do pedido (igual ao da plataforma).
pub const DOMINIO_PEDIDO: &[u8] = concat!(hyurax_identidade::raiz!(), "-PLATAFORMA-PEDIDO-v1").as_bytes();
/// Prioridade das unidades da plataforma na fila do worker (JOB = 150, LAB = 100).
const PRIORIDADE: u8 = 200;
/// Pulso padrão.
const PULSO: Duration = Duration::from_secs(5);
/// Prazo de cada pedido HTTP.
const PRAZO_HTTP: Duration = Duration::from_secs(30);
/// Resumo (ganhos, reputação) a cada isso.
const RESUMO_MS: u64 = 30_000;

/// `H(DOMINIO_PEDIDO || str metodo || str caminho || u64 tempo || H(corpo))`.
pub fn mensagem_do_pedido(metodo: &str, caminho: &str, tempo_ms: u64, corpo: &[u8]) -> [u8; HASH_LEN] {
    let mut w = Writer::new();
    w.raw(DOMINIO_PEDIDO);
    let _ = w.string(metodo);
    let _ = w.string(caminho);
    w.u64(tempo_ms);
    w.raw(&sha512(corpo));
    sha512(&w.into_bytes())
}

/// O que o dono escolheu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Endereço da plataforma (`https://...`).
    pub url: String,
    /// Número do nó na plataforma, depois do vínculo.
    pub no: Option<u64>,
    /// Fornecer agora.
    pub ligado: bool,
    /// Núcleos para a plataforma.
    pub linhas: u32,
    /// Memória para a plataforma, em MiB.
    pub ram_mib: u64,
    /// Aceitar trabalho na GPU.
    pub gpu: bool,
    /// Nome do computador.
    pub nome: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { url: String::new(), no: None, ligado: false, linhas: 1, ram_mib: 1024, gpu: false, nome: String::new() }
    }
}

impl Config {
    fn ler(pasta: Option<&PathBuf>) -> Self {
        let mut c = Self::default();
        let Some(texto) = pasta.and_then(|p| std::fs::read_to_string(p.join(ARQUIVO)).ok()) else { return c };
        for linha in texto.lines() {
            let Some((k, v)) = linha.trim().split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "url" if hyurax_net::https::ler_url(v).is_some() => c.url = v.to_string(),
                "no" => c.no = v.parse().ok(),
                "ligado" => c.ligado = v == "1",
                "linhas" => c.linhas = v.parse().unwrap_or(1).clamp(1, 1024),
                "ram_mib" => c.ram_mib = v.parse().unwrap_or(1024).clamp(256, 1 << 22),
                "gpu" => c.gpu = v == "1",
                "nome" => c.nome = v.chars().take(60).collect(),
                _ => {}
            }
        }
        c
    }

    fn texto(&self) -> String {
        format!(
            "# Agente da plataforma HYURAX Compute. Apagar desvincula este computador.\nurl={}\n{}ligado={}\nlinhas={}\nram_mib={}\ngpu={}\nnome={}\n",
            self.url,
            self.no.map(|n| format!("no={n}\n")).unwrap_or_default(),
            u8::from(self.ligado),
            self.linhas,
            self.ram_mib,
            u8::from(self.gpu),
            self.nome.replace(['\n', '\r'], " ")
        )
    }
}

/// Uma unidade recebida da plataforma.
#[derive(Clone, Debug)]
struct Atribuida {
    id: u64,
    job: [u8; HASH_LEN],
    unidade: u64,
    esp: Especificacao,
    semente: [u8; HASH_LEN],
    prazo_ms: u64,
    gpu: bool,
    memoria: u64,
    autoconferir: bool,
}

/// O que ficou para mandar à plataforma.
#[derive(Clone, Debug)]
enum Saida {
    Resultado { corpo: Vec<u8> },
    Falha { id: u64, motivo: String },
}

/// O que a tela mostra.
#[derive(Clone, Debug, Default)]
struct Situacao {
    conectado: bool,
    ultimo_pulso_ms: u64,
    ultimo_erro: String,
    estado_no: String,
    mensagem: String,
    atualizar: bool,
    resumo: Option<Value>,
    resumo_ms: u64,
    enviados: u64,
    aceitos: u64,
    recusados: u64,
}

/// O agente.
pub struct Agente {
    pasta: Option<PathBuf>,
    segredo: [u8; SECRET_LEN],
    chave: [u8; PUBKEY_LEN],
    versao: &'static str,
    config: Mutex<Config>,
    fila: Mutex<VecDeque<Atribuida>>,
    em_curso: Mutex<BTreeMap<([u8; HASH_LEN], u64), Atribuida>>,
    cancelados: Mutex<BTreeSet<u64>>,
    saida: Mutex<VecDeque<Saida>>,
    situacao: Mutex<Situacao>,
    acordar: Condvar,
    sinal: Mutex<bool>,
    ultrax: Mutex<Weak<Ultrax>>,
    metricas: Mutex<Weak<Metricas>>,
    lab_antes: AtomicBool,
    bytes: AtomicU64,
    encerrado: AtomicBool,
    /// Watts de um núcleo (estimativa de potência da telemetria).
    pub watts_nucleo: AtomicU64,
}

fn erro_http(r: &hyurax_net::https::Resposta) -> String {
    serde_json::from_slice::<Value>(&r.corpo).ok().and_then(|v| v.get("erro").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| format!("a plataforma respondeu {}", r.status))
}

impl Agente {
    /// Abre o agente com a chave do worker e a pasta de configuração.
    pub fn abrir(pasta: Option<PathBuf>, segredo_do_worker: [u8; SECRET_LEN], versao: &'static str) -> Arc<Self> {
        let config = Config::ler(pasta.as_ref());
        Arc::new(Self {
            pasta,
            chave: ed25519_public_key(&segredo_do_worker),
            segredo: segredo_do_worker,
            versao,
            config: Mutex::new(config),
            fila: Mutex::new(VecDeque::new()),
            em_curso: Mutex::new(BTreeMap::new()),
            cancelados: Mutex::new(BTreeSet::new()),
            saida: Mutex::new(VecDeque::new()),
            situacao: Mutex::new(Situacao::default()),
            acordar: Condvar::new(),
            sinal: Mutex::new(false),
            ultrax: Mutex::new(Weak::new()),
            metricas: Mutex::new(Weak::new()),
            lab_antes: AtomicBool::new(true),
            bytes: AtomicU64::new(0),
            encerrado: AtomicBool::new(false),
            watts_nucleo: AtomicU64::new(12),
        })
    }

    /// Liga ao worker e às métricas, põe o agendador composto (ciência +
    /// plataforma) no ULTRAX e começa o laço numa linha própria.
    pub fn ligar(self: &Arc<Self>, ultrax: &Arc<Ultrax>, ciencia: &Arc<Ciencia>, metricas: &Arc<Metricas>) {
        if let Ok(mut u) = self.ultrax.lock() {
            *u = Arc::downgrade(ultrax);
        }
        if let Ok(mut m) = self.metricas.lock() {
            *m = Arc::downgrade(metricas);
        }
        self.lab_antes.store(ultrax.lab.load(Ordering::Relaxed), Ordering::Relaxed);
        ultrax.ligar_agendador(Arc::new(Composto { ciencia: Arc::clone(ciencia), plataforma: Arc::clone(self) }));
        self.aplicar_no_worker();
        let a = Arc::clone(self);
        std::thread::spawn(move || a.laco());
    }

    /// Para o laço de vez.
    pub fn encerrar(&self) {
        self.encerrado.store(true, Ordering::Relaxed);
        self.cutucar();
    }

    fn cutucar(&self) {
        if let Ok(mut s) = self.sinal.lock() {
            *s = true;
        }
        self.acordar.notify_all();
    }

    fn worker(&self) -> Option<Arc<Ultrax>> {
        self.ultrax.lock().ok().and_then(|u| u.upgrade())
    }

    fn gravar(&self, c: &Config) {
        if let Some(p) = &self.pasta {
            let _ = std::fs::write(p.join(ARQUIVO), c.texto());
        }
    }

    /// A configuração atual.
    pub fn config(&self) -> Config {
        self.config.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Ligar o fornecimento liga o ULTRAX (com pelo menos os núcleos e a
    /// memória dados à plataforma) e desliga a carga de teste (LAB): núcleo
    /// emprestado e parado fica parado, sem gastar energia à toa.
    fn aplicar_no_worker(&self) {
        let c = self.config();
        let Some(u) = self.worker() else { return };
        if c.ligado && c.no.is_some() {
            u.lab.store(false, Ordering::Relaxed);
            if u.linhas.load(Ordering::Relaxed) < c.linhas {
                u.linhas.store(c.linhas, Ordering::Relaxed);
            }
            let mib = u32::try_from(c.ram_mib).unwrap_or(u32::MAX);
            if u.memoria_mib.load(Ordering::Relaxed) < mib {
                u.memoria_mib.store(mib, Ordering::Relaxed);
            }
            u.ligar(true);
        } else {
            u.lab.store(self.lab_antes.load(Ordering::Relaxed), Ordering::Relaxed);
        }
    }

    // -----------------------------------------------------------------------
    // pedidos assinados
    // -----------------------------------------------------------------------

    fn pedir(&self, metodo: &str, caminho: &str, corpo: &[u8]) -> Result<hyurax_net::https::Resposta, String> {
        let url = self.config().url;
        if url.is_empty() {
            return Err("informe o endereço da plataforma".into());
        }
        let tempo = agora_ms();
        let assinatura = ed25519_sign(&self.segredo, &mensagem_do_pedido(metodo, caminho, tempo, corpo));
        let cab = [
            ("X-Hyx-No", hex(&self.chave)),
            ("X-Hyx-Tempo", tempo.to_string()),
            ("X-Hyx-Assinatura", hex(&assinatura)),
            ("Content-Type", if caminho.ends_with("/resultado") { "application/octet-stream".to_string() } else { "application/json".to_string() }),
        ];
        let r = hyurax_net::https::pedir(metodo, &url, caminho, &cab, corpo, PRAZO_HTTP, 1 << 20).map_err(|e| format!("sem conexão com a plataforma: {e}"))?;
        self.bytes.fetch_add(u64::try_from(corpo.len().saturating_add(r.corpo.len())).unwrap_or(0), Ordering::Relaxed);
        Ok(r)
    }

    fn hardware(&self) -> Value {
        let m = self.metricas.lock().ok().and_then(|m| m.upgrade());
        let s = m.map(|m| m.sistema());
        let g = s.as_ref().and_then(|s| s.gpus.first());
        json!({
            "cpu_modelo": s.as_ref().map(|s| s.cpu.clone()).unwrap_or_default(),
            "nucleos": s.as_ref().map_or(1, |s| s.nucleos_logicos),
            "ram_mib": s.as_ref().and_then(|s| s.ram_total_mib).unwrap_or(0),
            "gpu_modelo": g.map(|g| g.nome.clone()).unwrap_or_default(),
            "gpu_vram_mib": g.and_then(|g| g.memoria_mib).unwrap_or(0),
            "disco_livre_mib": 0,
            "so": s.as_ref().map(|s| s.so.clone()).unwrap_or_default(),
        })
    }

    /// Vincula este computador com o código do painel.
    pub fn vincular(&self, url: &str, codigo: &str, nome: &str, linhas: u32, ram_mib: u64, gpu: bool) -> Result<Value, String> {
        let url = url.trim().trim_end_matches('/').to_string();
        if hyurax_net::https::ler_url(&url).is_none() {
            return Err("endereço da plataforma inválido (use https://...)".into());
        }
        {
            let mut c = self.config.lock().map_err(|_| "trava")?;
            c.url.clone_from(&url);
            c.nome = nome.trim().chars().take(60).collect();
            c.linhas = linhas.clamp(1, 1024);
            c.ram_mib = ram_mib.clamp(256, 1 << 22);
            c.gpu = gpu;
        }
        let c = self.config();
        let corpo = json!({
            "codigo": codigo.trim(),
            "nome": c.nome,
            "versao": self.versao,
            "hardware": self.hardware(),
            "limites": { "linhas": c.linhas, "ram_mib": c.ram_mib, "gpu": c.gpu },
        })
        .to_string();
        let r = self.pedir("POST", "/api/v1/agente/registrar", corpo.as_bytes())?;
        if r.status != 200 {
            return Err(erro_http(&r));
        }
        let v: Value = serde_json::from_slice(&r.corpo).map_err(|e| format!("resposta ilegível: {e}"))?;
        let no = v.get("no").and_then(Value::as_u64).ok_or("a plataforma não devolveu o número do nó")?;
        let nova = {
            let mut c = self.config.lock().map_err(|_| "trava")?;
            c.no = Some(no);
            c.ligado = true;
            c.clone()
        };
        self.gravar(&nova);
        self.aplicar_no_worker();
        self.cutucar();
        Ok(v)
    }

    /// Liga ou pausa o fornecimento. Pausar cancela o que roda.
    pub fn ligar_fornecimento(&self, ligado: bool) {
        let nova = {
            let Ok(mut c) = self.config.lock() else { return };
            c.ligado = ligado;
            c.clone()
        };
        self.gravar(&nova);
        if !ligado {
            self.cancelar_tudo();
        }
        self.aplicar_no_worker();
        self.cutucar();
    }

    /// Muda os limites (valem no próximo pulso).
    pub fn ajustar(&self, linhas: u32, ram_mib: u64, gpu: bool) {
        let nova = {
            let Ok(mut c) = self.config.lock() else { return };
            c.linhas = linhas.clamp(1, 1024);
            c.ram_mib = ram_mib.clamp(256, 1 << 22);
            c.gpu = gpu;
            c.clone()
        };
        self.gravar(&nova);
        self.aplicar_no_worker();
        self.cutucar();
    }

    /// Esquece o vínculo (o computador continua no painel até o dono remover).
    pub fn desvincular(&self) {
        self.cancelar_tudo();
        if let Ok(mut c) = self.config.lock() {
            c.no = None;
            c.ligado = false;
        }
        if let Some(p) = &self.pasta {
            let _ = std::fs::remove_file(p.join(ARQUIVO));
        }
        self.aplicar_no_worker();
    }

    fn cancelar_tudo(&self) {
        if let Ok(mut f) = self.fila.lock() {
            f.clear();
        }
        let ids: Vec<u64> = self.em_curso.lock().map(|e| e.values().map(|a| a.id).collect()).unwrap_or_default();
        if let Ok(mut c) = self.cancelados.lock() {
            c.extend(ids);
        }
    }

    // -----------------------------------------------------------------------
    // laço
    // -----------------------------------------------------------------------

    fn laco(self: Arc<Self>) {
        let mut espera = PULSO;
        loop {
            if self.encerrado.load(Ordering::Relaxed) {
                return;
            }
            let c = self.config();
            if c.ligado && c.no.is_some() && !c.url.is_empty() {
                self.enviar_saida();
                espera = self.pulsar().unwrap_or(PULSO);
                self.buscar_resumo();
            }
            // dorme até o próximo pulso, ou até alguém cutucar (resultado pronto, ajuste)
            if let Ok(mut s) = self.sinal.lock() {
                if !*s {
                    s = self.acordar.wait_timeout(s, espera).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0);
                }
                *s = false;
            }
        }
    }

    fn anotar_erro(&self, e: &str) {
        if let Ok(mut s) = self.situacao.lock() {
            s.conectado = false;
            s.ultimo_erro = e.to_string();
        }
    }

    fn pulsar(&self) -> Option<Duration> {
        let c = self.config();
        let u = self.worker()?;
        let em_curso: Vec<Atribuida> = self.em_curso.lock().map(|e| e.values().cloned().collect()).unwrap_or_default();
        let na_fila: Vec<Atribuida> = self.fila.lock().map(|f| f.iter().cloned().collect()).unwrap_or_default();
        let ocupadas = u32::try_from(em_curso.len().saturating_add(na_fila.len())).unwrap_or(u32::MAX);
        let mem_ocupada: u64 = em_curso.iter().chain(na_fila.iter()).map(|a| a.memoria.div_ceil(1 << 20)).fold(0u64, u64::saturating_add);
        let ativas = u.unidades_ativas();
        let mut progresso = Vec::new();
        for a in em_curso.iter().chain(na_fila.iter()) {
            let viva = ativas.iter().find(|x| x.job == a.job && x.indice == a.unidade);
            progresso.push(json!({ "atribuicao": a.id, "feitas": viva.map_or(0, |v| v.feitas), "total": viva.map_or(0, |v| v.total), "executando": viva.is_some() }));
        }
        let leitura = self.metricas.lock().ok().and_then(|m| m.upgrade()).and_then(|m| {
            let s = m.sistema();
            m.ultima().map(|l| (l, s))
        });
        let tel = leitura.map(|(l, s)| {
            let total = s.ram_total_mib.map(|x| x as f64);
            let usada = total.zip(l.ram_livre_mib).map(|(t, livre)| (t - livre).max(0.0) as u64);
            let watts = self.watts_nucleo.load(Ordering::Relaxed) as f64;
            let potencia = l.cpu_total.map(|cpu| (cpu / 100.0) * f64::from(s.nucleos_logicos) * watts);
            json!({ "cpu": l.cpu_total, "ram_usada_mib": usada, "gpu": l.gpu_calculo, "vram_mib": l.gpu_dedicada_mib.map(|x| x as u64), "temperatura": Value::Null, "potencia_w": potencia, "rede_bytes": self.bytes.swap(0, Ordering::Relaxed) })
        });
        let gpu = c.gpu && u.gpu_ligada.load(Ordering::Relaxed);
        let corpo = json!({
            "versao": self.versao,
            "hardware": self.hardware(),
            "livres": c.linhas.saturating_sub(ocupadas),
            "ram_livre_mib": c.ram_mib.saturating_sub(mem_ocupada),
            "tipos": TipoDeTrabalho::TODOS.iter().map(|t| t.codigo()).collect::<Vec<_>>(),
            "gpu": gpu,
            "pausado": false,
            "progresso": progresso,
            "telemetria": tel.unwrap_or_else(|| json!({})),
        })
        .to_string();
        let r = match self.pedir("POST", "/api/v1/agente/pulso", corpo.as_bytes()) {
            Ok(r) => r,
            Err(e) => {
                self.anotar_erro(&e);
                return None;
            }
        };
        if r.status == 410 {
            // a plataforma não conhece mais este computador
            self.anotar_erro(&erro_http(&r));
            self.desvincular();
            return None;
        }
        if r.status != 200 {
            self.anotar_erro(&erro_http(&r));
            return None;
        }
        let v: Value = serde_json::from_slice(&r.corpo).unwrap_or(Value::Null);
        // parar o que a plataforma mandou parar
        let cancelar: Vec<u64> = v.get("cancelar").and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        if !cancelar.is_empty() {
            if let Ok(mut f) = self.fila.lock() {
                f.retain(|a| !cancelar.contains(&a.id));
            }
            if let Ok(mut c) = self.cancelados.lock() {
                c.extend(cancelar.iter().copied());
            }
        }
        // as unidades novas
        let mut novas = 0usize;
        for a in v.get("atribuicoes").and_then(Value::as_array).cloned().unwrap_or_default() {
            if let Some(x) = ler_atribuicao(&a) {
                let repetida = self.em_curso.lock().is_ok_and(|e| e.values().any(|y| y.id == x.id)) || self.fila.lock().is_ok_and(|f| f.iter().any(|y| y.id == x.id));
                if !repetida && let Ok(mut f) = self.fila.lock() {
                    f.push_back(x);
                    novas = novas.saturating_add(1);
                }
            }
        }
        if novas > 0 {
            u.pedir_trabalho();
        }
        if let Ok(mut s) = self.situacao.lock() {
            s.conectado = true;
            s.ultimo_pulso_ms = agora_ms();
            s.ultimo_erro.clear();
            s.estado_no = v.get("estado").and_then(Value::as_str).unwrap_or("").to_string();
            s.mensagem = v.get("mensagem").and_then(Value::as_str).unwrap_or("").to_string();
            s.atualizar = v.get("atualizar").and_then(Value::as_bool).unwrap_or(false);
        }
        let proximo = v.get("proximo_pulso_ms").and_then(Value::as_u64).unwrap_or(5000).clamp(1000, 60_000);
        Some(Duration::from_millis(proximo))
    }

    fn enviar_saida(&self) {
        loop {
            let Some(s) = self.saida.lock().ok().and_then(|mut f| f.pop_front()) else { return };
            let (caminho, corpo) = match &s {
                Saida::Resultado { corpo, .. } => ("/api/v1/agente/resultado", corpo.clone()),
                Saida::Falha { id, motivo } => ("/api/v1/agente/falha", json!({ "atribuicao": id, "motivo": motivo }).to_string().into_bytes()),
            };
            match self.pedir("POST", caminho, &corpo) {
                Ok(r) => {
                    let aceito = r.status == 200 && serde_json::from_slice::<Value>(&r.corpo).ok().and_then(|v| v.get("aceito").and_then(Value::as_bool)).unwrap_or(false);
                    if let Ok(mut st) = self.situacao.lock()
                        && matches!(s, Saida::Resultado { .. })
                    {
                        st.enviados = st.enviados.saturating_add(1);
                        if aceito {
                            st.aceitos = st.aceitos.saturating_add(1);
                        } else {
                            st.recusados = st.recusados.saturating_add(1);
                        }
                        if r.status >= 400 && r.status != 409 {
                            st.ultimo_erro = erro_http(&r);
                        }
                    }
                }
                Err(e) => {
                    // sem rede: guarda e tenta no próximo pulso
                    if let Ok(mut f) = self.saida.lock() {
                        f.push_front(s);
                    }
                    self.anotar_erro(&e);
                    return;
                }
            }
        }
    }

    fn buscar_resumo(&self) {
        let vencido = self.situacao.lock().map(|s| agora_ms().saturating_sub(s.resumo_ms) > RESUMO_MS).unwrap_or(false);
        if !vencido {
            return;
        }
        if let Ok(r) = self.pedir("GET", "/api/v1/agente/resumo", b"")
            && r.status == 200
            && let Ok(v) = serde_json::from_slice::<Value>(&r.corpo)
            && let Ok(mut s) = self.situacao.lock()
        {
            s.resumo = Some(v);
            s.resumo_ms = agora_ms();
        }
    }

    /// O estado para a tela Fornecer.
    pub fn json(&self) -> Value {
        let c = self.config();
        let s = self.situacao.lock().map(|s| s.clone()).unwrap_or_default();
        let em_curso = self.em_curso.lock().map(|e| e.len()).unwrap_or(0);
        let na_fila = self.fila.lock().map(|f| f.len()).unwrap_or(0);
        json!({
            "url": c.url,
            "vinculado": c.no.is_some(),
            "no": c.no,
            "ligado": c.ligado,
            "linhas": c.linhas,
            "ram_mib": c.ram_mib,
            "gpu": c.gpu,
            "nome": c.nome,
            "chave": hex(&self.chave),
            "conectado": s.conectado,
            "ultimo_pulso_ms": s.ultimo_pulso_ms,
            "ultimo_erro": s.ultimo_erro,
            "estado": s.estado_no,
            "mensagem": s.mensagem,
            "atualizar": s.atualizar,
            "unidades_rodando": em_curso,
            "unidades_na_fila": na_fila,
            "enviados": s.enviados,
            "aceitos": s.aceitos,
            "recusados": s.recusados,
            "resumo": s.resumo,
        })
    }

    // -----------------------------------------------------------------------
    // lado do worker (pelo agendador composto)
    // -----------------------------------------------------------------------

    fn tem(&self, job: &[u8; HASH_LEN], indice: u64) -> bool {
        self.em_curso.lock().is_ok_and(|e| e.contains_key(&(*job, indice)))
    }

    fn proxima(&self, gpu: Option<bool>) -> Option<PedidoDeUnidade> {
        if !self.config().ligado {
            return None;
        }
        let a = {
            let mut f = self.fila.lock().ok()?;
            let agora = agora_ms();
            f.retain(|a| a.prazo_ms > agora);
            let pos = f.iter().position(|a| gpu.is_none_or(|g| a.gpu == g))?;
            f.remove(pos)?
        };
        let pedido = PedidoDeUnidade {
            job: a.job,
            indice: a.unidade,
            especificacao: a.esp,
            semente: a.semente,
            prioridade: PRIORIDADE,
            prazo_ms: a.prazo_ms,
            autoconferir: a.autoconferir,
        };
        if let Ok(mut e) = self.em_curso.lock() {
            e.insert((a.job, a.unidade), a);
        }
        Some(pedido)
    }

    fn terminou(&self, d: &DesfechoDeUnidade) {
        let Some(a) = self.em_curso.lock().ok().and_then(|mut e| e.remove(&(d.job, d.indice))) else { return };
        let cancelada_aqui = self.cancelados.lock().is_ok_and(|mut c| c.remove(&a.id));
        let saida = match (d.estado, &d.resultado, &d.registro, d.assinatura) {
            (Estado::Liquidada, Some(resultado), Some(registro), Some(assinatura)) => {
                let mut w = Writer::new();
                w.u64(a.id);
                let _ = w.var_bytes(&registro.codificar());
                w.fixed(&assinatura);
                w.u64(d.ms_calculo);
                w.u8(u8::from(d.gpu.is_some()));
                if w.var_bytes(resultado).is_err() {
                    Some(Saida::Falha { id: a.id, motivo: "resultado grande demais para enviar".into() })
                } else {
                    Some(Saida::Resultado { corpo: w.into_bytes() })
                }
            }
            // recusada uma vez: o worker repete; só a desistência vai para a plataforma
            (Estado::Recusada, _, _, _) if !d.abandonada => {
                if let Ok(mut e) = self.em_curso.lock() {
                    e.insert((a.job, a.unidade), a);
                }
                None
            }
            _ if cancelada_aqui => None,
            _ => Some(Saida::Falha { id: a.id, motivo: if d.nota.is_empty() { format!("parou em {}", d.estado.nome()) } else { d.nota.clone() } }),
        };
        if let Some(s) = saida {
            if let Ok(mut f) = self.saida.lock() {
                f.push_back(s);
            }
            self.cutucar();
        }
    }

    fn ainda_quer(&self, job: &[u8; HASH_LEN], indice: u64) -> bool {
        let Some(id) = self.em_curso.lock().ok().and_then(|e| e.get(&(*job, indice)).map(|a| a.id)) else { return true };
        self.config().ligado && !self.cancelados.lock().is_ok_and(|c| c.contains(&id))
    }

    fn tem_fila(&self) -> bool {
        self.config().ligado && self.fila.lock().is_ok_and(|f| !f.is_empty())
    }
}

fn ler_atribuicao(v: &Value) -> Option<Atribuida> {
    let esp_b = bytes_de_hex(v.get("especificacao")?.as_str()?)?;
    let mut r = Reader::new(&esp_b);
    let esp = Especificacao::decodificar(&mut r).ok()?;
    r.finish().ok()?;
    let semente: [u8; HASH_LEN] = de_hex(v.get("semente")?.as_str()?)?;
    let job: [u8; HASH_LEN] = de_hex(v.get("job")?.as_str()?)?;
    let tipo = esp.tipo();
    Some(Atribuida {
        id: v.get("id")?.as_u64()?,
        job,
        unidade: v.get("unidade")?.as_u64()?,
        esp,
        semente,
        prazo_ms: v.get("prazo_ms")?.as_u64()?,
        gpu: v.get("gpu").and_then(Value::as_bool).unwrap_or(false),
        memoria: v.get("memoria").and_then(Value::as_u64).unwrap_or_else(|| esp.memoria_bytes()),
        // Freivalds é barato: confere aqui também, e erro de GPU nem sai
        autoconferir: tipo == TipoDeTrabalho::Matriz,
    })
}

/// O agendador do ULTRAX quando há agente: a plataforma primeiro (o dono
/// emprestou esses núcleos), depois os JOBs do próprio dono e os da rede.
pub struct Composto {
    /// A ciência (JOBs locais e da rede P2P).
    pub ciencia: Arc<Ciencia>,
    /// O agente da plataforma.
    pub plataforma: Arc<Agente>,
}

impl Agendador for Composto {
    fn proxima(&self, agora_ms: u64) -> Option<PedidoDeUnidade> {
        self.plataforma.proxima(Some(false)).or_else(|| self.ciencia.proxima(agora_ms))
    }

    fn proxima_do_tipo(&self, agora_ms: u64, tipo: TipoDeTrabalho) -> Option<PedidoDeUnidade> {
        let da_plataforma = if tipo == TipoDeTrabalho::Matriz { self.plataforma.proxima(Some(true)) } else { None };
        da_plataforma.or_else(|| self.ciencia.proxima_do_tipo(agora_ms, tipo))
    }

    fn comecou(&self, job: &[u8; HASH_LEN], indice: u64, linha: u32, entrada: Option<[u8; HASH_LEN]>) {
        if !self.plataforma.tem(job, indice) {
            self.ciencia.comecou(job, indice, linha, entrada);
        }
    }

    fn terminou(&self, desfecho: DesfechoDeUnidade) {
        if self.plataforma.tem(&desfecho.job, desfecho.indice) {
            self.plataforma.terminou(&desfecho);
        } else {
            self.ciencia.terminou(desfecho);
        }
    }

    fn tem_trabalho(&self) -> bool {
        self.plataforma.tem_fila() || self.ciencia.tem_trabalho()
    }

    fn ainda_quer(&self, job: &[u8; HASH_LEN], indice: u64) -> bool {
        if self.plataforma.tem(job, indice) { self.plataforma.ainda_quer(job, indice) } else { self.ciencia.ainda_quer(job, indice) }
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    /// A assinatura do agente bate com a do gabarito Python.
    #[test]
    fn assinatura_igual_ao_gabarito() {
        let caminho: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", "plataforma.json"].iter().collect();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(caminho).unwrap()).unwrap();
        let d = &doc["data"];
        assert_eq!(d["dominio_pedido"].as_str().unwrap().as_bytes(), DOMINIO_PEDIDO);
        let c = &d["pedidos"][0];
        let segredo: [u8; 32] = core::array::from_fn(|i| u8::try_from(i).unwrap());
        let corpo = bytes_de_hex(c["corpo"].as_str().unwrap()).unwrap();
        let a = ed25519_sign(&segredo, &mensagem_do_pedido("POST", c["caminho"].as_str().unwrap(), c["tempo_ms"].as_u64().unwrap(), &corpo));
        assert_eq!(hex(&a), c["assinatura"].as_str().unwrap());
    }

    #[test]
    fn config_ida_e_volta() {
        let c = Config { url: "https://compute.exemplo".into(), no: Some(7), ligado: true, linhas: 3, ram_mib: 4096, gpu: true, nome: "PC da sala".into() };
        let pasta = std::env::temp_dir().join(format!("hyx-agente-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&pasta);
        std::fs::write(pasta.join(ARQUIVO), c.texto()).unwrap();
        assert_eq!(Config::ler(Some(&pasta)), c);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn atribuicao_lida_do_json() {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap();
        let mut w = Writer::new();
        esp.codificar(&mut w);
        let v = json!({ "id": 3, "tarefa": 1, "unidade": 0, "especificacao": hex(&w.into_bytes()), "semente": hex(&[1u8; 64]), "job": hex(&[2u8; 64]), "prazo_ms": 99, "gpu": false, "memoria": 4096 });
        let a = ler_atribuicao(&v).unwrap();
        assert_eq!((a.id, a.unidade, a.esp, a.autoconferir), (3, 0, esp, true));
        let mut ruim = v.clone();
        ruim["semente"] = json!("abc");
        assert!(ler_atribuicao(&ruim).is_none());
    }
}
