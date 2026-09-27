//! A mineração dos blocos da cadeia.
//!
//! Cada rodada monta um candidato em cima da ponta atual e procura o nonce
//! pelo Argon2id (`hyurax-pow`). O candidato já traz a prova de trabalho útil
//! do bloco (`hyurax-usefulpow`: a multiplicação de matrizes que todo nó
//! confere). A rodada acaba quando acha, quando chega bloco de outro nó ou
//! quando mandam parar.
//!
//! O limite de CPU é uma pausa entre tentativas, calculada pelo tempo medido
//! de uma tentativa nesta máquina: com 50%, a linha trabalha e descansa o
//! mesmo tempo. O limite é um **ajuste**, não uma medida: o uso real de CPU
//! vem de [`crate::metricas`].

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyurax_block::Block;
use hyurax_consensus::block_reward;
use hyurax_crypto::ADDRESS_LEN;
use hyurax_net::Rede;
use hyurax_pow::ConfigMineracao;

use crate::barramento::Barramento;
use crate::cadeia;
use crate::config::ConfigDoNo;
use crate::util::{agora_unix, hex, hyx};

/// Quantas amostras de ritmo (uma por segundo) ficam guardadas.
const AMOSTRAS_MAX: usize = 120;

/// O estado da mineração.
pub struct Mineracao {
    /// Minerar agora (o dono ligou e há carteira).
    pub ligada: AtomicBool,
    /// Linhas (núcleos) procurando o nonce.
    pub linhas: AtomicU32,
    /// Limite de CPU por linha, de 10 a 100 por cento. É ajuste, não medida.
    pub limite_cpu: AtomicU32,
    /// Tentativas somadas desde que o programa abriu.
    pub tentativas: AtomicU64,
    /// Blocos achados por este nó desde que o programa abriu.
    pub meus: AtomicU64,
    /// Blocos achados que perderam a corrida para outro nó.
    pub perdidos: AtomicU64,
    /// Núcleos lógicos da máquina.
    pub nucleos: u32,
    interromper: AtomicBool,
    rodada: Mutex<Option<(Instant, u64)>>,
    ms_tentativa: Mutex<f64>,
    amostras: Mutex<VecDeque<(f64, u64)>>,
    inicio: Instant,
}

impl Mineracao {
    /// Mineração parada, com `linhas` e `limite_cpu`.
    pub fn nova(nucleos: u32, linhas: u32, limite_cpu: u32) -> Arc<Self> {
        Arc::new(Self {
            ligada: AtomicBool::new(false),
            linhas: AtomicU32::new(linhas.clamp(1, nucleos.max(1))),
            limite_cpu: AtomicU32::new(limite_cpu.clamp(10, 100)),
            tentativas: AtomicU64::new(0),
            meus: AtomicU64::new(0),
            perdidos: AtomicU64::new(0),
            nucleos: nucleos.max(1),
            interromper: AtomicBool::new(false),
            rodada: Mutex::new(None),
            ms_tentativa: Mutex::new(0.0),
            amostras: Mutex::new(VecDeque::new()),
            inicio: Instant::now(),
        })
    }

    /// Faz a rodada atual parar (limite mudou, bloco novo chegou, desligou).
    pub fn interromper(&self) {
        self.interromper.store(true, Ordering::Relaxed);
    }

    /// Muda linhas e limite; vale na próxima rodada, que começa já.
    pub fn ajustar(&self, linhas: Option<u32>, limite_cpu: Option<u32>) {
        if let Some(n) = linhas {
            self.linhas.store(n.clamp(1, self.nucleos), Ordering::Relaxed);
        }
        if let Some(u) = limite_cpu {
            self.limite_cpu.store(u.clamp(10, 100), Ordering::Relaxed);
        }
        self.interromper();
    }

    /// Guarda uma amostra (tempo, tentativas); chamar uma vez por segundo.
    pub fn amostrar(&self) {
        let t = self.inicio.elapsed().as_secs_f64();
        if let Ok(mut a) = self.amostras.lock() {
            a.push_back((t, self.tentativas.load(Ordering::Relaxed)));
            while a.len() > AMOSTRAS_MAX {
                a.pop_front();
            }
        }
    }

