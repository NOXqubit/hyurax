//! O barramento de eventos: um lugar só, em ordem, para tudo o que o núcleo
//! conta.
//!
//! Cada evento tem um número de sequência, a hora, o tipo e um corpo JSON. A
//! tela lê pelo fluxo (`/api/v1/fluxo`, Server-Sent Events) a partir do último
//! número que viu. Os registros de texto também vão para o arquivo de registro
//! (e para o terminal, no programa de terminal).
//!
//! Tipos de evento:
//!
//! | tipo | origem | corpo |
//! |---|---|---|
//! | `registro` | qualquer serviço | `{categoria, texto}` |
//! | `tarefa` | ULTRAX | o passo de uma tarefa (criada, executando, conferida…) |
//! | `ciencia` | JOBs | o evento do JOB, igual ao gravado em `eventos.jsonl` |
//! | `amostra` | motores | o estado real do cálculo (ver `hyurax_ultrax::observador`) |
//! | `bloco` | nó | bloco novo na cadeia |

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::util::agora_ms;

/// Quantos eventos ficam na memória. Quem ficar para trás perde os mais
/// velhos e é avisado pelo salto na sequência.
const CAPACIDADE: usize = 4096;
/// O arquivo de registro recomeça depois deste tamanho (o anterior fica como `.1`).
const REGISTRO_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Um evento.
#[derive(Clone, Debug)]
pub struct Evento {
    /// Número de sequência, crescente, a partir de 1.
    pub seq: u64,
    /// Milissegundos desde 1970.
    pub quando_ms: u64,
    /// O tipo (ver a tabela do módulo).
    pub tipo: &'static str,
    /// O corpo, em JSON.
    pub json: String,
}

impl Evento {
    /// O evento inteiro em JSON.
    pub fn json_completo(&self) -> String {
        format!("{{\"seq\":{},\"quando_ms\":{},\"tipo\":\"{}\",\"dados\":{}}}", self.seq, self.quando_ms, self.tipo, self.json)
    }
}

/// O barramento.
pub struct Barramento {
    seq: AtomicU64,
    fila: Mutex<VecDeque<Evento>>,
    chegou: Condvar,
    registro: Option<PathBuf>,
    terminal: bool,
    escrita: Mutex<()>,
}

impl Barramento {
    /// Um barramento. `registro` é o arquivo de registro (ou nenhum);
    /// `terminal` imprime os registros de texto.
    pub fn novo(registro: Option<PathBuf>, terminal: bool) -> Arc<Self> {
        Arc::new(Self {
            seq: AtomicU64::new(0),
            fila: Mutex::new(VecDeque::with_capacity(CAPACIDADE)),
            chegou: Condvar::new(),
            registro,
            terminal,
            escrita: Mutex::new(()),
        })
    }

    /// Publica um evento. Devolve o número dele.
    pub fn publicar(&self, tipo: &'static str, json: String) -> u64 {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        let ev = Evento { seq, quando_ms: agora_ms(), tipo, json };
        if let Ok(mut f) = self.fila.lock() {
            f.push_back(ev);
            while f.len() > CAPACIDADE {
                f.pop_front();
            }
        }
        self.chegou.notify_all();
        seq
    }

    /// Um registro de texto, para gente ler.
    pub fn registrar(&self, categoria: &'static str, texto: impl Into<String>) {
        let texto = texto.into();
        if self.terminal {
            println!("  [{categoria}] {texto}");
        }
        self.anotar(categoria, &texto);
        self.publicar("registro", serde_json::json!({ "categoria": categoria, "texto": texto }).to_string());
    }

    fn anotar(&self, categoria: &str, texto: &str) {
        let Some(arquivo) = &self.registro else { return };
        let _vez = self.escrita.lock();
        if std::fs::metadata(arquivo).is_ok_and(|m| m.len() > REGISTRO_MAX_BYTES) {
            let _ = std::fs::rename(arquivo, arquivo.with_extension("log.1"));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(arquivo) {
            let _ = writeln!(f, "{} [{categoria}] {texto}", agora_ms() / 1000);
        }
    }

    /// Eventos depois de `seq`, no máximo `max`.
    pub fn desde(&self, seq: u64, max: usize) -> Vec<Evento> {
        self.fila.lock().map(|f| f.iter().filter(|e| e.seq > seq).take(max).cloned().collect()).unwrap_or_default()
    }

    /// Como [`Self::desde`], esperando até `ate` se ainda não houver nada novo.
    pub fn esperar(&self, seq: u64, max: usize, ate: Duration) -> Vec<Evento> {
        let Ok(f) = self.fila.lock() else { return Vec::new() };
        let Ok((f, _)) = self.chegou.wait_timeout_while(f, ate, |f| f.back().is_none_or(|e| e.seq <= seq)) else {
            return Vec::new();
        };
        f.iter().filter(|e| e.seq > seq).take(max).cloned().collect()
    }

    /// Os últimos `n` eventos de um tipo, do mais novo para o mais velho.
    pub fn ultimos(&self, tipo: &str, n: usize) -> Vec<Evento> {
        self.fila.lock().map(|f| f.iter().rev().filter(|e| e.tipo == tipo).take(n).cloned().collect()).unwrap_or_default()
    }

    /// O número do último evento publicado.
    pub fn ultimo_seq(&self) -> u64 {
        self.seq.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    #[test]
    fn sequencia_em_ordem_e_espera_por_evento_novo() {
        let b = Barramento::novo(None, false);
        b.registrar("no", "um");
        b.publicar("bloco", "{\"altura\":1}".into());
        let todos = b.desde(0, 10);
        assert_eq!(todos.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(todos[0].tipo, "registro");
        assert!(todos[0].json.contains("\"texto\":\"um\""));
        assert!(b.desde(2, 10).is_empty());
        // alguém publica enquanto outro espera
        let b2 = Arc::clone(&b);
        let t = std::thread::spawn(move || b2.esperar(2, 10, Duration::from_secs(5)));
        std::thread::sleep(Duration::from_millis(50));
        b.publicar("bloco", "{}".into());
        let novos = t.join().unwrap();
        assert_eq!(novos.len(), 1);
        assert_eq!(novos[0].seq, 3);
        assert!(b.esperar(3, 10, Duration::from_millis(20)).is_empty(), "sem evento novo, volta vazio no prazo");
    }

    #[test]
    fn capacidade_descarta_os_mais_velhos() {
        let b = Barramento::novo(None, false);
        for _ in 0..(CAPACIDADE + 10) {
            b.publicar("x", "{}".into());
        }
        let todos = b.desde(0, usize::MAX);
        assert_eq!(todos.len(), CAPACIDADE);
        assert_eq!(todos[0].seq, 11);
    }

    #[test]
    fn registro_vai_para_o_arquivo() {
        let arq = std::env::temp_dir().join(format!("hyurax-barramento-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&arq);
        let b = Barramento::novo(Some(arq.clone()), false);
        b.registrar("carteira", "criada");
        let texto = std::fs::read_to_string(&arq).unwrap();
        assert!(texto.contains("[carteira] criada"));
        let _ = std::fs::remove_file(arq);
    }
}
