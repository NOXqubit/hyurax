//! O núcleo ligado: junta o nó, a carteira, a mineração, o ULTRAX, a ciência,
//! as métricas e o barramento, e cuida do ciclo de vida (subir, vigiar,
//! gravar e encerrar).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyurax_crypto::ADDRESS_LEN;
use hyurax_net::Rede;

use crate::ajustes::Ajustes;
use crate::barramento::Barramento;
use crate::cadeia::{self, Saida};
use crate::carteira::servico::Carteira;
use crate::ciencia::Ciencia;
use crate::config::ConfigDoNo;
use crate::maquinas::{self, Maquinas};
use crate::metricas::Metricas;
use crate::mineracao::Mineracao;
use crate::pastas;
use crate::ultrax::{self, Ultrax};
use crate::util::{agora_unix, hex};
use crate::{identidade, VERSAO};

/// Quem está usando o núcleo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    /// O programa com janela: ajustes gravados, carteira pela interface.
    Janela,
    /// O programa de terminal: registros no terminal, nada de ajustes gravados.
    Terminal,
}

/// O que o núcleo precisa para ligar.
pub struct Partida {
    /// Janela ou terminal.
    pub modo: Modo,
    /// Rede, pastas, porta e sementes.
    pub config: ConfigDoNo,
    /// O arquivo da carteira (`None`: só recebe por endereço fixo).
    pub arquivo_carteira: Option<PathBuf>,
    /// Endereço de recompensa fixo (terminal, `--endereco`).
    pub endereco_fixo: Option<[u8; ADDRESS_LEN]>,
    /// Linhas da mineração quando não há ajuste gravado.
    pub linhas_padrao: u32,
    /// Pausa fixa entre tentativas (terminal, `--pausa-ms`); 0 = limite de CPU.
    pub pausa_fixa_ms: u64,
    /// Painel visível na rede local (só leitura).
    pub painel_na_rede: bool,
}

/// O núcleo ligado.
pub struct Nucleo {
    /// Janela ou terminal.
    pub modo: Modo,
    /// Rede, pastas, porta e sementes.
    pub config: ConfigDoNo,
    /// O nó na rede.
    pub rede: Arc<Rede>,
    /// Os eventos.
    pub barramento: Arc<Barramento>,
    /// A carteira.
    pub carteira: Carteira,
    /// A mineração dos blocos.
    pub mineracao: Arc<Mineracao>,
    /// O worker de trabalho útil.
    pub ultrax: Arc<Ultrax>,
    /// Os JOBs científicos.
    pub ciencia: Arc<Ciencia>,
    /// CPU, RAM e GPU da máquina.
    pub metricas: Arc<Metricas>,
    /// As outras máquinas do dono.
    pub maquinas: Maquinas,
    /// Os ajustes (a parte que não mora em outro serviço).
    pub ajustes: Mutex<Ajustes>,
    /// Sementes do dono.
    pub sementes: Mutex<Vec<String>>,
    /// O painel responde a outros aparelhos da rede local (só leitura).
    pub na_rede: AtomicBool,
    /// A API externa responde (contas de cliente, `/api/v1/externa/`).
    pub api_externa: AtomicBool,
    /// A API das carteiras de celular responde (`/api/v1/leve/`).
    pub carteiras_leves: AtomicBool,
    /// As contas de cliente da API externa.
    pub contas: crate::contas::Contas,
    /// O plano deste nó (vouchers assinados; ver `crate::planos`).
    pub planos: crate::planos::Planos,
    /// Porta do nó para outros nós (0: não escuta).
    pub porta_p2p: u16,
    /// Quando o núcleo ligou.
    pub inicio: Instant,
    /// Núcleos lógicos da máquina.
    pub nucleos: u32,
    /// A chave de sessão do painel desta abertura (ver `api`): sem ela, nem
    /// um programa deste computador manda comando.
    pub chave_painel: String,
    /// Assina o resumo público (a chave de worker deste nó), para as outras
    /// máquinas do dono saberem que foi esta que respondeu.
    pub chave_do_resumo: [u8; hyurax_crypto::SECRET_LEN],
    /// A atualização segura (manifesto assinado).
    pub atualizacao: Mutex<crate::atualizacao::Estado>,
    /// A nuvem: mercado de máquinas, armazenamento distribuído e livro.
    pub nuvem: Arc<crate::nuvem::Nuvem>,
    /// Quem fecha o programa quando o instalador da versão nova abre (a
    /// janela registra; no terminal, ninguém).
    pub ao_sair: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    /// A trava da pasta de dados (solta quando o núcleo some).
    _trava: std::fs::File,
}

