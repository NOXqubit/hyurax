//! As linhas de CPU: executar, assinar, conferir e fechar cada tarefa, e o limitador de CPU.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Ultrax {
    // -----------------------------------------------------------------------
    // Worker
    // -----------------------------------------------------------------------

    pub(super) fn linha(self: Arc<Self>, i: u32) {
        loop {
            if self.encerrado() {
                return;
            }
            if !self.ligado.load(Ordering::Relaxed) || i >= self.linhas.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(300));
                continue;
            }
            let item = match self.fila.lock() {
                Ok(mut f) => match f.pop_front() {
                    Some(item) => Some(item),
                    None => {
                        // espera algo entrar; o tempo máximo cobre desligar e mudar o número de linhas
                        let _ = self.tem_na_fila.wait_timeout(f, Duration::from_millis(200));
                        None
                    }
                },
                Err(_) => None,
            };
            if let Some(item) = item {
                // a fila andou: o gerador repõe enquanto esta linha calcula
                self.pedir_mais();
                self.processar(i, item);
                // e ao terminar, a unidade seguinte do JOB pode ter ficado livre
                self.pedir_mais();
            }
        }
    }

    /// Reserva memória para uma tarefa, se couber no teto.
    pub(super) fn reservar(&self, bytes: u64) -> bool {
        let teto = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
        self.reservada
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |r| r.checked_add(bytes).filter(|&t| t <= teto))
            .is_ok()
    }

    pub(super) fn liberar(&self, bytes: u64) {
        let _ = self.reservada.fetch_update(Ordering::AcqRel, Ordering::Acquire, |r| Some(r.saturating_sub(bytes)));
    }

    /// Acorda o gerador para repor a fila.
    pub(super) fn pedir_mais(&self) {
        if let Ok(mut p) = self.pede_mais.lock() {
            *p = true;
        }
        self.pede_mais_cv.notify_one();
    }

    /// Leva uma tarefa da fila até o fim do ciclo, ou até ser interrompida.
    #[allow(clippy::too_many_lines)]
    pub(super) fn processar(&self, linha: u32, mut item: NaFila) {
        let numero = item.tarefa.numero();
        let esp = item.tarefa.especificacao;
        let agora = agora_ms();
        if item.tarefa.vencida(agora) {
            let _ = item.tarefa.avancar(Estado::Expirada, agora, "prazo vencido na fila");
            self.encerrar(&item, None, 0, "prazo vencido na fila");
            return;
        }

        // Gerenciador de recursos: a memória é reservada antes de atribuir, ou
        // a tarefa não roda.
        let memoria = esp.memoria_bytes();
        // unidade de um JOB cancelado ou concluído enquanto esperava na fila
        if let (Some((job, i)), Some(a)) = (item.job, self.agendador())
            && !a.ainda_quer(&job, i)
        {
            let motivo = "o JOB foi cancelado ou já terminou";
            let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), motivo);
            self.encerrar(&item, None, 0, motivo);
            return;
        }
        if !self.reservar(memoria) {
            let teto = u64::from(self.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024);
            if memoria > teto {
                let motivo = format!("pede {:.1} MiB e o teto é {:.0} MiB", mib(memoria), mib(teto));
                let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), motivo.clone());
                // não cabe e não vai caber: conta como falha da unidade, em vez
                // de voltar para a fila e girar sem fim
                item.recusas = item.recusas.max(2);
                self.encerrar(&item, None, 0, &motivo);
            } else {
                // cabe no teto, mas outra linha está usando: espera a vez na fila
                self.enfileirar(item, false);
                std::thread::sleep(Duration::from_millis(500));
            }
            return;
        }
        if item.tarefa.avancar(Estado::Atribuida, agora_ms(), format!("linha {linha}")).is_err() {
            self.liberar(memoria);
            return;
        }
        self.marcar(numero, "TASK ASSIGNED", format!("linha {linha} · worker {}", curto(&self.worker)));
        self.marcar(
            numero,
            "RESOURCE ALLOCATED",
            format!(
                "{:.1} MiB reservados · CPU até {}% · 1 núcleo · GPU não usada",
                mib(memoria),
                self.uso_cpu.load(Ordering::Relaxed)
            ),
        );

        let semente = match item.desafio {
            Some(_) => semente_do_desafio(&esp),
            None => item.tarefa.semente_para(&self.worker),
        };
        let entrada = hash_da_entrada(&esp, &semente).ok();
        let total = operacoes_da_instancia(&esp, &semente);
        let feitas = Arc::new(AtomicU64::new(0));
        if let Ok(mut a) = self.ativas.lock() {
            a.push(Ativa {
                linha,
                gpu: None,
                numero,
                id: item.tarefa.id,
                especificacao: esp,
                metodo: item.tarefa.metodo,
                desafio: item.desafio.is_some(),
                estado: Estado::Executando,
                feitas: Arc::clone(&feitas),
                total,
                operacoes: 0,
                inicio_ms: agora_ms(),
                memoria,
                entrada,
                job: item.job,
            });
        }
        let _ = item.tarefa.avancar(Estado::Executando, agora_ms(), "");
        self.marcar(numero, "WORK STARTED", format!("{} {}", esp.tipo().descricao(), esp.resumo()));
        if let (Some((job, i)), Some(a)) = (item.job, self.agendador()) {
            a.comecou(&job, i, linha, entrada);
        }

        let inicio_ms = agora_ms();
        let mut controle = Controle::novo(self, linha, item.tarefa.prazo_ms, &feitas).da_unidade(item.job);
        let mut observador = ObservadorDaLinha {
            saida: self.saida_de_amostras.lock().ok().and_then(|s| s.clone()),
            contexto: ContextoDaAmostra {
                tarefa: numero,
                job: item.job,
                linha,
                recurso: "CPU",
                desafio: item.desafio.is_some(),
                resumo: format!("{} {}", esp.tipo().descricao(), esp.resumo()),
                especificacao: esp,
            },
            ultima: None,
        };
        let execucao = trabalho::executar_observado(&esp, &semente, &mut |ops| controle.passo(ops), &mut observador);
        let ms_calculo = u64::try_from(controle.trabalhando.as_millis()).unwrap_or(u64::MAX);
        let exec = match execucao {
            Ok(exec) => exec,
            Err(e) => {
                let (estado, motivo) = controle.desfecho(&e);
                let _ = item.tarefa.avancar(estado, agora_ms(), motivo.clone());
                self.sair(numero, memoria);
                self.somar_tempo(ms_calculo, 0);
                self.encerrar(&item, None, ms_calculo, &motivo);
                return;
            }
        };
        self.concluir(Conclusao { linha, item, esp, semente, entrada, exec, inicio_ms, ms_calculo, memoria, total, feitas, gpu: None });
    }

    /// Depois da execução, na CPU ou na GPU: assinar, conferir e liquidar.
    #[allow(clippy::too_many_lines)]
    pub(super) fn concluir(&self, c: Conclusao) {
        let Conclusao { linha, mut item, esp, semente, entrada, mut exec, inicio_ms, ms_calculo, memoria, total, feitas, gpu } = c;
        let numero = item.tarefa.numero();
        let fim_ms = agora_ms();
        if ms_calculo > 50 {
            let ops_s = exec.operacoes as f64 / (ms_calculo as f64 / 1000.0);
            if gpu.is_some() {
                if let Ok(mut r) = self.ritmo_gpu.lock() {
                    *r = *r * 0.5 + ops_s * 0.5;
                }
            } else if let Ok(mut r) = self.ritmo.lock()
                && let Some(v) = r.get_mut(indice(esp.tipo()))
            {
                *v = *v * 0.5 + ops_s * 0.5;
            }
        }
        let onde = gpu.as_deref().map_or_else(|| "de cálculo".to_string(), |nome| format!("na GPU ({nome}), no relógio"));
        self.marcar(
            numero,
            "WORK COMPLETED",
            format!(
                "{} operações em {:.2} s {onde} ({:.1} M/s) · {:.2} s no relógio",
                exec.operacoes,
                ms_calculo as f64 / 1000.0,
                exec.operacoes as f64 / (ms_calculo.max(1) as f64 * 1000.0),
                (fim_ms.saturating_sub(inicio_ms)) as f64 / 1000.0
            ),
        );

        // Proof engine: o registro assinado pela chave do worker.
        let registro = RegistroDeProva {
            tarefa: item.tarefa.id,
            entrada: entrada.unwrap_or([0; HASH_LEN]),
            especificacao: esp,
            metodo: item.tarefa.metodo,
            resultado: hash_do_resultado(&exec.resultado),
            operacoes: exec.operacoes,
            worker: self.worker,
            inicio_ms,
            fim_ms,
        };
        let assinatura = registro.assinar(&self.segredo);
        let _ = item.tarefa.avancar(Estado::Enviada, agora_ms(), "");
        self.marcar(numero, "RESULT SUBMITTED", format!("RESULT_HASH {}…", curto(&registro.resultado)));

        // Verification engine.
        let _ = item.tarefa.avancar(Estado::Verificando, agora_ms(), "");
        let total_verificacao = match (item.desafio, esp.tipo()) {
            (Some(_), _) => 1,
            // a mochila refaz a programação dinâmica da instância, o mesmo total da execução
            (None, TipoDeTrabalho::Mochila) => total,
            (None, _) => trabalho::operacoes_de_verificacao(&esp),
        };
        self.mudar_fase(numero, Estado::Verificando, total_verificacao);
        self.marcar(
            numero,
            "VERIFICATION STARTED",
            format!(
                "{} · autoconferência na CPU desta máquina ({MODO}){}",
                item.tarefa.metodo.nome(),
                if gpu.is_some() { " · o resultado veio da GPU" } else { "" }
            ),
        );
        let comeco_verificacao = Instant::now();
        // a conferência é sempre na CPU, com outro algoritmo: é o que pega o erro da GPU
        let mut controle = Controle::novo(self, linha, item.tarefa.prazo_ms, &feitas);
        let julgamento: Result<Parecer, ErroDeTrabalho> = if !assinatura.is_some_and(|a| registro.assinatura_confere(&a)) {
            Ok(Parecer::Recusado(Recusa("o registro de prova não tem assinatura válida deste worker".into())))
        } else if let Some(d) = item.desafio.and_then(|q| DESAFIOS.get(q)) {
            feitas.store(1, Ordering::Relaxed);
            Ok(if hex(&registro.resultado) == d.esperado {
                Parecer::Aceito
            } else {
                Parecer::Recusado(Recusa(
                    "a resposta difere da que o gabarito conhece: esta máquina calculou errado (defeito de hardware ou programa alterado)".into(),
                ))
            })
        } else {
            trabalho::verificar_controlado(&esp, &semente, &exec.resultado, &mut |ops| controle.passo(ops)).map(|r| match r {
                Ok(()) => Parecer::Aceito,
                Err(recusa) => Parecer::Recusado(recusa),
            })
        };
        // cálculo puro, como o da execução: as pausas do limite de CPU ficam de fora
        let ms_verificacao = if item.desafio.is_some() {
            u64::try_from(comeco_verificacao.elapsed().as_millis()).unwrap_or(u64::MAX)
        } else {
            u64::try_from(controle.trabalhando.as_millis()).unwrap_or(u64::MAX)
        };
        self.sair(numero, memoria);
        self.somar_tempo(ms_calculo, ms_verificacao);
        let parecer = match julgamento {
            Ok(p) => p,
            Err(e) => {
                let (estado, motivo) = controle.desfecho(&e);
                let motivo = format!("{motivo}; o resultado ficou sem julgamento");
                let _ = item.tarefa.avancar(estado, agora_ms(), motivo.clone());
                self.encerrar(&item, Some((&registro, assinatura)), ms_calculo, &motivo);
                return;
            }
        };
        if let Ok(mut p) = self.placar.lock() {
            p.reputacao.registrar(&parecer);
        }

        match parecer {
            Parecer::Aceito => {
                let _ = item.tarefa.avancar(Estado::Verificada, agora_ms(), "");
                self.marcar(
                    numero,
                    "VERIFICATION PASSED",
                    format!("{} · {:.2} s de cálculo · autoconferência nesta máquina ({MODO})", item.tarefa.metodo.nome(), ms_verificacao as f64 / 1000.0),
                );
                if let Ok(mut p) = self.placar.lock() {
                    p.score.somar(exec.operacoes, true);
                    p.liquidadas = p.liquidadas.saturating_add(1);
                    if let Some(c) = p.por_tipo.get_mut(indice(esp.tipo())) {
                        *c = c.saturating_add(1);
                    }
                    if item.desafio.is_some() {
                        p.desafios_certos = p.desafios_certos.saturating_add(1);
                    }
                }
                let pontos = WorkScore { operacoes_verificadas: exec.operacoes, operacoes_sem_credito: 0 }.texto();
                let _ = item.tarefa.avancar(Estado::Liquidada, agora_ms(), format!("+{pontos} de Work Score"));
                self.marcar(numero, "SETTLEMENT COMPLETED", format!("+{pontos} de Work Score · medida de contribuição, sem valor em HYX"));
                if esp.tipo() == TipoDeTrabalho::Ia && item.desafio.is_none() && item.job.is_none() {
                    self.guardar_modelo(numero, &exec);
                }
                self.encerrar(&item, Some((&registro, assinatura)), ms_calculo, "");
                if let (Some((job, indice)), Some(a)) = (item.job, self.agendador()) {
                    a.terminou(DesfechoDeUnidade {
                        job,
                        indice,
                        tarefa: item.tarefa.id,
                        estado: Estado::Liquidada,
                        resultado: Some(std::mem::take(&mut exec.resultado)),
                        registro: Some(registro.clone()),
                        assinatura,
                        operacoes_verificacao: total_verificacao,
                        ms_calculo,
                        ms_verificacao,
                        memoria,
                        gpu: gpu.clone(),
                        nota: String::new(),
                        abandonada: false,
                    });
                }
            }
            Parecer::Recusado(recusa) => {
                let _ = item.tarefa.avancar(Estado::Recusada, agora_ms(), recusa.0.clone());
                self.marcar(numero, "VERIFICATION FAILED", recusa.0.clone());
                let segunda = item.recusas > 0;
                if let Ok(mut p) = self.placar.lock() {
                    p.score.somar(exec.operacoes, false);
                    p.recusadas = p.recusadas.saturating_add(1);
                    // o desafio conta uma vez, pelo desfecho: errado é errar as duas vezes
                    if segunda && item.desafio.is_some() {
                        p.desafios_errados = p.desafios_errados.saturating_add(1);
                    }
                }
                (self.aviso)("ultrax", format!("tarefa #{numero} recusada na conferência: {recusa}"));
                self.encerrar(&item, Some((&registro, assinatura)), ms_calculo, &recusa.0);
                if let (Some((job, indice)), Some(a)) = (item.job, self.agendador()) {
                    a.terminou(DesfechoDeUnidade {
                        job,
                        indice,
                        tarefa: item.tarefa.id,
                        estado: Estado::Recusada,
                        resultado: None,
                        registro: Some(registro.clone()),
                        assinatura,
                        operacoes_verificacao: total_verificacao,
                        ms_calculo,
                        ms_verificacao,
                        memoria,
                        gpu: gpu.clone(),
                        nota: recusa.0.clone(),
                        abandonada: false,
                    });
                }
                item.recusas = item.recusas.saturating_add(1);
                if segunda {
                    let _ = item.tarefa.avancar(Estado::Cancelada, agora_ms(), "recusada duas vezes");
                    self.encerrar(&item, None, 0, "recusada duas vezes: erro que se repete");
                } else if item.tarefa.avancar(Estado::NaFila, agora_ms(), "repetida para separar defeito passageiro de erro sistemático").is_ok() {
                    // de novo, uma vez: erro que some era passageiro; erro que volta é sistemático
                    self.marcar(numero, "TASK QUEUED", "repetida uma vez depois da recusa".into());
                    self.enfileirar(item, true);
                }
            }
            // O worker LAB só julga por conferência; maioria e repetição não acontecem aqui.
            Parecer::Divergente | Parecer::Repetida => {}
        }
    }

    /// Um treino verificado: vira o último treino, e o melhor se errar menos.
    pub(super) fn guardar_modelo(&self, numero: u32, exec: &trabalho::Execucao) {
        let Some((pesos, erro)) = ia::decodificar(&exec.resultado) else { return };
        let texto = match self.modelo.lock() {
            Ok(mut m) => {
                m.treinos = m.treinos.saturating_add(1);
                m.ultima_curva = resumir_curva(&exec.curva, 96);
                m.ultimo_erro = Some(erro);
                if m.melhor.as_ref().is_none_or(|(_, e)| erro < *e) {
                    m.melhor = Some((pesos, erro));
                    m.tarefa = numero;
                    let base = ia::Base::embutida();
                    self.marcar(
                        numero,
                        "MODEL IMPROVED",
                        format!("novo melhor modelo: erro típico de {:.2} em log S", ia::rmse_em_logs(erro, base) as f64 / 1000.0),
                    );
                }
                m.texto()
            }
            Err(_) => return,
        };
        let _vez = self.escrita.lock();
        let arquivo = self.pasta.join("modelo.txt");
        let temporario = arquivo.with_extension("tmp");
        if std::fs::write(&temporario, texto).is_ok() {
            let _ = std::fs::rename(&temporario, &arquivo);
        }
    }

    /// O painel da IA: o melhor modelo, o último treino e uma molécula de
    /// validação com a solubilidade medida e a prevista pelo melhor modelo.
    /// A molécula muda a cada 8 segundos.
    pub(super) fn json_ia(&self) -> String {
        let base = ia::Base::embutida();
        let Ok(m) = self.modelo.lock() else { return "null".into() };
        let (_, validacao) = base.divisao();
        let vez = usize::try_from(agora_ms() / 8000).unwrap_or(0);
        let amostra = validacao.get(vez % validacao.len().max(1)).and_then(|&k| base.moleculas.get(k));
        let mut j = String::with_capacity(2048);
        let _ = write!(
            j,
            "{{\"moleculas\":{},\"validacao\":{},\"treinos\":{},\"tarefa\":{},\"melhor_erro\":{},\"melhor_rmse_mili\":{},\
             \"ultimo_rmse_mili\":{},\"ultima_curva\":[{}],\"desvio_mili\":{},\"descritores\":[{}],\"amostra\":",
            base.moleculas.len(),
            validacao.len(),
            m.treinos,
            m.tarefa,
            m.melhor.as_ref().map_or("null".into(), |(_, e)| e.to_string()),
            m.melhor.as_ref().map_or("null".into(), |(_, e)| ia::rmse_em_logs(*e, base).to_string()),
            m.ultimo_erro.map_or("null".into(), |e| ia::rmse_em_logs(e, base).to_string()),
            m.ultima_curva.iter().map(|&c| ia::rmse_em_logs(c, base).to_string()).collect::<Vec<_>>().join(","),
            base.desvio_mili,
            ia::DESCRITORES.iter().map(|d| texto_json(d)).collect::<Vec<_>>().join(","),
        );
        match amostra {
            Some(mol) => {
                let previsto = m.melhor.as_ref().map(|(p, _)| base.logs_de(ia::prever(p, &mol.x).0));
                let _ = write!(
                    j,
                    "{{\"id\":{},\"nome\":{},\"formula\":{},\"smiles\":{},\"massa_mili\":{},\"medido_mili\":{},\"previsto_mili\":{}}}}}",
                    texto_json(&mol.id),
                    texto_json(&mol.nome),
                    texto_json(&mol.formula),
                    texto_json(&mol.smiles),
                    mol.massa_mili,
                    mol.logs_mili,
                    previsto.map_or("null".into(), |v| v.to_string()),
                );
            }
            None => j.push_str("null}"),
        }
        j
    }

    pub(super) fn json_gpu(&self) -> String {
        format!(
            "{{\"ligada\":{},\"uso\":{},\"nome\":{},\"tarefas\":{},\"ritmo\":{:.0}}}",
            self.gpu_ligada.load(Ordering::Relaxed),
            self.gpu_uso.load(Ordering::Relaxed),
            texto_json(&self.gpu_nome.lock().map(|g| g.clone()).unwrap_or_default()),
            self.gpu.lock().map_or(0, |g| g.len()),
            self.ritmo_gpu.lock().map_or(0.0, |r| *r),
        )
    }

    pub(super) fn somar_tempo(&self, ms_calculo: u64, ms_verificacao: u64) {
        if let Ok(mut p) = self.placar.lock() {
            p.ms_calculo = p.ms_calculo.saturating_add(ms_calculo);
            p.ms_verificacao = p.ms_verificacao.saturating_add(ms_verificacao);
        }
    }

    pub(super) fn mudar_fase(&self, numero: u32, estado: Estado, total: u64) {
        if let Ok(mut a) = self.ativas.lock()
            && let Some(x) = a.iter_mut().find(|x| x.numero == numero)
        {
            x.operacoes = x.feitas.swap(0, Ordering::Relaxed);
            x.estado = estado;
            x.total = total.max(1);
        }
    }

    pub(super) fn sair(&self, numero: u32, memoria: u64) {
        if let Ok(mut a) = self.ativas.lock() {
            a.retain(|x| x.numero != numero);
        }
        self.liberar(memoria);
    }

    /// Fecha um trecho do ciclo: assina o veredito, grava no histórico, conta
    /// os encerramentos sem julgamento, grava o placar e guarda a lembrança
    /// para o painel.
    pub(super) fn encerrar(&self, item: &NaFila, registro: Option<(&RegistroDeProva, Option<[u8; 64]>)>, ms_calculo: u64, nota: &str) {
        let t = &item.tarefa;
        match t.estado {
            // recusada duas vezes é erro que se repete, não parada de quem manda
            Estado::Cancelada if item.recusas >= 2 => {
                if let Ok(mut p) = self.placar.lock() {
                    p.abandonadas = p.abandonadas.saturating_add(1);
                }
                self.marcar(t.numero(), "TASK ABANDONED", nota.to_string());
            }
            Estado::Cancelada => {
                if let Ok(mut p) = self.placar.lock() {
                    p.canceladas = p.canceladas.saturating_add(1);
                }
                self.marcar(t.numero(), "TASK CANCELLED", nota.to_string());
            }
            Estado::Expirada => {
                if let Ok(mut p) = self.placar.lock() {
                    p.expiradas = p.expiradas.saturating_add(1);
                }
                self.marcar(t.numero(), "TASK EXPIRED", nota.to_string());
            }
            _ => {}
        }
        // o veredito vale para este registro: trocar o estado no arquivo depois quebra a assinatura
        let veredito = registro.and_then(|(r, _)| r.assinar_veredito(&self.segredo, t.estado.nome()));
        let linha = linha_do_historico(item, registro, veredito, ms_calculo, nota);
        self.gravar_no_arquivo("historico.txt", &linha, HISTORICO_MAX_BYTES);
        let lembranca = Lembranca {
            numero: t.numero(),
            tipo: t.especificacao.tipo(),
            resumo: t.especificacao.resumo(),
            metodo: t.metodo,
            desafio: item.desafio.is_some(),
            estado: t.estado,
            operacoes: registro.map_or(0, |(r, _)| r.operacoes),
            ms_calculo,
            fim_ms: agora_ms(),
            resultado: registro.map(|(r, _)| r.resultado),
            nota: nota.to_string(),
        };
        if let Ok(mut h) = self.historico.lock() {
            h.push_front(lembranca);
            h.truncate(HISTORICO_NA_MEMORIA);
            // soma com o histórico travado: quem lê os dois juntos nunca vê um sem o outro
            self.encerrados.fetch_add(1, Ordering::Relaxed);
        }
        if t.estado.e_final() {
            self.finais.fetch_add(1, Ordering::Relaxed);
        }
        self.gravar_placar();
        // liquidada e recusada avisam em concluir(), com o resultado na mão
        if matches!(t.estado, Estado::Cancelada | Estado::Expirada)
            && let (Some((job, indice)), Some(a)) = (item.job, self.agendador())
        {
            a.terminou(DesfechoDeUnidade {
                job,
                indice,
                tarefa: t.id,
                estado: t.estado,
                resultado: None,
                registro: registro.map(|(r, _)| r.clone()),
                assinatura: registro.and_then(|(_, a)| a),
                operacoes_verificacao: 0,
                ms_calculo,
                ms_verificacao: 0,
                memoria: 0,
                gpu: None,
                nota: nota.to_string(),
                abandonada: item.recusas >= 2,
            });
        }
    }
}

