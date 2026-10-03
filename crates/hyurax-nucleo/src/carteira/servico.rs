//! A carteira deste computador, como serviço do núcleo: criar, importar,
//! enviar HYX e o segundo fator (código de 6 dígitos).
//!
//! A senha só passa pela memória na hora de cifrar ou de assinar, e não é
//! guardada nem registrada em lugar nenhum.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, TryLockError};

use hyurax_crypto::ADDRESS_LEN;
use hyurax_net::Rede;
use serde_json::{Value, json};

use super::seguranca::{Seguranca, Tentativas};
use super::{arquivo, cifrar_segredo, endereco, envio, gravar, totp};
use crate::arquivos;
use crate::config::ConfigDoNo;
use crate::util::{agora_unix, hex, hyx};

/// Só uma criação ou importação por vez. Sem isto, dois pedidos juntos
/// passavam os dois pela conferência "já existe?" durante a cifragem (que
/// leva até um minuto), e o segundo sobrescrevia a carteira do primeiro.
static MEXENDO_NA_CARTEIRA: Mutex<()> = Mutex::new(());

fn trava_da_carteira() -> Result<MutexGuard<'static, ()>, String> {
    match MEXENDO_NA_CARTEIRA.try_lock() {
        Ok(g) => Ok(g),
        Err(TryLockError::Poisoned(p)) => Ok(p.into_inner()),
        Err(TryLockError::WouldBlock) => Err("já estou criando uma carteira; espere terminar".into()),
    }
}

/// A carteira deste computador.
pub struct Carteira {
    /// O arquivo (`None` no nó de terminal que só recebe por `--endereco`).
    pub arquivo: Option<PathBuf>,
    endereco: Mutex<Option<[u8; ADDRESS_LEN]>>,
    seguranca: Mutex<Seguranca>,
    totp_pendente: Mutex<Option<Vec<u8>>>,
    tentativas: Mutex<Tentativas>,
    destravado: AtomicBool,
    pasta_config: PathBuf,
}

impl Carteira {
    /// Abre a carteira do arquivo (se existir) e o segundo fator da pasta.
    pub fn abrir(arquivo: Option<PathBuf>, endereco_fixo: Option<[u8; ADDRESS_LEN]>, pasta_config: PathBuf) -> Self {
        let endereco = endereco_fixo.or_else(|| {
            let texto = std::fs::read_to_string(arquivo.as_ref()?).ok()?;
            arquivo::endereco(&texto).ok()
        });
        let seguranca = Seguranca::ler(&pasta_config);
        let trava = seguranca.ligado() && seguranca.trava;
        Self {
            arquivo,
            endereco: Mutex::new(endereco),
            seguranca: Mutex::new(seguranca),
            totp_pendente: Mutex::new(None),
            tentativas: Mutex::new(Tentativas::default()),
            destravado: AtomicBool::new(!trava),
            pasta_config,
        }
    }

    /// O endereço, se já houver carteira.
    pub fn endereco(&self) -> Option<[u8; ADDRESS_LEN]> {
        self.endereco.lock().ok().and_then(|e| *e)
    }

    /// O programa está trancado agora (cadeado ligado e ainda sem o código)?
    pub fn trancado(&self) -> bool {
        !self.destravado.load(Ordering::Relaxed) && self.seguranca.lock().is_ok_and(|s| s.ligado() && s.trava)
    }

    /// O estado do segundo fator: (ligado, exige no envio, tranca ao abrir).
    pub fn seguranca(&self) -> (bool, bool, bool) {
        self.seguranca.lock().map(|s| (s.ligado(), s.exige_envio, s.trava)).unwrap_or((false, false, false))
    }

    /// Confere um código de 6 dígitos com o segredo gravado, contando os
    /// erros (espera crescente) e recusando código já usado.
    fn conferir_codigo(&self, codigo: &str) -> Result<(), String> {
        let s = self.seguranca.lock().map_err(|_| "segurança travada".to_string())?;
        let mut t = self.tentativas.lock().map_err(|_| "segurança travada".to_string())?;
        t.conferir(&s, agora_unix(), codigo)
    }

    /// A carteira deste computador guarda o segredo sem senha (formato antigo)?
    pub fn sem_senha(&self) -> bool {
        self.arquivo.as_ref().and_then(|a| std::fs::read_to_string(a).ok()).is_some_and(|t| arquivo::e_formato_antigo(&t))
    }

    fn marcar(&self, e: [u8; ADDRESS_LEN]) {
        if let Ok(mut x) = self.endereco.lock() {
            *x = Some(e);
        }
    }

