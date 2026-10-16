//! Tarefas (seções 13 a 18 e 22 do documento da V1): o catálogo de motores,
//! a estimativa, a criação com reserva de saldo, a entrega do resultado, a
//! verificação, o fechamento e a liquidação.
//!
//! **O que roda no computador do fornecedor:** só os oito motores do
//! ULTRAX, compilados no programa. O cliente manda parâmetros, nunca código
//! nem comando: não há caminho para executar algo que veio de fora, ler
//! arquivo do fornecedor ou abrir rede de dentro do motor (seção 3).
//!
//! **Como o resultado é conferido:**
//!
//! 1. sempre: o registro de prova assinado pela chave do nó, a entrada
//!    (`hash(especificação, semente)`), o hash do resultado, o tamanho e as
//!    operações pelo modelo de custo do motor;
//! 2. matriz: Freivalds no próprio servidor (erro ≤ 2⁻⁸⁰), barato;
//! 3. rotas e triagem: conferência estrutural do resultado no servidor;
//! 4. os outros: concordância entre nós de donos diferentes, por amostragem
//!    que depende da reputação (tudo no período de prova; ver `regras.rs`).
//!
//! Duas entregas que discordam pedem um terceiro nó; a minoria é marcada como
//! divergente e perde reputação. Quem fica abaixo de 50 vai para a quarentena.

use std::collections::BTreeMap;

use hyurax_codec::Reader;
use hyurax_crypto::HASH_LEN;
use hyurax_ultrax::agregador::Agregador;
use hyurax_ultrax::prova::RegistroDeProva;
use hyurax_ultrax::trabalho::{self, Especificacao, TipoDeTrabalho, hash_da_entrada, hash_do_resultado};
use serde_json::{Value, json};

use super::contas::Quem;
use super::nos::{INSTAVEL_MS, abertas_do_no};
use super::{Erro, Nucleo, Plataforma, R};
use crate::dinheiro::{Conta, Linha, Motivo};
use crate::estado::{Estado, Tarefa};
use crate::eventos::{Desfecho, EstadoAdm, EstadoTarefa, Evento, Papel};
use crate::modelo::{Escopo, Id, PedidoDeTarefa, texto_curto};
use crate::regras::{self, Pagador};
use crate::util::{aleatorio, hex, sorteio_bp};

/// Tentativas que falham antes de a unidade ser dada como falha.
pub const TENTATIVAS_MAX: u8 = 4;
/// Conferência sorteada espera um nó por isso antes de ser dispensada.
pub const CONFERENCIA_ESPERA_MS: u64 = 10 * 60_000;
/// Folga depois do prazo para aceitar uma entrega que estava a caminho.
const FOLGA_DE_ENTREGA_MS: u64 = 30_000;
/// Resultados guardados por isso depois do fim da tarefa.
pub const GUARDA_DOS_RESULTADOS_MS: u64 = 30 * 86_400_000;
/// Maior corpo de entrega (o maior resultado é a matriz 1024², 8 MiB).
pub const ENTREGA_MAX: usize = 9 << 20;

/// O catálogo de motores (o que o cliente pode pedir), com rótulos, faixas,
/// padrões e como cada parâmetro vira o inteiro da especificação.
pub fn catalogo() -> Value {
    let p = |rotulo: &str, min: f64, max: f64, padrao: f64, cod: &str, passo: f64| json!({ "rotulo": rotulo, "min": min, "max": max, "padrao": padrao, "codificacao": cod, "passo": passo });
    let motores = vec![
        json!({
            "tipo": "matrix", "codigo": 1, "nome": "Multiplicação de matrizes", "categoria": "Matemática",
            "descricao": "C = A·B com matrizes inteiras n×n. Cada unidade é um par de matrizes.",
            "verificacao": "Freivalds no servidor (prova matemática, erro ≤ 2⁻⁸⁰)", "gpu": true,
            "tamanho": p("Lado da matriz", 1.0, 1024.0, 256.0, "inteiro", 1.0), "passos": Value::Null, "parametros": [], "unidades": 12,
            "unidade_significa": "par de matrizes",
        }),
        json!({
            "tipo": "knapsack", "codigo": 2, "nome": "Otimização de alocação (mochila)", "categoria": "Otimização",
            "descricao": "Mochila 0/1 resolvida até o ótimo. Cada unidade é uma instância.",
            "verificacao": "concordância entre nós, por amostragem de reputação", "gpu": false,
            "tamanho": p("Itens", 1.0, 256.0, 64.0, "inteiro", 1.0), "passos": Value::Null, "parametros": [], "unidades": 12,
            "unidade_significa": "instância",
        }),
        json!({
            "tipo": "diffusion", "codigo": 3, "nome": "Difusão de calor", "categoria": "Simulação",
            "descricao": "Difusão numa grade 2D em ponto fixo. Cada unidade é uma condição inicial.",
            "verificacao": "concordância entre nós, por amostragem de reputação", "gpu": false,
            "tamanho": p("Lado da grade", 2.0, 256.0, 64.0, "inteiro", 1.0), "passos": p("Passos", 1.0, 1024.0, 100.0, "inteiro", 1.0), "parametros": [], "unidades": 12,
            "unidade_significa": "condição inicial",
        }),
        json!({
            "tipo": "ai-training", "codigo": 4, "nome": "Treino de rede neural (solubilidade)", "categoria": "IA",
            "descricao": "Treina uma rede pequena que prevê a solubilidade de moléculas reais (AqSolDB). Cada unidade é um treino com sorteio próprio.",
            "verificacao": "concordância entre nós, por amostragem de reputação", "gpu": false,
            "tamanho": p("Lote", 1.0, 256.0, 32.0, "inteiro", 1.0), "passos": p("Passos de treino", 1.0, 4096.0, 500.0, "inteiro", 1.0), "parametros": [], "unidades": 6,
            "unidade_significa": "treino",
        }),
        json!({
            "tipo": "population-genetics", "codigo": 5, "nome": "Genética de populações (Wright-Fisher)", "categoria": "Biologia",
            "descricao": "Deriva, seleção e mutação em L loci. Cada unidade é uma réplica independente.",
            "verificacao": "concordância entre nós, por amostragem de reputação", "gpu": false,
            "tamanho": p("Indivíduos (N)", 2.0, 100_000.0, 500.0, "inteiro", 1.0), "passos": p("Gerações", 1.0, 10_000.0, 200.0, "inteiro", 1.0),
            "parametros": [
                p("Loci", 1.0, 64.0, 8.0, "inteiro", 1.0),
                p("Mutação (por milhão)", 0.0, 10_000.0, 0.0, "inteiro", 1.0),
                p("Seleção s (−1 a +1)", -1.0, 1.0, 0.01, "selecao", 0.001),
                p("Dominância h (%)", 0.0, 100.0, 50.0, "inteiro", 1.0),
                p("Frequência inicial p0", 0.0, 1.0, 0.1, "fracao", 0.01),
            ],
            "unidades": 20, "unidade_significa": "réplica",
        }),
        json!({
            "tipo": "crop-breeding", "codigo": 6, "nome": "Melhoramento de culturas", "categoria": "Agronomia",
            "descricao": "QTL aditivos, ambiente (água, nitrogênio, solo) e seleção por gerações. Cada unidade é uma réplica.",
            "verificacao": "concordância entre nós, por amostragem de reputação", "gpu": false,
            "tamanho": p("Plantas (N)", 4.0, 20_000.0, 200.0, "inteiro", 1.0), "passos": p("Gerações de seleção", 1.0, 1000.0, 10.0, "inteiro", 1.0),
            "parametros": [
                p("QTL", 1.0, 200.0, 20.0, "inteiro", 1.0),
                p("Arquitetura (código)", 0.0, 4_294_967_295.0, 42.0, "inteiro", 1.0),
                p("Selecionadas (%)", 1.0, 100.0, 20.0, "inteiro", 1.0),
                p("Ruído ambiental (% do desvio genético)", 0.0, 1000.0, 50.0, "inteiro", 1.0),
                p("Água (% da demanda)", 0.0, 200.0, 60.0, "inteiro", 1.0),
                p("Nitrogênio (% da dose)", 0.0, 200.0, 100.0, "inteiro", 1.0),
                p("Solo (% da referência)", 0.0, 200.0, 100.0, "inteiro", 1.0),
                p("Interação G×E (%)", 0.0, 100.0, 30.0, "inteiro", 1.0),
            ],
            "unidades": 8, "unidade_significa": "réplica",
        }),
        json!({
            "tipo": "routing", "codigo": 7, "nome": "Rotas (caixeiro-viajante, 2-opt)", "categoria": "Logística",
            "descricao": "Busca local 2-opt numa instância de cidades. Cada unidade parte de uma rota sorteada; vale a melhor.",
            "verificacao": "certificado da rota no servidor e concordância entre nós", "gpu": false,
            "tamanho": p("Cidades", 4.0, 2000.0, 200.0, "inteiro", 1.0), "passos": p("Teto de passadas do 2-opt", 1.0, 10_000.0, 1000.0, "inteiro", 1.0),
            "parametros": [p("Instância (código)", 0.0, 4_294_967_295.0, 11.0, "inteiro", 1.0)],
            "unidades": 12, "unidade_significa": "partida",
        }),
        json!({
            "tipo": "molecular-screening", "codigo": 8, "nome": "Triagem de moléculas (AqSolDB)", "categoria": "Química",
            "descricao": "Filtros de massa, logP, doadores e aceptores de H, ligações giratórias, área polar e solubilidade prevista sobre moléculas reais. Cada unidade é uma faixa do catálogo.",
            "verificacao": "estrutura no servidor e concordância entre nós", "gpu": false,
            "tamanho": p("Moléculas por unidade", 1.0, 4096.0, 500.0, "inteiro", 1.0), "passos": Value::Null,
            "parametros": [
                p("Catálogo (0 = AqSolDB)", 0.0, 0.0, 0.0, "inteiro", 1.0),
                p("Começa na molécula", 0.0, 8288.0, 0.0, "inteiro", 1.0),
                p("Massa máxima (g/mol)", 0.0, 2000.0, 500.0, "mili", 1.0),
                p("logP máximo", -20.0, 20.0, 5.0, "mili_deslocado", 0.1),
                p("Doadores de H máximos", 0.0, 100.0, 5.0, "inteiro", 1.0),
                p("Aceptores de H máximos", 0.0, 100.0, 10.0, "inteiro", 1.0),
                p("Ligações giratórias máximas", 0.0, 100.0, 10.0, "inteiro", 1.0),
                p("Área polar máxima (Å²)", 0.0, 1000.0, 140.0, "mili", 1.0),
                p("log S previsto mínimo", -20.0, 20.0, -4.0, "mili_deslocado", 0.1),
            ],
            "unidades": 17, "unidade_significa": "faixa de moléculas",
        }),
    ];
    json!({
        "motores": motores,
        "codificacoes": {
            "inteiro": "o próprio número",
            "selecao": "round(s · 10000) + 10000",
            "fracao": "round(v · 10000)",
            "mili": "round(v · 1000)",
            "mili_deslocado": "round(v · 1000) + 20000",
        },
        "seguranca": "Só estes motores rodam no computador do fornecedor. O pedido leva parâmetros, nunca código nem comando.",
    })
}

