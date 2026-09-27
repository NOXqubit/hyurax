// ✝ Eclesiastes 12:13 — “De tudo o que se tem ouvido, o fim é.”
//! Consolidação dos resultados de um JOB, com memória constante.
//!
//! Cada unidade conferida entra com o índice e os bytes do resultado. O
//! agregador guarda só o que o relatório precisa (o melhor resultado, somas,
//! contagens), nunca a lista de resultados, então um JOB de um bilhão de
//! unidades cabe no mesmo espaço que um de dez. (Genética e melhoramento
//! guardam uma soma por geração: a memória cresce com as gerações do modelo,
//! não com as unidades.)
//!
//! **Não depende da ordem de chegada.** Empate escolhe o menor índice, e somas
//! são somas: dois nós que consolidaram o mesmo conjunto de unidades chegam
//! no mesmo agregador, byte a byte. Uma unidade que entra duas vezes é erro de
//! quem chama (o agendador só soma unidade nova, pelos intervalos).
//!
//! Tudo em inteiros; média, desvio e intervalo de confiança só viram número
//! decimal na hora de mostrar.

use hyurax_codec::{Reader, Writer};

use crate::job::ErroDeJob;
use crate::trabalho::{Especificacao, MOCHILA_RESULTADO_LEN, TipoDeTrabalho};
use crate::{genetica, ia, melhoramento, rotas, triagem};

/// Versão da codificação do agregador.
pub const VERSAO: u8 = 2;
/// Quantas moléculas a triagem guarda no topo.
pub const TOPO_TRIAGEM: usize = 50;

/// O estado consolidado de um JOB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Agregador {
    /// Matriz e difusão: o resultado é conferido e contado; o valor
    /// científico está na verificação, não num número a consolidar.
    Contagem {
        /// Unidades somadas.
        unidades: u64,
    },
    /// Mochila: o maior valor ótimo entre as instâncias, e de qual unidade.
    Mochila {
        /// Unidades somadas.
        unidades: u64,
        /// `(valor ótimo, índice)` da melhor; empate pelo menor índice.
        melhor: Option<(u64, u64)>,
        /// Soma dos valores ótimos, para a média.
        soma: u128,
    },
    /// Treino de IA: o menor erro de validação e de qual unidade.
    Ia {
        /// Unidades somadas.
        unidades: u64,
        /// `(erro Q12, índice)` do melhor treino; empate pelo menor índice.
        melhor: Option<(u64, u64)>,
        /// Soma dos erros.
        soma: u128,
        /// Soma dos quadrados.
        soma_quadrados: u128,
    },
    /// Genética: cada unidade é uma réplica com L loci independentes.
    Genetica {
        /// Réplicas somadas.
        unidades: u64,
        /// 2N, cópias por locus.
        copias: u64,
        /// Loci somados (réplicas × L).
        loci: u64,
        /// Loci com A fixado no fim.
        fixados: u64,
        /// Loci que perderam A no fim.
        perdidos: u64,
        /// Por geração, a soma das contagens de A em todos os loci somados.
        soma: Vec<u128>,
        /// Por geração, a soma dos quadrados das contagens.
        soma_quadrados: Vec<u128>,
    },
    /// Melhoramento: cada unidade é uma réplica do programa de seleção.
    Melhoramento {
        /// Réplicas somadas.
        unidades: u64,
        /// G, gerações de seleção.
        passos: u32,
        /// L, QTL.
        qtl: u32,
        /// Fator ambiental de Liebig (Q16), igual em todas as réplicas.
        fator: u32,
        /// Recurso limitante.
        limitante: u8,
        /// Por geração, a soma da média do valor genético.
        soma_media_g: Vec<u128>,
        /// Por geração, a soma da média do fenótipo.
        soma_media_p: Vec<u128>,
        /// Soma dos quadrados do ganho genético (última menos primeira).
        soma_quadrados_ganho: u128,
        /// Soma da variância genética na primeira e na última geração.
        soma_var_g: (u128, u128),
    },
    /// Rotas: a melhor rota da instância, entre todas as partidas.
    Rotas {
        /// Partidas somadas.
        unidades: u64,
        /// `(comprimento, índice)` da melhor; empate pelo menor índice.
        melhor: Option<(u64, u64)>,
        /// Soma dos comprimentos finais.
        soma: u128,
        /// Soma dos quadrados.
        soma_quadrados: u128,
        /// Partidas que terminaram em ótimo local do 2-opt.
        otimos_locais: u64,
    },
    /// Triagem: as melhores moléculas e as contagens por filtro.
    Triagem {
        /// Faixas somadas.
        unidades: u64,
        /// Moléculas avaliadas.
        moleculas: u64,
        /// Moléculas que passaram em todos os filtros.
        aprovadas: u64,
        /// Quantas passaram em cada filtro.
        por_filtro: [u64; 7],
        /// As melhores `(nota, índice no catálogo, log S previsto)`, da maior
        /// nota para a menor; empate pelo menor índice.
        topo: Vec<(u32, u64, i32)>,
        /// Soma de |previsto − medido| em milésimos de log S.
        erro_abs: u128,
        /// Soma de (previsto − medido)².
        erro_quad: u128,
    },
}

