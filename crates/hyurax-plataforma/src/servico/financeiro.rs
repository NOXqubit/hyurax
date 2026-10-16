//! Dinheiro (seções 20 a 23, 26 e 27 do documento da V1): saldo, pagamentos,
//! extrato, faturas, retenção e liberação dos ganhos, saques e estornos.
//!
//! **Formas de pagamento desta versão:**
//!
//! - `pix-manual`: o cliente paga por Pix na chave da empresa (configurada
//!   pelo administrador) com o código do pedido na mensagem; a equipe confere
//!   no extrato do banco e confirma no painel. É a forma da primeira venda,
//!   antes de existir um provedor integrado;
//! - `teste`: só com o servidor em modo de teste. Confirma na hora, e todas
//!   as telas mostram que é dinheiro de teste.
//!
//! Um provedor integrado (Asaas, Mercado Pago...) entra como mais uma forma,
//! com o aviso do provedor conferido antes de creditar. Isso depende da conta
//! PJ e do contrato com o provedor (docs/MONETIZACAO.md, Fase M1).
//!
//! **Saques:** o ganho fica retido pelos dias da configuração (para cobrir
//! estorno e fraude), libera para o disponível, e o saque é pedido no painel e
//! pago pela empresa por Pix, à mão, depois de conferido.

use serde_json::{Value, json};

use super::contas::Quem;
use super::{Erro, Plataforma, R};
use crate::dinheiro::{Conta, Linha, Moeda, Motivo};
use crate::estado::Estado;
use crate::eventos::{EstadoTarefa, Evento};
use crate::modelo::{Escopo, Id, texto_curto};
use crate::util::mes;

/// Pagamento de teste: aparece em toda tela.
pub const PROVEDOR_TESTE: &str = "teste";
/// Pix conferido à mão pela equipe.
pub const PROVEDOR_PIX: &str = "pix-manual";

fn saldo_json(e: &Estado, usuario: Id, moeda: Moeda) -> Value {
    let l = &e.livro;
    let v = |c: Conta| {
        let x = l.saldo(c, moeda);
        json!({ "centavos": x, "texto": moeda.formatar(x) })
    };
    json!({
        "moeda": moeda.codigo(),
        "disponivel": v(Conta::ClienteDisponivel(usuario)),
        "reservado": v(Conta::ClienteReservado(usuario)),
        "ganhos_pendentes": v(Conta::FornecedorPendente(usuario)),
        "ganhos_disponiveis": v(Conta::FornecedorDisponivel(usuario)),
        "saques_em_andamento": v(Conta::FornecedorSacando(usuario)),
    })
}

/// A chave Pix que o fornecedor informou parece válida? CPF ou CNPJ (só
/// dígitos), e-mail, celular com +55 ou chave aleatória.
pub fn chave_pix_valida(texto: &str) -> bool {
    let t = texto.trim();
    let digitos: String = t.chars().filter(char::is_ascii_digit).collect();
    let so_numero = t.chars().all(|c| c.is_ascii_digit() || ".-/ ".contains(c));
    let aleatoria = t.len() == 36 && t.chars().all(|c| c.is_ascii_hexdigit() || c == '-') && t.chars().filter(|&c| c == '-').count() == 4;
    let celular = t.starts_with('+') && (12..=14).contains(&digitos.len());
    (so_numero && (digitos.len() == 11 || digitos.len() == 14)) || crate::auth::validar_email(t).is_some() || celular || aleatoria
}

impl Plataforma {
    /// O saldo da conta.
    pub fn saldo(&self, q: &Quem) -> R<Value> {
        q.pode(Escopo::SaldoLer)?;
        let teste = self.opcoes.modo_teste;
        self.ler(|e| {
            let mut j = saldo_json(e, q.usuario, q.moeda);
            crate::util::por(&mut j, "ambiente_teste", json!(teste));
            j
        })
    }

