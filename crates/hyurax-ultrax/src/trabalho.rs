// ✝ Eclesiastes 9:10 — “Tudo quanto te vier à mão para fazer, faze-o conforme as tuas forças.”
//! Os tipos de trabalho: executar, verificar e contar operações.
//!
//! Matriz, mochila e difusão são a tradução de `reference/hyurax/utrax.py`. O
//! resultado sai nos mesmos bytes do Python (inteiros de 64 bits em
//! little-endian, como o `tobytes` do numpy, e a mochila em big-endian), e a
//! conferência é por `vectors/utrax.json`.
//!
//! Tudo em aritmética inteira. Dois workers honestos chegam no mesmo resultado
//! bit a bit em qualquer máquina, que é o que a verificação redundante exige.

use std::fmt;

use hyurax_codec::{CodecError, Reader, Writer};
use hyurax_crypto::{HASH_LEN, sha512, xof};
use hyurax_usefulpow::generate_matrices;

/// Domínio do desafio de Freivalds do mercado (`utrax.DOMAIN_FREIVALDS`).
pub const DOMINIO_FREIVALDS: &[u8] = dominio!("FREIVALDS-v1");
/// Domínio da instância: o mesmo gerador do trabalho útil do consenso.
pub const DOMINIO_INSTANCIA: &[u8] = hyurax_usefulpow::DOMAIN_INSTANCE;
/// Domínio do hash da entrada de uma tarefa.
pub const DOMINIO_ENTRADA: &[u8] = dominio!("ENTRADA-v1");
/// Domínio do hash do resultado.
pub const DOMINIO_RESULTADO: &[u8] = dominio!("RESULTADO-v1");

/// Rodadas de Freivalds. Erro de aceitar resultado falso `<= 2^-80`.
pub const RODADAS_FREIVALDS: usize = 4;
/// Bits de cada coordenada do vetor de desafio.
pub const BITS_FREIVALDS: u32 = 20;
/// Entradas das matrizes ficam em `[0, 1000)`.
pub const MATRIZ_ENTRADA_MAX: u32 = 1000;
/// Maior lado de matriz. Acima disso a conferência inteira poderia estourar.
pub const MATRIZ_LADO_MAX: u32 = 1024;
/// Mais itens que isso não cabem na máscara de 32 bytes.
pub const MOCHILA_ITENS_MAX: u32 = 256;
/// Maior grade da difusão.
pub const DIFUSAO_GRADE_MAX: u32 = 256;
/// Mais passos de difusão que isso é recusado.
pub const DIFUSAO_PASSOS_MAX: u32 = 1024;
/// Ponto fixo da difusão: 1,0 vale `2^20`.
pub const DIFUSAO_ESCALA: i64 = 1 << 20;
/// Valor fixo da borda da grade.
pub const DIFUSAO_BORDA: i64 = DIFUSAO_ESCALA / 2;
/// Taxa de difusão: 1/5.
const DIFUSAO_TAXA_DEN: i64 = 5;

/// Bytes do resultado da mochila: `u64` do valor ótimo e 32 bytes de máscara.
pub const MOCHILA_RESULTADO_LEN: usize = 40;

/// O que o trabalho é, para quem olha a tela.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Categoria {
    /// Álgebra linear, simulação numérica.
    Matematico,
    /// Otimização combinatória.
    Otimizacao,
    /// Inteligência artificial.
    Ia,
    /// Conferir resultados e integridade.
    Verificacao,
    /// Medir e testar a rede.
    Rede,
}

impl Categoria {
    /// Nome em maiúsculas, como aparece no painel.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Matematico => "MATHEMATICAL WORK",
            Self::Otimizacao => "OPTIMIZATION WORK",
            Self::Ia => "AI WORK",
            Self::Verificacao => "VERIFICATION WORK",
            Self::Rede => "NETWORK WORK",
        }
    }
}

/// Como o resultado é conferido. Faz parte da tarefa, não se escolhe depois.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MetodoDeVerificacao {
    /// Prova matemática probabilística, bem mais barata que refazer.
    Freivalds,
    /// Refaz a conta inteira e compara.
    Recomputacao,
    /// A resposta já é conhecida por quem criou a tarefa.
    ResultadoEsperado,
    /// Vários workers independentes; vale a maioria.
    Redundancia,
}

impl MetodoDeVerificacao {
    /// Código na codificação canônica.
    pub fn codigo(self) -> u8 {
        match self {
            Self::Freivalds => 1,
            Self::Recomputacao => 2,
            Self::ResultadoEsperado => 3,
            Self::Redundancia => 4,
        }
    }

    /// O inverso de [`Self::codigo`].
    pub fn de_codigo(codigo: u8) -> Option<Self> {
        match codigo {
            1 => Some(Self::Freivalds),
            2 => Some(Self::Recomputacao),
            3 => Some(Self::ResultadoEsperado),
            4 => Some(Self::Redundancia),
            _ => None,
        }
    }

