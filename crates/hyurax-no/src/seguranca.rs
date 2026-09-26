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

use crate::totp;

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
        let temporario = arquivo.with_extension("tmp");
        std::fs::write(&temporario, texto).map_err(|e| format!("não consegui gravar {ARQUIVO}: {e}"))?;
        std::fs::rename(&temporario, &arquivo).map_err(|e| format!("não consegui gravar {ARQUIVO}: {e}"))
    }

    /// Existe segundo fator configurado?
    pub fn ligado(&self) -> bool {
        self.segredo.is_some()
    }

    /// O código digitado vale agora?  Sem segredo, nada a conferir: `false`.
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
    fn segredo_curto_no_arquivo_e_recusado() {
        let p = pasta("curto");
        std::fs::write(p.join(ARQUIVO), format!("totp={}\nexige_envio=1\n", totp::base32(b"abc"))).unwrap();
        assert!(!Seguranca::ler(&p).ligado());
        let _ = std::fs::remove_dir_all(&p);
    }
}