/// O que a validação de um pedido devolve.
struct Validado {
    modelo: Especificacao,
    job: hyurax_ultrax::job::EspecificacaoDeJob,
    memoria: u64,
    ops_max: u64,
    pedido: PedidoDeTarefa,
}

fn validar_pedido(e: &Estado, mut p: PedidoDeTarefa) -> R<Validado> {
    let pol = e.config.politica;
    p.tipo = p.tipo.trim().to_string();
    let tipo = p.motor().ok_or_else(|| Erro::pedido(format!("motor desconhecido: {} (veja /api/v1/catalogo)", p.tipo)))?;
    if e.config.motores_desligados.contains(&p.tipo) {
        return Err(Erro::indisponivel(format!("o motor {} está desligado pela equipe no momento", p.tipo)));
    }
    if p.paralelo == 0 {
        p.paralelo = 4;
    }
    if p.copias == 0 {
        p.copias = 1;
    }
    if p.prazo_s == 0 {
        p.prazo_s = 86_400;
    }
    p.descricao = texto_curto(&p.descricao.replace(['\r', '\n', '\t'], " "), 2000);
    p.regiao = texto_curto(&p.regiao, 24).to_uppercase();
    if !(1..=pol.unidades_max).contains(&p.unidades) {
        return Err(Erro::pedido(format!("unidades: de 1 a {}", pol.unidades_max)));
    }
    if p.paralelo > pol.paralelo_max {
        return Err(Erro::pedido(format!("núcleos em paralelo: de 1 a {}", pol.paralelo_max)));
    }
    if p.copias > pol.copias_max {
        return Err(Erro::pedido(format!("cópias por unidade: de 1 a {}", pol.copias_max)));
    }
    if !(60..=pol.prazo_max_s).contains(&p.prazo_s) {
        return Err(Erro::pedido(format!("prazo: de 60 s a {} dias", pol.prazo_max_s / 86_400)));
    }
    if p.gpu && tipo != TipoDeTrabalho::Matriz {
        return Err(Erro::pedido("nesta versão só a multiplicação de matrizes roda na GPU"));
    }
    let (modelo, job) = p.job().map_err(Erro::pedido)?;
    // a unidade mais cara (na triagem a faixa anda; o tamanho é o mesmo)
    let ultima = trabalho::derivar_unidade(&modelo, p.unidades.saturating_sub(1)).map_err(|e| Erro::pedido(e.to_string()))?;
    let memoria = modelo.memoria_bytes().max(ultima.memoria_bytes());
    let ops_max = modelo.operacoes_maximas().max(ultima.operacoes_maximas());
    if memoria > pol.memoria_max {
        return Err(Erro::pedido(format!("cada unidade pede {} MiB; o teto é {} MiB. Use um problema menor por unidade.", memoria.div_ceil(1 << 20), pol.memoria_max >> 20)));
    }
    if ops_max > pol.operacoes_max {
        return Err(Erro::pedido("cada unidade passa do teto de operações; use um problema menor por unidade e mais unidades"));
    }
    Ok(Validado { modelo, job, memoria, ops_max, pedido: p })
}

/// Capacidade online agora para um motor: (nós, donos diferentes, núcleos).
fn capacidade(e: &Estado, v: &super::Vivo, tipo: TipoDeTrabalho, gpu: bool, agora: u64, sem_dono: Id) -> (u32, u32, u32) {
    let mut donos = std::collections::BTreeSet::new();
    let (mut nos, mut nucleos) = (0u32, 0u32);
    for no in e.nos.values().filter(|x| !x.removido && !x.pausado && x.adm == EstadoAdm::Normal && x.dono != sem_dono) {
        let Some(p) = v.pulsos.get(&no.id) else { continue };
        let tem = (p.tipos & 1u16.wrapping_shl(u32::from(tipo.codigo()))) != 0;
        if agora.saturating_sub(p.ultimo_ms) <= INSTAVEL_MS && !p.pausado_local && tem && (!gpu || (p.gpu && no.limites.gpu)) {
            nos = nos.saturating_add(1);
            nucleos = nucleos.saturating_add(no.limites.linhas);
            donos.insert(no.dono);
        }
    }
    (nos, u32::try_from(donos.len()).unwrap_or(u32::MAX), nucleos)
}