    /// Nome, como aparece no painel e no registro.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Freivalds => "Freivalds (4 rodadas, erro <= 2^-80)",
            Self::Recomputacao => "recomputação",
            Self::ResultadoEsperado => "resultado esperado",
            Self::Redundancia => "redundância entre workers",
        }
    }
}

/// Os tipos de trabalho que o programa sabe executar.
///
/// Só estes rodam. Não existe caminho para executar código que veio de fora.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TipoDeTrabalho {
    /// `C = A · B`, matrizes `n×n` de inteiros.
    Matriz,
    /// Mochila 0/1; a resposta exigida é o ótimo.
    Mochila,
    /// Difusão de calor numa grade, em ponto fixo.
    Difusao,
}

impl TipoDeTrabalho {
    /// Todos, na ordem do código.
    pub const TODOS: [Self; 3] = [Self::Matriz, Self::Mochila, Self::Difusao];

    /// Código na codificação canônica.
    pub fn codigo(self) -> u8 {
        match self {
            Self::Matriz => 1,
            Self::Mochila => 2,
            Self::Difusao => 3,
        }
    }

    /// O inverso de [`Self::codigo`].
    pub fn de_codigo(codigo: u8) -> Option<Self> {
        Self::TODOS.into_iter().find(|t| t.codigo() == codigo)
    }

    /// Nome curto, igual ao `WorkType` do gabarito.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Matriz => "matrix",
            Self::Mochila => "knapsack",
            Self::Difusao => "diffusion",
        }
    }

    /// Descrição em português.
    pub fn descricao(self) -> &'static str {
        match self {
            Self::Matriz => "multiplicação de matrizes",
            Self::Mochila => "otimização da mochila",
            Self::Difusao => "simulação de difusão de calor",
        }
    }

    /// Categoria para o painel.
    pub fn categoria(self) -> Categoria {
        match self {
            Self::Matriz | Self::Difusao => Categoria::Matematico,
            Self::Mochila => Categoria::Otimizacao,
        }
    }

    /// Método de verificação do tipo.
    pub fn metodo(self) -> MetodoDeVerificacao {
        match self {
            Self::Matriz => MetodoDeVerificacao::Freivalds,
            Self::Mochila | Self::Difusao => MetodoDeVerificacao::Recomputacao,
        }
    }
}

/// Erro ao montar, codificar ou executar um trabalho.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroDeTrabalho {
    /// Tamanho fora da faixa do tipo.
    Tamanho {
        /// O tipo.
        tipo: TipoDeTrabalho,
        /// O tamanho pedido.
        valor: u32,
        /// O menor aceito.
        minimo: u32,
        /// O maior aceito.
        maximo: u32,
    },
    /// Passos fora da faixa, ou passos num tipo que não usa passos.
    Passos(u32),
    /// Código de tipo desconhecido.
    TipoDesconhecido(u8),
    /// Codificação inválida.
    Codec(CodecError),
    /// Quem pediu a execução mandou parar.
    Cancelado,
    /// Método de verificação e forma de instância que não combinam.
    Combinacao(&'static str),
}

impl fmt::Display for ErroDeTrabalho {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tamanho { tipo, valor, minimo, maximo } => {
                write!(f, "tamanho {valor} fora da faixa de {} ({minimo} a {maximo})", tipo.nome())
            }
            Self::Passos(p) => write!(f, "passos fora da faixa: {p}"),
            Self::TipoDesconhecido(c) => write!(f, "tipo de trabalho desconhecido: {c}"),
            Self::Codec(e) => write!(f, "{e}"),
            Self::Cancelado => write!(f, "execução cancelada"),
            Self::Combinacao(motivo) => write!(f, "combinação inválida: {motivo}"),
        }
    }
}

impl std::error::Error for ErroDeTrabalho {}

impl From<CodecError> for ErroDeTrabalho {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

/// Por que um resultado foi recusado. O texto é a evidência.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recusa(pub String);

impl fmt::Display for Recusa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// O que uma tarefa pede: tipo e parâmetros, já validados.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Especificacao {
    tipo: TipoDeTrabalho,
    tamanho: u32,
    passos: u32,
}

impl Especificacao {
    /// Monta e valida. `passos` só vale para a difusão; nos outros tipos é 0.
    pub fn nova(tipo: TipoDeTrabalho, tamanho: u32, passos: u32) -> Result<Self, ErroDeTrabalho> {
        let (minimo, maximo) = match tipo {
            TipoDeTrabalho::Matriz => (1, MATRIZ_LADO_MAX),
            TipoDeTrabalho::Mochila => (1, MOCHILA_ITENS_MAX),
            TipoDeTrabalho::Difusao => (2, DIFUSAO_GRADE_MAX),
        };
        if !(minimo..=maximo).contains(&tamanho) {
            return Err(ErroDeTrabalho::Tamanho { tipo, valor: tamanho, minimo, maximo });
        }
        let passos_ok = match tipo {
            TipoDeTrabalho::Difusao => (1..=DIFUSAO_PASSOS_MAX).contains(&passos),
            TipoDeTrabalho::Matriz | TipoDeTrabalho::Mochila => passos == 0,
        };
        if !passos_ok {
            return Err(ErroDeTrabalho::Passos(passos));
        }
        Ok(Self { tipo, tamanho, passos })
    }

