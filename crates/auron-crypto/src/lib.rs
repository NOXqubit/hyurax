// ✝ Lucas 21:28 — “Quando essas coisas começarem a acontecer, olhai para cima e levantai a vossa cabeça, porque a vossa redenção está próxima.”
//! AACL — Auron Adaptive Cryptographic Layer.
//!
//! Tradução de `reference/auron/crypto.py`. Cada saída é conferida byte a
//! byte contra `vectors/hash.json` e `vectors/crypto_ed25519.json`, gerados
//! pela implementação Python. Um módulo só está migrado quando cada vetor
//! bate; compilar não prova nada.
//!
//! | Identificador    | Uso                                     |
//! |------------------|-----------------------------------------|
//! | `HASH-SHA512-V1` | hash geral, identificadores, Merkle, XOF |
//! | `SIG-ED25519-V1` | assinatura de transação                 |
//! | `SIG-BP512-V1`   | legado do protótipo, **não utilizável** |

#![forbid(unsafe_code)]
// Em teste, entrar em pânico é como se sinaliza falha. Os lints que proíbem
// isso valem para o código de produção, não para a suíte.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha512};

/// Hash geral, identificadores e Merkle.
pub const HASH_SHA512_V1: &str = "HASH-SHA512-V1";

/// Assinatura padrão, RFC 8032.
pub const SIG_ED25519_V1: &str = "SIG-ED25519-V1";

/// Brainpool P-512 do protótipo. Continua registrado só para que o
/// identificador nunca seja reaproveitado com outro significado.
pub const SIG_BP512_V1: &str = "SIG-BP512-V1";

/// Bytes de um hash `HASH-SHA512-V1`.
pub const HASH_LEN: usize = 64;

/// Bytes de um segredo Ed25519.
pub const SECRET_LEN: usize = 32;

/// Bytes de uma chave pública Ed25519.
pub const PUBKEY_LEN: usize = 32;

/// Bytes de uma assinatura Ed25519.
pub const SIGNATURE_LEN: usize = 64;

/// Bytes de um endereço.
pub const ADDRESS_LEN: usize = 20;

// O endereço é um prefixo do hash. Se alguém mudar um dos dois tamanhos, o
// build quebra aqui, em vez de o endereço sair truncado em silêncio.
const _: () = assert!(ADDRESS_LEN <= HASH_LEN);

/// `HASH-SHA512-V1`: o `H` da especificação.
pub fn sha512(dados: &[u8]) -> [u8; HASH_LEN] {
    Sha512::digest(dados).into()
}

/// Expansão determinística em modo contador.
///
/// O bloco `i` é `SHA-512(domain || seed || i)`, com `i` em `u64` big-endian
/// a partir de zero, e a saída é a concatenação dos blocos truncada em `n`
/// bytes.
///
/// A ordem `domain` antes de `seed` é contrato. A ordem trocada produz bytes
/// tão aleatórios quanto os certos, e nenhum teste de sanidade distinguiria
/// os dois; só os vetores pegam.
///
/// O Python recusa `n` negativo em tempo de execução. Aqui o tipo já impede.
pub fn xof(seed: &[u8], n: usize, domain: &[u8]) -> Vec<u8> {
    let mut saida = Vec::with_capacity(n);
    // O contador não chega perto de dar a volta: seriam 2^64 blocos de 64
    // bytes, muito além do que um `Vec` consegue guardar.
    for contador in 0_u64.. {
        if saida.len() >= n {
            break;
        }
        let bloco = Sha512::new()
            .chain_update(domain)
            .chain_update(seed)
            .chain_update(contador.to_be_bytes())
            .finalize();
        let falta = n.saturating_sub(saida.len());
        saida.extend(bloco.iter().take(falta));
    }
    saida
}

/// Chave pública Ed25519 a partir do segredo de 32 bytes, RFC 8032.
pub fn ed25519_public_key(segredo: &[u8; SECRET_LEN]) -> [u8; PUBKEY_LEN] {
    SigningKey::from_bytes(segredo).verifying_key().to_bytes()
}

/// Assinatura Ed25519, RFC 8032.
///
/// Determinística por construção: o mesmo segredo sobre a mesma mensagem dá
/// sempre os mesmos 64 bytes. Não há nonce aleatório que possa vazar a chave.
pub fn ed25519_sign(segredo: &[u8; SECRET_LEN], mensagem: &[u8]) -> [u8; SIGNATURE_LEN] {
    SigningKey::from_bytes(segredo).sign(mensagem).to_bytes()
}

