//! Os tipos que entram no diário: o que o cliente pede, o hardware do nó, os
//! limites, os papéis, a configuração da plataforma.

use std::collections::{BTreeMap, BTreeSet};

use hyurax_ultrax::job::{Dominio, EspecificacaoDeJob, Nivel, PedidoDeJob};
use hyurax_ultrax::trabalho::{Especificacao, TipoDeTrabalho};
use serde::{Deserialize, Serialize};

use crate::dinheiro::Moeda;
use crate::regras::Tabela;

/// Número de usuário, nó, tarefa, pagamento...
pub type Id = u64;

/// Os papéis de um usuário. Um usuário pode ser cliente e fornecedor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Papeis {
    /// Compra computação.
    pub cliente: bool,
    /// Fornece computação.
    pub fornecedor: bool,
    /// Administra a plataforma.
    pub admin: bool,
}

impl Papeis {
    /// Os nomes, como a API mostra.
    pub fn nomes(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.cliente {
            v.push("CUSTOMER");
        }
        if self.fornecedor {
            v.push("PROVIDER");
        }
        if self.admin {
            v.push("ADMIN");
        }
        v
    }
}

/// O que uma chave de API pode fazer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Escopo {
    /// Ler tarefas e resultados.
    #[serde(rename = "tarefas:ler")]
    TarefasLer,
    /// Criar e cancelar tarefas.
    #[serde(rename = "tarefas:criar")]
    TarefasCriar,
    /// Ler o saldo e o extrato.
    #[serde(rename = "saldo:ler")]
    SaldoLer,
}

impl Escopo {
    /// Todos.
    pub const TODOS: [Self; 3] = [Self::TarefasLer, Self::TarefasCriar, Self::SaldoLer];

    /// Lê o nome.
    pub fn ler(texto: &str) -> Option<Self> {
        match texto {
            "tarefas:ler" => Some(Self::TarefasLer),
            "tarefas:criar" => Some(Self::TarefasCriar),
            "saldo:ler" => Some(Self::SaldoLer),
            _ => None,
        }
    }

    /// O nome.
    pub fn nome(self) -> &'static str {
        match self {
            Self::TarefasLer => "tarefas:ler",
            Self::TarefasCriar => "tarefas:criar",
            Self::SaldoLer => "saldo:ler",
        }
    }
}

/// O hardware que o agente relata. Sem nome de máquina, usuário ou IP local:
/// nada que identifique a pessoa (seção 31 do documento da V1).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hardware {
    /// Modelo da CPU.
    pub cpu_modelo: String,
    /// Núcleos lógicos.
    pub nucleos: u32,
    /// Memória total, em MiB.
    pub ram_mib: u64,
    /// Modelo da GPU (vazio se não houver).
    pub gpu_modelo: String,
    /// Memória da GPU, em MiB.
    pub gpu_vram_mib: u64,
    /// Disco livre na pasta do programa, em MiB.
    pub disco_livre_mib: u64,
    /// Sistema operacional.
    pub so: String,
}

impl Hardware {
    /// Corta textos longos e tira caracteres de controle.
    pub fn limpo(mut self) -> Self {
        self.cpu_modelo = texto_curto(&self.cpu_modelo, 120);
        self.gpu_modelo = texto_curto(&self.gpu_modelo, 120);
        self.so = texto_curto(&self.so, 80);
        self.nucleos = self.nucleos.min(4096);
        self
    }
}

/// O que o fornecedor deixa a plataforma usar no nó.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limites {
    /// Linhas de CPU (núcleos) para a plataforma.
    pub linhas: u32,
    /// Memória para a plataforma, em MiB.
    pub ram_mib: u64,
    /// Aceita trabalho de GPU.
    pub gpu: bool,
}

impl Default for Limites {
    fn default() -> Self {
        Self { linhas: 2, ram_mib: 2048, gpu: false }
    }
}

/// O que o cliente pede (o formato de workload da seção 15 do documento da V1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PedidoDeTarefa {
    /// O motor: `matrix`, `knapsack`, `diffusion`, `ai-training`,
    /// `population-genetics`, `crop-breeding`, `routing` ou `molecular-screening`.
    pub tipo: String,
    /// O tamanho do problema (ver o catálogo).
    pub tamanho: u32,
    /// Passos, gerações ou passadas (0 onde não se aplica).
    #[serde(default)]
    pub passos: u32,
    /// Parâmetros extras do motor.
    #[serde(default)]
    pub parametros: Vec<u32>,
    /// Quantas unidades (réplicas, partidas ou faixas).
    pub unidades: u64,
    /// Núcleos em paralelo: unidades rodando ao mesmo tempo (0 = 4).
    #[serde(default)]
    pub paralelo: u32,
    /// Rodar na GPU (só a multiplicação de matrizes).
    #[serde(default)]
    pub gpu: bool,
    /// Prazo da tarefa inteira, em segundos (0 = um dia).
    #[serde(default)]
    pub prazo_s: u64,
    /// Cópias independentes por unidade, em nós de donos diferentes (0 = 1).
    #[serde(default)]
    pub copias: u8,
    /// O que se quer descobrir.
    #[serde(default)]
    pub descricao: String,
    /// Região exigida (vazia: qualquer uma).
    #[serde(default)]
    pub regiao: String,
}

