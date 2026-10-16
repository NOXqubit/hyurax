//! Administração (seções 33, 37 e 38 do documento da V1): painel com as
//! métricas, usuários, nós, tarefas, pagamentos, saques, preços e limites,
//! o kill switch em quatro níveis, alertas e o registro de auditoria.
//!
//! Toda ação de administrador vira evento com o número dele (`por`): o
//! registro de auditoria é o próprio diário.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::contas::Quem;
use super::nos::{abertas_do_no, json_do_no};
use super::{Erro, Plataforma, R};
use crate::dinheiro::{Conta, Moeda};
use crate::eventos::{EstadoAdm, EstadoTarefa, Evento};
use crate::modelo::{Config, Id, texto_curto};
use crate::regras;

impl Plataforma {
    /// As métricas da seção 55 e a saúde do sistema.
    #[allow(clippy::too_many_lines)]
    pub fn painel(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        let agora = self.agora();
        let n = self.nucleo()?;
        let e = &n.estado;
        let v = self.vivo()?;
        let online = e.nos.values().filter(|x| !x.removido && v.pulsos.get(&x.id).is_some_and(|p| agora.saturating_sub(p.ultimo_ms) <= super::nos::OFFLINE_MS)).count();
        let mut por_estado: BTreeMap<&str, u64> = BTreeMap::new();
        for t in e.tarefas.values() {
            let c = por_estado.entry(t.estado.nome()).or_insert(0);
            *c = c.saturating_add(1);
        }
        let terminadas = e.tarefas.values().filter(|t| t.estado.final_()).count() as f64;
        let concluidas = e.tarefas.values().filter(|t| t.estado == EstadoTarefa::Completed).count() as f64;
        let mut moedas = Vec::new();
        for m in [Moeda::Brl, Moeda::Usd] {
            let mut gmv: i64 = 0;
            let mut fornecedores: i64 = 0;
            let mut conferencia: i64 = 0;
            let mut pagantes = std::collections::BTreeSet::new();
            for t in e.tarefas.values().filter(|t| t.moeda == m) {
                if let Some(l) = &t.liquidada {
                    gmv = gmv.saturating_add(l.cobranca);
                    fornecedores = fornecedores.saturating_add(l.por_no.iter().map(|x| x.1).fold(0i64, i64::saturating_add));
                    conferencia = conferencia.saturating_add(l.conferencia);
                    if l.cobranca > 0 {
                        pagantes.insert(t.cliente);
                    }
                }
            }
            let taxas: i64 = e.pagamentos.values().filter(|p| p.moeda == m && p.estado == "confirmado").map(|p| p.taxa).fold(0, i64::saturating_add);
            let receita = e.livro.saldo(Conta::PlataformaReceita, m);
            let caixa = e.livro.saldo(Conta::Caixa, m).saturating_neg();
            let take = if gmv > 0 { receita as f64 / gmv as f64 * 100.0 } else { 0.0 };
            let margem = if gmv > 0 { (gmv.saturating_sub(fornecedores).saturating_sub(taxas)) as f64 / gmv as f64 * 100.0 } else { 0.0 };
            let media = gmv.checked_div(i64::try_from(pagantes.len()).unwrap_or(1)).unwrap_or(0);
            moedas.push(json!({
                "moeda": m.codigo(),
                "gmv": m.formatar(gmv), "receita": m.formatar(receita), "caixa": m.formatar(caixa),
                "ganhos_fornecedores": m.formatar(fornecedores), "custo_conferencias": m.formatar(conferencia),
                "taxas_de_pagamento": m.formatar(taxas), "take_rate_pct": take, "margem_bruta_pct": margem,
                "gasto_medio_por_cliente": m.formatar(media), "clientes_pagantes": pagantes.len(),
                "saldo_dos_clientes": m.formatar(e.usuarios.keys().map(|u| e.livro.saldo(Conta::ClienteDisponivel(*u), m)).fold(0, i64::saturating_add)),
                "livro_fecha": e.livro.soma(m) == 0,
            }));
        }
        let ops: u128 = e.tarefas.values().map(|t| t.operacoes).fold(0u128, u128::saturating_add);
        let horas = ops as f64 / regras::Tabela::REAIS.ops_por_s_cpu as f64 / 3600.0;
        let atribs = e.atribuicoes.len() as f64;
        let falhas = e.atribuicoes.values().filter(|a| matches!(a.desfecho, Some((crate::eventos::Desfecho::Vencida | crate::eventos::Desfecho::Abandonada, _, _)))).count() as f64;
        let entregas = e.atribuicoes.values().filter(|a| a.entrega.is_some()).count() as f64;
        let erradas = e.atribuicoes.values().filter(|a| matches!(a.desfecho, Some((crate::eventos::Desfecho::Recusada | crate::eventos::Desfecho::Divergente, _, _)))).count() as f64;
        let mut lat: Vec<u32> = v.saude.latencias.iter().copied().collect();
        lat.sort_unstable();
        let pct = |p: usize| lat.get(lat.len().saturating_mul(p) / 100).copied().unwrap_or(0);
        Ok(json!({
            "produto": {
                "usuarios": e.usuarios.len(),
                "clientes": e.usuarios.values().filter(|u| u.papeis.cliente).count(),
                "fornecedores": e.usuarios.values().filter(|u| u.papeis.fornecedor).count(),
                "nos": e.nos.values().filter(|x| !x.removido).count(),
                "nos_online": online,
                "nos_em_quarentena": e.nos.values().filter(|x| x.adm == EstadoAdm::Quarantined).count(),
                "horas_de_computacao": horas,
                "tarefas": por_estado,
                "taxa_de_sucesso_pct": if terminadas > 0.0 { concluidas / terminadas * 100.0 } else { 0.0 },
            },
            "negocio": moedas,
            "qualidade": {
                "uptime_s": agora.saturating_sub(self.inicio_ms) / 1000,
                "latencia_p50_ms": pct(50), "latencia_p95_ms": pct(95),
                "taxa_de_falha_pct": if atribs > 0.0 { falhas / atribs * 100.0 } else { 0.0 },
                "falha_de_verificacao_pct": if entregas > 0.0 { erradas / entregas * 100.0 } else { 0.0 },
                "pedidos": v.saude.pedidos, "erros_5xx": v.saude.erros_5xx, "erros_4xx": v.saude.erros_4xx,
            },
            "sistema": {
                "eventos_no_diario": n.diario.quantos(),
                "selo_do_diario": n.diario.selo(),
                "ultima_copia_ms": v.saude.ultima_copia_ms,
                "erro_da_copia": v.saude.erro_copia,
                "falhas_de_aplicacao": v.saude.falhas_de_aplicacao,
                "modo_leitura": self.degradada(),
                "tarefas_ativas": e.ativas.len(),
                "atribuicoes_abertas": e.atribuicoes.values().filter(|a| a.aberta()).count(),
                "alertas_abertos": e.alertas.values().filter(|a| a.resolvido.is_none()).count(),
                "parada_de_emergencia": e.config.parada_de_emergencia,
                "modo_teste": self.opcoes.modo_teste,
            },
        }))
    }

