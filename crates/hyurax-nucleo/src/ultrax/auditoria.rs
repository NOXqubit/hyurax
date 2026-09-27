//! A auditoria do histórico: TASK_ID, assinaturas e uma amostra refeita do zero.

#[allow(clippy::wildcard_imports)]
use super::*;

// ---------------------------------------------------------------------------
// Auditoria
// ---------------------------------------------------------------------------

/// O que a auditoria achou.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Auditoria {
    /// Linhas lidas.
    pub registros: u64,
    /// Linhas que não se leem.
    pub ilegiveis: u64,
    /// `TASK_ID` refeito a partir dos campos e igual ao gravado.
    pub ids_certos: u64,
    /// Registros de prova com assinatura válida, do worker deste nó.
    pub assinaturas_certas: u64,
    /// Registros com prova (execução terminada).
    pub com_prova: u64,
    /// Vereditos assinados que conferem com o estado gravado.
    pub vereditos_certos: u64,
    /// Tarefas refeitas do zero.
    pub refeitas: u64,
    /// Refeitas com o mesmo resultado e as mesmas operações.
    pub refeitas_iguais: u64,
    /// O placar foi comparado com o histórico (só dá quando o histórico está inteiro).
    pub placar_conferido: bool,
    /// Registros cujas operações só se conferem refazendo (rotas): aqui só
    /// o teto é conferido, e as refeitas da amostra conferem o número exato.
    pub so_refazendo: u64,
    /// Descrição de cada problema achado.
    pub problemas: Vec<String>,
    /// Limites do que a auditoria prova, para ninguém ler mais do que ela diz.
    pub avisos: Vec<String>,
}

/// O worker deste nó, pela `no.chave` da pasta, sem criar arquivo nenhum.
pub(super) fn worker_da_pasta(dados: &Path) -> Option<[u8; PUBKEY_LEN]> {
    let texto = std::fs::read_to_string(dados.join("no.chave")).ok()?;
    let segredo: [u8; SECRET_LEN] = texto.lines().find_map(|l| l.trim().strip_prefix("segredo=")).and_then(de_hex)?;
    Some(ed25519_public_key(&chave_do_worker(&segredo)))
}

/// As operações que a instância exige pelo modelo de custo do tipo.
pub(super) fn operacoes_da_instancia(esp: &Especificacao, semente: &[u8]) -> u64 {
    operacoes_conhecidas(esp, semente).unwrap_or_else(|| esp.operacoes_maximas())
}

/// As operações que a tarefa tem de declarar, quando dá para saber sem
/// executar. Nas rotas, o 2-opt para quando não acha melhora: a conta
/// depende do caminho, e só refazendo se confere (a auditoria refaz uma
/// amostra e compara as operações também).
pub(super) fn operacoes_conhecidas(esp: &Especificacao, semente: &[u8]) -> Option<u64> {
    match esp.tipo() {
        TipoDeTrabalho::Mochila => Some(
            trabalho::instancia_mochila(esp.tamanho(), semente)
                .map_or(esp.operacoes_maximas(), |i| u64::from(esp.tamanho()).saturating_mul(u64::from(i.capacidade).saturating_add(1))),
        ),
        TipoDeTrabalho::Rotas => None,
        _ => Some(esp.operacoes_maximas()),
    }
}