/// Endereço de uma chave Ed25519: os primeiros 20 bytes de
/// `SHA-512("SIG-ED25519-V1" || chave_publica)`.
///
/// O identificador do algoritmo entra no hash de propósito. A mesma chave sob
/// algoritmos diferentes precisa dar endereços diferentes; senão, trocar de
/// algoritmo no futuro cria colisão de identidade.
pub fn address_from_ed25519_pubkey(chave_publica: &[u8; PUBKEY_LEN]) -> [u8; ADDRESS_LEN] {
    let hash = Sha512::new()
        .chain_update(SIG_ED25519_V1.as_bytes())
        .chain_update(chave_publica)
        .finalize();
    let mut endereco = [0_u8; ADDRESS_LEN];
    endereco
        .iter_mut()
        .zip(hash.iter())
        .for_each(|(destino, origem)| *destino = *origem);
    endereco
}

/// Verificação Ed25519 com exatamente o conjunto de aceitação do oráculo.
///
/// Devolve `false` para qualquer entrada inválida, inclusive tamanho errado.
/// Nunca devolve erro nem entra em pânico: no consenso, "assinatura ruim" e
/// "entrada malformada" precisam dar o mesmo resultado em todo nó.
///
/// O `verify` do ed25519-dalek, sozinho, NÃO reproduz o oráculo. Medido com o
/// dalek compilado, contra testemunhas geradas pelo Python:
///
/// - Equação sem cofator, `S >= L` recusado, `R` não-canônico recusado: o
///   dalek já concorda com o Python.
/// - Chave pública não-canônica (`y >= p`, ou bit de sinal ligado com
///   `x = 0`): o Python recusa e o dalek aceita. São 26 encodings, e os de
///   ordem pequena deixam forjar assinatura sem segredo nenhum. Fechado aqui
///   pela ida e volta de encoding: a chave só vale se recomprimir nos mesmos
///   bytes que chegaram.
///
/// Ponto de ordem pequena em `A` ou em `R` é aceito, igual ao oráculo: é a
/// regra da AURON-SPEC-01, seção 2, decidida em 10/09/2026. Por isso nunca
/// trocar por `verify_strict`, que os recusa, e o nó passaria a rejeitar
/// transações que a especificação considera válidas. Nem por `verify_batch`,
/// que aceita `R` não-canônico.
///
/// `vectors/crypto_ed25519_verify.json` trava as duas coisas.
pub fn ed25519_verify(chave_publica: &[u8], mensagem: &[u8], assinatura: &[u8]) -> bool {
    let Ok(chave) = <[u8; PUBKEY_LEN]>::try_from(chave_publica) else {
        return false;
    };
    let Ok(assinatura) = <[u8; SIGNATURE_LEN]>::try_from(assinatura) else {
        return false;
    };
    let Ok(verificadora) = VerifyingKey::from_bytes(&chave) else {
        return false;
    };
    if verificadora.to_edwards().compress().as_bytes() != &chave {
        return false;
    }
    verificadora
        .verify(mensagem, &Signature::from_bytes(&assinatura))
        .is_ok()
}

#[cfg(test)]
mod testes {
    use super::*;

    /// RFC 8032, seção 7.1, testes 1 a 3: segredo, chave pública, mensagem e
    /// assinatura. É a mesma lista que `reference/tests/test_01_crypto.py`
    /// usa para validar o oráculo, então os dois lados batem com o padrão
    /// externo, e não só um com o outro.
    const RFC8032: [(&str, &str, &str, &str); 3] = [
        (
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "",
            concat!(
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555",
                "fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
            ),
        ),
        (
            "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "72",
            concat!(
                "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da0",
                "85ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00"
            ),
        ),
        (
            "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            "af82",
            concat!(
                "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac",
                "18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a"
            ),
        ),
    ];

    #[test]
    fn rfc8032_secao_7_1() {
        for (i, (segredo, publica, mensagem, assinatura)) in RFC8032.iter().enumerate() {
            let segredo: [u8; SECRET_LEN] = hex::decode(segredo).unwrap().try_into().unwrap();
            let mensagem = hex::decode(mensagem).unwrap();
            assert_eq!(
                hex::encode(ed25519_public_key(&segredo)),
                *publica,
                "RFC 8032 teste {}: chave pública",
                i + 1
            );
            assert_eq!(
                hex::encode(ed25519_sign(&segredo, &mensagem)),
                *assinatura,
                "RFC 8032 teste {}: assinatura",
                i + 1
            );
        }
    }

    #[test]
    fn xof_de_tamanho_zero_nao_gera_nada() {
        assert!(xof(b"semente", 0, b"DOM").is_empty());
    }

    #[test]
    fn xof_separa_dominio() {
        assert_ne!(xof(b"semente", 64, b"A"), xof(b"semente", 64, b"B"));
    }