    /// Usuários, com busca por e-mail ou nome.
    pub fn admin_usuarios(&self, q: &Quem, busca: &str) -> R<Value> {
        q.admin()?;
        let busca = busca.trim().to_lowercase();
        self.ler(|e| {
            let lista: Vec<Value> = e
                .usuarios
                .values()
                .rev()
                .filter(|u| busca.is_empty() || u.email.contains(&busca) || u.nome.to_lowercase().contains(&busca))
                .take(300)
                .map(|u| {
                    json!({
                        "id": u.id, "email": u.email, "nome": u.nome, "papeis": u.papeis.nomes(), "moeda": u.moeda.codigo(),
                        "pais": u.pais, "bloqueado": u.bloqueado, "motivo_bloqueio": u.motivo_bloqueio, "verificado": u.verificado,
                        "criado_ms": u.criado_ms, "ip_cadastro": u.ip_cadastro,
                        "saldo": u.moeda.formatar(e.livro.saldo(Conta::ClienteDisponivel(u.id), u.moeda)),
                        "ganhos": u.moeda.formatar(e.livro.saldo(Conta::FornecedorDisponivel(u.id), u.moeda).saturating_add(e.livro.saldo(Conta::FornecedorPendente(u.id), u.moeda))),
                        "nos": e.nos.values().filter(|x| x.dono == u.id && !x.removido).count(),
                        "tarefas": e.tarefas.values().filter(|t| t.cliente == u.id).count(),
                    })
                })
                .collect();
            json!({ "usuarios": lista })
        })
    }

