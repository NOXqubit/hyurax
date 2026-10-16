//! `hyurax-no agente`: este computador como fornecedor da plataforma
//! HYURAX Compute, sem janela (servidor, máquina de laboratório, Linux).
//!
//! ```text
//! hyurax-no agente --plataforma https://compute.exemplo --codigo HYX-ABCD-EFGH
//!                  [--linhas 4] [--memoria 4096] [--gpu] [--nome "Servidor 1"] [--pasta ...]
//! ```
//!
//! Na primeira vez o código de vínculo (gerado no painel da plataforma)
//! registra o computador; depois disso basta `hyurax-no agente`: o vínculo
//! fica em `plataforma.txt`, na pasta de configuração.

use std::time::Duration;

use hyurax_nucleo::servico::{Modo, Nucleo, Partida};

use crate::opcoes;

/// `hyurax-no agente`
pub fn comando(args: &[String]) -> Result<(), String> {
    let mut url = None;
    let mut codigo = None;
    let mut memoria: Option<u64> = None;
    let mut gpu = false;
    let mut nome = None;
    let mut resto = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--plataforma" => url = Some(it.next().ok_or("--plataforma precisa do endereço")?.clone()),
            "--codigo" => codigo = Some(it.next().ok_or("--codigo precisa do código de vínculo")?.clone()),
            "--memoria" => memoria = Some(it.next().ok_or("--memoria precisa de MiB")?.parse().map_err(|_| "--memoria precisa ser número (MiB)")?),
            "--nome" => nome = Some(it.next().ok_or("--nome precisa de um nome")?.clone()),
            "--gpu" => gpu = true,
            _ => resto.push(a.clone()),
        }
    }
    let o = opcoes::ler(&resto)?;
    let n = Nucleo::ligar(Partida {
        modo: Modo::Terminal,
        config: o.no.clone(),
        arquivo_carteira: None,
        endereco_fixo: None,
        linhas_padrao: o.linhas,
        pausa_fixa_ms: 0,
        painel_na_rede: false,
    })?;
    let agente = &n.plataforma;
    let antes = agente.config();
    let linhas = o.linhas;
    let ram = memoria.unwrap_or(antes.ram_mib.max(1024));
    match (antes.no, codigo) {
        (_, Some(c)) => {
            let url = url.or(if antes.url.is_empty() { None } else { Some(antes.url.clone()) }).ok_or("informe --plataforma https://...")?;
            let nome = nome.unwrap_or_else(|| std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "Servidor".into()));
            let v = agente.vincular(&url, &c, &nome, linhas, ram, gpu)?;
            println!("Computador vinculado à plataforma {url} como nó {}.", v.get("no").and_then(serde_json::Value::as_u64).unwrap_or(0));
        }
        (Some(no), None) => {
            agente.ajustar(linhas, ram, gpu);
            agente.ligar_fornecimento(true);
            println!("Nó {no} da plataforma {}: fornecendo {linhas} núcleo(s) e {ram} MiB.", antes.url);
        }
        (None, None) => return Err("este computador ainda não está vinculado: rode com --plataforma https://... --codigo HYX-... (gere o código no painel)".into()),
    }
    println!("Só os motores do programa rodam aqui; o cliente manda parâmetros, nunca código. Ctrl+C para parar.");
    let mut ultimo = String::new();
    loop {
        std::thread::sleep(Duration::from_secs(10));
        let j = agente.json();
        let campo = |k: &str| j.get(k).cloned().unwrap_or(serde_json::Value::Null);
        let estado = if campo("conectado").as_bool().unwrap_or(false) { campo("estado").as_str().unwrap_or("conectado").to_string() } else { "sem conexão".to_string() };
        let erro = campo("ultimo_erro").as_str().filter(|e| !e.is_empty()).map(|e| format!(" · {e}")).unwrap_or_default();
        let linha = format!(
            "  {estado} · {} rodando, {} na fila · {} entregue(s), {} aceita(s){erro}",
            campo("unidades_rodando"),
            campo("unidades_na_fila"),
            campo("enviados"),
            campo("aceitos"),
        );
        if linha != ultimo {
            println!("{linha}");
            ultimo = linha;
        }
    }
}
