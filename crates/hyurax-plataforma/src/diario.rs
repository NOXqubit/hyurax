//! O diário: um arquivo só de acréscimo, com cada evento numa linha e um hash
//! encadeado que denuncia qualquer linha alterada, apagada ou fora de ordem.
//!
//! ```text
//! {"n":1,"t":1791000000000,"ev":{"tipo":"USER_CREATED",...}}\t<hash>\n
//! hash_n = SHA-512(DOMINIO || hash_{n-1} || bytes do JSON)      hash_0 = 64 zeros
//! ```
//!
//! - **Durável:** cada lote de eventos sai num `write` só, seguido de
//!   `sync_data`. Só depois disso o estado na memória muda.
//! - **Queda no meio da escrita:** a última linha incompleta (sem `\n` ou com
//!   hash que não fecha) é cortada ao reabrir. Linha ruim no meio do arquivo
//!   não é queda: é adulteração ou disco com defeito, e o diário não abre.
//! - **Uma instância só:** a pasta fica travada (`File::try_lock`) enquanto o
//!   processo vive; o sistema solta a trava se o processo cair.
//! - **Cópia:** [`Diario::copiar`] grava a cópia, relê e confere a corrente
//!   inteira antes de dizer que deu certo.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use hyurax_crypto::{HASH_LEN, sha512};
use serde::{Deserialize, Serialize};

use crate::eventos::Evento;

/// Domínio do hash encadeado.
pub const DOMINIO_DIARIO: &[u8] = concat!(hyurax_identidade::raiz!(), "-PLATAFORMA-DIARIO-v1").as_bytes();
/// Linha mais longa aceita (um evento com resultado grande não entra aqui:
/// resultados vão para arquivos próprios).
const LINHA_MAX: usize = 1 << 20;

/// Um evento lido do diário.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Registro {
    /// Número, a partir de 1.
    pub n: u64,
    /// Quando foi gravado.
    pub t: u64,
    /// O evento.
    pub ev: Evento,
}

/// O diário aberto para escrita.
pub struct Diario {
    caminho: PathBuf,
    arquivo: File,
    _trava: File,
    ultimo: [u8; HASH_LEN],
    n: u64,
}

fn encadear(anterior: &[u8; HASH_LEN], json: &[u8]) -> [u8; HASH_LEN] {
    let mut dados = Vec::with_capacity(DOMINIO_DIARIO.len().saturating_add(HASH_LEN).saturating_add(json.len()));
    dados.extend_from_slice(DOMINIO_DIARIO);
    dados.extend_from_slice(anterior);
    dados.extend_from_slice(json);
    sha512(&dados)
}

fn hex(b: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len().saturating_mul(2));
    for &x in b {
        s.push(char::from(D.get(usize::from(x >> 4)).copied().unwrap_or(b'0')));
        s.push(char::from(D.get(usize::from(x & 15)).copied().unwrap_or(b'0')));
    }
    s
}

/// O que a leitura achou.
pub struct Leitura {
    /// Os registros válidos, em ordem.
    pub registros: Vec<Registro>,
    /// Bytes válidos (o resto é a cauda cortada).
    pub bytes_validos: u64,
    /// Havia uma última linha incompleta.
    pub cauda_cortada: bool,
    /// O último hash.
    pub ultimo: [u8; HASH_LEN],
}

/// Lê e confere um diário inteiro, sem abrir para escrita.
pub fn ler(caminho: &Path) -> Result<Leitura, String> {
    let arquivo = match File::open(caminho) {
        Ok(a) => a,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Leitura { registros: Vec::new(), bytes_validos: 0, cauda_cortada: false, ultimo: [0; HASH_LEN] });
        }
        Err(e) => return Err(format!("não abri o diário {}: {e}", caminho.display())),
    };
    let mut leitor = BufReader::new(arquivo);
    let mut registros = Vec::new();
    let mut ultimo = [0u8; HASH_LEN];
    let mut validos: u64 = 0;
    let mut linha = Vec::new();
    let mut numero: u64 = 0;
    loop {
        linha.clear();
        let lidos = (&mut leitor).take(u64::try_from(LINHA_MAX).unwrap_or(u64::MAX).saturating_add(1)).read_until(b'\n', &mut linha);
        let lidos = lidos.map_err(|e| format!("erro lendo o diário: {e}"))?;
        if lidos == 0 {
            break;
        }
        numero = numero.saturating_add(1);
        let completa = linha.last() == Some(&b'\n');
        let conteudo = linha.strip_suffix(b"\n").unwrap_or(&linha);
        let analisada = analisar(conteudo, &ultimo, numero);
        match (analisada, completa) {
            (Ok((reg, h)), true) => {
                ultimo = h;
                validos = validos.saturating_add(u64::try_from(lidos).unwrap_or(0));
                registros.push(reg);
            }
            (resultado, completa) => {
                // linha sem fim e nada depois dela: queda no meio da escrita.
                // Linha completa com hash errado não é queda (a escrita põe o
                // `\n` por último): é adulteração, mesmo sendo a última.
                let mut resto = Vec::new();
                leitor.read_to_end(&mut resto).map_err(|e| format!("erro lendo o diário: {e}"))?;
                if !completa && resto.is_empty() {
                    return Ok(Leitura { registros, bytes_validos: validos, cauda_cortada: true, ultimo });
                }
                let motivo = resultado.err().unwrap_or_else(|| "linha sem fim".into());
                return Err(format!("o diário está corrompido na linha {numero}: {motivo}. Restaure a última cópia."));
            }
        }
    }
    Ok(Leitura { registros, bytes_validos: validos, cauda_cortada: false, ultimo })
}

