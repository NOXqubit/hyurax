//! O livro de contas da nuvem, em `nuvem/livro.txt` (pasta de dados).
//!
//! Só acréscimo, uma linha por lançamento, cada uma com o hash da anterior
//! ([`hyurax_nuvem::livro`]): apagar, trocar ou reordenar uma linha quebra a
//! conferência de todas as seguintes, e a tela mostra onde quebrou.
//!
//! O que entra:
//! - **consumo** de cada JOB, lido do próprio JOB (o contador não diverge);
//! - a **partilha** de um consumo de cliente (conta da API externa) ou de
//!   aluguel: provedor, plataforma e reserva, nos percentuais do dono;
//! - **recibos** de aluguel recebidos (o que outro nó reconhece dever);
//! - **armazenamento** pago a quem guarda fragmentos (pela prova de guarda).
//!
//! Créditos de computação, não dinheiro: nada aqui paga ninguém.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use hyurax_crypto::HASH_LEN;
use hyurax_nuvem::livro::{Lancamento, repartir};

use crate::util::{de_hex, hex};

/// Um lançamento lido do disco, com o próprio hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linha {
    /// O lançamento.
    pub l: Lancamento,
    /// O hash encadeado.
    pub hash: [u8; HASH_LEN],
}

/// O livro.
pub struct Livro {
    arquivo: Option<PathBuf>,
    estado: Mutex<Estado>,
}

#[derive(Default)]
struct Estado {
    linhas: Vec<Linha>,
    /// Onde a conferência quebrou (seq), se quebrou.
    quebra: Option<u64>,
    /// Consumo já lançado de cada JOB.
    lancado: BTreeMap<[u8; HASH_LEN], u64>,
}

/// Um JOB a liquidar: (JOB, conta, milicréditos consumidos, fornecedor do aluguel).
pub type JobALiquidar = ([u8; HASH_LEN], u32, u64, Option<[u8; 32]>);

/// Teto de linhas guardadas na memória (o arquivo guarda todas).
const MEMORIA_MAX: usize = 20_000;

