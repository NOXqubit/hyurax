//! O observador: o motor conta o que está calculando, sem mudar o cálculo.
//!
//! Cada motor, num ponto natural do laço (uma linha da matriz, uma geração,
//! uma passada do 2-opt, uma molécula), pergunta [`Observador::quer`]. Se o
//! observador quiser, o motor entrega uma [`Amostra`] com uma **cópia** do
//! estado real naquele instante. O motor nunca lê nada do observador além do
//! "quer": o resultado é o mesmo com ou sem observador, e os testes conferem
//! isso motor por motor, contra os vetores do gabarito.
//!
//! Quando o estado é grande demais para a tela, a amostra leva um recorte
//! regular (um a cada `a_cada` valores, ou uma grade menor) e diz isso: os
//! números que vão são os do cálculo, nunca uma média inventada ou um valor
//! interpolado.
//!
//! É daqui que sai o que a visualização 3D desenha.

use std::fmt::Write as _;

/// Quem recebe as amostras.
pub trait Observador {
    /// O motor pergunta a cada ponto natural do laço. `true`: quer uma
    /// amostra agora. Serve para limitar a frequência sem o motor saber de
    /// relógio.
    fn quer(&mut self) -> bool;
    /// Uma amostra do estado real.
    fn amostra(&mut self, a: Amostra);
}

/// Não observa nada (o caminho normal do cálculo e da conferência).
pub struct Nenhum;

impl Observador for Nenhum {
    fn quer(&mut self) -> bool {
        false
    }
    fn amostra(&mut self, _: Amostra) {}
}

/// Uma amostra do estado real de um motor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Amostra {
    /// `C = A·B`: a linha `linha` de C acabou de ser calculada.
    Matriz {
        /// Lado das matrizes.
        lado: u32,
        /// A linha de C recém-calculada.
        linha: u32,
        /// Valores dessa linha, um a cada `a_cada` colunas.
        valores: Vec<i64>,
        /// Passo do recorte (1 = a linha inteira).
        a_cada: u32,
    },
    /// Mochila: o item `item` acabou de entrar na tabela.
    Mochila {
        /// Itens da instância.
        itens: u32,
        /// Capacidade da mochila.
        capacidade: u32,
        /// O item recém-considerado.
        item: u32,
        /// Peso do item.
        peso: u32,
        /// Valor do item.
        valor: u32,
        /// O melhor valor para cada capacidade, um a cada `a_cada`.
        melhor: Vec<u64>,
        /// Passo do recorte.
        a_cada: u32,
    },
    /// Difusão: a grade depois do passo `passo`.
    Difusao {
        /// Lado da grade.
        lado: u32,
        /// Passo recém-feito (1 = o primeiro).
        passo: u32,
        /// Passos pedidos.
        passos: u32,
        /// A grade, um ponto a cada `a_cada` linhas e colunas.
        grade: Vec<i64>,
        /// Lado da grade recortada.
        lado_amostra: u32,
        /// Passo do recorte.
        a_cada: u32,
    },
    /// Treino de IA: depois do passo `passo`.
    Ia {
        /// Passo recém-feito (1 = o primeiro).
        passo: u32,
        /// Passos pedidos.
        passos: u32,
        /// Perda do lote, em ponto fixo Q12 (a curva do treino).
        perda: u64,
        /// Pesos da camada oculta para a saída (W2), em ponto fixo.
        pesos_saida: Vec<i64>,
    },
    /// Genética: contagens do alelo A depois da geração `geracao`.
    Genetica {
        /// Geração recém-feita (1 = a primeira).
        geracao: u32,
        /// Gerações pedidas.
        geracoes: u32,
        /// Cópias do gene na população (2N).
        copias: u32,
        /// Cópias do alelo A em cada locus.
        contagens: Vec<u32>,
    },
    /// Melhoramento: a população avaliada na geração `geracao`.
    Melhoramento {
        /// Geração (0 = a população inicial).
        geracao: u32,
        /// Gerações pedidas.
        geracoes: u32,
        /// Plantas da população.
        plantas: u32,
        /// (valor genético, fenótipo, selecionada), uma planta a cada `a_cada`.
        pontos: Vec<(i64, i64, bool)>,
        /// Passo do recorte.
        a_cada: u32,
    },
    /// Rotas: a rota depois da passada `passada` do 2-opt (0 = a partida).
    Rotas {
        /// Passada recém-feita.
        passada: u32,
        /// Teto de passadas.
        passos: u32,
        /// Comprimento da rota agora.
        comprimento: u64,
        /// Comprimento da partida.
        inicial: u64,
        /// A ordem das cidades, inteira.
        rota: Vec<u16>,
    },
    /// Triagem: uma molécula avaliada.
    Triagem {
        /// Índice da molécula no catálogo inteiro.
        indice: u32,
        /// Filtros aprovados (bit a bit).
        mascara: u8,
        /// log S previsto, em milésimos.
        previsto_mili: i32,
        /// Nota da triagem.
        nota: u32,
        /// Passou em todos os filtros.
        aprovada: bool,
    },
}

/// Recorte regular: um a cada `a_cada` valores, com no máximo `max`.
pub fn recortar<T: Copy>(v: &[T], max: usize) -> (Vec<T>, u32) {
    let max = max.max(1);
    let a_cada = v.len().div_ceil(max).max(1);
    (v.iter().step_by(a_cada).copied().collect(), u32::try_from(a_cada).unwrap_or(u32::MAX))
}