/// A política de execução declarada (seção 16).
fn politica_json(p: &PedidoDeTarefa, memoria: u64) -> Value {
    json!({
        "cpu": format!("{} núcleo(s) em paralelo, uma unidade por núcleo", p.paralelo),
        "ram": format!("{} MiB por unidade, reservados antes de começar", memoria.div_ceil(1 << 20).max(1)),
        "gpu": if p.gpu { "sim (WebGL 2 no programa do fornecedor)" } else { "não" },
        "storage": "nenhum disco: a entrada nasce da especificação e da semente; a saída volta pela rede",
        "network": "nenhuma: o motor não tem rede nem acesso a arquivo",
        "timeout": format!("{} s para a tarefa inteira", p.prazo_s),
        "runtime": format!("hyurax-ultrax/{}", p.tipo),
    })
}

impl Plataforma {
    /// A estimativa de preço e tempo, sem criar nada.
    pub fn estimar(&self, q: &Quem, p: PedidoDeTarefa) -> R<Value> {
        let agora = self.agora();
        let n = self.nucleo()?;
        let val = validar_pedido(&n.estado, p)?;
        let tabela = n.estado.config.tabela(q.moeda);
        let copias = u64::from(val.pedido.copias);
        let unidade = regras::custo_da_execucao(&tabela, val.ops_max, val.memoria, val.pedido.gpu)?;
        let total = regras::estimativa(&tabela, val.ops_max, val.memoria, val.pedido.unidades, copias, val.pedido.gpu)?;
        let reserva = i64::try_from(regras::centavos(total).max(1)).map_err(|_| Erro::pedido("estimativa grande demais"))?;
        let disponivel = n.estado.livro.saldo(Conta::ClienteDisponivel(q.usuario), q.moeda);
        let v = self.vivo()?;
        let (nos, donos, nucleos) = capacidade(&n.estado, &v, val.modelo.tipo(), val.pedido.gpu, agora, q.usuario);
        let ritmo = if val.pedido.gpu { tabela.ops_por_s_gpu } else { tabela.ops_por_s_cpu }.max(1);
        let paralelo = u64::from(val.pedido.paralelo.min(nucleos.max(1)));
        let lotes = val.pedido.unidades.saturating_mul(copias).div_ceil(paralelo.max(1));
        let segundos = val.ops_max.checked_div(ritmo).unwrap_or(0).max(1).saturating_mul(lotes);
        let mut avisos = Vec::new();
        if nos == 0 {
            avisos.push("nenhum computador online executa este motor agora: a tarefa fica na fila até um aparecer".to_string());
        }
        if u64::from(donos) < copias {
            avisos.push(format!("você pediu {copias} cópias por unidade e há {donos} fornecedor(es) diferente(s) online: a tarefa pode esperar"));
        }
        if disponivel < reserva {
            avisos.push(format!("saldo insuficiente: a reserva é {} e o saldo é {}", q.moeda.formatar(reserva), q.moeda.formatar(disponivel)));
        }
        Ok(json!({
            "moeda": q.moeda.codigo(),
            "unidades": val.pedido.unidades,
            "copias": copias,
            "operacoes_por_unidade_max": val.ops_max,
            "memoria_por_unidade": val.memoria,
            "custo_por_unidade_micro": unidade.to_string(),
            "estimativa_micro": total.to_string(),
            "reserva_centavos": reserva,
            "reserva": q.moeda.formatar(reserva),
            "saldo": q.moeda.formatar(disponivel),
            "tempo_estimado_s": segundos,
            "capacidade": { "nos": nos, "fornecedores": donos, "nucleos": nucleos },
            "politica": politica_json(&val.pedido, val.memoria),
            "tabela": tabela,
            "como_cobra": "Reserva pelo teto de operações. No fim, cobra só as unidades conferidas, pelas operações do modelo de custo; o resto volta ao saldo.",
            "avisos": avisos,
        }))
    }

    /// Cria a tarefa: valida, reserva o saldo e põe na fila.
    pub fn criar_tarefa(&self, q: &Quem, p: PedidoDeTarefa, via_api: bool) -> R<Value> {
        q.cliente()?;
        q.pode(Escopo::TarefasCriar)?;
        let agora = self.agora();
        let nonce: [u8; 32] = aleatorio().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        if n.estado.config.parada_de_emergencia {
            return Err(Erro::indisponivel("a plataforma está em parada de emergência; tente mais tarde"));
        }
        let val = validar_pedido(&n.estado, p)?;
        let pol = n.estado.config.politica;
        let ativas = n.estado.ativas.iter().filter_map(|i| n.estado.tarefas.get(i)).filter(|t| t.cliente == q.usuario).count();
        if ativas >= usize::try_from(pol.tarefas_ativas_max).unwrap_or(usize::MAX) {
            return Err(Erro::limite(format!("você já tem {ativas} tarefas em andamento; espere alguma terminar")));
        }
        {
            let mut v = self.vivo()?;
            let lista = v.criacoes.entry(q.usuario).or_default();
            lista.retain(|&t| agora.saturating_sub(t) < 3_600_000);
            if lista.len() >= usize::try_from(pol.tarefas_por_hora).unwrap_or(usize::MAX) {
                return Err(Erro::limite(format!("limite de {} tarefas por hora", pol.tarefas_por_hora)));
            }
            lista.push(agora);
        }
        let tabela = n.estado.config.tabela(q.moeda);
        let total = regras::estimativa(&tabela, val.ops_max, val.memoria, val.pedido.unidades, u64::from(val.pedido.copias), val.pedido.gpu)?;
        let reserva = i64::try_from(regras::centavos(total).max(1)).map_err(|_| Erro::pedido("estimativa grande demais"))?;
        let disponivel = n.estado.livro.saldo(Conta::ClienteDisponivel(q.usuario), q.moeda);
        if disponivel < reserva {
            return Err(Erro { status: 402, mensagem: format!("saldo insuficiente: a reserva é {} e o saldo é {}", q.moeda.formatar(reserva), q.moeda.formatar(disponivel)) });
        }
        let id = n.estado.seq.tarefa;
        let job_id = val.job.id().map_err(|e| Erro::pedido(e.to_string()))?;
        let job = regras::job_da_tarefa(&job_id, id, &nonce);
        let prazo_ms = agora.saturating_add(val.pedido.prazo_s.saturating_mul(1000));
        let pedido = val.pedido.clone();
        let eventos = vec![
            Evento::TaskCreated {
                id,
                cliente: q.usuario,
                pedido: pedido.clone(),
                moeda: q.moeda,
                tabela,
                fornecedor_bp: n.estado.config.fornecedor_bp,
                job_id: hex(&job_id),
                nonce: hex(&nonce),
                job: hex(&job),
                estimativa_micro: total.to_string(),
                reserva,
                prazo_ms,
                via_api,
                t: agora,
            },
            Plataforma::lancamento(
                &n.estado,
                0,
                agora,
                Motivo::Reserva,
                format!("tarefa:{id}"),
                vec![
                    Linha { conta: Conta::ClienteDisponivel(q.usuario), moeda: q.moeda, valor: reserva.saturating_neg() },
                    Linha { conta: Conta::ClienteReservado(q.usuario), moeda: q.moeda, valor: reserva },
                ],
            ),
            Evento::TaskState { tarefa: id, estado: EstadoTarefa::Queued, motivo: "validada e com saldo reservado".into(), por: q.usuario, t: agora },
        ];
        self.registrar(&mut n, eventos)?;
        let _ = (val.modelo, val.memoria);
        Ok(json!({ "id": id, "estado": "QUEUED", "reserva": q.moeda.formatar(reserva), "reserva_centavos": reserva, "job": hex(&job) }))
    }

