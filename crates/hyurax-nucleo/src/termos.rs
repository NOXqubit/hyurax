// ✝ Mateus 5:37 — “Seja, porém, o vosso falar: Sim, sim; não, não.”
//! Os termos de uso: o texto (em `docs/TERMOS-DE-USO.md`), a versão e o aceite.
//!
//! O instalador e o programa mostram o mesmo texto e gravam o aceite em
//! `PASTA/termos.txt`, com a versão e o horário. Subir [`VERSAO`] faz o
//! programa pedir o aceite de novo.

use std::fmt::Write as _;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// O texto, em Markdown, com um comentário HTML no começo (que não aparece).
pub const TEXTO: &str = include_str!("../../../docs/TERMOS-DE-USO.md");
/// A versão dos termos. Mudança relevante no texto = versão nova.
pub const VERSAO: u32 = 2;

const ARQUIVO: &str = "termos.txt";

/// O aceite gravado vale para a versão atual?
pub fn aceitos(pasta: &Path) -> bool {
    std::fs::read_to_string(pasta.join(ARQUIVO)).is_ok_and(|t| {
        t.lines()
            .find_map(|l| l.trim().strip_prefix("versao="))
            .and_then(|v| v.trim().parse::<u32>().ok())
            .is_some_and(|v| v >= VERSAO)
    })
}

/// Grava o aceite da versão atual.
pub fn aceitar(pasta: &Path, onde: &str) -> Result<(), String> {
    std::fs::create_dir_all(pasta).map_err(|e| format!("{}: {e}", pasta.display()))?;
    let agora = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let texto = format!(
        "# Aceite dos termos de uso do Hyurax. Apagar faz o programa perguntar de novo.\nversao={VERSAO}\naceito_em={agora}\nonde={onde}\n"
    );
    std::fs::write(pasta.join(ARQUIVO), texto).map_err(|e| format!("não consegui gravar o aceite: {e}"))
}