/// Confere tudo o que uma linha afirma, menos refazer a conta.
pub(super) fn conferir_linha(r: &Registro, t: Option<&Tarefa>, esperado: Option<[u8; PUBKEY_LEN]>, a: &mut Auditoria) {
    let n = r.numero();
    if t.is_some_and(|t| t.id == r.id) {
        a.ids_certos = a.ids_certos.saturating_add(1);
    } else {
        a.problemas.push(format!("#{n:08}: o TASK_ID não confere com os campos (registro alterado)"));
    }
    // desafio só é desafio se for exatamente um dos casos do gabarito
    let desafio = desafio_de(&r.especificacao);
    if r.desafio {
        if desafio.is_none() || r.metodo != MetodoDeVerificacao::ResultadoEsperado || r.instancia != Instancia::Compartilhada {
            a.problemas.push(format!("#{n:08}: marcada como desafio, mas não é um desafio do gabarito"));
        }
    } else if r.instancia == Instancia::DeJob {
        // a semente de uma unidade de JOB é função do JOB e do índice, e só dele
        let presa = match (r.job, r.indice) {
            (Some(job), Some(i)) => r.origem == semente_da_unidade(&job, i).to_vec(),
            _ => false,
        };
        if !presa {
            a.problemas.push(format!("#{n:08}: unidade de JOB cuja semente não é a do JOB e índice gravados"));
        }
    } else if r.instancia != Instancia::PorWorker {
        a.problemas.push(format!("#{n:08}: tarefa normal com instância compartilhada (copiável)"));
    }
    let depois_da_entrega = matches!(
        r.estado,
        Estado::Enviada | Estado::Verificando | Estado::Verificada | Estado::Recusada | Estado::Liquidada
    );
    let Some(prova) = r.prova() else {
        if depois_da_entrega {
            a.problemas.push(format!("#{n:08}: estado {} sem registro de prova", r.estado.nome()));
        }
        return;
    };
    a.com_prova = a.com_prova.saturating_add(1);
    if esperado.is_some_and(|w| w != prova.worker) {
        a.problemas.push(format!("#{n:08}: assinado por outro worker, não pelo deste nó"));
    } else if r.assinatura.is_some_and(|s| prova.assinatura_confere(&s)) {
        a.assinaturas_certas = a.assinaturas_certas.saturating_add(1);
    } else {
        a.problemas.push(format!("#{n:08}: assinatura do registro de prova não confere"));
    }
    if r.veredito.is_some_and(|v| prova.veredito_confere(r.estado.nome(), &v)) {
        a.vereditos_certos = a.vereditos_certos.saturating_add(1);
    } else {
        a.problemas.push(format!("#{n:08}: o veredito assinado não confere com o estado {}", r.estado.nome()));
    }
    // as operações são a base do Work Score: precisam ser as do modelo de custo
    let semente = if r.desafio { semente_do_desafio(&r.especificacao) } else { t.map_or([0; HASH_LEN], |t| t.semente_para(&prova.worker)) };
    match operacoes_conhecidas(&r.especificacao, &semente) {
        Some(devidas) if r.operacoes != devidas => {
            a.problemas.push(format!("#{n:08}: declara {} operações, e a tarefa tem {devidas}", r.operacoes));
        }
        Some(_) => {}
        None => {
            // o teto do modelo de custo ainda vale: declarar mais que ele é mentira
            if r.operacoes > r.especificacao.operacoes_maximas() {
                a.problemas.push(format!("#{n:08}: declara {} operações, acima do teto da tarefa", r.operacoes));
            }
            a.so_refazendo = a.so_refazendo.saturating_add(1);
        }
    }
    if r.desafio && r.estado == Estado::Liquidada && desafio.is_some_and(|d| Some(d.esperado.to_string()) != r.resultado.map(|x| hex(&x))) {
        a.problemas.push(format!("#{n:08}: desafio liquidado com resposta diferente da do gabarito"));
    }
}

