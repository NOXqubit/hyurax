//! Os eventos do diário. Cada mudança de estado da plataforma é um evento, e
//! o estado é só o resultado de aplicar todos, em ordem (ver `estado.rs`).
//!
//! Isso dá três coisas de uma vez: o **sistema de eventos** (seção 36 do
//! documento da V1), o **registro de auditoria** (seção 42: quem fez o quê e
//! quando, inclusive o administrador) e a **recuperação** (seção 44: ao
//! reabrir, o diário é relido e tudo volta como estava).
//!
//! Regra que o teste de segurança confere: **nenhum evento carrega senha,
//! token de sessão, chave de API ou código de vínculo**. Só o hash deles.

use serde::{Deserialize, Serialize};

use crate::dinheiro::{Lancamento, Moeda};
use crate::modelo::{Config, Escopo, Hardware, Id, Limites, Papeis, PedidoDeTarefa};
use crate::regras::Pagador;

/// Estado de uma tarefa (seção 13 do documento da V1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EstadoTarefa {
    /// Recebida.
    Created,
    /// Validada, com saldo reservado, esperando nó.
    Queued,
    /// Um nó foi escolhido para a primeira unidade.
    Scheduled,
    /// O agente recebeu a primeira unidade.
    Dispatched,
    /// Unidades executando.
    Running,
    /// Tudo entregue; faltam conferências.
    Verifying,
    /// Concluída e liquidada.
    Completed,
    /// Nenhuma unidade pôde ser concluída.
    Failed,
    /// Cancelada pelo cliente ou pelo administrador.
    Cancelled,
    /// O prazo venceu.
    Timeout,
    /// Recusada pela política (motor desligado, conta bloqueada).
    Rejected,
}

impl EstadoTarefa {
    /// Não muda mais.
    pub fn final_(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled | Self::Timeout | Self::Rejected)
    }

    /// O nome, como a API mostra.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Created => "CREATED",
            Self::Queued => "QUEUED",
            Self::Scheduled => "SCHEDULED",
            Self::Dispatched => "DISPATCHED",
            Self::Running => "RUNNING",
            Self::Verifying => "VERIFYING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Timeout => "TIMEOUT",
            Self::Rejected => "REJECTED",
        }
    }
}

/// O papel de uma atribuição na unidade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Papel {
    /// Uma das cópias que o cliente pediu.
    Copia,
    /// Conferência sorteada pela plataforma (ou desempate).
    Conferencia,
}

/// Como uma atribuição terminou.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Desfecho {
    /// Resultado aceito.
    Aceita,
    /// Resultado conferido e errado (Freivalds, formato, assinatura).
    Recusada,
    /// Resultado diferente da maioria.
    Divergente,
    /// O prazo venceu sem entrega.
    Vencida,
    /// O nó desistiu (memória, erro de execução).
    Abandonada,
    /// Cancelada pela plataforma (tarefa encerrada, kill switch).
    Cancelada,
    /// Entregue, mas a unidade já tinha fechado com outras cópias.
    Dispensada,
}

/// Estado administrativo de um nó.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EstadoAdm {
    /// Sem restrição.
    Normal,
    /// Isolado por suspeita: não recebe trabalho.
    Quarantined,
    /// Em manutenção: não recebe trabalho.
    Maintenance,
    /// Banido.
    Banned,
}

