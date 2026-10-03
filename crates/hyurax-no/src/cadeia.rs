//! `hyurax-no minerar|enviar|no|estado`: o nó e a cadeia pelo terminal.

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, Instant};

use hyurax_block::Block;
use hyurax_consensus::block_reward;
use hyurax_nucleo::cadeia::{self, saida_do_terminal};
use hyurax_nucleo::carteira::{arquivo, endereco, senha};
use hyurax_nucleo::util::{hex, hyx};
use hyurax_pow::ConfigMineracao;
use hyurax_tx::{HYX, Output, sign_transfer_outputs};

use crate::opcoes;

/// `hyurax-no minerar`
pub fn minerar(args: &[String]) -> Result<(), String> {
    let o = opcoes::ler(args)?;
    let destino = o.endereco.ok_or("falta --endereco (crie com: hyurax-no carteira nova --arquivo carteira.txt)")?;
    let _trava = o.no.pastas.travar()?;
    let rede = cadeia::subir(&o.no, &saida_do_terminal())?;
    // minerar antes de alcançar a rede é minerar num ramo que vai ser jogado fora
    if !o.no.sementes.is_empty() {
        println!("Sincronizando com a rede antes de minerar…");
        if !cadeia::esperar_sincronizar(&rede, Duration::from_secs(90)) {
            println!("  aviso: não alcancei a rede em 90 s; seguindo com a cadeia que tenho");
        }
    }
    {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        println!(
            "Rede {} · altura {} · {} linha(s), {:.0} MiB",
            o.no.rede.nome,
            no.chain.height(),
            o.linhas,
            (u64::from(o.no.rede.pow.memoria_kib) * u64::from(o.linhas)) as f64 / 1024.0
        );
    }
    let (mut feitos, mut perdidos) = (0u64, 0u64);
    while o.blocos == 0 || feitos < o.blocos {
        let inicio = Instant::now();
        // monta o candidato com a trava do nó, e minera SEM ela
        let candidato = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            no.chain.build_candidate(destino, no.mempool_ordenado(), None, Vec::new()).map_err(|e| e.to_string())?
        };
        let cfg = ConfigMineracao { linhas: o.linhas, nonce_inicial: 0, limite: Some(1u64 << 32), pausa: Duration::from_millis(o.pausa_ms) };
        let achado = hyurax_pow::minerar(&candidato.header.encode(), o.no.rede.pow, cfg, &AtomicBool::new(false), &AtomicU64::new(0))
            .map_err(|e| e.to_string())?
            .achado
            .ok_or("não achei nonce no limite de tentativas")?;
        let altura = candidato.header.height;
        let n = candidato.useful_proof.as_ref().map_or(0, |p| p.n);
        let bits = candidato.header.bits;
        let bloco = Block { header: candidato.header.with_nonce(achado.nonce), ..candidato };
        match rede.submeter_bloco(bloco) {
            Ok(true) => {
                cadeia::salvar(&rede, &o.no)?;
                feitos = feitos.saturating_add(1);
                println!(
                    "  bloco {altura} em {:.1} s · prova útil {n}×{n} · bits {bits:#010x} · recompensa {} HYX (libera em {} blocos) · {} par(es)",
                    inicio.elapsed().as_secs_f64(),
                    hyx(u128::from(block_reward(altura, &o.no.rede))),
                    o.no.rede.coinbase_maturity,
                    rede.pares_conectados()
                );
            }
            Ok(false) | Err(_) => {
                perdidos = perdidos.saturating_add(1);
                println!("  bloco {altura} perdido na corrida (outro nó chegou primeiro); recomeçando");
            }
        }
    }
    if perdidos > 0 {
        println!("{feitos} bloco(s) meus, {perdidos} perdido(s) na corrida.");
    }
    cadeia::salvar(&rede, &o.no)
}