    /// Cancela a tarefa: o que já foi conferido é cobrado, o resto volta.
    pub fn cancelar_tarefa(&self, q: &Quem, id: Id) -> R<Value> {
        q.pode(Escopo::TarefasCriar)?;
        let mut n = self.nucleo()?;
        let t = n.estado.tarefas.get(&id).filter(|t| t.cliente == q.usuario || (q.papeis.admin && q.escopos.is_none())).ok_or_else(|| Erro::nao_achado("tarefa não encontrada"))?;
        if t.estado.final_() {
            return Err(Erro::conflito(format!("a tarefa já terminou ({})", t.estado.nome())));
        }
        let quem = if t.cliente == q.usuario { "pelo cliente" } else { "pelo administrador" };
        self.finalizar(&mut n, id, EstadoTarefa::Cancelled, &format!("cancelada {quem}"), q.usuario)?;
        let x = n.estado.tarefas.get(&id).ok_or_else(|| Erro::interno("tarefa sumiu"))?;
        Ok(json_da_tarefa(x, false))
    }

    /// As tarefas do cliente.
    pub fn minhas_tarefas(&self, q: &Quem) -> R<Value> {
        q.pode(Escopo::TarefasLer)?;
        self.ler(|e| {
            let mut v: Vec<&Tarefa> = e.tarefas.values().filter(|t| t.cliente == q.usuario).collect();
            v.sort_by_key(|t| std::cmp::Reverse(t.id));
            json!({ "tarefas": v.into_iter().take(500).map(|t| json_da_tarefa(t, false)).collect::<Vec<_>>() })
        })
    }

    /// Uma tarefa, com o detalhe das unidades.
    pub fn tarefa(&self, q: &Quem, id: Id) -> R<Value> {
        q.pode(Escopo::TarefasLer)?;
        let n = self.nucleo()?;
        let t = n.estado.tarefas.get(&id).filter(|t| t.cliente == q.usuario || (q.papeis.admin && q.escopos.is_none())).ok_or_else(|| Erro::nao_achado("tarefa não encontrada"))?;
        let mut j = json_da_tarefa(t, true);
        let v = self.vivo()?;
        // progresso das unidades em voo, pelo último pulso de cada nó
        let mut voo = Vec::new();
        for u in t.unidades.values() {
            for a in u.em_voo.iter().filter_map(|a| n.estado.atribuicoes.get(a)) {
                let prog = v.pulsos.get(&a.no).and_then(|p| p.progresso.iter().find(|x| x.0 == a.id)).map(|x| (x.1, x.2));
                voo.push(json!({ "unidade": a.unidade, "atribuicao": a.id, "executando": a.executando, "feitas": prog.map(|x| x.0), "total": prog.map(|x| x.1), "prazo_ms": a.prazo_ms }));
            }
        }
        crate::util::por(&mut j, "em_voo_detalhe", Value::Array(voo));
        Ok(j)
    }

    /// O resultado agregado (o melhor, a média, o topo...) das unidades conferidas.
    pub fn resultado_da_tarefa(&self, q: &Quem, id: Id) -> R<Value> {
        q.pode(Escopo::TarefasLer)?;
        let (modelo, resultados, apagados) = self.ler(|e| {
            e.tarefas.get(&id).filter(|t| t.cliente == q.usuario || (q.papeis.admin && q.escopos.is_none())).map(|t| (t.modelo, t.resultados.clone(), t.resultados_apagados))
        })?
        .ok_or_else(|| Erro::nao_achado("tarefa não encontrada"))?;
        if apagados {
            return Err(Erro { status: 410, mensagem: "os resultados desta tarefa já passaram do prazo de guarda (30 dias) e foram apagados".into() });
        }
        let mut ag = Agregador::novo(&modelo);
        let mut lidos = 0u64;
        for &i in resultados.keys() {
            if let Ok(b) = std::fs::read(self.arquivo_do_resultado(id, i))
                && ag.somar(i, &b).is_ok()
            {
                lidos = lidos.saturating_add(1);
            }
        }
        let agregado: Value = serde_json::from_str(&ag.json()).unwrap_or(Value::Null);
        Ok(json!({
            "tarefa": id,
            "unidades_conferidas": lidos,
            "texto": ag.texto(),
            "agregado": agregado,
            "unidades": resultados.iter().take(10_000).map(|(i, h)| json!({ "unidade": i, "hash": hex(h) })).collect::<Vec<_>>(),
        }))
    }

    /// Os bytes do resultado de uma unidade.
    pub fn bytes_da_unidade(&self, q: &Quem, id: Id, unidade: u64) -> R<Vec<u8>> {
        q.pode(Escopo::TarefasLer)?;
        let hash = self
            .ler(|e| e.tarefas.get(&id).filter(|t| t.cliente == q.usuario || (q.papeis.admin && q.escopos.is_none())).map(|t| t.resultados.get(&unidade).copied()))?
            .ok_or_else(|| Erro::nao_achado("tarefa não encontrada"))?
            .ok_or_else(|| Erro::nao_achado("unidade sem resultado conferido"))?;
        let b = std::fs::read(self.arquivo_do_resultado(id, unidade)).map_err(|_| Erro { status: 410, mensagem: "resultado apagado (prazo de guarda)".into() })?;
        if hash_do_resultado(&b) != hash {
            return Err(Erro::interno(&format!("o arquivo da unidade {unidade} da tarefa {id} não confere com o hash do diário")));
        }
        Ok(b)
    }

    /// O relatório completo: cada unidade, quem calculou, como foi conferida e
    /// o hash da prova de cada entrega. Com isto, qualquer um refaz a conta.
    pub fn relatorio(&self, q: &Quem, id: Id) -> R<Value> {
        q.pode(Escopo::TarefasLer)?;
        let n = self.nucleo()?;
        let t = n.estado.tarefas.get(&id).filter(|t| t.cliente == q.usuario || (q.papeis.admin && q.escopos.is_none())).ok_or_else(|| Erro::nao_achado("tarefa não encontrada"))?;
        let unidades: Vec<Value> = t
            .unidades
            .iter()
            .take(20_000)
            .map(|(i, u)| {
                let entregas: Vec<Value> = u
                    .todas
                    .iter()
                    .filter_map(|a| n.estado.atribuicoes.get(a))
                    .map(|a| {
                        json!({
                            "atribuicao": a.id, "no": a.no, "papel": if a.papel == Papel::Copia { "copia" } else { "conferencia" },
                            "hash": a.entrega.as_ref().map(|e| hex(&e.hash)), "prova": a.entrega.as_ref().map(|e| e.prova.clone()),
                            "operacoes": a.entrega.as_ref().map(|e| e.operacoes), "ms_calculo": a.entrega.as_ref().map(|e| e.ms_calculo),
                            "desfecho": a.desfecho.as_ref().map(|d| super::nos::desfecho_nome(d.0)), "motivo": a.desfecho.as_ref().map(|d| d.1.clone()),
                        })
                    })
                    .collect();
                let semente = regras::semente(&t.job, *i);
                json!({ "unidade": i, "semente": hex(&semente), "resultado": u.concluida.map(|h| hex(&h)), "falhou": u.falhou, "entregas": entregas })
            })
            .collect();
        let mut j = json_da_tarefa(t, true);
        let relatorio = json!({
            "job_ultrax": hex(&t.job_id),
            "job_da_tarefa": hex(&t.job),
            "como_refazer": "semente da unidade i = SHA-512(\"HYURAX-ULTRAX-UNIDADE-v1\" || job_da_tarefa || u64 i); o motor e os parâmetros estão no pedido",
            "unidades": unidades,
        });
        crate::util::por(&mut j, "relatorio", relatorio);
        Ok(j)
    }

