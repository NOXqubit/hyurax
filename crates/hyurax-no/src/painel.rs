//! `hyurax-no painel`: o núcleo inteiro com a interface no navegador.

use std::sync::atomic::Ordering;
use std::time::Duration;

use hyurax_nucleo::carteira::arquivo;
use hyurax_nucleo::servico::{Modo, Nucleo, Partida};

use crate::opcoes;

/// `hyurax-no painel`
pub fn comando(args: &[String]) -> Result<(), String> {
    let o = opcoes::ler(args)?;
    let endereco = match (o.endereco, &o.arquivo) {
        (Some(e), _) => Some(e),
        (None, Some(a)) => Some(arquivo::endereco(&hyurax_nucleo::arquivos::ler(a)?)?),
        (None, None) => None,
    };
    let n = Nucleo::ligar(Partida {
        modo: Modo::Terminal,
        config: o.no.clone(),
        arquivo_carteira: o.arquivo.clone(),
        endereco_fixo: endereco,
        linhas_padrao: o.linhas,
        pausa_fixa_ms: o.pausa_ms,
        painel_na_rede: o.painel_rede,
    })?;
    if o.api_externa {
        n.api_externa.store(true, Ordering::Relaxed);
    }
    let porta = hyurax_nucleo::api::abrir(&n, o.painel_porta, hyurax_interface::ARQUIVOS)?;
    // a chave da sessão vai no endereço (depois do #): sem ela o painel só
    // mostra o aviso para abrir por aqui
    println!("Painel: {}", hyurax_nucleo::api::endereco_da_janela(porta, &n.chave_painel));
    if n.na_rede.load(Ordering::Relaxed) {
        println!("  (visível na rede local; comandos só deste computador)");
    }
    if o.api_externa {
        println!("  API externa ligada em /api/v1/externa/ (contas: hyurax-no contas)");
    }
    if endereco.is_none() {
        println!("  sem --arquivo nem --endereco: nada de mineração (não há para onde mandar a recompensa)");
    }
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
