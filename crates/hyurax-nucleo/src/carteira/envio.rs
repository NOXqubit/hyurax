//! Enviar HYX pelo programa, e o histórico da carteira.
//!
//! O terminal já fazia isto (`hyurax-no enviar`). Aqui é o mesmo caminho, com
//! as mesmas conferências, feito pela janela:
//!
//! 1. o endereço de destino, o valor e a taxa são conferidos **antes** de pedir
//!    qualquer coisa pesada;
//! 2. o saldo gastável precisa cobrir valor + taxa (a recompensa de mineração só
//!    libera depois da maturidade, e isso aparece na mensagem de erro);
//! 3. a carteira é aberta com a senha (Argon2id, alguns décimos de segundo) e o
//!    segredo vive só no tempo desta função;
//! 4. a transferência é assinada, entra no mempool deste nó e sai para os pares.
//!
//! Enviar é irreversível: não existe cancelar depois de assinado. Por isso a
//! janela mostra uma tela de conferência antes, e este módulo nunca "arruma"
//! nada por conta própria — endereço estranho é erro, não palpite.

use std::sync::Arc;

use hyurax_chain::Chain;
use hyurax_crypto::ADDRESS_LEN;
use hyurax_net::Rede;
use hyurax_tx::{HYX, Output, Transfer, Tx, sign_transfer_outputs};

use crate::carteira::{arquivo as carteira, endereco};
use crate::util::{hex, hyx, unidades_de_hyx};

/// Taxa sugerida quando o dono não escreve nada: zero.
///
/// A rede de teste não tem disputa por espaço no bloco, e cobrar taxa por
/// enfeite só confundiria quem está aprendendo. Quem quiser, digita.
pub const TAXA_PADRAO: u64 = 0;

/// Um envio já conferido, pronto para assinar.
#[derive(Debug)]
pub struct Pedido {
    pub para: [u8; ADDRESS_LEN],
    pub valor: u64,
    pub taxa: u64,
}

/// O que aconteceu depois de assinar.
pub struct Enviada {
    pub txid: [u8; 64],
    pub nonce: u64,
    pub pares: usize,
}

/// Confere o que veio do formulário, sem tocar na carteira nem na rede.
///
/// # Errors
/// Endereço fora do formato, endereço igual ao próprio, valor zero ou soma que
/// estoura.
pub fn conferir(
    de: &[u8; ADDRESS_LEN],
    para_texto: &str,
    valor_texto: &str,
    taxa_texto: &str,
    nome_da_rede: &str,
) -> Result<Pedido, String> {
    let limpo: String = para_texto.chars().filter(|c| !c.is_whitespace()).collect();
    if limpo.is_empty() {
        return Err("falta o endereço de destino.".into());
    }
    let para: [u8; ADDRESS_LEN] = endereco::ler(&limpo, nome_da_rede)?;
    if para == *de {
        return Err("esse é o seu próprio endereço. Mandar para si mesmo só gastaria a taxa.".into());
    }
    let valor = unidades_de_hyx(valor_texto)?;
    if valor == 0 {
        return Err("o valor precisa ser maior que zero.".into());
    }
    let taxa = if taxa_texto.trim().is_empty() { TAXA_PADRAO } else { unidades_de_hyx(taxa_texto)? };
    valor.checked_add(taxa).ok_or("valor mais taxa passa do total que existe de HYX.")?;
    Ok(Pedido { para, valor, taxa })
}

/// Quanto dá para enviar agora, já descontando a taxa: o que o botão "tudo" usa.
pub fn maximo(saldo: u64, taxa: u64) -> u64 {
    saldo.saturating_sub(taxa)
}