    /// O tipo.
    pub fn tipo(&self) -> TipoDeTrabalho {
        self.tipo
    }

    /// Lado da matriz, itens da mochila ou lado da grade.
    pub fn tamanho(&self) -> u32 {
        self.tamanho
    }

    /// Passos da difusão; 0 nos outros tipos.
    pub fn passos(&self) -> u32 {
        self.passos
    }

    /// `u8 tipo || u32 tamanho || u32 passos`.
    pub fn codificar(&self, w: &mut Writer) {
        w.u8(self.tipo.codigo());
        w.u32(self.tamanho);
        w.u32(self.passos);
    }

    /// O inverso de [`Self::codificar`], validando de novo.
    pub fn decodificar(r: &mut Reader<'_>) -> Result<Self, ErroDeTrabalho> {
        let codigo = r.u8()?;
        let tipo = TipoDeTrabalho::de_codigo(codigo).ok_or(ErroDeTrabalho::TipoDesconhecido(codigo))?;
        let tamanho = r.u32()?;
        let passos = r.u32()?;
        Self::nova(tipo, tamanho, passos)
    }

    /// Texto curto para o painel: `1024 × 1024`, `200 itens`, `64 × 64, 500 passos`.
    pub fn resumo(&self) -> String {
        match self.tipo {
            TipoDeTrabalho::Matriz => format!("{0} × {0}", self.tamanho),
            TipoDeTrabalho::Mochila => format!("{} itens", self.tamanho),
            TipoDeTrabalho::Difusao => format!("grade {0} × {0}, {1} passos", self.tamanho, self.passos),
        }
    }

    /// Operações que a execução faz, pelo modelo de custo do tipo, quando não
    /// dependem da instância. A mochila depende da capacidade sorteada; ali
    /// vale o que [`executar`] devolve.
    ///
    /// - matriz: `n³` multiplicações com soma;
    /// - difusão: `g² · passos` atualizações de célula.
    pub fn operacoes_fixas(&self) -> Option<u64> {
        let t = u64::from(self.tamanho);
        match self.tipo {
            TipoDeTrabalho::Matriz => Some(t.saturating_mul(t).saturating_mul(t)),
            TipoDeTrabalho::Difusao => Some(t.saturating_mul(t).saturating_mul(u64::from(self.passos))),
            TipoDeTrabalho::Mochila => None,
        }
    }

    /// Teto de operações, para o gerenciador de recursos estimar antes de rodar.
    pub fn operacoes_maximas(&self) -> u64 {
        self.operacoes_fixas().unwrap_or_else(|| {
            // capacidade = soma dos pesos / 2, com peso <= 99
            let n = u64::from(self.tamanho);
            n.saturating_mul((n.saturating_mul(99) / 2).saturating_add(1))
        })
    }

    /// Memória no pico, em bytes: execução e verificação da mesma tarefa, com
    /// o resultado guardado entre as duas. É o que o gerenciador de recursos
    /// reserva antes de começar.
    pub fn memoria_bytes(&self) -> u64 {
        let t = u64::from(self.tamanho);
        let quadrado = t.saturating_mul(t);
        match self.tipo {
            // resultado em bytes (8), e na verificação: C em u64 (8), A e B em
            // u32 (8) e em u64 (16), e o XOF que gera A e B (8)
            TipoDeTrabalho::Matriz => quadrado.saturating_mul(8 + 8 + 8 + 16 + 8),
            // tabela de escolhas (1 byte por célula) e a linha da programação
            // dinâmica, com folga para a verificação refazer a linha
            TipoDeTrabalho::Mochila => self.operacoes_maximas().saturating_mul(1 + 1),
            // resultado (8) e, na recomputação, duas grades (16) e o resultado refeito (8)
            TipoDeTrabalho::Difusao => quadrado.saturating_mul(8 + 16 + 8 + 16),
        }
    }
}

/// O que sai de uma execução.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Execucao {
    /// O resultado, nos mesmos bytes do gabarito.
    pub resultado: Vec<u8>,
    /// Operações pelo modelo de custo declarado do tipo. Na mochila, o modelo
    /// conta `n · (C+1)` células, mesmo as que um item pesado pula.
    pub operacoes: u64,
}

/// `INPUT_HASH`: identifica a entrada da tarefa.
///
/// A entrada é função determinística de especificação e semente, então o hash
/// das duas identifica a entrada sem montar os 8 MB de uma matriz 1024 × 1024
/// só para isso. Tarefa com dados de fora, quando existir, usa o hash dos
/// dados.
pub fn hash_da_entrada(esp: &Especificacao, semente: &[u8]) -> Result<[u8; HASH_LEN], ErroDeTrabalho> {
    let mut w = Writer::new();
    w.raw(DOMINIO_ENTRADA);
    esp.codificar(&mut w);
    w.var_bytes(semente)?;
    Ok(sha512(&w.into_bytes()))
}