pub(super) fn curto(bytes: &[u8]) -> String {
    hex(bytes.get(..8).unwrap_or(bytes))
}

/// Limite de CPU, prazo e botão de parar, aplicados a cada pedaço de trabalho.
pub(super) struct Controle<'a> {
    pub(super) u: &'a Ultrax,
    pub(super) linha: u32,
    pub(super) prazo_ms: u64,
    pub(super) feitas: &'a AtomicU64,
    pub(super) marco: Instant,
    /// Tempo de cálculo puro, sem as pausas.
    pub(super) trabalhando: Duration,
    /// Pausa devida e ainda não dormida.
    pub(super) divida: Duration,
    pub(super) motivo: Option<&'static str>,
    /// A unidade de JOB que este pedaço calcula, se for unidade de JOB.
    pub(super) job: Option<([u8; HASH_LEN], u64)>,
    /// Última consulta ao agendador, e se ele já disse que não quer mais.
    consultado: std::cell::Cell<Option<Instant>>,
    desistiu: std::cell::Cell<bool>,
}

impl<'a> Controle<'a> {
    pub(super) fn novo(u: &'a Ultrax, linha: u32, prazo_ms: u64, feitas: &'a AtomicU64) -> Self {
        Self {
            u,
            linha,
            prazo_ms,
            feitas,
            marco: Instant::now(),
            trabalhando: Duration::ZERO,
            divida: Duration::ZERO,
            motivo: None,
            job: None,
            consultado: std::cell::Cell::new(None),
            desistiu: std::cell::Cell::new(false),
        }
    }

