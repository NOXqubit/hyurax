//! `eter`: manda e recebe arquivos pelo Éter, por vários meios ao mesmo tempo.
//!
//! ```text
//! eter enviar  foto.jpg  --wifi --som saida/ --bluetooth COM5
//! eter receber recebidos/ --wifi --som entrada/ --bluetooth COM5
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use hyurax_eter::serial::{self, MeioSerial};
use hyurax_eter::som::{MeioSom, Perfil};
use hyurax_eter::wifi::{self, MeioWifi};
use hyurax_eter::{Meio, Quadro, Recepcao, espalhar};

const AJUDA: &str = "\
eter: o Éter do Hyurax. Um arquivo sai por vários meios ao mesmo tempo e
chega inteiro, conferido pelo SHA-512.

  eter enviar  <arquivo>        [meios]
  eter receber <pasta-destino>  [meios] [--segundos N]

Meios (use um ou mais):
  --wifi                 difusão UDP na rede local (porta 8791)
  --porta N              outra porta para o Wi-Fi
  --som <pasta>          enviar: grava o WAV nesta pasta, para tocar
                         receber: lê os WAV gravados que aparecerem nesta pasta
  --perfil fm|ar         modem de som: fm (1200 bit/s, padrão) ou ar (300 bit/s)
  --bluetooth <porta>    porta serial do Bluetooth: COM5, /dev/rfcomm0
  --pasta <pasta>        pasta ou pendrive (atravessa o tempo)

Exemplo pelo ar, sem rede nenhuma:
  eter enviar mensagem.txt --som saida --perfil ar
  (toque saida/eter-*.wav no alto-falante; grave no outro aparelho)
  eter receber recebidos --som gravacoes --perfil ar
";

struct Opcoes {
    alvo: PathBuf,
    wifi: bool,
    porta: u16,
    som: Option<PathBuf>,
    perfil: Perfil,
    bluetooth: Option<String>,
    pasta: Option<PathBuf>,
    segundos: u64,
}

fn ler_opcoes(args: &[String]) -> Result<Opcoes, String> {
    let mut it = args.iter();
    let alvo = it.next().ok_or("falta o arquivo ou a pasta")?;
    let mut o = Opcoes {
        alvo: PathBuf::from(alvo),
        wifi: false,
        porta: wifi::PORTA_PADRAO,
        som: None,
        perfil: Perfil::FM,
        bluetooth: None,
        pasta: None,
        segundos: 600,
    };
    while let Some(a) = it.next() {
        let mut valor = || it.next().cloned().ok_or(format!("{a} precisa de um valor"));
        match a.as_str() {
            "--wifi" => o.wifi = true,
            "--porta" => o.porta = valor()?.parse().map_err(|_| "porta inválida")?,
            "--som" => o.som = Some(PathBuf::from(valor()?)),
            "--perfil" => {
                let v = valor()?;
                o.perfil = Perfil::pelo_nome(&v).ok_or(format!("perfil {v} não existe: use fm ou ar"))?;
            }
            "--bluetooth" => o.bluetooth = Some(valor()?),
            "--pasta" => o.pasta = Some(PathBuf::from(valor()?)),
            "--segundos" => o.segundos = valor()?.parse().map_err(|_| "segundos inválido")?,
            outro => return Err(format!("não conheço {outro}")),
        }
    }
    if !o.wifi && o.som.is_none() && o.bluetooth.is_none() && o.pasta.is_none() {
        return Err("escolha pelo menos um meio: --wifi, --som, --bluetooth ou --pasta".into());
    }
    Ok(o)
}

/// Abre os meios pedidos. No envio, a pasta do som é a de saída; no recebimento, a de entrada.
fn abrir_meios(o: &Opcoes, enviando: bool) -> Result<Vec<Box<dyn Meio>>, String> {
    let mut meios: Vec<Box<dyn Meio>> = Vec::new();
    if o.wifi {
        meios.push(Box::new(MeioWifi::difusao(o.porta).map_err(|e| format!("wi-fi: {e}"))?));
    }
    if let Some(p) = &o.som {
        let rascunho = std::env::temp_dir().join("eter-som");
        let (saida, entrada) = if enviando { (p.clone(), rascunho) } else { (rascunho, p.clone()) };
        let nome = if o.perfil == Perfil::AR { "som (ar)" } else { "som (fm)" };
        meios.push(Box::new(
            MeioSom::novo(nome, o.perfil, saida, entrada, 1400).map_err(|e| format!("som: {e}"))?,
        ));
    }
    if let Some(porta) = &o.bluetooth {
        meios.push(Box::new(
            MeioSerial::porta("bluetooth", porta, serial::MTU_PADRAO).map_err(|e| format!("bluetooth: {e}"))?,
        ));
    }
    if let Some(p) = &o.pasta {
        meios.push(Box::new(
            hyurax_eter::meios::MeioPasta::novo("pasta", p, p, 64 * 1024).map_err(|e| format!("pasta: {e}"))?,
        ));
    }
    Ok(meios)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn enviar(o: &Opcoes) -> Result<(), String> {
    let dados = fs::read(&o.alvo).map_err(|e| format!("{}: {e}", o.alvo.display()))?;
    let nome = o
        .alvo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "objeto".into());
    let mut meios = abrir_meios(o, true)?;
    let mut refs: Vec<&mut dyn Meio> = meios.iter_mut().map(|m| m.as_mut() as &mut dyn Meio).collect();
    let manifesto = espalhar(&dados, &nome, &mut refs).map_err(|e| e.to_string())?;
    println!("Enviado: {nome}, {} bytes em {} pedaços", manifesto.tamanho, manifesto.quantidade);
    println!("SHA-512: {}…", hex(manifesto.id.get(..16).unwrap_or_default()));
    for m in &meios {
        println!("  {}: {} bytes/s", m.nome(), m.vazao());
    }
    if let Some(p) = &o.som {
        println!("Áudio pronto em {}: toque no alto-falante ou no transmissor FM.", p.display());
    }
    Ok(())
}

