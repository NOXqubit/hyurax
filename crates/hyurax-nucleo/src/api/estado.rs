//! O estado do núcleo em JSON (`/api/v1/estado`) e o resumo público
//! (`/api/v1/resumo`).
//!
//! Cada número da tela sai daqui ou do fluxo de eventos. Os que não são
//! medida dizem o que são: o limite de CPU é `AJUSTE`, a energia é `ESTIMADO`,
//! e as métricas da máquina trazem a origem (ver [`crate::metricas`]).

use std::sync::atomic::Ordering;

use hyurax_block::Block;
use hyurax_consensus::block_reward;
use hyurax_tx::{HYX, Tx};
use serde_json::{Value, json};

use crate::carteira::{endereco, envio};
use crate::servico::{Modo, Nucleo};
use crate::util::{agora_ms, hex, hyx};
use crate::{PRODUTO, VERSAO, termos};

/// Quantos blocos recentes vão no estado.
const BLOCOS: usize = 24;
/// Quantos movimentos da carteira vão no estado.
const MOVIMENTOS: usize = 30;
/// Quantos registros de texto vão no estado.
const REGISTROS: usize = 60;

/// Um número que é ajuste do dono, não medida.
fn ajuste(v: u32, unidade: &str, texto: &str) -> Value {
    json!({ "valor": v, "unidade": unidade, "origem": "AJUSTE", "fonte": texto })
}