    /// Liga o controle à unidade de JOB: se o JOB for cancelado, o cálculo para.
    pub(super) fn da_unidade(mut self, job: Option<([u8; HASH_LEN], u64)>) -> Self {
        self.job = job;
        self
    }

    /// O JOB desistiu desta unidade? Pergunta ao agendador no máximo duas
    /// vezes por segundo (a trava dos JOBs não entra em cada pedaço).
    fn job_desistiu(&self) -> bool {
        let Some((job, indice)) = self.job else { return false };
        if self.desistiu.get() {
            return true;
        }
        if self.consultado.get().is_some_and(|t| t.elapsed() < Duration::from_millis(500)) {
            return false;
        }
        self.consultado.set(Some(Instant::now()));
        let desistiu = self.u.agendador().is_some_and(|a| !a.ainda_quer(&job, indice));
        self.desistiu.set(desistiu);
        desistiu
    }

    /// Chamado a cada pedaço. Devolve `false` para interromper.
    pub(super) fn passo(&mut self, ops: u64) -> bool {
        self.feitas.fetch_add(ops, Ordering::Relaxed);
        let gasto = self.marco.elapsed();
        self.trabalhando = self.trabalhando.saturating_add(gasto);
        if let Some(motivo) = self.parar() {
            self.motivo = Some(motivo);
            return false;
        }
        let uso = f64::from(self.u.uso_cpu.load(Ordering::Relaxed).clamp(10, 100));
        if uso < 100.0 {
            // com 25%, cada segundo de cálculo pede três de descanso
            self.divida = self.divida.saturating_add(gasto.mul_f64(100.0 / uso - 1.0));
            if self.divida >= PAUSA_MINIMA {
                let pedido = self.divida.min(PAUSA_MAXIMA);
                let comeco = Instant::now();
                // dorme em fatias, para o botão de parar responder logo
                while comeco.elapsed() < pedido {
                    if let Some(motivo) = self.parar() {
                        self.motivo = Some(motivo);
                        return false;
                    }
                    std::thread::sleep((pedido.saturating_sub(comeco.elapsed())).min(Duration::from_millis(100)));
                }
                self.divida = self.divida.saturating_sub(comeco.elapsed());
            }
        }
        self.marco = Instant::now();
        true
    }

