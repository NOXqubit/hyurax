// ✝ Provérbios 21:5 — “Os planos do diligente tendem à abundância.”
//! A tarefa e o ciclo de vida.
//!
//! ```text
//! CREATED → QUEUED → ASSIGNED → EXECUTING → SUBMITTED → VERIFYING → VERIFIED → SETTLED
//!                                                            └──→ REJECTED ──→ QUEUED
//! ```
//!
//! Até o veredito, a tarefa pode ser cancelada ou expirar em qualquer passo
//! (menos em CREATED, que só se cancela). Só essas transições passam; qualquer
//! outra é erro. Resultado recusado não trava a tarefa: ela volta para a fila
//! (spec §18). Cancelar durante a conferência deixa o resultado **sem
//! julgamento**, o que não é o mesmo que recusado. Cada transição vira um
//! evento com horário, para auditoria.

use std::fmt;

use hyurax_codec::Writer;
use hyurax_crypto::{HASH_LEN, sha512};

use crate::trabalho::{ErroDeTrabalho, Especificacao, MetodoDeVerificacao};

/// Domínio do identificador da tarefa.
pub const DOMINIO_TAREFA: &[u8] = dominio!("TAREFA-v1");
/// Domínio da semente que a tarefa dá a cada worker (`ultrax.DOMAIN_TASK_SEED`).
pub const DOMINIO_SEMENTE: &[u8] = dominio!("TASK-v1");

/// Onde a tarefa está.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Estado {
    /// Acabou de ser criada.
    Criada,
    /// Esperando um worker.
    NaFila,
    /// Um worker ficou com ela.
    Atribuida,
    /// Em execução.
    Executando,
    /// O worker entregou o resultado.
    Enviada,
    /// O resultado está sendo conferido.
    Verificando,
    /// O resultado passou na conferência.
    Verificada,
    /// O resultado não passou. A tarefa pode voltar para a fila.
    Recusada,
    /// A contribuição foi contabilizada.
    Liquidada,
    /// Alguém mandou parar.
    Cancelada,
    /// Passou do prazo.
    Expirada,
}

impl Estado {
    /// Nome em inglês, como aparece no registro e no painel.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Criada => "CREATED",
            Self::NaFila => "QUEUED",
            Self::Atribuida => "ASSIGNED",
            Self::Executando => "EXECUTING",
            Self::Enviada => "SUBMITTED",
            Self::Verificando => "VERIFYING",
            Self::Verificada => "VERIFIED",
            Self::Recusada => "REJECTED",
            Self::Liquidada => "SETTLED",
            Self::Cancelada => "CANCELLED",
            Self::Expirada => "EXPIRED",
        }
    }

    /// Estado de onde não se sai.
    pub fn e_final(self) -> bool {
        matches!(self, Self::Liquidada | Self::Cancelada | Self::Expirada)
    }

    /// A tabela de transições. É a regra inteira do ciclo de vida.
    pub fn pode_ir_para(self, para: Self) -> bool {
        use Estado::*;
        matches!(
            (self, para),
            (Criada, NaFila | Cancelada)
                | (NaFila, Atribuida | Cancelada | Expirada)
                // o worker desistiu ou caiu: volta para a fila
                | (Atribuida, Executando | NaFila | Cancelada | Expirada)
                | (Executando, Enviada | NaFila | Cancelada | Expirada)
                | (Enviada, Verificando | Cancelada | Expirada)
                // interromper a conferência não é recusar: o resultado fica sem julgamento
                | (Verificando, Verificada | Recusada | Cancelada | Expirada)
                | (Verificada, Liquidada)
                // submissão inválida não trava a tarefa
                | (Recusada, NaFila | Cancelada | Expirada)
        )
    }
}

impl fmt::Display for Estado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nome())
    }
}

/// Transição fora da tabela.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransicaoInvalida {
    /// De onde.
    pub de: Estado,
    /// Para onde tentaram ir.
    pub para: Estado,
}

impl fmt::Display for TransicaoInvalida {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "transição inválida: {} → {}", self.de, self.para)
    }
}

impl std::error::Error for TransicaoInvalida {}

/// Uma transição registrada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evento {
    /// O estado em que a tarefa entrou.
    pub estado: Estado,
    /// Quando, em milissegundos desde 1970.
    pub instante_ms: u64,
    /// Detalhe legível: quem, quanto, por quê.
    pub nota: String,
}