    /// O agente entrega um resultado (corpo binário, ver `servico::nos`).
    #[allow(clippy::too_many_lines)]
    pub fn entregar(&self, chave: [u8; 32], corpo: &[u8]) -> R<Value> {
        let mut r = Reader::new(corpo);
        let ler = |e: hyurax_codec::CodecError| Erro::pedido(format!("entrega mal formada: {e}"));
        let atribuicao = r.u64().map_err(ler)?;
        let registro = r.var_bytes().map_err(ler)?.to_vec();
        let assinatura: [u8; 64] = r.fixed().map_err(ler)?;
        let ms_calculo = r.u64().map_err(ler)?;
        let gpu = r.u8().map_err(ler)? == 1;
        let resultado = r.var_bytes().map_err(ler)?.to_vec();
        r.finish().map_err(ler)?;
        let agora = self.agora();

        // 1. de quem é, o que era para fazer
        let (no_id, tarefa, unidade, esp, semente) = {
            let n = self.nucleo()?;
            let no_id = *n.estado.no_por_chave.get(&chave).ok_or_else(|| Erro { status: 410, mensagem: "computador não registrado".into() })?;
            let a = n.estado.atribuicoes.get(&atribuicao).filter(|a| a.no == no_id).ok_or_else(|| Erro::nao_achado("atribuição desconhecida para este computador"))?;
            if !a.aberta() {
                return Err(Erro::conflito("a atribuição já foi fechada (prazo, cancelamento ou entrega anterior)"));
            }
            if agora > a.prazo_ms.saturating_add(FOLGA_DE_ENTREGA_MS) {
                return Err(Erro::conflito("o prazo da atribuição venceu"));
            }
            let t = n.estado.tarefas.get(&a.tarefa).ok_or_else(|| Erro::interno("tarefa sumiu"))?;
            let (esp, semente) = t.unidade(a.unidade).ok_or_else(|| Erro::interno("unidade inválida"))?;
            (no_id, a.tarefa, a.unidade, esp, semente)
        };

        // 2. confere fora da trava (Freivalds pode levar alguns milissegundos)
        let julgamento = conferir(&chave, &registro, &assinatura, &esp, &semente, &resultado);
        let (operacoes, prova) = match &julgamento {
            Ok((ops, prova)) => (*ops, prova.clone()),
            Err(_) => (0, String::new()),
        };
        if julgamento.is_ok() {
            std::fs::write(self.arquivo_da_entrega(atribuicao), &resultado).map_err(|e| Erro::interno(&format!("não gravei a entrega: {e}")))?;
        }

        // 3. grava e julga
        let mut n = self.nucleo()?;
        if !n.estado.atribuicoes.get(&atribuicao).is_some_and(crate::estado::Atribuicao::aberta) {
            let _ = std::fs::remove_file(self.arquivo_da_entrega(atribuicao));
            return Err(Erro::conflito("a atribuição fechou enquanto o resultado chegava"));
        }
        let mut eventos = vec![Evento::ResultDelivered {
            atribuicao,
            hash: hex(&hash_do_resultado(&resultado)),
            operacoes,
            bytes: u64::try_from(resultado.len()).unwrap_or(u64::MAX),
            ms_calculo,
            gpu,
            prova,
            t: agora,
        }];
        // a primeira entrega prova que a execução começou, mesmo que o agente
        // não tenha mandado progresso antes
        if n.estado.tarefas.get(&tarefa).is_some_and(|x| matches!(x.estado, EstadoTarefa::Queued | EstadoTarefa::Scheduled | EstadoTarefa::Dispatched)) {
            eventos.push(Evento::TaskState { tarefa, estado: EstadoTarefa::Running, motivo: String::new(), por: 0, t: agora });
        }
        let resposta = match &julgamento {
            Err(motivo) => {
                eventos.push(Evento::AssignmentClosed { atribuicao, desfecho: Desfecho::Recusada, motivo: motivo.clone(), t: agora });
                json!({ "aceito": false, "desfecho": "recusada", "motivo": motivo })
            }
            Ok(_) => {
                // amostragem: a primeira entrega de uma unidade sem cópias pode
                // ser conferida por outro nó (o nó não sabe qual)
                if let Some(ev) = self.sortear_conferencia(&n.estado, tarefa, unidade, no_id, &esp, agora)? {
                    eventos.push(ev);
                }
                json!({ "aceito": true, "desfecho": "recebida", "motivo": "" })
            }
        };
        self.registrar(&mut n, eventos)?;
        if julgamento.is_err() {
            self.punir_se_preciso(&mut n, no_id)?;
        }
        self.julgar_unidade(&mut n, tarefa, unidade)?;
        self.depois_de_mudar_unidade(&mut n, tarefa)?;
        let desfecho = n.estado.atribuicoes.get(&atribuicao).and_then(|a| a.desfecho.as_ref()).map(|d| super::nos::desfecho_nome(d.0));
        let mut resposta = resposta;
        if let Some(d) = desfecho {
            crate::util::por(&mut resposta, "desfecho", json!(d));
        }
        Ok(resposta)
    }

    fn sortear_conferencia(&self, e: &Estado, tarefa: Id, unidade: u64, no_id: Id, esp: &Especificacao, agora: u64) -> R<Option<Evento>> {
        let Some(t) = e.tarefas.get(&tarefa) else { return Ok(None) };
        let Some(u) = t.unidades.get(&unidade) else { return Ok(None) };
        let ja_entregue = !u.entregas.is_empty();
        if esp.tipo() == TipoDeTrabalho::Matriz || t.pedido.copias > 1 || u.conferencias > 0 || ja_entregue {
            return Ok(None);
        }
        let Some(no) = e.nos.get(&no_id) else { return Ok(None) };
        let h = no.historico;
        let bp = regras::amostragem_bp(h.pontuacao(), h.verificadas);
        if sorteio_bp().map_err(|x| Erro::interno(&x))? >= bp {
            return Ok(None);
        }
        // só pede se houver outro fornecedor que possa conferir agora
        let v = self.vivo()?;
        let (_, donos, _) = capacidade(e, &v, esp.tipo(), t.pedido.gpu, agora, t.cliente);
        let outro = e.nos.values().any(|x| {
            x.dono != no.dono
                && x.dono != t.cliente
                && !x.removido
                && x.adm == EstadoAdm::Normal
                && v.pulsos.get(&x.id).is_some_and(|p| agora.saturating_sub(p.ultimo_ms) <= INSTAVEL_MS && (p.tipos & 1u16.wrapping_shl(u32::from(esp.tipo().codigo()))) != 0)
        });
        if donos == 0 || !outro {
            return Ok(None);
        }
        let motivo = if h.verificadas < regras::PERIODO_DE_PROVA { "prova" } else { "amostragem" };
        Ok(Some(Evento::CheckRequested { tarefa, unidade, motivo: motivo.into(), t: agora }))
    }