impl Agregador {
    /// Vazio, para a especificação-modelo do JOB.
    pub fn novo(esp: &Especificacao) -> Self {
        match esp.tipo() {
            TipoDeTrabalho::Mochila => Self::Mochila { unidades: 0, melhor: None, soma: 0 },
            TipoDeTrabalho::Ia => Self::Ia { unidades: 0, melhor: None, soma: 0, soma_quadrados: 0 },
            TipoDeTrabalho::Genetica => Self::Genetica {
                unidades: 0,
                copias: u64::from(esp.tamanho()).saturating_mul(2),
                loci: 0,
                fixados: 0,
                perdidos: 0,
                soma: Vec::new(),
                soma_quadrados: Vec::new(),
            },
            TipoDeTrabalho::Melhoramento => Self::Melhoramento {
                unidades: 0,
                passos: esp.passos(),
                qtl: esp.parametros().first().copied().unwrap_or(0),
                fator: 0,
                limitante: 0,
                soma_media_g: Vec::new(),
                soma_media_p: Vec::new(),
                soma_quadrados_ganho: 0,
                soma_var_g: (0, 0),
            },
            TipoDeTrabalho::Rotas => Self::Rotas { unidades: 0, melhor: None, soma: 0, soma_quadrados: 0, otimos_locais: 0 },
            TipoDeTrabalho::Triagem => Self::Triagem {
                unidades: 0,
                moleculas: 0,
                aprovadas: 0,
                por_filtro: [0; 7],
                topo: Vec::new(),
                erro_abs: 0,
                erro_quad: 0,
            },
            TipoDeTrabalho::Matriz | TipoDeTrabalho::Difusao => Self::Contagem { unidades: 0 },
        }
    }

    /// Unidades somadas até aqui.
    pub fn unidades(&self) -> u64 {
        match self {
            Self::Contagem { unidades }
            | Self::Mochila { unidades, .. }
            | Self::Ia { unidades, .. }
            | Self::Genetica { unidades, .. }
            | Self::Melhoramento { unidades, .. }
            | Self::Rotas { unidades, .. }
            | Self::Triagem { unidades, .. } => *unidades,
        }
    }