    /// Cria uma carteira nova, com senha, se ainda não houver nenhuma.
    ///
    /// # Errors
    /// Já existe carteira, senhas diferentes ou fracas, ou disco.
    pub fn criar(&self, senha: &str, repetida: &str) -> Result<[u8; ADDRESS_LEN], String> {
        let destino = self.arquivo.as_ref().ok_or("este nó não guarda carteira")?;
        let _trava = trava_da_carteira()?;
        if self.endereco().is_some() || destino.exists() {
            return Err("já existe uma carteira neste computador".into());
        }
        if senha != repetida {
            return Err("as duas senhas não são iguais".into());
        }
        arquivo::senha_aceitavel(senha)?;
        // a chave vem direto do sistema operacional, não do gerador derivado
        let segredo = hyurax_net::entropia::entropia_do_sistema()?;
        let conteudo = cifrar_segredo(&segredo, senha)?;
        gravar(destino, &conteudo)?;
        let e = arquivo::endereco(&conteudo)?;
        self.marcar(e);
        Ok(e)
    }

    /// Importa o conteúdo de um `carteira.txt`. Um arquivo do formato antigo
    /// (segredo em texto) só entra cifrado: `senha` e `repetida` escolhem a
    /// senha nova. Devolve o endereço e se o arquivo veio sem senha.
    ///
    /// # Errors
    /// Já existe carteira, o texto não é uma carteira do Hyurax, ou formato
    /// antigo sem senha nova válida.
    pub fn importar(&self, conteudo: &str, senha: &str, repetida: &str) -> Result<([u8; ADDRESS_LEN], bool), String> {
        let destino = self.arquivo.as_ref().ok_or("este nó não guarda carteira")?;
        let _trava = trava_da_carteira()?;
        if self.endereco().is_some() || destino.exists() {
            return Err("já existe uma carteira neste computador".into());
        }
        let conteudo = conteudo.trim();
        if conteudo.is_empty() {
            return Err("cole o conteúdo do arquivo carteira.txt".into());
        }
        let e = arquivo::endereco(conteudo).map_err(|x| format!("não é uma carteira do Hyurax: {x}"))?;
        let antiga = arquivo::e_formato_antigo(conteudo);
        if antiga {
            // o segredo em texto não vai para o disco: entra já cifrado
            if senha.is_empty() {
                return Err("este arquivo guarda o segredo sem senha: escolha uma senha nova (duas vezes) para ele ser cifrado na importação".into());
            }
            if senha != repetida {
                return Err("as duas senhas não são iguais".into());
            }
            let segredo = arquivo::abrir(conteudo, "")?;
            gravar(destino, &cifrar_segredo(&segredo, senha)?)?;
        } else {
            gravar(destino, &format!("{conteudo}\n"))?;
        }
        self.marcar(e);
        Ok((e, antiga))
    }

    /// Cifra com senha uma carteira do formato antigo que já está neste
    /// computador (de uma versão anterior do programa).
    ///
    /// # Errors
    /// Sem carteira, carteira já cifrada, senhas diferentes ou fracas, ou disco.
    pub fn cifrar(&self, senha: &str, repetida: &str) -> Result<(), String> {
        let destino = self.arquivo.as_ref().ok_or("este nó não guarda carteira")?;
        let _trava = trava_da_carteira()?;
        let texto = arquivos::ler(destino)?;
        if !arquivo::e_formato_antigo(&texto) {
            return Err("esta carteira já é cifrada com senha".into());
        }
        if senha != repetida {
            return Err("as duas senhas não são iguais".into());
        }
        let segredo = arquivo::abrir(&texto, "")?;
        gravar(destino, &cifrar_segredo(&segredo, senha)?)
    }

    /// Confere um envio sem assinar (o endereço de destino, o valor e a taxa).
    ///
    /// # Errors
    /// Endereço, valor ou taxa inválidos.
    pub fn conferir(&self, config: &ConfigDoNo, para: &str, valor: &str, taxa: &str) -> Result<envio::Pedido, String> {
        let de = self.endereco().ok_or("ainda não há carteira neste computador")?;
        envio::conferir(&de, para, valor, taxa, config.rede.nome)
    }

