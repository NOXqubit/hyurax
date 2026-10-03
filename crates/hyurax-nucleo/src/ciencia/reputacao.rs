//! A reputação dos workers, gravada em disco, e a classificação que sai dela:
//! Gold Score e nó verificado (Documento Mestre §11).
//!
//! O que entra aqui é só o que **este** nó conferiu: cada entrega verificada
//! ou recusada passou pela conferência local (ver `rede.rs`). Ninguém declara
//! a própria reputação, e nada vem de outro nó.
//!
//! ```text
//! nota       = (verificadas + 1) / (verificadas + recusadas + 2), em milésimos
//! volume     = min(verificadas, 200) / 200
//! Gold Score = nota × volume   (0 a 1000)
//! ```
//!
//! **Nó verificado** exige as quatro coisas, todas medidas aqui:
//! - visto pela primeira vez há pelo menos 90 dias (a referência do documento);
//! - pelo menos 100 unidades verificadas por este nó;
//! - no máximo 1% de recusas com evidência entre as julgadas;
//! - alguma entrega nos últimos 30 dias.
//!
//! Tempo de cadastro sozinho não vale nada: cada unidade verificada custou
//! cálculo de verdade, refeito aqui. Isso encarece identidades falsas, mas
//! não as impede (§12: nenhum mecanismo isolado resolve Sybil), e a visão é
//! local: outro nó pode ter outra opinião sobre o mesmo worker.

use std::collections::BTreeMap;
use std::path::Path;

use hyurax_crypto::PUBKEY_LEN;
use hyurax_ultrax::reputacao::Reputacao;

use crate::util::{de_hex, hex};

/// Arquivo da reputação, na pasta da ciência.
pub const ARQUIVO: &str = "reputacao.txt";
/// Quantos workers ficam guardados. Passou disso, sai quem tem menos entregas.
pub const MAXIMO: usize = 4096;
/// Dias de observação para um nó verificado.
pub const DIAS_PARA_VERIFICADO: u64 = 90;
/// Unidades verificadas para um nó verificado.
pub const VERIFICADAS_PARA_VERIFICADO: u64 = 100;
/// Recusas toleradas, em milésimos das julgadas.
pub const RECUSAS_TOLERADAS_MILESIMOS: u64 = 10;
/// Dias sem entrega até deixar de ser verificado.
pub const DIAS_DE_ATIVIDADE: u64 = 30;
/// Unidades verificadas para o volume chegar a 1.
pub const VOLUME_CHEIO: u64 = 200;

const DIA_MS: u64 = 86_400_000;

/// O que este nó viu de um worker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Historico {
    /// Os números.
    pub rep: Reputacao,
    /// Primeira vez que este nó julgou uma entrega dele (ms desde 1970).
    pub desde_ms: u64,
    /// Última entrega julgada.
    pub ultimo_ms: u64,
}

/// A classificação de um worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classificacao {
    /// 0 a 1000.
    pub gold: u64,
    /// Nó verificado pelas quatro regras.
    pub verificado: bool,
    /// O que falta, em texto, quando não é verificado.
    pub falta: Vec<String>,
}

/// Gold Score: nota × volume (ver o topo do módulo).
pub fn gold(rep: &Reputacao) -> u64 {
    let volume = rep.verificadas.min(VOLUME_CHEIO);
    rep.nota().saturating_mul(volume) / VOLUME_CHEIO
}

/// Classifica um worker no instante `agora`.
pub fn classificar(h: &Historico, agora: u64) -> Classificacao {
    let mut falta = Vec::new();
    let dias = agora.saturating_sub(h.desde_ms) / DIA_MS;
    if h.desde_ms == 0 || dias < DIAS_PARA_VERIFICADO {
        falta.push(format!("{dias} de {DIAS_PARA_VERIFICADO} dias de observação"));
    }
    if h.rep.verificadas < VERIFICADAS_PARA_VERIFICADO {
        falta.push(format!("{} de {VERIFICADAS_PARA_VERIFICADO} unidades verificadas", h.rep.verificadas));
    }
    let julgadas = h.rep.verificadas.saturating_add(h.rep.recusadas);
    let recusas = h.rep.recusadas.saturating_mul(1000).checked_div(julgadas).unwrap_or(0);
    if recusas > RECUSAS_TOLERADAS_MILESIMOS {
        falta.push(format!("recusas em {},{}% das julgadas (máximo 1%)", recusas / 10, recusas % 10));
    }
    if agora.saturating_sub(h.ultimo_ms) > DIAS_DE_ATIVIDADE.saturating_mul(DIA_MS) {
        falta.push(format!("sem entrega nos últimos {DIAS_DE_ATIVIDADE} dias"));
    }
    Classificacao { gold: gold(&h.rep), verificado: falta.is_empty(), falta }
}