/// Um evento do diário.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "evento", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Evento {
    // ---------------------------------------------------------------- contas
    /// `USER_CREATED`.
    UserCreated {
        /// Número.
        id: Id,
        /// E-mail, em minúsculas.
        email: String,
        /// Nome.
        nome: String,
        /// Papéis.
        papeis: Papeis,
        /// Moeda da conta.
        moeda: Moeda,
        /// País (ISO 3166, duas letras).
        pais: String,
        /// Hash da senha (Argon2id).
        senha: String,
        /// Hashes dos códigos de recuperação.
        recuperacao: Vec<String>,
        /// Versão dos termos aceitos.
        termos: u32,
        /// IP do cadastro.
        ip: String,
        /// Quando.
        t: u64,
    },
    /// O usuário passou a ter mais um papel.
    RoleGranted {
        /// Usuário.
        usuario: Id,
        /// Papéis depois da mudança.
        papeis: Papeis,
        /// Quem fez (o próprio ou um administrador).
        por: Id,
        /// Quando.
        t: u64,
    },
    /// Senha trocada (pelo dono, por código de recuperação ou pelo admin).
    PasswordChanged {
        /// Usuário.
        usuario: Id,
        /// O hash novo.
        senha: String,
        /// Como: `troca`, `recuperacao` ou `admin`.
        como: String,
        /// Quem fez.
        por: Id,
        /// Quando.
        t: u64,
    },
    /// Códigos de recuperação novos.
    RecoveryCodesIssued {
        /// Usuário.
        usuario: Id,
        /// Hashes.
        recuperacao: Vec<String>,
        /// Quando.
        t: u64,
    },
    /// Um código de recuperação foi usado.
    RecoveryCodeUsed {
        /// Usuário.
        usuario: Id,
        /// Qual.
        indice: u32,
        /// Quando.
        t: u64,
    },
    /// Conta bloqueada ou desbloqueada.
    UserStatus {
        /// Usuário.
        usuario: Id,
        /// Bloqueada?
        bloqueado: bool,
        /// Por quê.
        motivo: String,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },
    /// E-mail marcado como conferido.
    UserVerified {
        /// Usuário.
        usuario: Id,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },
    /// `LOGIN`: sessão aberta.
    Login {
        /// Hash do token.
        sessao: String,
        /// Usuário.
        usuario: Id,
        /// Vale até.
        expira: u64,
        /// IP.
        ip: String,
        /// Navegador, curto.
        agente: String,
        /// Quando.
        t: u64,
    },
    /// `LOGOUT` (ou sessão encerrada pela troca de senha ou pelo admin).
    Logout {
        /// Hash do token.
        sessao: String,
        /// Por quê.
        motivo: String,
        /// Quando.
        t: u64,
    },
    /// Chave de API criada.
    ApiKeyCreated {
        /// Número.
        id: Id,
        /// Dono.
        usuario: Id,
        /// Nome dado pelo dono.
        nome: String,
        /// O começo da chave, para reconhecer na lista.
        prefixo: String,
        /// Hash da chave.
        hash: String,
        /// O que ela pode.
        escopos: Vec<Escopo>,
        /// Pedidos por minuto.
        limite_por_min: u32,
        /// Quando.
        t: u64,
    },
    /// Chave de API revogada.
    ApiKeyRevoked {
        /// Número.
        id: Id,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },

    // ---------------------------------------------------------------- nós
    /// Código de vínculo para registrar um computador.
    LinkCodeIssued {
        /// Hash do código.
        codigo: String,
        /// Fornecedor.
        usuario: Id,
        /// Vale até.
        expira: u64,
        /// Quando.
        t: u64,
    },
    /// `NODE_REGISTERED`.
    NodeRegistered {
        /// Número.
        id: Id,
        /// Fornecedor.
        dono: Id,
        /// Chave do worker (hex, 32 bytes).
        chave: String,
        /// Nome dado pelo fornecedor.
        nome: String,
        /// Hardware.
        hardware: Hardware,
        /// Versão do agente.
        versao: String,
        /// Hash do código de vínculo usado.
        codigo: String,
        /// Limites iniciais.
        limites: Limites,
        /// Quando.
        t: u64,
    },
    /// O agente relatou hardware ou versão diferentes.
    NodeUpdated {
        /// Nó.
        no: Id,
        /// Hardware.
        hardware: Hardware,
        /// Versão do agente.
        versao: String,
        /// Quando.
        t: u64,
    },
    /// O fornecedor mudou o nome, a região ou os limites.
    NodeSettings {
        /// Nó.
        no: Id,
        /// Nome.
        nome: String,
        /// Região.
        regiao: String,
        /// Limites.
        limites: Limites,
        /// Quando.
        t: u64,
    },
    /// O fornecedor pausou ou retomou.
    NodePaused {
        /// Nó.
        no: Id,
        /// Pausado?
        pausado: bool,
        /// Quando.
        t: u64,
    },
    /// `NODE_DISABLED` e afins: quarentena, manutenção, banimento, liberação.
    NodeAdmin {
        /// Nó.
        no: Id,
        /// Estado novo.
        estado: EstadoAdm,
        /// Por quê.
        motivo: String,
        /// Quem (0: a própria plataforma).
        por: Id,
        /// Quando.
        t: u64,
    },
    /// `NODE_ONLINE` / `NODE_OFFLINE`: o pulso do agente começou ou parou.
    NodePresence {
        /// Nó.
        no: Id,
        /// Online?
        online: bool,
        /// Quando.
        t: u64,
    },
    /// O fornecedor desligou o computador da plataforma.
    NodeRemoved {
        /// Nó.
        no: Id,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },

    // ---------------------------------------------------------------- tarefas
    /// `TASK_CREATED`: a tarefa, a reserva e o JOB próprio.
    TaskCreated {
        /// Número.
        id: Id,
        /// Cliente.
        cliente: Id,
        /// O pedido.
        pedido: PedidoDeTarefa,
        /// Moeda.
        moeda: Moeda,
        /// Tabela de preços no momento da criação (hex não; números).
        tabela: crate::regras::Tabela,
        /// Parte do fornecedor no momento da criação.
        fornecedor_bp: u32,
        /// `JOB_ID` do ULTRAX (hex).
        job_id: String,
        /// Nonce da tarefa (hex, 32 bytes).
        nonce: String,
        /// JOB da tarefa (hex).
        job: String,
        /// Estimativa, em micro (texto decimal).
        estimativa_micro: String,
        /// Reserva, em centavos.
        reserva: i64,
        /// Prazo absoluto, em ms.
        prazo_ms: u64,
        /// Pela API (chave) ou pelo painel.
        via_api: bool,
        /// Quando.
        t: u64,
    },
    /// Mudança de estado da tarefa.
    TaskState {
        /// Tarefa.
        tarefa: Id,
        /// Estado novo.
        estado: EstadoTarefa,
        /// Por quê.
        motivo: String,
        /// Quem (0: a plataforma).
        por: Id,
        /// Quando.
        t: u64,
    },
    /// `TASK_ASSIGNED`: uma unidade foi entregue a um nó.
    TaskAssigned {
        /// Número da atribuição.
        id: Id,
        /// Tarefa.
        tarefa: Id,
        /// Unidade.
        unidade: u64,
        /// Nó.
        no: Id,
        /// Papel.
        papel: Papel,
        /// Prazo da atribuição.
        prazo_ms: u64,
        /// Quando.
        t: u64,
    },
    /// O agente confirmou que recebeu (e, com `executando`, que começou).
    AssignmentProgress {
        /// Atribuição.
        atribuicao: Id,
        /// Começou a executar?
        executando: bool,
        /// Quando.
        t: u64,
    },
    /// O nó entregou um resultado (que ainda vai ser julgado).
    ResultDelivered {
        /// Atribuição.
        atribuicao: Id,
        /// Hash do resultado (hex).
        hash: String,
        /// Operações pelo modelo de custo.
        operacoes: u64,
        /// Bytes do resultado.
        bytes: u64,
        /// Tempo de cálculo relatado.
        ms_calculo: u64,
        /// Na GPU?
        gpu: bool,
        /// Hash do registro de prova assinado (hex).
        prova: String,
        /// Quando.
        t: u64,
    },
    /// Uma conferência por outro nó foi sorteada para a unidade.
    CheckRequested {
        /// Tarefa.
        tarefa: Id,
        /// Unidade.
        unidade: u64,
        /// Por quê (`amostragem`, `prova`, `desempate`).
        motivo: String,
        /// Quando.
        t: u64,
    },
    /// A conferência sorteada foi dispensada: nenhum outro fornecedor ficou
    /// disponível no prazo. A unidade fecha pela integridade.
    CheckDropped {
        /// Tarefa.
        tarefa: Id,
        /// Unidade.
        unidade: u64,
        /// Por quê.
        motivo: String,
        /// Quando.
        t: u64,
    },
    /// Como a atribuição terminou.
    AssignmentClosed {
        /// Atribuição.
        atribuicao: Id,
        /// Desfecho.
        desfecho: Desfecho,
        /// Por quê.
        motivo: String,
        /// Quando.
        t: u64,
    },
    /// `RESULT_VERIFIED`: a unidade fechou com este resultado.
    UnitCompleted {
        /// Tarefa.
        tarefa: Id,
        /// Unidade.
        unidade: u64,
        /// Hash do resultado aceito (hex).
        hash: String,
        /// Atribuições que concordaram, com quem paga cada uma.
        pagas: Vec<(Id, Pagador)>,
        /// Como foi conferida (`freivalds`, `concordancia`, `integridade`).
        verificacao: String,
        /// Quando.
        t: u64,
    },
    /// A unidade não pôde ser concluída.
    UnitFailed {
        /// Tarefa.
        tarefa: Id,
        /// Unidade.
        unidade: u64,
        /// Por quê.
        motivo: String,
        /// Quando.
        t: u64,
    },
    /// Cobrança da tarefa e divisão (o lançamento vem junto).
    TaskSettled {
        /// Tarefa.
        tarefa: Id,
        /// O que o cliente pagou, em centavos.
        cobranca: i64,
        /// Ganho de cada nó, em centavos.
        por_no: Vec<(Id, i64)>,
        /// Parte das conferências (custo da plataforma).
        conferencia: i64,
        /// O que ficou com a plataforma (pode ser negativo).
        plataforma: i64,
        /// Devolvido ao saldo do cliente.
        devolvido: i64,
        /// Libera para saque em.
        libera_ms: u64,
        /// Quando.
        t: u64,
    },
    /// Os resultados guardados foram apagados (fim da retenção).
    ResultsPurged {
        /// Tarefa.
        tarefa: Id,
        /// Quando.
        t: u64,
    },

    // ---------------------------------------------------------------- dinheiro
    /// Um lançamento no livro.
    Ledger(Lancamento),
    /// Pedido de pagamento (adicionar saldo).
    PaymentCreated {
        /// Número.
        id: Id,
        /// Cliente.
        usuario: Id,
        /// Centavos.
        valor: i64,
        /// Moeda.
        moeda: Moeda,
        /// `pix-manual` ou `teste`.
        provedor: String,
        /// Identificador para a mensagem do Pix.
        codigo: String,
        /// Quando.
        t: u64,
    },
    /// `PAYMENT_COMPLETED` (o lançamento vem junto).
    PaymentConfirmed {
        /// Pagamento.
        id: Id,
        /// Referência do banco ou do provedor.
        referencia: String,
        /// Taxa do provedor, em centavos.
        taxa: i64,
        /// Quem confirmou (0: o provedor de teste).
        por: Id,
        /// Quando.
        t: u64,
    },
    /// Pagamento cancelado (nunca chegou).
    PaymentCancelled {
        /// Pagamento.
        id: Id,
        /// Por quê.
        motivo: String,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },
    /// Estorno (chargeback); o lançamento vem junto.
    PaymentRefunded {
        /// Pagamento.
        id: Id,
        /// Por quê.
        motivo: String,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },
    /// Ganho liberado da retenção (o lançamento vem junto).
    EarningsReleased {
        /// Tarefa de origem.
        tarefa: Id,
        /// Fornecedor.
        usuario: Id,
        /// Centavos.
        valor: i64,
        /// Moeda.
        moeda: Moeda,
        /// Quando.
        t: u64,
    },
    /// Ganho segurado pela análise de fraude.
    EarningsHeld {
        /// Tarefa de origem.
        tarefa: Id,
        /// Fornecedor.
        usuario: Id,
        /// Por quê.
        motivo: String,
        /// Quando.
        t: u64,
    },
    /// `PAYOUT_CREATED`: pedido de saque.
    PayoutRequested {
        /// Número.
        id: Id,
        /// Fornecedor.
        usuario: Id,
        /// Centavos.
        valor: i64,
        /// Moeda.
        moeda: Moeda,
        /// Chave Pix de destino.
        destino: String,
        /// Quando.
        t: u64,
    },
    /// Saque aprovado, pago ou recusado.
    PayoutUpdated {
        /// Saque.
        id: Id,
        /// `aprovado`, `pago` ou `recusado`.
        estado: String,
        /// Referência do pagamento ou motivo da recusa.
        nota: String,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },

    // ---------------------------------------------------------------- operação
    /// `ADMIN_ACTION`: configuração nova (preços, comissão, limites, kill switch).
    ConfigChanged {
        /// A configuração inteira.
        config: Config,
        /// O que mudou, em texto.
        resumo: String,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },
    /// `SECURITY_ALERT`.
    SecurityAlert {
        /// Número.
        id: Id,
        /// Tipo (`fraude:...`, `seguranca:...`).
        tipo: String,
        /// `baixa`, `media`, `alta` ou `critica`.
        severidade: String,
        /// Alvo (`usuario:3`, `no:5`, `ip:...`).
        alvo: String,
        /// Detalhe.
        detalhe: String,
        /// Quando.
        t: u64,
    },
    /// Alerta resolvido.
    AlertResolved {
        /// Alerta.
        id: Id,
        /// Nota.
        nota: String,
        /// Quem.
        por: Id,
        /// Quando.
        t: u64,
    },
}

