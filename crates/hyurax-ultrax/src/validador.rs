// ✝ Deuteronômio 19:15 — “Pelo depoimento de duas ou três testemunhas se estabelecerá o fato.”
//! O validador: compara entregas de workers diferentes e dá um parecer para
//! cada uma.
//!
//! Dois caminhos, e a diferença entre eles é o que decide quem é punido:
//!
//! - [`por_verificacao`]: cada entrega é conferida pelo método do tipo
//!   (Freivalds, recomputação). Quem falha é **recusado com evidência**: o
//!   motivo da recusa. Duas respostas diferentes podem estar ambas certas (a
//!   mochila pode ter mais de uma escolha ótima), e as duas passam.
//! - [`por_maioria`]: para trabalho sem conferência própria, vale a maioria
//!   estrita de workers **distintos**. Quem diverge fica **divergente**, não
//!   recusado: maioria não é prova, e o pedido é nova verificação, não
//!   punição.
//!
//! Na instância compartilhada, que a maioria exige, um worker preguiçoso
//! poderia copiar a resposta de outro. Contra isso, cada um publica antes um
//! [`compromisso`] com o hash do resultado e a própria identidade, e só depois
//! revela.

use crate::tarefa::Tarefa;
use crate::trabalho::{Recusa, verificar};
use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, sha512};

/// Domínio do compromisso antes da revelação.
pub const DOMINIO_COMPROMISSO: &[u8] = dominio!("COMPROMISSO-v1");

/// O parecer sobre uma entrega.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parecer {
    /// Passou.
    Aceito,
    /// Não passou, e o motivo é a evidência.
    Recusado(Recusa),
    /// Diferente da maioria, sem prova de erro. Pede nova verificação.
    Divergente,
    /// O mesmo worker entregou de novo; só a primeira conta.
    Repetida,
}

/// O desfecho da tarefa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Desfecho {
    /// Pelo menos um resultado foi aceito.
    Verificado {
        /// Quantas entregas passaram.
        aceitos: usize,
    },
    /// Não houve maioria: nada é aceito, e ninguém é punido.
    Disputado,
    /// Todas as entregas falharam na conferência.
    Recusado,
}

/// A decisão: desfecho e parecer de cada entrega, na ordem em que vieram.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decisao {
    /// O desfecho.
    pub desfecho: Desfecho,
    /// `(worker, parecer)`.
    pub pareceres: Vec<([u8; PUBKEY_LEN], Parecer)>,
}

/// Uma entrega de resultado completo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrega {
    /// Quem entregou.
    pub worker: [u8; PUBKEY_LEN],
    /// O resultado, nos bytes do tipo.
    pub resultado: Vec<u8>,
}

/// Compromisso: `H(DOMINIO_COMPROMISSO || hash do resultado || worker)`.
///
/// O worker está dentro: copiar o compromisso de outro não serve, porque na
/// revelação a conta sai diferente.
pub fn compromisso(resultado: &[u8; HASH_LEN], worker: &[u8; PUBKEY_LEN]) -> [u8; HASH_LEN] {
    let mut dados = DOMINIO_COMPROMISSO.to_vec();
    dados.extend_from_slice(resultado);
    dados.extend_from_slice(worker);
    sha512(&dados)
}

/// A revelação bate com o compromisso publicado antes?
pub fn revelacao_confere(publicado: &[u8; HASH_LEN], resultado: &[u8; HASH_LEN], worker: &[u8; PUBKEY_LEN]) -> bool {
    compromisso(resultado, worker) == *publicado
}

/// Confere cada entrega pelo método do tipo, com a semente de cada worker.
pub fn por_verificacao(tarefa: &Tarefa, entregas: &[Entrega]) -> Decisao {
    let mut vistos: Vec<[u8; PUBKEY_LEN]> = Vec::new();
    let mut pareceres = Vec::with_capacity(entregas.len());
    for e in entregas {
        let parecer = if vistos.contains(&e.worker) {
            Parecer::Repetida
        } else {
            vistos.push(e.worker);
            match verificar(&tarefa.especificacao, &tarefa.semente_para(&e.worker), &e.resultado) {
                Ok(()) => Parecer::Aceito,
                Err(r) => Parecer::Recusado(r),
            }
        };
        pareceres.push((e.worker, parecer));
    }
    let aceitos = pareceres.iter().filter(|(_, p)| *p == Parecer::Aceito).count();
    let desfecho = match aceitos {
        0 if pareceres.is_empty() => Desfecho::Disputado,
        0 => Desfecho::Recusado,
        aceitos => Desfecho::Verificado { aceitos },
    };
    Decisao { desfecho, pareceres }
}