/// Lê o arquivo. Linha inválida é pulada (o arquivo é só deste programa).
pub fn ler(pasta: &Path) -> BTreeMap<[u8; PUBKEY_LEN], Historico> {
    let Ok(texto) = std::fs::read_to_string(pasta.join(ARQUIVO)) else { return BTreeMap::new() };
    texto
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let mut c = l.split_whitespace();
            let worker = de_hex::<PUBKEY_LEN>(c.next()?)?;
            let mut n = || c.next().and_then(|x| x.parse::<u64>().ok());
            let rep = Reputacao {
                enviadas: n()?,
                verificadas: n()?,
                recusadas: n()?,
                divergentes: n()?,
                disputadas: n()?,
                ..Reputacao::default()
            };
            Some((worker, Historico { rep, desde_ms: n()?, ultimo_ms: n()? }))
        })
        .take(MAXIMO)
        .collect()
}

/// Grava o arquivo inteiro (troca atômica).
///
/// # Errors
/// Disco.
pub fn gravar(pasta: &Path, todos: &BTreeMap<[u8; PUBKEY_LEN], Historico>) -> Result<(), String> {
    let mut t = String::from(
        "# Hyurax: reputação dos workers vista por este nó (só o que foi conferido aqui).\n\
         # worker enviadas verificadas recusadas divergentes disputadas desde_ms ultimo_ms\n",
    );
    for (w, h) in todos {
        t.push_str(&format!(
            "{} {} {} {} {} {} {} {}\n",
            hex(w),
            h.rep.enviadas,
            h.rep.verificadas,
            h.rep.recusadas,
            h.rep.divergentes,
            h.rep.disputadas,
            h.desde_ms,
            h.ultimo_ms
        ));
    }
    crate::arquivos::gravar_atomico(&pasta.join(ARQUIVO), t.as_bytes())
}

/// Abre espaço para um worker novo: sai quem tem menos entregas (e, no
/// empate, quem foi visto há mais tempo).
pub fn abrir_espaco(todos: &mut BTreeMap<[u8; PUBKEY_LEN], Historico>, novo: &[u8; PUBKEY_LEN]) {
    if todos.len() < MAXIMO || todos.contains_key(novo) {
        return;
    }
    if let Some(sai) = todos.iter().min_by_key(|(_, h)| (h.rep.enviadas, h.ultimo_ms)).map(|(w, _)| *w) {
        todos.remove(&sai);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn com(verificadas: u64, recusadas: u64, desde_dias: u64, ultimo_dias: u64, agora: u64) -> Historico {
        Historico {
            rep: Reputacao { enviadas: verificadas + recusadas, verificadas, recusadas, ..Reputacao::default() },
            desde_ms: agora - desde_dias * DIA_MS,
            ultimo_ms: agora - ultimo_dias * DIA_MS,
        }
    }

    #[test]
    fn verificado_exige_as_quatro_regras() {
        let agora = 1_800_000_000_000;
        assert!(classificar(&com(150, 1, 100, 2, agora), agora).verificado);
        // tempo sozinho não basta
        let pouco = classificar(&com(10, 0, 400, 1, agora), agora);
        assert!(!pouco.verificado && pouco.falta.iter().any(|f| f.contains("unidades verificadas")));
        // volume sem tempo de observação também não
        assert!(!classificar(&com(500, 0, 30, 1, agora), agora).verificado);
        // recusas acima de 1%
        let ruim = classificar(&com(150, 5, 100, 1, agora), agora);
        assert!(!ruim.verificado && ruim.falta.iter().any(|f| f.contains("recusas")));
        // parado há mais de 30 dias
        assert!(!classificar(&com(150, 0, 200, 45, agora), agora).verificado);
    }

    #[test]
    fn gold_cresce_com_volume_e_cai_com_recusas() {
        let novo = Reputacao::default();
        assert_eq!(gold(&novo), 0, "sem histórico, sem Gold Score");
        let pouco = Reputacao { verificadas: 20, ..Reputacao::default() };
        let muito = Reputacao { verificadas: 400, ..Reputacao::default() };
        let ruim = Reputacao { verificadas: 400, recusadas: 100, ..Reputacao::default() };
        assert!(gold(&pouco) < gold(&muito));
        assert!(gold(&ruim) < gold(&muito));
        assert!(gold(&muito) <= 1000);
    }

    #[test]
    fn grava_le_e_respeita_o_teto() {
        let pasta = std::env::temp_dir().join(format!("hyurax-reputacao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        let mut todos = BTreeMap::new();
        let h = Historico { rep: Reputacao { enviadas: 3, verificadas: 2, recusadas: 1, divergentes: 0, disputadas: 0, ..Reputacao::default() }, desde_ms: 5, ultimo_ms: 9 };
        todos.insert([1u8; 32], h);
        gravar(&pasta, &todos).unwrap();
        assert_eq!(ler(&pasta), todos);
        let mut cheio: BTreeMap<[u8; 32], Historico> = (0..MAXIMO as u32)
            .map(|i| {
                let mut w = [0u8; 32];
                w[..4].copy_from_slice(&i.to_be_bytes());
                (w, Historico { rep: Reputacao { enviadas: u64::from(i) + 1, ..Reputacao::default() }, ..Historico::default() })
            })
            .collect();
        abrir_espaco(&mut cheio, &[9u8; 32]);
        assert_eq!(cheio.len(), MAXIMO - 1, "saiu um para o novo");
        assert!(!cheio.contains_key(&[0u8; 32]), "saiu quem tinha menos entregas");
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
