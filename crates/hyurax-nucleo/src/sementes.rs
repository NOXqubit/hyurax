//! Onde um nó recém-aberto procura a rede.
//!
//! O problema de sempre numa rede sem dono: para entrar, é preciso conhecer
//! alguém que já está dentro. O Hyurax procura em três lugares, nesta ordem, e
//! para no primeiro que der alguma coisa:
//!
//! 1. **`PASTA/sementes.txt`** — a lista do dono da máquina. Um endereço por
//!    linha. Nada sai do computador para ler isto.
//! 2. **A lista embutida no programa** ([`crate::SEMENTES_TESTNET`]).
//! 3. **A lista publicada no repositório do projeto**, buscada com o `curl` do
//!    sistema. É a única coisa que o nó de terminal busca na internet, acontece
//!    só quando as duas de cima vieram vazias, e `--sem-sementes-padrao`
//!    desliga.
//!
//! O passo 3 existe para o lançamento não depender de servidor pago nem de
//! versão nova do programa: quem põe um nó no ar manda o endereço, a linha
//! entra num arquivo de texto do repositório, e todo mundo que abrir o programa
//! amanhã já acha a rede. É o mesmo papel que as "DNS seeds" fazem no Bitcoin,
//! com a diferença de estar num arquivo que qualquer um lê e confere.
//!
//! O que vem de lá **não é confiança**: é só um endereço para bater. Quem
//! atender ainda precisa passar pelo aperto de mão cifrado, falar a mesma rede
//! e provar cada bloco que mandar. Uma lista adulterada faz o nó perder tempo
//! com um endereço morto, não faz ele aceitar cadeia errada.

use std::path::Path;

/// Onde a lista pública mora. Arquivo de texto no próprio repositório: sem
/// servidor para pagar, e o histórico de quem entrou fica no git.
const URL_PUBLICADA: &str = "https://raw.githubusercontent.com/NOXqubit/hyurax/main/rede/sementes-testnet.txt";
/// Nome do arquivo de sementes dentro da pasta de dados.
pub const ARQUIVO: &str = "sementes.txt";
/// Teto de endereços que qualquer fonte pode dar.
pub const MAXIMO: usize = 16;

/// `host:porta`, com porta de 1 a 65535 e host sem espaço nem caractere
/// estranho. Serve para IPv4, nome de domínio e IPv6 entre colchetes.
#[must_use]
pub fn valida(s: &str) -> bool {
    // semente atrás de HTTPS: wss://nome/p2p (ver docs/NO-SEMENTE.md)
    if s.starts_with("ws://") || s.starts_with("wss://") {
        return hyurax_net::endereco_websocket_valido(s);
    }
    let Some((host, porta)) = s.rsplit_once(':') else { return false };
    !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'))
        && porta.parse::<u16>().is_ok_and(|p| p > 0)
}

/// Lê uma lista de endereços de um texto: um por linha, `#` começa comentário.
///
/// Linha inválida é ignorada em silêncio de propósito — a lista é pública e um
/// caractere torto de alguém não pode derrubar o nó de todo mundo.
#[must_use]
pub fn ler_lista(texto: &str) -> Vec<String> {
    texto
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| valida(l))
        .map(String::from)
        .take(MAXIMO)
        .collect()
}

/// A lista que o dono da máquina escreveu em `PASTA/sementes.txt`.
#[must_use]
pub fn do_arquivo(pasta: &Path) -> Vec<String> {
    std::fs::read_to_string(pasta.join(ARQUIVO)).map(|t| ler_lista(&t)).unwrap_or_default()
}

/// Busca a lista publicada no repositório.
///
/// Usa o `curl` que já vem no Windows 10 e 11, no macOS e em quase todo Linux,
/// em vez de uma biblioteca de HTTPS — a regra do projeto é dependência só em
/// Rust puro, e TLS em Rust puro ainda arrasta compilador C. Sem `curl`, ou sem
/// internet, devolve lista vazia e o nó segue a vida: quem tiver `--semente`
/// ou um par na rede local entra do mesmo jeito.
///
/// # Errors
/// Falha de rede, `curl` ausente ou resposta grande demais.
pub fn publicadas() -> Result<Vec<String>, String> {
    let saida = std::process::Command::new("curl")
        .args(["-fsS", "--max-time", "12", "--max-filesize", "4096", URL_PUBLICADA])
        .output()
        .map_err(|e| format!("curl não abriu: {e}"))?;
    if !saida.status.success() {
        return Err(String::from_utf8_lossy(&saida.stderr).trim().to_string());
    }
    if saida.stdout.len() > 4096 {
        return Err("resposta grande demais para uma lista de sementes".into());
    }
    Ok(ler_lista(&String::from_utf8_lossy(&saida.stdout)))
}

/// O endereço de onde a lista pública vem, para o programa poder dizer na tela.
#[must_use]
pub fn url_publicada() -> &'static str {
    URL_PUBLICADA
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    #[test]
    fn semente_por_websocket() {
        assert!(valida("wss://hyurax-semente.onrender.com/p2p"));
        assert!(valida("ws://127.0.0.1:9000/p2p"));
        assert!(!valida("wss:///p2p"));
        assert!(!valida("https://hyurax-semente.onrender.com/p2p"));
        assert!(!valida("wss://a b/p2p"));
        let lista = ler_lista("# comentário
wss://s.onrender.com/p2p
203.0.113.7:8790
lixo
");
        assert_eq!(lista, vec!["wss://s.onrender.com/p2p".to_string(), "203.0.113.7:8790".to_string()]);
    }

    #[test]
    fn so_aceita_host_e_porta() {
        assert!(valida("203.0.113.7:8790"));
        assert!(valida("semente.hyurax.com:8790"));
        assert!(valida("[2001:db8::1]:8790"));
        assert!(!valida("203.0.113.7"));
        assert!(!valida("203.0.113.7:0"));
        assert!(!valida("203.0.113.7:99999"));
        assert!(!valida("host com espaço:8790"));
        assert!(!valida(":8790"));
        assert!(!valida(""));
    }

    #[test]
    fn le_a_lista_com_comentario_e_lixo() {
        let texto = "\
# Sementes da testnet do Hyurax.
203.0.113.7:8790
  198.51.100.9:8790   # o nó do fulano

linha torta que não é endereço
203.0.113.10:8790#sem espaço antes do comentário
";
        assert_eq!(ler_lista(texto), ["203.0.113.7:8790", "198.51.100.9:8790", "203.0.113.10:8790"]);
    }

    #[test]
    fn uma_lista_enorme_e_cortada() {
        let texto = (0..100).map(|i| format!("203.0.113.{i}:8790\n")).collect::<String>();
        assert_eq!(ler_lista(&texto).len(), MAXIMO);
    }

    #[test]
    fn arquivo_que_nao_existe_e_lista_vazia() {
        assert!(do_arquivo(Path::new("pasta-que-nao-existe-mesmo")).is_empty());
    }

    #[test]
    fn a_lista_publicada_mora_no_repositorio_do_projeto() {
        // Se o endereço mudar, é decisão consciente: o programa inteiro procura
        // a rede por aqui.
        assert!(url_publicada().starts_with("https://raw.githubusercontent.com/NOXqubit/hyurax/"));
        assert!(url_publicada().ends_with("/rede/sementes-testnet.txt"));
    }
}