fn ler_linha(texto: &str) -> Option<Linha> {
    let mut c = texto.split(' ');
    let seq = c.next()?.parse().ok()?;
    let instante = c.next()?.parse().ok()?;
    let tipo = c.next()?.to_string();
    let conta = c.next()?.parse().ok()?;
    let job = de_hex(c.next()?)?;
    let contraparte = de_hex(c.next()?)?;
    let valor_mili = c.next()?.parse().ok()?;
    let nota_hex = c.next()?;
    let bytes: Vec<u8> = (0..nota_hex.len() / 2).filter_map(|i| nota_hex.get(i * 2..i * 2 + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
    let nota = String::from_utf8(bytes).ok()?;
    let hash = de_hex(c.next()?)?;
    Some(Linha { l: Lancamento { seq, instante, tipo, conta, job, contraparte, valor_mili, nota }, hash })
}

impl Livro {
    /// Abre o livro (`None`: só na memória, para testes) e confere o
    /// encadeamento inteiro.
    pub fn abrir(arquivo: Option<PathBuf>) -> Self {
        let mut e = Estado::default();
        if let Some(a) = &arquivo {
            let texto = std::fs::read_to_string(a).unwrap_or_default();
            let mut anterior = [0u8; HASH_LEN];
            for t in texto.lines().filter(|t| !t.starts_with('#') && !t.trim().is_empty()) {
                let Some(linha) = ler_linha(t) else {
                    e.quebra.get_or_insert(e.linhas.last().map_or(1, |l| l.l.seq.saturating_add(1)));
                    continue;
                };
                if e.quebra.is_none() && linha.l.hash(&anterior).ok() != Some(linha.hash) {
                    e.quebra = Some(linha.l.seq);
                }
                anterior = linha.hash;
                if linha.l.tipo == "consumo" && linha.l.job != [0; HASH_LEN] {
                    let v = e.lancado.entry(linha.l.job).or_insert(0);
                    *v = v.saturating_add(linha.l.valor_mili);
                }
                e.linhas.push(linha);
                if e.linhas.len() > MEMORIA_MAX {
                    e.linhas.remove(0);
                }
            }
        }
        Self { arquivo, estado: Mutex::new(e) }
    }

    /// Acrescenta um lançamento. Devolve o hash.
    ///
    /// # Errors
    /// Tipo desconhecido, nota longa ou disco.
    #[allow(clippy::too_many_arguments)]
    pub fn lancar(&self, instante: u64, tipo: &str, conta: u32, job: [u8; HASH_LEN], contraparte: [u8; 32], valor_mili: u64, nota: &str) -> Result<[u8; HASH_LEN], String> {
        let mut e = self.estado.lock().map_err(|_| "livro travado".to_string())?;
        let (seq, anterior) = e.linhas.last().map_or((1, [0u8; HASH_LEN]), |l| (l.l.seq.saturating_add(1), l.hash));
        let l = Lancamento { seq, instante, tipo: tipo.to_string(), conta, job, contraparte, valor_mili, nota: nota.to_string() };
        let hash = l.hash(&anterior).map_err(|e| e.to_string())?;
        if let Some(a) = &self.arquivo {
            if let Some(p) = a.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("não consegui criar {}: {e}", p.display()))?;
            }
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(a).map_err(|e| format!("não consegui abrir o livro: {e}"))?;
            writeln!(
                f,
                "{} {} {} {} {} {} {} {} {}",
                l.seq,
                l.instante,
                l.tipo,
                l.conta,
                hex(&l.job),
                hex(&l.contraparte),
                l.valor_mili,
                hex(l.nota.as_bytes()),
                hex(&hash)
            )
            .and_then(|()| f.sync_all())
            .map_err(|e| format!("não consegui gravar o livro: {e}"))?;
        }
        if l.tipo == "consumo" && l.job != [0; HASH_LEN] {
            let v = e.lancado.entry(l.job).or_insert(0);
            *v = v.saturating_add(l.valor_mili);
        }
        e.linhas.push(Linha { l, hash });
        if e.linhas.len() > MEMORIA_MAX {
            e.linhas.remove(0);
        }
        Ok(hash)
    }

    /// Lança o consumo novo de cada JOB (o que passou do já lançado) e, para
    /// conta de cliente ou aluguel, a partilha. `jobs`: (JOB, conta,
    /// milicréditos consumidos até agora, fornecedor do aluguel).
    ///
    /// # Errors
    /// Disco.
    pub fn liquidar(&self, instante: u64, jobs: &[JobALiquidar], provedor_pct: u8, plataforma_pct: u8) -> Result<usize, String> {
        let mut feitos = 0usize;
        for (job, conta, consumo, fornecedor) in jobs {
            let ja = self.estado.lock().map(|e| e.lancado.get(job).copied().unwrap_or(0)).unwrap_or(u64::MAX);
            let delta = consumo.saturating_sub(ja);
            if delta == 0 {
                continue;
            }
            let contraparte = fornecedor.unwrap_or([0; 32]);
            let nota = match (conta, fornecedor) {
                (_, Some(_)) => "aluguel de capacidade",
                (0, None) => "uso próprio",
                _ => "conta de cliente",
            };
            self.lancar(instante, "consumo", *conta, *job, contraparte, delta, nota)?;
            if *conta != 0 || fornecedor.is_some() {
                let (p, pl, r) = repartir(delta, provedor_pct, plataforma_pct).map_err(|e| e.to_string())?;
                self.lancar(instante, "provedor", *conta, *job, contraparte, p, "")?;
                self.lancar(instante, "plataforma", *conta, *job, [0; 32], pl, "")?;
                self.lancar(instante, "reserva", *conta, *job, [0; 32], r, "")?;
            }
            feitos = feitos.saturating_add(1);
        }
        Ok(feitos)
    }

    /// As últimas `n` linhas, da mais nova para a mais velha.
    pub fn ultimas(&self, n: usize) -> Vec<Linha> {
        self.estado.lock().map(|e| e.linhas.iter().rev().take(n).cloned().collect()).unwrap_or_default()
    }

    /// Onde a conferência quebrou, se quebrou.
    pub fn quebra(&self) -> Option<u64> {
        self.estado.lock().ok().and_then(|e| e.quebra)
    }

    /// Totais por tipo.
    pub fn totais(&self) -> BTreeMap<String, u64> {
        let mut t = BTreeMap::new();
        if let Ok(e) = self.estado.lock() {
            for l in &e.linhas {
                let v = t.entry(l.l.tipo.clone()).or_insert(0u64);
                *v = v.saturating_add(l.l.valor_mili);
            }
        }
        t
    }

    /// Faturas: consumo por conta e por mês (`AAAA-MM`, UTC).
    pub fn faturas(&self) -> Vec<(u32, String, u64, usize)> {
        let mut f: BTreeMap<(u32, String), (u64, usize)> = BTreeMap::new();
        if let Ok(e) = self.estado.lock() {
            for l in e.linhas.iter().filter(|l| l.l.tipo == "consumo") {
                let v = f.entry((l.l.conta, mes(l.l.instante))).or_insert((0, 0));
                v.0 = v.0.saturating_add(l.l.valor_mili);
                v.1 = v.1.saturating_add(1);
            }
        }
        f.into_iter().rev().map(|((c, m), (v, n))| (c, m, v, n)).collect()
    }

    /// O livro em JSON para a tela.
    pub fn json(&self, n: usize, provedor_pct: u8, plataforma_pct: u8) -> serde_json::Value {
        let linhas: Vec<serde_json::Value> = self
            .ultimas(n)
            .iter()
            .map(|l| {
                serde_json::json!({
                    "seq": l.l.seq, "instante": l.l.instante, "tipo": l.l.tipo, "conta": l.l.conta,
                    "job": if l.l.job == [0; HASH_LEN] { String::new() } else { hex(&l.l.job) },
                    "contraparte": if l.l.contraparte == [0; 32] { String::new() } else { hex(&l.l.contraparte) },
                    "valor_mili": l.l.valor_mili, "nota": l.l.nota, "hash": hex(l.hash.get(..16).unwrap_or_default()),
                })
            })
            .collect();
        let faturas: Vec<serde_json::Value> =
            self.faturas().into_iter().map(|(c, m, v, q)| serde_json::json!({ "conta": c, "mes": m, "consumo_mili": v, "lancamentos": q })).collect();
        serde_json::json!({
            "integro": self.quebra().is_none(),
            "quebra_em": self.quebra(),
            "tarifas": { "provedor_pct": provedor_pct, "plataforma_pct": plataforma_pct, "reserva_pct": 100u8.saturating_sub(provedor_pct).saturating_sub(plataforma_pct) },
            "totais": self.totais(),
            "faturas": faturas,
            "linhas": linhas,
        })
    }
}