/// `hyurax-no enviar`
pub fn enviar(args: &[String]) -> Result<(), String> {
    let o = opcoes::ler(args)?;
    let arq = o.arquivo.clone().ok_or("falta --arquivo (a carteira que paga)")?;
    let para = o.para.ok_or("falta --para (endereço de destino, como thyx1…)")?;
    let valor = o.valor.filter(|&v| v > 0).ok_or("falta --valor maior que zero (em HYX, por exemplo 1.5)")?;
    let texto = hyurax_nucleo::arquivos::ler(&arq)?;
    let origem = arquivo::endereco(&texto)?;
    if origem == para {
        return Err("origem e destino são o mesmo endereço".into());
    }
    let _trava = o.no.pastas.travar()?;
    let rede = cadeia::subir(&o.no, &saida_do_terminal())?;
    println!("Sincronizando com a rede…");
    if !o.no.sementes.is_empty() && !cadeia::esperar_sincronizar(&rede, Duration::from_secs(90)) {
        println!("  aviso: não alcancei a rede em 90 s; seguindo com a cadeia que tenho");
    }
    cadeia::salvar(&rede, &o.no)?;
    // os pares mandam o mempool logo depois do aperto de mão: esperar um pouco
    // evita escolher um nonce que já está numa transação pendente minha
    if rede.pares_conectados() > 0 {
        std::thread::sleep(Duration::from_millis(1500));
    }
    let (nonce, saldo, altura) = {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        (no.proximo_nonce(&origem), no.chain.state.balance(&origem, &HYX), no.chain.height())
    };
    let total = valor.checked_add(o.taxa).ok_or("valor mais taxa estoura")?;
    if u128::from(total) > u128::from(saldo) {
        return Err(format!(
            "saldo gastável insuficiente na altura {altura}: {} HYX, precisa de {} HYX (a recompensa de mineração só libera depois de {} blocos)",
            hyx(u128::from(saldo)),
            hyx(u128::from(total)),
            o.no.rede.coinbase_maturity
        ));
    }
    if arquivo::e_formato_antigo(&texto) {
        return Err("esta carteira guarda o segredo sem senha: proteja antes com `hyurax-no carteira cifrar --arquivo ...`".into());
    }
    let s = senha::ler("Senha da carteira")?;
    let segredo = arquivo::abrir(&texto, &s)?;
    let saida = Output { recipient: para, asset_id: HYX, amount: valor };
    let tx = sign_transfer_outputs(&segredo, &o.no.rede.magic, origem, vec![saida], o.taxa, nonce).map_err(|e| e.to_string())?;
    let id = tx.txid().map_err(|e| e.to_string())?;
    match rede.submeter_tx(tx) {
        Ok(true) => {}
        Ok(false) => return Err("já existe uma transação com esse nonce esperando no mempool".into()),
        Err(e) => return Err(format!("a própria validação recusou: {}", e.0)),
    }
    println!(
        "Transação {} assinada: {} HYX para {} (taxa {} HYX, nonce {nonce}).",
        hex(&id),
        hyx(u128::from(valor)),
        endereco::mostrar(&para, o.no.rede.nome),
        hyx(u128::from(o.taxa))
    );
    if let Some(arquivo_do_pacote) = &o.pacote {
        // sem internet: a transação vai num arquivo que atravessa pendrive,
        // Bluetooth ou som (eter enviar) e entra na rede pelo primeiro nó
        // conectado que importar o pacote
        let pacote = rede.exportar_pacote(0)?;
        std::fs::write(arquivo_do_pacote, pacote).map_err(|e| format!("não consegui gravar {}: {e}", arquivo_do_pacote.display()))?;
        println!("Pacote do Éter gravado em {} (leve até um nó conectado: pasta eter/entrada, ou hyurax-no pacote importar).", arquivo_do_pacote.display());
    }
    if rede.pares_conectados() == 0 {
        println!("Aviso: nenhum par conectado. A transação só existe neste nó; use --semente, ou --pacote para levá-la por outro meio.");
        return Ok(());
    }
    std::thread::sleep(Duration::from_secs(3));
    println!("Enviada para {} par(es). Ela entra num bloco quando algum minerador a incluir.", rede.pares_conectados());
    Ok(())
}

/// `hyurax-no no`: só o nó, sem minerar.
pub fn servir(args: &[String]) -> Result<(), String> {
    let o = opcoes::ler(args)?;
    let _trava = o.no.pastas.travar()?;
    let rede = cadeia::subir(&o.no, &saida_do_terminal())?;
    println!("Nó no ar. Ctrl+C para parar. A cadeia é gravada a cada mudança.");
    let mut ultima = (u64::MAX, usize::MAX);
    let mut ultimo_export = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let (altura, pares, mempool) = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            (no.chain.height(), rede.pares_conectados(), no.mempool_len())
        };
        let mudou = (altura, pares) != ultima;
        if altura != ultima.0 {
            cadeia::salvar(&rede, &o.no)?;
            println!("  altura {altura} · {pares} par(es) · {mempool} no mempool");
        }
        if let Some(destino) = &o.exportar
            && (mudou || ultimo_export.elapsed() > Duration::from_secs(60))
        {
            if let Err(e) = cadeia::exportar_resumo(&rede, &o.no, destino) {
                println!("  aviso: não consegui exportar {}: {e}", destino.display());
            }
            ultimo_export = Instant::now();
        }
        ultima = (altura, pares);
    }
}

/// `hyurax-no estado`
pub fn estado(args: &[String]) -> Result<(), String> {
    let o = opcoes::ler(args)?;
    let c = cadeia::abrir(o.no.rede, &cadeia::arquivo(&o.no))?;
    println!("Rede:              {} (teste)", o.no.rede.nome);
    println!("Altura:            {}", c.height());
    println!("Ponta:             {}", hex(&c.tip_hash()));
    println!("Trabalho total:    {}", c.total_work().to_decimal());
    println!("Emitido:           {} HYX", hyx(u128::from(c.state.total_emitted)));
    if let Some(e) = o.endereco {
        println!("Saldo gastável:    {} HYX", hyx(u128::from(c.state.balance(&e, &HYX))));
        println!("Esperando liberar: {} HYX", hyx(c.state.immature_balance(&e)));
    }
    Ok(())
}
