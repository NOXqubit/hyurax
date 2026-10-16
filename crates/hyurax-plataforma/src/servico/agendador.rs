//! O agendador (seção 12 do documento da V1): quando um nó pulsa com
//! capacidade livre, escolhe as unidades que cabem nele.
//!
//! ```text
//! tarefa na fila → requisitos → nós disponíveis → ordem → escolha → despacho
//! ```
//!
//! O que entra na conta:
//!
//! - **cabe?** motor que o agente executa, GPU quando pedida, memória da
//!   unidade dentro do que o fornecedor liberou e do que está livre, região,
//!   núcleos em paralelo da tarefa;
//! - **pode?** o nó não é do próprio cliente (ninguém confere o próprio
//!   trabalho nem gira dinheiro consigo mesmo), e as cópias e conferências de
//!   uma unidade vão para donos diferentes;
//! - **é o mais adequado?** não o mais forte: reputação, estabilidade do pulso,
//!   ocupação e folga de memória (máquina grande fica para unidade grande). Se
//!   outro nó online for bem mais adequado, a unidade espera até 10 s por ele;
//!   depois disso vai para quem pediu;
//! - **justiça entre clientes:** quem foi atendido há mais tempo vem primeiro.
//!
//! Retentativas e conferências passam na frente de unidades novas: são elas
//! que seguram o fim de uma tarefa.

use std::collections::BTreeSet;

use hyurax_codec::Writer;
use hyurax_ultrax::trabalho;
use serde_json::{Value, json};

use super::nos::{INSTAVEL_MS, abertas_do_no};
use super::{Nucleo, Plataforma, Pulso, R};
use crate::estado::{Estado, No, Tarefa};
use crate::eventos::{Desfecho, EstadoAdm, EstadoTarefa, Evento, Papel};
use crate::modelo::Id;
use crate::util::hex;

/// Unidades entregues por pulso, no máximo.
const POR_PULSO: usize = 4;
/// Quanto uma unidade espera por um nó bem mais adequado.
const ESPERA_POR_ADEQUADO_MS: u64 = 10_000;
/// Diferença de nota que justifica esperar.
const DIFERENCA: i64 = 15;
/// Prazo mínimo de uma atribuição.
const PRAZO_MINIMO_MS: u64 = 120_000;
/// Folga de ritmo: o prazo supõe uma máquina 8 vezes mais lenta que a referência.
const FOLGA: u64 = 8;
/// Prazo máximo de uma atribuição.
const PRAZO_MAXIMO_MS: u64 = 6 * 3_600_000;

/// MiB arredondado para cima.
fn mib(bytes: u64) -> u64 {
    bytes.div_ceil(1 << 20)
}

/// A nota de adequação de um nó para uma unidade de `mem` bytes.
pub fn nota(no: &No, pulso: &Pulso, abertas: usize, mem: u64) -> i64 {
    let rep = i64::from(no.historico.pontuacao());
    let estabilidade = i64::try_from(pulso.seguidos.min(720).saturating_mul(20) / 720).unwrap_or(0);
    let linhas = u64::from(no.limites.linhas.max(1));
    let ocupacao = i64::try_from(u64::try_from(abertas).unwrap_or(u64::MAX).saturating_mul(10).checked_div(linhas).unwrap_or(10)).unwrap_or(10).min(10);
    let folga = {
        let razao = no.limites.ram_mib.checked_div(mib(mem).max(1)).unwrap_or(0);
        i64::from(razao.max(1).ilog2().min(5))
    };
    rep.saturating_add(estabilidade).saturating_sub(ocupacao).saturating_sub(folga)
}

/// O nó pode receber esta tarefa (sem olhar a unidade)?
#[allow(clippy::too_many_arguments)]
fn cabe(e: &Estado, t: &Tarefa, em_voo: u32, no: &No, pulso: &Pulso, ram_mib: u64, gpu_livre: bool, cpu_livre: bool) -> bool {
    let tipo = t.modelo.tipo();
    let mem = t.modelo.memoria_bytes();
    !e.config.motores_desligados.contains(&t.pedido.tipo)
        && t.cliente != no.dono
        && (pulso.tipos & 1u16.wrapping_shl(u32::from(tipo.codigo()))) != 0
        && mib(mem) <= ram_mib
        && if t.pedido.gpu { gpu_livre } else { cpu_livre }
        && (t.pedido.regiao.is_empty() || t.pedido.regiao.eq_ignore_ascii_case(&no.regiao))
        && em_voo < t.pedido.paralelo.max(1)
        && e.usuarios.get(&t.cliente).is_some_and(|u| !u.bloqueado)
}

/// O nó pode pegar esta unidade (donos diferentes nas cópias)?
fn pode_na_unidade(e: &Estado, t: &Tarefa, unidade: u64, no: &No) -> bool {
    let Some(u) = t.unidades.get(&unidade) else { return true };
    u.todas.iter().filter_map(|a| e.atribuicoes.get(a)).all(|a| {
        let ativa = a.desfecho.is_none() || matches!(a.desfecho, Some((Desfecho::Aceita, _, _)));
        a.no != no.id && !(ativa && a.dono == no.dono)
    })
}

