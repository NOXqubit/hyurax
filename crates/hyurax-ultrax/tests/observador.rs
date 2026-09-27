//! O observador não muda o cálculo, e o que ele recebe é o dado real.
//!
//! Para cada motor: o resultado com um observador que quer amostra em todo
//! ponto é **igual, byte a byte**, ao resultado sem observador; e as amostras
//! batem com o resultado (a última linha da matriz é a última linha de C, as
//! contagens da genética são as gravadas, o comprimento da rota é o
//! devolvido, a nota de cada molécula é a da linha do resultado).

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use hyurax_ultrax::observador::{Amostra, Observador};
use hyurax_ultrax::trabalho::{self, Especificacao, TipoDeTrabalho};
use hyurax_ultrax::{rotas, triagem};

/// Quer amostra sempre, e guarda todas.
#[derive(Default)]
struct Tudo(Vec<Amostra>);

impl Observador for Tudo {
    fn quer(&mut self) -> bool {
        true
    }
    fn amostra(&mut self, a: Amostra) {
        self.0.push(a);
    }
}

const SEMENTE: &[u8] = b"teste do observador";

fn os_dois(esp: &Especificacao) -> (Vec<u8>, Vec<Amostra>) {
    let sem = trabalho::executar(esp, SEMENTE, &mut |_| true).unwrap().resultado;
    let mut obs = Tudo::default();
    let com = trabalho::executar_observado(esp, SEMENTE, &mut |_| true, &mut obs).unwrap().resultado;
    assert_eq!(sem, com, "{}: o observador mudou o resultado", esp.resumo());
    assert!(!obs.0.is_empty(), "{}: nenhuma amostra", esp.resumo());
    assert!(obs.0.iter().all(|a| a.tipo() == esp.tipo().nome()), "{}: amostra de outro tipo", esp.resumo());
    (sem, obs.0)
}

fn i64s(bytes: &[u8]) -> Vec<i64> {
    bytes.as_chunks::<8>().0.iter().map(|c| i64::from_le_bytes(*c)).collect()
}

#[test]
fn matriz_amostra_as_linhas_reais_de_c() {
    let n = 24u32;
    let (r, amostras) = os_dois(&Especificacao::nova(TipoDeTrabalho::Matriz, n, 0).unwrap());
    assert_eq!(amostras.len(), n as usize, "uma amostra por linha");
    let c = i64s(&r);
    for a in &amostras {
        let Amostra::Matriz { linha, valores, a_cada, .. } = a else { panic!() };
        assert_eq!(*a_cada, 1);
        let ini = (*linha * n) as usize;
        assert_eq!(valores.as_slice(), &c[ini..ini + n as usize], "linha {linha}");
    }
}

#[test]
fn mochila_uma_amostra_por_item() {
    let (_, amostras) = os_dois(&Especificacao::nova(TipoDeTrabalho::Mochila, 20, 0).unwrap());
    assert_eq!(amostras.len(), 20);
}

#[test]
fn difusao_a_ultima_grade_e_a_do_resultado() {
    let lado = 20u32;
    let (r, amostras) = os_dois(&Especificacao::nova(TipoDeTrabalho::Difusao, lado, 7).unwrap());
    assert_eq!(amostras.len(), 7);
    let Some(Amostra::Difusao { grade, a_cada, lado_amostra, passo, .. }) = amostras.last() else { panic!() };
    assert_eq!((*a_cada, *lado_amostra, *passo), (1, lado, 7));
    assert_eq!(grade, &i64s(&r));
}

#[test]
fn ia_uma_amostra_por_passo_com_a_curva() {
    let esp = Especificacao::nova(TipoDeTrabalho::Ia, 8, 12).unwrap();
    let exec = trabalho::executar(&esp, SEMENTE, &mut |_| true).unwrap();
    let (_, amostras) = os_dois(&esp);
    assert_eq!(amostras.len(), 12);
    let perdas: Vec<u64> = amostras.iter().map(|a| if let Amostra::Ia { perda, .. } = a { *perda } else { panic!() }).collect();
    assert_eq!(perdas, exec.curva, "a perda de cada passo é a da curva do treino");
}

#[test]
fn genetica_as_contagens_sao_as_gravadas() {
    let esp = Especificacao::nova_com(TipoDeTrabalho::Genetica, 200, 30, &[6, 0, 10_100, 50, 1000]).unwrap();
    let (r, amostras) = os_dois(&esp);
    assert_eq!(amostras.len(), 30, "uma amostra por geração");
    // o fim do resultado: as contagens da última geração, u32 big-endian por locus
    let Some(Amostra::Genetica { contagens, copias, geracao, .. }) = amostras.last() else { panic!() };
    assert_eq!((*copias, *geracao), (400, 30));
    let fim: Vec<u32> = r[r.len() - 6 * 4..].as_chunks::<4>().0.iter().map(|c| u32::from_be_bytes(*c)).collect();
    assert_eq!(contagens, &fim);
}

#[test]
fn melhoramento_uma_amostra_por_geracao_com_as_selecionadas() {
    let esp = Especificacao::nova_com(TipoDeTrabalho::Melhoramento, 100, 5, &[20, 42, 20, 50, 60, 100, 100, 30]).unwrap();
    let (_, amostras) = os_dois(&esp);
    assert_eq!(amostras.len(), 6, "a população inicial e 5 gerações");
    for a in &amostras {
        let Amostra::Melhoramento { pontos, plantas, .. } = a else { panic!() };
        assert_eq!(*plantas, 100);
        assert_eq!(pontos.iter().filter(|p| p.2).count(), 20, "20% selecionadas");
    }
}

#[test]
fn rotas_o_ultimo_comprimento_e_o_do_resultado() {
    let esp = Especificacao::nova_com(TipoDeTrabalho::Rotas, 60, 200, &[11]).unwrap();
    let (r, amostras) = os_dois(&esp);
    let d = rotas::Resultado::decodificar(&r, 60).unwrap();
    let Some(Amostra::Rotas { comprimento, inicial, rota, .. }) = amostras.last() else { panic!() };
    assert_eq!((*comprimento, *inicial), (d.comprimento, d.inicial));
    assert_eq!(rota.len(), 60);
    let Some(Amostra::Rotas { passada: 0, comprimento: primeiro, .. }) = amostras.first() else { panic!("a partida vem primeiro") };
    assert_eq!(*primeiro, d.inicial);
}

#[test]
fn triagem_cada_molecula_com_a_nota_do_resultado() {
    let esp = Especificacao::nova_com(TipoDeTrabalho::Triagem, 40, 0, &[0, 100, 500_000, 25_000, 5, 10, 10, 140_000, 10_000]).unwrap();
    let (r, amostras) = os_dois(&esp);
    let (inicio, _, linhas) = triagem::decodificar(&r).unwrap();
    assert_eq!(amostras.len(), 40);
    for (k, a) in amostras.iter().enumerate() {
        let Amostra::Triagem { indice, mascara, nota, previsto_mili, .. } = a else { panic!() };
        assert_eq!(*indice, inicio + k as u32);
        assert_eq!((*mascara, *previsto_mili, *nota), linhas[k]);
    }
}