    /// Fecha a unidade quando há cópias iguais o bastante.
    pub(crate) fn julgar_unidade(&self, n: &mut Nucleo, tarefa: Id, unidade: u64) -> R<()> {
        let agora = self.agora();
        let Some(t) = n.estado.tarefas.get(&tarefa) else { return Ok(()) };
        let Some(u) = t.unidades.get(&unidade).cloned() else { return Ok(()) };
        if u.fechada() {
            return Ok(());
        }
        let (grupo, hash) = u.maior_grupo();
        let necessarias = usize::from(u.necessarias.max(1));
        let mut eventos = Vec::new();
        let mut aceitas: Vec<Id> = Vec::new();
        if grupo >= necessarias
            && let Some((hash, ops)) = hash
        {
            let membros: Vec<Id> = u.entregas.iter().filter(|(_, h, o)| *h == hash && *o == ops).map(|(a, _, _)| *a).collect();
            // o cliente paga as cópias que pediu; o resto é conferência da plataforma
            let copias = usize::from(t.pedido.copias.max(1));
            let mut ordem: Vec<Id> = membros.clone();
            ordem.sort_by_key(|a| (n.estado.atribuicoes.get(a).map(|x| x.papel != Papel::Copia), *a));
            let pagas: Vec<(Id, Pagador)> = ordem.iter().enumerate().map(|(k, a)| (*a, if k < copias { Pagador::Cliente } else { Pagador::Plataforma })).collect();
            let verificacao = if t.modelo.tipo() == TipoDeTrabalho::Matriz {
                "freivalds"
            } else if necessarias >= 2 {
                "concordancia"
            } else {
                "integridade"
            };
            for a in &membros {
                eventos.push(Evento::AssignmentClosed { atribuicao: *a, desfecho: Desfecho::Aceita, motivo: String::new(), t: agora });
            }
            for (a, h, o) in &u.entregas {
                if *h != hash || *o != ops {
                    let motivo = if *h == hash { "operações diferentes da maioria" } else { "resultado diferente da maioria" };
                    eventos.push(Evento::AssignmentClosed { atribuicao: *a, desfecho: Desfecho::Divergente, motivo: motivo.into(), t: agora });
                }
            }
            // as que ainda rodam não são mais precisas
            let mut parar = Vec::new();
            for a in &u.em_voo {
                eventos.push(Evento::AssignmentClosed { atribuicao: *a, desfecho: Desfecho::Cancelada, motivo: "a unidade fechou com outras cópias".into(), t: agora });
                if let Some(x) = n.estado.atribuicoes.get(a) {
                    parar.push((x.no, *a));
                }
            }
            eventos.push(Evento::UnitCompleted { tarefa, unidade, hash: hex(&hash), pagas, verificacao: verificacao.into(), t: agora });
            aceitas = membros;
            if !parar.is_empty() {
                let mut v = self.vivo()?;
                for (no, a) in parar {
                    v.cancelar.entry(no).or_default().push(a);
                }
            }
        } else if u.entregas.len() >= necessarias.saturating_add(2) {
            // discordância que não se resolve: o motor não deveria variar
            for (a, _, _) in &u.entregas {
                eventos.push(Evento::AssignmentClosed { atribuicao: *a, desfecho: Desfecho::Dispensada, motivo: "sem maioria entre as entregas".into(), t: agora });
            }
            eventos.push(Evento::UnitFailed { tarefa, unidade, motivo: "as entregas não chegaram a uma maioria".into(), t: agora });
            eventos.push(Evento::SecurityAlert {
                id: n.estado.seq.alerta,
                tipo: "verificacao:sem-maioria".into(),
                severidade: "alta".into(),
                alvo: format!("tarefa:{tarefa}"),
                detalhe: format!("unidade {unidade}: {} entregas sem maioria", u.entregas.len()),
                t: agora,
            });
        }
        if eventos.is_empty() {
            return Ok(());
        }
        let divergentes: Vec<Id> = eventos
            .iter()
            .filter_map(|ev| match ev {
                Evento::AssignmentClosed { atribuicao, desfecho: Desfecho::Divergente, .. } => n.estado.atribuicoes.get(atribuicao).map(|a| a.no),
                _ => None,
            })
            .collect();
        self.registrar(n, eventos)?;
        // arquivos: o primeiro aceito vira o resultado; o resto sai
        if let Some(primeiro) = aceitas.first() {
            let destino = self.arquivo_do_resultado(tarefa, unidade);
            if let Some(pai) = destino.parent() {
                let _ = std::fs::create_dir_all(pai);
            }
            if std::fs::rename(self.arquivo_da_entrega(*primeiro), &destino).is_err() {
                eprintln!("[resultado] não movi a entrega {primeiro} para {}", destino.display());
            }
        }
        for (a, _, _) in &u.entregas {
            let _ = std::fs::remove_file(self.arquivo_da_entrega(*a));
        }
        for no in divergentes {
            self.punir_se_preciso(n, no)?;
        }
        Ok(())
    }

    /// Quarentena para quem passou do limite de resultados errados.
    fn punir_se_preciso(&self, n: &mut Nucleo, no_id: Id) -> R<()> {
        let Some(no) = n.estado.nos.get(&no_id) else { return Ok(()) };
        let h = no.historico;
        if no.adm != EstadoAdm::Normal || !regras::quarentena(h.pontuacao(), h.incorretas) {
            return Ok(());
        }
        let t = self.agora();
        let motivo = format!("reputação {} e {} resultado(s) errado(s)", h.pontuacao(), h.incorretas);
        let alerta = n.estado.seq.alerta;
        self.registrar(
            n,
            vec![
                Evento::NodeAdmin { no: no_id, estado: EstadoAdm::Quarantined, motivo: motivo.clone(), por: 0, t },
                Evento::SecurityAlert { id: alerta, tipo: "fraude:resultado-falso".into(), severidade: "alta".into(), alvo: format!("no:{no_id}"), detalhe: motivo, t },
            ],
        )?;
        self.cancelar_do_no(n, no_id, "nó em quarentena")
    }

    /// Depois de qualquer mudança numa unidade: tentativas esgotadas viram
    /// falha, e a tarefa fecha quando tudo fechou.
    pub(crate) fn depois_de_mudar_unidade(&self, n: &mut Nucleo, tarefa: Id) -> R<()> {
        let agora = self.agora();
        let Some(t) = n.estado.tarefas.get(&tarefa) else { return Ok(()) };
        if t.estado.final_() {
            return Ok(());
        }
        let mut eventos = Vec::new();
        for (&i, u) in &t.unidades {
            if !u.fechada() && u.tentativas >= TENTATIVAS_MAX && u.em_voo.is_empty() && u.entregas.is_empty() {
                eventos.push(Evento::UnitFailed { tarefa, unidade: i, motivo: format!("{} tentativas sem resultado aceito", u.tentativas), t: agora });
            }
        }
        self.registrar(n, eventos)?;
        let Some(t) = n.estado.tarefas.get(&tarefa) else { return Ok(()) };
        if t.tudo_fechado() {
            let (estado, motivo) = if t.concluidas > 0 {
                (EstadoTarefa::Completed, format!("{} de {} unidades conferidas", t.concluidas, t.pedido.unidades))
            } else {
                (EstadoTarefa::Failed, "nenhuma unidade pôde ser concluída".into())
            };
            return self.finalizar(n, tarefa, estado, &motivo, 0);
        }
        // tudo despachado, nada rodando, e ainda há unidade aberta: falta só
        // conferência ou desempate
        if t.proxima >= t.pedido.unidades && t.em_voo == 0 && matches!(t.estado, EstadoTarefa::Running | EstadoTarefa::Dispatched | EstadoTarefa::Scheduled) {
            self.registrar(n, vec![Evento::TaskState { tarefa, estado: EstadoTarefa::Verifying, motivo: "esperando conferências".into(), por: 0, t: agora }])?;
        }
        Ok(())
    }