/// O prazo de uma atribuição.
pub fn prazo_da_atribuicao(t: &Tarefa, unidade: u64, agora: u64) -> u64 {
    let esp = trabalho::derivar_unidade(&t.modelo, unidade).unwrap_or(t.modelo);
    let ops = esp.operacoes_maximas().saturating_add(trabalho::operacoes_de_verificacao(&esp));
    let ritmo = if t.pedido.gpu { t.tabela.ops_por_s_gpu } else { t.tabela.ops_por_s_cpu }.max(1);
    let ms = ops.saturating_mul(1000).saturating_mul(FOLGA).checked_div(ritmo).unwrap_or(PRAZO_MAXIMO_MS);
    agora.saturating_add(ms.saturating_add(PRAZO_MINIMO_MS).min(PRAZO_MAXIMO_MS)).min(t.prazo_ms.max(agora))
}

impl Plataforma {
    /// Escolhe e grava as atribuições para o nó que pulsou. Devolve o JSON
    /// que vai para o agente.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn despachar(&self, n: &mut Nucleo, no_id: Id, agora: u64) -> R<Vec<Value>> {
        let e = &n.estado;
        if e.config.parada_de_emergencia {
            return Ok(Vec::new());
        }
        let Some(no) = e.nos.get(&no_id).cloned() else { return Ok(Vec::new()) };
        let (pulso, outros, atendido, adiado) = {
            let v = self.vivo()?;
            let Some(p) = v.pulsos.get(&no_id).cloned() else { return Ok(Vec::new()) };
            // os outros nós online, com o que relataram no último pulso
            let outros: Vec<(No, Pulso, usize)> = e
                .nos
                .values()
                .filter(|x| x.id != no_id && !x.removido && !x.pausado && x.adm == EstadoAdm::Normal)
                .filter_map(|x| {
                    let p = v.pulsos.get(&x.id)?;
                    (agora.saturating_sub(p.ultimo_ms) <= INSTAVEL_MS && !p.pausado_local && p.livres > 0).then(|| (x.clone(), p.clone(), abertas_do_no(e, x.id).len()))
                })
                .collect();
            (p, outros, v.atendido.clone(), v.adiado.clone())
        };
        let abertas: Vec<Id> = abertas_do_no(e, no_id);
        let mem_aberta: u64 = abertas
            .iter()
            .filter_map(|a| e.atribuicoes.get(a))
            .filter_map(|a| e.tarefas.get(&a.tarefa).and_then(|t| trabalho::derivar_unidade(&t.modelo, a.unidade).ok()))
            .map(|esp| mib(esp.memoria_bytes()))
            .fold(0u64, u64::saturating_add);
        let gpu_aberta = abertas.iter().filter_map(|a| e.atribuicoes.get(a)).any(|a| e.tarefas.get(&a.tarefa).is_some_and(|t| t.pedido.gpu));
        let cpu_abertas = u32::try_from(abertas.len()).unwrap_or(u32::MAX).saturating_sub(u32::from(gpu_aberta));
        let mut cpu_livres = pulso.livres.min(no.limites.linhas.saturating_sub(cpu_abertas));
        let mut gpu_livre = pulso.gpu && no.limites.gpu && !gpu_aberta;
        let mut ram_livre = pulso.ram_livre_mib.min(no.limites.ram_mib.saturating_sub(mem_aberta));

        // justiça: o cliente atendido há mais tempo primeiro
        let mut fila: Vec<&Tarefa> = e.ativas.iter().filter_map(|id| e.tarefas.get(id)).filter(|t| t.tem_o_que_despachar()).collect();
        fila.sort_by_key(|t| (atendido.get(&t.cliente).copied().unwrap_or(0), t.id));