    /// Bloqueia, desbloqueia, confere o e-mail ou dá/tira o papel de admin.
    pub fn admin_usuario(&self, q: &Quem, id: Id, acao: &str, motivo: &str) -> R<Value> {
        q.admin()?;
        if id == q.usuario && acao != "verificar" {
            return Err(Erro::proibido("o administrador não muda a própria conta por aqui"));
        }
        let mut n = self.nucleo()?;
        let u = n.estado.usuarios.get(&id).cloned().ok_or_else(|| Erro::nao_achado("usuário não encontrado"))?;
        let t = self.agora();
        let motivo = texto_curto(motivo, 200);
        let mut ativas = Vec::new();
        let eventos = match acao {
            "bloquear" => {
                if motivo.len() < 3 {
                    return Err(Erro::pedido("descreva o motivo do bloqueio"));
                }
                ativas = n.estado.ativas.iter().filter(|x| n.estado.tarefas.get(x).is_some_and(|tt| tt.cliente == id)).copied().collect();
                let mut ev = vec![Evento::UserStatus { usuario: id, bloqueado: true, motivo: motivo.clone(), por: q.usuario, t }];
                for no in n.estado.nos.values().filter(|x| x.dono == id && !x.removido && x.adm == EstadoAdm::Normal) {
                    ev.push(Evento::NodeAdmin { no: no.id, estado: EstadoAdm::Maintenance, motivo: format!("conta bloqueada: {motivo}"), por: q.usuario, t });
                }
                ev
            }
            "desbloquear" => vec![Evento::UserStatus { usuario: id, bloqueado: false, motivo, por: q.usuario, t }],
            "verificar" => vec![Evento::UserVerified { usuario: id, por: q.usuario, t }],
            "admin" | "tirar-admin" => {
                let mut papeis = u.papeis;
                papeis.admin = acao == "admin";
                vec![Evento::RoleGranted { usuario: id, papeis, por: q.usuario, t }]
            }
            _ => return Err(Erro::pedido("ação: bloquear, desbloquear, verificar, admin ou tirar-admin")),
        };
        let nos_da_conta: Vec<Id> = if acao == "bloquear" { n.estado.nos.values().filter(|x| x.dono == id).map(|x| x.id).collect() } else { Vec::new() };
        self.registrar(&mut n, eventos)?;
        for no in nos_da_conta {
            self.cancelar_do_no(&mut n, no, "conta bloqueada")?;
        }
        for tarefa in ativas {
            self.finalizar(&mut n, tarefa, EstadoTarefa::Rejected, "conta bloqueada pelo administrador", q.usuario)?;
        }
        Ok(json!({ "ok": true }))
    }

    /// Admin: códigos de recuperação novos para um usuário que perdeu o
    /// acesso (entregues por um canal conferido). Derruba as sessões.
    pub fn admin_codigos(&self, q: &Quem, id: Id) -> R<Value> {
        q.admin()?;
        let (codigos, hashes) = crate::auth::codigos_de_recuperacao().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        n.estado.usuarios.get(&id).ok_or_else(|| Erro::nao_achado("usuário não encontrado"))?;
        let t = self.agora();
        let sessoes: Vec<Evento> =
            n.estado.sessoes.iter().filter(|(_, s)| s.usuario == id).map(|(h, _)| Evento::Logout { sessao: h.clone(), motivo: "recuperação pelo administrador".into(), t }).collect();
        let mut eventos = vec![Evento::RecoveryCodesIssued { usuario: id, recuperacao: hashes, t }];
        eventos.extend(sessoes);
        self.registrar(&mut n, eventos)?;
        Ok(json!({ "codigos": codigos, "aviso": "entregue ao dono da conta por um canal conferido; os códigos não aparecem de novo" }))
    }

    /// Todos os nós.
    pub fn admin_nos(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        let agora = self.agora();
        let n = self.nucleo()?;
        let v = self.vivo()?;
        let lista: Vec<Value> = n.estado.nos.values().rev().take(500).map(|x| json_do_no(&n.estado, x, v.pulsos.get(&x.id), agora, false)).collect();
        Ok(json!({ "nos": lista }))
    }