    #[test]
    fn xof_curto_e_prefixo_do_longo() {
        // Truncar precisa ser só truncar: pedir menos bytes não pode mudar os
        // primeiros, senão dois nós que pedem tamanhos diferentes divergem.
        let longo = xof(b"semente", 200, b"DOM");
        assert_eq!(xof(b"semente", 70, b"DOM"), &longo[..70]);
    }

    #[test]
    fn endereco_amarra_o_algoritmo() {
        let publica = ed25519_public_key(&[0x11; SECRET_LEN]);
        let sem_algoritmo = &sha512(&publica)[..ADDRESS_LEN];
        assert_ne!(address_from_ed25519_pubkey(&publica).as_slice(), sem_algoritmo);
    }

    /// Testemunhas geradas contra o oráculo Python e medidas contra o dalek
    /// compilado. O oráculo recusa todas, e continua recusando qualquer que
    /// seja a decisão sobre pontos de ordem pequena. As quatro de chave
    /// não-canônica são as que o `verify` do dalek, sozinho, ACEITA: se alguém
    /// tirar a checagem de ida e volta, este teste quebra.
    const RECUSAS: [(&str, &str, &str, &str); 14] = [
        ("mensagem trocada", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737", "7472616e73666572656e636961206465205445535445", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f460868c8a6e0959dd6a0390af93af039dccb6ddb1af77721194fd37fe8d6da1003"),
        ("R adulterado em 1 bit", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737", "7472616e73666572656e636961206465207465737465", "8d3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f460868c8a6e0959dd6a0390af93af039dccb6ddb1af77721194fd37fe8d6da1003"),
        ("s + L (maleavel)", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737", "7472616e73666572656e636961206465207465737465", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f46f53bbe03fbf8af2e77d6019c19ea18f1cb6ddb1af77721194fd37fe8d6da1013"),
        ("s = L", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737", "7472616e73666572656e636961206465207465737465", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f46edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010"),
        ("R identidade nao-canonica (y=p+1)", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737", "7472616e73666572656e636961206465207465737465", "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f9e393cee0c9cdadddb63575def59d4d71dc41ec8e0cc955514c3236578c87009"),
        ("A identidade y=p+1, forjada", "eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f", "7472616e73666572656e636961206465207465737465", "58666666666666666666666666666666666666666666666666666666666666660100000000000000000000000000000000000000000000000000000000000000"),
        ("A identidade c/ bit de sinal, forjada", "0100000000000000000000000000000000000000000000000000000000000080", "7472616e73666572656e636961206465207465737465", "58666666666666666666666666666666666666666666666666666666666666660100000000000000000000000000000000000000000000000000000000000000"),
        ("A ordem 2 c/ bit de sinal, forjada", "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "7472616e73666572656e636961206465207465737465", "c9a3f86aae465f0e56513864510f3997561fa2c9e85ea21dc2292309f3cd60220200000000000000000000000000000000000000000000000000000000000000"),
        ("A ordem 4 y=p+0, forjada", "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f", "7472616e73666572656e636961206465207465737465", "d4b4f5784868c3020403246717ec169ff79e26608ea126a1ab69ee77d1b167120300000000000000000000000000000000000000000000000000000000000000"),
        ("A fora da curva (y=2)", "0200000000000000000000000000000000000000000000000000000000000000", "7472616e73666572656e636961206465207465737465", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f460868c8a6e0959dd6a0390af93af039dccb6ddb1af77721194fd37fe8d6da1003"),
        ("assinatura de 63 bytes", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737", "7472616e73666572656e636961206465207465737465", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f460868c8a6e0959dd6a0390af93af039dccb6ddb1af77721194fd37fe8d6da10"),
        ("assinatura de 65 bytes", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737", "7472616e73666572656e636961206465207465737465", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f460868c8a6e0959dd6a0390af93af039dccb6ddb1af77721194fd37fe8d6da100300"),
        ("chave de 31 bytes", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c97787", "7472616e73666572656e636961206465207465737465", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f460868c8a6e0959dd6a0390af93af039dccb6ddb1af77721194fd37fe8d6da1003"),
        ("chave de 33 bytes", "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c977873700", "7472616e73666572656e636961206465207465737465", "8c3882b60df75b530f43f78ff103e50aa4be599f67eebb12d32fd903cc9d2f460868c8a6e0959dd6a0390af93af039dccb6ddb1af77721194fd37fe8d6da1003"),
    ];

    #[test]
    fn recusa_o_que_o_oraculo_recusa() {
        for (rotulo, publica, mensagem, assinatura) in RECUSAS {
            let aceitou = ed25519_verify(
                &hex::decode(publica).unwrap(),
                &hex::decode(mensagem).unwrap(),
                &hex::decode(assinatura).unwrap(),
            );
            assert!(!aceitou, "{rotulo}: o oráculo recusa e o Rust aceitou");
        }
    }
}