    pub(super) fn parar(&self) -> Option<&'static str> {
        if !self.u.ligado.load(Ordering::Relaxed) {
            Some("o ULTRAX foi desligado")
        } else if self.linha >= self.u.linhas.load(Ordering::Relaxed) {
            Some("a linha foi desligada nos ajustes")
        } else if agora_ms() > self.prazo_ms {
            Some("prazo vencido")
        } else if self.job_desistiu() {
            Some("o JOB foi cancelado ou já terminou")
        } else if self.u.reservada.load(Ordering::Relaxed)
            > u64::from(self.u.memoria_mib.load(Ordering::Relaxed)).saturating_mul(1024 * 1024)
        {
            // baixaram o teto com tarefas rodando: elas param, e a memória volta
            Some("o teto de memória baixou")
        } else {
            None
        }
    }

    /// O estado e o motivo de uma interrupção.
    pub(super) fn desfecho(&self, e: &ErroDeTrabalho) -> (Estado, String) {
        match (e, self.motivo) {
            (ErroDeTrabalho::Cancelado, Some("prazo vencido")) => (Estado::Expirada, "prazo vencido".into()),
            (ErroDeTrabalho::Cancelado, Some(motivo)) => (Estado::Cancelada, motivo.into()),
            (outro, _) => (Estado::Cancelada, outro.to_string()),
        }
    }
}

/// O observador de uma linha: repassa as amostras do motor, no máximo uma a
/// cada [`INTERVALO_DAS_AMOSTRAS`]. Não mexe no cálculo (ver
/// `hyurax_ultrax::observador`).
pub(super) struct ObservadorDaLinha {
    pub(super) saida: Option<SaidaDeAmostras>,
    pub(super) contexto: ContextoDaAmostra,
    pub(super) ultima: Option<Instant>,
}

impl hyurax_ultrax::observador::Observador for ObservadorDaLinha {
    fn quer(&mut self) -> bool {
        if self.saida.is_none() {
            return false;
        }
        let agora = Instant::now();
        if self.ultima.is_some_and(|u| agora.duration_since(u) < INTERVALO_DAS_AMOSTRAS) {
            return false;
        }
        self.ultima = Some(agora);
        true
    }

    fn amostra(&mut self, a: hyurax_ultrax::observador::Amostra) {
        if let Some(f) = &self.saida {
            f(&self.contexto, a);
        }
    }
}