impl Nucleo {
    /// Liga tudo.
    ///
    /// # Errors
    /// Pasta sem permissão, cadeia corrompida ou porta ocupada.
    pub fn ligar(p: Partida) -> Result<Arc<Self>, String> {
        let pastas = p.config.pastas.clone();
        pastas.criar()?;
        let trava = pastas.travar()?;
        let terminal = p.modo == Modo::Terminal;
        let barramento = Barramento::novo(Some(pastas.registros().join("hyurax.log")), terminal);
        barramento.registrar("sistema", format!("Hyurax / Ultrax {VERSAO} · rede {}", p.config.rede.nome));
        match pastas::migrar_da_0x(&pastas) {
            Ok(m) => {
                if !m.movidos.is_empty() {
                    barramento.registrar("sistema", format!("dados da versão 0.x levados para {}: {}", pastas.dados.display(), m.movidos.join(", ")));
                }
                if !m.arquivados.is_empty() {
                    barramento.registrar(
                        "sistema",
                        format!("histórico da 0.x guardado em arquivo-0.x (não confere mais com os rótulos da 1.0): {}", m.arquivados.join(", ")),
                    );
                }
            }
            Err(e) => barramento.registrar("erro", format!("migração dos dados da 0.x: {e}")),
        }
        let saida: Saida = {
            let b = Arc::clone(&barramento);
            Arc::new(move |t| b.registrar("no", t))
        };
        let rede = cadeia::subir(&p.config, &saida)?;
        let ajustes = if p.modo == Modo::Janela { Ajustes::ler(&pastas.config) } else { Ajustes::default() };
        let nucleos = u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1);
        let id = identidade::na_pasta(&pastas.config)?;
        let chave_painel = crate::api::nova_chave(&pastas.config)?;
        let ultrax = {
            let b = Arc::clone(&barramento);
            Ultrax::abrir(
                &pastas.dados,
                id.segredo(),
                nucleos,
                &ultrax::Partida {
                    ligado: ajustes.ultrax,
                    linhas: ajustes.ultrax_linhas,
                    uso_cpu: ajustes.ultrax_limite_cpu,
                    memoria_mib: ajustes.ultrax_memoria_mib,
                    debug: ajustes.ultrax_debug,
                    gpu: ajustes.ultrax_gpu,
                    gpu_uso: ajustes.ultrax_gpu_limite,
                },
                Box::new(move |tipo, texto| b.registrar(tipo, texto)),
            )
        };
        {
            let b = Arc::clone(&barramento);
            ultrax.ao_amostrar(Arc::new(move |ctx, a| {
                b.publicar("amostra", format!("{{\"contexto\":{},\"amostra\":{}}}", ctx.json(), a.json()));
            }));
            let b = Arc::clone(&barramento);
            ultrax.ao_marcar(Arc::new(move |tarefa, evento, detalhe| {
                b.publicar("tarefa", serde_json::json!({ "tarefa": tarefa, "evento": evento, "detalhe": detalhe }).to_string());
            }));
        }
        let ciencia = Ciencia::abrir(&pastas.dados, ultrax.worker(), VERSAO)?;
        ciencia.ligar(&ultrax);
        {
            let b = Arc::clone(&barramento);
            ciencia.ao_emitir(Arc::new(move |linha: &str| {
                b.publicar("ciencia", linha.to_string());
            }));
        }
        ciencia.ligar_rede(&rede, hyurax_ultrax::prova::chave_do_worker(id.segredo()), ajustes.aceitar_rede);
        let metricas = Metricas::iniciar();
        let nuvem = {
            let (b, b2, m, u) = (Arc::clone(&barramento), Arc::clone(&barramento), Arc::clone(&metricas), Arc::clone(&ultrax));
            crate::nuvem::Nuvem::abrir(
                &pastas.dados,
                &pastas.config,
                hyurax_ultrax::prova::chave_do_worker(id.segredo()),
                rede.identidade_publica(),
                crate::nuvem::Ganchos {
                    registrar: Box::new(move |c, t| b.registrar(c, t)),
                    publicar: Box::new(move |j| {
                        b2.publicar("nuvem", j);
                    }),
                    maquina: Box::new(move || {
                        let s = m.sistema();
                        let g = s.gpus.first();
                        crate::nuvem::Maquina {
                            linhas: u16::try_from(u.linhas.load(Ordering::Relaxed)).unwrap_or(1),
                            ram_mib: s.ram_total_mib.and_then(|x| u32::try_from(x).ok()).unwrap_or(0),
                            gpu: g.map(|g| g.nome.clone()).unwrap_or_default(),
                            vram_mib: g.and_then(|g| g.memoria_mib).and_then(|x| u32::try_from(x).ok()).unwrap_or(0),
                            creditos_hora: 0,
                        }
                    }),
                },
            )
        };
        nuvem.ligar_rede(&rede);
        {
            let fraca = Arc::downgrade(&nuvem);
            ciencia.ao_verificar_de_fora(Arc::new(move |job, worker, mili| {
                if let Some(nv) = fraca.upgrade() {
                    nv.unidade_verificada(job, worker, mili);
                }
            }));
        }
        let carteira = Carteira::abrir(p.arquivo_carteira.clone(), p.endereco_fixo, pastas.config.clone());
        let mineracao = Mineracao::nova(nucleos, ajustes.linhas.unwrap_or(p.linhas_padrao), ajustes.limite_cpu);
        let sementes = if p.modo == Modo::Janela { ajustes.sementes.clone() } else { p.config.sementes.clone() };
        let maquinas = Maquinas::com_arquivo(pastas.config.join(maquinas::ARQUIVO_DAS_CONHECIDAS));
        if let Ok(mut l) = maquinas.lista.lock() {
            l.clone_from(&ajustes.maquinas);
        }
        let n = Arc::new(Self {
            modo: p.modo,
            porta_p2p: p.config.porta,
            config: p.config,
            rede,
            barramento,
            carteira,
            mineracao,
            ultrax,
            ciencia,
            metricas,
            nuvem,
            maquinas,
            na_rede: AtomicBool::new(ajustes.na_rede || p.painel_na_rede),
            api_externa: AtomicBool::new(ajustes.api_externa),
            carteiras_leves: AtomicBool::new(ajustes.carteiras_leves),
            contas: crate::contas::Contas::abrir(Some(pastas.config.clone())),
            planos: crate::planos::Planos::abrir(Some(&pastas.config)),
            sementes: Mutex::new(sementes),
            ajustes: Mutex::new(ajustes.clone()),
            inicio: Instant::now(),
            nucleos,
            chave_painel,
            chave_do_resumo: hyurax_ultrax::prova::chave_do_worker(id.segredo()),
            atualizacao: Mutex::new(crate::atualizacao::Estado::default()),
            ao_sair: Mutex::new(None),
            _trava: trava,
        });
        // o livro da nuvem lê o consumo de cada JOB, e de quem é o JOB
        {
            let fraco = Arc::downgrade(&n);
            n.nuvem.ao_liquidar(Box::new(move || {
                let Some(n) = fraco.upgrade() else { return Vec::new() };
                let jobs: Vec<([u8; hyurax_crypto::HASH_LEN], u64)> =
                    n.ciencia.jobs.lock().map(|j| j.iter().map(|x| (x.id, x.consumo.milicreditos())).collect()).unwrap_or_default();
                jobs.into_iter().map(|(id, mili)| (id, n.contas.conta_do_job(&id).unwrap_or(0), mili)).collect()
            }));
        }
        // a comissão da plataforma no livro segue o plano do nó
        {
            let fraco = Arc::downgrade(&n);
            n.nuvem.ao_cobrar_comissao(Box::new(move || {
                fraco.upgrade().map_or(15, |n| n.planos.comissao_pct(&n.ultrax.worker(), crate::util::agora_ms()))
            }));
        }
        // a atualização segura: um minuto depois de abrir, e a cada 12 h (só
        // no programa com janela; o terminal confere à mão, se quiser)
        if n.modo == Modo::Janela && crate::atualizacao::chave_do_projeto().is_some() {
            let fraco = Arc::downgrade(&n);
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(60));
                loop {
                    let Some(n) = fraco.upgrade() else { return };
                    n.buscar_atualizacao();
                    drop(n);
                    std::thread::sleep(std::time::Duration::from_secs(12 * 3600));
                }
            });
        }
        if n.carteira.endereco().is_none() && n.modo == Modo::Janela {
            n.barramento.registrar("carteira", "nenhuma carteira ainda: crie ou importe uma para minerar e receber HYX de teste");
        }
        if ajustes.minerar && n.carteira.endereco().is_some() {
            n.mineracao.ligada.store(true, Ordering::Relaxed);
        }
        // a mineração
        {
            let (m, rede, config, b) = (Arc::clone(&n.mineracao), Arc::clone(&n.rede), n.config.clone(), Arc::clone(&n.barramento));
            let fraco = Arc::downgrade(&n);
            let endereco = Arc::new(move || fraco.upgrade().and_then(|n| n.carteira.endereco()));
            let pausa = p.pausa_fixa_ms;
            std::thread::spawn(move || m.laco(rede, config, endereco, b, pausa));
        }
        // o vigia: ritmo, gravação da cadeia, blocos de fora e pares
        {
            let fraco = Arc::downgrade(&n);
            std::thread::spawn(move || vigiar(&fraco));
        }
        // as outras máquinas do dono
        {
            let fraco = Arc::downgrade(&n);
            std::thread::spawn(move || {
                while let Some(n) = fraco.upgrade() {
                    n.maquinas.olhar_todas(agora_unix());
                    drop(n);
                    std::thread::sleep(maquinas::INTERVALO);
                }
            });
        }
        // a pasta do Éter: pacotes que chegam por pendrive, Bluetooth ou som
        {
            let fraco = Arc::downgrade(&n);
            std::thread::spawn(move || crate::eter::vigiar(&fraco));
        }
        n.ultrax.iniciar();
        Ok(n)
    }

    /// Grava os ajustes do dono (só no programa com janela).
    pub fn gravar_ajustes(&self) {
        if self.modo != Modo::Janela {
            return;
        }
        let u = &self.ultrax;
        let mut a = self.ajustes.lock().map(|a| a.clone()).unwrap_or_default();
        a.minerar = self.mineracao.ligada.load(Ordering::Relaxed);
        a.linhas = Some(self.mineracao.linhas.load(Ordering::Relaxed));
        a.limite_cpu = self.mineracao.limite_cpu.load(Ordering::Relaxed);
        a.na_rede = self.na_rede.load(Ordering::Relaxed);
        a.api_externa = self.api_externa.load(Ordering::Relaxed);
        a.carteiras_leves = self.carteiras_leves.load(Ordering::Relaxed);
        a.ultrax = u.ligado.load(Ordering::Relaxed);
        a.ultrax_linhas = u.linhas.load(Ordering::Relaxed);
        a.ultrax_limite_cpu = u.uso_cpu.load(Ordering::Relaxed);
        a.ultrax_memoria_mib = u.memoria_mib.load(Ordering::Relaxed);
        a.ultrax_debug = u.debug.load(Ordering::Relaxed);
        a.ultrax_gpu = u.gpu_ligada.load(Ordering::Relaxed);
        a.ultrax_gpu_limite = u.gpu_uso.load(Ordering::Relaxed);
        a.aceitar_rede = self.ciencia.aceita_da_rede();
        a.sementes = self.sementes.lock().map(|s| s.clone()).unwrap_or_default();
        a.maquinas = self.maquinas.lista.lock().map(|m| m.clone()).unwrap_or_default();
        if let Ok(mut x) = self.ajustes.lock() {
            x.clone_from(&a);
        }
        if let Err(e) = a.gravar(&self.config.pastas.config) {
            self.barramento.registrar("erro", format!("não consegui gravar os ajustes: {e}"));
        }
    }

    /// Troca as sementes e conecta nelas.
    ///
    /// # Errors
    /// Endereço inválido ou lista grande demais.
    pub fn trocar_sementes(&self, texto: &str) -> Result<Vec<String>, String> {
        let novas: Vec<String> =
            texto.split([',', '\n', ' ', ';']).map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
        if novas.len() > crate::sementes::MAXIMO {
            return Err(format!("no máximo {} sementes", crate::sementes::MAXIMO));
        }
        if let Some(ruim) = novas.iter().find(|s| !crate::sementes::valida(s)) {
            return Err(format!("\"{ruim}\" não é IP:PORTA (exemplo: 203.0.113.7:8790)"));
        }
        if let Ok(mut s) = self.sementes.lock() {
            s.clone_from(&novas);
        }
        self.gravar_ajustes();
        for s in &novas {
            self.rede.semear(s);
            match self.rede.conectar_texto(s) {
                Ok(()) => self.barramento.registrar("rede", format!("conectando em {s}")),
                Err(e) => self.barramento.registrar("rede", format!("não consegui conectar em {s}: {e}")),
            }
        }
        Ok(novas)
    }

    /// Busca o manifesto da versão mais nova e confere a assinatura.
    pub fn buscar_atualizacao(&self) {
        let r = crate::atualizacao::buscar();
        let mut nova = None;
        if let Ok(mut e) = self.atualizacao.lock() {
            e.verificado_ms = crate::util::agora_ms();
            match r {
                Ok(m) if crate::atualizacao::mais_nova(&m.versao, VERSAO) => {
                    e.erro = None;
                    if e.disponivel.as_ref().map(|x| &x.versao) != Some(&m.versao) {
                        nova = Some(m.versao.clone());
                    }
                    e.disponivel = Some(m);
                }
                Ok(_) => {
                    e.erro = None;
                    e.disponivel = None;
                }
                Err(erro) => e.erro = Some(erro),
            }
        }
        if let Some(v) = nova {
            self.barramento.registrar("sistema", format!("versão {v} disponível: manifesto assinado pela chave de lançamento e conferido"));
        }
    }

    /// Baixa o instalador da versão nova, confere contra o manifesto assinado,
    /// abre o instalador (que espera este programa fechar) e pede para fechar.
    ///
    /// # Errors
    /// Sem versão nova conferida, ou já baixando.
    pub fn instalar_atualizacao(self: &Arc<Self>) -> Result<(), String> {
        let m = {
            let mut e = self.atualizacao.lock().map_err(|_| "estado da atualização travado".to_string())?;
            if e.baixando {
                return Err("já estou baixando a atualização".into());
            }
            let m = e.disponivel.clone().ok_or("não há versão nova conferida: busque de novo")?;
            e.baixando = true;
            e.erro = None;
            m
        };
        let n = Arc::clone(self);
        std::thread::spawn(move || {
            let pasta = n.config.pastas.dados.join("atualizacoes");
            n.barramento.registrar("sistema", format!("baixando o instalador da versão {} e conferindo com o manifesto assinado", m.versao));
            let r = crate::atualizacao::baixar(&m, &pasta).and_then(|instalador| {
                std::process::Command::new(&instalador)
                    .arg("--esperar-pid")
                    .arg(std::process::id().to_string())
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| format!("não consegui abrir o instalador: {e}"))
            });
            if let Ok(mut e) = n.atualizacao.lock() {
                e.baixando = false;
                if let Err(erro) = &r {
                    e.erro = Some(erro.clone());
                }
            }
            match r {
                Ok(()) => {
                    n.barramento.registrar("sistema", format!("instalador da versão {} conferido e aberto; o programa fecha para ele atualizar", m.versao));
                    if let Ok(s) = n.ao_sair.lock()
                        && let Some(f) = s.as_ref()
                    {
                        f();
                    }
                }
                Err(erro) => n.barramento.registrar("erro", format!("atualização: {erro}")),
            }
        });
        Ok(())
    }

    /// Para a mineração e o ULTRAX e grava a cadeia: chamar antes de fechar.
    pub fn encerrar(&self) {
        self.mineracao.ligada.store(false, Ordering::Relaxed);
        self.mineracao.interromper();
        self.ultrax.ligar(false);
        // as linhas param no próximo pedaço; espera um pouco para os desfechos
        // (unidades devolvidas à fila) chegarem à ciência antes do checkpoint
        let inicio = Instant::now();
        while self.ultrax.ocupado() && inicio.elapsed() < std::time::Duration::from_secs(3) {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        self.ciencia.encerrar();
        self.ultrax.gravar_placar();
        if let Err(e) = cadeia::salvar(&self.rede, &self.config) {
            self.barramento.registrar("erro", format!("não consegui gravar a cadeia ao fechar: {e}"));
        }
        // a rede para por último: as threads saem, e a porta aberta no
        // roteador (UPnP/NAT-PMP) é devolvida
        self.rede.desligar();
        self.barramento.registrar("sistema", "encerrado");
    }
}