impl PedidoDeTarefa {
    /// O motor pedido.
    pub fn motor(&self) -> Option<TipoDeTrabalho> {
        TipoDeTrabalho::TODOS.into_iter().find(|t| t.nome() == self.tipo)
    }

    /// A especificação-modelo e o JOB do ULTRAX, validados pelo próprio motor.
    pub fn job(&self) -> Result<(Especificacao, EspecificacaoDeJob), String> {
        let tipo = self.motor().ok_or_else(|| format!("motor desconhecido: {}", self.tipo))?;
        let esp = Especificacao::nova_com(tipo, self.tamanho, self.passos, &self.parametros).map_err(|e| e.to_string())?;
        let copias = self.copias.max(1);
        let job = EspecificacaoDeJob::nova(PedidoDeJob {
            dominio: dominio_do_motor(tipo),
            modelo: esp,
            unidades: self.unidades,
            nivel: if copias >= 2 { Nivel::Concordancia } else { Nivel::Integridade },
            redundancia: copias,
            prazo_s: self.prazo_s,
            orcamento_milicreditos: 0,
            descricao: self.descricao.clone(),
        })
        .map_err(|e| e.to_string())?;
        Ok((esp, job))
    }
}

/// O domínio de pesquisa de cada motor.
pub fn dominio_do_motor(tipo: TipoDeTrabalho) -> Dominio {
    match tipo {
        TipoDeTrabalho::Triagem => Dominio::Molecular,
        TipoDeTrabalho::Genetica => Dominio::Genetica,
        TipoDeTrabalho::Melhoramento => Dominio::Agricultura,
        TipoDeTrabalho::Rotas => Dominio::Logistica,
        TipoDeTrabalho::Mochila => Dominio::Economia,
        TipoDeTrabalho::Ia => Dominio::Ia,
        TipoDeTrabalho::Matriz | TipoDeTrabalho::Difusao => Dominio::Matematica,
    }
}

/// Os dados do Pix para o pagamento manual (a primeira forma de pagamento,
/// antes da integração com um provedor).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pix {
    /// A chave Pix da empresa.
    pub chave: String,
    /// O titular, como aparece no app do banco.
    pub titular: String,
}

/// Os tetos da política de execução (seção 16 do documento da V1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Politica {
    /// Unidades por tarefa.
    pub unidades_max: u64,
    /// Memória de uma unidade, em bytes.
    pub memoria_max: u64,
    /// Operações de uma unidade.
    pub operacoes_max: u64,
    /// Prazo de uma tarefa, em segundos.
    pub prazo_max_s: u64,
    /// Núcleos em paralelo por tarefa.
    pub paralelo_max: u32,
    /// Cópias por unidade.
    pub copias_max: u8,
    /// Tarefas criadas por cliente por hora.
    pub tarefas_por_hora: u32,
    /// Tarefas em andamento por cliente.
    pub tarefas_ativas_max: u32,
    /// Pagamentos pendentes por cliente.
    pub pagamentos_pendentes_max: u32,
    /// Contas criadas por IP por dia.
    pub cadastros_por_ip_dia: u32,
}

impl Default for Politica {
    fn default() -> Self {
        Self {
            unidades_max: 100_000,
            memoria_max: 2 << 30,
            operacoes_max: 1 << 47,
            prazo_max_s: 7 * 86_400,
            paralelo_max: 256,
            copias_max: 3,
            tarefas_por_hora: 60,
            tarefas_ativas_max: 20,
            pagamentos_pendentes_max: 3,
            cadastros_por_ip_dia: 5,
        }
    }
}

