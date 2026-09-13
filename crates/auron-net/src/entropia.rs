//! Aleatoriedade para chaves e para a cifra, sem biblioteca externa.
//!
//! O `getrandom` não compila no alvo Windows GNU desta máquina (exige
//! `dlltool`), então a entropia vem direto do sistema operacional:
//!
//! - Linux e Android (Termux): `/dev/urandom`;
//! - Windows: `RandomNumberGenerator` do .NET, pelo PowerShell que vem em todo
//!   Windows.
//!
//! Ler do sistema é caro no Windows (sobe um processo), então isso acontece uma
//! vez por processo. Depois, cada pedido de bytes sai de SHA-512 sobre a
//! semente, um contador que nunca repete e o relógio. Sem a semente, ninguém
//! prevê a saída; com contador único, duas saídas nunca coincidem.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use auron_crypto::sha512;

/// Domínio do gerador: separa estes bytes de qualquer outro uso de SHA-512.
const DOMINIO: &[u8] = b"AURON-NET-RNG-v1";

/// Lê 32 bytes de entropia do sistema operacional.
///
/// # Errors
/// Quando o sistema não entrega entropia. Nunca cai para uma fonte fraca.
#[cfg(unix)]
pub fn entropia_do_sistema() -> Result<[u8; 32], String> {
    use std::io::Read;
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("não consegui ler /dev/urandom: {e}"))?;
    confere(bytes)
}

/// Lê 32 bytes de entropia do sistema operacional.
///
/// # Errors
/// Quando o sistema não entrega entropia. Nunca cai para uma fonte fraca.
#[cfg(windows)]
pub fn entropia_do_sistema() -> Result<[u8; 32], String> {
    let saida = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$b = [byte[]]::new(32); [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($b); ($b | ForEach-Object { $_.ToString('x2') }) -join ''",
        ])
        .output()
        .map_err(|e| format!("não consegui chamar o PowerShell: {e}"))?;
    let texto = String::from_utf8_lossy(&saida.stdout);
    let texto = texto.trim();
    if texto.len() != 64 {
        return Err("o PowerShell não devolveu 32 bytes".into());
    }
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        let par = texto.get(i.saturating_mul(2)..i.saturating_mul(2).saturating_add(2)).ok_or("entropia malformada")?;
        *b = u8::from_str_radix(par, 16).map_err(|_| "entropia malformada")?;
    }
    confere(bytes)
}

fn confere(bytes: [u8; 32]) -> Result<[u8; 32], String> {
    if bytes == [0u8; 32] {
        return Err("entropia zerada; recusando".into());
    }
    Ok(bytes)
}

fn semente() -> Result<&'static [u8; 32], String> {
    static SEMENTE: OnceLock<Result<[u8; 32], String>> = OnceLock::new();
    SEMENTE.get_or_init(entropia_do_sistema).as_ref().map_err(Clone::clone)
}

/// Enche `destino` com bytes imprevisíveis.
///
/// # Errors
/// Quando a entropia do sistema não pôde ser lida.
pub fn preencher(destino: &mut [u8]) -> Result<(), String> {
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let base = semente()?;
    for pedaco in destino.chunks_mut(32) {
        let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let mut entrada = Vec::with_capacity(72);
        entrada.extend_from_slice(DOMINIO);
        entrada.extend_from_slice(base);
        entrada.extend_from_slice(&n.to_be_bytes());
        entrada.extend_from_slice(&nanos.to_be_bytes());
        let bloco = sha512(&entrada);
        let parte = bloco.get(..pedaco.len()).ok_or("pedaço maior que o bloco")?;
        pedaco.copy_from_slice(parte);
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn duas_saidas_nunca_coincidem() {
        let (mut a, mut b) = ([0u8; 64], [0u8; 64]);
        preencher(&mut a).unwrap_or_default();
        preencher(&mut b).unwrap_or_default();
        assert_ne!(a, [0u8; 64], "gerador devolveu zeros");
        assert_ne!(a, b, "duas saídas iguais");
    }
}