/// Assina e manda. A senha e o segredo não saem desta função.
///
/// # Errors
/// Saldo insuficiente, senha errada, assinatura recusada pela própria validação
/// ou nonce já ocupado no mempool.
pub fn enviar(
    rede: &Arc<Rede>,
    magic: &[u8; 4],
    maturidade: u64,
    texto_da_carteira: &str,
    senha: &str,
    pedido: &Pedido,
) -> Result<Enviada, String> {
    let de = carteira::endereco(texto_da_carteira)?;
    if pedido.para == de {
        return Err("esse é o seu próprio endereço.".into());
    }
    let total = pedido.valor.checked_add(pedido.taxa).ok_or("valor mais taxa estoura")?;
    let (nonce, saldo, altura) = {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        (no.proximo_nonce(&de), no.chain.state.balance(&de, &HYX), no.chain.height())
    };
    if total > saldo {
        return Err(format!(
            "saldo gastável insuficiente na altura {altura}: você tem {} HYX e precisa de {} HYX. \
             A recompensa de mineração só libera depois de {maturidade} blocos.",
            hyx(u128::from(saldo)),
            hyx(u128::from(total))
        ));
    }
    // Só agora a senha é usada: abrir a carteira custa memória e tempo de
    // propósito, e não faz sentido pagar isso para depois descobrir saldo curto.
    let segredo = carteira::abrir(texto_da_carteira, senha)?;
    let saida = Output { recipient: pedido.para, asset_id: HYX, amount: pedido.valor };
    let tx = sign_transfer_outputs(&segredo, magic, de, vec![saida], pedido.taxa, nonce).map_err(|e| e.to_string())?;
    let txid = tx.txid().map_err(|e| e.to_string())?;
    match rede.submeter_tx(tx) {
        Ok(true) => {}
        Ok(false) => return Err("já existe uma transação sua esperando com esse nonce. Espere ela entrar num bloco.".into()),
        Err(e) => return Err(format!("a própria validação recusou: {}", e.0)),
    }
    Ok(Enviada { txid, nonce, pares: rede.pares_conectados() })
}

/// Uma linha do histórico da carteira.
pub struct Movimento {
    /// Horário do bloco; zero enquanto a transação está esperando.
    pub quando: u64,
    pub altura: u64,
    pub pendente: bool,
    /// Entrou na carteira (recebido ou minerado) ou saiu.
    pub entrada: bool,
    /// O outro lado, em hexadecimal. Vazio na recompensa de mineração.
    pub outro: String,
    pub valor: u64,
    pub taxa: u64,
    pub txid: String,
    /// `recompensa` (bloco minerado) ou `transferencia`.
    pub tipo: &'static str,
}

/// Quantos blocos o histórico olha para trás, no máximo.
const BLOCOS_OLHADOS: usize = 20_000;

fn soma_para(t: &Transfer, quem: &[u8; ADDRESS_LEN]) -> u64 {
    t.outputs
        .iter()
        .filter(|s| s.recipient == *quem && s.asset_id == HYX)
        .fold(0u64, |a, s| a.saturating_add(s.amount))
}

fn primeiro_destino(t: &Transfer, rede: &str) -> String {
    t.outputs.first().map(|s| endereco::mostrar(&s.recipient, rede)).unwrap_or_default()
}