    /// Soma uma unidade conferida. Resultado ilegível é recusado sem mudar
    /// nada.
    pub fn somar(&mut self, indice: u64, resultado: &[u8]) -> Result<(), ErroDeJob> {
        match self {
            Self::Contagem { unidades } => {
                *unidades = unidades.saturating_add(1);
            }
            Self::Mochila { unidades, melhor, soma } => {
                if resultado.len() != MOCHILA_RESULTADO_LEN {
                    return Err(ErroDeJob::Faixa("resultado de mochila com tamanho errado"));
                }
                let valor = resultado
                    .first_chunk::<8>()
                    .map(|b| u64::from_be_bytes(*b))
                    .ok_or(ErroDeJob::Faixa("resultado de mochila curto"))?;
                *unidades = unidades.saturating_add(1);
                *soma = soma.saturating_add(u128::from(valor));
                if melhor.is_none_or(|(v, i)| valor > v || (valor == v && indice < i)) {
                    *melhor = Some((valor, indice));
                }
            }
            Self::Ia { unidades, melhor, soma, soma_quadrados } => {
                let (_, erro) = ia::decodificar(resultado).ok_or(ErroDeJob::Faixa("resultado de IA ilegível"))?;
                *unidades = unidades.saturating_add(1);
                *soma = soma.saturating_add(u128::from(erro));
                *soma_quadrados = soma_quadrados.saturating_add(u128::from(erro).saturating_mul(u128::from(erro)));
                if melhor.is_none_or(|(e, i)| erro < e || (erro == e && indice < i)) {
                    *melhor = Some((erro, indice));
                }
            }
            Self::Genetica { unidades, copias, loci, fixados, perdidos, soma, soma_quadrados } => {
                let t = genetica::Trajetoria::decodificar(resultado).ok_or(ErroDeJob::Faixa("resultado de genética ilegível"))?;
                if u64::from(t.tamanho).saturating_mul(2) != *copias || (!soma.is_empty() && soma.len() != (t.geracoes as usize).saturating_add(1)) {
                    return Err(ErroDeJob::Faixa("réplica de genética com outra população ou outras gerações"));
                }
                let e = t.estatisticas();
                somar_vetor(soma, &e.soma_por_geracao);
                somar_vetor(soma_quadrados, &e.soma_quadrados_por_geracao);
                *unidades = unidades.saturating_add(1);
                *loci = loci.saturating_add(u64::from(t.loci));
                *fixados = fixados.saturating_add(u64::from(e.fixados));
                *perdidos = perdidos.saturating_add(u64::from(e.perdidos));
            }
            Self::Melhoramento { unidades, passos, qtl, fator, limitante, soma_media_g, soma_media_p, soma_quadrados_ganho, soma_var_g } => {
                let r = melhoramento::Resultado::decodificar(resultado, *passos, *qtl)
                    .ok_or(ErroDeJob::Faixa("resultado de melhoramento ilegível"))?;
                let (Some(primeira), Some(ultima)) = (r.linhas.first(), r.linhas.last()) else {
                    return Err(ErroDeJob::Faixa("resultado de melhoramento sem gerações"));
                };
                if *unidades > 0 && (*fator != r.fator || *limitante != r.limitante) {
                    return Err(ErroDeJob::Faixa("réplica de melhoramento com outro ambiente"));
                }
                let ganho = i128::from(ultima.media_g).saturating_sub(i128::from(primeira.media_g));
                let medias_g: Vec<u64> = r.linhas.iter().map(|l| l.media_g).collect();
                let medias_p: Vec<u64> = r.linhas.iter().map(|l| l.media_p).collect();
                somar_vetor(soma_media_g, &medias_g);
                somar_vetor(soma_media_p, &medias_p);
                *soma_quadrados_ganho = soma_quadrados_ganho.saturating_add(u128::try_from(ganho.saturating_mul(ganho)).unwrap_or(u128::MAX));
                *soma_var_g = (
                    soma_var_g.0.saturating_add(u128::from(primeira.var_g)),
                    soma_var_g.1.saturating_add(u128::from(ultima.var_g)),
                );
                *fator = r.fator;
                *limitante = r.limitante;
                *unidades = unidades.saturating_add(1);
            }
            Self::Rotas { unidades, melhor, soma, soma_quadrados, otimos_locais } => {
                let comprimento = resultado
                    .first_chunk::<8>()
                    .map(|b| u64::from_be_bytes(*b))
                    .filter(|_| resultado.len() > rotas::CABECALHO)
                    .ok_or(ErroDeJob::Faixa("resultado de rotas ilegível"))?;
                let otimo = resultado.get(rotas::CABECALHO.saturating_sub(1)).copied() == Some(1);
                *unidades = unidades.saturating_add(1);
                *soma = soma.saturating_add(u128::from(comprimento));
                *soma_quadrados = soma_quadrados.saturating_add(u128::from(comprimento).saturating_mul(u128::from(comprimento)));
                if otimo {
                    *otimos_locais = otimos_locais.saturating_add(1);
                }
                if melhor.is_none_or(|(c, i)| comprimento < c || (comprimento == c && indice < i)) {
                    *melhor = Some((comprimento, indice));
                }
            }
            Self::Triagem { unidades, moleculas, aprovadas, por_filtro, topo, erro_abs, erro_quad } => {
                let (inicio, aprovadas_aqui, linhas) =
                    triagem::decodificar(resultado).ok_or(ErroDeJob::Faixa("resultado de triagem ilegível"))?;
                let catalogo = triagem::catalogo();
                for (k, (mascara, previsto, nota)) in linhas.iter().enumerate() {
                    let i = u64::from(inicio).saturating_add(u64::try_from(k).unwrap_or(u64::MAX));
                    for (bit, contador) in por_filtro.iter_mut().enumerate() {
                        if mascara & 1u8.wrapping_shl(u32::try_from(bit).unwrap_or(0)) != 0 {
                            *contador = contador.saturating_add(1);
                        }
                    }
                    if let Some(m) = usize::try_from(i).ok().and_then(|i| catalogo.moleculas.get(i)) {
                        let diferenca = i128::from(*previsto).saturating_sub(i128::from(m.logs_mili));
                        *erro_abs = erro_abs.saturating_add(diferenca.unsigned_abs());
                        *erro_quad = erro_quad.saturating_add(diferenca.unsigned_abs().saturating_mul(diferenca.unsigned_abs()));
                    }
                    // topo: maior nota primeiro, empate pelo menor índice
                    let entrada = (*nota, i, *previsto);
                    let pos = topo.partition_point(|&(n, j, _)| n > entrada.0 || (n == entrada.0 && j < entrada.1));
                    if pos < TOPO_TRIAGEM {
                        topo.insert(pos, entrada);
                        topo.truncate(TOPO_TRIAGEM);
                    }
                }
                *moleculas = moleculas.saturating_add(u64::try_from(linhas.len()).unwrap_or(u64::MAX));
                *aprovadas = aprovadas.saturating_add(u64::from(aprovadas_aqui));
                *unidades = unidades.saturating_add(1);
            }
        }
        Ok(())
    }

