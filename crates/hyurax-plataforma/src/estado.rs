//! O estado da plataforma, que é só o resultado de aplicar o diário em ordem.
//!
//! [`Estado::aplicar`] é a única função que muda o estado, e ela não olha o
//! relógio nem sorteia nada: tudo o que ela precisa vem no evento. Por isso
//! reler o diário dá exatamente o mesmo estado, byte a byte, e o teste de
//! recuperação confere isso comparando o resumo antes e depois de reabrir.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use hyurax_crypto::HASH_LEN;
use hyurax_ultrax::job::EspecificacaoDeJob;
use hyurax_ultrax::trabalho::{self, Especificacao};

use crate::dinheiro::{Livro, Moeda};
use crate::eventos::{Desfecho, EstadoAdm, EstadoTarefa, Evento, Papel};
use crate::modelo::{Config, Escopo, Hardware, Id, Limites, Papeis, PedidoDeTarefa};
use crate::regras::{self, Pagador, Tabela};
use crate::util::{de_hex, de_hex_fixo};

/// Um usuário.
#[derive(Clone, Debug)]
pub struct Usuario {
    /// Número.
    pub id: Id,
    /// E-mail.
    pub email: String,
    /// Nome.
    pub nome: String,
    /// Papéis.
    pub papeis: Papeis,
    /// Moeda.
    pub moeda: Moeda,
    /// País.
    pub pais: String,
    /// Hash da senha.
    pub senha: String,
    /// Hashes dos códigos de recuperação (vazio: usado).
    pub recuperacao: Vec<String>,
    /// Termos aceitos.
    pub termos: u32,
    /// Bloqueado?
    pub bloqueado: bool,
    /// Motivo do bloqueio.
    pub motivo_bloqueio: String,
    /// E-mail conferido.
    pub verificado: bool,
    /// Criado em.
    pub criado_ms: u64,
    /// IP do cadastro.
    pub ip_cadastro: String,
}

/// Uma sessão aberta.
#[derive(Clone, Debug)]
pub struct Sessao {
    /// Usuário.
    pub usuario: Id,
    /// Aberta em.
    pub criada_ms: u64,
    /// Vale até.
    pub expira_ms: u64,
    /// IP.
    pub ip: String,
    /// Navegador.
    pub agente: String,
}

/// Uma chave de API.
#[derive(Clone, Debug)]
pub struct ChaveApi {
    /// Número.
    pub id: Id,
    /// Dono.
    pub usuario: Id,
    /// Nome.
    pub nome: String,
    /// Começo da chave.
    pub prefixo: String,
    /// Hash.
    pub hash: String,
    /// Escopos.
    pub escopos: Vec<Escopo>,
    /// Pedidos por minuto.
    pub limite_por_min: u32,
    /// Criada em.
    pub criada_ms: u64,
    /// Revogada em.
    pub revogada_ms: Option<u64>,
}

/// Um código de vínculo.
#[derive(Clone, Debug)]
pub struct Vinculo {
    /// Fornecedor.
    pub usuario: Id,
    /// Vale até.
    pub expira_ms: u64,
    /// Já usado?
    pub usado: bool,
}

/// O histórico que dá a reputação.
#[derive(Clone, Copy, Debug, Default)]
pub struct Historico {
    /// Entregas feitas.
    pub entregas: u64,
    /// Entregas aceitas.
    pub aceitas: u64,
    /// Entregas aceitas que foram de fato conferidas (Freivalds ou concordância).
    pub verificadas: u64,
    /// Prazo vencido ou abandono.
    pub falhas: u64,
    /// Resultado errado confirmado.
    pub incorretas: u64,
}

impl Historico {
    /// A pontuação, de 0 a 100.
    pub fn pontuacao(&self) -> u8 {
        regras::pontuacao(self.verificadas, self.falhas, self.incorretas)
    }
}

/// Um computador registrado.
#[derive(Clone, Debug)]
pub struct No {
    /// Número.
    pub id: Id,
    /// Fornecedor.
    pub dono: Id,
    /// Chave do worker.
    pub chave: [u8; 32],
    /// Nome.
    pub nome: String,
    /// Região.
    pub regiao: String,
    /// Hardware.
    pub hardware: Hardware,
    /// Versão do agente.
    pub versao: String,
    /// Limites do fornecedor.
    pub limites: Limites,
    /// Pausado pelo fornecedor.
    pub pausado: bool,
    /// Estado administrativo.
    pub adm: EstadoAdm,
    /// Motivo do estado administrativo.
    pub motivo_adm: String,
    /// Removido pelo fornecedor.
    pub removido: bool,
    /// Registrado em.
    pub criado_ms: u64,
    /// Histórico.
    pub historico: Historico,
    /// Ganhos, por moeda, em centavos.
    pub ganhos: BTreeMap<Moeda, i64>,
    /// Online pelo último evento de presença.
    pub online: bool,
    /// Desde quando está no estado de presença atual.
    pub presenca_desde_ms: u64,
    /// Tempo online somado, em ms (para a disponibilidade).
    pub online_total_ms: u64,
}

/// Uma entrega de resultado.
#[derive(Clone, Debug)]
pub struct Entrega {
    /// Hash do resultado.
    pub hash: [u8; HASH_LEN],
    /// Operações.
    pub operacoes: u64,
    /// Bytes.
    pub bytes: u64,
    /// Tempo de cálculo relatado.
    pub ms_calculo: u64,
    /// Na GPU?
    pub gpu: bool,
    /// Hash do registro de prova.
    pub prova: String,
    /// Quando.
    pub t: u64,
}

