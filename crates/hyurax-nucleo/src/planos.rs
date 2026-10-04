//! O plano deste nó (`docs/MONETIZACAO.md`).
//!
//! O plano é comprado fora do programa, em reais, de uma empresa; o que chega
//! aqui é um **voucher** assinado pela chave de planos do projeto
//! (`hyurax_nuvem::plano`). O programa confere a assinatura sozinho, sem
//! perguntar a servidor nenhum, e guarda os vouchers em `planos.txt`, na pasta
//! de configuração. Sem voucher valendo, o plano é o Comunidade (grátis).
//!
//! O efeito que já existe: a comissão da plataforma no livro de contas cai
//! para a do plano. Créditos e armazenamento **gerenciados** são da
//! capacidade que o projeto mantém, que confere o mesmo voucher antes de
//! atender (Fase M1 em diante).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hyurax_crypto::PUBKEY_LEN;
use hyurax_nuvem::plano::{self, CATALOGO, Voucher};
use serde_json::{Value, json};

use crate::util::{de_hex, hex};

/// Arquivo dos vouchers, na pasta de configuração.
pub const ARQUIVO: &str = "planos.txt";
/// Mais vouchers guardados do que isto, os vencidos saem primeiro.
const MAXIMO: usize = 32;
/// A chave pública que assina os vouchers (o segredo fica fora do repositório).
const CHAVE_DE_PLANOS: &str = include_str!("../../../rede/chave-de-planos.pub");

/// A chave de planos embutida (`None` num programa montado sem ela).
pub fn chave_do_emissor() -> Option<[u8; PUBKEY_LEN]> {
    let linha = CHAVE_DE_PLANOS.lines().find(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())?;
    de_hex::<PUBKEY_LEN>(linha.trim())
}

/// Os vouchers deste nó.
pub struct Planos {
    arquivo: Option<PathBuf>,
    vouchers: Mutex<Vec<Voucher>>,
}

impl Planos {
    /// Lê `planos.txt` da pasta (sem pasta: só em memória, para teste).
    pub fn abrir(pasta: Option<&Path>) -> Self {
        let arquivo = pasta.map(|p| p.join(ARQUIVO));
        let vouchers = arquivo
            .as_ref()
            .and_then(|a| std::fs::read_to_string(a).ok())
            .map(|t| t.lines().filter_map(|l| Voucher::ler_texto(l).ok()).collect())
            .unwrap_or_default();
        Self { arquivo, vouchers: Mutex::new(vouchers) }
    }

    fn gravar(&self, lista: &[Voucher]) -> Result<(), String> {
        let Some(arquivo) = &self.arquivo else { return Ok(()) };
        let mut texto = String::from("# Vouchers de plano deste nó (docs/MONETIZACAO.md). Sem dado pessoal.\n");
        for v in lista {
            texto.push_str(&v.texto().map_err(|e| e.to_string())?);
            texto.push('\n');
        }
        crate::arquivos::gravar_privado(arquivo, &texto)
    }

    /// Ativa um voucher colado na tela.
    ///
    /// # Errors
    /// Texto que não é voucher, assinatura de outra chave, voucher de outro
    /// nó ou já vencido.
    pub fn ativar(&self, texto: &str, worker: &[u8; PUBKEY_LEN], agora_ms: u64) -> Result<Voucher, String> {
        let emissor = chave_do_emissor().ok_or("este programa foi montado sem a chave de planos")?;
        self.ativar_com(&emissor, texto, worker, agora_ms)
    }

    fn ativar_com(&self, emissor: &[u8; PUBKEY_LEN], texto: &str, worker: &[u8; PUBKEY_LEN], agora_ms: u64) -> Result<Voucher, String> {
        let v = Voucher::ler_texto(texto).map_err(|e| e.to_string())?;
        if !v.assinatura_confere(emissor) {
            return Err("a assinatura não confere: este voucher não foi emitido pelo projeto Hyurax".into());
        }
        if v.beneficiario != *worker {
            return Err("este voucher é de outro computador (o código do worker não confere)".into());
        }
        if v.fim_ms <= agora_ms {
            return Err("este voucher já venceu".into());
        }
        let mut lista = self.vouchers.lock().map_err(|_| "planos travados".to_string())?;
        if !lista.contains(&v) {
            lista.push(v.clone());
        }
        // os vencidos saem quando a lista enche
        if lista.len() > MAXIMO {
            lista.retain(|x| x.fim_ms > agora_ms);
            lista.truncate(MAXIMO);
        }
        self.gravar(&lista)?;
        Ok(v)
    }

    /// O voucher que vale agora para este worker: o de plano maior; empatando,
    /// o que dura mais.
    pub fn atual(&self, worker: &[u8; PUBKEY_LEN], agora_ms: u64) -> Option<Voucher> {
        self.atual_com(&chave_do_emissor()?, worker, agora_ms)
    }

