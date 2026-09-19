//! Os três meios juntos: um objeto sai ao mesmo tempo por Wi-Fi (UDP de
//! verdade), som (WAV modulado de verdade) e um fio serial (no lugar do
//! Bluetooth, um soquete local), e chega inteiro do outro lado.
//!
//! Em teste, falhar com pânico é o comportamento desejado: é o relatório.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use hyurax_eter::serial::MeioSerial;
use hyurax_eter::som::{MeioSom, Perfil};
use hyurax_eter::wifi::MeioWifi;
use hyurax_eter::{Meio, Recepcao, espalhar};

fn objeto(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 131 % 251) as u8).collect()
}

fn pasta(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("eter-teste-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// Par de Wi-Fi no mesmo computador: cada lado envia direto para a porta do outro.
fn par_wifi() -> (MeioWifi, MeioWifi) {
    let b = MeioWifi::abrir("wi-fi", 0, SocketAddr::from((Ipv4Addr::LOCALHOST, 9))).unwrap();
    let porta_b = b.porta_local().unwrap();
    let a = MeioWifi::abrir("wi-fi", 0, SocketAddr::from((Ipv4Addr::LOCALHOST, porta_b))).unwrap();
    (a, b)
}

/// Par serial sobre um soquete local: no lugar do Bluetooth, o mesmo fio de bytes.
fn par_serial(vazao: u64) -> (MeioSerial, MeioSerial) {
    let ouvinte = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let endereco = ouvinte.local_addr().unwrap();
    let a = TcpStream::connect(endereco).unwrap();
    let (b, _) = ouvinte.accept().unwrap();
    let meio_a = MeioSerial::de_fluxo("bluetooth", a.try_clone().unwrap(), a, 1400, vazao);
    let meio_b = MeioSerial::de_fluxo("bluetooth", b.try_clone().unwrap(), b, 1400, vazao);
    (meio_a, meio_b)
}

/// Recolhe dos meios até o objeto fechar; devolve também quantos quadros cada um trouxe.
fn recolher(meios: &mut [&mut dyn Meio]) -> (Option<Vec<u8>>, Vec<usize>) {
    let mut contagem = vec![0usize; meios.len()];
    let mut guardados: Vec<Vec<u8>> = Vec::new();
    for _ in 0..200 {
        for (i, meio) in meios.iter_mut().enumerate() {
            let quadros = meio.receber().unwrap();
            contagem[i] += quadros.len();
            guardados.extend(quadros);
        }
        // Manifesto pode chegar depois de fragmentos: repassa tudo a cada volta.
        let mut recepcao = Recepcao::nova();
        for q in &guardados {
            if let Ok(Some(obj)) = recepcao.quadro(q) {
                return (Some(obj), contagem);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    (None, contagem)
}

#[test]
fn um_objeto_pelos_tres_meios_ao_mesmo_tempo() {
    let dados = objeto(6_000);
    let (mut wifi_a, mut wifi_b) = par_wifi();
    // Serial "lento" para o rodízio por velocidade dar trabalho a todos.
    let (mut bt_a, mut bt_b) = par_serial(8_000);
    let som_pasta = pasta("som");
    let mut som_a = MeioSom::novo("som", Perfil::FM, &som_pasta, pasta("som-in"), 1400).unwrap();
    // Wi-Fi lento de propósito não dá para configurar; o som é o lento de verdade.

    let manifesto = espalhar(&dados, "mensagem.txt", &mut [&mut wifi_a, &mut bt_a, &mut som_a]).unwrap();

    // O áudio gravado é "tocado" para o outro lado: o arquivo vai para a pasta de entrada.
    let wav = som_a.ultimo_wav().unwrap().to_path_buf();
    let entrada = pasta("som-rx");
    let mut som_b = MeioSom::novo("som", Perfil::FM, pasta("som-rx-out"), &entrada, 1400).unwrap();
    std::fs::copy(&wav, entrada.join("gravado.wav")).unwrap();

    let (recebido, contagem) = recolher(&mut [&mut wifi_b, &mut bt_b, &mut som_b]);
    assert_eq!(recebido.as_deref(), Some(dados.as_slice()), "chegaram {contagem:?}");
    // Os três trouxeram alguma coisa (no mínimo o manifesto).
    assert!(contagem.iter().all(|c| *c > 0), "algum meio não trouxe nada: {contagem:?}");
    assert_eq!(manifesto.tamanho, 6_000);
}

#[test]
fn a_divisao_segue_a_velocidade_de_cada_meio() {
    use hyurax_eter::meios::MeioMemoria;

    /// Um meio de memória com a vazão que o teste quiser.
    struct ComVazao(MeioMemoria, u64);
    impl Meio for ComVazao {
        fn nome(&self) -> &str {
            self.0.nome()
        }
        fn mtu(&self) -> usize {
            self.0.mtu()
        }
        fn vazao(&self) -> u64 {
            self.1
        }
        fn enviar(&mut self, q: &[u8]) -> Result<(), hyurax_eter::EterError> {
            self.0.enviar(q)
        }
        fn receber(&mut self) -> Result<Vec<Vec<u8>>, hyurax_eter::EterError> {
            self.0.receber()
        }
    }

    let (rapido_a, mut rapido_b) = MeioMemoria::par("rápido", 2000);
    let (lento_a, mut lento_b) = MeioMemoria::par("lento", 2000);
    let mut rapido = ComVazao(rapido_a, 900_000);
    let mut lento = ComVazao(lento_a, 100_000);
    let dados = objeto(200_000);
    espalhar(&dados, "x", &mut [&mut rapido, &mut lento]).unwrap();
    let r = rapido_b.receber().unwrap().len() as f64;
    let l = lento_b.receber().unwrap().len() as f64;
    // 9 para 1, com folga para o manifesto que vai pelos dois.
    let razao = r / l;
    assert!((8.0..10.5).contains(&razao), "rápido {r}, lento {l}");
}

#[test]
fn se_um_meio_cai_o_que_falta_vem_pelos_outros() {
    let dados = objeto(20_000);
    let (mut wifi_a, mut wifi_b) = par_wifi();
    let (mut bt_a, mut bt_b) = par_serial(1_000_000);

    espalhar(&dados, "foto", &mut [&mut wifi_a, &mut bt_a]).unwrap();
    thread::sleep(Duration::from_millis(100));

    // O Wi-Fi "caiu": tudo o que veio por ele se perdeu. Só o Bluetooth chegou.
    let _perdido = wifi_b.receber().unwrap();
    let mut recepcao = Recepcao::nova();
    for q in bt_b.receber().unwrap() {
        assert!(recepcao.quadro(&q).unwrap().is_none());
    }
    let faltando = recepcao.faltando();
    assert!(!faltando.is_empty());

    // Quem recebeu pede o que falta; quem enviou manda de novo só aquilo, pelo meio que sobrou.
    let (_, fragmentos) = hyurax_eter::fatiar(&dados, "foto", recepcao.manifesto().unwrap().pedaco).unwrap();
    for i in &faltando {
        let q = hyurax_eter::Quadro::Fragmento(fragmentos[*i as usize].clone()).codificar().unwrap();
        bt_a.enviar(&q).unwrap();
    }
    thread::sleep(Duration::from_millis(100));
    let mut fechou = None;
    for q in bt_b.receber().unwrap() {
        if let Some(obj) = recepcao.quadro(&q).unwrap() {
            fechou = Some(obj);
        }
    }
    assert_eq!(fechou.as_deref(), Some(dados.as_slice()));
}