    /// Codificação canônica, para o checkpoint.
    pub fn codificar(&self, w: &mut Writer) {
        w.u8(VERSAO);
        let opcional = |w: &mut Writer, par: &Option<(u64, u64)>| match par {
            Some((a, b)) => {
                w.u8(1);
                w.u64(*a);
                w.u64(*b);
            }
            None => w.u8(0),
        };
        let grande = |w: &mut Writer, v: u128| w.raw(&v.to_be_bytes());
        let vetor = |w: &mut Writer, v: &[u128]| {
            w.u32(u32::try_from(v.len()).unwrap_or(u32::MAX));
            for x in v {
                w.raw(&x.to_be_bytes());
            }
        };
        match self {
            Self::Contagem { unidades } => {
                w.u8(1);
                w.u64(*unidades);
            }
            Self::Mochila { unidades, melhor, soma } => {
                w.u8(2);
                w.u64(*unidades);
                opcional(w, melhor);
                grande(w, *soma);
            }
            Self::Ia { unidades, melhor, soma, soma_quadrados } => {
                w.u8(3);
                w.u64(*unidades);
                opcional(w, melhor);
                grande(w, *soma);
                grande(w, *soma_quadrados);
            }
            Self::Genetica { unidades, copias, loci, fixados, perdidos, soma, soma_quadrados } => {
                w.u8(4);
                for v in [*unidades, *copias, *loci, *fixados, *perdidos] {
                    w.u64(v);
                }
                vetor(w, soma);
                vetor(w, soma_quadrados);
            }
            Self::Melhoramento { unidades, passos, qtl, fator, limitante, soma_media_g, soma_media_p, soma_quadrados_ganho, soma_var_g } => {
                w.u8(5);
                w.u64(*unidades);
                w.u32(*passos);
                w.u32(*qtl);
                w.u32(*fator);
                w.u8(*limitante);
                vetor(w, soma_media_g);
                vetor(w, soma_media_p);
                grande(w, *soma_quadrados_ganho);
                grande(w, soma_var_g.0);
                grande(w, soma_var_g.1);
            }
            Self::Rotas { unidades, melhor, soma, soma_quadrados, otimos_locais } => {
                w.u8(6);
                w.u64(*unidades);
                opcional(w, melhor);
                grande(w, *soma);
                grande(w, *soma_quadrados);
                w.u64(*otimos_locais);
            }
            Self::Triagem { unidades, moleculas, aprovadas, por_filtro, topo, erro_abs, erro_quad } => {
                w.u8(7);
                w.u64(*unidades);
                w.u64(*moleculas);
                w.u64(*aprovadas);
                for v in por_filtro {
                    w.u64(*v);
                }
                w.u32(u32::try_from(topo.len()).unwrap_or(u32::MAX));
                for (nota, i, previsto) in topo {
                    w.u32(*nota);
                    w.u64(*i);
                    w.raw(&previsto.to_be_bytes());
                }
                grande(w, *erro_abs);
                grande(w, *erro_quad);
            }
        }
    }

