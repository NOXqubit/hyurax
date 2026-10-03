//! Contas de cliente da API externa (Documento Mestre §14, §17 e fase 5).
//!
//! Uma conta é quem manda JOBs de fora deste computador por
//! `/api/v1/externa/` (ver `api::externa`): um nome, uma chave de acesso e
//! um limite de créditos de computação. O dono do nó cria e revoga as contas
//! (`hyurax-no contas`); nada disso é dinheiro.
//!
//! - **A chave** (32 bytes aleatórios, em hex) aparece uma vez só, na
//!   criação. Em disco fica só o SHA-512 dela; a comparação não vaza, pelo
//!   tempo, quantos bytes bateram.
//! - **Créditos:** cada JOB da conta recebe orçamento igual ao saldo (ou
//!   menos, se pedir menos). O saldo é o limite menos o consumo dos JOBs da
//!   conta, lido do próprio JOB: não há contador separado que possa
//!   divergir. Um JOB para no orçamento; as unidades que já estavam em voo
//!   ainda contam (pode passar um pouco do limite).
//! - **Limites:** pedidos por minuto e JOBs abertos por conta.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use hyurax_crypto::{HASH_LEN, sha512};

use crate::util::{de_hex, hex};

/// Arquivo das contas, na pasta de configuração.
pub const ARQUIVO: &str = "contas.txt";
/// Arquivo de quem é cada JOB, na pasta de configuração.
pub const ARQUIVO_DOS_JOBS: &str = "contas-jobs.txt";
/// Quantas contas o nó aceita.
pub const MAXIMO: usize = 64;
/// Pedidos por minuto, por conta.
pub const PEDIDOS_POR_MINUTO: u32 = 120;
/// JOBs abertos (não finais) por conta.
pub const JOBS_ABERTOS: usize = 8;

/// Uma conta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conta {
    /// Número da conta (1, 2, ...).
    pub id: u32,
    /// Nome dado pelo dono.
    pub nome: String,
    /// SHA-512 da chave de acesso.
    pub hash_da_chave: [u8; HASH_LEN],
    /// Limite de créditos, em milicréditos.
    pub limite_milicreditos: u64,
    /// Quando foi criada (unix).
    pub criada: u64,
    /// Revogada: a chave não vale mais.
    pub revogada: bool,
}

/// As contas e quem é dono de cada JOB.
pub struct Contas {
    pasta: Option<PathBuf>,
    contas: Mutex<Vec<Conta>>,
    jobs: Mutex<BTreeMap<[u8; HASH_LEN], u32>>,
    /// Pedidos no minuto corrente, por conta: (minuto, quantos).
    ritmo: Mutex<BTreeMap<u32, (u64, u32)>>,
    /// Quando o arquivo das contas mudou pela última vez que foi lido: o
    /// terminal (`hyurax-no contas`) mexe no arquivo com o programa aberto, e
    /// uma revogação tem de valer na hora.
    lido_em: Mutex<Option<std::time::SystemTime>>,
}

