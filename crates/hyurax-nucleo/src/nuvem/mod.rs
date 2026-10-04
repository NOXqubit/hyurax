// ✝ Atos 2:44 — “Todos os que criam estavam juntos e tinham tudo em comum.”
//! A nuvem dentro do nó: mercado de máquinas, armazenamento distribuído e
//! livro de contas (`docs/NUVEM.md`). Não há servidor: o "plano de controle"
//! é cada nó conferindo o que recebe.
//!
//! - **Mercado:** cada nó pode anunciar a máquina (assinado pelo worker), e
//!   os anúncios circulam de par em par. Alugar capacidade é um JOB de
//!   verificação por concordância restrito ao fornecedor: cada unidade dele é
//!   refeita aqui antes de contar, e o cliente assina recibos do trabalho
//!   verificado.
//! - **Armazenamento:** o arquivo é cifrado aqui (a chave nunca sai), dividido
//!   em `k + m` fragmentos (Reed–Solomon) e cada fragmento vai para um nó
//!   diferente que ofereceu disco. Desafios de guarda conferem, sem baixar,
//!   que o fragmento continua lá; fragmento perdido é reconstruído a partir
//!   de `k` outros e mandado para outro nó.
//! - **Livro:** consumo, partilha (provedor, plataforma, reserva), recibos e
//!   armazenamento, encadeados por hash.
//!
//! Créditos de computação não são dinheiro nem HYX. Nada aqui paga ninguém.

pub mod armazem;
pub mod livro;
pub mod mercado;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN, ed25519_public_key, sha512};
use hyurax_net::Rede;
use hyurax_nuvem::anuncio::{Anuncio, TipoDeAnuncio};
use hyurax_nuvem::codigo::{codificar, reconstruir};
use hyurax_nuvem::mensagem::{Alvo, MensagemNuvem};
use hyurax_nuvem::recibo::Recibo;
use hyurax_nuvem::{PARTE_MAX, TIPO_NUVEM};
use serde_json::json;

use crate::util::{agora_ms, agora_unix, de_hex, hex};
use armazem::{EstadoDoFragmento, Fragmento, Guarda, Manifesto};
use livro::Livro;
use mercado::{Mercado, Recebido};

/// Arquivo dos ajustes da nuvem, na pasta de configuração.
pub const ARQUIVO_DE_AJUSTES: &str = "nuvem.txt";
/// O anúncio deste nó é refeito a cada tanto (a validade é de uma hora).
pub const REANUNCIAR_S: u64 = 20 * 60;
/// Validade do anúncio deste nó.
pub const VALIDADE_DO_ANUNCIO_S: u32 = 3600;
/// Uma recuperação parada há mais que isso é abandonada.
pub const RECUPERACAO_VALE_S: u64 = 300;
/// Um desafio sem resposta há mais que isso conta como falha.
pub const DESAFIO_RESPOSTA_S: u64 = 60;

/// Os ajustes da nuvem (o dono muda pela tela).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AjustesDaNuvem {
    /// Anunciar esta máquina no mercado.
    pub anunciar: bool,
    /// O que anuncia.
    pub tipo: TipoDeAnuncio,
    /// Preço por crédito verificado, em milicréditos (0: não aluga).
    pub preco_credito_mili: u64,
    /// Preço do armazenamento, milicréditos por GB por mês.
    pub preco_gb_mes_mili: u64,
    /// Preço de venda, em centavos (só no tipo venda).
    pub preco_venda_centavos: u64,
    /// Descrição do anúncio.
    pub descricao: String,
    /// Disco oferecido para guardar fragmentos de outros, em MiB (0: nada).
    pub disco_mib: u64,
    /// Parte do provedor num consumo, em %.
    pub provedor_pct: u8,
    /// Comissão da plataforma num consumo, em %. A reserva é o resto.
    pub plataforma_pct: u8,
    /// Intervalo entre desafios de guarda, em segundos.
    pub desafio_s: u64,
    /// Guardião ausente por mais que isso: o fragmento conta como perdido.
    pub perda_s: u64,
}

impl Default for AjustesDaNuvem {
    fn default() -> Self {
        Self {
            anunciar: false,
            tipo: TipoDeAnuncio::Capacidade,
            preco_credito_mili: 1000,
            preco_gb_mes_mili: 50_000,
            preco_venda_centavos: 0,
            descricao: String::new(),
            disco_mib: 0,
            provedor_pct: 80,
            plataforma_pct: 15,
            desafio_s: 600,
            perda_s: 86_400,
        }
    }
}

impl AjustesDaNuvem {
    /// Lê da pasta de configuração.
    pub fn ler(pasta: &std::path::Path) -> Self {
        let mut a = Self::default();
        let Ok(t) = std::fs::read_to_string(pasta.join(ARQUIVO_DE_AJUSTES)) else { return a };
        for l in t.lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            let v = v.trim();
            let n = v.parse::<u64>().ok();
            match k.trim() {
                "anunciar" => a.anunciar = v == "1",
                "tipo" => a.tipo = v.parse::<u8>().ok().and_then(TipoDeAnuncio::de_codigo).unwrap_or(a.tipo),
                "preco_credito_mili" => a.preco_credito_mili = n.unwrap_or(a.preco_credito_mili),
                "preco_gb_mes_mili" => a.preco_gb_mes_mili = n.unwrap_or(a.preco_gb_mes_mili),
                "preco_venda_centavos" => a.preco_venda_centavos = n.unwrap_or(a.preco_venda_centavos),
                "descricao" => {
                    let b: Vec<u8> = (0..v.len() / 2).filter_map(|i| v.get(i * 2..i * 2 + 2).and_then(|x| u8::from_str_radix(x, 16).ok())).collect();
                    a.descricao = String::from_utf8(b).unwrap_or_default().chars().take(200).collect();
                }
                "disco_mib" => a.disco_mib = n.filter(|x| *x <= 10_000_000).unwrap_or(a.disco_mib),
                "provedor_pct" => a.provedor_pct = n.and_then(|x| u8::try_from(x).ok()).filter(|x| *x <= 100).unwrap_or(a.provedor_pct),
                "plataforma_pct" => a.plataforma_pct = n.and_then(|x| u8::try_from(x).ok()).filter(|x| *x <= 100).unwrap_or(a.plataforma_pct),
                "desafio_s" => a.desafio_s = n.filter(|x| *x >= 1).unwrap_or(a.desafio_s),
                "perda_s" => a.perda_s = n.filter(|x| *x >= 1).unwrap_or(a.perda_s),
                _ => {}
            }
        }
        if u16::from(a.provedor_pct).saturating_add(u16::from(a.plataforma_pct)) > 100 {
            a.provedor_pct = 80;
            a.plataforma_pct = 15;
        }
        a
    }

    /// Grava na pasta de configuração.
    ///
    /// # Errors
    /// Disco.
    pub fn gravar(&self, pasta: &std::path::Path) -> Result<(), String> {
        let t = format!(
            "# Hyurax: ajustes da nuvem (mercado, armazenamento, tarifas). Créditos não são dinheiro.\n\
             anunciar={}\ntipo={}\npreco_credito_mili={}\npreco_gb_mes_mili={}\npreco_venda_centavos={}\ndescricao={}\n\
             disco_mib={}\nprovedor_pct={}\nplataforma_pct={}\ndesafio_s={}\nperda_s={}\n",
            u8::from(self.anunciar),
            self.tipo.codigo(),
            self.preco_credito_mili,
            self.preco_gb_mes_mili,
            self.preco_venda_centavos,
            hex(self.descricao.as_bytes()),
            self.disco_mib,
            self.provedor_pct,
            self.plataforma_pct,
            self.desafio_s,
            self.perda_s
        );
        let _ = std::fs::create_dir_all(pasta);
        crate::arquivos::gravar_atomico(&pasta.join(ARQUIVO_DE_AJUSTES), t.as_bytes())
    }
}

/// O que a máquina tem, para o anúncio (lido das métricas reais).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Maquina {
    /// Linhas de CPU que o ULTRAX usa.
    pub linhas: u16,
    /// Memória total, MiB.
    pub ram_mib: u32,
    /// Nome da GPU.
    pub gpu: String,
    /// Memória da GPU, MiB.
    pub vram_mib: u32,
    /// Créditos por hora do último benchmark (ESTIMADO; 0: sem medida).
    pub creditos_hora: u64,
}