    /// Assina e manda uma transferência. A ordem das conferências é de
    /// propósito: o que é de graça primeiro (formato, segundo fator), o que
    /// custa depois (abrir a carteira, falar com a rede).
    ///
    /// # Errors
    /// Pedido inválido, código errado, senha errada ou recusa da rede.
    pub fn enviar(&self, rede: &std::sync::Arc<Rede>, config: &ConfigDoNo, pedido: &envio::Pedido, senha: &str, codigo: &str) -> Result<Value, String> {
        if senha.is_empty() {
            return Err("digite a senha da carteira para assinar o envio.".into());
        }
        let destino = self.arquivo.as_ref().ok_or("este nó não guarda carteira")?;
        let texto = arquivos::ler(destino)?;
        if arquivo::e_formato_antigo(&texto) {
            return Err("esta carteira guarda o segredo sem senha: proteja com senha (Carteira) antes de enviar".into());
        }
        let exige = self.seguranca.lock().is_ok_and(|s| s.ligado() && s.exige_envio);
        if exige {
            self.conferir_codigo(codigo)?;
        }
        let feita = envio::enviar(rede, &config.rede.magic, config.rede.coinbase_maturity, &texto, senha, pedido)?;
        Ok(json!({
            "txid": hex(&feita.txid),
            "valor": hyx(u128::from(pedido.valor)),
            "taxa": hyx(u128::from(pedido.taxa)),
            "para": endereco::mostrar(&pedido.para, config.rede.nome),
            "nonce": feita.nonce,
            "pares": feita.pares,
        }))
    }

    /// Começa a ligar o segundo fator: sorteia um segredo e devolve o QR.
    /// Só vale depois do primeiro código certo; até lá, nada é gravado.
    ///
    /// # Errors
    /// Já ligado, ou gerador indisponível.
    pub fn seguranca_comecar(&self) -> Result<Value, String> {
        if self.seguranca.lock().is_ok_and(|s| s.ligado()) {
            return Err("o segundo fator já está ligado neste computador".into());
        }
        let mut segredo = vec![0u8; totp::SEGREDO_LEN];
        hyurax_net::entropia::preencher(&mut segredo)?;
        let conta = self.endereco().map(|e| hex(&e).chars().take(10).collect::<String>()).unwrap_or_default();
        let resposta = json!({ "segredo": totp::base32(&segredo), "uri": totp::uri(&segredo, &conta) });
        if let Ok(mut p) = self.totp_pendente.lock() {
            *p = Some(segredo);
        }
        Ok(resposta)
    }

    /// Confirma o segredo com o primeiro código certo e grava.
    ///
    /// # Errors
    /// Sem segredo pendente, ou código errado.
    pub fn seguranca_confirmar(&self, codigo: &str, trava: bool) -> Result<(), String> {
        let segredo = self.totp_pendente.lock().ok().and_then(|p| p.clone()).ok_or("comece de novo: o segredo desta tela já não vale")?;
        if !totp::confere(&segredo, agora_unix(), codigo) {
            return Err("código errado. Confira a hora do celular e digite o código que está na tela agora.".into());
        }
        let nova = Seguranca { segredo: Some(segredo), exige_envio: true, trava };
        nova.gravar(&self.pasta_config)?;
        if let Ok(mut s) = self.seguranca.lock() {
            *s = nova;
        }
        if let Ok(mut p) = self.totp_pendente.lock() {
            *p = None;
        }
        self.destravado.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Muda o que o segundo fator protege, ou desliga. Sempre com um código
    /// certo: quem não tem o celular não mexe no cadeado. Devolve `true` se
    /// desligou.
    ///
    /// # Errors
    /// Não ligado, ou código errado.
    pub fn seguranca_mudar(&self, codigo: &str, desligar: bool, exige_envio: bool, trava: bool) -> Result<bool, String> {
        if !self.seguranca.lock().is_ok_and(|s| s.ligado()) {
            return Err("o segundo fator não está ligado".into());
        }
        self.conferir_codigo(codigo)?;
        let mut guarda = self.seguranca.lock().map_err(|_| "segurança travada".to_string())?;
        let nova = if desligar { Seguranca::default() } else { Seguranca { segredo: guarda.segredo.clone(), exige_envio, trava } };
        nova.gravar(&self.pasta_config)?;
        let desligou = !nova.ligado();
        *guarda = nova;
        drop(guarda);
        self.destravado.store(true, Ordering::Relaxed);
        Ok(desligou)
    }

    /// Destranca o programa nesta abertura.
    ///
    /// # Errors
    /// Código errado.
    pub fn destravar(&self, codigo: &str) -> Result<(), String> {
        self.conferir_codigo(codigo)?;
        self.destravado.store(true, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn so_uma_carteira_nasce_por_vez() {
        let primeira = trava_da_carteira().unwrap();
        let segunda = std::thread::spawn(|| trava_da_carteira().map(|_| ())).join().unwrap();
        assert_eq!(segunda, Err("já estou criando uma carteira; espere terminar".to_string()));
        drop(primeira);
        assert!(std::thread::spawn(|| trava_da_carteira().map(|_| ())).join().unwrap().is_ok());
    }
}