    /// Pede para adicionar saldo. Devolve as instruções de pagamento.
    pub fn criar_pagamento(&self, q: &Quem, valor: i64) -> R<Value> {
        q.cliente()?;
        if q.escopos.is_some() {
            return Err(Erro::proibido("pagamento só pelo painel"));
        }
        let agora = self.agora();
        let mut n = self.nucleo()?;
        let c = &n.estado.config;
        let minimo = c.pagamento_minimo.get(&q.moeda).copied().unwrap_or(1_000);
        let maximo = c.pagamento_maximo.get(&q.moeda).copied().unwrap_or(500_000);
        if !(minimo..=maximo).contains(&valor) {
            return Err(Erro::pedido(format!("valor de {} a {}", q.moeda.formatar(minimo), q.moeda.formatar(maximo))));
        }
        let pendentes = n.estado.pagamentos.values().filter(|p| p.usuario == q.usuario && p.estado == "pendente").count();
        if pendentes >= usize::try_from(c.politica.pagamentos_pendentes_max).unwrap_or(usize::MAX) {
            return Err(Erro::limite("há pagamentos pendentes demais; conclua ou cancele um antes"));
        }
        let provedor = if self.opcoes.modo_teste {
            PROVEDOR_TESTE
        } else if !c.pix.chave.is_empty() && q.moeda == Moeda::Brl {
            PROVEDOR_PIX
        } else {
            return Err(Erro::indisponivel("o pagamento ainda não está aberto: a empresa está concluindo a conta de recebimento"));
        };
        let pix = c.pix.clone();
        let id = n.estado.seq.pagamento;
        let codigo = format!("HYX-P-{id:06}");
        let mut eventos = vec![Evento::PaymentCreated { id, usuario: q.usuario, valor, moeda: q.moeda, provedor: provedor.into(), codigo: codigo.clone(), t: agora }];
        if provedor == PROVEDOR_TESTE {
            eventos.push(Evento::PaymentConfirmed { id, referencia: "ambiente de teste".into(), taxa: 0, por: 0, t: agora });
            eventos.push(Plataforma::lancamento(
                &n.estado,
                0,
                agora,
                Motivo::Pagamento,
                format!("pagamento:{id}"),
                vec![Linha { conta: Conta::Caixa, moeda: q.moeda, valor: valor.saturating_neg() }, Linha { conta: Conta::ClienteDisponivel(q.usuario), moeda: q.moeda, valor }],
            ));
        }
        self.registrar(&mut n, eventos)?;
        let instrucoes = if provedor == PROVEDOR_TESTE {
            json!({ "tipo": "teste", "texto": "Ambiente de teste: o saldo entrou na hora. Não é dinheiro de verdade." })
        } else {
            json!({
                "tipo": "pix",
                "chave": pix.chave,
                "titular": pix.titular,
                "valor": q.moeda.formatar(valor),
                "mensagem": codigo,
                "texto": format!("Pague {} por Pix para a chave acima e escreva {codigo} na mensagem. O saldo entra quando a equipe confirmar o recebimento (em dias úteis, normalmente no mesmo dia).", q.moeda.formatar(valor)),
            })
        };
        Ok(json!({ "id": id, "codigo": codigo, "estado": if provedor == PROVEDOR_TESTE { "confirmado" } else { "pendente" }, "instrucoes": instrucoes }))
    }

    /// Os pagamentos do cliente.
    pub fn meus_pagamentos(&self, q: &Quem) -> R<Value> {
        q.pode(Escopo::SaldoLer)?;
        self.ler(|e| {
            let mut v: Vec<Value> = e
                .pagamentos
                .values()
                .filter(|p| p.usuario == q.usuario)
                .map(|p| json!({ "id": p.id, "codigo": p.codigo, "valor": p.moeda.formatar(p.valor), "centavos": p.valor, "estado": p.estado, "provedor": p.provedor, "criado_ms": p.criado_ms, "atualizado_ms": p.atualizado_ms, "nota": p.nota }))
                .collect();
            v.reverse();
            json!({ "pagamentos": v })
        })
    }