/// Lê o histórico, confere cada linha (id, worker, assinatura, veredito,
/// operações, desafio) e refaz do zero até `amostra` tarefas liquidadas,
/// escolhidas ao acaso. Com o histórico inteiro, compara também o placar.
pub fn auditar(dados: &Path, config: &Path, amostra: usize) -> Result<Auditoria, String> {
    let pasta = dados.join("ultrax");
    let anterior = std::fs::read_to_string(pasta.join("historico.txt.1")).ok();
    let mut texto = anterior.clone().unwrap_or_default();
    texto.push_str(&std::fs::read_to_string(pasta.join("historico.txt")).map_err(|e| format!("sem histórico em {}: {e}", pasta.display()))?);
    let mut a = Auditoria::default();
    let esperado = worker_da_pasta(config);
    if esperado.is_none() {
        a.avisos.push("sem no.chave na pasta: não dá para saber se o worker dos registros é o deste nó".into());
    }
    a.avisos.push("quem tem o no.chave consegue assinar qualquer registro: a auditoria prova que o histórico é coerente, não que o dono é honesto".into());
    let mut liquidadas = Vec::new();
    let mut ids_liquidados: Vec<[u8; HASH_LEN]> = Vec::new();
    let (mut soma_liquidadas, mut soma_operacoes) = (0u64, 0u64);
    for linha in texto.lines().filter(|l| !l.trim().is_empty()) {
        a.registros = a.registros.saturating_add(1);
        let Some(r) = Registro::ler(linha) else {
            a.ilegiveis = a.ilegiveis.saturating_add(1);
            a.problemas.push(format!("linha {} ilegível", a.registros));
            continue;
        };
        let refeita = Tarefa::nova(r.especificacao, r.metodo, r.instancia, &r.origem, r.prioridade, r.criada_ms, r.prazo_ms).ok();
        conferir_linha(&r, refeita.as_ref(), esperado, &mut a);
        if r.estado == Estado::Liquidada {
            if ids_liquidados.contains(&r.id) {
                a.problemas.push(format!("#{:08}: liquidada mais de uma vez", r.numero()));
                continue;
            }
            ids_liquidados.push(r.id);
            soma_liquidadas = soma_liquidadas.saturating_add(1);
            soma_operacoes = soma_operacoes.saturating_add(r.operacoes);
            if let Some(t) = refeita {
                liquidadas.push((r, t));
            }
        }
    }
    // o placar não é assinado: só dá para conferir contra o histórico inteiro
    if anterior.is_none() {
        let placar = std::fs::read_to_string(pasta.join("placar.txt")).map(|t| Placar::de_texto(&t)).unwrap_or_default();
        a.placar_conferido = true;
        if placar.liquidadas != soma_liquidadas || placar.score.operacoes_verificadas != soma_operacoes {
            a.problemas.push(format!(
                "placar.txt diz {} liquidadas e {} operações verificadas; o histórico soma {soma_liquidadas} e {soma_operacoes}",
                placar.liquidadas, placar.score.operacoes_verificadas
            ));
        }
    } else {
        a.avisos.push("o histórico já recomeçou uma vez (historico.txt.1): o placar não pode ser comparado com ele".into());
    }
    // amostra ao acaso, sem repetir
    let mut escolhidas = Vec::new();
    while escolhidas.len() < amostra.min(liquidadas.len()) {
        let mut sorteio = [0u8; 8];
        hyurax_net::entropia::preencher(&mut sorteio)?;
        let k = usize::try_from(u64::from_be_bytes(sorteio) % liquidadas.len() as u64).unwrap_or(0);
        if !escolhidas.contains(&k) {
            escolhidas.push(k);
        }
    }
    for k in escolhidas {
        let Some((r, t)) = liquidadas.get(k) else { continue };
        let Some(worker) = r.worker else { continue };
        let semente = if r.desafio { semente_do_desafio(&r.especificacao) } else { t.semente_para(&worker) };
        a.refeitas = a.refeitas.saturating_add(1);
        match trabalho::executar(&r.especificacao, &semente, &mut |_| true) {
            Ok(exec) if Some(hash_do_resultado(&exec.resultado)) == r.resultado && exec.operacoes == r.operacoes => {
                a.refeitas_iguais = a.refeitas_iguais.saturating_add(1);
            }
            Ok(_) => a.problemas.push(format!("#{:08}: refeita do zero, deu outro resultado", r.numero())),
            Err(e) => a.problemas.push(format!("#{:08}: não consegui refazer: {e}", r.numero())),
        }
    }
    Ok(a)
}