    /// As amostras guardadas: (segundos desde que abriu, tentativas somadas).
    pub fn amostras(&self) -> Vec<(f64, u64)> {
        self.amostras.lock().map(|a| a.iter().copied().collect()).unwrap_or_default()
    }

    /// Tentativas por segundo nos últimos ~10 s (medido).
    pub fn ritmo(&self) -> f64 {
        let Ok(a) = self.amostras.lock() else { return 0.0 };
        let (Some(fim), Some(comeco)) = (a.back(), a.iter().rev().nth(10).or(a.front())) else {
            return 0.0;
        };
        let dt = fim.0 - comeco.0;
        if dt <= 0.0 { 0.0 } else { fim.1.saturating_sub(comeco.1) as f64 / dt }
    }

    /// Há quanto tempo a rodada atual começou, e em que altura.
    pub fn rodada(&self) -> Option<(f64, u64)> {
        self.rodada.lock().ok().and_then(|r| *r).map(|(t, a)| (t.elapsed().as_secs_f64(), a))
    }

    /// Tempo medido de uma tentativa, em milissegundos (0 se ainda não mediu).
    pub fn ms_tentativa(&self) -> f64 {
        self.ms_tentativa.lock().map(|m| *m).unwrap_or(0.0)
    }

    /// A pausa entre tentativas que faz a linha ocupar só o limite escolhido.
    pub fn pausa_do_limite(&self) -> Duration {
        let uso = f64::from(self.limite_cpu.load(Ordering::Relaxed).clamp(10, 100));
        if uso >= 100.0 {
            return Duration::ZERO;
        }
        let ms = self.ms_tentativa();
        if ms <= 0.0 {
            // ainda não mediu: começa com uma pausa modesta e corrige depois
            return Duration::from_millis(u64::from(100 - uso.min(99.0) as u32));
        }
        Duration::from_millis((ms * (100.0 / uso - 1.0)).clamp(0.0, 5_000.0) as u64)
    }

    fn medir_tentativa(&self, tentativas: u64, duracao: Duration, linhas: u32) {
        if tentativas == 0 {
            return;
        }
        let ms = duracao.as_secs_f64() * 1000.0 * f64::from(linhas.max(1)) / tentativas as f64;
        // desconta a pausa imposta: interessa o tempo puro de cálculo
        let ms = (ms - self.pausa_do_limite().as_secs_f64() * 1000.0).max(1.0);
        if let Ok(mut m) = self.ms_tentativa.lock() {
            *m = if *m <= 0.0 { ms } else { *m * 0.7 + ms * 0.3 };
        }
    }