/// A configuração da plataforma. Muda só pelo administrador, e cada mudança
/// fica no diário (nada de comissão fixa no frontend, seção 23).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// Preços por moeda.
    pub tabelas: BTreeMap<Moeda, Tabela>,
    /// Parte do fornecedor, em pontos-base (7000 = 70%).
    pub fornecedor_bp: u32,
    /// Dias de retenção do ganho antes de liberar para saque.
    pub retencao_dias: u32,
    /// Saque mínimo, em centavos, por moeda.
    pub saque_minimo: BTreeMap<Moeda, i64>,
    /// Pagamento mínimo, em centavos, por moeda.
    pub pagamento_minimo: BTreeMap<Moeda, i64>,
    /// Pagamento máximo, em centavos, por moeda.
    pub pagamento_maximo: BTreeMap<Moeda, i64>,
    /// A política de execução.
    pub politica: Politica,
    /// Motores desligados (kill switch por categoria).
    pub motores_desligados: BTreeSet<String>,
    /// Parada de emergência: nada é despachado e tudo que roda é cancelado.
    pub parada_de_emergencia: bool,
    /// Cadastro de contas aberto.
    pub cadastro_aberto: bool,
    /// Pix da empresa para o pagamento manual.
    pub pix: Pix,
    /// Versão mínima do agente (abaixo dela, o nó não recebe trabalho).
    pub versao_minima_agente: String,
    /// Versão dos termos de uso que o cadastro aceita.
    pub termos_versao: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tabelas: BTreeMap::from([(Moeda::Brl, Tabela::REAIS), (Moeda::Usd, Tabela::DOLARES)]),
            fornecedor_bp: 7_000,
            retencao_dias: 7,
            saque_minimo: BTreeMap::from([(Moeda::Brl, 2_000), (Moeda::Usd, 500)]),
            pagamento_minimo: BTreeMap::from([(Moeda::Brl, 1_000), (Moeda::Usd, 200)]),
            pagamento_maximo: BTreeMap::from([(Moeda::Brl, 500_000), (Moeda::Usd, 100_000)]),
            politica: Politica::default(),
            motores_desligados: BTreeSet::new(),
            parada_de_emergencia: false,
            cadastro_aberto: true,
            pix: Pix::default(),
            versao_minima_agente: "1.5.0".into(),
            termos_versao: 1,
        }
    }
}

impl Config {
    /// Confere a configuração inteira antes de aceitar uma mudança.
    pub fn validar(&self) -> Result<(), String> {
        for (m, t) in &self.tabelas {
            t.validar().map_err(|e| format!("{}: {e}", m.codigo()))?;
        }
        if self.fornecedor_bp > crate::regras::BP {
            return Err("parte do fornecedor acima de 100%".into());
        }
        if self.retencao_dias > 90 {
            return Err("retenção acima de 90 dias".into());
        }
        let p = &self.politica;
        if p.unidades_max == 0 || p.unidades_max > hyurax_ultrax::job::UNIDADES_MAX {
            return Err("unidades por tarefa fora da faixa".into());
        }
        if p.memoria_max > crate::regras::MEMORIA_MAX || p.operacoes_max > crate::regras::OPS_MAX {
            return Err("teto de memória ou de operações acima do que a regra de preço aceita".into());
        }
        if p.copias_max == 0 || p.copias_max > 7 || p.paralelo_max == 0 {
            return Err("cópias ou paralelo fora da faixa".into());
        }
        for nome in &self.motores_desligados {
            if !TipoDeTrabalho::TODOS.iter().any(|t| t.nome() == nome) {
                return Err(format!("motor desconhecido: {nome}"));
            }
        }
        if self.pix.chave.len() > 120 || self.pix.titular.len() > 120 {
            return Err("dados do Pix longos demais".into());
        }
        if versao(&self.versao_minima_agente).is_none() {
            return Err("versão mínima do agente inválida (use 1.5.0)".into());
        }
        Ok(())
    }

    /// A tabela de uma moeda.
    pub fn tabela(&self, moeda: Moeda) -> Tabela {
        self.tabelas.get(&moeda).copied().unwrap_or(if moeda == Moeda::Brl { Tabela::REAIS } else { Tabela::DOLARES })
    }
}

/// `1.5.0` → `(1, 5, 0)`.
pub fn versao(texto: &str) -> Option<(u32, u32, u32)> {
    let mut p = texto.trim().trim_start_matches('v').split('.');
    let v = (p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next().unwrap_or("0").parse().ok()?);
    p.next().is_none().then_some(v)
}

/// Texto de uma linha, sem caractere de controle, com no máximo `max` bytes.
pub fn texto_curto(texto: &str, max: usize) -> String {
    let mut s = String::new();
    for c in texto.trim().chars().filter(|c| !c.is_control()) {
        if s.len().saturating_add(c.len_utf8()) > max {
            break;
        }
        s.push(c);
    }
    s
}