/// Um aluguel de capacidade feito por este nó.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aluguel {
    /// Worker do fornecedor.
    pub fornecedor: [u8; PUBKEY_LEN],
    /// Identidade (Noise) do fornecedor.
    pub identidade: [u8; 32],
    /// Preço do anúncio no momento do aluguel.
    pub preco_credito_mili: u64,
    /// Operações verificadas feitas pelo fornecedor (os créditos saem do
    /// total, não unidade a unidade: uma unidade pequena valeria zero).
    pub operacoes: u64,
    /// Créditos já reconhecidos em recibo.
    pub recibo_mili: u64,
}

impl Aluguel {
    /// Créditos verificados do fornecedor, em milicréditos.
    pub fn creditos(&self) -> u64 {
        hyurax_ultrax::job::milicreditos(self.operacoes, 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fim {
    Baixar,
    Reparar,
    Renovar(u8),
}

struct Recuperacao {
    fim: Fim,
    inicio: u64,
    /// Fragmentos que chegaram e conferiram: índice → bytes.
    prontos: BTreeMap<usize, Vec<u8>>,
    /// Chegando: índice → (guardião, tamanho, hash, bytes).
    chegando: BTreeMap<u8, Chegando>,
    /// Pedidos feitos: índice → guardião.
    pedidos: BTreeMap<u8, [u8; 32]>,
    terminando: bool,
}

/// Quem o núcleo liga à nuvem: avisos, eventos e o que a máquina tem.
pub struct Ganchos {
    /// Registro de texto (categoria, texto).
    pub registrar: Box<dyn Fn(&'static str, String) + Send + Sync>,
    /// Evento para a tela (JSON).
    pub publicar: Box<dyn Fn(String) + Send + Sync>,
    /// O que a máquina tem agora.
    pub maquina: Box<dyn Fn() -> Maquina + Send + Sync>,
}

type Consumos = Box<dyn Fn() -> Vec<([u8; HASH_LEN], u32, u64)> + Send + Sync>;
type DesafioNoAr = BTreeMap<([u8; 32], Alvo), ([u8; 32], [u8; HASH_LEN], u64)>;
/// Um fragmento chegando: (guardião, tamanho, hash, bytes até aqui).
type Chegando = ([u8; 32], u32, [u8; HASH_LEN], Vec<u8>);
/// Um fragmento a mandar: (guardião, alvo, tamanho, hash, bytes).
type Envio = ([u8; 32], Alvo, u32, [u8; HASH_LEN], Vec<u8>);
/// Um desafio a mandar: (guardião, alvo, nonce, prova esperada).
type Desafiar = ([u8; 32], Alvo, [u8; 32], [u8; HASH_LEN]);
/// Recibos já lançados: (JOB, cliente) → total.
type RecibosLancados = BTreeMap<([u8; HASH_LEN], [u8; 32]), u64>;
/// A reputação de um worker: (nota 0–1000, classe).
pub type Reputacao<'a> = &'a dyn Fn(&[u8; 32]) -> Option<(u32, String)>;

/// A nuvem deste nó.
pub struct Nuvem {
    pasta: PathBuf,
    pasta_config: PathBuf,
    segredo: [u8; SECRET_LEN],
    /// A chave do worker deste nó (quem assina anúncios e recibos).
    pub worker: [u8; PUBKEY_LEN],
    /// A identidade deste nó na rede (Noise).
    pub identidade: [u8; 32],
    rede: Mutex<Option<Arc<Rede>>>,
    /// Os ajustes.
    pub ajustes: Mutex<AjustesDaNuvem>,
    /// O mercado.
    pub mercado: Mutex<Mercado>,
    /// O que este nó guarda para outros.
    pub guarda: Mutex<Guarda>,
    manifestos: Mutex<BTreeMap<[u8; 32], Manifesto>>,
    manifestos_sujos: AtomicBool,
    recuperacoes: Mutex<BTreeMap<[u8; 32], Recuperacao>>,
    desafios_no_ar: Mutex<DesafioNoAr>,
    /// Última notícia de cada arquivo (para a tela).
    pub noticias: Mutex<BTreeMap<[u8; 32], String>>,
    /// O livro.
    pub livro: Livro,
    alugueis: Mutex<BTreeMap<[u8; HASH_LEN], Aluguel>>,
    recibos_lancados: Mutex<RecibosLancados>,
    ultimo_anuncio: AtomicU64,
    ultima_liquidacao: AtomicU64,
    ultimo_recibo: AtomicU64,
    ganchos: Ganchos,
    consumos: Mutex<Option<Consumos>>,
}

fn arquivo_de_hex(t: &str) -> Option<[u8; 32]> {
    de_hex::<32>(t.trim())
}

fn nome_seguro(nome: &str) -> String {
    let n: String = nome.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).take(100).collect();
    let n = n.trim_start_matches('.').to_string();
    if n.is_empty() { "arquivo".into() } else { n }
}

impl Nuvem {
    /// Abre a nuvem: lê ajustes, guarda, manifestos, aluguéis e livro.
    pub fn abrir(pasta_dados: &std::path::Path, pasta_config: &std::path::Path, segredo_do_worker: [u8; SECRET_LEN], identidade: [u8; 32], ganchos: Ganchos) -> Arc<Self> {
        let pasta = pasta_dados.join("nuvem");
        let ajustes = AjustesDaNuvem::ler(pasta_config);
        let guarda = Guarda::abrir(pasta.join("guarda"), ajustes.disco_mib);
        let mut manifestos = BTreeMap::new();
        if let Ok(lista) = std::fs::read_dir(pasta.join("arquivos")) {
            for e in lista.flatten() {
                if let Some(m) = std::fs::read_to_string(e.path()).ok().and_then(|t| Manifesto::ler(&t)) {
                    manifestos.insert(m.arquivo, m);
                }
            }
        }
        let alugueis = ler_alugueis(&pasta.join("alugueis.txt"));
        Arc::new(Self {
            livro: Livro::abrir(Some(pasta.join("livro.txt"))),
            pasta,
            pasta_config: pasta_config.to_path_buf(),
            segredo: segredo_do_worker,
            worker: ed25519_public_key(&segredo_do_worker),
            identidade,
            rede: Mutex::new(None),
            ajustes: Mutex::new(ajustes),
            mercado: Mutex::new(Mercado::default()),
            guarda: Mutex::new(guarda),
            manifestos: Mutex::new(manifestos),
            manifestos_sujos: AtomicBool::new(false),
            recuperacoes: Mutex::new(BTreeMap::new()),
            desafios_no_ar: Mutex::new(BTreeMap::new()),
            noticias: Mutex::new(BTreeMap::new()),
            alugueis: Mutex::new(alugueis),
            recibos_lancados: Mutex::new(BTreeMap::new()),
            ultimo_anuncio: AtomicU64::new(0),
            ultima_liquidacao: AtomicU64::new(0),
            ultimo_recibo: AtomicU64::new(0),
            ganchos,
            consumos: Mutex::new(None),
        })
    }

    /// Quem informa o consumo dos JOBs ao livro: (JOB, conta, milicréditos).
    pub fn ao_liquidar(&self, f: Consumos) {
        if let Ok(mut c) = self.consumos.lock() {
            *c = Some(f);
        }
    }

    /// Liga à rede: registra o tratador e começa a manutenção (a cada 2 s).
    pub fn ligar_rede(self: &Arc<Self>, rede: &Arc<Rede>) {
        if let Ok(mut r) = self.rede.lock() {
            *r = Some(Arc::clone(rede));
        }
        let fraco = Arc::downgrade(self);
        rede.ao_receber_tipo(
            TIPO_NUVEM,
            Arc::new(move |par, identidade, corpo| {
                if let Some(n) = fraco.upgrade() {
                    n.receber(par, identidade, corpo);
                }
            }),
        );
        let fraco = Arc::downgrade(self);
        std::thread::spawn(move || manter(&fraco));
    }

    fn rede(&self) -> Option<Arc<Rede>> {
        self.rede.lock().ok().and_then(|r| r.clone())
    }

    fn par_de(&self, identidade: &[u8; 32]) -> Option<u64> {
        self.rede()?.pares_com_identidade().into_iter().find(|(_, i)| i == identidade).map(|(p, _)| p)
    }

    fn enviar(&self, par: u64, m: &MensagemNuvem) -> bool {
        match (self.rede(), m.codificar()) {
            (Some(r), Ok(corpo)) => r.enviar_tipo(par, TIPO_NUVEM, corpo),
            _ => false,
        }
    }

    /// Manda esperando a fila do par andar (para os pedaços grandes).
    fn enviar_com_espera(&self, identidade: &[u8; 32], m: &MensagemNuvem) -> bool {
        for _ in 0..1500 {
            let Some(par) = self.par_de(identidade) else { return false };
            if self.enviar(par, m) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Manda um fragmento inteiro: cabeçalho e pedaços.
    fn mandar_fragmento(&self, identidade: &[u8; 32], primeira: &MensagemNuvem, alvo: Alvo, bytes: &[u8]) -> bool {
        if !self.enviar_com_espera(identidade, primeira) {
            return false;
        }
        for (i, pedaco) in bytes.chunks(PARTE_MAX).enumerate() {
            let deslocamento = u32::try_from(i.saturating_mul(PARTE_MAX)).unwrap_or(u32::MAX);
            if !self.enviar_com_espera(identidade, &MensagemNuvem::Parte { alvo, deslocamento, dados: pedaco.to_vec() }) {
                return false;
            }
        }
        true
    }

    fn noticia(&self, arquivo: &[u8; 32], texto: String) {
        self.ganchos.publicar.as_ref()(json!({ "arquivo": hex(arquivo), "noticia": texto }).to_string());
        if let Ok(mut n) = self.noticias.lock() {
            n.insert(*arquivo, texto);
        }
    }

    // -------------------------------------------------------------- receber

    fn receber(self: &Arc<Self>, par: u64, identidade: [u8; 32], corpo: &[u8]) {
        let Ok(m) = MensagemNuvem::decodificar(corpo) else { return };
        let agora = agora_unix();
        match m {
            MensagemNuvem::Anuncio(a) => {
                let resultado = self.mercado.lock().map(|mut mc| mc.receber((*a).clone(), par, &self.worker, agora_ms()));
                if resultado.is_ok_and(|r| r == Recebido::Novo) {
                    if let (Some(r), Ok(c)) = (self.rede(), MensagemNuvem::Anuncio(a).codificar()) {
                        r.difundir_tipo(TIPO_NUVEM, &c, Some(par));
                    }
                    self.ganchos.publicar.as_ref()(json!({ "mercado": "anuncio" }).to_string());
                }
            }
            MensagemNuvem::Guardar { alvo, tamanho, hash } => {
                let resposta = self.guarda.lock().ok().and_then(|mut g| g.guardar(identidade, alvo, tamanho, hash, agora));
                if let Some(r) = resposta {
                    self.enviar(par, &r);
                }
            }
            MensagemNuvem::Parte { alvo, deslocamento, dados } => {
                // pedaço de um fragmento que este nó está guardando para outro,
                // ou de um fragmento deste nó voltando de um guardião
                if self.guarda.lock().is_ok_and(|g| g.esperando(&identidade, &alvo)) {
                    let resposta = self.guarda.lock().ok().and_then(|mut g| g.parte(identidade, alvo, deslocamento, &dados));
                    if let Some(r) = resposta {
                        if matches!(r, MensagemNuvem::Guardado { ok: true, .. }) {
                            (self.ganchos.registrar)("nuvem", format!("fragmento guardado para o nó {}… (cifrado; este nó não tem a chave)", hex(identidade.get(..6).unwrap_or_default())));
                        }
                        self.enviar(par, &r);
                    }
                } else {
                    self.chegou_parte(identidade, alvo, deslocamento, &dados);
                }
            }
            MensagemNuvem::Guardado { alvo, ok, motivo } => self.chegou_guardado(identidade, alvo, ok, &motivo),
            MensagemNuvem::Desafio { alvo, nonce } => {
                let n = Arc::clone(self);
                std::thread::spawn(move || {
                    let resposta = n.guarda.lock().map(|g| g.provar(&identidade, alvo, nonce));
                    if let Ok(r) = resposta {
                        n.enviar(par, &r);
                    }
                });
            }
            MensagemNuvem::Prova { alvo, nonce, h } => self.chegou_prova(identidade, alvo, nonce, h, agora),
            MensagemNuvem::Buscar(alvo) => {
                let n = Arc::clone(self);
                std::thread::spawn(move || {
                    let bytes = n.guarda.lock().ok().and_then(|g| g.ler(&identidade, &alvo));
                    match bytes {
                        Some(b) => {
                            let cab = MensagemNuvem::Entrega { alvo, tem: true, tamanho: u32::try_from(b.len()).unwrap_or(0), hash: sha512(&b) };
                            n.mandar_fragmento(&identidade, &cab, alvo, &b);
                        }
                        None => {
                            n.enviar(par, &MensagemNuvem::Entrega { alvo, tem: false, tamanho: 0, hash: [0; HASH_LEN] });
                        }
                    }
                });
            }
            MensagemNuvem::Entrega { alvo, tem, tamanho, hash } => self.chegou_entrega(identidade, alvo, tem, tamanho, hash),
            MensagemNuvem::Apagar(alvo) => {
                if let Ok(mut g) = self.guarda.lock() {
                    g.apagar(&identidade, &alvo);
                }
            }
            MensagemNuvem::Recibo(r) => self.chegou_recibo(&r, agora),
        }
    }

    fn chegou_guardado(&self, guardiao: [u8; 32], alvo: Alvo, ok: bool, motivo: &str) {
        let mut noticia = None;
        if let Ok(mut ms) = self.manifestos.lock()
            && let Some(m) = ms.get_mut(&alvo.arquivo)
            && let Some(f) = m.fragmentos.iter_mut().find(|f| f.indice == alvo.indice && f.guardiao == guardiao)
        {
            if ok && f.estado == EstadoDoFragmento::Enviando {
                f.estado = EstadoDoFragmento::Guardado;
                f.ultimo_ok = agora_unix();
                f.pago_ate = f.ultimo_ok;
                self.manifestos_sujos.store(true, Ordering::Relaxed);
                if m.saude() == "integro" {
                    noticia = Some(format!("\"{}\" guardado em {} nós ({} de dados + {} de paridade)", m.nome, m.fragmentos.len(), m.k, m.m));
                }
            } else if !ok {
                f.estado = EstadoDoFragmento::Perdido;
                self.manifestos_sujos.store(true, Ordering::Relaxed);
                noticia = Some(format!("o nó {}… recusou ou perdeu o fragmento {} de \"{}\": {motivo}", hex(guardiao.get(..6).unwrap_or_default()), alvo.indice, m.nome));
            }
        }
        if let Some(t) = noticia {
            self.noticia(&alvo.arquivo, t);
        }
    }

    fn chegou_prova(&self, guardiao: [u8; 32], alvo: Alvo, nonce: [u8; 32], h: [u8; HASH_LEN], agora: u64) {
        let Some((n_esperado, esperado, _)) = self.desafios_no_ar.lock().ok().and_then(|mut d| d.remove(&(guardiao, alvo))) else { return };
        let boa = n_esperado == nonce && esperado == h;
        let mut lancar = None;
        if let Ok(mut ms) = self.manifestos.lock()
            && let Some(m) = ms.get_mut(&alvo.arquivo)
            && let Some(f) = m.fragmentos.iter_mut().find(|f| f.indice == alvo.indice && f.guardiao == guardiao)
        {
            if boa {
                f.ultimo_ok = agora;
                f.falhas = 0;
                // armazenamento: bytes · preço por GB-mês · tempo
                let dt = agora.saturating_sub(f.pago_ate);
                let valor = u128::from(f.tamanho).saturating_mul(u128::from(f.preco_gb_mes_mili)).saturating_mul(u128::from(dt)) / (1_000_000_000u128 * 2_592_000);
                if valor > 0 {
                    f.pago_ate = agora;
                    lancar = Some((guardiao, u64::try_from(valor).unwrap_or(u64::MAX), m.nome.clone()));
                }
            } else {
                f.falhas = f.falhas.saturating_add(1);
                if f.falhas >= 2 {
                    f.estado = EstadoDoFragmento::Perdido;
                }
            }
            self.manifestos_sujos.store(true, Ordering::Relaxed);
        }
        if let Some((g, valor, nome)) = lancar {
            let _ = self.livro.lancar(agora, "consumo", 0, [0; HASH_LEN], g, valor, &format!("armazenamento: {}", nome.chars().take(80).collect::<String>()));
        }
        if !boa {
            self.noticia(&alvo.arquivo, format!("prova de guarda errada do nó {}… (fragmento {})", hex(guardiao.get(..6).unwrap_or_default()), alvo.indice));
        }
    }

    fn chegou_entrega(self: &Arc<Self>, guardiao: [u8; 32], alvo: Alvo, tem: bool, tamanho: u32, hash: [u8; HASH_LEN]) {
        let esperado = self.manifestos.lock().ok().and_then(|ms| {
            ms.get(&alvo.arquivo).and_then(|m| m.fragmentos.iter().find(|f| f.indice == alvo.indice).map(|f| (f.hash, f.tamanho)))
        });
        let Ok(mut rs) = self.recuperacoes.lock() else { return };
        let Some(r) = rs.get_mut(&alvo.arquivo) else { return };
        if r.pedidos.get(&alvo.indice) != Some(&guardiao) {
            return;
        }
        if tem && esperado == Some((hash, tamanho)) {
            r.chegando.insert(alvo.indice, (guardiao, tamanho, hash, Vec::new()));
        } else {
            r.pedidos.remove(&alvo.indice);
            drop(rs);
            self.marcar_perdido(&alvo, &guardiao, "o guardião não tem mais o fragmento");
        }
    }

    fn chegou_parte(self: &Arc<Self>, guardiao: [u8; 32], alvo: Alvo, deslocamento: u32, dados: &[u8]) {
        let pronto = {
            let Ok(mut rs) = self.recuperacoes.lock() else { return };
            let Some(r) = rs.get_mut(&alvo.arquivo) else { return };
            let Some((g, tamanho, hash, buf)) = r.chegando.get_mut(&alvo.indice) else { return };
            if *g != guardiao || deslocamento as usize != buf.len() || buf.len().saturating_add(dados.len()) > *tamanho as usize {
                r.chegando.remove(&alvo.indice);
                r.pedidos.remove(&alvo.indice);
                return;
            }
            buf.extend_from_slice(dados);
            if buf.len() < *tamanho as usize {
                return;
            }
            let ok = sha512(buf) == *hash;
            if let Some((_, _, _, bytes)) = r.chegando.remove(&alvo.indice)
                && ok
            {
                r.prontos.insert(usize::from(alvo.indice), bytes);
            }
            r.pedidos.remove(&alvo.indice);
            ok
        };
        if !pronto {
            self.marcar_perdido(&alvo, &guardiao, "o fragmento chegou com hash errado");
        }
        self.tentar_terminar(alvo.arquivo);
    }

    fn chegou_recibo(&self, r: &Recibo, agora: u64) {
        if r.fornecedor != self.worker || !r.confere() {
            return;
        }
        let delta = {
            let Ok(mut l) = self.recibos_lancados.lock() else { return };
            let ja = l.entry((r.job, r.cliente)).or_insert(0);
            let d = r.total_mili.saturating_sub(*ja);
            *ja = (*ja).max(r.total_mili);
            d
        };
        if delta > 0 {
            let _ = self.livro.lancar(agora, "recibo", 0, r.job, r.cliente, delta, "a receber: recibo assinado pelo cliente");
            (self.ganchos.registrar)("nuvem", format!("recibo de aluguel: +{} milicréditos de {}…", delta, hex(r.cliente.get(..6).unwrap_or_default())));
        }
    }

    fn marcar_perdido(&self, alvo: &Alvo, guardiao: &[u8; 32], motivo: &str) {
        let nome = {
            let Ok(mut ms) = self.manifestos.lock() else { return };
            let Some(m) = ms.get_mut(&alvo.arquivo) else { return };
            if let Some(f) = m.fragmentos.iter_mut().find(|f| f.indice == alvo.indice && f.guardiao == *guardiao) {
                f.estado = EstadoDoFragmento::Perdido;
            }
            self.manifestos_sujos.store(true, Ordering::Relaxed);
            m.nome.clone()
        };
        self.noticia(&alvo.arquivo, format!("fragmento {} de \"{nome}\" perdido: {motivo}", alvo.indice));
    }

    // ------------------------------------------------------------ guardar

    /// Os nós conectados que oferecem disco, do mais barato ao mais caro:
    /// (identidade, preço por GB-mês).
    fn guardioes_disponiveis(&self, tamanho_do_fragmento: u64, excluir: &BTreeSet<[u8; 32]>) -> Vec<([u8; 32], u64)> {
        let conectados: BTreeSet<[u8; 32]> = self.rede().map(|r| r.pares_com_identidade().into_iter().map(|(_, i)| i).collect()).unwrap_or_default();
        let agora = agora_ms();
        let mut v: Vec<([u8; 32], u64)> = self
            .mercado
            .lock()
            .map(|m| {
                m.anuncios
                    .values()
                    .filter(|a| a.vale_em(agora) && a.tipo != TipoDeAnuncio::Venda)
                    .filter(|a| a.disco_mib.saturating_mul(1024 * 1024) >= tamanho_do_fragmento)
                    .filter(|a| conectados.contains(&a.identidade) && !excluir.contains(&a.identidade) && a.identidade != self.identidade)
                    .map(|a| (a.identidade, a.preco_gb_mes_mili))
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|(i, p)| (*p, *i));
        v.dedup_by_key(|(i, _)| *i);
        v
    }

    /// Guarda um arquivo na rede: cifra aqui, divide em `k + m` fragmentos e
    /// manda cada um para um nó diferente. Devolve o identificador.
    ///
    /// # Errors
    /// Arquivo grande demais, `k`/`m` fora da faixa, nós guardiões
    /// insuficientes ou sem entropia.
    pub fn guardar_arquivo(self: &Arc<Self>, nome: &str, dados: &[u8], k: u8, m: u8) -> Result<[u8; 32], String> {
        if dados.len() > armazem::ARQUIVO_MAX {
            return Err(format!("arquivo grande demais: o máximo é {} MiB", armazem::ARQUIVO_MAX / (1024 * 1024)));
        }
        let total = usize::from(k).saturating_add(usize::from(m));
        let mut semente = [0u8; 32];
        hyurax_net::entropia::preencher(&mut semente)?;
        let derivar = |rotulo: &[u8]| -> [u8; 32] {
            let mut x = semente.to_vec();
            x.extend_from_slice(rotulo);
            let h = sha512(&x);
            let mut o = [0u8; 32];
            o.copy_from_slice(h.get(..32).unwrap_or(&[0; 32]));
            o
        };
        let arquivo = derivar(b"arquivo");
        let chave = derivar(b"chave");
        let cifrado = armazem::cifrar(&chave, &arquivo, dados)?;
        let fragmentos = codificar(&cifrado, usize::from(k), usize::from(m)).map_err(|e| e.to_string())?;
        let tamanho_frag = fragmentos.first().map_or(0, Vec::len) as u64;
        let guardioes = self.guardioes_disponiveis(tamanho_frag, &BTreeSet::new());
        if guardioes.len() < total {
            return Err(format!(
                "precisa de {total} nós guardiões conectados que ofereçam ao menos {} KiB de disco; há {} agora",
                tamanho_frag.div_ceil(1024),
                guardioes.len()
            ));
        }
        let semente_desafios = semente_dos_desafios(&chave);
        let agora = agora_unix();
        let mut man = Manifesto {
            arquivo,
            nome: nome.chars().take(200).collect(),
            tamanho: dados.len() as u64,
            cifrado: cifrado.len() as u64,
            k,
            m,
            chave,
            criado: agora,
            fragmentos: Vec::new(),
        };
        for ((i, f), (g, preco)) in fragmentos.iter().enumerate().zip(guardioes.iter()) {
            let indice = u8::try_from(i).map_err(|_| "índice")?;
            man.fragmentos.push(Fragmento {
                indice,
                tamanho: u32::try_from(f.len()).map_err(|_| "fragmento grande demais")?,
                hash: sha512(f),
                guardiao: *g,
                estado: EstadoDoFragmento::Enviando,
                ultimo_ok: 0,
                ausente_desde: 0,
                falhas: 0,
                desafios: armazem::desafios(&semente_desafios, &arquivo, indice, 0, f),
                preco_gb_mes_mili: *preco,
                pago_ate: agora,
            });
        }
        let envios: Vec<Envio> = man
            .fragmentos
            .iter()
            .zip(fragmentos)
            .map(|(f, bytes)| (f.guardiao, Alvo { arquivo, indice: f.indice }, f.tamanho, f.hash, bytes))
            .collect();
        self.gravar_manifesto(&man)?;
        if let Ok(mut ms) = self.manifestos.lock() {
            ms.insert(arquivo, man);
        }
        (self.ganchos.registrar)("nuvem", format!("guardando \"{}\" ({} bytes) em {total} nós: {k} fragmentos de dados + {m} de paridade, cifrado aqui", nome, dados.len()));
        for (g, alvo, tamanho, hash, bytes) in envios {
            let n = Arc::clone(self);
            std::thread::spawn(move || {
                if !n.mandar_fragmento(&g, &MensagemNuvem::Guardar { alvo, tamanho, hash }, alvo, &bytes) {
                    n.marcar_perdido(&alvo, &g, "não consegui mandar (o nó saiu da rede?)");
                }
            });
        }
        Ok(arquivo)
    }

    fn gravar_manifesto(&self, m: &Manifesto) -> Result<(), String> {
        let pasta = self.pasta.join("arquivos");
        std::fs::create_dir_all(&pasta).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
        crate::arquivos::gravar_privado(&pasta.join(format!("{}.txt", hex(&m.arquivo))), &m.texto())
    }

    fn gravar_manifestos_sujos(&self) {
        if !self.manifestos_sujos.swap(false, Ordering::Relaxed) {
            return;
        }
        let copia: Vec<Manifesto> = self.manifestos.lock().map(|m| m.values().cloned().collect()).unwrap_or_default();
        for m in &copia {
            if self.gravar_manifesto(m).is_err() {
                self.manifestos_sujos.store(true, Ordering::Relaxed);
            }
        }
    }

    // ---------------------------------------------------------- recuperar

    /// Começa a recuperar um arquivo: pede os fragmentos aos guardiões; com
    /// `k` conferidos, reconstrói, decifra e grava em `nuvem/recuperados/`.
    ///
    /// # Errors
    /// Arquivo desconhecido, ou já recuperando.
    pub fn recuperar(self: &Arc<Self>, arquivo: &[u8; 32]) -> Result<(), String> {
        self.comecar_recuperacao(arquivo, Fim::Baixar)
    }

    fn comecar_recuperacao(self: &Arc<Self>, arquivo: &[u8; 32], fim: Fim) -> Result<(), String> {
        let man = self.manifestos.lock().ok().and_then(|m| m.get(arquivo).cloned()).ok_or("arquivo desconhecido")?;
        {
            let mut rs = self.recuperacoes.lock().map_err(|_| "travado")?;
            if rs.contains_key(arquivo) {
                return Err("já estou recuperando este arquivo".into());
            }
            rs.insert(*arquivo, Recuperacao { fim, inicio: agora_unix(), prontos: BTreeMap::new(), chegando: BTreeMap::new(), pedidos: BTreeMap::new(), terminando: false });
        }
        let alvos: Vec<&Fragmento> = match fim {
            Fim::Renovar(i) => man.fragmentos.iter().filter(|f| f.indice == i && f.estado == EstadoDoFragmento::Guardado).collect(),
            _ => man.fragmentos.iter().filter(|f| f.estado == EstadoDoFragmento::Guardado).collect(),
        };
        let mut pedidos = 0usize;
        for f in alvos {
            let alvo = Alvo { arquivo: *arquivo, indice: f.indice };
            let Some(par) = self.par_de(&f.guardiao) else { continue };
            if let Ok(mut rs) = self.recuperacoes.lock()
                && let Some(r) = rs.get_mut(arquivo)
            {
                r.pedidos.insert(f.indice, f.guardiao);
            }
            if self.enviar(par, &MensagemNuvem::Buscar(alvo)) {
                pedidos = pedidos.saturating_add(1);
            }
        }
        let minimo = if matches!(fim, Fim::Renovar(_)) { 1 } else { usize::from(man.k) };
        if pedidos < minimo {
            if let Ok(mut rs) = self.recuperacoes.lock() {
                rs.remove(arquivo);
            }
            return Err(format!("só {pedidos} guardião(ões) conectado(s); precisa de {minimo}"));
        }
        if fim == Fim::Baixar {
            self.noticia(arquivo, format!("recuperando \"{}\": pedi {pedidos} fragmento(s)", man.nome));
        }
        Ok(())
    }

    fn tentar_terminar(self: &Arc<Self>, arquivo: [u8; 32]) {
        let Some(man) = self.manifestos.lock().ok().and_then(|m| m.get(&arquivo).cloned()) else { return };
        let (fim, prontos) = {
            let Ok(mut rs) = self.recuperacoes.lock() else { return };
            let Some(r) = rs.get_mut(&arquivo) else { return };
            let precisa = if matches!(r.fim, Fim::Renovar(_)) { 1 } else { usize::from(man.k) };
            if r.terminando || r.prontos.len() < precisa {
                return;
            }
            r.terminando = true;
            (r.fim, std::mem::take(&mut r.prontos))
        };
        let n = Arc::clone(self);
        std::thread::spawn(move || {
            let r = n.terminar(&man, fim, &prontos);
            if let Ok(mut rs) = n.recuperacoes.lock() {
                rs.remove(&arquivo);
            }
            match r {
                Ok(texto) => n.noticia(&arquivo, texto),
                Err(e) => n.noticia(&arquivo, format!("falhou: {e}")),
            }
        });
    }

    fn terminar(self: &Arc<Self>, man: &Manifesto, fim: Fim, prontos: &BTreeMap<usize, Vec<u8>>) -> Result<String, String> {
        let k = usize::from(man.k);
        let m = usize::from(man.m);
        let agora = agora_unix();
        if let Fim::Renovar(indice) = fim {
            let bytes = prontos.get(&usize::from(indice)).ok_or("fragmento não chegou")?;
            let novos = armazem::desafios(&semente_dos_desafios(&man.chave), &man.arquivo, indice, u32::try_from(agora).unwrap_or(0), bytes);
            if let Ok(mut ms) = self.manifestos.lock()
                && let Some(mm) = ms.get_mut(&man.arquivo)
                && let Some(f) = mm.fragmentos.iter_mut().find(|f| f.indice == indice)
            {
                f.desafios = novos;
                f.ultimo_ok = agora;
                f.falhas = 0;
            }
            self.manifestos_sujos.store(true, Ordering::Relaxed);
            return Ok(format!("fragmento {indice} de \"{}\" conferido inteiro (hash) e desafios renovados", man.nome));
        }
        let cifrado = reconstruir(prontos, k, m, usize::try_from(man.cifrado).unwrap_or(0)).map_err(|e| e.to_string())?;
        let dados = armazem::decifrar(&man.chave, &man.arquivo, &cifrado)?;
        match fim {
            Fim::Baixar => {
                let pasta = self.pasta.join("recuperados");
                std::fs::create_dir_all(&pasta).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
                let destino = pasta.join(nome_seguro(&man.nome));
                crate::arquivos::gravar_atomico(&destino, &dados)?;
                Ok(format!("\"{}\" recuperado ({} bytes) a partir de {} fragmentos conferidos, em {}", man.nome, dados.len(), prontos.len(), destino.display()))
            }
            Fim::Reparar => {
                let todos = codificar(&cifrado, k, m).map_err(|e| e.to_string())?;
                let ocupados: BTreeSet<[u8; 32]> = man.fragmentos.iter().filter(|f| f.estado != EstadoDoFragmento::Perdido).map(|f| f.guardiao).collect();
                let perdidos: Vec<u8> = man.fragmentos.iter().filter(|f| f.estado == EstadoDoFragmento::Perdido).map(|f| f.indice).collect();
                let tamanho = todos.first().map_or(0, Vec::len) as u64;
                let novos = self.guardioes_disponiveis(tamanho, &ocupados);
                let mut reenviados = 0usize;
                for (indice, (g, preco)) in perdidos.iter().zip(novos.iter()) {
                    let Some(bytes) = todos.get(usize::from(*indice)) else { continue };
                    let alvo = Alvo { arquivo: man.arquivo, indice: *indice };
                    let tamanho = u32::try_from(bytes.len()).unwrap_or(0);
                    let hash = sha512(bytes);
                    if let Ok(mut ms) = self.manifestos.lock()
                        && let Some(mm) = ms.get_mut(&man.arquivo)
                        && let Some(f) = mm.fragmentos.iter_mut().find(|f| f.indice == *indice)
                    {
                        f.guardiao = *g;
                        f.estado = EstadoDoFragmento::Enviando;
                        f.falhas = 0;
                        f.ausente_desde = 0;
                        f.preco_gb_mes_mili = *preco;
                        f.pago_ate = agora;
                        f.desafios = armazem::desafios(&semente_dos_desafios(&man.chave), &man.arquivo, *indice, u32::try_from(agora).unwrap_or(0), bytes);
                    }
                    self.manifestos_sujos.store(true, Ordering::Relaxed);
                    if self.mandar_fragmento(g, &MensagemNuvem::Guardar { alvo, tamanho, hash }, alvo, bytes) {
                        reenviados = reenviados.saturating_add(1);
                    } else {
                        self.marcar_perdido(&alvo, g, "não consegui mandar o fragmento reconstruído");
                    }
                }
                if reenviados == 0 {
                    return Err(format!("reconstruí \"{}\", mas não há nó guardião livre para os {} fragmento(s) perdido(s)", man.nome, perdidos.len()));
                }
                Ok(format!("reparo de \"{}\": {reenviados} fragmento(s) reconstruído(s) a partir de {k} e mandado(s) para outro(s) nó(s)", man.nome))
            }
            Fim::Renovar(_) => Ok(String::new()),
        }
    }

    /// Apaga um arquivo: pede aos guardiões para apagar e esquece o manifesto.
    ///
    /// # Errors
    /// Arquivo desconhecido.
    pub fn apagar_arquivo(&self, arquivo: &[u8; 32]) -> Result<(), String> {
        let man = self.manifestos.lock().ok().and_then(|mut m| m.remove(arquivo)).ok_or("arquivo desconhecido")?;
        for f in &man.fragmentos {
            if let Some(par) = self.par_de(&f.guardiao) {
                self.enviar(par, &MensagemNuvem::Apagar(Alvo { arquivo: *arquivo, indice: f.indice }));
            }
        }
        let _ = std::fs::remove_file(self.pasta.join("arquivos").join(format!("{}.txt", hex(arquivo))));
        Ok(())
    }

    /// O arquivo recuperado, se já está na pasta.
    pub fn recuperado(&self, arquivo: &[u8; 32]) -> Option<(String, Vec<u8>)> {
        let nome = self.manifestos.lock().ok()?.get(arquivo)?.nome.clone();
        let bytes = std::fs::read(self.pasta.join("recuperados").join(nome_seguro(&nome))).ok()?;
        Some((nome, bytes))
    }

    // ----------------------------------------------------------- aluguel

    /// Registra um aluguel: o JOB `job` só manda unidades para o fornecedor
    /// do anúncio de `worker` (quem chama restringe o JOB na ciência).
    ///
    /// # Errors
    /// Anúncio desconhecido, vencido, de venda ou sem preço.
    pub fn anuncio_para_alugar(&self, worker: &[u8; 32]) -> Result<Anuncio, String> {
        let a = self.mercado.lock().ok().and_then(|m| m.anuncios.get(worker).cloned()).ok_or("anúncio desconhecido")?;
        if !a.vale_em(agora_ms()) {
            return Err("o anúncio venceu".into());
        }
        if a.tipo == TipoDeAnuncio::Venda || a.preco_credito_mili == 0 {
            return Err("este anúncio não aluga capacidade".into());
        }
        if self.par_de(&a.identidade).is_none() {
            return Err("o nó do anúncio não está conectado agora".into());
        }
        Ok(a)
    }

    /// Guarda o aluguel do JOB.
    pub fn registrar_aluguel(&self, job: [u8; HASH_LEN], a: &Anuncio) {
        if let Ok(mut al) = self.alugueis.lock() {
            al.insert(job, Aluguel { fornecedor: a.worker, identidade: a.identidade, preco_credito_mili: a.preco_credito_mili, operacoes: 0, recibo_mili: 0 });
        }
        self.gravar_alugueis();
    }

    /// O fornecedor de um JOB alugado.
    pub fn fornecedor(&self, job: &[u8; HASH_LEN]) -> Option<[u8; PUBKEY_LEN]> {
        self.alugueis.lock().ok()?.get(job).map(|a| a.fornecedor)
    }

    /// Uma unidade do fornecedor foi verificada aqui (gancho da ciência).
    pub fn unidade_verificada(&self, job: &[u8; HASH_LEN], worker: &[u8; PUBKEY_LEN], operacoes: u64) {
        let mudou = self.alugueis.lock().is_ok_and(|mut al| match al.get_mut(job) {
            Some(a) if &a.fornecedor == worker => {
                a.operacoes = a.operacoes.saturating_add(operacoes);
                true
            }
            _ => false,
        });
        if mudou {
            self.gravar_alugueis();
        }
    }

    fn gravar_alugueis(&self) {
        let t: String = self
            .alugueis
            .lock()
            .map(|al| {
                al.iter()
                    .map(|(j, a)| format!("{} {} {} {} {} {}\n", hex(j), hex(&a.fornecedor), hex(&a.identidade), a.preco_credito_mili, a.operacoes, a.recibo_mili))
                    .collect()
            })
            .unwrap_or_default();
        let _ = std::fs::create_dir_all(&self.pasta);
        let _ = crate::arquivos::gravar_atomico(&self.pasta.join("alugueis.txt"), t.as_bytes());
    }

    fn mandar_recibos(&self) {
        let pendentes: Vec<([u8; HASH_LEN], Aluguel)> =
            self.alugueis.lock().map(|al| al.iter().filter(|(_, a)| a.creditos() > a.recibo_mili).map(|(j, a)| (*j, a.clone())).collect()).unwrap_or_default();
        for (job, a) in pendentes {
            let Some(par) = self.par_de(&a.identidade) else { continue };
            let Ok(r) = Recibo::assinar(&self.segredo, a.fornecedor, job, a.creditos(), a.preco_credito_mili, agora_ms()) else { continue };
            if self.enviar(par, &MensagemNuvem::Recibo(Box::new(r))) {
                if let Ok(mut al) = self.alugueis.lock()
                    && let Some(x) = al.get_mut(&job)
                {
                    x.recibo_mili = a.creditos();
                }
                self.gravar_alugueis();
            }
        }
    }

    // -------------------------------------------------------- manutenção

    fn meu_anuncio(&self) -> Option<Anuncio> {
        let aj = self.ajustes.lock().ok()?.clone();
        let mq = (self.ganchos.maquina)();
        Anuncio {
            worker: self.worker,
            identidade: self.identidade,
            tipo: aj.tipo,
            instante_ms: agora_ms(),
            validade_s: VALIDADE_DO_ANUNCIO_S,
            linhas: mq.linhas,
            ram_mib: mq.ram_mib,
            gpu: mq.gpu.chars().take(60).collect(),
            vram_mib: mq.vram_mib,
            disco_mib: aj.disco_mib,
            preco_credito_mili: aj.preco_credito_mili,
            preco_gb_mes_mili: aj.preco_gb_mes_mili,
            preco_venda_centavos: if aj.tipo == TipoDeAnuncio::Venda { aj.preco_venda_centavos } else { 0 },
            descricao: aj.descricao.chars().take(200).collect(),
            creditos_hora: mq.creditos_hora,
            assinatura: [0; 64],
        }
        .assinar(&self.segredo)
        .ok()
    }

    /// Anuncia já (depois de o dono mudar os ajustes).
    pub fn anunciar_agora(&self) {
        self.ultimo_anuncio.store(0, Ordering::Relaxed);
    }

    /// Muda os ajustes e grava.
    ///
    /// # Errors
    /// Percentuais que passam de 100, ou disco.
    pub fn mudar_ajustes(&self, novo: AjustesDaNuvem) -> Result<(), String> {
        if u16::from(novo.provedor_pct).saturating_add(u16::from(novo.plataforma_pct)) > 100 {
            return Err("provedor + plataforma não pode passar de 100%".into());
        }
        novo.gravar(&self.pasta_config)?;
        if let Ok(mut g) = self.guarda.lock() {
            g.cota_mib = novo.disco_mib;
        }
        if let Ok(mut a) = self.ajustes.lock() {
            *a = novo;
        }
        self.anunciar_agora();
        Ok(())
    }

    fn cuidar(self: &Arc<Self>) {
        let agora = agora_unix();
        let Some(rede) = self.rede() else { return };
        let pares = rede.pares_com_identidade();
        let conectados: BTreeSet<[u8; 32]> = pares.iter().map(|(_, i)| *i).collect();
        // 1. mercado: o nosso anúncio, e o lote para quem chegou agora
        let anunciar = self.ajustes.lock().is_ok_and(|a| a.anunciar);
        let mut lote_novo: Vec<u64> = Vec::new();
        if let Ok(mut m) = self.mercado.lock() {
            m.limpar(agora_ms());
            for (p, _) in &pares {
                if m.ja_receberam.insert(*p) {
                    lote_novo.push(*p);
                }
            }
            let vivos: BTreeSet<u64> = pares.iter().map(|(p, _)| *p).collect();
            m.ja_receberam.retain(|p| vivos.contains(p));
        }
        let meu = if anunciar { self.meu_anuncio() } else { None };
        if let Some(a) = &meu
            && agora.saturating_sub(self.ultimo_anuncio.load(Ordering::Relaxed)) >= REANUNCIAR_S
            && let Ok(c) = MensagemNuvem::Anuncio(Box::new(a.clone())).codificar()
        {
            self.ultimo_anuncio.store(agora, Ordering::Relaxed);
            rede.difundir_tipo(TIPO_NUVEM, &c, None);
        }
        if !lote_novo.is_empty() {
            let mut lote: Vec<Anuncio> = self.mercado.lock().map(|m| m.mais_novos(50)).unwrap_or_default();
            if let Some(a) = meu {
                lote.push(a);
            }
            for p in &lote_novo {
                for a in &lote {
                    if let Ok(c) = MensagemNuvem::Anuncio(Box::new(a.clone())).codificar() {
                        rede.enviar_tipo(*p, TIPO_NUVEM, c);
                    }
                }
            }
        }
        // 2. o que este nó guarda
        if let Ok(mut g) = self.guarda.lock() {
            g.limpar(agora);
        }
        // 3. os arquivos deste nó: presença, desafios, reparo
        let (desafio_s, perda_s) = self.ajustes.lock().map(|a| (a.desafio_s, a.perda_s)).unwrap_or((600, 86_400));
        let mut desafiar: Vec<Desafiar> = Vec::new();
        let mut renovar: Vec<([u8; 32], u8)> = Vec::new();
        let mut reparar: Vec<[u8; 32]> = Vec::new();
        let mut perdas: Vec<(Alvo, [u8; 32], &'static str)> = Vec::new();
        {
            let em_recuperacao: BTreeSet<[u8; 32]> = self.recuperacoes.lock().map(|r| r.keys().copied().collect()).unwrap_or_default();
            let mut no_ar = self.desafios_no_ar.lock().map(|d| d.clone()).unwrap_or_default();
            // desafio sem resposta: falha
            for ((g, alvo), (_, _, enviado)) in &no_ar {
                if agora.saturating_sub(*enviado) > DESAFIO_RESPOSTA_S {
                    perdas.push((*alvo, *g, "não respondeu ao desafio de guarda"));
                }
            }
            no_ar.retain(|_, (_, _, enviado)| agora.saturating_sub(*enviado) <= DESAFIO_RESPOSTA_S);
            if let Ok(mut ms) = self.manifestos.lock() {
                for m in ms.values_mut() {
                    for f in &mut m.fragmentos {
                        if f.estado != EstadoDoFragmento::Guardado {
                            continue;
                        }
                        let alvo = Alvo { arquivo: m.arquivo, indice: f.indice };
                        if conectados.contains(&f.guardiao) {
                            if f.ausente_desde != 0 {
                                f.ausente_desde = 0;
                                self.manifestos_sujos.store(true, Ordering::Relaxed);
                            }
                        } else {
                            if f.ausente_desde == 0 {
                                f.ausente_desde = agora;
                                self.manifestos_sujos.store(true, Ordering::Relaxed);
                            }
                            if agora.saturating_sub(f.ausente_desde) >= perda_s {
                                f.estado = EstadoDoFragmento::Perdido;
                                self.manifestos_sujos.store(true, Ordering::Relaxed);
                            }
                            continue;
                        }
                        if agora.saturating_sub(f.ultimo_ok) < desafio_s || no_ar.contains_key(&(f.guardiao, alvo)) || em_recuperacao.contains(&m.arquivo) {
                            continue;
                        }
                        if f.desafios.is_empty() {
                            renovar.push((m.arquivo, f.indice));
                        } else {
                            let (nonce, esperado) = f.desafios.remove(0);
                            self.manifestos_sujos.store(true, Ordering::Relaxed);
                            no_ar.insert((f.guardiao, alvo), (nonce, esperado, agora));
                            desafiar.push((f.guardiao, alvo, nonce, esperado));
                        }
                    }
                    let tem_perdido = m.fragmentos.iter().any(|f| f.estado == EstadoDoFragmento::Perdido);
                    if tem_perdido && m.guardados() >= usize::from(m.k) && !em_recuperacao.contains(&m.arquivo) {
                        reparar.push(m.arquivo);
                    }
                }
            }
            if let Ok(mut d) = self.desafios_no_ar.lock() {
                *d = no_ar;
            }
        }
        for (alvo, g, motivo) in perdas {
            let sem_prova = self.manifestos.lock().is_ok_and(|mut ms| {
                ms.get_mut(&alvo.arquivo).and_then(|m| m.fragmentos.iter_mut().find(|f| f.indice == alvo.indice && f.guardiao == g)).is_some_and(|f| {
                    f.falhas = f.falhas.saturating_add(1);
                    f.falhas >= 2
                })
            });
            self.manifestos_sujos.store(true, Ordering::Relaxed);
            if sem_prova {
                self.marcar_perdido(&alvo, &g, motivo);
            }
        }
        for (g, alvo, nonce, _) in desafiar {
            if let Some(par) = self.par_de(&g) {
                self.enviar(par, &MensagemNuvem::Desafio { alvo, nonce });
            }
        }
        for (arquivo, indice) in renovar {
            let _ = self.comecar_recuperacao(&arquivo, Fim::Renovar(indice));
        }
        for arquivo in reparar {
            if self.comecar_recuperacao(&arquivo, Fim::Reparar).is_ok() {
                (self.ganchos.registrar)("nuvem", format!("reparo começou: reconstruindo os fragmentos perdidos de {}…", hex(arquivo.get(..6).unwrap_or_default())));
            }
        }
        // 4. recuperações paradas
        let paradas: Vec<[u8; 32]> = self
            .recuperacoes
            .lock()
            .map(|rs| rs.iter().filter(|(_, r)| !r.terminando && agora.saturating_sub(r.inicio) > RECUPERACAO_VALE_S).map(|(a, _)| *a).collect())
            .unwrap_or_default();
        for a in paradas {
            if let Ok(mut rs) = self.recuperacoes.lock() {
                rs.remove(&a);
            }
            self.noticia(&a, "recuperação parada: fragmentos insuficientes chegaram a tempo".into());
        }
        self.gravar_manifestos_sujos();
        // 5. recibos e livro
        if agora.saturating_sub(self.ultimo_recibo.load(Ordering::Relaxed)) >= 10 {
            self.ultimo_recibo.store(agora, Ordering::Relaxed);
            self.mandar_recibos();
        }
        if agora.saturating_sub(self.ultima_liquidacao.load(Ordering::Relaxed)) >= 10 {
            self.ultima_liquidacao.store(agora, Ordering::Relaxed);
            self.liquidar(agora);
        }
    }

    /// Lança no livro o consumo novo dos JOBs.
    pub fn liquidar(&self, agora: u64) {
        let jobs = self.consumos.lock().ok().and_then(|c| c.as_ref().map(|f| f()));
        let Some(jobs) = jobs else { return };
        let (p, pl) = self.ajustes.lock().map(|a| (a.provedor_pct, a.plataforma_pct)).unwrap_or((80, 15));
        let com_fornecedor: Vec<livro::JobALiquidar> = jobs.into_iter().map(|(j, c, v)| (j, c, v, self.fornecedor(&j))).collect();
        if let Err(e) = self.livro.liquidar(agora, &com_fornecedor, p, pl) {
            (self.ganchos.registrar)("erro", format!("livro de contas: {e}"));
        }
    }

    // --------------------------------------------------------------- tela

    /// O resumo para o estado da tela (barato: sem ler disco).
    pub fn resumo(&self) -> serde_json::Value {
        let (anunciar, disco) = self.ajustes.lock().map(|a| (a.anunciar, a.disco_mib)).unwrap_or_default();
        let anuncios = self.mercado.lock().map(|m| m.anuncios.len()).unwrap_or(0);
        let (arquivos, degradados, em_risco) = self
            .manifestos
            .lock()
            .map(|ms| {
                let s: Vec<&'static str> = ms.values().map(Manifesto::saude).collect();
                (s.len(), s.iter().filter(|x| **x == "degradado").count(), s.iter().filter(|x| **x == "em_risco").count())
            })
            .unwrap_or_default();
        let (usado, guardados) = self.guarda.lock().map(|g| (g.usado(), g.itens.len())).unwrap_or_default();
        let totais = self.livro.totais();
        json!({
            "anunciando": anunciar, "anuncios": anuncios,
            "arquivos": arquivos, "degradados": degradados, "em_risco": em_risco,
            "guarda": { "cota_mib": disco, "usado_bytes": usado, "fragmentos": guardados },
            "alugueis": self.alugueis.lock().map(|a| a.len()).unwrap_or(0),
            "livro_integro": self.livro.quebra().is_none(),
            "consumo_mili": totais.get("consumo").copied().unwrap_or(0),
            "a_receber_mili": totais.get("recibo").copied().unwrap_or(0),
        })
    }

    /// O estado inteiro para a tela. `reputacao(worker)`: (nota 0–1000, classe).
    pub fn json(&self, reputacao: Reputacao<'_>) -> serde_json::Value {
        let agora = agora_ms();
        let conectados: BTreeSet<[u8; 32]> = self.rede().map(|r| r.pares_com_identidade().into_iter().map(|(_, i)| i).collect()).unwrap_or_default();
        let aj = self.ajustes.lock().map(|a| a.clone()).unwrap_or_default();
        let comissao = |preco: u64| {
            let (p, pl, r) = hyurax_nuvem::livro::repartir(preco, aj.provedor_pct, aj.plataforma_pct).unwrap_or((preco, 0, 0));
            json!({ "total_mili": preco, "proprietario_mili": p, "plataforma_mili": pl, "reserva_mili": r })
        };
        let anuncios: Vec<serde_json::Value> = self
            .mercado
            .lock()
            .map(|m| {
                let mut v: Vec<&Anuncio> = m.anuncios.values().collect();
                v.sort_by_key(|a| (a.preco_credito_mili, std::cmp::Reverse(a.instante_ms)));
                v.into_iter()
                    .map(|a| {
                        let rep = reputacao(&a.worker);
                        json!({
                            "worker": hex(&a.worker), "identidade": hex(&a.identidade), "tipo": a.tipo.nome(),
                            "instante_ms": a.instante_ms, "vence_ms": a.instante_ms.saturating_add(u64::from(a.validade_s) * 1000),
                            "linhas": a.linhas, "ram_mib": a.ram_mib, "gpu": a.gpu, "vram_mib": a.vram_mib, "disco_mib": a.disco_mib,
                            "preco_credito_mili": a.preco_credito_mili, "preco_gb_mes_mili": a.preco_gb_mes_mili,
                            "preco_venda_centavos": a.preco_venda_centavos, "descricao": a.descricao,
                            "creditos_hora_estimado": a.creditos_hora,
                            "por_1000_creditos": comissao(a.preco_credito_mili.saturating_mul(1000)),
                            "conectado": conectados.contains(&a.identidade),
                            "assinatura_confere": true,
                            "reputacao": rep.as_ref().map(|(n, _)| *n), "classe": rep.map(|(_, c)| c),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (cota, usado, itens) = self.guarda.lock().map(|g| (g.cota_mib, g.usado(), g.itens.len())).unwrap_or_default();
        let recuperando: BTreeSet<[u8; 32]> = self.recuperacoes.lock().map(|r| r.keys().copied().collect()).unwrap_or_default();
        let noticias = self.noticias.lock().map(|n| n.clone()).unwrap_or_default();
        let arquivos: Vec<serde_json::Value> = self
            .manifestos
            .lock()
            .map(|ms| {
                ms.values()
                    .map(|m| {
                        let frags: Vec<serde_json::Value> = m
                            .fragmentos
                            .iter()
                            .map(|f| {
                                json!({
                                    "indice": f.indice, "tamanho": f.tamanho, "guardiao": hex(&f.guardiao), "estado": f.estado.nome(),
                                    "conectado": conectados.contains(&f.guardiao), "ultimo_ok": f.ultimo_ok, "falhas": f.falhas,
                                    "desafios_restantes": f.desafios.len(), "paridade": f.indice >= m.k,
                                })
                            })
                            .collect();
                        json!({
                            "arquivo": hex(&m.arquivo), "nome": m.nome, "tamanho": m.tamanho, "k": m.k, "m": m.m, "criado": m.criado,
                            "saude": m.saude(), "guardados": m.guardados(), "recuperando": recuperando.contains(&m.arquivo),
                            "recuperado": self.pasta.join("recuperados").join(nome_seguro(&m.nome)).exists(),
                            "noticia": noticias.get(&m.arquivo), "fragmentos": frags,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let alugueis: Vec<serde_json::Value> = self
            .alugueis
            .lock()
            .map(|al| {
                al.iter()
                    .map(|(j, a)| {
                        json!({
                            "job": hex(j), "fornecedor": hex(&a.fornecedor), "preco_credito_mili": a.preco_credito_mili,
                            "operacoes": a.operacoes, "creditos_mili": a.creditos(), "recibo_mili": a.recibo_mili,
                            "total_mili": hyurax_nuvem::recibo::total(a.creditos(), a.preco_credito_mili),
                            "conectado": conectados.contains(&a.identidade),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mq = (self.ganchos.maquina)();
        json!({
            "aviso": "rede de TESTE: créditos de computação não são dinheiro nem HYX; nada aqui paga ninguém",
            "worker": hex(&self.worker), "identidade": hex(&self.identidade), "agora_ms": agora,
            "ajustes": {
                "anunciar": aj.anunciar, "tipo": aj.tipo.nome(), "preco_credito_mili": aj.preco_credito_mili,
                "preco_gb_mes_mili": aj.preco_gb_mes_mili, "preco_venda_centavos": aj.preco_venda_centavos,
                "descricao": aj.descricao, "disco_mib": aj.disco_mib, "provedor_pct": aj.provedor_pct,
                "plataforma_pct": aj.plataforma_pct, "desafio_s": aj.desafio_s, "perda_s": aj.perda_s,
            },
            "maquina": { "linhas": mq.linhas, "ram_mib": mq.ram_mib, "gpu": mq.gpu, "vram_mib": mq.vram_mib, "creditos_hora_estimado": mq.creditos_hora },
            "mercado": anuncios,
            "guarda": { "cota_mib": cota, "usado_bytes": usado, "fragmentos": itens },
            "arquivos": arquivos,
            "alugueis": alugueis,
            "livro": { "integro": self.livro.quebra().is_none(), "totais": self.livro.totais() },
        })
    }
}

fn semente_dos_desafios(chave: &[u8; 32]) -> [u8; 32] {
    let mut x = chave.to_vec();
    x.extend_from_slice(b"desafios");
    let h = sha512(&x);
    let mut o = [0u8; 32];
    o.copy_from_slice(h.get(..32).unwrap_or(&[0; 32]));
    o
}

fn ler_alugueis(caminho: &std::path::Path) -> BTreeMap<[u8; HASH_LEN], Aluguel> {
    std::fs::read_to_string(caminho)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut c = l.split(' ');
            Some((
                de_hex::<64>(c.next()?)?,
                Aluguel {
                    fornecedor: de_hex(c.next()?)?,
                    identidade: de_hex(c.next()?)?,
                    preco_credito_mili: c.next()?.parse().ok()?,
                    operacoes: c.next()?.parse().ok()?,
                    recibo_mili: c.next()?.parse().ok()?,
                },
            ))
        })
        .collect()
}

fn manter(fraco: &Weak<Nuvem>) {
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let Some(n) = fraco.upgrade() else { return };
        n.cuidar();
    }
}

/// O identificador de um arquivo, de hex.
pub fn arquivo(texto: &str) -> Option<[u8; 32]> {
    arquivo_de_hex(texto)
}