/// Como a semente da instância é derivada para cada worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Instancia {
    /// Cada worker recebe uma instância própria, amarrada ao `WORKER_ID`.
    /// Copiar o resultado de outro worker não serve. É o modo das tarefas que
    /// têm verificação matemática ou por recomputação.
    PorWorker,
    /// Todos os workers recebem a mesma instância, para comparar resultados.
    /// É o modo da verificação por redundância.
    Compartilhada,
    /// Unidade de um JOB: a semente é a da unidade, `H(DOMINIO_UNIDADE ||
    /// JOB_ID || i)` (ver [`crate::job::semente_da_unidade`]), e vai inteira
    /// na `origem`, então o `TASK_ID` fica preso a ela. O resultado depende
    /// só do JOB e do índice: quem pediu refaz e confere, e dois nós
    /// comparam o mesmo cálculo. Contra cópia entre workers da mesma
    /// unidade, a redundância usa compromisso antes de revelar.
    DeJob,
}

impl Instancia {
    /// Código na codificação do `TASK_ID` e no histórico.
    pub fn codigo(self) -> u8 {
        match self {
            Self::PorWorker => 1,
            Self::Compartilhada => 2,
            Self::DeJob => 3,
        }
    }

    /// O inverso de [`Self::codigo`].
    pub fn de_codigo(codigo: u8) -> Option<Self> {
        [Self::PorWorker, Self::Compartilhada, Self::DeJob].into_iter().find(|i| i.codigo() == codigo)
    }
}

/// Método e forma de instância precisam combinar, senão a verificação não
/// protege nada:
///
/// - instância **por worker** só com conferência própria (a do tipo) ou
///   resultado esperado. Com redundância, cada worker teria uma instância
///   diferente e não haveria o que comparar;
/// - instância **compartilhada** só com redundância ou resultado esperado. Com
///   a conferência do tipo, copiar o resultado de outro worker passaria.
pub fn validar_combinacao(
    esp: &Especificacao,
    metodo: MetodoDeVerificacao,
    instancia: Instancia,
) -> Result<(), ErroDeTrabalho> {
    use MetodoDeVerificacao::{Redundancia, ResultadoEsperado};
    match (instancia, metodo) {
        (_, ResultadoEsperado) => Ok(()),
        (Instancia::Compartilhada, Redundancia) => Ok(()),
        (Instancia::Compartilhada, _) => {
            Err(ErroDeTrabalho::Combinacao("instância compartilhada exige redundância ou resultado esperado"))
        }
        (Instancia::PorWorker, Redundancia) => {
            Err(ErroDeTrabalho::Combinacao("redundância exige a mesma instância para todos os workers"))
        }
        (Instancia::PorWorker, m) if m == esp.tipo().metodo() => Ok(()),
        (Instancia::PorWorker, _) => Err(ErroDeTrabalho::Combinacao("o método não é o do tipo de trabalho")),
        (Instancia::DeJob, Redundancia) => Ok(()),
        (Instancia::DeJob, m) if m == esp.tipo().metodo() => Ok(()),
        (Instancia::DeJob, _) => Err(ErroDeTrabalho::Combinacao("o método não é o do tipo de trabalho")),
    }
}

/// Uma tarefa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tarefa {
    /// `TASK_ID`.
    pub id: [u8; HASH_LEN],
    /// O que ela pede.
    pub especificacao: Especificacao,
    /// Como conferir.
    pub metodo: MetodoDeVerificacao,
    /// Como a instância é derivada.
    pub instancia: Instancia,
    /// De 0 (mais baixa) a 255.
    pub prioridade: u8,
    /// Quando foi criada, em milissegundos desde 1970.
    pub criada_ms: u64,
    /// Prazo, em milissegundos desde 1970.
    pub prazo_ms: u64,
    /// Estado atual.
    pub estado: Estado,
    /// Todas as transições, na ordem.
    pub eventos: Vec<Evento>,
    /// Em [`Instancia::DeJob`], a semente da unidade (que também é a
    /// `origem`); `None` nas outras formas.
    pub semente_fixa: Option<[u8; HASH_LEN]>,
}