fn escapar(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `**negrito**` e `` `código` `` dentro de uma linha já escapada.
fn em_linha(t: &str) -> String {
    let mut saida = String::with_capacity(t.len());
    let mut negrito = false;
    let mut codigo = false;
    let mut resto = t;
    while !resto.is_empty() {
        if let Some(depois) = resto.strip_prefix("**") {
            saida.push_str(if negrito { "</strong>" } else { "<strong>" });
            negrito = !negrito;
            resto = depois;
        } else if let Some(depois) = resto.strip_prefix('`') {
            saida.push_str(if codigo { "</code>" } else { "<code>" });
            codigo = !codigo;
            resto = depois;
        } else {
            let mut letras = resto.chars();
            if let Some(c) = letras.next() {
                saida.push(c);
            }
            resto = letras.as_str();
        }
    }
    if codigo {
        saida.push_str("</code>");
    }
    if negrito {
        saida.push_str("</strong>");
    }
    saida
}

/// Um item de lista, juntado inteiro antes de converter: negrito que
/// atravessa a quebra de linha fecha no lugar certo.
#[derive(Default)]
struct Item {
    texto: Vec<String>,
    subitens: Vec<Vec<String>>,
}

impl Item {
    fn escrever(&mut self, h: &mut String) {
        if self.texto.is_empty() && self.subitens.is_empty() {
            return;
        }
        let _ = write!(h, "<li>{}", em_linha(&self.texto.join(" ")));
        if !self.subitens.is_empty() {
            h.push_str("<ul>");
            for s in &self.subitens {
                let _ = write!(h, "<li>{}</li>", em_linha(&s.join(" ")));
            }
            h.push_str("</ul>");
        }
        h.push_str("</li>");
        self.texto.clear();
        self.subitens.clear();
    }
}

/// Os termos em HTML simples: títulos, parágrafos, listas (com um nível de
/// sub-lista), negrito e código. Só o que o texto usa; nada vem de fora,
/// então não há HTML de terceiros.
pub fn html() -> String {
    let corpo = match TEXTO.find("-->") {
        Some(fim) => TEXTO.get(fim.saturating_add(3)..).unwrap_or(TEXTO),
        None => TEXTO,
    };
    let mut h = String::with_capacity(corpo.len().saturating_mul(2));
    let mut paragrafo: Vec<String> = Vec::new();
    let mut em_lista = false;
    let mut item = Item::default();
    let fechar_paragrafo = |h: &mut String, p: &mut Vec<String>| {
        if !p.is_empty() {
            let _ = write!(h, "<p>{}</p>", em_linha(&p.join(" ")));
            p.clear();
        }
    };
    for linha in corpo.lines() {
        let l = linha.trim_end();
        let recuo = l.len().saturating_sub(l.trim_start().len());
        let escapada = escapar(l.trim_start());
        if recuo == 0
            && let Some(texto) = escapada.strip_prefix("- ")
        {
            fechar_paragrafo(&mut h, &mut paragrafo);
            if em_lista {
                item.escrever(&mut h);
            } else {
                h.push_str("<ul>");
                em_lista = true;
            }
            item.texto.push(texto.to_string());
            continue;
        }
        if em_lista && recuo >= 2 && !escapada.is_empty() {
            match escapada.strip_prefix("- ") {
                // sub-item
                Some(texto) if recuo < 4 => item.subitens.push(vec![texto.to_string()]),
                _ => match item.subitens.last_mut() {
                    // continua o sub-item
                    Some(s) if recuo >= 4 => s.push(escapada),
                    // continua o item
                    _ => item.texto.push(escapada),
                },
            }
            continue;
        }
        if em_lista {
            item.escrever(&mut h);
            h.push_str("</ul>");
            em_lista = false;
        }
        if let Some(t) = escapada.strip_prefix("## ") {
            fechar_paragrafo(&mut h, &mut paragrafo);
            let _ = write!(h, "<h3>{}</h3>", em_linha(t));
        } else if let Some(t) = escapada.strip_prefix("# ") {
            fechar_paragrafo(&mut h, &mut paragrafo);
            let _ = write!(h, "<h2>{}</h2>", em_linha(t));
        } else if escapada.is_empty() {
            fechar_paragrafo(&mut h, &mut paragrafo);
        } else {
            paragrafo.push(escapada);
        }
    }
    fechar_paragrafo(&mut h, &mut paragrafo);
    if em_lista {
        item.escrever(&mut h);
        h.push_str("</ul>");
    }
    h
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn negrito_atravessa_a_linha_e_sub_lista_vira_lista() {
        let h = html();
        assert!(!h.contains("**"), "nenhum asterisco cru");
        assert_eq!(h.matches("<strong>").count(), h.matches("</strong>").count());
        assert!(h.contains("<strong>Não são dinheiro, não são HYX e não dão direito a pagamento, recompensa ou participação</strong>"));
        assert!(h.contains("<ul><li>perda de dados;</li>"), "a lista de perdas é uma sub-lista");
    }

    #[test]
    fn o_html_tem_o_essencial_e_nao_tem_o_comentario() {
        let h = html();
        assert!(h.contains("<h2>Termos de uso e isenção de responsabilidade</h2>"));
        assert!(h.contains("<strong>não tem valor econômico</strong>"));
        assert!(h.contains("<code>%APPDATA%\\Hyurax</code>"));
        assert!(!h.contains("advogado"), "o comentário interno não aparece");
        assert!(!h.contains("<script"), "nada executável");
        assert!(h.matches("<h3>").count() >= 12);
    }

    #[test]
    fn aceite_vale_so_para_a_versao_atual() {
        let pasta = std::env::temp_dir().join(format!("hyurax-termos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        assert!(!aceitos(&pasta));
        aceitar(&pasta, "teste").unwrap();
        assert!(aceitos(&pasta));
        std::fs::write(pasta.join(ARQUIVO), "versao=0\n").unwrap();
        assert!(!aceitos(&pasta), "versão velha pede aceite de novo");
        let _ = std::fs::remove_dir_all(pasta);
    }
}