impl Evento {
    /// O nome do tipo, como aparece no registro.
    pub fn nome(&self) -> &'static str {
        match self {
            Self::UserCreated { .. } => "USER_CREATED",
            Self::RoleGranted { .. } => "ROLE_GRANTED",
            Self::PasswordChanged { .. } => "PASSWORD_CHANGED",
            Self::RecoveryCodesIssued { .. } => "RECOVERY_CODES_ISSUED",
            Self::RecoveryCodeUsed { .. } => "RECOVERY_CODE_USED",
            Self::UserStatus { .. } => "USER_STATUS",
            Self::UserVerified { .. } => "USER_VERIFIED",
            Self::Login { .. } => "LOGIN",
            Self::Logout { .. } => "LOGOUT",
            Self::ApiKeyCreated { .. } => "API_KEY_CREATED",
            Self::ApiKeyRevoked { .. } => "API_KEY_REVOKED",
            Self::LinkCodeIssued { .. } => "LINK_CODE_ISSUED",
            Self::NodeRegistered { .. } => "NODE_REGISTERED",
            Self::NodeUpdated { .. } => "NODE_UPDATED",
            Self::NodeSettings { .. } => "NODE_SETTINGS",
            Self::NodePaused { .. } => "NODE_PAUSED",
            Self::NodeAdmin { .. } => "NODE_ADMIN",
            Self::NodePresence { online: true, .. } => "NODE_ONLINE",
            Self::NodePresence { online: false, .. } => "NODE_OFFLINE",
            Self::NodeRemoved { .. } => "NODE_REMOVED",
            Self::TaskCreated { .. } => "TASK_CREATED",
            Self::TaskState { estado: EstadoTarefa::Running, .. } => "TASK_STARTED",
            Self::TaskState { estado: EstadoTarefa::Completed, .. } => "TASK_COMPLETED",
            Self::TaskState { estado: EstadoTarefa::Failed, .. } => "TASK_FAILED",
            Self::TaskState { estado: EstadoTarefa::Cancelled, .. } => "TASK_KILLED",
            Self::TaskState { .. } => "TASK_STATE",
            Self::TaskAssigned { .. } => "TASK_ASSIGNED",
            Self::AssignmentProgress { .. } => "ASSIGNMENT_PROGRESS",
            Self::ResultDelivered { .. } => "RESULT_DELIVERED",
            Self::CheckRequested { .. } => "CHECK_REQUESTED",
            Self::CheckDropped { .. } => "CHECK_DROPPED",
            Self::AssignmentClosed { .. } => "ASSIGNMENT_CLOSED",
            Self::UnitCompleted { .. } => "RESULT_VERIFIED",
            Self::UnitFailed { .. } => "UNIT_FAILED",
            Self::TaskSettled { .. } => "TASK_SETTLED",
            Self::ResultsPurged { .. } => "RESULTS_PURGED",
            Self::Ledger(_) => "LEDGER",
            Self::PaymentCreated { .. } => "PAYMENT_CREATED",
            Self::PaymentConfirmed { .. } => "PAYMENT_COMPLETED",
            Self::PaymentCancelled { .. } => "PAYMENT_CANCELLED",
            Self::PaymentRefunded { .. } => "PAYMENT_REFUNDED",
            Self::EarningsReleased { .. } => "EARNINGS_RELEASED",
            Self::EarningsHeld { .. } => "EARNINGS_HELD",
            Self::PayoutRequested { .. } => "PAYOUT_CREATED",
            Self::PayoutUpdated { .. } => "PAYOUT_UPDATED",
            Self::ConfigChanged { .. } => "ADMIN_ACTION",
            Self::SecurityAlert { .. } => "SECURITY_ALERT",
            Self::AlertResolved { .. } => "ALERT_RESOLVED",
        }
    }
}