    fn atual_com(&self, emissor: &[u8; PUBKEY_LEN], worker: &[u8; PUBKEY_LEN], agora_ms: u64) -> Option<Voucher> {
        let lista = self.vouchers.lock().ok()?;
        lista.iter().filter(|v| v.vale(emissor, worker, agora_ms)).max_by_key(|v| (v.plano, v.fim_ms)).cloned()
    }

    /// A comissão da plataforma no mercado, em %, para o livro de contas.
    pub fn comissao_pct(&self, worker: &[u8; PUBKEY_LEN], agora_ms: u64) -> u8 {
        let bp = self.atual(worker, agora_ms).map_or_else(|| CATALOGO.first().map_or(1500, |p| p.comissao_bp), |v| v.comissao_bp);
        u8::try_from(bp / 100).unwrap_or(100)
    }

    /// Para a tela e a API.
    pub fn json(&self, worker: &[u8; PUBKEY_LEN], agora_ms: u64) -> Value {
        let atual = self.atual(worker, agora_ms);
        let plano_id = atual.as_ref().map_or(plano::COMUNIDADE, |v| v.plano);
        json!({
            "catalogo": CATALOGO.iter().map(|p| json!({
                "id": p.id,
                "nome": p.nome,
                "mensal_centavos": p.mensal_centavos,
                "anual_centavos": p.anual_centavos,
                "creditos_mes": p.creditos_mes,
                "armazenamento_gib": p.armazenamento_gib,
                "comissao_bp": p.comissao_bp,
            })).collect::<Vec<_>>(),
            "plano_atual": plano_id,
            "voucher": atual.map(|v| json!({
                "plano": v.plano,
                "inicio_ms": v.inicio_ms,
                "fim_ms": v.fim_ms,
                "creditos_mes": v.creditos_mes,
                "armazenamento_gib": v.armazenamento_gib,
                "comissao_bp": v.comissao_bp,
                "serie": v.serie,
            })),
            "worker": hex(worker),
            "comissao_pct": self.comissao_pct(worker, agora_ms),
            "chave_de_planos": chave_do_emissor().is_some(),
            // a venda abre na Fase M1 (CNPJ, conta PJ, provedor de pagamento)
            "venda_aberta": false,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    #[test]
    fn sem_voucher_e_comunidade_e_voucher_de_outra_chave_nao_ativa() {
        let p = Planos::abrir(None);
        let w = [3u8; 32];
        assert!(p.atual(&w, 1).is_none());
        assert_eq!(p.comissao_pct(&w, 1), 15);
        assert_eq!(p.json(&w, 1)["plano_atual"], 0);
        // assinado por uma chave qualquer: recusado, seja qual for a embutida
        let falso = Voucher::emitir(&[7u8; 32], plano::PRO, w, 0, 86_400_000, 1).unwrap();
        let erro = p.ativar(&falso.texto().unwrap(), &w, 10).unwrap_err();
        assert!(erro.contains("não confere") || erro.contains("sem a chave"), "{erro}");
        assert!(p.ativar("hyurax-plano:zz", &w, 10).is_err());
    }

    #[test]
    fn voucher_valido_ativa_grava_e_vence() {
        let pasta = std::env::temp_dir().join(format!("hyurax-planos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(&pasta).unwrap();
        let segredo = [9u8; 32];
        let emissor = hyurax_crypto::ed25519_public_key(&segredo);
        let (w, outro) = ([3u8; 32], [4u8; 32]);
        let dia = 86_400_000;
        let pro = Voucher::emitir(&segredo, plano::PRO, w, 1_000, 1_000 + 30 * dia, 1).unwrap();
        let equipe = Voucher::emitir(&segredo, plano::EQUIPE, w, 1_000, 1_000 + 10 * dia, 2).unwrap();
        let p = Planos::abrir(Some(&pasta));
        assert!(p.ativar_com(&emissor, &pro.texto().unwrap(), &outro, 2_000).unwrap_err().contains("outro computador"));
        assert!(p.ativar_com(&emissor, &pro.texto().unwrap(), &w, 1_000 + 30 * dia).unwrap_err().contains("venceu"));
        p.ativar_com(&emissor, &pro.texto().unwrap(), &w, 2_000).unwrap();
        p.ativar_com(&emissor, &equipe.texto().unwrap(), &w, 2_000).unwrap();
        // o maior plano vale; quando ele vence, volta o Pro; depois, o grátis
        assert_eq!(p.atual_com(&emissor, &w, 2_000).unwrap().plano, plano::EQUIPE);
        assert_eq!(p.atual_com(&emissor, &w, 1_000 + 20 * dia).unwrap().plano, plano::PRO);
        assert!(p.atual_com(&emissor, &w, 1_000 + 30 * dia).is_none());
        // gravado: reabrir lê os mesmos vouchers
        let de_novo = Planos::abrir(Some(&pasta));
        assert_eq!(de_novo.atual_com(&emissor, &w, 2_000).unwrap().plano, plano::EQUIPE);
        let _ = std::fs::remove_dir_all(pasta);
    }
}