    /// Fecha a tarefa: cancela o que roda, cobra o que foi conferido, divide
    /// e devolve o resto da reserva (seções 14 e 22).
    pub(crate) fn finalizar(&self, n: &mut Nucleo, tarefa: Id, estado: EstadoTarefa, motivo: &str, por: Id) -> R<()> {
        let agora = self.agora();
        // 1. para o que ainda roda
        let abertas: Vec<(Id, Id)> = n.estado.atribuicoes.values().filter(|a| a.tarefa == tarefa && a.desfecho.is_none()).map(|a| (a.id, a.no)).collect();
        if !abertas.is_empty() {
            let eventos = abertas
                .iter()
                .map(|(a, _)| Evento::AssignmentClosed { atribuicao: *a, desfecho: Desfecho::Cancelada, motivo: format!("tarefa encerrada: {motivo}"), t: agora })
                .collect();
            self.registrar(n, eventos)?;
            let mut v = self.vivo()?;
            for (a, no) in &abertas {
                v.cancelar.entry(*no).or_default().push(*a);
            }
            drop(v);
            for (a, _) in &abertas {
                let _ = std::fs::remove_file(self.arquivo_da_entrega(*a));
            }
        }
        // 2. liquida
        let t = n.estado.tarefas.get(&tarefa).ok_or_else(|| Erro::interno("tarefa sumiu"))?;
        let execucoes: Vec<(u64, u128, Pagador)> = t
            .execucoes
            .iter()
            .map(|((no, p), micro)| (*no, *micro, if *p == crate::estado::PagadorOrd::Cliente { Pagador::Cliente } else { Pagador::Plataforma }))
            .collect();
        let liq = regras::liquidar(&execucoes, t.fornecedor_bp)?;
        let reserva = t.reserva;
        let cobranca_bruta = i64::try_from(liq.cobranca).map_err(|_| Erro::interno("cobrança fora da faixa"))?;
        // a reserva é o teto: o que passar dela fica por conta da plataforma
        let cobranca = cobranca_bruta.min(reserva);
        let por_no: Vec<(Id, i64)> = liq.fornecedores.iter().map(|(no, c)| (*no, i64::try_from(*c).unwrap_or(i64::MAX))).filter(|(_, c)| *c > 0).collect();
        let soma_nos = por_no.iter().map(|(_, c)| *c).fold(0i64, i64::saturating_add);
        let plataforma = cobranca.saturating_sub(soma_nos);
        let devolvido = reserva.saturating_sub(cobranca);
        let retencao = u64::from(n.estado.config.retencao_dias).saturating_mul(86_400_000);
        let libera_ms = agora.saturating_add(retencao);
        let (cliente, moeda) = (t.cliente, t.moeda);
        let mut linhas = vec![
            Linha { conta: Conta::ClienteReservado(cliente), moeda, valor: reserva.saturating_neg() },
            Linha { conta: Conta::ClienteDisponivel(cliente), moeda, valor: devolvido },
            Linha { conta: Conta::PlataformaReceita, moeda, valor: plataforma },
        ];
        for (no, c) in &por_no {
            let dono = n.estado.nos.get(no).map_or(0, |x| x.dono);
            linhas.push(Linha { conta: Conta::FornecedorPendente(dono), moeda, valor: *c });
        }
        let mut eventos = vec![
            Evento::TaskSettled {
                tarefa,
                cobranca,
                por_no,
                conferencia: i64::try_from(liq.conferencia).unwrap_or(i64::MAX),
                plataforma,
                devolvido,
                libera_ms,
                t: agora,
            },
            Plataforma::lancamento(&n.estado, 0, agora, Motivo::Liquidacao, format!("tarefa:{tarefa}"), linhas),
            Evento::TaskState { tarefa, estado, motivo: motivo.to_string(), por, t: agora },
        ];
        if cobranca_bruta > reserva {
            eventos.push(Evento::SecurityAlert {
                id: n.estado.seq.alerta,
                tipo: "cobranca:acima-da-reserva".into(),
                severidade: "media".into(),
                alvo: format!("tarefa:{tarefa}"),
                detalhe: format!("a conta deu {cobranca_bruta} e a reserva era {reserva}; o cliente pagou só a reserva"),
                t: agora,
            });
        }
        self.registrar(n, eventos)
    }

    pub(crate) fn vencer_prazos(&self) -> R<()> {
        let agora = self.agora();
        let mut n = self.nucleo()?;
        // atribuições vencidas
        let vencidas: Vec<(Id, Id, Id)> = n.estado.atribuicoes.values().filter(|a| a.aberta() && a.prazo_ms < agora).map(|a| (a.id, a.tarefa, a.no)).collect();
        if !vencidas.is_empty() {
            let eventos = vencidas.iter().map(|(a, _, _)| Evento::AssignmentClosed { atribuicao: *a, desfecho: Desfecho::Vencida, motivo: "prazo vencido sem entrega".into(), t: agora }).collect();
            self.registrar(&mut n, eventos)?;
            {
                let mut v = self.vivo()?;
                for (a, _, no) in &vencidas {
                    v.cancelar.entry(*no).or_default().push(*a);
                }
            }
            let mut tarefas: Vec<Id> = vencidas.iter().map(|x| x.1).collect();
            tarefas.dedup();
            for t in tarefas {
                self.depois_de_mudar_unidade(&mut n, t)?;
            }
        }
        // tarefas vencidas
        let tarefas: Vec<Id> = n.estado.ativas.iter().filter(|id| n.estado.tarefas.get(id).is_some_and(|t| t.prazo_ms < agora)).copied().collect();
        for t in tarefas {
            self.finalizar(&mut n, t, EstadoTarefa::Timeout, "o prazo da tarefa venceu", 0)?;
        }
        // conferências que nenhum nó pegou
        let mut dispensar = Vec::new();
        for id in n.estado.ativas.clone() {
            let Some(t) = n.estado.tarefas.get(&id) else { continue };
            for (&i, u) in &t.unidades {
                // só dispensa quando as entregas concordam: numa divergência,
                // fechar sem o desempate seria escolher um lado no escuro
                let concordam = !u.entregas.is_empty() && u.maior_grupo().0 == u.entregas.len();
                if !u.fechada() && u.conferencias > 0 && u.em_voo.is_empty() && concordam && agora.saturating_sub(u.pronta_ms) > CONFERENCIA_ESPERA_MS {
                    dispensar.push((id, i));
                }
            }
        }
        for (tarefa, unidade) in dispensar {
            self.registrar(&mut n, vec![Evento::CheckDropped { tarefa, unidade, motivo: "nenhum outro fornecedor disponível no prazo".into(), t: agora }])?;
            self.julgar_unidade(&mut n, tarefa, unidade)?;
            self.depois_de_mudar_unidade(&mut n, tarefa)?;
        }
        Ok(())
    }

    pub(crate) fn fechar_tarefas(&self) -> R<()> {
        let mut n = self.nucleo()?;
        for id in n.estado.ativas.clone() {
            self.depois_de_mudar_unidade(&mut n, id)?;
        }
        Ok(())
    }

    pub(crate) fn apagar_resultados_vencidos(&self) -> R<()> {
        let agora = self.agora();
        let mut n = self.nucleo()?;
        let vencidas: Vec<Id> = n
            .estado
            .tarefas
            .values()
            .filter(|t| !t.resultados_apagados && t.concluida_ms.is_some_and(|c| agora.saturating_sub(c) > GUARDA_DOS_RESULTADOS_MS))
            .map(|t| t.id)
            .take(20)
            .collect();
        for id in vencidas {
            let _ = std::fs::remove_dir_all(self.pasta().join("resultados").join(id.to_string()));
            self.registrar(&mut n, vec![Evento::ResultsPurged { tarefa: id, t: agora }])?;
        }
        Ok(())
    }

    /// Abertas de um nó (para a API de admin).
    pub fn abertas(&self, no: Id) -> R<usize> {
        self.ler(|e| abertas_do_no(e, no).len())
    }
}