fn nome_seguro(nome: &str) -> String {
    let limpo: String = Path::new(nome)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        .collect();
    if limpo.is_empty() || limpo == "." || limpo == ".." {
        "objeto-recebido".into()
    } else {
        limpo
    }
}

fn receber(o: &Opcoes) -> Result<(), String> {
    fs::create_dir_all(&o.alvo).map_err(|e| format!("{}: {e}", o.alvo.display()))?;
    let mut meios = abrir_meios(o, false)?;
    let nomes: Vec<String> = meios.iter().map(|m| m.nome().to_owned()).collect();
    println!("Escutando: {}. Ctrl+C para parar.", nomes.join(", "));
    let mut recepcao = Recepcao::nova();
    // Fragmentos que chegam antes do manifesto ficam guardados até ele vir.
    let mut antes_do_aviso: Vec<Vec<u8>> = Vec::new();
    let mut por_meio: BTreeMap<String, u64> = BTreeMap::new();
    let inicio = Instant::now();
    let mut ultimo_relato = String::new();
    while inicio.elapsed() < Duration::from_secs(o.segundos) {
        for meio in &mut meios {
            let quadros = match meio.receber() {
                Ok(q) => q,
                Err(e) => {
                    eprintln!("{}: {e}", meio.nome());
                    continue;
                }
            };
            for q in quadros {
                let contagem = por_meio.entry(meio.nome().to_owned()).or_default();
                *contagem = contagem.saturating_add(1);
                let era_aviso = matches!(Quadro::decodificar(&q), Ok(Quadro::Manifesto(_)));
                let tinha_aviso = recepcao.manifesto().is_some();
                let mut fila = vec![q];
                if era_aviso && !tinha_aviso {
                    fila.append(&mut antes_do_aviso);
                } else if !tinha_aviso {
                    antes_do_aviso.append(&mut fila);
                }
                for quadro in fila {
                    match recepcao.quadro(&quadro) {
                        Ok(Some(objeto)) => return gravar(o, &recepcao, &objeto, &por_meio),
                        Ok(None) => {}
                        Err(_) => {} // repetido, de outro objeto ou estragado: segue
                    }
                }
            }
        }
        if let Some(m) = recepcao.manifesto() {
            let falta = recepcao.faltando().len();
            let tem = u64::from(m.quantidade).saturating_sub(falta as u64);
            let relato = format!("{}: {tem} de {} pedaços · {por_meio:?}", m.nome, m.quantidade);
            if relato != ultimo_relato {
                println!("{relato}");
                ultimo_relato = relato;
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    let falta = recepcao.faltando();
    Err(format!("tempo esgotado; faltam {} pedaços: {falta:?}", falta.len()))
}

fn gravar(o: &Opcoes, recepcao: &Recepcao, objeto: &[u8], por_meio: &BTreeMap<String, u64>) -> Result<(), String> {
    let nome = recepcao.manifesto().map(|m| nome_seguro(&m.nome)).unwrap_or_else(|| "objeto".into());
    let destino = o.alvo.join(&nome);
    fs::write(&destino, objeto).map_err(|e| format!("{}: {e}", destino.display()))?;
    println!("Chegou inteiro: {} ({} bytes)", destino.display(), objeto.len());
    println!("SHA-512 conferido: {}…", hex(hyurax_crypto::sha512(objeto).get(..16).unwrap_or_default()));
    for (meio, n) in por_meio {
        println!("  {meio}: {n} quadros");
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (comando, resto) = match args.split_first() {
        Some((c, r)) => (c.as_str(), r),
        None => ("", &[][..]),
    };
    let resultado = match comando {
        "enviar" => ler_opcoes(resto).and_then(|o| enviar(&o)),
        "receber" => ler_opcoes(resto).and_then(|o| receber(&o)),
        _ => {
            print!("{AJUDA}");
            return ExitCode::SUCCESS;
        }
    };
    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::FAILURE
        }
    }
}
