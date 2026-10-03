//! O cadeado do programa: o código de 6 dígitos do Google Authenticator.
//!
//! Guarda o segredo do TOTP e as duas escolhas do dono:
//!
//! - **exigir o código para enviar HYX** (ligado junto com o segundo fator);
//! - **trancar o programa ao abrir**, pedindo o código antes de mostrar o painel.
//!
//! Mora em `PASTA/seguranca.txt`, ao lado da carteira. Apagar esse arquivo
//! desliga o segundo fator — e isso é dito na tela, porque um cadeado que
//! esconde como se abre não é honesto. Quem tem a pasta tem a carteira cifrada e
//! este arquivo; o código de 6 dígitos vale contra quem senta no computador e
//! sabe a sua senha, não contra quem copia os arquivos.

use std::path::Path;

use crate::carteira::totp;

/// Quantos erros seguidos de código antes de começar a esperar.
const ERROS_LIVRES: u32 = 5;
/// A primeira espera depois dos erros livres; cada erro seguinte dobra.
const ESPERA_INICIAL_S: u64 = 30;
/// A espera nunca passa disto.
const ESPERA_MAXIMA_S: u64 = 15 * 60;

/// As tentativas de código desta abertura do programa: contra quem tenta os
/// 10^6 códigos por um script, e contra reaproveitar um código já usado.
#[derive(Default)]
pub(crate) struct Tentativas {
    erros: u32,
    bloqueado_ate: u64,
    ultimo_passo: Option<u64>,
}

impl Tentativas {
    /// Segundos até poder tentar de novo (0 = pode agora).
    pub fn espera(&self, agora: u64) -> u64 {
        self.bloqueado_ate.saturating_sub(agora)
    }

    /// Confere o código respeitando a espera e o anti-reuso. `Err` diz por quê.
    pub fn conferir(&mut self, s: &Seguranca, agora: u64, digitado: &str) -> Result<(), String> {
        let falta = self.espera(agora);
        if falta > 0 {
            return Err(format!("muitos códigos errados seguidos: espere {falta} s e tente de novo."));
        }
        let passo = s.segredo.as_ref().and_then(|seg| totp::passo_que_confere(seg, agora, digitado));
        match passo {
            Some(p) if self.ultimo_passo.is_none_or(|u| p > u) => {
                self.erros = 0;
                self.bloqueado_ate = 0;
                self.ultimo_passo = Some(p);
                Ok(())
            }
            Some(_) => Err("este código já foi usado: espere o próximo aparecer no aplicativo.".into()),
            None => {
                self.erros = self.erros.saturating_add(1);
                if self.erros >= ERROS_LIVRES {
                    let dobras = (self.erros - ERROS_LIVRES).min(10);
                    let espera = ESPERA_INICIAL_S.saturating_mul(1u64 << dobras).min(ESPERA_MAXIMA_S);
                    self.bloqueado_ate = agora.saturating_add(espera);
                }
                Err("código de 6 dígitos errado ou vencido. O código muda a cada 30 segundos.".into())
            }
        }
    }
}

/// Nome do arquivo dentro da pasta de dados.
const ARQUIVO: &str = "seguranca.txt";

/// O segundo fator, do jeito que está gravado.
#[derive(Default)]
pub(crate) struct Seguranca {
    /// Segredo do TOTP. `None`: segundo fator desligado.
    pub segredo: Option<Vec<u8>>,
    /// Pedir o código para enviar HYX.
    pub exige_envio: bool,
    /// Pedir o código ao abrir o programa.
    pub trava: bool,
}

impl Seguranca {
    /// Lê o arquivo. Arquivo que não existe, ou linha estragada, vira desligado.
    pub fn ler(dados: &Path) -> Self {
        let mut s = Self::default();
        let Ok(texto) = std::fs::read_to_string(dados.join(ARQUIVO)) else { return s };
        for linha in texto.lines() {
            match linha.trim().split_once('=') {
                Some(("totp", v)) => s.segredo = totp::de_base32(v.trim()).filter(|b| b.len() >= 10),
                Some(("exige_envio", v)) => s.exige_envio = v.trim() == "1",
                Some(("trava_ao_abrir", v)) => s.trava = v.trim() == "1",
                _ => {}
            }
        }
        if s.segredo.is_none() {
            // Sem segredo não há o que exigir: as duas chaves caem juntas.
            s.exige_envio = false;
            s.trava = false;
        }
        s
    }

