//! Atualização segura: só se instala o que a chave de lançamento do projeto
//! assinou (Documento Mestre §3, §12, §15 e §17).
//!
//! Cada versão publicada leva, ao lado do instalador, um `atualizacao.txt`:
//! versão, rede, nome e endereço do instalador, tamanho e SHA-512, e a
//! assinatura Ed25519 de tudo isso pela chave de lançamento. A chave pública
//! vem embutida no programa (`rede/chave-de-lancamento.pub`).
//!
//! O programa:
//! 1. busca o manifesto da versão mais nova (pelo `curl` do sistema, como a
//!    lista de sementes: TLS em Rust puro ainda arrasta compilador C);
//! 2. confere a assinatura com a chave embutida, e que o endereço é o das
//!    Releases do projeto;
//! 3. baixa o instalador e confere tamanho e SHA-512 contra o manifesto
//!    assinado;
//! 4. só então abre o instalador (que ainda mostra os termos e pede o aceite).
//!
//! Sem chave embutida, nada disso liga: a tela diz PENDENTE.

use std::path::{Path, PathBuf};

use hyurax_crypto::{PUBKEY_LEN, SECRET_LEN, ed25519_public_key, ed25519_sign, ed25519_verify, sha512};

use crate::util::{de_hex, hex};

/// A chave pública de lançamento do projeto, em hexadecimal (vazia: não há).
const CHAVE_DE_LANCAMENTO: &str = include_str!("../../../rede/chave-de-lancamento.pub");
/// De onde vem o manifesto da versão mais nova.
pub const URL_DO_MANIFESTO: &str = "https://github.com/NOXqubit/hyurax/releases/latest/download/atualizacao.txt";
/// Todo instalador vem das Releases do projeto, e de mais nenhum lugar.
pub const PREFIXO_DO_INSTALADOR: &str = "https://github.com/NOXqubit/hyurax/releases/download/";
/// Teto do manifesto e do instalador.
const MANIFESTO_MAX: usize = 16 * 1024;
const INSTALADOR_MAX: u64 = 256 * 1024 * 1024;
/// O formato, na primeira linha.
const FORMATO: &str = concat!(hyurax_identidade::raiz!(), "-ATUALIZACAO-v1");

/// O que o programa sabe da atualização agora.
#[derive(Clone, Debug, Default)]
pub struct Estado {
    /// Quando a última busca terminou (ms).
    pub verificado_ms: u64,
    /// A versão nova, assinada e conferida, se houver.
    pub disponivel: Option<Manifesto>,
    /// O que deu errado na última busca ou instalação.
    pub erro: Option<String>,
    /// Baixando e conferindo o instalador agora.
    pub baixando: bool,
}

/// A chave pública de lançamento embutida, se houver.
pub fn chave_do_projeto() -> Option<[u8; PUBKEY_LEN]> {
    let t: String = CHAVE_DE_LANCAMENTO.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<String>();
    de_hex::<PUBKEY_LEN>(t.trim())
}

/// O que uma versão publicada declara.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifesto {
    pub versao: String,
    pub rede: String,
    pub instalador: String,
    pub endereco: String,
    pub tamanho: u64,
    pub sha512: [u8; 64],
    pub notas: String,
}

fn limpo(t: &str) -> String {
    t.chars().filter(|c| !c.is_control()).take(400).collect()
}

impl Manifesto {
    /// As linhas assinadas, sempre na mesma ordem.
    fn corpo(&self) -> String {
        format!(
            "formato={FORMATO}\nversao={}\nrede={}\ninstalador={}\nendereco={}\ntamanho={}\nsha512={}\nnotas={}\n",
            limpo(&self.versao),
            limpo(&self.rede),
            limpo(&self.instalador),
            limpo(&self.endereco),
            self.tamanho,
            hex(&self.sha512),
            limpo(&self.notas)
        )
    }

    /// O manifesto assinado, pronto para publicar.
    pub fn assinar(&self, segredo: &[u8; SECRET_LEN]) -> String {
        let corpo = self.corpo();
        format!("{corpo}assinatura={}\n", hex(&ed25519_sign(segredo, corpo.as_bytes())))
    }

