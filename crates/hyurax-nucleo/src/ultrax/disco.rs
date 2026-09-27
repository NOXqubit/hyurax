//! Placar, histórico e telemetria em disco.

#[allow(clippy::wildcard_imports)]
use super::*;

impl Ultrax {
    // -----------------------------------------------------------------------
    // Disco e telemetria
    // -----------------------------------------------------------------------

    /// Manda as amostras dos motores para `f` (a visualização).
    pub fn ao_amostrar(&self, f: SaidaDeAmostras) {
        if let Ok(mut s) = self.saida_de_amostras.lock() {
            *s = Some(f);
        }
    }

    /// Manda cada passo de cada tarefa para `f`.
    pub fn ao_marcar(&self, f: SaidaDeTarefas) {
        if let Ok(mut s) = self.saida_de_tarefas.lock() {
            *s = Some(f);
        }
    }

    /// Tarefas encerradas desde que abriu (qualquer estado final).
    pub fn encerradas(&self) -> u64 {
        self.encerrados.load(Ordering::Relaxed)
    }

    /// Tarefas com desfecho final desde que abriu.
    pub fn finalizadas(&self) -> u64 {
        self.finais.load(Ordering::Relaxed)
    }

    /// As `n` últimas tarefas encerradas, da mais nova para a mais velha.
    pub fn historico_recente(&self, n: usize) -> Vec<Lembranca> {
        self.historico.lock().map(|h| h.iter().take(n).cloned().collect()).unwrap_or_default()
    }

    /// Há tarefa executando agora?
    pub fn tem_ativas(&self) -> bool {
        self.ativas.lock().is_ok_and(|a| !a.is_empty())
    }

    /// Grava o placar em disco.
    pub fn gravar_placar(&self) {
        let texto = self.placar().texto();
        let arquivo = self.pasta.join("placar.txt");
        let temporario = arquivo.with_extension("tmp");
        if std::fs::write(&temporario, texto).is_ok() {
            let _ = std::fs::rename(&temporario, &arquivo);
        }
    }

    /// Acrescenta uma linha, de uma escrita só; passando do tamanho, o arquivo
    /// atual vira `.1` (e o `.1` anterior se perde: o histórico guarda as
    /// tarefas mais recentes, não todas desde sempre).
    pub(super) fn gravar_no_arquivo(&self, nome: &str, linha: &str, maximo: u64) {
        let _vez = self.escrita.lock();
        let arquivo = self.pasta.join(nome);
        if std::fs::metadata(&arquivo).is_ok_and(|m| m.len() > maximo) {
            let _ = std::fs::rename(&arquivo, self.pasta.join(format!("{nome}.1")));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&arquivo) {
            let mut bytes = linha.as_bytes().to_vec();
            bytes.push(b'\n');
            let _ = f.write_all(&bytes);
        }
    }

    pub(super) fn carregar_historico_recente(&self) {
        let Ok(texto) = std::fs::read_to_string(self.pasta.join("historico.txt")) else { return };
        let mut recentes = VecDeque::new();
        for linha in texto.lines().rev().take(HISTORICO_NA_MEMORIA) {
            if let Some(r) = Registro::ler(linha) {
                recentes.push_back(Lembranca {
                    numero: r.numero(),
                    tipo: r.especificacao.tipo(),
                    resumo: r.especificacao.resumo(),
                    metodo: r.metodo,
                    desafio: r.desafio,
                    estado: r.estado,
                    operacoes: r.operacoes,
                    ms_calculo: r.ms_calculo,
                    fim_ms: r.fim_ms.unwrap_or(r.criada_ms),
                    resultado: r.resultado,
                    nota: r.nota.clone(),
                });
            }
        }
        if let Ok(mut h) = self.historico.lock() {
            *h = recentes;
        }
    }

    pub(super) fn marcar(&self, tarefa: u32, evento: &'static str, detalhe: String) {
        if let Some(f) = self.saida_de_tarefas.lock().ok().and_then(|s| s.clone()) {
            f(tarefa, evento, &detalhe);
        }
        let ms = agora_ms();
        if self.debug.load(Ordering::Relaxed) {
            let linha = format!("{ms} {MODO} #{tarefa:08} {evento} {detalhe}");
            self.gravar_no_arquivo("telemetria.log", &linha, TELEMETRIA_MAX_BYTES);
        }
        if let Ok(mut t) = self.telemetria.lock() {
            t.push_front(Marca { ms, tarefa, evento, detalhe });
            t.truncate(TELEMETRIA_NA_MEMORIA);
        }

    }
}