/// Laço de fundo: amostra o ritmo, grava a cadeia, percebe blocos de fora e
/// conta os pares. Termina quando o núcleo some.
fn vigiar(fraco: &std::sync::Weak<Nucleo>) {
    let mut ultima_altura = u64::MAX;
    let mut ultimos_pares = usize::MAX;
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(n) = fraco.upgrade() else { return };
        n.mineracao.amostrar();
        let Ok((altura, ponta)) = n.rede.no.lock().map(|no| (no.chain.height(), no.chain.tip_hash())) else { continue };
        let pares = n.rede.pares_conectados();
        if altura != ultima_altura {
            if ultima_altura != u64::MAX {
                // um bloco que não foi deste nó já foi anunciado pela mineração
                n.barramento.publicar("bloco", serde_json::json!({ "altura": altura, "hash": hex(&ponta) }).to_string());
            }
            // a rodada em curso minera em cima de ponta velha
            if n.mineracao.rodada().is_some_and(|(_, a)| a <= altura) {
                n.mineracao.interromper();
            }
            if let Err(e) = cadeia::salvar(&n.rede, &n.config) {
                n.barramento.registrar("erro", format!("não consegui gravar a cadeia: {e}"));
            }
            ultima_altura = altura;
        }
        if pares != ultimos_pares {
            if ultimos_pares != usize::MAX {
                n.barramento.registrar("rede", format!("{pares} par(es) conectado(s)"));
            }
            ultimos_pares = pares;
        }
    }
}
