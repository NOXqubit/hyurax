//! O gerador de tarefas do modo LAB e o orquestrador da fila.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Ultrax {
    // -----------------------------------------------------------------------
    // Gerador de tarefas (LAB)
    // -----------------------------------------------------------------------

    pub(super) fn orquestrar(self: Arc<Self>) {
        let mut ultimo_gravado = Instant::now();
        let mut proximo_segundo = Instant::now().checked_add(Duration::from_secs(1)).unwrap_or_else(Instant::now);
        loop {
            // espera o próximo segundo, ou uma linha pedindo mais trabalho:
            // com tarefa curta, esperar o segundo inteiro deixava as linhas paradas
            let restante = proximo_segundo.saturating_duration_since(Instant::now());
            if let Ok(pedido) = self.pede_mais.lock()
                && let Ok((mut pedido, _)) = self.pede_mais_cv.wait_timeout_while(pedido, restante, |p| !*p)
            {
                *pedido = false;
            }
            let agora = Instant::now();
            let virou_o_segundo = agora >= proximo_segundo;
            if virou_o_segundo {
                proximo_segundo = proximo_segundo.checked_add(Duration::from_secs(1)).unwrap_or(agora);
                if proximo_segundo < agora {
                    // ficou para trás (máquina travada): não conta segundos que não viu
                    proximo_segundo = agora.checked_add(Duration::from_secs(1)).unwrap_or(agora);
                }
            }
            if self.ligado.load(Ordering::Relaxed) {
                if virou_o_segundo {
                    if let Ok(mut p) = self.placar.lock() {
                        p.segundos_ligado = p.segundos_ligado.saturating_add(1);
                        p.reputacao.segundos_ativo = p.reputacao.segundos_ativo.saturating_add(1);
                    }
                    self.expirar_na_fila();
                    self.expirar_na_gpu();
                }
                let quer = (self.linhas.load(Ordering::Relaxed) as usize).saturating_add(FILA_EXTRA);
                while self.ligado.load(Ordering::Relaxed) && self.fila.lock().map_or(usize::MAX, |f| f.len()) < quer {
                    // unidade de JOB primeiro; a LAB só preenche o que sobra
                    let agendador = self.agendador();
                    let proxima = agendador.as_ref().and_then(|a| a.proxima(agora_ms()));
                    let pedido_de_job = proxima.is_some();
                    let item = match proxima {
                        Some(pedido) => self.unidade_de_job(pedido),
                        None if self.lab.load(Ordering::Relaxed) && !agendador.is_some_and(|a| a.tem_trabalho()) => self.gerar(agora_ms()),
                        None => break,
                    };
                    match item {
                        // unidade de JOB passa na frente da carga LAB que já estava na fila
                        Ok(item) => self.enfileirar(item, pedido_de_job),
                        Err(e) => {
                            self.marcar(0, "GENERATOR ERROR", e);
                            break;
                        }
                    }
                }
            }
            // grava mesmo desligado: o tempo e os cancelamentos do desligar também contam
            if ultimo_gravado.elapsed() >= Duration::from_secs(30) {
                self.gravar_placar();
                ultimo_gravado = Instant::now();
            }
        }
    }

    /// Põe na fila, se o ULTRAX ainda estiver ligado; senão, cancela. A
    /// conferência e o empilhamento acontecem com a fila travada, então nada
    /// entra depois de desligar.
    pub(super) fn enfileirar(&self, item: NaFila, na_frente: bool) {
        let sobrou = match self.fila.lock() {
            Ok(mut f) if self.ligado.load(Ordering::Relaxed) => {
                if na_frente {
                    f.push_front(item);
                } else {
                    f.push_back(item);
                }
                self.tem_na_fila.notify_one();
                None
            }
            _ => Some(item),
        };
        if let Some(mut item) = sobrou {
            let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), "o ULTRAX foi desligado");
            self.encerrar(&item, None, 0, "o ULTRAX foi desligado");
        }
    }

    /// Cria uma tarefa e já a põe em QUEUED.
    pub(super) fn gerar(&self, agora: u64) -> Result<NaFila, String> {
        let k = self.sequencia.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut p) = self.placar.lock() {
            p.geradas = p.geradas.max(k.saturating_add(1));
        }
        let mut origem = [0u8; 32];
        hyurax_net::entropia::preencher(&mut origem)?;
        let e_desafio = k % DESAFIO_A_CADA == DESAFIO_A_CADA - 1;
        let (especificacao, metodo, instancia, prioridade, desafio) = if e_desafio {
            let qual = usize::try_from((k / DESAFIO_A_CADA) % DESAFIOS.len() as u64).unwrap_or(0);
            let d = DESAFIOS.get(qual).ok_or("desafio inexistente")?;
            let esp = Especificacao::nova(d.tipo, d.tamanho, d.passos).map_err(|e| e.to_string())?;
            (esp, MetodoDeVerificacao::ResultadoEsperado, Instancia::Compartilhada, 200, Some(qual))
        } else {
            // os desafios não tiram a vez de ninguém: a rotação conta só as normais
            let normais = k.saturating_sub(k / DESAFIO_A_CADA);
            let tipo = TipoDeTrabalho::ROTACAO_LAB
                .get(usize::try_from(normais % TipoDeTrabalho::ROTACAO_LAB.len() as u64).unwrap_or(0))
                .copied()
                .unwrap_or(TipoDeTrabalho::Matriz);
            let esp = self.dimensionar(tipo)?;
            (esp, tipo.metodo(), Instancia::PorWorker, 100, None)
        };
        let estimado = self.segundos_estimados(&especificacao);
        let prazo = agora.saturating_add(PRAZO_MINIMO_MS.max((estimado / USO_MINIMO * PRAZO_FOLGA * 1000.0) as u64));
        let mut tarefa = Tarefa::nova(especificacao, metodo, instancia, &origem, prioridade, agora, prazo)
            .map_err(|e| e.to_string())?;
        let numero = tarefa.numero();
        let rotulo = if desafio.is_some() { format!("{SELO} · tarefa-desafio de resposta conhecida") } else { SELO.to_string() };
        self.marcar(
            numero,
            "TASK CREATED",
            format!(
                "{} {} · {} · verificação: {} · {rotulo}",
                especificacao.tipo().descricao(),
                especificacao.resumo(),
                especificacao.tipo().categoria().nome(),
                metodo.nome()
            ),
        );
        tarefa.avancar(Estado::NaFila, agora_ms(), "").map_err(|e| e.to_string())?;
        self.marcar(numero, "TASK QUEUED", String::new());
        Ok(NaFila { tarefa, desafio, recusas: 0, origem: origem.to_vec(), job: None })
    }

    /// Transforma uma unidade de JOB numa tarefa, já em QUEUED. A semente é a
    /// da unidade ([`Instancia::DeJob`]), e o método é o do tipo.
    pub(super) fn unidade_de_job(&self, p: PedidoDeUnidade) -> Result<NaFila, String> {
        let agora = agora_ms();
        let metodo = p.especificacao.tipo().metodo();
        let mut tarefa = Tarefa::nova(p.especificacao, metodo, Instancia::DeJob, &p.semente, p.prioridade, agora, p.prazo_ms)
            .map_err(|e| e.to_string())?;
        let numero = tarefa.numero();
        self.marcar(
            numero,
            "TASK CREATED",
            format!(
                "JOB {}… unidade {} · {} {} · {} · verificação: {}",
                curto(&p.job),
                p.indice,
                p.especificacao.tipo().descricao(),
                p.especificacao.resumo(),
                p.especificacao.tipo().categoria().nome(),
                metodo.nome()
            ),
        );
        tarefa.avancar(Estado::NaFila, agora_ms(), "").map_err(|e| e.to_string())?;
        self.marcar(numero, "TASK QUEUED", String::new());
        Ok(NaFila { tarefa, desafio: None, recusas: 0, origem: p.semente.to_vec(), job: Some((p.job, p.indice)) })
    }

    /// Execução e verificação, com a CPU inteira, no ritmo medido.
    pub(super) fn segundos_estimados(&self, esp: &Especificacao) -> f64 {
        let ritmo = self.ritmo.lock().map_or(RITMO_INICIAL, |r| r.get(indice(esp.tipo())).copied().unwrap_or(RITMO_INICIAL));
        let operacoes = esp.operacoes_maximas().saturating_add(trabalho::operacoes_de_verificacao(esp));
        operacoes as f64 / ritmo.max(1.0)
    }

    /// O tamanho que cabe em [`SEGUNDOS_ALVO`] de cálculo neste ritmo, e na
    /// fatia de memória de uma linha.
    pub(super) fn dimensionar(&self, tipo: TipoDeTrabalho) -> Result<Especificacao, String> {
        let ritmo = self.ritmo.lock().map_or(RITMO_INICIAL, |r| r.get(indice(tipo)).copied().unwrap_or(RITMO_INICIAL));
        let alvo = (ritmo * SEGUNDOS_ALVO).max(1.0);
        let linhas = u64::from(self.linhas.load(Ordering::Relaxed).max(1));
        let orcamento = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024) / linhas;
        let cabe = |esp: &Especificacao| esp.memoria_bytes() <= orcamento;
        let erro = |e: ErroDeTrabalho| e.to_string();
        let esp = match tipo {
            TipoDeTrabalho::Ia => {
                // lote de 32; os passos enchem o tempo-alvo
                let lote = 32u32;
                let por_passo = f64::from(lote) * 368.0 + 193.0;
                let mut passos = ((alvo / por_passo) as u32).clamp(16, ia::PASSOS_MAX);
                loop {
                    let esp = Especificacao::nova(tipo, lote, passos).map_err(erro)?;
                    if cabe(&esp) || passos <= 16 {
                        break esp;
                    }
                    passos = passos.saturating_mul(3) / 4;
                }
            }
            TipoDeTrabalho::Matriz => {
                let mut n = (alvo.cbrt() as u32).clamp(32, trabalho::MATRIZ_LADO_MAX);
                loop {
                    let esp = Especificacao::nova(tipo, n, 0).map_err(erro)?;
                    if cabe(&esp) || n <= 32 {
                        break esp;
                    }
                    n = n.saturating_mul(7) / 8;
                }
            }
            TipoDeTrabalho::Mochila => {
                // operações ≈ n · 25n
                let mut n = ((alvo / 25.0).sqrt() as u32).clamp(16, trabalho::MOCHILA_ITENS_MAX);
                loop {
                    let esp = Especificacao::nova(tipo, n, 0).map_err(erro)?;
                    if cabe(&esp) || n <= 16 {
                        break esp;
                    }
                    n = n.saturating_mul(7) / 8;
                }
            }
            TipoDeTrabalho::Difusao => {
                let mut g = 128u32;
                // sobe para a grade maior uma vez só; depois, só desce até caber
                let mut subiu = false;
                loop {
                    let celulas = f64::from(g) * f64::from(g);
                    let passos = ((alvo / celulas) as u32).clamp(1, trabalho::DIFUSAO_PASSOS_MAX);
                    if passos == trabalho::DIFUSAO_PASSOS_MAX && g < trabalho::DIFUSAO_GRADE_MAX && !subiu {
                        g = trabalho::DIFUSAO_GRADE_MAX;
                        subiu = true;
                        continue;
                    }
                    let esp = Especificacao::nova(tipo, g, passos).map_err(erro)?;
                    if cabe(&esp) || g <= 16 {
                        break esp;
                    }
                    g = g.saturating_mul(3) / 4;
                }
            }
            // os tipos científicos só rodam por JOB, com os parâmetros de quem pediu
            TipoDeTrabalho::Genetica | TipoDeTrabalho::Melhoramento | TipoDeTrabalho::Rotas | TipoDeTrabalho::Triagem => {
                return Err(format!("{} só roda por JOB, não no rodízio LAB", tipo.nome()));
            }
        };
        if cabe(&esp) {
            Ok(esp)
        } else {
            Err(format!("o teto de memória não comporta nem a menor tarefa de {}", tipo.descricao()))
        }
    }

    pub(super) fn expirar_na_fila(&self) {
        let agora = agora_ms();
        let vencidas: Vec<NaFila> = match self.fila.lock() {
            Ok(mut f) => {
                let (vencidas, ficam): (Vec<_>, Vec<_>) = f.drain(..).partition(|t| t.tarefa.vencida(agora));
                f.extend(ficam);
                vencidas
            }
            Err(_) => return,
        };
        for mut item in vencidas {
            let _ = item.tarefa.avancar(Estado::Expirada, agora, "prazo vencido na fila");
            self.encerrar(&item, None, 0, "prazo vencido na fila");
        }
    }

    pub(super) fn descartar_fila(&self, motivo: &str) {
        let itens: Vec<NaFila> = self.fila.lock().map(|mut f| f.drain(..).collect()).unwrap_or_default();
        for mut item in itens {
            let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), motivo);
            self.encerrar(&item, None, 0, motivo);
        }
    }
}