    /// Lê o manifesto e confere a assinatura com `chave`. Nada além das linhas
    /// assinadas vale; o endereço tem de ser o das Releases do projeto.
    ///
    /// # Errors
    /// Formato, campo, assinatura ou endereço que não conferem.
    pub fn conferir(texto: &str, chave: &[u8; PUBKEY_LEN]) -> Result<Self, String> {
        if texto.len() > MANIFESTO_MAX {
            return Err("manifesto grande demais".into());
        }
        let campo = |nome: &str| -> Result<String, String> {
            texto
                .lines()
                .find_map(|l| l.strip_prefix(nome).and_then(|r| r.strip_prefix('=')))
                .map(str::to_string)
                .ok_or_else(|| format!("manifesto sem o campo {nome}"))
        };
        if campo("formato")? != FORMATO {
            return Err("formato de manifesto desconhecido".into());
        }
        let assinatura: [u8; 64] = de_hex::<64>(&campo("assinatura")?).ok_or("assinatura ilegível")?;
        let m = Self {
            versao: campo("versao")?,
            rede: campo("rede")?,
            instalador: campo("instalador")?,
            endereco: campo("endereco")?,
            tamanho: campo("tamanho")?.parse().map_err(|_| "tamanho ilegível".to_string())?,
            sha512: de_hex::<64>(&campo("sha512")?).ok_or("SHA-512 ilegível")?,
            notas: campo("notas")?,
        };
        // a assinatura cobre exatamente as linhas que este programa leria de novo
        if !ed25519_verify(chave, m.corpo().as_bytes(), &assinatura) {
            return Err("a assinatura do manifesto não confere com a chave de lançamento do projeto".into());
        }
        if !m.endereco.starts_with(PREFIXO_DO_INSTALADOR) || m.endereco.contains(['"', '\'', ' ', '\\']) {
            return Err("o instalador não vem das Releases do projeto".into());
        }
        if m.tamanho == 0 || m.tamanho > INSTALADOR_MAX {
            return Err("tamanho de instalador fora da faixa".into());
        }
        if !m.instalador.ends_with(".exe") || m.instalador.contains(['/', '\\', ':']) {
            return Err("nome de instalador inválido".into());
        }
        Ok(m)
    }
}

/// `a` é versão mais nova que `b`? (números separados por ponto; texto
/// depois de `-` não conta)
pub fn mais_nova(a: &str, b: &str) -> bool {
    let partes = |v: &str| -> Vec<u64> { v.split('-').next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    let (x, y) = (partes(a), partes(b));
    for i in 0..x.len().max(y.len()) {
        let (p, q) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if p != q {
            return p > q;
        }
    }
    false
}

/// Confere um instalador baixado contra o manifesto assinado.
///
/// # Errors
/// Arquivo ilegível, tamanho ou SHA-512 diferentes.
pub fn conferir_arquivo(m: &Manifesto, arquivo: &Path) -> Result<(), String> {
    let bytes = std::fs::read(arquivo).map_err(|e| format!("não consegui ler o instalador baixado: {e}"))?;
    if u64::try_from(bytes.len()).ok() != Some(m.tamanho) {
        return Err(format!("o instalador baixado tem {} bytes; o manifesto assinado diz {}", bytes.len(), m.tamanho));
    }
    if sha512(&bytes) != m.sha512 {
        return Err("o SHA-512 do instalador baixado não é o do manifesto assinado".into());
    }
    Ok(())
}

/// Monta o manifesto de um instalador (para quem publica).
///
/// # Errors
/// Instalador ilegível.
pub fn manifesto_de(instalador: &Path, versao: &str, endereco: &str, notas: &str) -> Result<Manifesto, String> {
    let bytes = std::fs::read(instalador).map_err(|e| format!("não consegui ler {}: {e}", instalador.display()))?;
    let nome = instalador.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or("instalador sem nome")?;
    Ok(Manifesto {
        versao: versao.into(),
        rede: "testnet".into(),
        instalador: nome,
        endereco: endereco.into(),
        tamanho: u64::try_from(bytes.len()).map_err(|_| "instalador grande demais")?,
        sha512: sha512(&bytes),
        notas: notas.into(),
    })
}

/// A chave pública de um segredo de lançamento.
pub fn publica_de(segredo: &[u8; SECRET_LEN]) -> [u8; PUBKEY_LEN] {
    ed25519_public_key(segredo)
}

fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let saida = std::process::Command::new("curl").args(args).output().map_err(|e| format!("curl não abriu: {e}"))?;
    if !saida.status.success() {
        return Err(String::from_utf8_lossy(&saida.stderr).trim().chars().take(300).collect());
    }
    Ok(saida.stdout)
}

