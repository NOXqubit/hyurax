//! A cadeia no disco e o nó na rede: abrir, subir, sincronizar e gravar.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use hyurax_chain::Chain;
use hyurax_consensus::ParametrosRede;
use hyurax_net::{No, Rede};
use hyurax_store::{encode_chain, gravar_codificada, load_chain};

use crate::config::ConfigDoNo;
use crate::util::{agora_unix, hex, hyx};
use crate::{identidade, sementes};

/// Para onde vão as mensagens do nó: o terminal imprime, a janela manda para
/// o barramento de eventos. O programa com janela não pode escrever no
/// console (sem leitor, a escrita trava a linha que a fez).
pub type Saida = Arc<dyn Fn(String) + Send + Sync>;

/// Saída que imprime no terminal.
pub fn saida_do_terminal() -> Saida {
    Arc::new(|t| println!("{t}"))
}

/// O arquivo da cadeia desta rede.
pub fn arquivo(config: &ConfigDoNo) -> PathBuf {
    config.pastas.dados.join(format!("{}.cadeia", config.rede.nome))
}

/// Abre a cadeia do disco, ou começa da gênese.
///
/// # Errors
/// Arquivo corrompido ou de outra rede.
pub fn abrir(rede: ParametrosRede, arquivo: &Path) -> Result<Chain, String> {
    if arquivo.exists() {
        // o próprio disco: pula só o Argon2id, e revalida todo o resto
        load_chain(arquivo, true, Some(rede)).map_err(|e| e.to_string())
    } else {
        Chain::nova(rede).map_err(|e| e.to_string())
    }
}

/// Sobe o nó: abre a cadeia, escuta (com porta) e conecta às sementes.
///
/// # Errors
/// Pasta sem permissão, cadeia corrompida ou porta ocupada.
pub fn subir(config: &ConfigDoNo, saida: &Saida) -> Result<Arc<Rede>, String> {
    config.pastas.criar()?;
    let cadeia = abrir(config.rede, &arquivo(config))?;
    let identidade = identidade::na_pasta(&config.pastas.config)?;
    saida(format!("identidade do nó: {}", hex(&identidade.publica())));
    let rede = Rede::com_identidade(No::novo(cadeia), identidade);
    if config.porta > 0 {
        let porta = rede
            .escutar(("0.0.0.0", config.porta))
            .map_err(|e| format!("não consegui escutar na porta {}: {e}", config.porta))?;
        saida(format!("escutando outros nós na porta {porta}"));
    }
    for semente in config.sementes.iter().filter(|s| !s.is_empty()) {
        rede.semear(semente);
        match rede.conectar(semente.as_str()) {
            Ok(()) => saida(format!("conectando em {semente}")),
            Err(e) => saida(format!("não consegui conectar em {semente}: {e}")),
        }
    }
    procurar_a_rede(config, &rede, saida);
    Ok(rede)
}

/// Sem semente nenhuma, procura a lista publicada. Só na testnet, e numa
/// linha à parte: o programa abre na hora, e a rede aparece quando aparecer.
fn procurar_a_rede(config: &ConfigDoNo, rede: &Arc<Rede>, saida: &Saida) {
    if !config.sementes.is_empty() || config.sem_sementes_padrao || config.rede.nome != ParametrosRede::TESTNET.nome {
        return;
    }
    let (rede, saida) = (Arc::clone(rede), Arc::clone(saida));
    std::thread::spawn(move || {
        saida(format!("sem semente configurada: lendo a lista publicada em {}", sementes::url_publicada()));
        match sementes::publicadas() {
            Ok(lista) if lista.is_empty() => {
                saida("a lista publicada ainda não tem nenhum nó; o nó fica sozinho até alguém se conectar".into());
            }
            Ok(lista) => {
                for semente in lista {
                    rede.semear(&semente);
                    match rede.conectar(semente.as_str()) {
                        Ok(()) => saida(format!("conectando em {semente} (lista publicada)")),
                        Err(e) => saida(format!("não consegui conectar em {semente}: {e}")),
                    }
                }
            }
            Err(e) => saida(format!("não consegui ler a lista publicada: {e}")),
        }
    });
}

/// Grava a cadeia. Os bytes são montados com a trava do nó, que é rápido; o
/// disco fica de fora dela. Uma gravação de cada vez, e a vez é pega antes
/// de montar os bytes: senão a cadeia mais velha podia chegar ao disco por
/// último. Ordem das travas: esta primeiro, a do nó depois.
///
/// # Errors
/// Disco cheio ou sem permissão.
pub fn salvar(rede: &Rede, config: &ConfigDoNo) -> Result<(), String> {
    static GRAVANDO: Mutex<()> = Mutex::new(());
    let _vez = GRAVANDO.lock();
    let dados = {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        encode_chain(&no.chain).map_err(|e| e.to_string())?
    };
    gravar_codificada(&dados, &arquivo(config)).map_err(|e| e.to_string())
}

/// Espera alcançar o trabalho que os pares anunciaram (no máximo `limite`).
/// Devolve `false` se não alcançou.
pub fn esperar_sincronizar(rede: &Rede, limite: Duration) -> bool {
    let inicio = Instant::now();
    while inicio.elapsed() < limite {
        if rede.alcancou_os_pares() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

/// Resumo público da cadeia em JSON, para um explorador de blocos estático.
/// Só dados que já são públicos: altura, hashes e contagens.
///
/// # Errors
/// Disco cheio ou sem permissão.
pub fn exportar_resumo(rede: &Rede, config: &ConfigDoNo, destino: &Path) -> Result<(), String> {
    let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
    let c = &no.chain;
    let blocos: Vec<serde_json::Value> = c
        .entries
        .iter()
        .rev()
        .take(20)
        .map(|e| {
            let h = &e.block.header;
            serde_json::json!({
                "altura": h.height,
                "hash": hex(&e.block.block_hash()),
                "anterior": hex(&h.prev_hash),
                "horario": h.timestamp,
                "bits": format!("{:#010x}", h.bits),
                "transacoes": e.block.transactions.len(),
                "trabalho_util_n": e.block.useful_proof.as_ref().map_or(0, |p| p.n),
            })
        })
        .collect();
    let j = serde_json::json!({
        "formato": "hyurax-explorador-v1",
        "rede": config.rede.nome,
        "altura": c.height(),
        "ponta": hex(&c.tip_hash()),
        "trabalho": c.total_work().to_decimal(),
        "emitido": hyx(u128::from(c.state.total_emitted)),
        "pares": rede.pares_conectados(),
        "mempool": no.mempool_len(),
        "atualizado": agora_unix(),
        "blocos": blocos,
    });
    drop(no);
    crate::arquivos::gravar_atomico(destino, format!("{j}\n").as_bytes())
}