/// Recorte regular de uma grade quadrada: um ponto a cada `a_cada` linhas e
/// colunas, com no máximo `max_lado` por lado. Devolve (grade, lado, a_cada).
pub fn recortar_grade<T: Copy + Default>(g: &[T], lado: usize, max_lado: usize) -> (Vec<T>, u32, u32) {
    let a_cada = lado.div_ceil(max_lado.max(1)).max(1);
    let indices: Vec<usize> = (0..lado).step_by(a_cada).collect();
    let mut saida = Vec::with_capacity(indices.len().saturating_mul(indices.len()));
    for &i in &indices {
        for &j in &indices {
            saida.push(g.get(i.saturating_mul(lado).saturating_add(j)).copied().unwrap_or_default());
        }
    }
    (saida, u32::try_from(indices.len()).unwrap_or(u32::MAX), u32::try_from(a_cada).unwrap_or(u32::MAX))
}

fn lista<T: std::fmt::Display>(v: &[T]) -> String {
    let mut s = String::from("[");
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{x}");
    }
    s.push(']');
    s
}

impl Amostra {
    /// O tipo de trabalho, como no gabarito (`matrix`, `population-genetics`…).
    pub fn tipo(&self) -> &'static str {
        match self {
            Self::Matriz { .. } => "matrix",
            Self::Mochila { .. } => "knapsack",
            Self::Difusao { .. } => "diffusion",
            Self::Ia { .. } => "ai-training",
            Self::Genetica { .. } => "population-genetics",
            Self::Melhoramento { .. } => "crop-breeding",
            Self::Rotas { .. } => "routing",
            Self::Triagem { .. } => "molecular-screening",
        }
    }

    /// Em JSON (só números e listas de números; nada a escapar).
    pub fn json(&self) -> String {
        let t = self.tipo();
        match self {
            Self::Matriz { lado, linha, valores, a_cada } => {
                format!("{{\"tipo\":\"{t}\",\"lado\":{lado},\"linha\":{linha},\"a_cada\":{a_cada},\"valores\":{}}}", lista(valores))
            }
            Self::Mochila { itens, capacidade, item, peso, valor, melhor, a_cada } => format!(
                "{{\"tipo\":\"{t}\",\"itens\":{itens},\"capacidade\":{capacidade},\"item\":{item},\"peso\":{peso},\"valor\":{valor},\"a_cada\":{a_cada},\"melhor\":{}}}",
                lista(melhor)
            ),
            Self::Difusao { lado, passo, passos, grade, lado_amostra, a_cada } => format!(
                "{{\"tipo\":\"{t}\",\"lado\":{lado},\"passo\":{passo},\"passos\":{passos},\"lado_amostra\":{lado_amostra},\"a_cada\":{a_cada},\"grade\":{}}}",
                lista(grade)
            ),
            Self::Ia { passo, passos, perda, pesos_saida } => format!(
                "{{\"tipo\":\"{t}\",\"passo\":{passo},\"passos\":{passos},\"perda_q12\":{perda},\"pesos_saida\":{}}}",
                lista(pesos_saida)
            ),
            Self::Genetica { geracao, geracoes, copias, contagens } => format!(
                "{{\"tipo\":\"{t}\",\"geracao\":{geracao},\"geracoes\":{geracoes},\"copias\":{copias},\"contagens\":{}}}",
                lista(contagens)
            ),
            Self::Melhoramento { geracao, geracoes, plantas, pontos, a_cada } => {
                let mut p = String::from("[");
                for (i, (g, f, s)) in pontos.iter().enumerate() {
                    if i > 0 {
                        p.push(',');
                    }
                    let _ = write!(p, "[{g},{f},{}]", u8::from(*s));
                }
                p.push(']');
                format!("{{\"tipo\":\"{t}\",\"geracao\":{geracao},\"geracoes\":{geracoes},\"plantas\":{plantas},\"a_cada\":{a_cada},\"pontos\":{p}}}")
            }
            Self::Rotas { passada, passos, comprimento, inicial, rota } => format!(
                "{{\"tipo\":\"{t}\",\"passada\":{passada},\"passos\":{passos},\"comprimento\":{comprimento},\"inicial\":{inicial},\"rota\":{}}}",
                lista(rota)
            ),
            Self::Triagem { indice, mascara, previsto_mili, nota, aprovada } => format!(
                "{{\"tipo\":\"{t}\",\"indice\":{indice},\"mascara\":{mascara},\"previsto_mili\":{previsto_mili},\"nota\":{nota},\"aprovada\":{aprovada}}}"
            ),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn recorte_e_regular_e_diz_o_passo() {
        let v: Vec<u32> = (0..10).collect();
        assert_eq!(recortar(&v, 100), (v.clone(), 1));
        assert_eq!(recortar(&v, 4), (vec![0, 3, 6, 9], 3));
        let g: Vec<u32> = (0..16).collect(); // 4×4
        assert_eq!(recortar_grade(&g, 4, 2), (vec![0, 2, 8, 10], 2, 2));
    }

    #[test]
    fn json_e_numero_e_lista() {
        let a = Amostra::Genetica { geracao: 3, geracoes: 10, copias: 200, contagens: vec![20, 41] };
        assert_eq!(a.json(), "{\"tipo\":\"population-genetics\",\"geracao\":3,\"geracoes\":10,\"copias\":200,\"contagens\":[20,41]}");
    }
}
