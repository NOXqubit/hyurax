//! `hyurax-no carteira nova|ver|cifrar`.

use hyurax_consensus::ParametrosRede;
use hyurax_crypto::ADDRESS_LEN;
use hyurax_net::entropia::entropia_do_sistema;
use hyurax_nucleo::arquivos;
use hyurax_nucleo::carteira::{self, arquivo, endereco, senha};
use hyurax_nucleo::util::hex;

use crate::opcoes;

/// Pede a senha nova duas vezes (a variável de ambiente vale como as duas).
pub fn pedir_senha_nova() -> Result<String, String> {
    let s = senha::ler("Senha nova da carteira (mínimo 10 caracteres)")?;
    arquivo::senha_aceitavel(&s)?;
    if std::env::var(senha::VARIAVEL).is_err() && senha::ler("Repita a senha")? != s {
        return Err("as duas senhas não são iguais".into());
    }
    Ok(s)
}

fn mostrar_endereco(e: &[u8; ADDRESS_LEN]) {
    println!("Endereço:          {}", endereco::mostrar(e, ParametrosRede::TESTNET.nome));
    println!("  em hexadecimal:  {}  (sem dígito verificador; prefira o de cima)", hex(e));
}

/// `hyurax-no carteira ...`
pub fn comando(args: &[String]) -> Result<(), String> {
    let (acao, resto) = args.split_first().ok_or("use: carteira nova|ver|cifrar --arquivo ARQUIVO")?;
    let o = opcoes::ler(resto)?;
    let destino = o.arquivo.ok_or("falta --arquivo")?;
    match acao.as_str() {
        "nova" => {
            if destino.exists() {
                return Err(format!("{} já existe; não sobrescrevo carteira", destino.display()));
            }
            let s = pedir_senha_nova()?;
            // a chave vem direto do sistema operacional, não do gerador derivado
            let segredo = entropia_do_sistema()?;
            let conteudo = carteira::cifrar_segredo(&segredo, &s)?;
            carteira::gravar(&destino, &conteudo)?;
            println!("Carteira criada em {}", destino.display());
            mostrar_endereco(&arquivo::endereco(&conteudo)?);
            println!("Guarde uma cópia do arquivo e NÃO esqueça a senha: sem os dois, o saldo fica perdido.");
            Ok(())
        }
        "ver" => {
            let texto = arquivos::ler(&destino)?;
            mostrar_endereco(&arquivo::endereco(&texto)?);
            if arquivo::e_formato_antigo(&texto) {
                println!("Aviso: esta carteira guarda o segredo em texto. Proteja com: hyurax-no carteira cifrar --arquivo {}", destino.display());
            }
            Ok(())
        }
        "cifrar" => {
            let texto = arquivos::ler(&destino)?;
            if !arquivo::e_formato_antigo(&texto) {
                return Err("esta carteira já está cifrada".into());
            }
            let segredo = arquivo::abrir(&texto, "")?;
            let s = pedir_senha_nova()?;
            let conteudo = carteira::cifrar_segredo(&segredo, &s)?;
            carteira::gravar(&destino, &conteudo)?;
            println!("Carteira {} agora está cifrada com senha.", destino.display());
            println!("Se existir cópia antiga do arquivo em outro lugar, apague: ela continua sem senha.");
            Ok(())
        }
        outro => Err(format!("ação desconhecida: {outro}")),
    }
}