fn analisar(conteudo: &[u8], anterior: &[u8; HASH_LEN], esperado: u64) -> Result<(Registro, [u8; HASH_LEN]), String> {
    let tab = conteudo.iter().rposition(|&b| b == b'\t').ok_or("sem hash")?;
    let (json, resto) = conteudo.split_at(tab);
    let hash_txt = resto.get(1..).ok_or("sem hash")?;
    let h = encadear(anterior, json);
    if hex(&h).as_bytes() != hash_txt {
        return Err("o hash encadeado não fecha".into());
    }
    let reg: Registro = serde_json::from_slice(json).map_err(|e| format!("evento ilegível: {e}"))?;
    if reg.n != esperado {
        return Err(format!("número {} fora de ordem (esperado {esperado})", reg.n));
    }
    Ok((reg, h))
}

impl Diario {
    /// Abre (ou cria) o diário da pasta, trava a pasta e devolve os registros.
    pub fn abrir(pasta: &Path) -> Result<(Self, Vec<Registro>, bool), String> {
        std::fs::create_dir_all(pasta).map_err(|e| format!("não criei a pasta {}: {e}", pasta.display()))?;
        let trava = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(pasta.join("diario.trava"))
            .map_err(|e| format!("não abri a trava: {e}"))?;
        trava.try_lock().map_err(|_| format!("outra instância da plataforma está usando a pasta {}", pasta.display()))?;
        let caminho = pasta.join("diario.hyx");
        let leitura = ler(&caminho)?;
        // escrita sem `append`: no Windows, arquivo aberto só para acréscimo
        // não pode ser cortado. O cursor vai para o fim e só este processo
        // escreve (a trava garante).
        let mut arquivo = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&caminho)
            .map_err(|e| format!("não abri o diário para escrita: {e}"))?;
        if leitura.cauda_cortada {
            arquivo.set_len(leitura.bytes_validos).map_err(|e| format!("não cortei a cauda do diário: {e}"))?;
            arquivo.sync_all().map_err(|e| format!("{e}"))?;
        }
        arquivo.seek(SeekFrom::End(0)).map_err(|e| format!("{e}"))?;
        let n = leitura.registros.last().map_or(0, |r| r.n);
        Ok((Self { caminho, arquivo, _trava: trava, ultimo: leitura.ultimo, n }, leitura.registros, leitura.cauda_cortada))
    }

    /// Grava um lote de eventos e só volta depois do `sync_data`.
    pub fn gravar(&mut self, t: u64, eventos: &[Evento]) -> Result<Vec<Registro>, String> {
        let mut bloco = Vec::new();
        let mut ultimo = self.ultimo;
        let mut n = self.n;
        let mut registros = Vec::with_capacity(eventos.len());
        for ev in eventos {
            n = n.checked_add(1).ok_or("diário cheio")?;
            let reg = Registro { n, t, ev: ev.clone() };
            let json = serde_json::to_vec(&reg).map_err(|e| format!("evento não serializa: {e}"))?;
            if json.len() > LINHA_MAX {
                return Err("evento grande demais para o diário".into());
            }
            ultimo = encadear(&ultimo, &json);
            bloco.extend_from_slice(&json);
            bloco.push(b'\t');
            bloco.extend_from_slice(hex(&ultimo).as_bytes());
            bloco.push(b'\n');
            registros.push(reg);
        }
        self.arquivo.write_all(&bloco).map_err(|e| format!("não gravei no diário: {e}"))?;
        self.arquivo.sync_data().map_err(|e| format!("não sincronizei o diário: {e}"))?;
        self.ultimo = ultimo;
        self.n = n;
        Ok(registros)
    }

    /// Eventos gravados até agora.
    pub fn quantos(&self) -> u64 {
        self.n
    }

    /// O último hash da corrente (hex): quem guarda este valor fora da
    /// máquina percebe se o diário for reescrito.
    pub fn selo(&self) -> String {
        hex(&self.ultimo)
    }

    /// Copia o diário para `destino` e confere a cópia inteira.
    pub fn copiar(&self, destino: &Path) -> Result<u64, String> {
        if let Some(pai) = destino.parent() {
            std::fs::create_dir_all(pai).map_err(|e| format!("{e}"))?;
        }
        let temporario = destino.with_extension("parcial");
        std::fs::copy(&self.caminho, &temporario).map_err(|e| format!("não copiei o diário: {e}"))?;
        let conferida = ler(&temporario)?;
        if conferida.cauda_cortada || conferida.registros.last().map_or(0, |r| r.n) < self.n {
            let _ = std::fs::remove_file(&temporario);
            return Err("a cópia não conferiu".into());
        }
        OpenOptions::new().write(true).open(&temporario).and_then(|f| f.sync_all()).map_err(|e| format!("{e}"))?;
        std::fs::rename(&temporario, destino).map_err(|e| format!("{e}"))?;
        Ok(self.n)
    }

    /// Os bytes do diário (para a cópia baixada pelo administrador).
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.caminho).map_err(|e| format!("{e}"))
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::eventos::Evento;

    fn ev(i: u64) -> Evento {
        Evento::AlertResolved { id: i, nota: format!("nota {i}"), por: 1, t: i }
    }

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hyx-diario-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn grava_relê_e_corta_a_cauda() {
        let p = pasta("cauda");
        {
            let (mut d, regs, _) = Diario::abrir(&p).unwrap();
            assert!(regs.is_empty());
            d.gravar(1, &[ev(1), ev(2)]).unwrap();
            d.gravar(2, &[ev(3)]).unwrap();
        }
        // queda no meio da escrita: meia linha no fim
        let caminho = p.join("diario.hyx");
        let mut f = OpenOptions::new().append(true).open(&caminho).unwrap();
        f.write_all(b"{\"n\":4,\"t\":3,\"ev\":{\"tipo\":\"ALERT_RES").unwrap();
        drop(f);
        let (mut d, regs, cortou) = Diario::abrir(&p).unwrap();
        assert!(cortou);
        assert_eq!(regs.len(), 3);
        assert_eq!(regs[2].ev, ev(3));
        d.gravar(4, &[ev(4)]).unwrap();
        drop(d);
        let (_, regs, cortou) = Diario::abrir(&p).unwrap();
        assert!(!cortou);
        assert_eq!(regs.iter().map(|r| r.n).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn linha_alterada_no_meio_nao_abre() {
        let p = pasta("adulterado");
        {
            let (mut d, _, _) = Diario::abrir(&p).unwrap();
            d.gravar(1, &[ev(1), ev(2), ev(3)]).unwrap();
        }
        let caminho = p.join("diario.hyx");
        let texto = std::fs::read_to_string(&caminho).unwrap().replace("nota 2", "nota X");
        std::fs::write(&caminho, texto).unwrap();
        let erro = Diario::abrir(&p).err().unwrap();
        assert!(erro.contains("corrompido na linha 2"), "{erro}");
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn linha_apagada_no_meio_nao_abre() {
        let p = pasta("apagado");
        {
            let (mut d, _, _) = Diario::abrir(&p).unwrap();
            d.gravar(1, &[ev(1), ev(2), ev(3)]).unwrap();
        }
        let caminho = p.join("diario.hyx");
        let texto = std::fs::read_to_string(&caminho).unwrap();
        let linhas: Vec<&str> = texto.lines().collect();
        std::fs::write(&caminho, format!("{}\n{}\n", linhas[0], linhas[2])).unwrap();
        assert!(Diario::abrir(&p).is_err());
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn uma_instancia_so_e_copia_conferida() {
        let p = pasta("trava");
        let (mut d, _, _) = Diario::abrir(&p).unwrap();
        assert!(Diario::abrir(&p).is_err(), "a segunda instância não abre");
        d.gravar(1, &[ev(1)]).unwrap();
        let destino = p.join("copias").join("um.hyx");
        assert_eq!(d.copiar(&destino).unwrap(), 1);
        assert_eq!(ler(&destino).unwrap().registros.len(), 1);
        drop(d);
        let _ = std::fs::remove_dir_all(&p);
    }
}