/// O estado inteiro. `pode_mandar`: o pedido veio deste computador.
pub fn estado(n: &Nucleo, pode_mandar: bool, url_celular: &str) -> Value {
    let rede_nome = n.config.rede.nome;
    let trancado = n.carteira.trancado();
    let base = json!({
        "produto": PRODUTO,
        "versao": VERSAO,
        "rede": { "nome": rede_nome, "tipo": "TESTNET", "aviso": "rede de teste: o HYX não tem valor" },
        "modo": if n.modo == Modo::Janela { "janela" } else { "terminal" },
        "agora_ms": agora_ms(),
        "pode_mandar": pode_mandar,
        "trancado": trancado,
        "termos": { "aceitos": n.modo == Modo::Terminal || termos::aceitos(&n.config.pastas.config), "versao": termos::VERSAO },
    });
    if trancado {
        // trancado: só o bastante para desenhar o cadeado
        return base;
    }
    let mut j = base;
    let endereco = n.carteira.endereco();
    let Ok(no) = n.rede.no.lock() else { return j };
    let c = &no.chain;
    let altura = c.height();
    let (saldo, imaturo) = endereco.map_or((0, 0), |e| (c.state.balance(&e, &HYX), c.state.immature_balance(&e)));
    let e_meu =
        |b: &Block| endereco.is_some_and(|e| matches!(b.transactions.first(), Some(Tx::Coinbase(cb)) if cb.recipient == e));
    let meus_na_cadeia = c.entries.iter().filter(|e| e_meu(&e.block)).count();
    let n_util = c.tip().and_then(|b| b.useful_proof.as_ref().map(|p| p.n)).unwrap_or(n.config.rede.uteis.useful_size_base);
    let mut anterior: Option<u64> = None;
    let recentes: Vec<_> = c.entries.iter().rev().take(BLOCOS.saturating_add(1)).collect();
    let mut blocos = Vec::new();
    for (i, e) in recentes.iter().rev().enumerate() {
        let h = &e.block.header;
        let intervalo = anterior.map_or(0, |a| h.timestamp.saturating_sub(a));
        anterior = Some(h.timestamp);
        if i == 0 && recentes.len() > BLOCOS {
            continue; // só serviu para medir o intervalo do seguinte
        }
        blocos.push(json!({
            "altura": h.height,
            "hash": hex(&e.block.block_hash()),
            "horario": h.timestamp,
            "intervalo": intervalo,
            "txs": e.block.transactions.len(),
            "n": e.block.useful_proof.as_ref().map_or(0, |p| p.n),
            "meu": e_meu(&e.block),
            "bits": format!("{:#010x}", h.bits),
        }));
    }
    let movimentos: Vec<Value> = endereco
        .map(|meu| {
            envio::historico(c, &no.mempool_ordenado(), &meu, MOVIMENTOS)
                .iter()
                .map(|m| {
                    json!({
                        "quando": m.quando, "altura": m.altura, "pendente": m.pendente, "entrada": m.entrada,
                        "outro": m.outro, "valor": hyx(u128::from(m.valor)), "taxa": hyx(u128::from(m.taxa)),
                        "txid": m.txid, "tipo": m.tipo,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let proximo_nonce = endereco.map(|e| no.proximo_nonce(&e));
    let m = n.rede.estado_da_malha();
    let malha = json!({
        "alcance": m.alcance,
        "externo": m.externo,
        "vizinhos": m.vizinhos,
        "pontes_conhecidas": m.pontes_conhecidas,
        "reservas": m.reservas_minhas,
        "circuitos": m.circuitos,
        "roteador": m.roteador,
    });
    let no_json = json!({
        "altura": altura,
        "ponta": hex(&c.tip_hash()),
        "trabalho": c.total_work().to_decimal(),
        "emitido": hyx(u128::from(c.state.total_emitted)),
        "pares": n.rede.pares_conectados(),
        "sincronizado": n.rede.pares_conectados() > 0 && n.rede.alcancou_os_pares(),
        "mempool": no.mempool_len(),
        "porta_p2p": n.porta_p2p,
        // a malha (docs/HYURAX-MALHA.md): o que este nó sabe do próprio alcance
        "malha": malha,
        "recompensa": hyx(u128::from(block_reward(altura.saturating_add(1), &n.config.rede))),
        "maturidade": n.config.rede.coinbase_maturity,
        "prova_util": {
            "familia": "matrizes (Freivalds)",
            "n": n_util,
            "rodadas": n.config.rede.uteis.useful_rounds,
            "lado_min": n.config.rede.uteis.useful_size_min,
            "lado_base": n.config.rede.uteis.useful_size_base,
            "lado_max": n.config.rede.uteis.useful_size_max,
        },
    });
    drop(no);
    let (fator, exige_envio, trava) = n.carteira.seguranca();
    let carteira = json!({
        "existe": endereco.is_some(),
        "endereco": endereco.map(|e| endereco::mostrar(&e, rede_nome)),
        "saldo": hyx(u128::from(saldo)),
        "imaturo": hyx(imaturo),
        "proximo_nonce": proximo_nonce,
        "pode_enviar": endereco.is_some() && pode_mandar && saldo > 0,
        "sem_senha": n.carteira.sem_senha(),
        "maximo_envio": hyx(u128::from(envio::maximo(saldo, envio::TAXA_PADRAO))),
        "taxa_padrao": hyx(u128::from(envio::TAXA_PADRAO)),
        "seguranca": { "ligado": fator, "exige_envio": exige_envio, "trava": trava },
        "historico": movimentos,
    });
    let m = &n.mineracao;
    let linhas = m.linhas.load(Ordering::Relaxed);
    let mineracao = json!({
        "ligada": m.ligada.load(Ordering::Relaxed),
        "linhas": linhas,
        "nucleos": n.nucleos,
        "limite_cpu": ajuste(m.limite_cpu.load(Ordering::Relaxed), "%", "limite por linha escolhido pelo dono; não é o uso medido"),
        "ritmo": { "valor": m.ritmo(), "unidade": "tentativas/s", "origem": "REAL", "fonte": "tentativas contadas nos últimos 10 s" },
        "ms_tentativa": m.ms_tentativa(),
        "tentativas": m.tentativas.load(Ordering::Relaxed),
        "meus_sessao": m.meus.load(Ordering::Relaxed),
        "meus_cadeia": meus_na_cadeia,
        "perdidos": m.perdidos.load(Ordering::Relaxed),
        "rodada": m.rodada().map(|(s, a)| json!({ "segundos": s, "altura": a })),
        "memoria_mib": f64::from(n.config.rede.pow.memoria_kib) / 1024.0 * f64::from(linhas),
        "amostras": m.amostras().iter().map(|(t, x)| json!([t, x])).collect::<Vec<_>>(),
    });
    let ajustes = n.ajustes.lock().map(|a| a.clone()).unwrap_or_default();
    let registros: Vec<Value> = n
        .barramento
        .ultimos("registro", REGISTROS)
        .iter()
        .filter_map(|e| serde_json::from_str::<Value>(&e.json).ok().map(|v| json!({ "seq": e.seq, "quando_ms": e.quando_ms, "dados": v })))
        .collect();
    let maquinas: Vec<Value> = n
        .maquinas
        .vistas
        .lock()
        .map(|v| v.iter().map(|x| json!({ "alvo": x.alvo, "quando": x.quando, "ok": x.ok, "erro": x.erro, "resumo": x.resumo, "chave": x.chave })).collect())
        .unwrap_or_default();
    let ultrax: Value = serde_json::from_str(&n.ultrax.json()).unwrap_or(Value::Null);
    if let Some(o) = j.as_object_mut() {
        o.insert("no".into(), no_json);
        o.insert("carteira".into(), carteira);
        o.insert("mineracao".into(), mineracao);
        o.insert("ultrax".into(), ultrax);
        o.insert("metricas".into(), n.metricas.json(ajustes.watts_nucleo, ajustes.centavos_kwh));
        o.insert("blocos".into(), Value::Array(blocos));
        o.insert("registros".into(), Value::Array(registros));
        o.insert("maquinas".into(), Value::Array(maquinas));
        o.insert("ultimo_evento".into(), json!(n.barramento.ultimo_seq()));
        o.insert("nuvem".into(), n.nuvem.resumo());
        // os JOBs em números (o detalhe fica em /api/v1/ciencia)
        let (ativos, total, consumo) = n
            .ciencia
            .jobs
            .lock()
            .map(|j| {
                let ativos = j.iter().filter(|x| matches!(x.estado, crate::ciencia::EstadoDoJob::Rodando | crate::ciencia::EstadoDoJob::AguardandoNos)).count();
                (ativos, j.len(), j.iter().map(|x| x.consumo.milicreditos()).sum::<u64>())
            })
            .unwrap_or_default();
        o.insert("jobs".into(), json!({ "ativos": ativos, "total": total, "consumo_mili": consumo }));
        o.insert(
            "seguranca".into(),
            json!({
                "identidade_no": hex(&n.rede.identidade_publica()),
                "worker": hex(&n.nuvem.worker),
                "executavel_sha512": if pode_mandar { executavel_sha512() } else { None },
                "chave_de_lancamento": crate::atualizacao::chave_do_projeto().is_some(),
                "assinatura_windows": "PENDENTE",
                "api_local": "chave de sessão por abertura, Host e Origin conferidos",
                "cifra_entre_nos": "Noise XX (identidade provada no aperto de mão)",
                "segundo_fator": n.carteira.seguranca().0,
            }),
        );
        let atu = n.atualizacao.lock().map(|a| a.clone()).unwrap_or_default();
        o.insert(
            "atualizacao".into(),
            json!({
                "atual": VERSAO,
                "chave": crate::atualizacao::chave_do_projeto().is_some(),
                "verificado_ms": atu.verificado_ms,
                "disponivel": atu.disponivel.as_ref().map(|m| json!({ "versao": m.versao, "notas": m.notas, "tamanho": m.tamanho })),
                "erro": atu.erro,
                "baixando": atu.baixando,
            }),
        );
        o.insert(
            "ajustes".into(),
            json!({
                "watts_nucleo": ajustes.watts_nucleo,
                "centavos_kwh": ajustes.centavos_kwh,
                "na_rede": n.na_rede.load(Ordering::Relaxed),
                "api_externa": n.api_externa.load(Ordering::Relaxed),
                // as contas (sem chave: em disco só há o hash) só para quem pode mandar
                "contas": if pode_mandar {
                    n.contas.listar().iter().map(|c| json!({ "id": c.id, "nome": c.nome, "limite_milicreditos": c.limite_milicreditos, "revogada": c.revogada, "jobs": n.contas.jobs_da_conta(c.id).len() })).collect::<Vec<_>>()
                } else {
                    Vec::new()
                },
                "url_celular": url_celular,
                "avisar_bloco": ajustes.avisar_bloco,
                "som_bloco": ajustes.som_bloco,
                "sementes": n.sementes.lock().map(|s| s.clone()).unwrap_or_default(),
                "maquinas": n.maquinas.lista.lock().map(|m| m.clone()).unwrap_or_default(),
                // as pastas só aparecem para quem pode mandar (este computador)
                "pasta_config": if pode_mandar { n.config.pastas.config.display().to_string() } else { String::new() },
                "pasta_dados": if pode_mandar { n.config.pastas.dados.display().to_string() } else { String::new() },
            }),
        );
    }
    j
}

/// SHA-512 do executável que está rodando (calculado uma vez): o dono confere
/// contra a soma publicada na Release.
fn executavel_sha512() -> Option<String> {
    static H: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    H.get_or_init(|| {
        let caminho = std::env::current_exe().ok()?;
        let bytes = std::fs::read(caminho).ok()?;
        Some(hex(&hyurax_crypto::sha512(&bytes)))
    })
    .clone()
}

/// O resumo público, para as outras máquinas do dono (só leitura).
pub fn resumo(n: &Nucleo) -> Value {
    if n.carteira.trancado() {
        return json!({ "produto": PRODUTO, "versao": VERSAO, "trancado": true });
    }
    let (altura, saldo) = n
        .rede
        .no
        .lock()
        .map(|no| (no.chain.height(), n.carteira.endereco().map_or(0, |e| no.chain.state.balance(&e, &HYX))))
        .unwrap_or((0, 0));
    let m = &n.mineracao;
    let cpu = n.metricas.ultima().and_then(|l| l.cpu_processo);
    json!({
        "produto": PRODUTO,
        "versao": VERSAO,
        "rede": n.config.rede.nome,
        "trancado": false,
        "altura": altura,
        "pares": n.rede.pares_conectados(),
        "minerando": m.ligada.load(Ordering::Relaxed),
        "ritmo": m.ritmo(),
        "linhas": m.linhas.load(Ordering::Relaxed),
        "nucleos": n.nucleos,
        "endereco": n.carteira.endereco().map(|e| endereco::mostrar(&e, n.config.rede.nome)),
        "saldo": hyx(u128::from(saldo)),
        "ultrax_ligado": n.ultrax.ligado.load(Ordering::Relaxed),
        "cpu_processo": { "valor": cpu, "unidade": "%", "origem": if cpu.is_some() { "REAL" } else { "PENDENTE" } },
    })
}

/// A saúde do nó, para monitoramento (`/api/v1/saude`, público como o
/// resumo, sem endereço nem saldo). `ok` é falso quando algo pede atenção, e
/// `alertas` diz o quê. Cada número é medido aqui; nada é estimado.
pub fn saude(n: &Nucleo) -> Value {
    let agora_s = crate::util::agora_unix();
    let (altura, ponta_s) = n
        .rede
        .no
        .lock()
        .map(|no| (no.chain.height(), no.chain.tip().map_or(0, |b| b.header.timestamp)))
        .unwrap_or((0, 0));
    let pares = n.rede.pares_conectados();
    let situacoes = n.ciencia.situacoes();
    let abertos = situacoes.iter().filter(|s| !s.estado.e_final()).count();
    let mut alertas: Vec<String> = Vec::new();
    if pares == 0 {
        alertas.push("nenhum par conectado".into());
    }
    let sem_bloco = agora_s.saturating_sub(ponta_s);
    // dez intervalos sem bloco novo: ou a rede parou, ou este nó ficou para trás
    if ponta_s > 0 && sem_bloco > n.config.rede.target_spacing.saturating_mul(10) {
        alertas.push(format!("último bloco há {} min", sem_bloco / 60));
    }
    if n.carteira.trancado() {
        alertas.push("programa trancado".into());
    }
    json!({
        "ok": alertas.is_empty(),
        "alertas": alertas,
        "produto": PRODUTO,
        "versao": VERSAO,
        "rede": n.config.rede.nome,
        "ligado_ha_s": n.inicio.elapsed().as_secs(),
        "altura": altura,
        "ultimo_bloco_ha_s": if ponta_s > 0 { Some(sem_bloco) } else { None },
        "pares": pares,
        "minerando": n.mineracao.ligada.load(Ordering::Relaxed),
        "ultrax_ligado": n.ultrax.ligado.load(Ordering::Relaxed),
        "jobs_abertos": abertos,
        "api_externa": n.api_externa.load(Ordering::Relaxed),
    })
}
