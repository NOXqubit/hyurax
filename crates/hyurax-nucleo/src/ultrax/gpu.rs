//! O backend de GPU (WebGL 2 da janela): a conta é feita lá, a CPU confere antes de creditar.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Ultrax {
    // -----------------------------------------------------------------------
    // GPU: a página faz a conta, a CPU confere
    // -----------------------------------------------------------------------

    /// Lado da matriz que cabe em [`SEGUNDOS_ALVO`] no ritmo medido da GPU.
    pub(super) fn dimensionar_gpu(&self) -> Result<Especificacao, String> {
        let ritmo = self.ritmo_gpu.lock().map_or(RITMO_INICIAL_GPU, |r| *r);
        let teto = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
        let mut n = ((ritmo * SEGUNDOS_ALVO).cbrt() as u32).clamp(64, trabalho::MATRIZ_LADO_MAX);
        loop {
            let esp = Especificacao::nova(TipoDeTrabalho::Matriz, n, 0).map_err(|e| e.to_string())?;
            if esp.memoria_bytes() <= teto || n <= 64 {
                return Ok(esp);
            }
            n = n.saturating_mul(7) / 8;
        }
    }

    /// A página pede uma tarefa para a GPU dela. Devolve o número, o lado e,
    /// se for unidade de JOB, o `JOB_ID` e o índice.
    ///
    /// Primeiro uma unidade de JOB de matriz, se houver: é trabalho pedido, e
    /// a GPU faz a mesma conta que a CPU faria (a unidade fecha no JOB igual).
    /// Sem nenhuma, e com o LAB ligado, uma matriz de carga de teste.
    pub fn gpu_pegar(&self, nome: &str) -> Result<(u32, u32, Option<UnidadeDoJob>), String> {
        if !self.ligado.load(Ordering::Relaxed) || !self.gpu_ligada.load(Ordering::Relaxed) {
            return Err("a GPU está desligada no ULTRAX".into());
        }
        let nome: String = nome.chars().filter(|c| !c.is_control()).take(120).collect();
        let nome = if nome.trim().is_empty() { "GPU".to_string() } else { nome };
        if let Ok(mut g) = self.gpu_nome.lock() {
            g.clone_from(&nome);
        }
        if self.gpu.lock().map_or(usize::MAX, |g| g.len()) >= GPU_TAREFAS_MAX {
            return Err("a GPU já tem tarefa em curso".into());
        }
        let agora = agora_ms();
        let do_job = self.agendador().and_then(|a| a.proxima_do_tipo(agora, TipoDeTrabalho::Matriz));
        let (mut item, origem_texto) = match do_job {
            Some(pedido) => {
                let item = self.unidade_de_job(pedido)?;
                (item, "JOB")
            }
            None => {
                if !self.lab.load(Ordering::Relaxed) || self.agendador().is_some_and(|a| a.tem_trabalho()) {
                    return Err("sem tarefa de matriz para a GPU agora".into());
                }
                let esp = self.dimensionar_gpu()?;
                let mut origem = [0u8; 32];
                hyurax_net::entropia::preencher(&mut origem)?;
                let estimado = esp.operacoes_maximas() as f64 / self.ritmo_gpu.lock().map_or(RITMO_INICIAL_GPU, |r| *r).max(1.0);
                let prazo = agora.saturating_add(PRAZO_MINIMO_MS.max((estimado / USO_MINIMO * PRAZO_FOLGA * 1000.0) as u64));
                let tarefa = Tarefa::nova(esp, MetodoDeVerificacao::Freivalds, Instancia::PorWorker, &origem, 100, agora, prazo)
                    .map_err(|e| e.to_string())?;
                self.marcar(
                    tarefa.numero(),
                    "TASK CREATED",
                    format!("{} {} · {} · na GPU · verificação: {} na CPU · {SELO}", esp.tipo().descricao(), esp.resumo(), esp.tipo().categoria().nome(), tarefa.metodo.nome()),
                );
                (NaFila { tarefa, desafio: None, recusas: 0, origem: origem.to_vec(), job: None }, "LAB")
            }
        };
        let esp = item.tarefa.especificacao;
        let numero = item.tarefa.numero();
        let memoria = esp.memoria_bytes();
        if !self.reservar(memoria) {
            let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), "sem memória livre no teto do ULTRAX");
            self.encerrar(&item, None, 0, "sem memória livre no teto do ULTRAX");
            return Err("sem memória livre no teto do ULTRAX agora".into());
        }
        if item.tarefa.estado == Estado::Criada {
            let _ = item.tarefa.avancar(Estado::NaFila, agora_ms(), "");
        }
        let _ = item.tarefa.avancar(Estado::Atribuida, agora_ms(), format!("GPU: {nome}"));
        self.marcar(numero, "TASK ASSIGNED", format!("GPU: {nome} · worker {} · {origem_texto}", curto(&self.worker)));
        self.marcar(
            numero,
            "RESOURCE ALLOCATED",
            format!("{:.1} MiB reservados · GPU até {}% · WebGL 2", mib(memoria), self.gpu_uso.load(Ordering::Relaxed)),
        );
        let _ = item.tarefa.avancar(Estado::Executando, agora_ms(), "");
        self.marcar(numero, "WORK STARTED", format!("{} {} na GPU", esp.tipo().descricao(), esp.resumo()));
        let semente = item.tarefa.semente_para(&self.worker);
        let entrada = hash_da_entrada(&esp, &semente).ok();
        let feitas = Arc::new(AtomicU64::new(0));
        if let Ok(mut a) = self.ativas.lock() {
            a.push(Ativa {
                linha: 0,
                gpu: Some(nome.clone()),
                numero,
                id: item.tarefa.id,
                especificacao: esp,
                metodo: item.tarefa.metodo,
                desafio: false,
                estado: Estado::Executando,
                feitas: Arc::clone(&feitas),
                total: esp.operacoes_maximas(),
                operacoes: 0,
                inicio_ms: agora_ms(),
                memoria,
                entrada,
                job: item.job,
            });
        }
        if let (Some((job, i)), Some(a)) = (item.job, self.agendador()) {
            a.comecou(&job, i, 0, entrada);
        }
        let job = item.job;
        if let Ok(mut g) = self.gpu.lock() {
            g.push(NaGpu { item, semente, entrada, memoria, inicio_ms: agora_ms(), feitas, nome });
        }
        Ok((numero, esp.tamanho(), job))
    }

    /// As matrizes `A` e `B` de uma tarefa da GPU, em u32 little-endian, nessa ordem.
    pub fn gpu_entrada(&self, numero: u32) -> Option<Vec<u8>> {
        let (semente, n) = self
            .gpu
            .lock()
            .ok()?
            .iter()
            .find(|g| g.item.tarefa.numero() == numero)
            .map(|g| (g.semente, g.item.tarefa.especificacao.tamanho()))?;
        let (a, b) = trabalho::matrizes(n, &semente).ok()?;
        Some(a.iter().chain(&b).flat_map(|v| v.to_le_bytes()).collect())
    }

    /// Quantas linhas de `C` a GPU já calculou.
    pub fn gpu_progresso(&self, numero: u32, linhas: u64) {
        if let Ok(g) = self.gpu.lock()
            && let Some(x) = g.iter().find(|g| g.item.tarefa.numero() == numero)
        {
            let n = u64::from(x.item.tarefa.especificacao.tamanho());
            x.feitas.store(linhas.min(n).saturating_mul(n).saturating_mul(n), Ordering::Relaxed);
        }
    }

    /// O resultado da GPU chega: `C` em u32 little-endian. A CPU confere por
    /// Freivalds antes de creditar qualquer coisa.
    pub fn gpu_resultado(&self, numero: u32, bytes: &[u8]) -> Result<(), String> {
        let x = self
            .gpu
            .lock()
            .ok()
            .and_then(|mut g| g.iter().position(|x| x.item.tarefa.numero() == numero).map(|k| g.remove(k)))
            .ok_or("tarefa de GPU desconhecida ou já encerrada")?;
        let esp = x.item.tarefa.especificacao;
        // u32 da GPU para o formato do resultado (i64 little-endian); tamanho
        // errado segue assim mesmo, e a conferência recusa com o motivo
        let resultado: Vec<u8> = bytes.as_chunks::<4>().0.iter().flat_map(|c| i64::from(u32::from_le_bytes(*c)).to_le_bytes()).collect();
        let ms = agora_ms().saturating_sub(x.inicio_ms);
        let exec = trabalho::Execucao { resultado, operacoes: esp.operacoes_fixas().unwrap_or(0), curva: Vec::new() };
        let total = exec.operacoes;
        self.concluir(Conclusao {
            linha: 0,
            item: x.item,
            esp,
            semente: x.semente,
            entrada: x.entrada,
            exec,
            inicio_ms: x.inicio_ms,
            ms_calculo: ms,
            memoria: x.memoria,
            total,
            feitas: x.feitas,
            gpu: Some(x.nome),
        });
        Ok(())
    }

    /// A página desistiu da tarefa (parou, fechou a aba, deu erro no WebGL).
    pub fn gpu_cancelar(&self, numero: u32, motivo: &str) {
        let achada = self
            .gpu
            .lock()
            .ok()
            .and_then(|mut g| g.iter().position(|x| x.item.tarefa.numero() == numero).map(|k| g.remove(k)));
        if let Some(x) = achada {
            self.fechar_gpu(x, Estado::Cancelada, motivo);
        }
    }

    pub(super) fn cancelar_gpu(&self, motivo: &str) {
        let todas: Vec<NaGpu> = self.gpu.lock().map(|mut g| g.drain(..).collect()).unwrap_or_default();
        for x in todas {
            self.fechar_gpu(x, Estado::Cancelada, motivo);
        }
    }

    pub(super) fn expirar_na_gpu(&self) {
        let agora = agora_ms();
        let vencidas: Vec<NaGpu> = match self.gpu.lock() {
            Ok(mut g) => {
                let (vencidas, ficam): (Vec<_>, Vec<_>) = g.drain(..).partition(|x| x.item.tarefa.vencida(agora));
                g.extend(ficam);
                vencidas
            }
            Err(_) => return,
        };
        for x in vencidas {
            self.fechar_gpu(x, Estado::Expirada, "prazo vencido: a janela parou de mandar o resultado da GPU");
        }
    }

    pub(super) fn fechar_gpu(&self, mut x: NaGpu, estado: Estado, motivo: &str) {
        let numero = x.item.tarefa.numero();
        let _ = x.item.tarefa.avancar(estado, agora_ms(), motivo);
        self.sair(numero, x.memoria);
        let ms = agora_ms().saturating_sub(x.inicio_ms);
        self.encerrar(&x.item, None, ms, motivo);
    }
}
