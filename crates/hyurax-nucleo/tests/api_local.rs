//! A API local de ponta a ponta: um núcleo de verdade (rede local de teste,
//! pasta temporária) e pedidos HTTP pelo socket, como a janela e um programa
//! qualquer deste computador fariam.
//!
//! Confere as regras de `hyurax_nucleo::api`: chave de sessão, `Host` contra
//! DNS rebinding, `Origin` contra página maliciosa, e o que fica público.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use hyurax_nucleo::config::{ConfigDoNo, rede_do_nome};
use hyurax_nucleo::pastas::Pastas;
use hyurax_nucleo::servico::{Modo, Nucleo, Partida};

const INTERFACE: hyurax_nucleo::api::Arquivos = &[("/index.html", "text/html", b"<p>HYURAX</p>")];

fn ligar(nome: &str) -> (Arc<Nucleo>, u16, std::path::PathBuf) {
    let pasta = std::env::temp_dir().join(format!("hyurax-api-local-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    std::fs::create_dir_all(&pasta).unwrap();
    let config = ConfigDoNo::nova(rede_do_nome("regtest").unwrap(), Pastas::unica(&pasta), 0, Vec::new(), true);
    let n = Nucleo::ligar(Partida {
        modo: Modo::Terminal,
        config,
        arquivo_carteira: None,
        endereco_fixo: None,
        linhas_padrao: 1,
        pausa_fixa_ms: 0,
        painel_na_rede: false,
    })
    .unwrap();
    let porta = hyurax_nucleo::api::abrir(&n, 0, INTERFACE).unwrap();
    (n, porta, pasta)
}

/// Um pedido cru. Devolve o status (200, 401…) e o corpo.
fn pedido(porta: u16, metodo: &str, caminho: &str, cabecalhos: &[(&str, &str)], corpo: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut texto = format!("{metodo} {caminho} HTTP/1.1\r\nConnection: close\r\n");
    if !cabecalhos.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-length")) {
        texto.push_str(&format!("Content-Length: {}\r\n", corpo.len()));
    }
    if !cabecalhos.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        texto.push_str(&format!("Host: 127.0.0.1:{porta}\r\n"));
    }
    for (k, v) in cabecalhos {
        texto.push_str(&format!("{k}: {v}\r\n"));
    }
    texto.push_str("\r\n");
    texto.push_str(corpo);
    s.write_all(texto.as_bytes()).unwrap();
    let mut resposta = String::new();
    let _ = s.read_to_string(&mut resposta);
    let status = resposta.split(' ').nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
    let corpo = resposta.split_once("\r\n\r\n").map(|(_, c)| c.to_string()).unwrap_or_default();
    (status, corpo)
}