/// Uma unidade entregue a um nó.
#[derive(Clone, Debug)]
pub struct Atribuicao {
    /// Número.
    pub id: Id,
    /// Tarefa.
    pub tarefa: Id,
    /// Unidade.
    pub unidade: u64,
    /// Nó.
    pub no: Id,
    /// Dono do nó.
    pub dono: Id,
    /// Papel.
    pub papel: Papel,
    /// Emitida em.
    pub emitida_ms: u64,
    /// Prazo.
    pub prazo_ms: u64,
    /// O agente confirmou que recebeu.
    pub recebida: bool,
    /// O agente começou a executar.
    pub executando: bool,
    /// A entrega.
    pub entrega: Option<Entrega>,
    /// Como terminou.
    pub desfecho: Option<(Desfecho, String, u64)>,
}

impl Atribuicao {
    /// Ainda esperando entrega?
    pub fn aberta(&self) -> bool {
        self.desfecho.is_none() && self.entrega.is_none()
    }
}

/// Uma unidade de tarefa, depois de tocada pela primeira vez.
#[derive(Clone, Debug, Default)]
pub struct Unidade {
    /// Cópias concordantes necessárias (as do cliente mais as conferências).
    pub necessarias: u8,
    /// Conferências pedidas pela plataforma.
    pub conferencias: u8,
    /// Tentativas que falharam.
    pub tentativas: u8,
    /// Entregas ainda não julgadas, pela ordem de chegada: atribuição, hash
    /// do resultado e operações relatadas. Cópias só concordam quando as
    /// duas coisas batem (ninguém infla a conta das rotas sozinho).
    pub entregas: Vec<(Id, [u8; HASH_LEN], u64)>,
    /// Atribuições sem entrega ainda.
    pub em_voo: Vec<Id>,
    /// Todas as atribuições desta unidade.
    pub todas: Vec<Id>,
    /// Pronta para receber nó desde.
    pub pronta_ms: u64,
    /// Hash aceito.
    pub concluida: Option<[u8; HASH_LEN]>,
    /// Motivo da falha.
    pub falhou: Option<String>,
}

impl Unidade {
    /// Fechada (concluída ou falha)?
    pub fn fechada(&self) -> bool {
        self.concluida.is_some() || self.falhou.is_some()
    }

    /// O maior grupo de entregas iguais (hash e operações): (tamanho, chave).
    pub fn maior_grupo(&self) -> (usize, Option<([u8; HASH_LEN], u64)>) {
        let mut melhor = (0usize, None);
        for (_, h, o) in &self.entregas {
            let n = self.entregas.iter().filter(|(_, x, y)| x == h && y == o).count();
            if n > melhor.0 {
                melhor = (n, Some((*h, *o)));
            }
        }
        melhor
    }

    /// Precisa de mais uma atribuição agora? Conta o maior grupo de entregas
    /// iguais mais as que estão em voo: duas entregas que discordam não
    /// somam, e é preciso um terceiro nó para desempatar.
    pub fn precisa_de_no(&self) -> bool {
        !self.fechada() && self.maior_grupo().0.saturating_add(self.em_voo.len()) < usize::from(self.necessarias)
    }
}

/// O que a verificação fez na tarefa.
#[derive(Clone, Copy, Debug, Default)]
pub struct Verificacao {
    /// Unidades conferidas por Freivalds no servidor.
    pub freivalds: u64,
    /// Unidades conferidas por concordância entre nós.
    pub concordancia: u64,
    /// Unidades aceitas só pela integridade (assinatura, entrada, hash).
    pub integridade: u64,
    /// Entregas divergentes ou recusadas.
    pub divergencias: u64,
}

/// A liquidação gravada.
#[derive(Clone, Debug)]
pub struct Liquidada {
    /// Cobrança.
    pub cobranca: i64,
    /// Por nó.
    pub por_no: Vec<(Id, i64)>,
    /// Conferência.
    pub conferencia: i64,
    /// Plataforma.
    pub plataforma: i64,
    /// Devolvido.
    pub devolvido: i64,
    /// Quando.
    pub t: u64,
}