/// Monta o histórico: o que está esperando no mempool primeiro, depois o que já
/// entrou em bloco, do mais novo para o mais velho.
pub fn historico(chain: &Chain, esperando: &[Transfer], meu: &[u8; ADDRESS_LEN], limite: usize) -> Vec<Movimento> {
    let mut saida: Vec<Movimento> = Vec::new();
    for t in esperando {
        let recebido = soma_para(t, meu);
        let meu_envio = t.sender == *meu;
        if !meu_envio && recebido == 0 {
            continue;
        }
        let txid = Tx::Transfer(t.clone()).txid().map(|h| hex(&h)).unwrap_or_default();
        saida.push(Movimento {
            quando: 0,
            altura: 0,
            pendente: true,
            entrada: !meu_envio,
            outro: if meu_envio { primeiro_destino(t, chain.params.nome) } else { endereco::mostrar(&t.sender, chain.params.nome) },
            valor: if meu_envio { t.outputs.iter().fold(0u64, |a, s| a.saturating_add(s.amount)) } else { recebido },
            taxa: if meu_envio { t.fee } else { 0 },
            txid,
            tipo: "transferencia",
        });
    }
    for entrada in chain.entries.iter().rev().take(BLOCOS_OLHADOS) {
        if saida.len() >= limite {
            break;
        }
        let quando = entrada.block.header.timestamp;
        let altura = entrada.block.header.height;
        for tx in &entrada.block.transactions {
            if saida.len() >= limite {
                break;
            }
            match tx {
                Tx::Coinbase(c) if c.recipient == *meu => saida.push(Movimento {
                    quando,
                    altura,
                    pendente: false,
                    entrada: true,
                    outro: String::new(),
                    valor: c.amount,
                    taxa: 0,
                    txid: tx.txid().map(|h| hex(&h)).unwrap_or_default(),
                    tipo: "recompensa",
                }),
                Tx::Transfer(t) => {
                    let recebido = soma_para(t, meu);
                    let meu_envio = t.sender == *meu;
                    if !meu_envio && recebido == 0 {
                        continue;
                    }
                    saida.push(Movimento {
                        quando,
                        altura,
                        pendente: false,
                        entrada: !meu_envio,
                        outro: if meu_envio { primeiro_destino(t, chain.params.nome) } else { endereco::mostrar(&t.sender, chain.params.nome) },
                        valor: if meu_envio { t.outputs.iter().fold(0u64, |a, s| a.saturating_add(s.amount)) } else { recebido },
                        taxa: if meu_envio { t.fee } else { 0 },
                        txid: tx.txid().map(|h| hex(&h)).unwrap_or_default(),
                        tipo: "transferencia",
                    });
                }
                Tx::Coinbase(_) => {}
            }
        }
    }
    saida
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    const MEU: [u8; ADDRESS_LEN] = [0x11; ADDRESS_LEN];
    const OUTRO: [u8; ADDRESS_LEN] = [0x22; ADDRESS_LEN];

    #[test]
    fn conferir_recusa_o_que_nao_e_endereco() {
        assert!(conferir(&MEU, "", "1", "", "hyurax-testnet").is_err());
        assert!(conferir(&MEU, "22", "1", "", "hyurax-testnet").is_err());
        assert!(conferir(&MEU, &hex(&OUTRO)[..38], "1", "", "hyurax-testnet").is_err());
        assert!(conferir(&MEU, &format!("{}zz", &hex(&OUTRO)[..38]), "1", "", "hyurax-testnet").is_err());
    }

    #[test]
    fn conferir_aceita_espaco_no_endereco_e_recusa_o_proprio() {
        let com_espaco = format!("{} {}", &hex(&OUTRO)[..20], &hex(&OUTRO)[20..]);
        assert_eq!(conferir(&MEU, &com_espaco, "1.5", "", "hyurax-testnet").unwrap().valor, 150_000_000);
        assert!(conferir(&MEU, &hex(&MEU), "1", "", "hyurax-testnet").unwrap_err().contains("seu próprio"));
    }

    #[test]
    fn conferir_aceita_o_formato_com_verificador_e_pega_digitacao() {
        let certo = endereco::mostrar(&OUTRO, "hyurax-testnet");
        assert_eq!(conferir(&MEU, &certo, "1", "", "hyurax-testnet").unwrap().para, OUTRO);
        let mut errado: Vec<char> = certo.chars().collect();
        errado[10] = if errado[10] == 'q' { 'p' } else { 'q' };
        let errado: String = errado.into_iter().collect();
        assert!(conferir(&MEU, &errado, "1", "", "hyurax-testnet").unwrap_err().contains("verificador"));
        let principal = endereco::mostrar(&OUTRO, "hyurax-mainnet");
        assert!(conferir(&MEU, &principal, "1", "", "hyurax-testnet").unwrap_err().contains("rede principal"));
    }

    #[test]
    fn conferir_recusa_valor_zero_e_valor_estranho() {
        assert!(conferir(&MEU, &hex(&OUTRO), "0", "", "hyurax-testnet").is_err());
        assert!(conferir(&MEU, &hex(&OUTRO), "0.000000001", "", "hyurax-testnet").is_err()); // 9 casas
        assert!(conferir(&MEU, &hex(&OUTRO), "1,5", "", "hyurax-testnet").is_err()); // vírgula não é ponto
        assert!(conferir(&MEU, &hex(&OUTRO), "-1", "", "hyurax-testnet").is_err());
        assert!(conferir(&MEU, &hex(&OUTRO), "1", "abc", "hyurax-testnet").is_err());
    }

    #[test]
    fn a_taxa_vazia_vira_a_padrao_e_a_escrita_vale() {
        assert_eq!(conferir(&MEU, &hex(&OUTRO), "2", "", "hyurax-testnet").unwrap().taxa, TAXA_PADRAO);
        assert_eq!(conferir(&MEU, &hex(&OUTRO), "2", "  ", "hyurax-testnet").unwrap().taxa, TAXA_PADRAO);
        assert_eq!(conferir(&MEU, &hex(&OUTRO), "2", "0.001", "hyurax-testnet").unwrap().taxa, 100_000);
    }

    #[test]
    fn o_maximo_desconta_a_taxa_e_nunca_passa_de_zero() {
        assert_eq!(maximo(1_000, 100), 900);
        assert_eq!(maximo(50, 100), 0);
    }
}