#[test]
fn chave_host_e_origin_protegem_a_api_local() {
    let (n, porta, pasta) = ligar("regras");
    let chave = n.chave_painel.clone();
    let host_local = format!("127.0.0.1:{porta}");

    // a chave está na pasta de configuração, para a janela (e só o dono lê)
    assert_eq!(hyurax_nucleo::api::ler_chave(&pasta).as_deref(), Some(chave.as_str()));

    // público: o resumo (outras máquinas do dono) e a interface
    let (s, corpo) = pedido(porta, "GET", "/api/v1/resumo", &[], "");
    assert_eq!(s, 200);
    assert!(corpo.contains("\"produto\":\"Hyurax"));
    assert_eq!(pedido(porta, "GET", "/", &[], "").0, 200);

    // leitura completa sem a chave: recusada
    assert_eq!(pedido(porta, "GET", "/api/v1/estado", &[], "").0, 401);
    assert_eq!(pedido(porta, "GET", "/api/v1/estado", &[("X-Hyurax-Chave", &"0".repeat(64))], "").0, 401);
    // com a chave: cabeçalho ou ?chave= (o fluxo e os links usam a busca)
    let (s, corpo) = pedido(porta, "GET", "/api/v1/estado", &[("X-Hyurax-Chave", &chave)], "");
    assert_eq!(s, 200);
    assert!(corpo.contains("\"pode_mandar\":true"));
    assert_eq!(pedido(porta, "GET", &format!("/api/v1/estado?chave={chave}"), &[], "").0, 200);

    // DNS rebinding: um domínio apontando para 127.0.0.1 não lê nada, nem com a chave
    let host_mau = format!("x.atacante.example:{porta}");
    assert_eq!(pedido(porta, "GET", "/api/v1/estado", &[("Host", &host_mau), ("X-Hyurax-Chave", &chave)], "").0, 403);
    assert_eq!(pedido(porta, "GET", "/api/v1/resumo", &[("Host", &host_mau)], "").0, 403);

    // comando: sem a chave, recusado; com a chave e Origin de outra página, recusado
    let form = [("Content-Type", "application/x-www-form-urlencoded")];
    assert_eq!(pedido(porta, "POST", "/api/v1/ultrax", &form, "linhas=1").0, 403);
    let com_origem_ma = [form[0], ("X-Hyurax-Chave", &chave), ("Origin", "http://pagina-maliciosa.example")];
    assert_eq!(pedido(porta, "POST", "/api/v1/ultrax", &com_origem_ma, "linhas=1").0, 403);
    // com a chave, Host local e Origin local: vale
    let origem_local = format!("http://{host_local}");
    let certo = [form[0], ("X-Hyurax-Chave", &chave), ("Origin", origem_local.as_str())];
    assert_eq!(pedido(porta, "POST", "/api/v1/ultrax", &certo, "linhas=1").0, 200);

    // corpo anunciado maior que o teto de formulário: recusado antes de ler
    let grande = [certo[0], certo[1], certo[2], ("Content-Length", "9216")];
    assert_eq!(pedido(porta, "POST", "/api/v1/ultrax", &grande, "").0, 400);

    // a saúde, pública como o resumo, para monitoramento
    let (s, saude) = pedido(porta, "GET", "/api/v1/saude", &[], "");
    assert_eq!(s, 200);
    let saude: serde_json::Value = serde_json::from_str(&saude).unwrap();
    assert_eq!(saude["ok"], false, "sem pares, pede atenção");
    assert!(saude["alertas"].as_array().unwrap().iter().any(|a| a.as_str().unwrap().contains("par")));
    assert!(saude.get("saldo").is_none() && saude.get("endereco").is_none(), "nada de carteira na saúde");

    // o resumo lido por outra máquina do dono: com desafio, volta assinado
    // pela chave de worker deste nó; a chave fica fixada na primeira vez
    let alvo = format!("127.0.0.1:{porta}");
    let (vista, chave) = hyurax_nucleo::maquinas::olhar(&alvo, 1, None);
    assert!(vista.ok, "resumo assinado: {}", vista.erro);
    let chave = chave.expect("a chave de quem assinou");
    assert_eq!(chave, hyurax_crypto::ed25519_public_key(&n.chave_do_resumo));
    assert!(hyurax_nucleo::maquinas::olhar(&alvo, 2, Some(&chave)).0.ok);
    let (outra, _) = hyurax_nucleo::maquinas::olhar(&alvo, 3, Some(&[1u8; 32]));
    assert!(!outra.ok && outra.erro.contains("mudou"), "{}", outra.erro);

    n.encerrar();
    let _ = std::fs::remove_dir_all(pasta);
}