    /// Grava o arquivo (temporário e troca de nome, como a carteira).
    ///
    /// # Errors
    /// Falha de escrita na pasta de dados.
    pub fn gravar(&self, dados: &Path) -> Result<(), String> {
        let arquivo = dados.join(ARQUIVO);
        let Some(segredo) = &self.segredo else {
            // Desligado: o arquivo some inteiro, em vez de ficar um resto confuso.
            if arquivo.exists() {
                std::fs::remove_file(&arquivo).map_err(|e| format!("não consegui apagar {ARQUIVO}: {e}"))?;
            }
            return Ok(());
        };
        let texto = format!(
            "# Segundo fator do Hyurax (código de 6 dígitos).\n\
             # Apagar este arquivo desliga o código e volta a valer só a senha.\n\
             # Quem copiar este arquivo consegue gerar os mesmos códigos: ele protege\n\
             # contra quem usa este computador, não contra quem leva os arquivos.\n\
             totp={}\n\
             exige_envio={}\n\
             trava_ao_abrir={}\n",
            totp::base32(segredo),
            u8::from(self.exige_envio),
            u8::from(self.trava),
        );
        crate::arquivos::gravar_privado(&arquivo, &texto)
    }

    /// Existe segundo fator configurado?
    pub fn ligado(&self) -> bool {
        self.segredo.is_some()
    }

    /// O código digitado vale agora?  Sem segredo, nada a conferir: `false`.
    #[cfg(test)]
    pub fn confere(&self, agora: u64, digitado: &str) -> bool {
        self.segredo.as_ref().is_some_and(|s| totp::confere(s, agora, digitado))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    fn pasta(nome: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("hyurax-seg-{nome}"));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn grava_e_le_de_volta() {
        let p = pasta("ida-e-volta");
        let s = Seguranca { segredo: Some(vec![7u8; totp::SEGREDO_LEN]), exige_envio: true, trava: true };
        s.gravar(&p).unwrap();
        let lida = Seguranca::ler(&p);
        assert_eq!(lida.segredo.as_deref(), Some(&[7u8; totp::SEGREDO_LEN][..]));
        assert!(lida.exige_envio && lida.trava && lida.ligado());
        // O código gerado com o segredo lido confere.
        let agora = 1_700_000_000;
        assert!(lida.confere(agora, &totp::codigo(&[7u8; totp::SEGREDO_LEN], agora)));
        assert!(!lida.confere(agora, "000000"));
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn desligar_apaga_o_arquivo() {
        let p = pasta("desligar");
        Seguranca { segredo: Some(vec![1u8; 20]), exige_envio: true, trava: false }.gravar(&p).unwrap();
        assert!(p.join(ARQUIVO).exists());
        Seguranca::default().gravar(&p).unwrap();
        assert!(!p.join(ARQUIVO).exists());
        assert!(!Seguranca::ler(&p).ligado());
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn sem_segredo_as_exigencias_caem() {
        let p = pasta("sem-segredo");
        std::fs::write(p.join(ARQUIVO), "exige_envio=1\ntrava_ao_abrir=1\n").unwrap();
        let s = Seguranca::ler(&p);
        assert!(!s.ligado() && !s.exige_envio && !s.trava);
        assert!(!s.confere(1, "123456"));
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn codigo_nao_vale_duas_vezes_e_erros_seguidos_fazem_esperar() {
        let segredo = vec![9u8; totp::SEGREDO_LEN];
        let s = Seguranca { segredo: Some(segredo.clone()), exige_envio: true, trava: true };
        let mut t = Tentativas::default();
        let agora = 1_700_000_000;
        let certo = totp::codigo(&segredo, agora);
        assert!(t.conferir(&s, agora, &certo).is_ok());
        // o mesmo código, de novo, dentro da mesma janela: recusado
        assert!(t.conferir(&s, agora + 5, &certo).unwrap_err().contains("já foi usado"));
        // cinco erros seguidos: daí em diante, espera
        let errado = if certo == "000000" { "111111" } else { "000000" };
        for _ in 0..ERROS_LIVRES {
            assert!(t.conferir(&s, agora + 10, errado).is_err());
        }
        assert_eq!(t.espera(agora + 10), ESPERA_INICIAL_S);
        let proximo = totp::codigo(&segredo, agora + 30);
        assert!(t.conferir(&s, agora + 30, &proximo).unwrap_err().contains("espere"));
        // passada a espera, o código certo do momento vale e zera a conta
        let depois = agora + 10 + ESPERA_INICIAL_S + 30;
        assert!(t.conferir(&s, depois, &totp::codigo(&segredo, depois)).is_ok());
        assert_eq!(t.espera(depois), 0);
    }

    #[test]
    fn segredo_curto_no_arquivo_e_recusado() {
        let p = pasta("curto");
        std::fs::write(p.join(ARQUIVO), format!("totp={}\nexige_envio=1\n", totp::base32(b"abc"))).unwrap();
        assert!(!Seguranca::ler(&p).ligado());
        let _ = std::fs::remove_dir_all(&p);
    }
}