    /// O cliente desiste de um pagamento pendente.
    pub fn cancelar_pagamento(&self, q: &Quem, id: Id) -> R<()> {
        let mut n = self.nucleo()?;
        let p = n.estado.pagamentos.get(&id).filter(|p| p.usuario == q.usuario).ok_or_else(|| Erro::nao_achado("pagamento não encontrado"))?;
        if p.estado != "pendente" {
            return Err(Erro::conflito("só pagamento pendente pode ser cancelado"));
        }
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::PaymentCancelled { id, motivo: "cancelado pelo cliente".into(), por: q.usuario, t }])
    }

    /// O extrato: os lançamentos que tocam as contas do usuário, do mais novo.
    pub fn extrato(&self, q: &Quem, mes_filtro: Option<&str>) -> R<Value> {
        q.pode(Escopo::SaldoLer)?;
        self.ler(|e| {
            let minhas = |c: &Conta| {
                matches!(c,
                    Conta::ClienteDisponivel(u) | Conta::ClienteReservado(u) | Conta::FornecedorPendente(u)
                    | Conta::FornecedorDisponivel(u) | Conta::FornecedorSacando(u) if *u == q.usuario)
            };
            let mut linhas = Vec::new();
            for l in e.livro.lancamentos.iter().rev() {
                if mes_filtro.is_some_and(|m| mes(l.t) != m) {
                    continue;
                }
                for x in l.linhas.iter().filter(|x| minhas(&x.conta)) {
                    linhas.push(json!({
                        "lancamento": l.id, "t": l.t, "motivo": l.motivo, "referencia": l.referencia,
                        "conta": x.conta.to_string(), "valor": x.moeda.formatar(x.valor), "centavos": x.valor, "moeda": x.moeda.codigo(),
                    }));
                }
                if linhas.len() >= 2000 {
                    break;
                }
            }
            json!({ "linhas": linhas })
        })
    }

    /// A fatura de um mês: o que entrou, o que foi gasto em cada tarefa e o
    /// que voltou.
    pub fn fatura(&self, q: &Quem, mes_alvo: &str) -> R<Value> {
        q.pode(Escopo::SaldoLer)?;
        if mes_alvo.len() != 7 || mes_alvo.as_bytes().get(4) != Some(&b'-') {
            return Err(Erro::pedido("mês no formato AAAA-MM"));
        }
        self.ler(|e| {
            let moeda = q.moeda;
            let mut tarefas = Vec::new();
            let mut gasto: i64 = 0;
            for t in e.tarefas.values().filter(|t| t.cliente == q.usuario) {
                let Some(l) = &t.liquidada else { continue };
                if mes(l.t) != mes_alvo || t.moeda != moeda {
                    continue;
                }
                gasto = gasto.saturating_add(l.cobranca);
                tarefas.push(json!({
                    "tarefa": t.id, "tipo": t.pedido.tipo, "estado": t.estado.nome(), "unidades": t.concluidas,
                    "cobrado": moeda.formatar(l.cobranca), "centavos": l.cobranca, "quando": l.t,
                }));
            }
            let mut pago: i64 = 0;
            let mut pagamentos = Vec::new();
            for p in e.pagamentos.values().filter(|p| p.usuario == q.usuario && p.estado == "confirmado" && mes(p.atualizado_ms) == mes_alvo && p.moeda == moeda) {
                pago = pago.saturating_add(p.valor);
                pagamentos.push(json!({ "id": p.id, "codigo": p.codigo, "valor": moeda.formatar(p.valor), "quando": p.atualizado_ms, "provedor": p.provedor }));
            }
            json!({
                "mes": mes_alvo, "moeda": moeda.codigo(),
                "total_gasto": moeda.formatar(gasto), "total_gasto_centavos": gasto,
                "total_pago": moeda.formatar(pago), "total_pago_centavos": pago,
                "tarefas": tarefas, "pagamentos": pagamentos,
                "nota": "Fatura de consumo da plataforma. A nota fiscal é emitida pela empresa quando a venda estiver aberta.",
            })
        })
    }

    /// Os ganhos do fornecedor: retidos (com a data de liberação), liberados
    /// e os saques.
    pub fn ganhos(&self, q: &Quem) -> R<Value> {
        q.fornecedor()?;
        self.ler(|e| {
            let moeda = q.moeda;
            let mut retidos: Vec<Value> = e
                .retidos
                .iter()
                .filter(|r| r.usuario == q.usuario)
                .map(|r| json!({ "tarefa": r.tarefa, "valor": r.moeda.formatar(r.valor), "centavos": r.valor, "moeda": r.moeda.codigo(), "libera_ms": r.libera_ms, "liberado": r.liberado, "segurado": r.segurado }))
                .collect();
            retidos.reverse();
            let mut saques: Vec<Value> = e
                .saques
                .values()
                .filter(|s| s.usuario == q.usuario)
                .map(|s| json!({ "id": s.id, "valor": s.moeda.formatar(s.valor), "estado": s.estado, "destino": mascarar(&s.destino), "criado_ms": s.criado_ms, "atualizado_ms": s.atualizado_ms, "nota": s.nota }))
                .collect();
            saques.reverse();
            let minimo = e.config.saque_minimo.get(&moeda).copied().unwrap_or(2_000);
            json!({
                "saldo": saldo_json(e, q.usuario, moeda),
                "retencao_dias": e.config.retencao_dias,
                "saque_minimo": moeda.formatar(minimo),
                "parte_do_fornecedor_pct": f64::from(e.config.fornecedor_bp) / 100.0,
                "retidos": retidos,
                "saques": saques,
            })
        })
    }

    /// O fornecedor pede um saque do disponível.
    pub fn pedir_saque(&self, q: &Quem, valor: i64, destino: &str) -> R<Value> {
        q.fornecedor()?;
        let destino = texto_curto(destino, 80);
        if !chave_pix_valida(&destino) {
            return Err(Erro::pedido("chave Pix inválida (CPF, CNPJ, e-mail, celular com +55 ou chave aleatória)"));
        }
        let mut n = self.nucleo()?;
        let moeda = q.moeda;
        let minimo = n.estado.config.saque_minimo.get(&moeda).copied().unwrap_or(2_000);
        if valor < minimo {
            return Err(Erro::pedido(format!("o saque mínimo é {}", moeda.formatar(minimo))));
        }
        let disponivel = n.estado.livro.saldo(Conta::FornecedorDisponivel(q.usuario), moeda);
        if valor > disponivel {
            return Err(Erro::pedido(format!("disponível para saque: {}", moeda.formatar(disponivel))));
        }
        if n.estado.saques.values().any(|s| s.usuario == q.usuario && (s.estado == "solicitado" || s.estado == "aprovado")) {
            return Err(Erro::conflito("já há um saque em andamento"));
        }
        let id = n.estado.seq.saque;
        let t = self.agora();
        let eventos = vec![
            Evento::PayoutRequested { id, usuario: q.usuario, valor, moeda, destino, t },
            Plataforma::lancamento(
                &n.estado,
                0,
                t,
                Motivo::SaquePedido,
                format!("saque:{id}"),
                vec![Linha { conta: Conta::FornecedorDisponivel(q.usuario), moeda, valor: valor.saturating_neg() }, Linha { conta: Conta::FornecedorSacando(q.usuario), moeda, valor }],
            ),
        ];
        self.registrar(&mut n, eventos)?;
        Ok(json!({ "id": id, "estado": "solicitado", "valor": moeda.formatar(valor) }))
    }

    /// Libera os ganhos que passaram da retenção (manutenção).
    pub(crate) fn liberar_ganhos(&self) -> R<()> {
        let agora = self.agora();
        let mut n = self.nucleo()?;
        let mut eventos = Vec::new();
        let mut desloc = 0u64;
        for r in n.estado.retidos.iter().filter(|r| !r.liberado && r.segurado.is_none() && r.libera_ms <= agora).take(200) {
            // conta bloqueada não libera: a análise de fraude decide antes
            if n.estado.usuarios.get(&r.usuario).is_none_or(|u| u.bloqueado) {
                continue;
            }
            eventos.push(Evento::EarningsReleased { tarefa: r.tarefa, usuario: r.usuario, valor: r.valor, moeda: r.moeda, t: agora });
            eventos.push(Plataforma::lancamento(
                &n.estado,
                desloc,
                agora,
                Motivo::Liberacao,
                format!("tarefa:{}", r.tarefa),
                vec![
                    Linha { conta: Conta::FornecedorPendente(r.usuario), moeda: r.moeda, valor: r.valor.saturating_neg() },
                    Linha { conta: Conta::FornecedorDisponivel(r.usuario), moeda: r.moeda, valor: r.valor },
                ],
            ));
            desloc = desloc.saturating_add(1);
        }
        self.registrar(&mut n, eventos)
    }

    /// Admin: confirma um pagamento pendente (o Pix caiu na conta).
    pub fn admin_confirmar_pagamento(&self, q: &Quem, id: Id, referencia: &str, taxa: i64) -> R<()> {
        q.admin()?;
        let mut n = self.nucleo()?;
        let p = n.estado.pagamentos.get(&id).cloned().ok_or_else(|| Erro::nao_achado("pagamento não encontrado"))?;
        if p.estado != "pendente" {
            return Err(Erro::conflito(format!("o pagamento está {}", p.estado)));
        }
        if !(0..=p.valor).contains(&taxa) {
            return Err(Erro::pedido("taxa entre zero e o valor do pagamento"));
        }
        let referencia = texto_curto(referencia, 120);
        if referencia.is_empty() {
            return Err(Erro::pedido("informe a referência do banco (o identificador do Pix recebido)"));
        }
        let t = self.agora();
        let eventos = vec![
            Evento::PaymentConfirmed { id, referencia, taxa, por: q.usuario, t },
            Plataforma::lancamento(
                &n.estado,
                0,
                t,
                Motivo::Pagamento,
                format!("pagamento:{id}"),
                vec![
                    Linha { conta: Conta::Caixa, moeda: p.moeda, valor: taxa.saturating_sub(p.valor) },
                    Linha { conta: Conta::ClienteDisponivel(p.usuario), moeda: p.moeda, valor: p.valor },
                    Linha { conta: Conta::PlataformaReceita, moeda: p.moeda, valor: taxa.saturating_neg() },
                ],
            ),
        ];
        self.registrar(&mut n, eventos)
    }

    /// Admin: cancela um pagamento que nunca chegou.
    pub fn admin_cancelar_pagamento(&self, q: &Quem, id: Id, motivo: &str) -> R<()> {
        q.admin()?;
        let mut n = self.nucleo()?;
        let p = n.estado.pagamentos.get(&id).ok_or_else(|| Erro::nao_achado("pagamento não encontrado"))?;
        if p.estado != "pendente" {
            return Err(Erro::conflito(format!("o pagamento está {}", p.estado)));
        }
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::PaymentCancelled { id, motivo: texto_curto(motivo, 200), por: q.usuario, t }])
    }

    /// Admin: estorno (chargeback). Tira o valor do saldo do cliente (que
    /// pode ficar negativo); conta negativa é bloqueada e os ganhos ainda
    /// retidos de tarefas desse cliente ficam segurados para análise.
    pub fn admin_estornar_pagamento(&self, q: &Quem, id: Id, motivo: &str) -> R<()> {
        q.admin()?;
        let mut n = self.nucleo()?;
        let p = n.estado.pagamentos.get(&id).cloned().ok_or_else(|| Erro::nao_achado("pagamento não encontrado"))?;
        if p.estado != "confirmado" {
            return Err(Erro::conflito("só pagamento confirmado pode ser estornado"));
        }
        let t = self.agora();
        let motivo = texto_curto(motivo, 200);
        let saldo_depois = n.estado.livro.saldo(Conta::ClienteDisponivel(p.usuario), p.moeda).saturating_sub(p.valor);
        let mut eventos = vec![
            Evento::PaymentRefunded { id, motivo: motivo.clone(), por: q.usuario, t },
            Plataforma::lancamento(
                &n.estado,
                0,
                t,
                Motivo::Estorno,
                format!("pagamento:{id}"),
                vec![Linha { conta: Conta::ClienteDisponivel(p.usuario), moeda: p.moeda, valor: p.valor.saturating_neg() }, Linha { conta: Conta::Caixa, moeda: p.moeda, valor: p.valor }],
            ),
            Evento::SecurityAlert {
                id: n.estado.seq.alerta,
                tipo: "fraude:estorno".into(),
                severidade: "alta".into(),
                alvo: format!("usuario:{}", p.usuario),
                detalhe: format!("estorno do pagamento {} ({}): {motivo}", p.codigo, p.moeda.formatar(p.valor)),
                t,
            },
        ];
        if saldo_depois < 0 {
            eventos.push(Evento::UserStatus { usuario: p.usuario, bloqueado: true, motivo: "saldo negativo depois de estorno".into(), por: q.usuario, t });
        }
        // segura os ganhos retidos que vieram de tarefas deste cliente
        let tarefas_do_cliente: Vec<Id> = n.estado.tarefas.values().filter(|x| x.cliente == p.usuario).map(|x| x.id).collect();
        for r in n.estado.retidos.iter().filter(|r| !r.liberado && r.segurado.is_none() && tarefas_do_cliente.contains(&r.tarefa)) {
            eventos.push(Evento::EarningsHeld { tarefa: r.tarefa, usuario: r.usuario, motivo: format!("estorno do pagamento {}", p.codigo), t });
        }
        let ativas: Vec<Id> = if saldo_depois < 0 { n.estado.ativas.iter().filter(|x| tarefas_do_cliente.contains(x)).copied().collect() } else { Vec::new() };
        self.registrar(&mut n, eventos)?;
        for tarefa in ativas {
            self.finalizar(&mut n, tarefa, EstadoTarefa::Rejected, "conta bloqueada por estorno", q.usuario)?;
        }
        Ok(())
    }

    /// Admin: libera ganhos segurados de uma tarefa (análise concluída).
    pub fn admin_soltar_ganhos(&self, q: &Quem, tarefa: Id) -> R<u32> {
        q.admin()?;
        let mut n = self.nucleo()?;
        let t = self.agora();
        let mut eventos = Vec::new();
        let mut desloc = 0u64;
        for r in n.estado.retidos.iter().filter(|r| r.tarefa == tarefa && !r.liberado && r.segurado.is_some()) {
            eventos.push(Evento::EarningsReleased { tarefa, usuario: r.usuario, valor: r.valor, moeda: r.moeda, t });
            eventos.push(Plataforma::lancamento(
                &n.estado,
                desloc,
                t,
                Motivo::Liberacao,
                format!("tarefa:{tarefa}"),
                vec![
                    Linha { conta: Conta::FornecedorPendente(r.usuario), moeda: r.moeda, valor: r.valor.saturating_neg() },
                    Linha { conta: Conta::FornecedorDisponivel(r.usuario), moeda: r.moeda, valor: r.valor },
                ],
            ));
            desloc = desloc.saturating_add(1);
        }
        let quantos = u32::try_from(eventos.len() / 2).unwrap_or(u32::MAX);
        self.registrar(&mut n, eventos)?;
        Ok(quantos)
    }

    /// Admin: aprova, marca como pago ou recusa um saque.
    pub fn admin_saque(&self, q: &Quem, id: Id, acao: &str, nota: &str) -> R<()> {
        q.admin()?;
        let mut n = self.nucleo()?;
        let s = n.estado.saques.get(&id).cloned().ok_or_else(|| Erro::nao_achado("saque não encontrado"))?;
        let t = self.agora();
        let nota = texto_curto(nota, 200);
        let eventos = match (s.estado.as_str(), acao) {
            ("solicitado", "aprovar") => vec![Evento::PayoutUpdated { id, estado: "aprovado".into(), nota, por: q.usuario, t }],
            ("solicitado" | "aprovado", "pagar") => {
                if nota.is_empty() {
                    return Err(Erro::pedido("informe a referência do Pix enviado"));
                }
                vec![
                    Evento::PayoutUpdated { id, estado: "pago".into(), nota, por: q.usuario, t },
                    Plataforma::lancamento(
                        &n.estado,
                        0,
                        t,
                        Motivo::SaquePago,
                        format!("saque:{id}"),
                        vec![Linha { conta: Conta::FornecedorSacando(s.usuario), moeda: s.moeda, valor: s.valor.saturating_neg() }, Linha { conta: Conta::Caixa, moeda: s.moeda, valor: s.valor }],
                    ),
                ]
            }
            ("solicitado" | "aprovado", "recusar") => vec![
                Evento::PayoutUpdated { id, estado: "recusado".into(), nota, por: q.usuario, t },
                Plataforma::lancamento(
                    &n.estado,
                    0,
                    t,
                    Motivo::SaqueRecusado,
                    format!("saque:{id}"),
                    vec![Linha { conta: Conta::FornecedorSacando(s.usuario), moeda: s.moeda, valor: s.valor.saturating_neg() }, Linha { conta: Conta::FornecedorDisponivel(s.usuario), moeda: s.moeda, valor: s.valor }],
                ),
            ],
            (estado, _) => return Err(Erro::conflito(format!("o saque está {estado}; ação {acao} não se aplica"))),
        };
        self.registrar(&mut n, eventos)
    }

    /// Admin: ajuste manual de saldo (crédito de cortesia, correção). Sai da
    /// receita da plataforma, e o motivo fica no diário.
    pub fn admin_ajuste(&self, q: &Quem, usuario: Id, valor: i64, motivo: &str) -> R<()> {
        q.admin()?;
        let motivo = texto_curto(motivo, 200);
        if motivo.len() < 5 {
            return Err(Erro::pedido("descreva o motivo do ajuste"));
        }
        if valor == 0 {
            return Err(Erro::pedido("valor zero"));
        }
        let mut n = self.nucleo()?;
        let moeda = n.estado.usuarios.get(&usuario).map(|u| u.moeda).ok_or_else(|| Erro::nao_achado("usuário não encontrado"))?;
        let t = self.agora();
        let eventos = vec![Plataforma::lancamento(
            &n.estado,
            0,
            t,
            Motivo::Ajuste,
            format!("ajuste:{usuario}:{motivo}"),
            vec![Linha { conta: Conta::ClienteDisponivel(usuario), moeda, valor }, Linha { conta: Conta::PlataformaReceita, moeda, valor: valor.saturating_neg() }],
        )];
        self.registrar(&mut n, eventos)
    }
}

/// Mostra só o começo e o fim de uma chave Pix.
pub fn mascarar(texto: &str) -> String {
    let c: Vec<char> = texto.chars().collect();
    if c.len() <= 6 {
        return "•••".into();
    }
    let ini: String = c.iter().take(3).collect();
    let fim: String = c.iter().skip(c.len().saturating_sub(3)).collect();
    format!("{ini}•••{fim}")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn chaves_pix() {
        assert!(chave_pix_valida("123.456.789-09"));
        assert!(chave_pix_valida("12.345.678/0001-95"));
        assert!(chave_pix_valida("fulano@exemplo.com"));
        assert!(chave_pix_valida("+5511987654321"));
        assert!(chave_pix_valida("123e4567-e89b-12d3-a456-426614174000"));
        assert!(!chave_pix_valida("123"));
        assert!(!chave_pix_valida("qualquer coisa"));
        assert_eq!(mascarar("fulano@exemplo.com"), "ful•••com");
    }
}