impl Tarefa {
    /// Cria a tarefa, já com o evento `CREATED`.
    ///
    /// `origem` é o que torna esta tarefa única (a semente que o criador
    /// escolheu); o `TASK_ID` é o hash de tudo o que a define.
    #[allow(clippy::too_many_arguments)]
    pub fn nova(
        especificacao: Especificacao,
        metodo: MetodoDeVerificacao,
        instancia: Instancia,
        origem: &[u8],
        prioridade: u8,
        criada_ms: u64,
        prazo_ms: u64,
    ) -> Result<Self, ErroDeTrabalho> {
        validar_combinacao(&especificacao, metodo, instancia)?;
        let semente_fixa = match instancia {
            Instancia::DeJob => Some(
                <[u8; HASH_LEN]>::try_from(origem)
                    .map_err(|_| ErroDeTrabalho::Combinacao("unidade de JOB precisa da semente de 64 bytes como origem"))?,
            ),
            Instancia::PorWorker | Instancia::Compartilhada => None,
        };
        let mut w = Writer::new();
        w.raw(DOMINIO_TAREFA);
        especificacao.codificar(&mut w);
        w.u8(metodo.codigo());
        w.u8(instancia.codigo());
        w.var_bytes(origem)?;
        w.u8(prioridade);
        w.u64(criada_ms);
        w.u64(prazo_ms);
        let id = sha512(&w.into_bytes());
        Ok(Self {
            id,
            especificacao,
            metodo,
            instancia,
            prioridade,
            criada_ms,
            prazo_ms,
            estado: Estado::Criada,
            eventos: vec![Evento { estado: Estado::Criada, instante_ms: criada_ms, nota: String::new() }],
            semente_fixa,
        })
    }

    /// Número curto para a tela: os 4 primeiros bytes do id, em decimal.
    pub fn numero(&self) -> u32 {
        u32::from_be_bytes(self.id.first_chunk::<4>().copied().unwrap_or_default()) % 100_000_000
    }

    /// Muda de estado, se a tabela deixar, e registra o evento.
    pub fn avancar(&mut self, para: Estado, instante_ms: u64, nota: impl Into<String>) -> Result<(), TransicaoInvalida> {
        if !self.estado.pode_ir_para(para) {
            return Err(TransicaoInvalida { de: self.estado, para });
        }
        self.estado = para;
        self.eventos.push(Evento { estado: para, instante_ms, nota: nota.into() });
        Ok(())
    }

    /// Passou do prazo e ainda não terminou?
    pub fn vencida(&self, agora_ms: u64) -> bool {
        agora_ms > self.prazo_ms && !self.estado.e_final() && self.estado.pode_ir_para(Estado::Expirada)
    }

