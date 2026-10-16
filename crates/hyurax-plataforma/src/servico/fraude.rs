//! Antifraude (seção 28 do documento da V1).
//!
//! Os sinais que já barram na hora, cada um no lugar onde acontece:
//!
//! | Sinal | Onde | O que faz |
//! |---|---|---|
//! | muitas contas do mesmo IP | `contas::cadastrar` | recusa e alerta |
//! | senha errada em sequência | `contas::entrar` | trava 15 min e alerta |
//! | criação excessiva de tarefas | `tarefas::criar_tarefa` | limite por hora |
//! | pagamentos pendentes demais | `financeiro::criar_pagamento` | recusa |
//! | resultado falso | `tarefas::julgar_unidade` | reputação e quarentena |
//! | nó do próprio cliente | `agendador` | nunca recebe a tarefa dele |
//! | mesmo hardware em contas diferentes | `nos::registrar_no` | alerta |
//! | pedido de agente repetido | `nos::autenticar_agente` | recusa e alerta |
//! | estorno | `financeiro::admin_estornar_pagamento` | bloqueia e segura ganhos |
//!
//! Aqui fica a varredura de padrões, a cada 10 minutos: fornecedor que só
//! ganha de um cliente que se cadastrou do mesmo IP (dinheiro girando entre
//! contas da mesma pessoa) e cliente com muitas tarefas que nunca terminam.

use std::collections::BTreeMap;

use super::{Plataforma, R};
use crate::modelo::Id;

/// Intervalo da varredura.
pub const VARREDURA_MS: u64 = 10 * 60_000;

impl Plataforma {
    /// A varredura de padrões (chamada pela manutenção).
    pub fn varredura_de_fraude(&self) -> R<()> {
        let agora = self.agora();
        {
            let mut v = self.vivo()?;
            if agora.saturating_sub(v.ultima_varredura) < VARREDURA_MS {
                return Ok(());
            }
            v.ultima_varredura = agora;
        }
        let suspeitas: Vec<(String, String)> = self.ler(|e| {
            let mut s = Vec::new();
            // 1. ganhos de um cliente só, do mesmo IP de cadastro
            let mut por_par: BTreeMap<(Id, Id), u64> = BTreeMap::new();
            let mut por_fornecedor: BTreeMap<Id, u64> = BTreeMap::new();
            for t in e.tarefas.values().filter(|t| t.liquidada.is_some() && agora.saturating_sub(t.criada_ms) < 7 * 86_400_000) {
                for (no, _) in t.liquidada.as_ref().map(|l| l.por_no.clone()).unwrap_or_default() {
                    if let Some(dono) = e.nos.get(&no).map(|x| x.dono) {
                        let c = por_par.entry((dono, t.cliente)).or_insert(0);
                        *c = c.saturating_add(1);
                        let f = por_fornecedor.entry(dono).or_insert(0);
                        *f = f.saturating_add(1);
                    }
                }
            }
            for ((dono, cliente), quantas) in por_par {
                let total = por_fornecedor.get(&dono).copied().unwrap_or(0);
                let mesmo_ip = e.usuarios.get(&dono).zip(e.usuarios.get(&cliente)).is_some_and(|(a, b)| a.ip_cadastro == b.ip_cadastro && !a.ip_cadastro.is_empty());
                if quantas >= 5 && quantas.saturating_mul(10) >= total.saturating_mul(9) && mesmo_ip {
                    s.push((format!("usuario:{dono}"), format!("{quantas} de {total} tarefas pagas na semana vieram do cliente {cliente}, cadastrado do mesmo IP")));
                }
            }
            // 2. cliente com muitas tarefas que falham ou vencem
            let mut ruins: BTreeMap<Id, (u64, u64)> = BTreeMap::new();
            for t in e.tarefas.values().filter(|t| t.estado.final_() && agora.saturating_sub(t.criada_ms) < 86_400_000) {
                let r = ruins.entry(t.cliente).or_insert((0, 0));
                r.1 = r.1.saturating_add(1);
                if t.concluidas == 0 {
                    r.0 = r.0.saturating_add(1);
                }
            }
            for (cliente, (falhas, total)) in ruins {
                if total >= 20 && falhas.saturating_mul(10) >= total.saturating_mul(8) {
                    s.push((format!("usuario:{cliente}"), format!("{falhas} de {total} tarefas do dia sem nenhuma unidade concluída")));
                }
            }
            s
        })?;
        for (alvo, detalhe) in suspeitas {
            self.alertar_uma_vez("fraude:padrao", "media", &alvo, &detalhe)?;
        }
        Ok(())
    }
}
