// ✝ Eclesiastes 11:1 — “Lança o teu pão sobre as águas, porque depois de muitos dias o acharás.”
//! Meios de verdade que já dão para usar hoje.
//!
//! Estes dois provam a tese da camada: se o transporte funciona com uma pasta
//! de arquivos — um pendrive, uma pasta sincronizada, um cartão de memória que
//! alguém levou a pé —, então o meio é mesmo trocável, e Bluetooth, LoRa ou
//! satélite entram depois sem tocar no protocolo.
//!
//! A [`MeioPasta`] é também o primeiro transporte que **atravessa o tempo**:
//! quem envia e quem recebe não precisam estar ligados juntos.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Meio, EterError};

/// Extensão dos quadros gravados em disco.
const EXTENSAO: &str = "eter";

fn falha(e: impl core::fmt::Display) -> EterError {
    EterError::Meio(e.to_string())
}

/// Transporte por pasta: pendrive, cartão, pasta compartilhada.
///
/// Envia gravando um arquivo por quadro; recebe lendo os quadros que ainda não
/// leu. Os arquivos lidos são movidos para a subpasta `lidos`, então a mesma
/// pasta pode ser lida várias vezes sem repetir trabalho.
pub struct MeioPasta {
    nome: String,
    saida: PathBuf,
    entrada: PathBuf,
    mtu: usize,
    contador: u64,
}

impl MeioPasta {
    /// Abre um meio que grava em `saida` e lê de `entrada`.
    ///
    /// As duas podem ser a mesma pasta em demonstrações; em uso real, uma é a
    /// pasta do outro lado.
    pub fn novo(
        nome: &str,
        saida: impl AsRef<Path>,
        entrada: impl AsRef<Path>,
        mtu: usize,
    ) -> Result<Self, EterError> {
        let saida = saida.as_ref().to_path_buf();
        let entrada = entrada.as_ref().to_path_buf();
        fs::create_dir_all(&saida).map_err(falha)?;
        fs::create_dir_all(&entrada).map_err(falha)?;
        fs::create_dir_all(entrada.join("lidos")).map_err(falha)?;
        Ok(Self {
            nome: nome.to_owned(),
            saida,
            entrada,
            mtu,
            contador: 0,
        })
    }

    fn proximo_nome(&mut self) -> String {
        self.contador = self.contador.saturating_add(1);
        let agora = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        format!("{agora:020}-{:06}.{EXTENSAO}", self.contador)
    }
}

impl Meio for MeioPasta {
    fn nome(&self) -> &str {
        &self.nome
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn enviar(&mut self, quadro: &[u8]) -> Result<(), EterError> {
        if quadro.len() > self.mtu {
            return Err(EterError::MeioPequenoDemais {
                mtu: self.mtu,
                preciso: quadro.len(),
            });
        }
        let nome = self.proximo_nome();
        let arquivo = self.saida.join(nome);
        // Grava com outro nome e renomeia: quem lê nunca vê arquivo pela metade.
        let temporario = arquivo.with_extension("parcial");
        fs::write(&temporario, quadro).map_err(falha)?;
        fs::rename(&temporario, &arquivo).map_err(falha)
    }

    fn receber(&mut self) -> Result<Vec<Vec<u8>>, EterError> {
        let lidos = self.entrada.join("lidos");
        let mut arquivos: Vec<PathBuf> = fs::read_dir(&self.entrada)
            .map_err(falha)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == EXTENSAO))
            .collect();
        arquivos.sort();

        let mut quadros = Vec::new();
        for arquivo in arquivos {
            let dados = match fs::read(&arquivo) {
                Ok(d) => d,
                Err(_) => continue, // outro processo pegou antes; não é erro
            };
            if let Some(nome) = arquivo.file_name() {
                let _ = fs::rename(&arquivo, lidos.join(nome));
            }
            quadros.push(dados);
        }
        Ok(quadros)
    }
}

/// Meio de mentira, para teste e demonstração: entrega na memória.
///
/// [`MeioMemoria::par`] devolve as duas pontas de um mesmo canal.
pub struct MeioMemoria {
    nome: String,
    mtu: usize,
    minha_saida: Arc<Mutex<Vec<Vec<u8>>>>,
    minha_entrada: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl MeioMemoria {
    /// Cria as duas pontas de um canal com o mesmo nome e tamanho de quadro.
    pub fn par(nome: &str, mtu: usize) -> (Self, Self) {
        let a: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
        let b: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
        (
            Self {
                nome: nome.to_owned(),
                mtu,
                minha_saida: Arc::clone(&a),
                minha_entrada: Arc::clone(&b),
            },
            Self {
                nome: nome.to_owned(),
                mtu,
                minha_saida: b,
                minha_entrada: a,
            },
        )
    }
}

impl Meio for MeioMemoria {
    fn nome(&self) -> &str {
        &self.nome
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn enviar(&mut self, quadro: &[u8]) -> Result<(), EterError> {
        if quadro.len() > self.mtu {
            return Err(EterError::MeioPequenoDemais {
                mtu: self.mtu,
                preciso: quadro.len(),
            });
        }
        match self.minha_saida.lock() {
            Ok(mut fila) => {
                fila.push(quadro.to_vec());
                Ok(())
            }
            Err(_) => Err(EterError::Meio("fila travada".into())),
        }
    }

    fn receber(&mut self) -> Result<Vec<Vec<u8>>, EterError> {
        match self.minha_entrada.lock() {
            Ok(mut fila) => Ok(core::mem::take(&mut fila)),
            Err(_) => Err(EterError::Meio("fila travada".into())),
        }
    }
}