/// Maioria estrita entre workers distintos, com pelo menos `quorum` iguais.
///
/// `entregas` traz `(worker, hash do resultado)`. Empate ou maioria abaixo do
/// quórum: [`Desfecho::Disputado`], e todo mundo fica divergente.
pub fn por_maioria(entregas: &[([u8; PUBKEY_LEN], [u8; HASH_LEN])], quorum: usize) -> Decisao {
    let mut vistos: Vec<[u8; PUBKEY_LEN]> = Vec::new();
    let mut validas: Vec<(usize, [u8; HASH_LEN])> = Vec::new();
    for (i, (worker, hash)) in entregas.iter().enumerate() {
        if !vistos.contains(worker) {
            vistos.push(*worker);
            validas.push((i, *hash));
        }
    }
    // o grupo mais votado
    let mut vencedor: Option<([u8; HASH_LEN], usize)> = None;
    for (_, hash) in &validas {
        let votos = validas.iter().filter(|(_, h)| h == hash).count();
        if vencedor.is_none_or(|(_, v)| votos > v) {
            vencedor = Some((*hash, votos));
        }
    }
    let maioria = vencedor.filter(|&(_, votos)| votos >= quorum.max(1) && votos.saturating_mul(2) > validas.len());

    let pareceres = entregas
        .iter()
        .enumerate()
        .map(|(i, (worker, hash))| {
            let parecer = if !validas.iter().any(|(j, _)| *j == i) {
                Parecer::Repetida
            } else if maioria.is_some_and(|(h, _)| h == *hash) {
                Parecer::Aceito
            } else {
                Parecer::Divergente
            };
            (*worker, parecer)
        })
        .collect();
    let desfecho = match maioria {
        Some((_, votos)) => Desfecho::Verificado { aceitos: votos },
        None => Desfecho::Disputado,
    };
    Decisao { desfecho, pareceres }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::tarefa::Instancia;
    use crate::trabalho::{Especificacao, MetodoDeVerificacao, TipoDeTrabalho, executar, hash_do_resultado};

    const A: [u8; 32] = [0xA; 32];
    const B: [u8; 32] = [0xB; 32];
    const C: [u8; 32] = [0xC; 32];

    fn tarefa(tipo: TipoDeTrabalho, tamanho: u32, instancia: Instancia) -> Tarefa {
        let esp = Especificacao::nova(tipo, tamanho, 0).unwrap();
        Tarefa::nova(esp, tipo.metodo(), instancia, b"t", 0, 0, 1).unwrap()
    }

    fn honesto(t: &Tarefa, worker: [u8; 32]) -> Entrega {
        let exec = executar(&t.especificacao, &t.semente_para(&worker), &mut |_| true).unwrap();
        Entrega { worker, resultado: exec.resultado }
    }

    /// O caso do pedido: A entrega Y, B e C entregam X.
    #[test]
    fn um_contra_dois_com_verificacao_recusa_com_evidencia() {
        let t = tarefa(TipoDeTrabalho::Matriz, 16, Instancia::PorWorker);
        let mut a = honesto(&t, A);
        a.resultado[0] ^= 1;
        let d = por_verificacao(&t, &[a, honesto(&t, B), honesto(&t, C)]);
        assert_eq!(d.desfecho, Desfecho::Verificado { aceitos: 2 });
        assert!(matches!(&d.pareceres[0].1, Parecer::Recusado(r) if r.0.contains("Freivalds")));
        assert_eq!(d.pareceres[1].1, Parecer::Aceito);
    }

    #[test]
    fn um_contra_dois_por_maioria_so_marca_divergencia() {
        let (x, y) = (hash_do_resultado(b"X"), hash_do_resultado(b"Y"));
        let d = por_maioria(&[(A, y), (B, x), (C, x)], 2);
        assert_eq!(d.desfecho, Desfecho::Verificado { aceitos: 2 });
        assert_eq!(d.pareceres[0].1, Parecer::Divergente, "sem evidência, não é recusa");
        assert_eq!(d.pareceres[2].1, Parecer::Aceito);
    }

    #[test]
    fn empate_vira_disputa() {
        let (x, y) = (hash_do_resultado(b"X"), hash_do_resultado(b"Y"));
        let d = por_maioria(&[(A, x), (B, y)], 1);
        assert_eq!(d.desfecho, Desfecho::Disputado);
        assert!(d.pareceres.iter().all(|(_, p)| *p == Parecer::Divergente));
    }

    #[test]
    fn o_mesmo_worker_nao_vota_duas_vezes() {
        let (x, y) = (hash_do_resultado(b"X"), hash_do_resultado(b"Y"));
        // A tenta ganhar no volume
        let d = por_maioria(&[(A, y), (A, y), (A, y), (B, x), (C, x)], 2);
        assert_eq!(d.desfecho, Desfecho::Verificado { aceitos: 2 });
        assert_eq!(d.pareceres[1].1, Parecer::Repetida);
        assert_eq!(d.pareceres[0].1, Parecer::Divergente);
    }

    #[test]
    fn todas_erradas_e_recusa() {
        let t = tarefa(TipoDeTrabalho::Mochila, 20, Instancia::PorWorker);
        let d = por_verificacao(&t, &[Entrega { worker: A, resultado: vec![0; 40] }]);
        assert_eq!(d.desfecho, Desfecho::Recusado);
    }

    #[test]
    fn copiar_resultado_de_outro_worker_nao_serve() {
        let t = tarefa(TipoDeTrabalho::Matriz, 8, Instancia::PorWorker);
        let copia = Entrega { worker: B, resultado: honesto(&t, A).resultado };
        let d = por_verificacao(&t, &[copia]);
        assert_eq!(d.desfecho, Desfecho::Recusado);
    }

    #[test]
    fn compromisso_amarra_resultado_e_worker() {
        let x = hash_do_resultado(b"X");
        let c = compromisso(&x, &A);
        assert!(revelacao_confere(&c, &x, &A));
        assert!(!revelacao_confere(&c, &x, &B), "copiou o compromisso de A");
        assert!(!revelacao_confere(&c, &hash_do_resultado(b"Y"), &A));
    }

    #[test]
    fn metodo_da_tarefa_e_o_do_tipo() {
        assert_eq!(TipoDeTrabalho::Matriz.metodo(), MetodoDeVerificacao::Freivalds);
    }
}