#[test]
fn moleculas_se_buscam_e_vem_com_a_previsao_da_ia() {
    let (n, porta, pasta) = ligar("moleculas");
    let chave = n.chave_painel.clone();
    let cab = [("X-Hyurax-Chave", chave.as_str())];

    // busca por nome, com espaço codificado, sem diferença de maiúsculas
    let (s, corpo) = pedido(porta, "GET", "/api/v1/ciencia/moleculas?busca=CAFFEINE", &cab, "");
    assert_eq!(s, 200);
    let v: serde_json::Value = serde_json::from_str(&corpo).unwrap();
    assert!(v["total"].as_u64().unwrap() > 8000);
    let cafeina = v["achadas"].as_array().unwrap().iter().find(|a| a["nome"] == "caffeine").expect("cafeína no catálogo");
    assert_eq!(cafeina["formula"], "C8H10N4O2");
    let (_, corpo) = pedido(porta, "GET", "/api/v1/ciencia/moleculas?busca=salicylic%20acid", &cab, "");
    let v: serde_json::Value = serde_json::from_str(&corpo).unwrap();
    assert!(v["achadas"].as_array().unwrap().iter().all(|a| a["nome"].as_str().unwrap().to_lowercase().contains("salicylic acid")));
    let (_, corpo) = pedido(porta, "GET", "/api/v1/ciencia/moleculas?busca=", &cab, "");
    let v: serde_json::Value = serde_json::from_str(&corpo).unwrap();
    assert_eq!(v["achadas"].as_array().unwrap().len(), 24);

    // a molécula: medido, previsto pelo modelo embutido e o erro típico dele
    let i = cafeina["indice"].as_u64().unwrap();
    let (s, corpo) = pedido(porta, "GET", &format!("/api/v1/ciencia/molecula/{i}"), &cab, "");
    assert_eq!(s, 200);
    let m: serde_json::Value = serde_json::from_str(&corpo).unwrap();
    assert_eq!(m["formula"], "C8H10N4O2");
    let medido = m["logs_medido_mili"].as_i64().unwrap();
    let previsto = m["previsto_mili"].as_i64().expect("modelo embutido");
    let erro = m["erro_tipico_mili"].as_u64().unwrap();
    assert!((-3000..1000).contains(&medido), "{medido}");
    assert!((-6000..3000).contains(&previsto), "{previsto}");
    assert!((100..3000).contains(&erro), "{erro}");

    // sem a chave, nada
    assert_eq!(pedido(porta, "GET", "/api/v1/ciencia/moleculas?busca=x", &[], "").0, 401);

    n.encerrar();
    let _ = std::fs::remove_dir_all(pasta);
}

#[test]
fn carteira_de_celular_le_saldo_e_entrega_transacao_assinada_fora() {
    use hyurax_nucleo::carteira::endereco;
    let (n, porta, pasta) = ligar("leve");
    let rede = n.config.rede;
    let magic = rede.magic;

    // info: rede, prefixo e magic, sem chave nenhuma (deste computador)
    let (s, corpo) = pedido(porta, "GET", "/api/v1/leve/info", &[], "");
    assert_eq!(s, 200, "{corpo}");
    let info: serde_json::Value = serde_json::from_str(&corpo).unwrap();
    assert_eq!(info["prefixo"], "rhyx");
    assert_eq!(info["magic"], magic.iter().map(|b| format!("{b:02x}")).collect::<String>());

    // uma conta sem nada: saldo zero, nonce zero, aceita Bech32m e hexadecimal
    let segredo = [7u8; 32];
    let de = hyurax_crypto::address_from_ed25519_pubkey(&hyurax_crypto::ed25519_public_key(&segredo));
    let texto = endereco::mostrar(&de, rede.nome);
    let (s, corpo) = pedido(porta, "GET", &format!("/api/v1/leve/conta/{texto}"), &[], "");
    assert_eq!(s, 200, "{corpo}");
    let c: serde_json::Value = serde_json::from_str(&corpo).unwrap();
    assert_eq!((c["saldo_unidades"].as_u64(), c["proximo_nonce"].as_u64()), (Some(0), Some(0)));
    let hexa: String = de.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(pedido(porta, "GET", &format!("/api/v1/leve/conta/{hexa}"), &[], "").0, 200);
    assert_eq!(pedido(porta, "GET", "/api/v1/leve/conta/thyx1errado", &[], "").0, 400);
    let (s, corpo) = pedido(porta, "GET", &format!("/api/v1/leve/historico/{texto}"), &[], "");
    assert_eq!(s, 200);
    assert!(corpo.contains("\"movimentos\":[]"), "{corpo}");

    // transação assinada fora do nó, sem saldo: a validação do nó recusa
    let para = hyurax_crypto::address_from_ed25519_pubkey(&hyurax_crypto::ed25519_public_key(&[9u8; 32]));
    let saida = hyurax_tx::Output { recipient: para, asset_id: hyurax_tx::HYX, amount: 1000 };
    let tx = hyurax_tx::sign_transfer_outputs(&segredo, &magic, de, vec![saida], 0, 0).unwrap();
    let corpo_tx: String = hyurax_tx::Tx::Transfer(tx).encode().unwrap().iter().map(|b| format!("{b:02x}")).collect();
    let (s, corpo) = pedido(porta, "POST", "/api/v1/leve/transacao", &[], &corpo_tx);
    assert_eq!(s, 400, "{corpo}");
    assert!(corpo.contains("saldo gastável insuficiente"), "{corpo}");
    // lixo e corpo grande demais
    assert_eq!(pedido(porta, "POST", "/api/v1/leve/transacao", &[], "zz").0, 400);
    let (s, _) = pedido(porta, "POST", "/api/v1/leve/transacao", &[], &"00".repeat(20_000));
    assert!(s == 400 || s == 413, "{s}");
    assert_eq!(pedido(porta, "GET", "/api/v1/leve/nada", &[], "").0, 404);

    n.encerrar();
    let _ = std::fs::remove_dir_all(pasta);
}