fn modificado(p: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn nome_valido(nome: &str) -> bool {
    !nome.is_empty() && nome.len() <= 60 && nome.chars().all(|c| c.is_alphanumeric() || " -_.".contains(c))
}

fn mesmo_hash(a: &[u8; HASH_LEN], b: &[u8; HASH_LEN]) -> bool {
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Contas {
    /// Contas gravadas em `pasta` (`None`: só na memória, para testes).
    pub fn abrir(pasta: Option<PathBuf>) -> Self {
        let (contas, jobs) = match &pasta {
            Some(p) => (ler_contas(&std::fs::read_to_string(p.join(ARQUIVO)).unwrap_or_default()), ler_jobs(&std::fs::read_to_string(p.join(ARQUIVO_DOS_JOBS)).unwrap_or_default())),
            None => (Vec::new(), BTreeMap::new()),
        };
        let lido_em = pasta.as_ref().and_then(|p| modificado(&p.join(ARQUIVO)));
        Self { pasta, contas: Mutex::new(contas), jobs: Mutex::new(jobs), ritmo: Mutex::new(BTreeMap::new()), lido_em: Mutex::new(lido_em) }
    }

    /// Relê os arquivos se o das contas mudou desde a última leitura.
    fn recarregar_se_mudou(&self) {
        let Some(p) = &self.pasta else { return };
        let agora = modificado(&p.join(ARQUIVO));
        let Ok(mut lido) = self.lido_em.lock() else { return };
        if *lido == agora {
            return;
        }
        *lido = agora;
        let contas = ler_contas(&std::fs::read_to_string(p.join(ARQUIVO)).unwrap_or_default());
        let jobs = ler_jobs(&std::fs::read_to_string(p.join(ARQUIVO_DOS_JOBS)).unwrap_or_default());
        if let Ok(mut c) = self.contas.lock() {
            *c = contas;
        }
        if let Ok(mut j) = self.jobs.lock() {
            *j = jobs;
        }
    }

    fn gravar(&self) -> Result<(), String> {
        let Some(p) = &self.pasta else { return Ok(()) };
        let contas = self.contas.lock().map_err(|_| "contas travadas")?.clone();
        let mut t = String::from("# Hyurax: contas da API externa. Só o SHA-512 da chave fica aqui.\n# id nome(hex) sha512_da_chave limite_milicreditos criada revogada\n");
        for c in &contas {
            t.push_str(&format!("{} {} {} {} {} {}\n", c.id, hex(c.nome.as_bytes()), hex(&c.hash_da_chave), c.limite_milicreditos, c.criada, u8::from(c.revogada)));
        }
        crate::arquivos::gravar_privado(&p.join(ARQUIVO), &t)?;
        if let Ok(mut lido) = self.lido_em.lock() {
            *lido = modificado(&p.join(ARQUIVO));
        }
        Ok(())
    }

    /// Cria uma conta. Devolve a conta e a chave de acesso (a única vez em que
    /// ela aparece).
    ///
    /// # Errors
    /// Nome inválido, contas demais, sem entropia ou disco.
    pub fn criar(&self, nome: &str, limite_milicreditos: u64, agora: u64) -> Result<(Conta, String), String> {
        let nome = nome.trim();
        if !nome_valido(nome) {
            return Err("nome: de 1 a 60 letras, números, espaço, - _ .".into());
        }
        let chave = hex(&hyurax_net::entropia::entropia_do_sistema()?);
        let conta = {
            let mut contas = self.contas.lock().map_err(|_| "contas travadas")?;
            if contas.iter().filter(|c| !c.revogada).count() >= MAXIMO {
                return Err(format!("no máximo {MAXIMO} contas ativas"));
            }
            let id = contas.iter().map(|c| c.id).max().unwrap_or(0).saturating_add(1);
            let conta = Conta { id, nome: nome.to_string(), hash_da_chave: sha512(chave.as_bytes()), limite_milicreditos, criada: agora, revogada: false };
            contas.push(conta.clone());
            conta
        };
        self.gravar()?;
        Ok((conta, chave))
    }

    /// Revoga uma conta: a chave para de valer na hora. Os JOBs dela ficam.
    ///
    /// # Errors
    /// Conta desconhecida ou disco.
    pub fn revogar(&self, id: u32) -> Result<(), String> {
        {
            let mut contas = self.contas.lock().map_err(|_| "contas travadas")?;
            let c = contas.iter_mut().find(|c| c.id == id).ok_or("conta desconhecida")?;
            c.revogada = true;
        }
        self.gravar()
    }

    /// Muda o limite de créditos.
    ///
    /// # Errors
    /// Conta desconhecida ou disco.
    pub fn mudar_limite(&self, id: u32, limite_milicreditos: u64) -> Result<(), String> {
        {
            let mut contas = self.contas.lock().map_err(|_| "contas travadas")?;
            let c = contas.iter_mut().find(|c| c.id == id).ok_or("conta desconhecida")?;
            c.limite_milicreditos = limite_milicreditos;
        }
        self.gravar()
    }

    /// Todas as contas.
    pub fn listar(&self) -> Vec<Conta> {
        self.recarregar_se_mudou();
        self.contas.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// A conta ativa dona desta chave.
    pub fn autenticar(&self, chave: &str) -> Option<Conta> {
        self.recarregar_se_mudou();
        let h = sha512(chave.trim().as_bytes());
        let contas = self.contas.lock().ok()?;
        // passa por todas, para o tempo não dizer qual conta bateu
        let mut achada = None;
        for c in contas.iter() {
            if mesmo_hash(&c.hash_da_chave, &h) && !c.revogada {
                achada = Some(c.clone());
            }
        }
        achada
    }

    /// Conta um pedido da conta neste minuto. `false`: passou do limite.
    pub fn dentro_do_ritmo(&self, id: u32, agora: u64) -> bool {
        let minuto = agora / 60;
        let Ok(mut r) = self.ritmo.lock() else { return false };
        r.retain(|_, (m, _)| *m == minuto);
        let e = r.entry(id).or_insert((minuto, 0));
        e.1 = e.1.saturating_add(1);
        e.1 <= PEDIDOS_POR_MINUTO
    }

    /// Registra que o JOB é da conta (só acréscimo, em disco).
    ///
    /// # Errors
    /// Disco.
    pub fn registrar_job(&self, id: u32, job: &[u8; HASH_LEN]) -> Result<(), String> {
        if let Ok(mut j) = self.jobs.lock() {
            j.insert(*job, id);
        }
        let Some(p) = &self.pasta else { return Ok(()) };
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p.join(ARQUIVO_DOS_JOBS))
            .map_err(|e| format!("não consegui abrir {ARQUIVO_DOS_JOBS}: {e}"))?;
        writeln!(f, "{id} {}", hex(job)).and_then(|()| f.sync_all()).map_err(|e| format!("não consegui gravar {ARQUIVO_DOS_JOBS}: {e}"))
    }

    /// Os JOBs da conta.
    pub fn jobs_da_conta(&self, id: u32) -> Vec<[u8; HASH_LEN]> {
        self.jobs.lock().map(|j| j.iter().filter(|(_, c)| **c == id).map(|(job, _)| *job).collect()).unwrap_or_default()
    }

    /// O JOB é desta conta?
    pub fn e_da_conta(&self, id: u32, job: &[u8; HASH_LEN]) -> bool {
        self.jobs.lock().is_ok_and(|j| j.get(job) == Some(&id))
    }
}

fn ler_contas(texto: &str) -> Vec<Conta> {
    texto
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let mut c = l.split_whitespace();
            let id = c.next()?.parse().ok()?;
            let nome_hex = c.next()?;
            let bytes: Vec<u8> = (0..nome_hex.len() / 2).filter_map(|i| nome_hex.get(i * 2..i * 2 + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
            let nome = String::from_utf8(bytes).ok()?;
            Some(Conta {
                id,
                nome,
                hash_da_chave: de_hex(c.next()?)?,
                limite_milicreditos: c.next()?.parse().ok()?,
                criada: c.next()?.parse().ok()?,
                revogada: c.next()? == "1",
            })
        })
        .take(MAXIMO.saturating_mul(4))
        .collect()
}

fn ler_jobs(texto: &str) -> BTreeMap<[u8; HASH_LEN], u32> {
    texto
        .lines()
        .filter_map(|l| {
            let (id, job) = l.split_once(' ')?;
            Some((de_hex(job.trim())?, id.parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn chave_vale_ate_revogar_e_so_o_hash_fica_no_disco() {
        let pasta = std::env::temp_dir().join(format!("hyurax-contas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        let contas = Contas::abrir(Some(pasta.clone()));
        let (conta, chave) = contas.criar("Laboratório X", 5_000, 10).unwrap();
        assert_eq!(contas.autenticar(&chave).map(|c| c.id), Some(conta.id));
        assert!(contas.autenticar("0".repeat(64).as_str()).is_none());
        let disco = std::fs::read_to_string(pasta.join(ARQUIVO)).unwrap();
        assert!(!disco.contains(&chave), "a chave não vai para o disco");
        // reabre: a mesma conta, com acento no nome
        let de_novo = Contas::abrir(Some(pasta.clone()));
        assert_eq!(de_novo.listar(), contas.listar());
        assert_eq!(de_novo.listar().first().map(|c| c.nome.clone()).as_deref(), Some("Laboratório X"));
        de_novo.registrar_job(conta.id, &[3; 64]).unwrap();
        de_novo.revogar(conta.id).unwrap();
        let terceira = Contas::abrir(Some(pasta.clone()));
        assert!(terceira.autenticar(&chave).is_none(), "revogada não vale mais");
        // o programa aberto vê a revogação feita pelo terminal (outra instância)
        let (outra, chave2) = terceira.criar("Outra", 1, 11).unwrap();
        let aberto = Contas::abrir(Some(pasta.clone()));
        assert!(aberto.autenticar(&chave2).is_some());
        std::thread::sleep(std::time::Duration::from_millis(20));
        terceira.revogar(outra.id).unwrap();
        assert!(aberto.autenticar(&chave2).is_none(), "revogação vale na hora no programa aberto");
        assert!(terceira.e_da_conta(conta.id, &[3; 64]));
        assert!(!terceira.e_da_conta(conta.id + 1, &[3; 64]));
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn ritmo_e_nome() {
        let contas = Contas::abrir(None);
        for _ in 0..PEDIDOS_POR_MINUTO {
            assert!(contas.dentro_do_ritmo(1, 600));
        }
        assert!(!contas.dentro_do_ritmo(1, 610), "passou do limite no mesmo minuto");
        assert!(contas.dentro_do_ritmo(2, 610), "outra conta não é afetada");
        assert!(contas.dentro_do_ritmo(1, 660), "minuto novo");
        assert!(contas.criar("", 1, 1).is_err());
        assert!(contas.criar("a;rm -rf", 1, 1).is_err());
    }
}
