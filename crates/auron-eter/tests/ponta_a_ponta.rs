// ✝ Provérbios 25:25 — “Como água fresca para a alma sedenta, tais são as boas novas de terra distante.”
//! O objeto sai por meios diferentes e chega inteiro do outro lado.
//!
//! Em teste, falhar com pânico é o comportamento desejado: é o relatório.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use auron_eter::meios::{MeioMemoria, MeioPasta};
use auron_eter::{Meio, Recepcao, EterError, espalhar, pedaco_para_o_meio};

fn objeto(n: usize) -> Vec<u8> {
    (0..n).map(|i| u8::try_from(i % 251).unwrap_or(0)).collect()
}

fn recolher(meios: &mut [&mut dyn Meio]) -> (Option<Vec<u8>>, Vec<usize>) {
    let mut recepcao = Recepcao::nova();
    let mut pronto = None;
    let mut por_meio = Vec::new();
    // Duas voltas: na primeira pode chegar fragmento antes do manifesto.
    for _ in 0..2 {
        por_meio.clear();
        for meio in meios.iter_mut() {
            let quadros = meio.receber().unwrap_or_default();
            por_meio.push(quadros.len());
            for quadro in quadros {
                if let Ok(Some(obj)) = recepcao.quadro(&quadro) {
                    pronto = Some(obj);
                }
            }
        }
        if pronto.is_some() {
            break;
        }
    }
    (pronto, por_meio)
}

#[test]
fn dois_meios_carregam_metade_cada_e_o_objeto_chega_inteiro() {
    let dados = objeto(200_000);
    let (mut wifi_a, mut wifi_b) = MeioMemoria::par("wi-fi", 16 * 1024);
    let (mut bt_a, mut bt_b) = MeioMemoria::par("bluetooth", 16 * 1024);

    let manifesto = espalhar(&dados, "foto.jpg", &mut [&mut wifi_a, &mut bt_a]).unwrap();
    assert_eq!(manifesto.nome, "foto.jpg");

    let (recebido, por_meio) = recolher(&mut [&mut wifi_b, &mut bt_b]);
    assert_eq!(recebido.as_deref(), Some(dados.as_slice()));

    // Cada meio carregou parte: nenhum dos dois viu o objeto inteiro.
    let metade = manifesto.quantidade / 2;
    for quantos in &por_meio {
        assert!(*quantos > 0, "um dos meios não carregou nada");
        assert!(
            *quantos < usize::try_from(manifesto.quantidade).unwrap_or(usize::MAX),
            "um meio sozinho levou tudo"
        );
    }
    assert!(metade > 0);
}

#[test]
fn o_meio_pequeno_leva_so_o_aviso_e_nao_atrapalha() {
    let dados = objeto(120_000);
    let (mut wifi_a, mut wifi_b) = MeioMemoria::par("wi-fi", 16 * 1024);
    // Um quadro de rádio: não cabe fragmento nenhum, mas cabe o manifesto.
    let (mut radio_a, mut radio_b) = MeioMemoria::par("rádio", 400);

    let manifesto = espalhar(&dados, "bloco", &mut [&mut wifi_a, &mut radio_a]).unwrap();

    // No rádio veio exatamente uma coisa: o aviso.
    let no_radio = radio_b.receber().unwrap();
    assert_eq!(no_radio.len(), 1);
    let mut so_o_aviso = Recepcao::nova();
    assert!(so_o_aviso.quadro(&no_radio[0]).unwrap().is_none());
    assert_eq!(so_o_aviso.manifesto().map(|m| m.id), Some(manifesto.id));
    assert_eq!(
        so_o_aviso.faltando().len(),
        usize::try_from(manifesto.quantidade).unwrap_or(0),
        "quem só ouviu o rádio sabe o que existe e sabe que não tem nada ainda"
    );

    // O dado veio pelo Wi-Fi e fecha o objeto.
    let (recebido, _) = recolher(&mut [&mut wifi_b]);
    assert_eq!(recebido.as_deref(), Some(dados.as_slice()));
}

#[test]
fn sem_nenhum_meio_que_carregue_o_dado_o_erro_e_claro() {
    let dados = objeto(50_000);
    let (mut radio_a, _radio_b) = MeioMemoria::par("rádio", 400);
    assert!(matches!(
        espalhar(&dados, "", &mut [&mut radio_a]),
        Err(EterError::MeioPequenoDemais { .. })
    ));
}

#[test]
fn atravessa_o_tempo_numa_pasta_como_num_pendrive() {
    let agora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = std::env::temp_dir().join(format!("auron-eter-{agora}"));
    let pendrive = base.join("pendrive");

    let dados = objeto(90_000);
    {
        // Quem envia grava no pendrive e vai embora.
        let mut saida = MeioPasta::novo("pendrive", &pendrive, base.join("nada"), 16 * 1024).unwrap();
        espalhar(&dados, "foto.jpg", &mut [&mut saida]).unwrap();
    }

    // Outro programa, outra hora, lê o mesmo pendrive.
    let mut entrada = MeioPasta::novo("pendrive", base.join("nada2"), &pendrive, 16 * 1024).unwrap();
    let (recebido, _) = recolher(&mut [&mut entrada]);
    assert_eq!(recebido.as_deref(), Some(dados.as_slice()));

    // Segunda leitura não repete o que já foi lido.
    assert!(entrada.receber().unwrap().is_empty());
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn o_tamanho_do_pedaco_respeita_o_meio() {
    for mtu in [400usize, 1500, 16 * 1024] {
        if let Ok(pedaco) = pedaco_para_o_meio(mtu, 50_000) {
            assert!(usize::try_from(pedaco).unwrap_or(usize::MAX) < mtu);
        }
    }
}