/// Defeito da 1.3.1: com um par conectado (que já anunciou o trabalho dele),
/// montar o estado pegava a trava da cadeia duas vezes na mesma linha, e o
/// núcleo inteiro parava (painel, mineração, rede). O estado tem de responder
/// com par conectado, várias vezes seguidas.
#[test]
fn estado_responde_com_par_conectado() {
    use hyurax_consensus::ParametrosRede;
    let (n, porta, pasta) = ligar("par");
    let chave = n.chave_painel.clone();
    let cab = [("X-Hyurax-Chave", chave.as_str())];

    // um par com 2 blocos a mais: trabalho anunciado diferente de zero
    let p = ParametrosRede::REGTEST;
    let mut cadeia = hyurax_chain::Chain::nova(p).unwrap();
    for _ in 0..2 {
        let ts = cadeia.tip().unwrap().header.timestamp + p.target_spacing;
        let bloco = cadeia.mine([9u8; 20], vec![], Some(ts), 2).unwrap();
        cadeia.accept_block(bloco, Some(ts + 10)).unwrap();
    }
    let par = hyurax_net::Rede::nova(hyurax_net::No::novo(cadeia)).unwrap();
    let porta_par = par.escutar("127.0.0.1:0").unwrap();
    n.rede.conectar_texto(&format!("127.0.0.1:{porta_par}")).unwrap();
    let ate = std::time::Instant::now() + Duration::from_secs(60);
    while n.rede.trabalho_dos_pares() == [0u8; 32] && std::time::Instant::now() < ate {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_ne!(n.rede.trabalho_dos_pares(), [0u8; 32], "o par não anunciou o trabalho");

    for _ in 0..5 {
        let (s, corpo) = pedido(porta, "GET", "/api/v1/estado", &cab, "");
        assert_eq!(s, 200, "o estado não respondeu com par conectado: {corpo}");
        assert!(corpo.contains("\"sincronizado\":"), "{corpo}");
    }
    // e a cadeia segue destravada para quem vier depois
    assert_eq!(pedido(porta, "GET", "/api/v1/resumo", &[], "").0, 200);

    par.desligar();
    n.encerrar();
    let _ = std::fs::remove_dir_all(pasta);
}