/// `AAAA-MM` de um instante Unix (UTC), pelo algoritmo de dias civis.
pub fn mes(instante: u64) -> String {
    let dias = i64::try_from(instante / 86_400).unwrap_or(0);
    let z = dias.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn encadeia_reabre_e_pega_adulteracao() {
        let pasta = std::env::temp_dir().join(format!("hyurax-livro-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        let arquivo = pasta.join("livro.txt");
        let livro = Livro::abrir(Some(arquivo.clone()));
        let job = [7u8; HASH_LEN];
        assert_eq!(livro.liquidar(1_790_000_000, &[(job, 1, 3_000, None)], 80, 15).unwrap(), 1);
        // nada novo: não lança de novo
        assert_eq!(livro.liquidar(1_790_000_001, &[(job, 1, 3_000, None)], 80, 15).unwrap(), 0);
        assert_eq!(livro.liquidar(1_790_000_002, &[(job, 1, 3_500, None)], 80, 15).unwrap(), 1);
        let t = livro.totais();
        assert_eq!(t.get("consumo"), Some(&3_500));
        assert_eq!(t.get("provedor").copied().unwrap() + t.get("plataforma").copied().unwrap() + t.get("reserva").copied().unwrap(), 3_500);
        // reabre: igual, íntegro, e o já lançado continua valendo
        let de_novo = Livro::abrir(Some(arquivo.clone()));
        assert!(de_novo.quebra().is_none());
        assert_eq!(de_novo.liquidar(1_790_000_003, &[(job, 1, 3_500, None)], 80, 15).unwrap(), 0);
        // uso próprio: só o consumo, sem partilha
        de_novo.liquidar(1_790_000_004, &[([8; HASH_LEN], 0, 100, None)], 80, 15).unwrap();
        assert_eq!(de_novo.totais().get("consumo"), Some(&3_600));
        // adultera um valor no meio: a conferência quebra ali
        let texto = std::fs::read_to_string(&arquivo).unwrap();
        let trocado = texto.replacen(" 3000 ", " 9000 ", 1);
        assert_ne!(texto, trocado);
        std::fs::write(&arquivo, trocado).unwrap();
        assert_eq!(Livro::abrir(Some(arquivo)).quebra(), Some(1));
        assert_eq!(de_novo.faturas().first().map(|f| f.1.clone()).as_deref(), Some("2026-09"));
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn mes_civil() {
        assert_eq!(mes(0), "1970-01");
        assert_eq!(mes(1_791_072_000), "2026-10");
        assert_eq!(mes(951_782_400), "2000-02");
    }
}