    /// A semente da instância que este worker executa.
    ///
    /// Em [`Instancia::PorWorker`], é `H(DOMINIO_SEMENTE || id || worker)`, como
    /// o `Task.seed_for` do gabarito: sem o worker na conta, o segundo copiaria
    /// a resposta do primeiro. Em [`Instancia::Compartilhada`], o worker não
    /// entra, e a cópia se combate com compromisso antes de revelar (ver
    /// [`crate::validador::compromisso`]).
    pub fn semente_para(&self, worker: &[u8]) -> [u8; HASH_LEN] {
        if let Some(semente) = self.semente_fixa {
            return semente;
        }
        let mut dados = DOMINIO_SEMENTE.to_vec();
        dados.extend_from_slice(&self.id);
        if self.instancia == Instancia::PorWorker {
            dados.extend_from_slice(worker);
        }
        sha512(&dados)
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::trabalho::TipoDeTrabalho;

    fn tarefa(instancia: Instancia) -> Tarefa {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap();
        let metodo = match instancia {
            Instancia::PorWorker | Instancia::DeJob => MetodoDeVerificacao::Freivalds,
            Instancia::Compartilhada => MetodoDeVerificacao::Redundancia,
        };
        let origem: &[u8] = if instancia == Instancia::DeJob { &[7u8; 64] } else { b"origem" };
        Tarefa::nova(esp, metodo, instancia, origem, 5, 1_000, 61_000).unwrap()
    }

    #[test]
    fn caminho_completo() {
        let mut t = tarefa(Instancia::PorWorker);
        for (i, e) in [
            Estado::NaFila,
            Estado::Atribuida,
            Estado::Executando,
            Estado::Enviada,
            Estado::Verificando,
            Estado::Verificada,
            Estado::Liquidada,
        ]
        .into_iter()
        .enumerate()
        {
            t.avancar(e, 2_000 + i as u64, "").unwrap();
        }
        assert_eq!(t.eventos.len(), 8);
        assert_eq!(t.eventos[0].estado, Estado::Criada);
        assert!(t.estado.e_final());
        assert!(t.avancar(Estado::NaFila, 9_000, "").is_err(), "de SETTLED não se sai");
    }

    #[test]
    fn atalhos_sao_recusados() {
        let mut t = tarefa(Instancia::PorWorker);
        let erro = t.avancar(Estado::Verificada, 2_000, "").unwrap_err();
        assert_eq!(erro, TransicaoInvalida { de: Estado::Criada, para: Estado::Verificada });
        assert_eq!(t.estado, Estado::Criada, "erro não muda o estado");
        assert_eq!(t.eventos.len(), 1, "erro não registra evento");
        t.avancar(Estado::NaFila, 2_000, "").unwrap();
        assert!(t.avancar(Estado::Enviada, 2_001, "").is_err(), "não entrega sem executar");
    }

    #[test]
    fn recusada_volta_para_a_fila() {
        let mut t = tarefa(Instancia::PorWorker);
        for e in [Estado::NaFila, Estado::Atribuida, Estado::Executando, Estado::Enviada, Estado::Verificando, Estado::Recusada, Estado::NaFila] {
            t.avancar(e, 3_000, "").unwrap();
        }
        assert_eq!(t.estado, Estado::NaFila);
    }

    #[test]
    fn vencimento() {
        let mut t = tarefa(Instancia::PorWorker);
        assert!(!t.vencida(61_000));
        assert!(!t.vencida(61_001), "CREATED não expira, é cancelada");
        t.avancar(Estado::NaFila, 2_000, "").unwrap();
        assert!(t.vencida(61_001));
    }

    #[test]
    fn semente_por_worker_e_compartilhada() {
        let t = tarefa(Instancia::PorWorker);
        assert_ne!(t.semente_para(b"worker A"), t.semente_para(b"worker B"));
        let c = tarefa(Instancia::Compartilhada);
        assert_eq!(c.semente_para(b"worker A"), c.semente_para(b"worker B"));
        assert_ne!(t.id, c.id, "a forma da instância faz parte do id");
    }

    #[test]
    fn unidade_de_job_usa_a_semente_do_job_para_qualquer_worker() {
        let j = tarefa(Instancia::DeJob);
        assert_eq!(j.semente_para(b"worker A"), [7u8; 64]);
        assert_eq!(j.semente_para(b"worker B"), [7u8; 64]);
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap();
        assert!(
            Tarefa::nova(esp, MetodoDeVerificacao::Freivalds, Instancia::DeJob, b"curta", 0, 0, 1).is_err(),
            "a origem de uma unidade de JOB é a semente inteira"
        );
        assert!(Tarefa::nova(esp, MetodoDeVerificacao::Redundancia, Instancia::DeJob, &[1u8; 64], 0, 0, 1).is_ok());
        assert!(Tarefa::nova(esp, MetodoDeVerificacao::Recomputacao, Instancia::DeJob, &[1u8; 64], 0, 0, 1).is_err());
        for i in [Instancia::PorWorker, Instancia::Compartilhada, Instancia::DeJob] {
            assert_eq!(Instancia::de_codigo(i.codigo()), Some(i));
        }
    }

    #[test]
    fn combinacoes_que_nao_protegem_sao_recusadas() {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap();
        let nova = |m, i| Tarefa::nova(esp, m, i, b"o", 0, 0, 1);
        assert!(nova(MetodoDeVerificacao::Freivalds, Instancia::PorWorker).is_ok());
        assert!(nova(MetodoDeVerificacao::ResultadoEsperado, Instancia::Compartilhada).is_ok());
        assert!(nova(MetodoDeVerificacao::Redundancia, Instancia::Compartilhada).is_ok());
        assert!(nova(MetodoDeVerificacao::Freivalds, Instancia::Compartilhada).is_err(), "cópia passaria");
        assert!(nova(MetodoDeVerificacao::Redundancia, Instancia::PorWorker).is_err());
        assert!(nova(MetodoDeVerificacao::Recomputacao, Instancia::PorWorker).is_err(), "não é o método da matriz");
    }

    #[test]
    fn toda_transicao_da_tabela_e_coerente() {
        use Estado::*;
        let todos = [Criada, NaFila, Atribuida, Executando, Enviada, Verificando, Verificada, Recusada, Liquidada, Cancelada, Expirada];
        for de in todos {
            if de.e_final() {
                assert!(todos.iter().all(|&p| !de.pode_ir_para(p)), "{de} é final");
            } else {
                assert!(todos.iter().any(|&p| de.pode_ir_para(p)), "{de} não pode ser beco sem saída");
            }
            assert!(!de.pode_ir_para(de), "{de} não vai para si mesmo");
        }
    }
}