/// Busca o manifesto publicado e confere com a chave do projeto.
///
/// # Errors
/// Sem chave embutida, sem rede, ou manifesto que não confere.
pub fn buscar() -> Result<Manifesto, String> {
    let chave = chave_do_projeto().ok_or("este programa foi montado sem a chave de lançamento: atualização automática PENDENTE")?;
    let bytes = curl(&["-fsSL", "--proto", "=https", "--max-time", "20", "--max-filesize", "16384", URL_DO_MANIFESTO])?;
    let texto = String::from_utf8(bytes).map_err(|_| "manifesto que não é texto".to_string())?;
    Manifesto::conferir(&texto, &chave)
}

/// Baixa o instalador para `pasta` e confere. Devolve o caminho.
///
/// # Errors
/// Sem rede, ou arquivo que não confere com o manifesto assinado.
pub fn baixar(m: &Manifesto, pasta: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(pasta).map_err(|e| format!("não consegui criar {}: {e}", pasta.display()))?;
    let destino = pasta.join(&m.instalador);
    let parcial = destino.with_extension("baixando");
    let _ = std::fs::remove_file(&parcial);
    let teto = m.tamanho.to_string();
    let caminho = parcial.display().to_string();
    curl(&["-fsSL", "--proto", "=https", "--max-time", "900", "--max-filesize", &teto, "-o", &caminho, &m.endereco])?;
    if let Err(e) = conferir_arquivo(m, &parcial) {
        let _ = std::fs::remove_file(&parcial);
        return Err(e);
    }
    std::fs::rename(&parcial, &destino).map_err(|e| format!("não consegui guardar o instalador: {e}"))?;
    Ok(destino)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    const SEGREDO: [u8; 32] = [0x42; 32];

    fn exemplo() -> Manifesto {
        Manifesto {
            versao: "1.0.1".into(),
            rede: "testnet".into(),
            instalador: "hyurax-instalador-windows-x86_64.exe".into(),
            endereco: format!("{PREFIXO_DO_INSTALADOR}v1.0.1/hyurax-instalador-windows-x86_64.exe"),
            tamanho: 3,
            sha512: sha512(b"abc"),
            notas: "correções".into(),
        }
    }

    #[test]
    fn manifesto_assinado_confere_e_adulterado_nao() {
        let chave = publica_de(&SEGREDO);
        let texto = exemplo().assinar(&SEGREDO);
        assert_eq!(Manifesto::conferir(&texto, &chave).unwrap(), exemplo());
        // trocar qualquer coisa quebra a assinatura
        for (de, para) in [("versao=1.0.1", "versao=9.9.9"), ("tamanho=3", "tamanho=4"), ("notas=correções", "notas=outra")] {
            assert!(Manifesto::conferir(&texto.replace(de, para), &chave).is_err(), "{de}");
        }
        // outra chave não confere
        assert!(Manifesto::conferir(&texto, &publica_de(&[0x43; 32])).is_err());
    }

    #[test]
    fn instalador_so_das_releases_do_projeto() {
        let mut m = exemplo();
        m.endereco = "https://outro.example/hyurax.exe".into();
        assert!(Manifesto::conferir(&m.assinar(&SEGREDO), &publica_de(&SEGREDO)).unwrap_err().contains("Releases"));
    }

    #[test]
    fn arquivo_baixado_e_conferido_por_tamanho_e_sha512() {
        let p = std::env::temp_dir().join(format!("hyurax-atualizacao-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        let a = p.join("x.exe");
        std::fs::write(&a, b"abc").unwrap();
        assert!(conferir_arquivo(&exemplo(), &a).is_ok());
        std::fs::write(&a, b"abd").unwrap();
        assert!(conferir_arquivo(&exemplo(), &a).unwrap_err().contains("SHA-512"));
        std::fs::write(&a, b"abcd").unwrap();
        assert!(conferir_arquivo(&exemplo(), &a).unwrap_err().contains("bytes"));
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn comparacao_de_versao() {
        assert!(mais_nova("1.0.1", "1.0.0"));
        assert!(mais_nova("1.0.10", "1.0.9"));
        assert!(mais_nova("2.0", "1.9.9"));
        assert!(!mais_nova("1.0.0", "1.0.0"));
        assert!(!mais_nova("0.4.0-teste.1", "1.0.0"));
    }
}