    /// O inverso de [`Self::codificar`].
    pub fn decodificar(r: &mut Reader<'_>) -> Result<Self, ErroDeJob> {
        if r.u8()? != VERSAO {
            return Err(ErroDeJob::Faixa("versão de agregador desconhecida"));
        }
        let opcional = |r: &mut Reader<'_>| -> Result<Option<(u64, u64)>, ErroDeJob> {
            Ok(match r.u8()? {
                0 => None,
                1 => Some((r.u64()?, r.u64()?)),
                _ => return Err(ErroDeJob::Faixa("marcador inválido no agregador")),
            })
        };
        let grande = |r: &mut Reader<'_>| -> Result<u128, ErroDeJob> { Ok(u128::from_be_bytes(r.fixed::<16>()?)) };
        // o comprimento vem do arquivo: nada é reservado antes de ler cada item
        let vetor = |r: &mut Reader<'_>| -> Result<Vec<u128>, ErroDeJob> {
            let n = r.u32()?;
            let mut v = Vec::new();
            for _ in 0..n {
                v.push(u128::from_be_bytes(r.fixed::<16>()?));
            }
            Ok(v)
        };
        Ok(match r.u8()? {
            1 => Self::Contagem { unidades: r.u64()? },
            2 => Self::Mochila { unidades: r.u64()?, melhor: opcional(r)?, soma: grande(r)? },
            3 => Self::Ia { unidades: r.u64()?, melhor: opcional(r)?, soma: grande(r)?, soma_quadrados: grande(r)? },
            4 => Self::Genetica {
                unidades: r.u64()?,
                copias: r.u64()?,
                loci: r.u64()?,
                fixados: r.u64()?,
                perdidos: r.u64()?,
                soma: vetor(r)?,
                soma_quadrados: vetor(r)?,
            },
            5 => Self::Melhoramento {
                unidades: r.u64()?,
                passos: r.u32()?,
                qtl: r.u32()?,
                fator: r.u32()?,
                limitante: r.u8()?,
                soma_media_g: vetor(r)?,
                soma_media_p: vetor(r)?,
                soma_quadrados_ganho: grande(r)?,
                soma_var_g: (grande(r)?, grande(r)?),
            },
            6 => Self::Rotas {
                unidades: r.u64()?,
                melhor: opcional(r)?,
                soma: grande(r)?,
                soma_quadrados: grande(r)?,
                otimos_locais: r.u64()?,
            },
            7 => {
                let (unidades, moleculas, aprovadas) = (r.u64()?, r.u64()?, r.u64()?);
                let mut por_filtro = [0u64; 7];
                for v in &mut por_filtro {
                    *v = r.u64()?;
                }
                let n = r.u32()?;
                if usize::try_from(n).unwrap_or(usize::MAX) > TOPO_TRIAGEM {
                    return Err(ErroDeJob::Faixa("topo da triagem maior que o limite"));
                }
                let mut topo = Vec::new();
                for _ in 0..n {
                    topo.push((r.u32()?, r.u64()?, i32::from_be_bytes(r.fixed::<4>()?)));
                }
                Self::Triagem { unidades, moleculas, aprovadas, por_filtro, topo, erro_abs: grande(r)?, erro_quad: grande(r)? }
            }
            _ => return Err(ErroDeJob::Faixa("tipo de agregador desconhecido")),
        })
    }

    /// Resumo em texto para a tela e o relatório.
    pub fn texto(&self) -> String {
        match self {
            Self::Contagem { unidades } => format!("{unidades} unidade(s) conferida(s)"),
            Self::Mochila { unidades, melhor, soma } => match melhor {
                Some((v, i)) => format!(
                    "{unidades} instância(s) resolvidas no ótimo; maior valor ótimo {v} (unidade {i}); média {:.1}",
                    media(*soma, *unidades)
                ),
                None => "nenhuma instância ainda".into(),
            },
            Self::Ia { unidades, melhor, soma, soma_quadrados } => match melhor {
                Some((e, i)) => format!(
                    "{unidades} treino(s); menor erro de validação {:.4} (unidade {i}); média {:.4} ± {:.4} (desvio entre treinos), em unidades normalizadas Q12",
                    *e as f64 / 4096.0,
                    media(*soma, *unidades) / 4096.0,
                    desvio(*soma, *soma_quadrados, *unidades) / 4096.0
                ),
                None => "nenhum treino ainda".into(),
            },
            Self::Genetica { unidades, copias, loci, fixados, perdidos, soma, .. } => {
                if *loci == 0 {
                    return "nenhuma réplica ainda".into();
                }
                let n = *loci as f64;
                let (pf, pp) = (*fixados as f64 / n, *perdidos as f64 / n);
                let meia = |p: f64| 1.96 * (p * (1.0 - p) / n).sqrt();
                let freq = |g: usize| soma.get(g).map_or(0.0, |s| *s as f64 / (n * *copias as f64));
                format!(
                    "{unidades} réplica(s), {loci} loci: A fixado em {:.1}% (± {:.1} p.p., 95%), perdido em {:.1}% (± {:.1}); frequência média de A {:.4} no início e {:.4} na última geração",
                    pf * 100.0,
                    meia(pf) * 100.0,
                    pp * 100.0,
                    meia(pp) * 100.0,
                    freq(0),
                    freq(soma.len().saturating_sub(1))
                )
            }
            Self::Melhoramento { unidades, fator, limitante, soma_media_g, soma_media_p, soma_quadrados_ganho, soma_var_g, .. } => {
                if *unidades == 0 {
                    return "nenhuma réplica ainda".into();
                }
                let u = *unidades as f64;
                let primeira = soma_media_g.first().map_or(0.0, |s| *s as f64 / u);
                let ultima = soma_media_g.last().map_or(0.0, |s| *s as f64 / u);
                let ganho = ultima - primeira;
                let dp = if *unidades > 1 { ((*soma_quadrados_ganho as f64 - u * ganho * ganho) / (u - 1.0)).max(0.0).sqrt() } else { 0.0 };
                let recurso = melhoramento::NOMES_DOS_RECURSOS.get(usize::from(*limitante)).copied().unwrap_or("?");
                format!(
                    "{unidades} réplica(s): ganho genético médio {ganho:.1} ± {:.1} (erro-padrão entre réplicas) em {} geração(ões); fenótipo médio {:.1} → {:.1}; variância genética {:.1} → {:.1}; fator ambiental {:.3} (limitado por {recurso})",
                    dp / u.sqrt(),
                    soma_media_g.len().saturating_sub(1),
                    soma_media_p.first().map_or(0.0, |s| *s as f64 / u),
                    soma_media_p.last().map_or(0.0, |s| *s as f64 / u),
                    soma_var_g.0 as f64 / u,
                    soma_var_g.1 as f64 / u,
                    *fator as f64 / f64::from(melhoramento::Q16)
                )
            }
            Self::Rotas { unidades, melhor, soma, soma_quadrados, otimos_locais } => match melhor {
                Some((c, i)) => format!(
                    "{unidades} partida(s): melhor rota {c} (unidade {i}); média {:.1} ± {:.1}; {otimos_locais} em ótimo local do 2-opt (não é o ótimo global)",
                    media(*soma, *unidades),
                    desvio(*soma, *soma_quadrados, *unidades)
                ),
                None => "nenhuma partida ainda".into(),
            },
            Self::Triagem { moleculas, aprovadas, topo, erro_abs, erro_quad, .. } => {
                if *moleculas == 0 {
                    return "nenhuma molécula ainda".into();
                }
                let n = *moleculas as f64;
                let melhor = topo.first().and_then(|(_, i, p)| {
                    let m = triagem::catalogo().moleculas.get(usize::try_from(*i).ok()?)?;
                    Some(format!("; melhor nota: {} ({}), log S previsto {:.2}, medido {:.2}", m.nome, m.id, f64::from(*p) / 1000.0, m.logs_mili as f64 / 1000.0))
                });
                format!(
                    "{moleculas} molécula(s) reais triadas, {aprovadas} passam em todos os filtros{}; erro do modelo nestas moléculas contra o medido: {:.2} log S (raiz do EQM), {:.2} (erro absoluto médio)",
                    melhor.unwrap_or_default(),
                    ((*erro_quad as f64) / n).sqrt() / 1000.0,
                    (*erro_abs as f64) / n / 1000.0
                )
            }
        }
    }
}

