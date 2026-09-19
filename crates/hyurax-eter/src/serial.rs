//! O Éter por um fio serial: Bluetooth, rádio LoRa por USB, cabo.
//!
//! O Bluetooth clássico (perfil de porta serial, SPP) aparece para o sistema
//! como uma porta comum: `COM5` no Windows depois de parear, `/dev/rfcomm0` no
//! Linux. Módulos de rádio LoRa e rádios de dados ligados por USB também. Para
//! o Éter, tudo isso é o mesmo meio: um fio de bytes nos dois sentidos, onde
//! os quadros viajam dentro da [moldura](crate::enquadramento).
//!
//! A velocidade da porta (baud) não é configurada aqui: numa porta de
//! Bluetooth ela não existe de verdade, e num rádio USB fica com o sistema.
//!
//! No celular, o Termux não tem acesso ao Bluetooth: do lado Android este meio
//! precisa de um aplicativo próprio, que ainda não existe.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::enquadramento::{Desemoldurador, emoldurar};
use crate::{EterError, Meio};

/// Quadro padrão para Bluetooth serial.
pub const MTU_PADRAO: usize = 1024;

fn falha(e: impl core::fmt::Display) -> EterError {
    EterError::Meio(e.to_string())
}

/// Um fio de bytes: a porta serial ou qualquer par leitor/escritor.
pub struct MeioSerial {
    nome: String,
    mtu: usize,
    vazao: u64,
    escritor: Box<dyn Write + Send>,
    chegou: Arc<Mutex<Vec<u8>>>,
    desemoldurador: Desemoldurador,
}

impl MeioSerial {
    /// Abre uma porta pelo nome: `COM5`, `\\.\COM12`, `/dev/rfcomm0`.
    pub fn porta(nome: &str, porta: &str, mtu: usize) -> Result<Self, EterError> {
        let caminho = if cfg!(windows) && !porta.starts_with(r"\\") {
            // Portas acima de COM9 só abrem com este prefixo; as outras aceitam.
            format!(r"\\.\{porta}")
        } else {
            porta.to_owned()
        };
        let arquivo = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&caminho)
            .map_err(|e| EterError::Meio(format!("{caminho}: {e}")))?;
        let leitor = arquivo.try_clone().map_err(falha)?;
        // SPP do Bluetooth clássico entrega na prática uns 20 KB/s.
        Ok(Self::de_fluxo(nome, leitor, arquivo, mtu, 20_000))
    }

    /// Usa um leitor e um escritor quaisquer (um soquete, um cano, nos testes).
    ///
    /// O leitor vai para uma linha de execução própria, porque ler de uma porta
    /// serial trava até chegar algo e o [`Meio::receber`] não pode travar.
    pub fn de_fluxo(
        nome: &str,
        mut leitor: impl Read + Send + 'static,
        escritor: impl Write + Send + 'static,
        mtu: usize,
        vazao: u64,
    ) -> Self {
        let chegou: Arc<Mutex<Vec<u8>>> = Arc::default();
        let destino = Arc::clone(&chegou);
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match leitor.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => match destino.lock() {
                        Ok(mut fila) => fila.extend_from_slice(buf.get(..n).unwrap_or_default()),
                        Err(_) => break,
                    },
                }
            }
        });
        let mtu = mtu.min(crate::enquadramento::QUADRO_MAX);
        Self {
            nome: nome.to_owned(),
            mtu,
            vazao,
            escritor: Box::new(escritor),
            chegou,
            desemoldurador: Desemoldurador::novo(mtu),
        }
    }
}

impl Meio for MeioSerial {
    fn nome(&self) -> &str {
        &self.nome
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn vazao(&self) -> u64 {
        self.vazao
    }

    fn enviar(&mut self, quadro: &[u8]) -> Result<(), EterError> {
        if quadro.len() > self.mtu {
            return Err(EterError::MeioPequenoDemais {
                mtu: self.mtu,
                preciso: quadro.len(),
            });
        }
        let emoldurado = emoldurar(quadro).ok_or(EterError::MeioPequenoDemais {
            mtu: self.mtu,
            preciso: quadro.len(),
        })?;
        self.escritor.write_all(&emoldurado).map_err(falha)
    }

    fn descarregar(&mut self) -> Result<(), EterError> {
        self.escritor.flush().map_err(falha)
    }

    fn receber(&mut self) -> Result<Vec<Vec<u8>>, EterError> {
        let bytes = match self.chegou.lock() {
            Ok(mut fila) => core::mem::take(&mut *fila),
            Err(_) => return Err(EterError::Meio("fila do fio travada".into())),
        };
        Ok(self.desemoldurador.empurrar(&bytes))
    }
}