/// Uma tarefa.
#[derive(Clone, Debug)]
pub struct Tarefa {
    /// Número.
    pub id: Id,
    /// Cliente.
    pub cliente: Id,
    /// Pedido.
    pub pedido: PedidoDeTarefa,
    /// Especificação-modelo.
    pub modelo: Especificacao,
    /// JOB do ULTRAX.
    pub job_ultrax: EspecificacaoDeJob,
    /// `JOB_ID` do ULTRAX.
    pub job_id: [u8; HASH_LEN],
    /// JOB da tarefa (de onde saem as sementes).
    pub job: [u8; HASH_LEN],
    /// Moeda.
    pub moeda: Moeda,
    /// Tabela de preços congelada na criação.
    pub tabela: Tabela,
    /// Parte do fornecedor congelada na criação.
    pub fornecedor_bp: u32,
    /// Estimativa, em micro.
    pub estimativa_micro: u128,
    /// Reserva, em centavos.
    pub reserva: i64,
    /// Estado.
    pub estado: EstadoTarefa,
    /// Motivo do último estado.
    pub motivo: String,
    /// Criada em.
    pub criada_ms: u64,
    /// Começou a rodar em.
    pub iniciada_ms: Option<u64>,
    /// Terminou em.
    pub concluida_ms: Option<u64>,
    /// Prazo absoluto.
    pub prazo_ms: u64,
    /// Pela API?
    pub via_api: bool,
    /// Próxima unidade nunca tocada.
    pub proxima: u64,
    /// As unidades já tocadas.
    pub unidades: BTreeMap<u64, Unidade>,
    /// Hash aceito de cada unidade concluída.
    pub resultados: BTreeMap<u64, [u8; HASH_LEN]>,
    /// Unidades concluídas.
    pub concluidas: u64,
    /// Unidades que falharam.
    pub falhas: u64,
    /// Atribuições abertas (sem entrega).
    pub em_voo: u32,
    /// Micro de cada nó, por pagador (o que entra na liquidação).
    pub execucoes: BTreeMap<(Id, PagadorOrd), u128>,
    /// Operações aceitas.
    pub operacoes: u128,
    /// Verificação.
    pub verificacao: Verificacao,
    /// Liquidação.
    pub liquidada: Option<Liquidada>,
    /// Resultados já apagados.
    pub resultados_apagados: bool,
}

/// [`Pagador`] com ordem, para servir de chave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PagadorOrd {
    /// O cliente.
    Cliente,
    /// A plataforma.
    Plataforma,
}

impl From<Pagador> for PagadorOrd {
    fn from(p: Pagador) -> Self {
        match p {
            Pagador::Cliente => Self::Cliente,
            Pagador::Plataforma => Self::Plataforma,
        }
    }
}

impl Tarefa {
    /// A especificação e a semente da unidade `i`.
    pub fn unidade(&self, i: u64) -> Option<(Especificacao, [u8; HASH_LEN])> {
        let esp = trabalho::derivar_unidade(&self.modelo, i).ok()?;
        Some((esp, regras::semente(&self.job, i)))
    }

    /// Todas as unidades fechadas?
    pub fn tudo_fechado(&self) -> bool {
        self.proxima >= self.pedido.unidades && self.concluidas.saturating_add(self.falhas) >= self.pedido.unidades
    }

    /// Tem unidade esperando nó?
    pub fn tem_o_que_despachar(&self) -> bool {
        !self.estado.final_() && (self.proxima < self.pedido.unidades || self.unidades.values().any(Unidade::precisa_de_no))
    }
}

/// Um pagamento.
#[derive(Clone, Debug)]
pub struct Pagamento {
    /// Número.
    pub id: Id,
    /// Cliente.
    pub usuario: Id,
    /// Centavos.
    pub valor: i64,
    /// Moeda.
    pub moeda: Moeda,
    /// Provedor.
    pub provedor: String,
    /// Código para a mensagem do Pix.
    pub codigo: String,
    /// `pendente`, `confirmado`, `cancelado` ou `estornado`.
    pub estado: &'static str,
    /// Criado em.
    pub criado_ms: u64,
    /// Atualizado em.
    pub atualizado_ms: u64,
    /// Referência externa.
    pub referencia: String,
    /// Taxa.
    pub taxa: i64,
    /// Nota (motivo do cancelamento ou do estorno).
    pub nota: String,
}

/// Um saque.
#[derive(Clone, Debug)]
pub struct Saque {
    /// Número.
    pub id: Id,
    /// Fornecedor.
    pub usuario: Id,
    /// Centavos.
    pub valor: i64,
    /// Moeda.
    pub moeda: Moeda,
    /// Chave Pix.
    pub destino: String,
    /// `solicitado`, `aprovado`, `pago` ou `recusado`.
    pub estado: String,
    /// Criado em.
    pub criado_ms: u64,
    /// Atualizado em.
    pub atualizado_ms: u64,
    /// Referência ou motivo.
    pub nota: String,
}

/// Ganho em retenção.
#[derive(Clone, Debug)]
pub struct Retido {
    /// Tarefa.
    pub tarefa: Id,
    /// Fornecedor.
    pub usuario: Id,
    /// Moeda.
    pub moeda: Moeda,
    /// Centavos.
    pub valor: i64,
    /// Libera em.
    pub libera_ms: u64,
    /// Já liberado.
    pub liberado: bool,
    /// Segurado pela análise de fraude.
    pub segurado: Option<String>,
}

/// Um alerta de segurança ou de fraude.
#[derive(Clone, Debug)]
pub struct Alerta {
    /// Número.
    pub id: Id,
    /// Tipo.
    pub tipo: String,
    /// Severidade.
    pub severidade: String,
    /// Alvo.
    pub alvo: String,
    /// Detalhe.
    pub detalhe: String,
    /// Criado em.
    pub criado_ms: u64,
    /// Resolvido: nota, quem, quando.
    pub resolvido: Option<(String, Id, u64)>,
}

/// Os próximos números.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sequencias {
    /// Usuário.
    pub usuario: Id,
    /// Nó.
    pub no: Id,
    /// Tarefa.
    pub tarefa: Id,
    /// Atribuição.
    pub atribuicao: Id,
    /// Pagamento.
    pub pagamento: Id,
    /// Saque.
    pub saque: Id,
    /// Chave de API.
    pub chave: Id,
    /// Alerta.
    pub alerta: Id,
    /// Lançamento.
    pub lancamento: Id,
}

