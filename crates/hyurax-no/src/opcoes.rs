//! As opções da linha de comando.

use std::path::PathBuf;

use hyurax_crypto::ADDRESS_LEN;
use hyurax_nucleo::carteira::endereco;
use hyurax_nucleo::config::{self, ConfigDoNo};
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::util::unidades_de_hyx;

/// As opções.
pub struct Opcoes {
    /// Rede, pastas, porta e sementes.
    pub no: ConfigDoNo,
    /// Arquivo da carteira.
    pub arquivo: Option<PathBuf>,
    /// Endereço de recompensa (ou de consulta).
    pub endereco: Option<[u8; ADDRESS_LEN]>,
    /// Blocos a minerar (0 = sem parar).
    pub blocos: u64,
    /// Linhas da mineração.
    pub linhas: u32,
    /// Pausa fixa entre tentativas.
    pub pausa_ms: u64,
    /// Destino de um envio.
    pub para: Option<[u8; ADDRESS_LEN]>,
    /// Valor de um envio.
    pub valor: Option<u64>,
    /// Taxa de um envio.
    pub taxa: u64,
    /// Arquivo do resumo público (explorador).
    pub exportar: Option<PathBuf>,
    /// Porta do painel.
    pub painel_porta: u16,
    /// Painel visível na rede local.
    pub painel_rede: bool,
    /// API externa ligada (contas de cliente mandam JOBs).
    pub api_externa: bool,
}

/// Lê as opções. Sem `--pasta`, usa as pastas do sistema (as mesmas do
/// programa com janela).
///
/// # Errors
/// Opção desconhecida ou valor inválido.
pub fn ler(args: &[String]) -> Result<Opcoes, String> {
    let nucleos = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut rede = config::REDE_PADRAO;
    let mut pasta: Option<PathBuf> = None;
    let mut porta = 0u16;
    let mut sementes = Vec::new();
    let mut sem_padrao = false;
    let mut o = Opcoes {
        no: ConfigDoNo::nova(rede, Pastas::unica("."), 0, Vec::new(), true),
        arquivo: None,
        endereco: None,
        blocos: 1,
        linhas: u32::try_from((nucleos / 2).max(1)).unwrap_or(1),
        pausa_ms: 0,
        para: None,
        valor: None,
        taxa: 0,
        exportar: None,
        painel_porta: config::PORTA_PAINEL,
        painel_rede: false,
        api_externa: false,
    };
    // endereços se leem no fim, quando já se sabe a rede: o prefixo depende dela
    let (mut endereco_texto, mut para_texto) = (None::<String>, None::<String>);
    let mut it = args.iter();
    while let Some(nome) = it.next() {
        match nome.as_str() {
            "--sem-sementes-padrao" => {
                sem_padrao = true;
                continue;
            }
            "--painel-rede" => {
                o.painel_rede = true;
                continue;
            }
            "--api-externa" => {
                o.api_externa = true;
                continue;
            }
            _ => {}
        }
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        let numero = |nome: &str| valor.parse::<u64>().map_err(|_| format!("{nome} precisa ser número"));
        match nome.as_str() {
            "--rede" => rede = config::rede_do_nome(valor)?,
            "--pasta" => pasta = Some(PathBuf::from(valor)),
            "--arquivo" => o.arquivo = Some(PathBuf::from(valor)),
            "--endereco" => endereco_texto = Some(valor.clone()),
            "--blocos" => o.blocos = numero("--blocos")?,
            "--porta" => porta = valor.parse().map_err(|_| "--porta precisa ser número")?,
            "--painel-porta" => o.painel_porta = valor.parse().map_err(|_| "--painel-porta precisa ser número")?,
            "--semente" => sementes.extend(valor.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
            "--pausa-ms" => o.pausa_ms = numero("--pausa-ms")?,
            "--para" => para_texto = Some(valor.clone()),
            "--valor" => o.valor = Some(unidades_de_hyx(valor)?),
            "--taxa" => o.taxa = unidades_de_hyx(valor)?,
            "--exportar" => o.exportar = Some(PathBuf::from(valor)),
            "--linhas" => o.linhas = valor.parse::<u32>().ok().filter(|&n| n >= 1).ok_or("--linhas precisa ser pelo menos 1")?,
            _ => return Err(format!("opção desconhecida: {nome}")),
        }
    }
    let pastas = match pasta {
        Some(p) => Pastas::unica(p),
        None => Pastas::do_sistema()?,
    };
    o.no = ConfigDoNo::nova(rede, pastas, porta, sementes, sem_padrao);
    if let Some(t) = endereco_texto {
        o.endereco = Some(endereco::ler(&t, rede.nome).map_err(|e| format!("--endereco: {e}"))?);
    }
    if let Some(t) = para_texto {
        o.para = Some(endereco::ler(&t, rede.nome).map_err(|e| format!("--para: {e}"))?);
    }
    Ok(o)
}