fn somar_vetor(destino: &mut Vec<u128>, valores: &[u64]) {
    if destino.is_empty() {
        destino.resize(valores.len(), 0);
    }
    for (d, v) in destino.iter_mut().zip(valores) {
        *d = d.saturating_add(u128::from(*v));
    }
}

/// Média de uma soma inteira, só para mostrar.
pub fn media(soma: u128, n: u64) -> f64 {
    if n == 0 { 0.0 } else { soma as f64 / n as f64 }
}

/// Desvio-padrão amostral a partir de soma e soma dos quadrados, só para
/// mostrar.
pub fn desvio(soma: u128, soma_quadrados: u128, n: u64) -> f64 {
    if n < 2 {
        return 0.0;
    }
    let n = n as f64;
    let m = soma as f64 / n;
    ((soma_quadrados as f64 - n * m * m) / (n - 1.0)).max(0.0).sqrt()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod testes {
    use super::*;
    use crate::trabalho::executar;
    use hyurax_crypto::sha512;

    fn mochila(valor: u64) -> Vec<u8> {
        let mut r = valor.to_be_bytes().to_vec();
        r.extend_from_slice(&[0; 32]);
        r
    }

    fn resultados(esp: &Especificacao, n: u64) -> Vec<Vec<u8>> {
        (0..n).map(|k| executar(esp, &sha512(&k.to_be_bytes()), &mut |_| true).unwrap().resultado).collect()
    }

    fn vai_e_volta(a: &Agregador) {
        let mut w = Writer::new();
        a.codificar(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(&Agregador::decodificar(&mut r).unwrap(), a);
        r.finish().unwrap();
    }

    /// Soma os resultados nas duas ordens e confere que dá o mesmo, e que
    /// sobrevive ao checkpoint.
    fn nas_duas_ordens(esp: &Especificacao, rs: &[Vec<u8>]) -> Agregador {
        let mut a = Agregador::novo(esp);
        let mut b = Agregador::novo(esp);
        for (i, r) in rs.iter().enumerate() {
            a.somar(i as u64, r).unwrap();
        }
        for (i, r) in rs.iter().enumerate().rev() {
            b.somar(i as u64, r).unwrap();
        }
        assert_eq!(a, b, "{:?}", esp.tipo());
        vai_e_volta(&a);
        assert!(!a.texto().is_empty());
        a
    }

    #[test]
    fn mochila_empata_pelo_menor_indice() {
        let e = Especificacao::nova(TipoDeTrabalho::Mochila, 8, 0).unwrap();
        let rs: Vec<Vec<u8>> = [5u64, 9, 3, 9, 7].iter().map(|v| mochila(*v)).collect();
        let a = nas_duas_ordens(&e, &rs);
        assert!(matches!(a, Agregador::Mochila { melhor: Some((9, 1)), .. }));
    }

    #[test]
    fn os_quatro_motores_consolidam_em_qualquer_ordem() {
        let g = Especificacao::nova_com(TipoDeTrabalho::Genetica, 50, 20, &[4, 0, 10_000, 50, 5_000]).unwrap();
        let a = nas_duas_ordens(&g, &resultados(&g, 6));
        assert!(matches!(a, Agregador::Genetica { unidades: 6, loci: 24, .. }), "{a:?}");

        let m = Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 40, 3, &[10, 7, 50, 100, 100, 100, 100, 0]).unwrap();
        nas_duas_ordens(&m, &resultados(&m, 4));

        let r = Especificacao::nova_com(TipoDeTrabalho::Rotas, 30, 100, &[3]).unwrap();
        let a = nas_duas_ordens(&r, &resultados(&r, 5));
        assert!(matches!(a, Agregador::Rotas { otimos_locais: 5, .. }), "{a:?}");

        let t = Especificacao::nova_com(TipoDeTrabalho::Triagem, 200, 0, &[0, 0, 500_000, 25_000, 5, 10, 10, 140_000, 16_000]).unwrap();
        let faixas: Vec<Vec<u8>> = (0u64..4)
            .map(|k| {
                let u = crate::trabalho::derivar_unidade(&t, k).unwrap();
                executar(&u, b"", &mut |_| true).unwrap().resultado
            })
            .collect();
        let a = nas_duas_ordens(&t, &faixas);
        let Agregador::Triagem { moleculas, topo, .. } = &a else { panic!("não é triagem") };
        assert_eq!(*moleculas, 800);
        assert_eq!(topo.len(), TOPO_TRIAGEM);
        assert!(topo.windows(2).all(|w| w[0].0 > w[1].0 || (w[0].0 == w[1].0 && w[0].1 < w[1].1)), "topo em ordem");
    }

    #[test]
    fn resultado_ilegivel_e_recusado_sem_mudar_nada() {
        for e in [
            Especificacao::nova(TipoDeTrabalho::Mochila, 8, 0).unwrap(),
            Especificacao::nova(TipoDeTrabalho::Ia, 4, 3).unwrap(),
            Especificacao::nova_com(TipoDeTrabalho::Genetica, 50, 20, &[4, 0, 10_000, 50, 5_000]).unwrap(),
            Especificacao::nova_com(TipoDeTrabalho::Rotas, 30, 100, &[3]).unwrap(),
            Especificacao::nova_com(TipoDeTrabalho::Triagem, 20, 0, &[0; 9]).unwrap(),
        ] {
            let mut a = Agregador::novo(&e);
            let antes = a.clone();
            assert!(a.somar(0, &[1, 2, 3]).is_err(), "{:?}", e.tipo());
            assert_eq!(a, antes);
        }
    }
}