    /// Kill node, quarentena, manutenção, banimento ou liberação.
    pub fn admin_no(&self, q: &Quem, id: Id, estado: &str, motivo: &str) -> R<()> {
        q.admin()?;
        let novo = match estado {
            "QUARANTINED" | "quarentena" => EstadoAdm::Quarantined,
            "MAINTENANCE" | "manutencao" => EstadoAdm::Maintenance,
            "BANNED" | "banir" => EstadoAdm::Banned,
            "NORMAL" | "liberar" => EstadoAdm::Normal,
            _ => return Err(Erro::pedido("estado: QUARANTINED, MAINTENANCE, BANNED ou NORMAL")),
        };
        let mut n = self.nucleo()?;
        n.estado.nos.get(&id).ok_or_else(|| Erro::nao_achado("nó não encontrado"))?;
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::NodeAdmin { no: id, estado: novo, motivo: texto_curto(motivo, 200), por: q.usuario, t }])?;
        if novo != EstadoAdm::Normal {
            self.cancelar_do_no(&mut n, id, "parado pelo administrador")?;
        }
        Ok(())
    }

    /// Kill task (pelo administrador).
    pub fn admin_parar_tarefa(&self, q: &Quem, id: Id, motivo: &str) -> R<()> {
        q.admin()?;
        let mut n = self.nucleo()?;
        let t = n.estado.tarefas.get(&id).ok_or_else(|| Erro::nao_achado("tarefa não encontrada"))?;
        if t.estado.final_() {
            return Err(Erro::conflito("a tarefa já terminou"));
        }
        let m = texto_curto(motivo, 200);
        self.finalizar(&mut n, id, EstadoTarefa::Cancelled, &format!("parada pelo administrador: {m}"), q.usuario)
    }

    /// Todas as tarefas.
    pub fn admin_tarefas(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        self.ler(|e| {
            let lista: Vec<Value> = e
                .tarefas
                .values()
                .rev()
                .take(500)
                .map(|t| {
                    let mut j = super::tarefas::json_da_tarefa(t, false);
                    crate::util::por(&mut j, "cliente", json!(t.cliente));
                    crate::util::por(&mut j, "cliente_email", json!(e.usuarios.get(&t.cliente).map(|u| u.email.clone())));
                    j
                })
                .collect();
            json!({ "tarefas": lista })
        })
    }

    /// Todos os pagamentos.
    pub fn admin_pagamentos(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        self.ler(|e| {
            let lista: Vec<Value> = e
                .pagamentos
                .values()
                .rev()
                .take(500)
                .map(|p| {
                    json!({
                        "id": p.id, "codigo": p.codigo, "usuario": p.usuario, "email": e.usuarios.get(&p.usuario).map(|u| u.email.clone()),
                        "valor": p.moeda.formatar(p.valor), "centavos": p.valor, "moeda": p.moeda.codigo(), "estado": p.estado, "provedor": p.provedor,
                        "referencia": p.referencia, "taxa": p.moeda.formatar(p.taxa), "criado_ms": p.criado_ms, "atualizado_ms": p.atualizado_ms, "nota": p.nota,
                    })
                })
                .collect();
            json!({ "pagamentos": lista })
        })
    }

    /// Todos os saques.
    pub fn admin_saques(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        self.ler(|e| {
            let lista: Vec<Value> = e
                .saques
                .values()
                .rev()
                .take(500)
                .map(|s| {
                    json!({
                        "id": s.id, "usuario": s.usuario, "email": e.usuarios.get(&s.usuario).map(|u| u.email.clone()),
                        "valor": s.moeda.formatar(s.valor), "estado": s.estado, "destino": s.destino, "criado_ms": s.criado_ms,
                        "atualizado_ms": s.atualizado_ms, "nota": s.nota,
                    })
                })
                .collect();
            json!({ "saques": lista })
        })
    }

    /// A configuração.
    pub fn admin_config(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        self.ler(|e| serde_json::to_value(&e.config).unwrap_or(Value::Null))
    }

    /// Troca a configuração inteira (preços, comissão, limites, Pix,
    /// motores desligados, versão mínima). Confere tudo antes.
    pub fn admin_trocar_config(&self, q: &Quem, nova: Config, resumo: &str) -> R<()> {
        q.admin()?;
        nova.validar().map_err(Erro::pedido)?;
        let mut n = self.nucleo()?;
        let antiga = n.estado.config.clone();
        if antiga == nova {
            return Ok(());
        }
        let t = self.agora();
        let resumo = if resumo.trim().is_empty() { diferencas(&antiga, &nova) } else { texto_curto(resumo, 300) };
        // motores que acabaram de ser desligados: as tarefas deles param
        let desligados: Vec<String> = nova.motores_desligados.difference(&antiga.motores_desligados).cloned().collect();
        let parar: Vec<Id> = n.estado.ativas.iter().filter(|x| n.estado.tarefas.get(x).is_some_and(|tt| desligados.contains(&tt.pedido.tipo))).copied().collect();
        let emergencia = nova.parada_de_emergencia && !antiga.parada_de_emergencia;
        self.registrar(&mut n, vec![Evento::ConfigChanged { config: nova, resumo, por: q.usuario, t }])?;
        for tarefa in parar {
            self.finalizar(&mut n, tarefa, EstadoTarefa::Rejected, "motor desligado pela equipe", q.usuario)?;
        }
        if emergencia {
            // EMERGENCY COMPUTE STOP: tudo que roda é cancelado agora
            let nos: Vec<Id> = n.estado.nos.keys().copied().collect();
            for no in nos {
                if !abertas_do_no(&n.estado, no).is_empty() {
                    self.cancelar_do_no(&mut n, no, "parada de emergência")?;
                }
            }
        }
        Ok(())
    }

    /// Liga ou desliga a parada de emergência.
    pub fn admin_emergencia(&self, q: &Quem, ligar: bool) -> R<()> {
        let mut c = self.ler(|e| e.config.clone())?;
        c.parada_de_emergencia = ligar;
        self.admin_trocar_config(q, c, if ligar { "EMERGENCY COMPUTE STOP ligado" } else { "parada de emergência desligada" })
    }

    /// Liga ou desliga um motor (DISABLE WORKLOAD TYPE).
    pub fn admin_motor(&self, q: &Quem, tipo: &str, ligado: bool) -> R<()> {
        let mut c = self.ler(|e| e.config.clone())?;
        if ligado {
            c.motores_desligados.remove(tipo);
        } else {
            c.motores_desligados.insert(tipo.to_string());
        }
        self.admin_trocar_config(q, c, &format!("motor {tipo} {}", if ligado { "ligado" } else { "desligado" }))
    }

    /// Alertas.
    pub fn admin_alertas(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        self.ler(|e| {
            let lista: Vec<Value> = e
                .alertas
                .values()
                .rev()
                .take(500)
                .map(|a| {
                    json!({
                        "id": a.id, "tipo": a.tipo, "severidade": a.severidade, "alvo": a.alvo, "detalhe": a.detalhe, "criado_ms": a.criado_ms,
                        "resolvido": a.resolvido.as_ref().map(|(nota, por, t)| json!({ "nota": nota, "por": por, "t": t })),
                    })
                })
                .collect();
            json!({ "alertas": lista })
        })
    }

    /// Resolve um alerta.
    pub fn admin_resolver_alerta(&self, q: &Quem, id: Id, nota: &str) -> R<()> {
        q.admin()?;
        let mut n = self.nucleo()?;
        let a = n.estado.alertas.get(&id).ok_or_else(|| Erro::nao_achado("alerta não encontrado"))?;
        if a.resolvido.is_some() {
            return Ok(());
        }
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::AlertResolved { id, nota: texto_curto(nota, 300), por: q.usuario, t }])
    }

    /// O registro de auditoria: os eventos do diário, do mais novo, com filtro
    /// por tipo e por texto. Os segredos nunca estão aqui (só hashes).
    pub fn admin_registro(&self, q: &Quem, tipo: &str, busca: &str, antes_de: Option<u64>) -> R<Value> {
        q.admin()?;
        let caminho = self.pasta().join("diario.hyx");
        let leitura = crate::diario::ler(&caminho).map_err(|e| Erro::interno(&e))?;
        let busca = busca.trim().to_lowercase();
        let mut saida = Vec::new();
        for r in leitura.registros.iter().rev() {
            if antes_de.is_some_and(|a| r.n >= a) {
                continue;
            }
            if !tipo.is_empty() && r.ev.nome() != tipo {
                continue;
            }
            let mut j = serde_json::to_value(&r.ev).unwrap_or(Value::Null);
            // hashes de senha não são segredo, mas não ajudam ninguém a ler o registro
            if let Some(o) = j.as_object_mut() {
                for campo in ["senha", "recuperacao", "sessao", "hash", "codigo"] {
                    if o.contains_key(campo) && r.ev.nome() != "TASK_ASSIGNED" {
                        o.insert(campo.into(), json!("…"));
                    }
                }
            }
            let texto = j.to_string();
            if !busca.is_empty() && !texto.to_lowercase().contains(&busca) {
                continue;
            }
            saida.push(json!({ "n": r.n, "t": r.t, "tipo": r.ev.nome(), "evento": j }));
            if saida.len() >= 300 {
                break;
            }
        }
        Ok(json!({ "eventos": saida, "total": leitura.registros.len() }))
    }

    /// A cópia do diário, para o administrador guardar fora do servidor.
    pub fn admin_copia(&self, q: &Quem) -> R<Vec<u8>> {
        q.admin()?;
        let b = self.nucleo()?.diario.bytes().map_err(|e| Erro::interno(&e))?;
        self.alertar("auditoria:copia-baixada", "baixa", &format!("usuario:{}", q.usuario), "cópia do diário baixada pelo administrador")?;
        Ok(b)
    }

    /// O livro inteiro (as contas com saldo) para conferência.
    pub fn admin_livro(&self, q: &Quem) -> R<Value> {
        q.admin()?;
        self.ler(|e| {
            let contas: Vec<Value> = e.livro.contas().map(|(c, m, v)| json!({ "conta": c.to_string(), "moeda": m.codigo(), "centavos": v, "texto": m.formatar(v) })).collect();
            let ultimos: Vec<Value> = e
                .livro
                .lancamentos
                .iter()
                .rev()
                .take(200)
                .map(|l| json!({ "id": l.id, "t": l.t, "motivo": l.motivo, "referencia": l.referencia, "linhas": l.linhas.iter().map(|x| json!({ "conta": x.conta.to_string(), "valor": x.moeda.formatar(x.valor) })).collect::<Vec<_>>() }))
                .collect();
            let fecha = [Moeda::Brl, Moeda::Usd].iter().all(|m| e.livro.soma(*m) == 0);
            json!({ "contas": contas, "lancamentos": ultimos, "fecha": fecha })
        })
    }
}