        let mut escolhidas: Vec<(Id, u64, Papel)> = Vec::new();
        let mut adiar: Vec<((Id, u64), u64)> = Vec::new();
        let mut usados: BTreeSet<(Id, u64)> = BTreeSet::new();
        let mut em_voo_extra: std::collections::BTreeMap<Id, u32> = std::collections::BTreeMap::new();
        let mut novas: std::collections::BTreeMap<Id, u64> = std::collections::BTreeMap::new();
        for t in fila {
            // várias unidades da mesma tarefa no mesmo pulso, até a capacidade
            loop {
                if escolhidas.len() >= POR_PULSO || (cpu_livres == 0 && !gpu_livre) {
                    break;
                }
                let extra = em_voo_extra.get(&t.id).copied().unwrap_or(0);
                if !cabe(e, t, t.em_voo.saturating_add(extra), &no, &pulso, ram_livre, gpu_livre, cpu_livres > 0) {
                    break;
                }
                // primeiro as unidades que já existem e precisam de nó
                let mut candidata: Option<(u64, Papel, u64)> = None;
                for (&i, u) in &t.unidades {
                    if u.precisa_de_no() && !usados.contains(&(t.id, i)) && pode_na_unidade(e, t, i, &no) {
                        let copias_ativas = u
                            .todas
                            .iter()
                            .filter_map(|a| e.atribuicoes.get(a))
                            .filter(|a| a.papel == Papel::Copia && (a.desfecho.is_none() || matches!(a.desfecho, Some((Desfecho::Aceita, _, _)))))
                            .count();
                        let papel = if copias_ativas < usize::from(t.pedido.copias.max(1)) { Papel::Copia } else { Papel::Conferencia };
                        candidata = Some((i, papel, u.pronta_ms));
                        break;
                    }
                }
                // depois a próxima unidade nova (as já escolhidas neste pulso contam)
                let nova = t.proxima.saturating_add(novas.get(&t.id).copied().unwrap_or(0));
                if candidata.is_none() && nova < t.pedido.unidades {
                    candidata = Some((nova, Papel::Copia, adiado.get(&(t.id, nova)).copied().unwrap_or(agora)));
                }
                let Some((unidade, papel, pronta)) = candidata else { break };
                // o mais adequado: outro nó bem melhor recebe primeiro, por até 10 s
                let mem = t.modelo.memoria_bytes();
                let minha = nota(&no, &pulso, abertas.len().saturating_add(escolhidas.len()), mem);
                let melhor_outro = outros
                    .iter()
                    .filter(|(x, p, ab)| {
                        let em_voo_dele = t.em_voo.saturating_add(extra);
                        let _ = ab;
                        cabe(e, t, em_voo_dele, x, p, p.ram_livre_mib.min(x.limites.ram_mib), p.gpu && x.limites.gpu, p.livres > 0) && pode_na_unidade(e, t, unidade, x)
                    })
                    .map(|(x, p, ab)| nota(x, p, *ab, mem))
                    .max();
                let desde = adiado.get(&(t.id, unidade)).copied().unwrap_or(pronta.min(agora));
                if melhor_outro.is_some_and(|m| m > minha.saturating_add(DIFERENCA)) && agora.saturating_sub(desde) < ESPERA_POR_ADEQUADO_MS {
                    if !adiado.contains_key(&(t.id, unidade)) {
                        adiar.push(((t.id, unidade), agora));
                    }
                    break;
                }
                usados.insert((t.id, unidade));
                escolhidas.push((t.id, unidade, papel));
                if unidade >= t.proxima {
                    let c = novas.entry(t.id).or_insert(0);
                    *c = c.saturating_add(1);
                }
                *em_voo_extra.entry(t.id).or_insert(0) = extra.saturating_add(1);
                ram_livre = ram_livre.saturating_sub(mib(mem));
                if t.pedido.gpu {
                    gpu_livre = false;
                } else {
                    cpu_livres = cpu_livres.saturating_sub(1);
                }
            }
        }

        if escolhidas.is_empty() {
            if !adiar.is_empty() {
                let mut v = self.vivo()?;
                for (k, t) in adiar {
                    v.adiado.entry(k).or_insert(t);
                }
            }
            return Ok(Vec::new());
        }

        let mut eventos = Vec::new();
        let mut saida = Vec::new();
        let mut proximo = e.seq.atribuicao;
        let mut clientes = Vec::new();
        let mut agendadas: BTreeSet<Id> = BTreeSet::new();
        for (tid, unidade, papel) in &escolhidas {
            let Some(t) = e.tarefas.get(tid) else { continue };
            let Some((esp, semente)) = t.unidade(*unidade) else { continue };
            let prazo_ms = prazo_da_atribuicao(t, *unidade, agora);
            let id = proximo;
            proximo = proximo.saturating_add(1);
            eventos.push(Evento::TaskAssigned { id, tarefa: *tid, unidade: *unidade, no: no_id, papel: *papel, prazo_ms, t: agora });
            if t.estado == EstadoTarefa::Queued && agendadas.insert(*tid) {
                eventos.push(Evento::TaskState { tarefa: *tid, estado: EstadoTarefa::Scheduled, motivo: format!("nó {no_id} escolhido"), por: 0, t: agora });
            }
            let mut w = Writer::new();
            esp.codificar(&mut w);
            saida.push(json!({
                "id": id,
                "tarefa": tid,
                "unidade": unidade,
                "tipo": esp.tipo().nome(),
                "especificacao": hex(&w.into_bytes()),
                "semente": hex(&semente),
                "job": hex(&t.job),
                "prazo_ms": prazo_ms,
                "gpu": t.pedido.gpu,
                "memoria": esp.memoria_bytes(),
                "papel": if *papel == Papel::Copia { "copia" } else { "conferencia" },
            }));
            clientes.push(t.cliente);
        }
        self.registrar(n, eventos)?;
        let mut v = self.vivo()?;
        for c in clientes {
            v.atendido.insert(c, agora);
        }
        for (tid, unidade, _) in &escolhidas {
            v.adiado.remove(&(*tid, *unidade));
        }
        for (k, t) in adiar {
            v.adiado.entry(k).or_insert(t);
        }
        Ok(saida)
    }
}