fn proximo(atual: &mut Id, usado: Id) {
    if usado >= *atual {
        *atual = usado.saturating_add(1);
    }
}

/// O estado inteiro.
#[derive(Debug, Default)]
pub struct Estado {
    /// Configuração.
    pub config: Config,
    /// Usuários.
    pub usuarios: BTreeMap<Id, Usuario>,
    /// E-mail → usuário.
    pub por_email: HashMap<String, Id>,
    /// Hash do token → sessão.
    pub sessoes: HashMap<String, Sessao>,
    /// Chaves de API.
    pub chaves: BTreeMap<Id, ChaveApi>,
    /// Hash da chave → número.
    pub chave_por_hash: HashMap<String, Id>,
    /// Hash do código → vínculo.
    pub vinculos: HashMap<String, Vinculo>,
    /// Nós.
    pub nos: BTreeMap<Id, No>,
    /// Chave do worker → nó.
    pub no_por_chave: HashMap<[u8; 32], Id>,
    /// Tarefas.
    pub tarefas: BTreeMap<Id, Tarefa>,
    /// Tarefas que ainda não terminaram.
    pub ativas: BTreeSet<Id>,
    /// Atribuições.
    pub atribuicoes: BTreeMap<Id, Atribuicao>,
    /// Pagamentos.
    pub pagamentos: BTreeMap<Id, Pagamento>,
    /// Saques.
    pub saques: BTreeMap<Id, Saque>,
    /// Ganhos em retenção.
    pub retidos: Vec<Retido>,
    /// Alertas.
    pub alertas: BTreeMap<Id, Alerta>,
    /// O livro.
    pub livro: Livro,
    /// Próximos números.
    pub seq: Sequencias,
    /// Eventos aplicados.
    pub aplicados: u64,
}

impl Estado {
    /// Estado vazio, com a configuração padrão e os números começando em 1.
    pub fn novo() -> Self {
        Self {
            seq: Sequencias { usuario: 1, no: 1, tarefa: 1, atribuicao: 1, pagamento: 1, saque: 1, chave: 1, alerta: 1, lancamento: 1 },
            ..Self::default()
        }
    }