/// O que mudou entre duas configurações, em texto curto.
fn diferencas(a: &Config, b: &Config) -> String {
    let mut d = Vec::new();
    if a.tabelas != b.tabelas {
        d.push("preços".to_string());
    }
    if a.fornecedor_bp != b.fornecedor_bp {
        d.push(format!("parte do fornecedor {} → {} bp", a.fornecedor_bp, b.fornecedor_bp));
    }
    if a.retencao_dias != b.retencao_dias {
        d.push(format!("retenção {} → {} dias", a.retencao_dias, b.retencao_dias));
    }
    if a.politica != b.politica {
        d.push("limites".into());
    }
    if a.motores_desligados != b.motores_desligados {
        d.push(format!("motores desligados: {:?}", b.motores_desligados));
    }
    if a.parada_de_emergencia != b.parada_de_emergencia {
        d.push(format!("parada de emergência: {}", b.parada_de_emergencia));
    }
    if a.pix != b.pix {
        d.push("dados do Pix".into());
    }
    if a.cadastro_aberto != b.cadastro_aberto {
        d.push(format!("cadastro aberto: {}", b.cadastro_aberto));
    }
    if a.contato != b.contato {
        d.push("contato do suporte".into());
    }
    if a.versao_minima_agente != b.versao_minima_agente {
        d.push(format!("versão mínima do agente: {}", b.versao_minima_agente));
    }
    if d.is_empty() { "configuração".into() } else { d.join("; ") }
}