/// Confere uma entrega: registro de prova, assinatura, entrada, hash,
/// tamanho, operações e a conferência que o servidor consegue fazer barato.
/// Devolve as operações (pelo modelo de custo) e o hash da prova.
fn conferir(chave: &[u8; 32], registro: &[u8], assinatura: &[u8; 64], esp: &Especificacao, semente: &[u8; HASH_LEN], resultado: &[u8]) -> Result<(u64, String), String> {
    let reg = RegistroDeProva::decodificar(registro).map_err(|e| format!("registro de prova ilegível: {e}"))?;
    if &reg.worker != chave {
        return Err("o registro de prova é de outro worker".into());
    }
    if !reg.assinatura_confere(assinatura) {
        return Err("a assinatura do registro de prova não confere".into());
    }
    if reg.especificacao != *esp {
        return Err("o registro é de outra especificação".into());
    }
    if reg.entrada != hash_da_entrada(esp, semente).map_err(|e| e.to_string())? {
        return Err("o registro é de outra entrada (semente)".into());
    }
    if reg.resultado != hash_do_resultado(resultado) {
        return Err("o hash do resultado não confere com o registro".into());
    }
    if u64::try_from(resultado.len()).unwrap_or(u64::MAX) > trabalho::tamanho_maximo_do_resultado(esp) {
        return Err("resultado maior que o motor produz".into());
    }
    let ops = match esp.operacoes_fixas() {
        // nas rotas o modelo dá o teto; a execução para no ótimo local e
        // relata o que fez (igual em todo nó honesto: as cópias conferem)
        Some(teto) if esp.tipo() == TipoDeTrabalho::Rotas => {
            if reg.operacoes > teto || reg.operacoes < u64::from(esp.tamanho()) {
                return Err(format!("operações {} fora do modelo de custo (até {teto})", reg.operacoes));
            }
            reg.operacoes
        }
        Some(fixas) => {
            if reg.operacoes != fixas {
                return Err(format!("operações {} diferentes do modelo de custo ({fixas})", reg.operacoes));
            }
            fixas
        }
        None => {
            // a mochila: n · (C + 1), com C da instância (barato de gerar)
            let inst = trabalho::instancia_mochila(esp.tamanho(), semente).map_err(|e| e.to_string())?;
            let esperado = u64::from(esp.tamanho()).saturating_mul(u64::from(inst.capacidade).saturating_add(1));
            if reg.operacoes != esperado {
                return Err(format!("operações {} diferentes do modelo de custo ({esperado})", reg.operacoes));
            }
            esperado
        }
    };
    match esp.tipo() {
        TipoDeTrabalho::Matriz => trabalho::verificar(esp, semente, resultado).map_err(|r| format!("Freivalds recusou: {r}"))?,
        TipoDeTrabalho::Rotas => {
            hyurax_ultrax::rotas::certificar(esp, resultado).map_err(|r| format!("rota recusada: {r}"))?;
        }
        TipoDeTrabalho::Triagem if hyurax_ultrax::triagem::decodificar(resultado).is_none() => {
            return Err("resultado de triagem mal formado".into());
        }
        _ => {}
    }
    Ok((ops, hex(&reg.hash())))
}

/// Uma tarefa em JSON.
pub fn json_da_tarefa(t: &Tarefa, detalhe: bool) -> Value {
    let custo_parcial: u128 = t.execucoes.iter().filter(|((_, p), _)| *p == crate::estado::PagadorOrd::Cliente).map(|(_, m)| *m).fold(0u128, u128::saturating_add);
    let nome = t.modelo.tipo().descricao();
    let mut j = json!({
        "id": t.id,
        "estado": t.estado.nome(),
        "motivo": t.motivo,
        "tipo": t.pedido.tipo,
        "nome": nome,
        "descricao": t.pedido.descricao,
        "moeda": t.moeda.codigo(),
        "criada_ms": t.criada_ms,
        "iniciada_ms": t.iniciada_ms,
        "concluida_ms": t.concluida_ms,
        "prazo_ms": t.prazo_ms,
        "via_api": t.via_api,
        "progresso": {
            "unidades": t.pedido.unidades, "concluidas": t.concluidas, "falhas": t.falhas,
            "em_voo": t.em_voo, "despachadas": t.proxima,
        },
        "custo": {
            "reserva_centavos": t.reserva,
            "reserva": t.moeda.formatar(t.reserva),
            "ate_agora": t.moeda.formatar(i64::try_from(regras::centavos(custo_parcial)).unwrap_or(i64::MAX)),
            "cobrado": t.liquidada.as_ref().map(|l| t.moeda.formatar(l.cobranca)),
            "cobrado_centavos": t.liquidada.as_ref().map(|l| l.cobranca),
            "devolvido": t.liquidada.as_ref().map(|l| t.moeda.formatar(l.devolvido)),
        },
        "verificacao": {
            "freivalds": t.verificacao.freivalds, "concordancia": t.verificacao.concordancia,
            "integridade": t.verificacao.integridade, "divergencias": t.verificacao.divergencias,
        },
    });
    if detalhe {
        crate::util::por(&mut j, "pedido", serde_json::to_value(&t.pedido).unwrap_or(Value::Null));
        crate::util::por(&mut j, "politica", politica_json(&t.pedido, t.modelo.memoria_bytes()));
        crate::util::por(&mut j, "job", json!(hex(&t.job)));
        crate::util::por(&mut j, "operacoes_conferidas", json!(t.operacoes.to_string()));
        let mut falhas: BTreeMap<String, u64> = BTreeMap::new();
        for motivo in t.unidades.values().filter_map(|u| u.falhou.clone()) {
            let c = falhas.entry(motivo).or_insert(0);
            *c = c.saturating_add(1);
        }
        crate::util::por(&mut j, "falhas_por_motivo", serde_json::to_value(falhas).unwrap_or(Value::Null));
    }
    j
}

impl Plataforma {
    /// O mercado (seção 24): preços por categoria e a capacidade online agora.
    pub fn marketplace(&self) -> R<Value> {
        let agora = self.agora();
        let n = self.nucleo()?;
        let e = &n.estado;
        let v = self.vivo()?;
        let mut nos = 0u32;
        let mut nucleos = 0u64;
        let mut ram_mib = 0u64;
        let mut gpus = Vec::new();
        for no in e.nos.values().filter(|x| !x.removido && x.adm == EstadoAdm::Normal && !x.pausado) {
            if v.pulsos.get(&no.id).is_some_and(|p| agora.saturating_sub(p.ultimo_ms) <= INSTAVEL_MS && !p.pausado_local) {
                nos = nos.saturating_add(1);
                nucleos = nucleos.saturating_add(u64::from(no.limites.linhas));
                ram_mib = ram_mib.saturating_add(no.limites.ram_mib);
                if no.limites.gpu {
                    gpus.push(no.hardware.gpu_modelo.clone());
                }
            }
        }
        let precos: Vec<Value> = e
            .config
            .tabelas
            .iter()
            .map(|(m, t)| {
                let micro = |x: u64| m.formatar(i64::try_from(regras::centavos(u128::from(x))).unwrap_or(i64::MAX));
                json!({
                    "moeda": m.codigo(),
                    "cpu_4_nucleos_hora": micro(t.cpu_nucleo_hora.saturating_mul(4)),
                    "cpu_nucleo_hora": micro(t.cpu_nucleo_hora),
                    "ram_gb_hora": micro(t.ram_gib_hora),
                    "gpu_hora": micro(t.gpu_hora),
                    "tabela": t,
                })
            })
            .collect();
        Ok(json!({
            "precos": precos,
            "capacidade": { "nos_online": nos, "nucleos": nucleos, "ram_mib": ram_mib, "gpus": gpus },
            "motores": motores_ligados(&e.config.motores_desligados),
            "parte_do_fornecedor_pct": f64::from(e.config.fornecedor_bp) / 100.0,
            "retencao_dias": e.config.retencao_dias,
            "como_cobra": "por trabalho conferido: as operações do modelo de custo de cada motor, convertidas em horas de um núcleo de referência (1 bilhão de operações por segundo)",
            "cadastro_aberto": e.config.cadastro_aberto,
            "contato": e.config.contato,
            "parada_de_emergencia": e.config.parada_de_emergencia,
            "modo_teste": self.opcoes.modo_teste,
        }))
    }
}

/// Os motores do catálogo que não estão desligados, resumidos.
fn motores_ligados(desligados: &std::collections::BTreeSet<String>) -> Vec<Value> {
    let c = catalogo();
    let campo = |x: &Value, k: &str| x.get(k).cloned().unwrap_or(Value::Null);
    c.get("motores")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter(|x| !desligados.contains(x.get("tipo").and_then(Value::as_str).unwrap_or("")))
                .map(|x| json!({ "tipo": campo(x, "tipo"), "nome": campo(x, "nome"), "categoria": campo(x, "categoria"), "gpu": campo(x, "gpu") }))
                .collect()
        })
        .unwrap_or_default()
}