    /// Aplica um evento. Sem relógio, sem sorteio: só o que veio no evento.
    #[allow(clippy::too_many_lines)]
    pub fn aplicar(&mut self, ev: &Evento) -> Result<(), String> {
        self.aplicados = self.aplicados.saturating_add(1);
        match ev {
            Evento::UserCreated { id, email, nome, papeis, moeda, pais, senha, recuperacao, termos, ip, t } => {
                if self.usuarios.contains_key(id) || self.por_email.contains_key(email) {
                    return Err(format!("usuário {id} repetido"));
                }
                self.usuarios.insert(
                    *id,
                    Usuario {
                        id: *id,
                        email: email.clone(),
                        nome: nome.clone(),
                        papeis: *papeis,
                        moeda: *moeda,
                        pais: pais.clone(),
                        senha: senha.clone(),
                        recuperacao: recuperacao.clone(),
                        termos: *termos,
                        bloqueado: false,
                        motivo_bloqueio: String::new(),
                        verificado: false,
                        criado_ms: *t,
                        ip_cadastro: ip.clone(),
                    },
                );
                self.por_email.insert(email.clone(), *id);
                proximo(&mut self.seq.usuario, *id);
            }
            Evento::RoleGranted { usuario, papeis, .. } => self.usuario_mut(*usuario)?.papeis = *papeis,
            Evento::PasswordChanged { usuario, senha, .. } => {
                self.usuario_mut(*usuario)?.senha.clone_from(senha);
                // trocar a senha derruba todas as sessões da conta
                self.sessoes.retain(|_, s| s.usuario != *usuario);
            }
            Evento::RecoveryCodesIssued { usuario, recuperacao, .. } => self.usuario_mut(*usuario)?.recuperacao.clone_from(recuperacao),
            Evento::RecoveryCodeUsed { usuario, indice, .. } => {
                let u = self.usuario_mut(*usuario)?;
                if let Some(c) = u.recuperacao.get_mut(usize::try_from(*indice).unwrap_or(usize::MAX)) {
                    c.clear();
                }
            }
            Evento::UserStatus { usuario, bloqueado, motivo, .. } => {
                let u = self.usuario_mut(*usuario)?;
                u.bloqueado = *bloqueado;
                u.motivo_bloqueio.clone_from(motivo);
                if *bloqueado {
                    let id = *usuario;
                    self.sessoes.retain(|_, s| s.usuario != id);
                }
            }
            Evento::UserVerified { usuario, .. } => self.usuario_mut(*usuario)?.verificado = true,
            Evento::Login { sessao, usuario, expira, ip, agente, t } => {
                self.usuario_mut(*usuario)?;
                self.sessoes.insert(sessao.clone(), Sessao { usuario: *usuario, criada_ms: *t, expira_ms: *expira, ip: ip.clone(), agente: agente.clone() });
            }
            Evento::Logout { sessao, .. } => {
                self.sessoes.remove(sessao);
            }
            Evento::ApiKeyCreated { id, usuario, nome, prefixo, hash, escopos, limite_por_min, t } => {
                self.usuario_mut(*usuario)?;
                self.chaves.insert(
                    *id,
                    ChaveApi {
                        id: *id,
                        usuario: *usuario,
                        nome: nome.clone(),
                        prefixo: prefixo.clone(),
                        hash: hash.clone(),
                        escopos: escopos.clone(),
                        limite_por_min: *limite_por_min,
                        criada_ms: *t,
                        revogada_ms: None,
                    },
                );
                self.chave_por_hash.insert(hash.clone(), *id);
                proximo(&mut self.seq.chave, *id);
            }
            Evento::ApiKeyRevoked { id, t, .. } => {
                let c = self.chaves.get_mut(id).ok_or_else(|| format!("chave {id} desconhecida"))?;
                c.revogada_ms = Some(*t);
                self.chave_por_hash.remove(&c.hash);
            }
            Evento::LinkCodeIssued { codigo, usuario, expira, .. } => {
                self.vinculos.insert(codigo.clone(), Vinculo { usuario: *usuario, expira_ms: *expira, usado: false });
            }
            Evento::NodeRegistered { id, dono, chave, nome, hardware, versao, codigo, limites, t } => {
                let chave = de_hex_fixo::<32>(chave).ok_or("chave de nó inválida")?;
                if self.no_por_chave.contains_key(&chave) {
                    return Err(format!("chave do nó {id} repetida"));
                }
                if let Some(v) = self.vinculos.get_mut(codigo) {
                    v.usado = true;
                }
                self.nos.insert(
                    *id,
                    No {
                        id: *id,
                        dono: *dono,
                        chave,
                        nome: nome.clone(),
                        regiao: String::new(),
                        hardware: hardware.clone(),
                        versao: versao.clone(),
                        limites: *limites,
                        pausado: false,
                        adm: EstadoAdm::Normal,
                        motivo_adm: String::new(),
                        removido: false,
                        criado_ms: *t,
                        historico: Historico::default(),
                        ganhos: BTreeMap::new(),
                        online: false,
                        presenca_desde_ms: *t,
                        online_total_ms: 0,
                    },
                );
                self.no_por_chave.insert(chave, *id);
                proximo(&mut self.seq.no, *id);
            }
            Evento::NodeUpdated { no, hardware, versao, .. } => {
                let n = self.no_mut(*no)?;
                n.hardware = hardware.clone();
                n.versao.clone_from(versao);
            }
            Evento::NodeSettings { no, nome, regiao, limites, .. } => {
                let n = self.no_mut(*no)?;
                n.nome.clone_from(nome);
                n.regiao.clone_from(regiao);
                n.limites = *limites;
            }
            Evento::NodePaused { no, pausado, .. } => self.no_mut(*no)?.pausado = *pausado,
            Evento::NodeAdmin { no, estado, motivo, .. } => {
                let n = self.no_mut(*no)?;
                n.adm = *estado;
                n.motivo_adm.clone_from(motivo);
            }
            Evento::NodePresence { no, online, t } => {
                let n = self.no_mut(*no)?;
                if n.online {
                    n.online_total_ms = n.online_total_ms.saturating_add(t.saturating_sub(n.presenca_desde_ms));
                }
                n.online = *online;
                n.presenca_desde_ms = *t;
            }
            Evento::NodeRemoved { no, .. } => {
                let n = self.no_mut(*no)?;
                n.removido = true;
                let chave = n.chave;
                self.no_por_chave.remove(&chave);
            }
            Evento::TaskCreated { id, cliente, pedido, moeda, tabela, fornecedor_bp, job_id, nonce, job, estimativa_micro, reserva, prazo_ms, via_api, t } => {
                let (modelo, job_ultrax) = pedido.job()?;
                let job_id = de_hex_fixo::<HASH_LEN>(job_id).ok_or("job_id inválido")?;
                let nonce = de_hex_fixo::<32>(nonce).ok_or("nonce inválido")?;
                let job = de_hex_fixo::<HASH_LEN>(job).ok_or("job inválido")?;
                if job != regras::job_da_tarefa(&job_id, *id, &nonce) {
                    return Err(format!("o JOB da tarefa {id} não confere"));
                }
                self.tarefas.insert(
                    *id,
                    Tarefa {
                        id: *id,
                        cliente: *cliente,
                        pedido: pedido.clone(),
                        modelo,
                        job_ultrax,
                        job_id,
                        job,
                        moeda: *moeda,
                        tabela: *tabela,
                        fornecedor_bp: *fornecedor_bp,
                        estimativa_micro: estimativa_micro.parse().map_err(|_| "estimativa inválida")?,
                        reserva: *reserva,
                        estado: EstadoTarefa::Created,
                        motivo: String::new(),
                        criada_ms: *t,
                        iniciada_ms: None,
                        concluida_ms: None,
                        prazo_ms: *prazo_ms,
                        via_api: *via_api,
                        proxima: 0,
                        unidades: BTreeMap::new(),
                        resultados: BTreeMap::new(),
                        concluidas: 0,
                        falhas: 0,
                        em_voo: 0,
                        execucoes: BTreeMap::new(),
                        operacoes: 0,
                        verificacao: Verificacao::default(),
                        liquidada: None,
                        resultados_apagados: false,
                    },
                );
                self.ativas.insert(*id);
                proximo(&mut self.seq.tarefa, *id);
            }
            Evento::TaskState { tarefa, estado, motivo, t, .. } => {
                let x = self.tarefa_mut(*tarefa)?;
                x.estado = *estado;
                x.motivo.clone_from(motivo);
                if *estado == EstadoTarefa::Running && x.iniciada_ms.is_none() {
                    x.iniciada_ms = Some(*t);
                }
                if estado.final_() {
                    x.concluida_ms = Some(*t);
                    self.ativas.remove(tarefa);
                }
            }
            Evento::TaskAssigned { id, tarefa, unidade, no, papel, prazo_ms, t } => {
                let dono = self.nos.get(no).map(|n| n.dono).ok_or_else(|| format!("nó {no} desconhecido"))?;
                let x = self.tarefa_mut(*tarefa)?;
                if *unidade >= x.pedido.unidades {
                    return Err(format!("unidade {unidade} além do fim da tarefa {tarefa}"));
                }
                if *unidade >= x.proxima {
                    // as unidades nascem em ordem: a primeira vez que se toca
                    // a unidade i é quando i == proxima
                    if *unidade != x.proxima {
                        return Err(format!("unidade {unidade} fora de ordem na tarefa {tarefa}"));
                    }
                    x.proxima = x.proxima.saturating_add(1);
                    x.unidades.insert(*unidade, Unidade { necessarias: x.pedido.copias.max(1), pronta_ms: *t, ..Unidade::default() });
                }
                let u = x.unidades.get_mut(unidade).ok_or("unidade sumiu")?;
                u.em_voo.push(*id);
                u.todas.push(*id);
                x.em_voo = x.em_voo.saturating_add(1);
                self.atribuicoes.insert(
                    *id,
                    Atribuicao {
                        id: *id,
                        tarefa: *tarefa,
                        unidade: *unidade,
                        no: *no,
                        dono,
                        papel: *papel,
                        emitida_ms: *t,
                        prazo_ms: *prazo_ms,
                        recebida: false,
                        executando: false,
                        entrega: None,
                        desfecho: None,
                    },
                );
                proximo(&mut self.seq.atribuicao, *id);
            }
            Evento::AssignmentProgress { atribuicao, executando, .. } => {
                let a = self.atribuicoes.get_mut(atribuicao).ok_or_else(|| format!("atribuição {atribuicao} desconhecida"))?;
                a.recebida = true;
                a.executando |= *executando;
            }
            Evento::ResultDelivered { atribuicao, hash, operacoes, bytes, ms_calculo, gpu, prova, t } => {
                let hash = de_hex_fixo::<HASH_LEN>(hash).ok_or("hash inválido")?;
                let a = self.atribuicoes.get_mut(atribuicao).ok_or_else(|| format!("atribuição {atribuicao} desconhecida"))?;
                if !a.aberta() {
                    return Err(format!("atribuição {atribuicao} já entregue ou fechada"));
                }
                a.entrega = Some(Entrega { hash, operacoes: *operacoes, bytes: *bytes, ms_calculo: *ms_calculo, gpu: *gpu, prova: prova.clone(), t: *t });
                a.recebida = true;
                a.executando = true;
                let (tarefa, unidade, no) = (a.tarefa, a.unidade, a.no);
                let x = self.tarefa_mut(tarefa)?;
                let u = x.unidades.get_mut(&unidade).ok_or("unidade sem atribuição")?;
                u.em_voo.retain(|i| i != atribuicao);
                u.entregas.push((*atribuicao, hash, *operacoes));
                x.em_voo = x.em_voo.saturating_sub(1);
                let n = self.no_mut(no)?;
                n.historico.entregas = n.historico.entregas.saturating_add(1);
            }
            Evento::CheckRequested { tarefa, unidade, t, .. } => {
                let x = self.tarefa_mut(*tarefa)?;
                let u = x.unidades.get_mut(unidade).ok_or("unidade desconhecida")?;
                u.necessarias = u.necessarias.saturating_add(1);
                u.conferencias = u.conferencias.saturating_add(1);
                u.pronta_ms = *t;
            }
            Evento::CheckDropped { tarefa, unidade, .. } => {
                let x = self.tarefa_mut(*tarefa)?;
                let u = x.unidades.get_mut(unidade).ok_or("unidade desconhecida")?;
                if u.conferencias == 0 {
                    return Err(format!("unidade {unidade} sem conferência para dispensar"));
                }
                u.necessarias = u.necessarias.saturating_sub(1);
                u.conferencias = u.conferencias.saturating_sub(1);
            }
            Evento::AssignmentClosed { atribuicao, desfecho, motivo, t } => {
                let a = self.atribuicoes.get_mut(atribuicao).ok_or_else(|| format!("atribuição {atribuicao} desconhecida"))?;
                if a.desfecho.is_some() {
                    return Err(format!("atribuição {atribuicao} fechada duas vezes"));
                }
                a.desfecho = Some((*desfecho, motivo.clone(), *t));
                let (tarefa, unidade, no, sem_entrega) = (a.tarefa, a.unidade, a.no, a.entrega.is_none());
                let x = self.tarefa_mut(tarefa)?;
                let u = x.unidades.get_mut(&unidade).ok_or("unidade sem atribuição")?;
                u.em_voo.retain(|i| i != atribuicao);
                u.entregas.retain(|(i, _, _)| i != atribuicao);
                if sem_entrega {
                    x.em_voo = x.em_voo.saturating_sub(1);
                }
                let tenta = matches!(desfecho, Desfecho::Recusada | Desfecho::Divergente | Desfecho::Vencida | Desfecho::Abandonada);
                if tenta {
                    u.tentativas = u.tentativas.saturating_add(1);
                    u.pronta_ms = *t;
                }
                if matches!(desfecho, Desfecho::Recusada | Desfecho::Divergente) {
                    x.verificacao.divergencias = x.verificacao.divergencias.saturating_add(1);
                }
                let n = self.no_mut(no)?;
                match desfecho {
                    Desfecho::Aceita => n.historico.aceitas = n.historico.aceitas.saturating_add(1),
                    Desfecho::Recusada | Desfecho::Divergente => n.historico.incorretas = n.historico.incorretas.saturating_add(1),
                    Desfecho::Vencida | Desfecho::Abandonada => n.historico.falhas = n.historico.falhas.saturating_add(1),
                    Desfecho::Cancelada | Desfecho::Dispensada => {}
                }
            }
            Evento::UnitCompleted { tarefa, unidade, hash, pagas, verificacao, .. } => {
                let hash = de_hex_fixo::<HASH_LEN>(hash).ok_or("hash inválido")?;
                // custo de cada execução paga, pela tabela congelada na tarefa
                let mut custos = Vec::with_capacity(pagas.len());
                let conferida = verificacao != "integridade";
                {
                    let x = self.tarefas.get(tarefa).ok_or_else(|| format!("tarefa {tarefa} desconhecida"))?;
                    let (esp, _) = x.unidade(*unidade).ok_or("unidade inválida")?;
                    for (atrib, pagador) in pagas {
                        let a = self.atribuicoes.get(atrib).ok_or_else(|| format!("atribuição {atrib} desconhecida"))?;
                        let e = a.entrega.as_ref().ok_or("atribuição paga sem entrega")?;
                        let micro = regras::custo_da_execucao(&x.tabela, e.operacoes, esp.memoria_bytes(), x.pedido.gpu)?;
                        custos.push((a.no, *pagador, micro, e.operacoes));
                    }
                }
                for (no, _, _, _) in &custos {
                    if conferida {
                        let n = self.no_mut(*no)?;
                        n.historico.verificadas = n.historico.verificadas.saturating_add(1);
                    }
                }
                let x = self.tarefa_mut(*tarefa)?;
                let u = x.unidades.get_mut(unidade).ok_or("unidade desconhecida")?;
                if u.fechada() {
                    return Err(format!("unidade {unidade} da tarefa {tarefa} fechada duas vezes"));
                }
                u.concluida = Some(hash);
                x.resultados.insert(*unidade, hash);
                x.concluidas = x.concluidas.saturating_add(1);
                match verificacao.as_str() {
                    "freivalds" => x.verificacao.freivalds = x.verificacao.freivalds.saturating_add(1),
                    "concordancia" => x.verificacao.concordancia = x.verificacao.concordancia.saturating_add(1),
                    _ => x.verificacao.integridade = x.verificacao.integridade.saturating_add(1),
                }
                for (no, pagador, micro, ops) in custos {
                    let v = x.execucoes.entry((no, pagador.into())).or_insert(0);
                    *v = v.saturating_add(micro);
                    if pagador == Pagador::Cliente {
                        x.operacoes = x.operacoes.saturating_add(u128::from(ops));
                    }
                }
            }
            Evento::UnitFailed { tarefa, unidade, motivo, .. } => {
                let x = self.tarefa_mut(*tarefa)?;
                let u = x.unidades.get_mut(unidade).ok_or("unidade desconhecida")?;
                if u.fechada() {
                    return Err(format!("unidade {unidade} da tarefa {tarefa} fechada duas vezes"));
                }
                u.falhou = Some(motivo.clone());
                x.falhas = x.falhas.saturating_add(1);
            }
            Evento::TaskSettled { tarefa, cobranca, por_no, conferencia, plataforma, devolvido, libera_ms, t } => {
                let moeda = {
                    let x = self.tarefa_mut(*tarefa)?;
                    if x.liquidada.is_some() {
                        return Err(format!("tarefa {tarefa} liquidada duas vezes"));
                    }
                    x.liquidada = Some(Liquidada {
                        cobranca: *cobranca,
                        por_no: por_no.clone(),
                        conferencia: *conferencia,
                        plataforma: *plataforma,
                        devolvido: *devolvido,
                        t: *t,
                    });
                    x.moeda
                };
                let mut por_dono: BTreeMap<Id, i64> = BTreeMap::new();
                for (no, v) in por_no {
                    let n = self.no_mut(*no)?;
                    let g = n.ganhos.entry(moeda).or_insert(0);
                    *g = g.saturating_add(*v);
                    let d = por_dono.entry(n.dono).or_insert(0);
                    *d = d.saturating_add(*v);
                }
                for (usuario, valor) in por_dono {
                    if valor > 0 {
                        self.retidos.push(Retido { tarefa: *tarefa, usuario, moeda, valor, libera_ms: *libera_ms, liberado: false, segurado: None });
                    }
                }
            }
            Evento::ResultsPurged { tarefa, .. } => self.tarefa_mut(*tarefa)?.resultados_apagados = true,
            Evento::Ledger(l) => {
                let id = l.id;
                self.livro.aplicar(l.clone())?;
                proximo(&mut self.seq.lancamento, id);
            }
            Evento::PaymentCreated { id, usuario, valor, moeda, provedor, codigo, t } => {
                self.pagamentos.insert(
                    *id,
                    Pagamento {
                        id: *id,
                        usuario: *usuario,
                        valor: *valor,
                        moeda: *moeda,
                        provedor: provedor.clone(),
                        codigo: codigo.clone(),
                        estado: "pendente",
                        criado_ms: *t,
                        atualizado_ms: *t,
                        referencia: String::new(),
                        taxa: 0,
                        nota: String::new(),
                    },
                );
                proximo(&mut self.seq.pagamento, *id);
            }
            Evento::PaymentConfirmed { id, referencia, taxa, t, .. } => {
                let p = self.pagamento_mut(*id)?;
                p.estado = "confirmado";
                p.referencia.clone_from(referencia);
                p.taxa = *taxa;
                p.atualizado_ms = *t;
            }
            Evento::PaymentCancelled { id, motivo, t, .. } => {
                let p = self.pagamento_mut(*id)?;
                p.estado = "cancelado";
                p.nota.clone_from(motivo);
                p.atualizado_ms = *t;
            }
            Evento::PaymentRefunded { id, motivo, t, .. } => {
                let p = self.pagamento_mut(*id)?;
                p.estado = "estornado";
                p.nota.clone_from(motivo);
                p.atualizado_ms = *t;
            }
            Evento::EarningsReleased { tarefa, usuario, .. } => {
                if let Some(r) = self.retidos.iter_mut().find(|r| r.tarefa == *tarefa && r.usuario == *usuario && !r.liberado) {
                    r.liberado = true;
                } else {
                    return Err(format!("nada retido da tarefa {tarefa} para {usuario}"));
                }
            }
            Evento::EarningsHeld { tarefa, usuario, motivo, .. } => {
                for r in self.retidos.iter_mut().filter(|r| r.tarefa == *tarefa && r.usuario == *usuario && !r.liberado) {
                    r.segurado = Some(motivo.clone());
                }
            }
            Evento::PayoutRequested { id, usuario, valor, moeda, destino, t } => {
                self.saques.insert(
                    *id,
                    Saque {
                        id: *id,
                        usuario: *usuario,
                        valor: *valor,
                        moeda: *moeda,
                        destino: destino.clone(),
                        estado: "solicitado".into(),
                        criado_ms: *t,
                        atualizado_ms: *t,
                        nota: String::new(),
                    },
                );
                proximo(&mut self.seq.saque, *id);
            }
            Evento::PayoutUpdated { id, estado, nota, t, .. } => {
                let s = self.saques.get_mut(id).ok_or_else(|| format!("saque {id} desconhecido"))?;
                s.estado.clone_from(estado);
                s.nota.clone_from(nota);
                s.atualizado_ms = *t;
            }
            Evento::ConfigChanged { config, .. } => self.config = config.clone(),
            Evento::SecurityAlert { id, tipo, severidade, alvo, detalhe, t } => {
                self.alertas.insert(
                    *id,
                    Alerta {
                        id: *id,
                        tipo: tipo.clone(),
                        severidade: severidade.clone(),
                        alvo: alvo.clone(),
                        detalhe: detalhe.clone(),
                        criado_ms: *t,
                        resolvido: None,
                    },
                );
                proximo(&mut self.seq.alerta, *id);
            }
            Evento::AlertResolved { id, nota, por, t } => {
                let a = self.alertas.get_mut(id).ok_or_else(|| format!("alerta {id} desconhecido"))?;
                a.resolvido = Some((nota.clone(), *por, *t));
            }
        }
        Ok(())
    }