    /// O laço da mineração: roda para sempre numa linha própria. Minera
    /// enquanto [`Self::ligada`] e houver endereço de recompensa.
    pub fn laco(
        self: Arc<Self>,
        rede: Arc<Rede>,
        config: ConfigDoNo,
        endereco: Arc<dyn Fn() -> Option<[u8; ADDRESS_LEN]> + Send + Sync>,
        barramento: Arc<Barramento>,
        pausa_fixa_ms: u64,
    ) {
        let mut estava = false;
        loop {
            let alvo = endereco();
            let Some(alvo) = alvo.filter(|_| self.ligada.load(Ordering::Relaxed)) else {
                if estava {
                    barramento.registrar("mineracao", "mineração parada");
                    estava = false;
                }
                if let Ok(mut r) = self.rodada.lock() {
                    *r = None;
                }
                std::thread::sleep(Duration::from_millis(250));
                continue;
            };
            if !estava {
                let linhas = self.linhas.load(Ordering::Relaxed);
                barramento.registrar(
                    "mineracao",
                    format!(
                        "mineração ligada: {linhas} linha(s), {:.0} MiB de memória, limite de {}% da CPU por linha",
                        (u64::from(config.rede.pow.memoria_kib) * u64::from(linhas)) as f64 / 1024.0,
                        self.limite_cpu.load(Ordering::Relaxed)
                    ),
                );
                estava = true;
            }
            if let Err(e) = self.uma_rodada(&rede, &config, alvo, &barramento, pausa_fixa_ms) {
                barramento.registrar("erro", e);
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    }

    fn uma_rodada(
        &self,
        rede: &Arc<Rede>,
        config: &ConfigDoNo,
        endereco: [u8; ADDRESS_LEN],
        barramento: &Barramento,
        pausa_fixa_ms: u64,
    ) -> Result<(), String> {
        let candidato = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            no.chain
                .build_candidate(endereco, no.mempool_ordenado(), None, Vec::new())
                .map_err(|e| e.to_string())?
        };
        let altura = candidato.header.height;
        let inicio = Instant::now();
        if let Ok(mut r) = self.rodada.lock() {
            *r = Some((inicio, altura));
        }
        self.interromper.store(false, Ordering::Relaxed);
        let linhas = self.linhas.load(Ordering::Relaxed).max(1);
        let pausa = if pausa_fixa_ms > 0 { Duration::from_millis(pausa_fixa_ms) } else { self.pausa_do_limite() };
        let cfg = ConfigMineracao { linhas, nonce_inicial: agora_unix().wrapping_mul(0x9E37_79B9), limite: None, pausa };
        let antes = self.tentativas.load(Ordering::Relaxed);
        // um vigia liga a interrupção quando mandam parar
        let fim = AtomicBool::new(false);
        let resultado = std::thread::scope(|s| {
            s.spawn(|| {
                while !fim.load(Ordering::Relaxed) {
                    if !self.ligada.load(Ordering::Relaxed) {
                        self.interromper.store(true, Ordering::Relaxed);
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
            let r = hyurax_pow::minerar(&candidato.header.encode(), config.rede.pow, cfg, &self.interromper, &self.tentativas);
            fim.store(true, Ordering::Relaxed);
            r
        })
        .map_err(|e| e.to_string())?;
        self.medir_tentativa(self.tentativas.load(Ordering::Relaxed).saturating_sub(antes), inicio.elapsed(), linhas);
        let Some(achado) = resultado.achado else {
            return Ok(()); // interrompida: bloco novo chegou ou mandaram parar
        };
        let n = candidato.useful_proof.as_ref().map_or(0, |p| p.n);
        let bloco = Block { header: candidato.header.with_nonce(achado.nonce), ..candidato };
        let hash = hex(&bloco.block_hash());
        match rede.submeter_bloco(bloco) {
            Ok(true) => {
                cadeia::salvar(rede, config)?;
                self.meus.fetch_add(1, Ordering::Relaxed);
                let recompensa = hyx(u128::from(block_reward(altura, &config.rede)));
                let segundos = inicio.elapsed().as_secs_f64();
                barramento.registrar(
                    "bloco",
                    format!("bloco {altura} minerado por este nó em {segundos:.1} s · prova útil {n}×{n} · +{recompensa} HYX"),
                );
                barramento.publicar(
                    "bloco",
                    serde_json::json!({ "altura": altura, "hash": hash, "meu": true, "n": n, "segundos": segundos, "recompensa": recompensa })
                        .to_string(),
                );
            }
            Ok(false) | Err(_) => {
                self.perdidos.fetch_add(1, Ordering::Relaxed);
                barramento.registrar("mineracao", format!("bloco {altura} perdido na corrida: outro nó chegou primeiro"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limite_de_100_nao_pausa_e_menor_pausa_na_proporcao() {
        let m = Mineracao::nova(4, 1, 100);
        assert_eq!(m.pausa_do_limite(), Duration::ZERO);
        m.ajustar(None, Some(50));
        if let Ok(mut t) = m.ms_tentativa.lock() {
            *t = 40.0;
        }
        assert_eq!(m.pausa_do_limite(), Duration::from_millis(40), "com 50%, descansa o mesmo tempo que trabalha");
        m.ajustar(Some(99), Some(5));
        assert_eq!(m.linhas.load(Ordering::Relaxed), 4, "no máximo os núcleos da máquina");
        assert_eq!(m.limite_cpu.load(Ordering::Relaxed), 10, "o limite mínimo é 10%");
    }
}