/// `RESULT_HASH`.
pub fn hash_do_resultado(resultado: &[u8]) -> [u8; HASH_LEN] {
    let mut dados = DOMINIO_RESULTADO.to_vec();
    dados.extend_from_slice(resultado);
    sha512(&dados)
}

/// Inteiros determinísticos em `[0, modulo)`, 4 bytes big-endian cada
/// (`utrax._ints_from_seed`).
fn inteiros(semente: &[u8], quantos: usize, modulo: u32, dominio: &[u8]) -> Vec<u32> {
    xof(semente, quantos.saturating_mul(4), dominio)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| u32::from_be_bytes(c).checked_rem(modulo).unwrap_or(0))
        .collect()
}

/// Executa o trabalho. Custo: o modelo de [`Especificacao::operacoes_fixas`].
///
/// `continuar` recebe as operações feitas desde a última chamada, e é chamada
/// em pedaços pequenos (uma linha da matriz, um item da mochila, um passo da
/// difusão). Devolver `false` cancela. É por ali que o worker mostra progresso,
/// aplica o limite de CPU e respeita o botão de parar.
pub fn executar(
    esp: &Especificacao,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Execucao, ErroDeTrabalho> {
    match esp.tipo {
        TipoDeTrabalho::Matriz => executar_matriz(esp.tamanho, semente, continuar),
        TipoDeTrabalho::Mochila => executar_mochila(esp.tamanho, semente, continuar),
        TipoDeTrabalho::Difusao => executar_difusao(esp.tamanho, esp.passos, semente, continuar),
    }
}

/// Confere um resultado. `Err` traz o motivo, que é a evidência da recusa.
pub fn verificar(esp: &Especificacao, semente: &[u8], resultado: &[u8]) -> Result<(), Recusa> {
    verificar_controlado(esp, semente, resultado, &mut |_| true).unwrap_or_else(|e| Err(Recusa(e.to_string())))
}

/// Confere um resultado com o mesmo controle da execução: `continuar` recebe
/// as operações feitas e pode interromper.
///
/// Os dois níveis de `Result` separam coisas que não podem se misturar:
/// `Err(ErroDeTrabalho::Cancelado)` quer dizer que **ninguém julgou** o
/// resultado (mandaram parar); `Ok(Err(recusa))` quer dizer que o resultado foi
/// julgado e **está errado**. Tratar a interrupção como recusa puniria um
/// worker honesto.
pub fn verificar_controlado(
    esp: &Especificacao,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    match esp.tipo {
        TipoDeTrabalho::Matriz => verificar_matriz(esp.tamanho, semente, resultado, continuar),
        TipoDeTrabalho::Mochila => verificar_mochila(esp.tamanho, semente, resultado, continuar),
        TipoDeTrabalho::Difusao => verificar_difusao(esp.tamanho, esp.passos, semente, resultado, continuar),
    }
}

/// Operações que a verificação custa. A matriz é a única com prova curta.
pub fn operacoes_de_verificacao(esp: &Especificacao) -> u64 {
    match esp.tipo {
        TipoDeTrabalho::Matriz => {
            let t = u64::from(esp.tamanho);
            // três produtos matriz-vetor por rodada
            t.saturating_mul(t).saturating_mul(3).saturating_mul(RODADAS_FREIVALDS as u64)
        }
        TipoDeTrabalho::Mochila | TipoDeTrabalho::Difusao => esp.operacoes_maximas(),
    }
}

// ---------------------------------------------------------------------------
// Matriz
// ---------------------------------------------------------------------------

fn matrizes(n: u32, semente: &[u8]) -> Result<(Vec<u32>, Vec<u32>), ErroDeTrabalho> {
    generate_matrices(semente, n).map_err(|_| ErroDeTrabalho::Tamanho {
        tipo: TipoDeTrabalho::Matriz,
        valor: n,
        minimo: 1,
        maximo: MATRIZ_LADO_MAX,
    })
}

/// `C = A · B`, ordem i-k-j (a da memória). Cada entrada é soma de no máximo
/// 1024 produtos de 999·999, abaixo de 2^30: cabe folgado em `u64`.
fn executar_matriz(
    n: u32,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Execucao, ErroDeTrabalho> {
    let (a, b) = matrizes(n, semente)?;
    let lado = n as usize;
    let por_linha = u64::from(n).saturating_mul(u64::from(n));
    let mut acumulado = vec![0u64; lado];
    let mut c = Vec::with_capacity(lado.saturating_mul(lado).saturating_mul(8));
    for (k, linha_a) in a.chunks_exact(lado).enumerate() {
        acumulado.fill(0);
        for (&aik, linha_b) in linha_a.iter().zip(b.chunks_exact(lado)) {
            let aik = u64::from(aik);
            for (soma, &bkj) in acumulado.iter_mut().zip(linha_b) {
                *soma = soma.wrapping_add(aik.wrapping_mul(u64::from(bkj)));
            }
        }
        for &v in &acumulado {
            c.extend_from_slice(&i64::try_from(v).unwrap_or(i64::MAX).to_le_bytes());
        }
        // o último pedaço conta as operações, mas não cancela: o trabalho já acabou
        if !continuar(por_linha) && k.saturating_add(1) < lado {
            return Err(ErroDeTrabalho::Cancelado);
        }
    }
    Ok(Execucao { resultado: c, operacoes: por_linha.saturating_mul(u64::from(n)) })
}

/// `M · r`, com `M` linha a linha. Com as faixas conferidas antes, cada soma
/// fica abaixo de 2^60 (resultado <= n·998², r < 2^20, n <= 1024).
fn aplicar(m: &[u64], r: &[u64], n: usize) -> Vec<u64> {
    m.chunks_exact(n)
        .map(|linha| linha.iter().zip(r).map(|(&x, &y)| x.wrapping_mul(y)).fold(0u64, u64::wrapping_add))
        .collect()
}

fn verificar_matriz(
    n: u32,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    let lado = n as usize;
    if Some(resultado.len()) != lado.checked_mul(lado).and_then(|q| q.checked_mul(8)) {
        return Ok(Err(Recusa("resultado com tamanho que não fecha n·n".into())));
    }
    // Faixa antes de qualquer conta (spec §18): sem ela, entradas perto de
    // 2^63 fariam a conferência estourar e dar a volta em silêncio.
    let maior = u64::from(MATRIZ_ENTRADA_MAX - 1);
    let limite = u64::from(n).saturating_mul(maior.saturating_mul(maior));
    let mut c = Vec::with_capacity(lado.saturating_mul(lado));
    for bytes in resultado.as_chunks::<8>().0 {
        let v = i64::from_le_bytes(*bytes);
        match u64::try_from(v) {
            Ok(v) if v <= limite => c.push(v),
            _ => return Ok(Err(Recusa("resultado fora da faixa possível de um produto honesto".into()))),
        }
    }
    let (a, b) = matrizes(n, semente)?;
    let a: Vec<u64> = a.into_iter().map(u64::from).collect();
    let b: Vec<u64> = b.into_iter().map(u64::from).collect();

    // O desafio depende do próprio resultado alegado: não dá para procurar um
    // C errado que passe em vetores fixos.
    let mut entrada = semente.to_vec();
    entrada.extend_from_slice(&n.to_be_bytes());
    entrada.extend_from_slice(resultado);
    let valores = inteiros(&sha512(&entrada), RODADAS_FREIVALDS.saturating_mul(lado), 1 << BITS_FREIVALDS, DOMINIO_FREIVALDS);
    let por_rodada = u64::from(n).saturating_mul(u64::from(n)).saturating_mul(3);
    for (rodada, r) in (1u32..).zip(valores.chunks_exact(lado)) {
        let r: Vec<u64> = r.iter().copied().map(u64::from).collect();
        if aplicar(&a, &aplicar(&b, &r, lado), lado) != aplicar(&c, &r, lado) {
            return Ok(Err(Recusa(format!("o resultado não confere na rodada {rodada} de Freivalds"))));
        }
        // depois da última rodada o julgamento está feito: parar agora não o desfaz
        if !continuar(por_rodada) && (rodada as usize) < RODADAS_FREIVALDS {
            return Err(ErroDeTrabalho::Cancelado);
        }
    }
    Ok(Ok(()))
}

// ---------------------------------------------------------------------------
// Mochila
// ---------------------------------------------------------------------------

/// Uma instância da mochila (`utrax.generate_knapsack`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanciaMochila {
    /// Pesos, de 1 a 99.
    pub pesos: Vec<u32>,
    /// Valores, de 1 a 99.
    pub valores: Vec<u32>,
    /// Metade da soma dos pesos, arredondada para baixo.
    pub capacidade: u32,
}

/// Gera a instância da mochila a partir da semente.
pub fn instancia_mochila(itens: u32, semente: &[u8]) -> Result<InstanciaMochila, ErroDeTrabalho> {
    Especificacao::nova(TipoDeTrabalho::Mochila, itens, 0)?;
    let n = itens as usize;
    let gerar = |sufixo: u8| {
        let mut s = semente.to_vec();
        s.push(sufixo);
        // 0..=98 somado de 1: fica em 1..=99
        inteiros(&s, n, 99, DOMINIO_INSTANCIA).into_iter().map(|v| v.saturating_add(1)).collect::<Vec<_>>()
    };
    let pesos = gerar(b'w');
    let valores = gerar(b'v');
    let soma = pesos.iter().fold(0u32, |s, &p| s.saturating_add(p));
    Ok(InstanciaMochila { pesos, valores, capacidade: soma / 2 })
}

/// Programação dinâmica exata. Devolve o valor ótimo e, se pedida, a tabela
/// de escolhas para reconstruir a máscara.
///
/// Igual ao gabarito: um item só entra numa célula quando **melhora**
/// estritamente o valor, e a reconstrução vai do último item para o primeiro.
/// Isso escolhe uma máscara ótima específica; outras máscaras ótimas também
/// passam na verificação.
fn programacao_dinamica(
    inst: &InstanciaMochila,
    com_escolhas: bool,
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<(u64, Vec<Vec<bool>>), ErroDeTrabalho> {
    let cap = inst.capacidade as usize;
    let mut melhor = vec![0u64; cap.saturating_add(1)];
    let mut escolhas = Vec::new();
    let itens = inst.pesos.len();
    for (k, (&peso, &valor)) in inst.pesos.iter().zip(&inst.valores).enumerate() {
        let peso = peso as usize;
        let mut linha = if com_escolhas { vec![false; cap.saturating_add(1)] } else { Vec::new() };
        if peso <= cap {
            // de trás para frente: `melhor[j - peso]` ainda é o valor sem este item
            for j in (peso..=cap).rev() {
                let sem = melhor.get(j).copied().unwrap_or(0);
                let com = melhor.get(j.wrapping_sub(peso)).copied().unwrap_or(0).saturating_add(u64::from(valor));
                if com > sem {
                    if let Some(celula) = melhor.get_mut(j) {
                        *celula = com;
                    }
                    if let Some(marca) = linha.get_mut(j) {
                        *marca = true;
                    }
                }
            }
        }
        escolhas.push(linha);
        if !continuar((cap as u64).saturating_add(1)) && k.saturating_add(1) < itens {
            return Err(ErroDeTrabalho::Cancelado);
        }
    }
    Ok((melhor.get(cap).copied().unwrap_or(0), escolhas))
}

fn executar_mochila(
    itens: u32,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Execucao, ErroDeTrabalho> {
    let inst = instancia_mochila(itens, semente)?;
    let (otimo, escolhas) = programacao_dinamica(&inst, true, continuar)?;
    let mut mascara = [0u8; 32];
    let mut resta = inst.capacidade as usize;
    for (i, linha) in escolhas.iter().enumerate().rev() {
        if linha.get(resta).copied().unwrap_or(false) {
            // bit i da máscara, com o inteiro em big-endian de 32 bytes
            if let Some(byte) = mascara.get_mut(31usize.wrapping_sub(i / 8)) {
                *byte |= 1 << (i % 8);
            }
            resta = resta.saturating_sub(inst.pesos.get(i).copied().unwrap_or(0) as usize);
        }
    }
    let mut resultado = otimo.to_be_bytes().to_vec();
    resultado.extend_from_slice(&mascara);
    let operacoes = u64::from(itens).saturating_mul(u64::from(inst.capacidade).saturating_add(1));
    Ok(Execucao { resultado, operacoes })
}

/// Confere que o resultado é o **ótimo**, não só viável. Aceitar qualquer
/// solução viável deixava 40 bytes de zeros passarem como trabalho.
fn verificar_mochila(
    itens: u32,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    let Some((valor_bytes, mascara)) = resultado.split_first_chunk::<8>().filter(|(_, resto)| resto.len() == 32) else {
        return Ok(Err(Recusa("resultado da mochila precisa de 40 bytes".into())));
    };
    let declarado = u64::from_be_bytes(*valor_bytes);
    let tem = |i: usize| mascara.get(31usize.wrapping_sub(i / 8)).is_some_and(|b| b & (1 << (i % 8)) != 0);
    let n = itens as usize;
    if (n..256).any(tem) {
        return Ok(Err(Recusa("máscara com item que não existe".into())));
    }
    let inst = instancia_mochila(itens, semente)?;
    let (mut peso, mut valor) = (0u64, 0u64);
    for i in (0..n).filter(|&i| tem(i)) {
        peso = peso.saturating_add(u64::from(inst.pesos.get(i).copied().unwrap_or(0)));
        valor = valor.saturating_add(u64::from(inst.valores.get(i).copied().unwrap_or(0)));
    }
    if peso > u64::from(inst.capacidade) {
        return Ok(Err(Recusa(format!("a escolha pesa {peso} e a capacidade é {}", inst.capacidade))));
    }
    if valor != declarado {
        return Ok(Err(Recusa(format!("valor declarado {declarado} difere da soma da escolha {valor}"))));
    }
    let (otimo, _) = programacao_dinamica(&inst, false, continuar)?;
    if declarado != otimo {
        return Ok(Err(Recusa(format!("a escolha vale {declarado}, mas o ótimo é {otimo}"))));
    }
    Ok(Ok(()))
}

// ---------------------------------------------------------------------------
// Difusão
// ---------------------------------------------------------------------------

/// Difusão em ponto fixo inteiro (`utrax.diffusion_work`).
///
/// A cada passo, cada célula recebe `(soma dos 4 vizinhos − 4·ela) / 5`, com a
/// divisão truncando para o zero (o `/` do Rust já faz isso; o gabarito
/// precisou emular). Os vizinhos dão a volta nas bordas, como o `np.roll`, e
/// depois a borda volta ao valor fixo. É uma média, então os valores ficam na
/// faixa inicial `[0, 2^20]` e nenhuma conta chega perto de estourar.
fn executar_difusao(
    lado: u32,
    passos: u32,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Execucao, ErroDeTrabalho> {
    Especificacao::nova(TipoDeTrabalho::Difusao, lado, passos)?;
    let g = lado as usize;
    let celulas = g.saturating_mul(g);
    let mut grade: Vec<i64> =
        inteiros(semente, celulas, 1 << 20, DOMINIO_INSTANCIA).into_iter().map(i64::from).collect();
    let mut nova = vec![0i64; celulas];
    let em = |grade: &[i64], i: usize, j: usize| grade.get(i.saturating_mul(g).saturating_add(j)).copied().unwrap_or(0);
    let ultima = g.saturating_sub(1);
    for passo in 0..passos {
        for i in 0..g {
            let acima = if i == ultima { 0 } else { i.saturating_add(1) };
            let abaixo = if i == 0 { ultima } else { i.saturating_sub(1) };
            for j in 0..g {
                let direita = if j == ultima { 0 } else { j.saturating_add(1) };
                let esquerda = if j == 0 { ultima } else { j.saturating_sub(1) };
                let centro = em(&grade, i, j);
                let vizinhos = em(&grade, acima, j)
                    .wrapping_add(em(&grade, abaixo, j))
                    .wrapping_add(em(&grade, i, direita))
                    .wrapping_add(em(&grade, i, esquerda));
                let laplaciano = vizinhos.wrapping_sub(centro.wrapping_mul(4));
                let valor = if i == 0 || i == ultima || j == 0 || j == ultima {
                    DIFUSAO_BORDA
                } else {
                    centro.wrapping_add(laplaciano.wrapping_div(DIFUSAO_TAXA_DEN))
                };
                if let Some(c) = nova.get_mut(i.saturating_mul(g).saturating_add(j)) {
                    *c = valor;
                }
            }
        }
        std::mem::swap(&mut grade, &mut nova);
        if !continuar(celulas as u64) && passo.saturating_add(1) < passos {
            return Err(ErroDeTrabalho::Cancelado);
        }
    }
    let resultado = grade.iter().flat_map(|v| v.to_le_bytes()).collect();
    Ok(Execucao { resultado, operacoes: (celulas as u64).saturating_mul(u64::from(passos)) })
}

/// Sem prova curta: refaz e compara. O custo está declarado na tarefa.
fn verificar_difusao(
    lado: u32,
    passos: u32,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    let esperado = (lado as usize).saturating_mul(lado as usize).saturating_mul(8);
    // o tamanho primeiro: resultado que nem fecha a grade não merece a recomputação
    if resultado.len() != esperado {
        return Ok(Err(Recusa("resultado com tamanho que não fecha a grade".into())));
    }
    let refeito = executar_difusao(lado, passos, semente, continuar)?;
    Ok(match refeito.resultado.as_chunks::<8>().0.iter().zip(resultado.as_chunks::<8>().0).position(|(a, b)| a != b) {
        None => Ok(()),
        Some(k) => Err(Recusa(format!("a célula {k} difere da recomputação"))),
    })
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

    use super::*;

    const SEMENTE: &[u8] = b"semente de teste";

    fn rodar(esp: &Especificacao) -> Execucao {
        executar(esp, SEMENTE, &mut |_| true).unwrap()
    }

    #[test]
    fn especificacao_recusa_fora_da_faixa() {
        assert!(Especificacao::nova(TipoDeTrabalho::Matriz, 0, 0).is_err());
        assert!(Especificacao::nova(TipoDeTrabalho::Matriz, 1025, 0).is_err());
        assert!(Especificacao::nova(TipoDeTrabalho::Matriz, 8, 3).is_err());
        assert!(Especificacao::nova(TipoDeTrabalho::Mochila, 257, 0).is_err());
        assert!(Especificacao::nova(TipoDeTrabalho::Difusao, 1, 5).is_err());
        assert!(Especificacao::nova(TipoDeTrabalho::Difusao, 8, 0).is_err());
        assert!(Especificacao::nova(TipoDeTrabalho::Difusao, 8, 1025).is_err());
    }

    #[test]
    fn especificacao_ida_e_volta() {
        let esp = Especificacao::nova(TipoDeTrabalho::Difusao, 16, 10).unwrap();
        let mut w = Writer::new();
        esp.codificar(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(Especificacao::decodificar(&mut r).unwrap(), esp);
        r.finish().unwrap();
    }

    #[test]
    fn matriz_honesta_passa_e_adulterada_nao() {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 12, 0).unwrap();
        let exec = rodar(&esp);
        assert_eq!(exec.operacoes, 12 * 12 * 12);
        assert_eq!(verificar(&esp, SEMENTE, &exec.resultado), Ok(()));

        let mut errado = exec.resultado.clone();
        errado[8 * 30] ^= 1;
        let recusa = verificar(&esp, SEMENTE, &errado).unwrap_err();
        assert!(recusa.0.contains("Freivalds"), "{recusa}");

        // entrada negativa, fora da faixa
        let mut negativo = exec.resultado.clone();
        negativo[..8].copy_from_slice(&(-1i64).to_le_bytes());
        assert!(verificar(&esp, SEMENTE, &negativo).unwrap_err().0.contains("faixa"));

        // o resultado de outra semente não serve
        assert!(verificar(&esp, b"outra semente", &exec.resultado).is_err());
        assert!(verificar(&esp, SEMENTE, &exec.resultado[8..]).is_err());
    }

    #[test]
    fn mochila_exige_o_otimo() {
        let esp = Especificacao::nova(TipoDeTrabalho::Mochila, 30, 0).unwrap();
        let exec = rodar(&esp);
        assert_eq!(verificar(&esp, SEMENTE, &exec.resultado), Ok(()));
        assert!(verificar(&esp, SEMENTE, &[0u8; 40]).is_err(), "zeros não são trabalho");

        // tirar um item escolhido deixa a escolha viável, mas não ótima
        let mut pior = exec.resultado.clone();
        let inst = instancia_mochila(30, SEMENTE).unwrap();
        let i = (0..30).find(|&i| pior[8 + 31 - i / 8] & (1 << (i % 8)) != 0).unwrap();
        pior[8 + 31 - i / 8] &= !(1 << (i % 8));
        let valor = u64::from_be_bytes(pior[..8].try_into().unwrap()) - u64::from(inst.valores[i]);
        pior[..8].copy_from_slice(&valor.to_be_bytes());
        assert!(verificar(&esp, SEMENTE, &pior).unwrap_err().0.contains("ótimo"));

        // item que não existe
        let mut fantasma = exec.resultado.clone();
        fantasma[8] |= 0x80;
        assert!(verificar(&esp, SEMENTE, &fantasma).unwrap_err().0.contains("não existe"));
    }

    #[test]
    fn difusao_recomputada() {
        let esp = Especificacao::nova(TipoDeTrabalho::Difusao, 10, 7).unwrap();
        let exec = rodar(&esp);
        assert_eq!(exec.operacoes, 10 * 10 * 7);
        assert_eq!(verificar(&esp, SEMENTE, &exec.resultado), Ok(()));
        let mut errado = exec.resultado.clone();
        errado[8 * 55] ^= 4;
        assert!(verificar(&esp, SEMENTE, &errado).unwrap_err().0.contains("55"));
    }

    #[test]
    fn cancelar_para_no_meio() {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 64, 0).unwrap();
        let mut feitas = 0u64;
        let r = executar(&esp, SEMENTE, &mut |ops| {
            feitas += ops;
            feitas < 10 * 64 * 64
        });
        assert_eq!(r, Err(ErroDeTrabalho::Cancelado));
        assert_eq!(feitas, 10 * 64 * 64);
    }

    #[test]
    fn verificacao_interrompida_nao_e_recusa() {
        for esp in [
            Especificacao::nova(TipoDeTrabalho::Matriz, 16, 0).unwrap(),
            Especificacao::nova(TipoDeTrabalho::Mochila, 40, 0).unwrap(),
            Especificacao::nova(TipoDeTrabalho::Difusao, 12, 6).unwrap(),
        ] {
            let exec = rodar(&esp);
            let mut chamadas = 0u64;
            let r = verificar_controlado(&esp, SEMENTE, &exec.resultado, &mut |_| {
                chamadas += 1;
                false
            });
            assert_eq!(r, Err(ErroDeTrabalho::Cancelado), "{}", esp.resumo());
            assert_eq!(chamadas, 1);
            // sem interromper, o mesmo resultado passa
            assert_eq!(verificar_controlado(&esp, SEMENTE, &exec.resultado, &mut |_| true), Ok(Ok(())));
        }
    }

    #[test]
    fn verificacao_da_matriz_conta_as_operacoes() {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 20, 0).unwrap();
        let exec = rodar(&esp);
        let mut feitas = 0u64;
        verificar_controlado(&esp, SEMENTE, &exec.resultado, &mut |ops| {
            feitas += ops;
            true
        })
        .unwrap()
        .unwrap();
        assert_eq!(feitas, operacoes_de_verificacao(&esp));
    }

    #[test]
    fn hashes_separam_entrada_e_resultado() {
        let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 4, 0).unwrap();
        assert_ne!(hash_da_entrada(&esp, b"a").unwrap(), hash_da_entrada(&esp, b"b").unwrap());
        let outra = Especificacao::nova(TipoDeTrabalho::Matriz, 5, 0).unwrap();
        assert_ne!(hash_da_entrada(&esp, b"a").unwrap(), hash_da_entrada(&outra, b"a").unwrap());
        assert_ne!(hash_do_resultado(b"x"), hash_do_resultado(b"y"));
    }
}