    fn usuario_mut(&mut self, id: Id) -> Result<&mut Usuario, String> {
        self.usuarios.get_mut(&id).ok_or_else(|| format!("usuário {id} desconhecido"))
    }

    fn no_mut(&mut self, id: Id) -> Result<&mut No, String> {
        self.nos.get_mut(&id).ok_or_else(|| format!("nó {id} desconhecido"))
    }

    fn tarefa_mut(&mut self, id: Id) -> Result<&mut Tarefa, String> {
        self.tarefas.get_mut(&id).ok_or_else(|| format!("tarefa {id} desconhecida"))
    }

    fn pagamento_mut(&mut self, id: Id) -> Result<&mut Pagamento, String> {
        self.pagamentos.get_mut(&id).ok_or_else(|| format!("pagamento {id} desconhecido"))
    }

    /// Um resumo do estado inteiro, para conferir que reler o diário dá o
    /// mesmo estado.
    pub fn impressao(&self) -> String {
        let mut s = String::new();
        for (c, m, v) in self.livro.contas() {
            s.push_str(&format!("{c}/{}={v};", m.codigo()));
        }
        for t in self.tarefas.values() {
            s.push_str(&format!("t{}:{}:{}:{}:{};", t.id, t.estado.nome(), t.concluidas, t.falhas, t.em_voo));
        }
        for n in self.nos.values() {
            let h = n.historico;
            s.push_str(&format!("n{}:{}:{}:{}:{}:{:?};", n.id, h.entregas, h.verificadas, h.falhas, h.incorretas, n.adm));
        }
        s.push_str(&format!("u{};s{};a{};p{};q{}", self.usuarios.len(), self.sessoes.len(), self.atribuicoes.len(), self.pagamentos.len(), self.saques.len()));
        s
    }
}

/// Hex de volta em bytes de tamanho livre.
pub fn bytes_de_hex(texto: &str) -> Option<Vec<u8>> {
    de_hex(texto)
}
