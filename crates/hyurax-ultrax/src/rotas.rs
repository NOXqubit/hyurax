// ✝ Provérbios 16:9 — “O coração do homem planeja o seu caminho.”
//! Rotas: caixeiro-viajante simétrico e euclidiano, por 2-opt (tipo 7).
//!
//! Tradução de `reference/hyurax/rotas.py`, que é a especificação (o modelo,
//! os bytes e o custo estão lá por extenso), conferida byte a byte contra
//! `vectors/rotas.json`.
//!
//! **O que é:** busca local 2-opt numa instância sintética de `n` pontos
//! uniformes no quadrado `[0, 10000)²`. A instância vem do parâmetro
//! `instancia`, não da semente: todas as unidades de um JOB resolvem as
//! mesmas cidades, cada uma a partir de uma rota sorteada pela própria
//! semente (Fisher-Yates com xoshiro256**), e o JOB fica com a melhor.
//!
//! **O que não é:** não é o ótimo global. O 2-opt para no primeiro ótimo
//! local, que em pontos uniformes costuma ficar alguns por cento acima do
//! ótimo, e nada aqui prova quanto. A instância é sorteada, não é um mapa:
//! não há ruas, trânsito, janelas de horário nem capacidade de veículo. É
//! resultado computacional, que precisa de validação antes de valer para uma
//! operação real.
//!
//! Tudo em inteiros: a distância é `floor(sqrt(dx² + dy²))` pela raiz inteira
//! exata, calculada na hora (nenhuma matriz `n²`). A varredura do 2-opt tem
//! ordem fixa e aplica a **primeira melhora** que acha, continuando a mesma
//! passada sobre a rota nova. Dois nós honestos chegam na mesma rota bit a bit.
//!
//! O que cada conferência prova:
//!
//! - [`certificar`], O(n): a rota é uma permutação canônica das `n` cidades e
//!   o comprimento declarado é a soma exata das arestas. Prova que existe
//!   rota com aquele comprimento; **não** prova que ela saiu desta semente,
//!   nem nada sobre otimalidade.
//! - [`certificar_otimo_local`], O(n²): além disso, nenhuma troca 2-opt
//!   encurta a rota. Prova otimalidade **local**; não prova a global nem a
//!   origem da rota.
//! - [`verificar`]: o certificado e depois a recomputação, byte a byte. Prova
//!   que o resultado é exatamente o que especificação e semente produzem.

use hyurax_crypto::xof;

use crate::trabalho::{ErroDeTrabalho, Especificacao, Execucao, Recusa, TipoDeTrabalho};

/// Quantos parâmetros extras a especificação leva: a instância.
pub const PARAMETROS: usize = 1;
/// Faixa aceita de `tamanho` (cidades).
pub const TAMANHO: (u32, u32) = (4, 2_000);
/// Faixa aceita de `passos` (teto de passadas completas do 2-opt).
pub const PASSOS: (u32, u32) = (1, 10_000);
/// As coordenadas ficam em `[0, LADO)`.
pub const LADO: u64 = 10_000;
/// Teto de avaliações de distância por unidade, no pior caso de
/// [`operacoes`]. Perto de 20 s no Atom de desenvolvimento em release, e o
/// dobro com a verificação (medida no módulo Python).
pub const TETO_AVALIACOES: u64 = 5_000_000_000;
/// `continuar` é chamada ao fim de uma linha da varredura quando as
/// avaliações pendentes chegam a este bloco.
pub const BLOCO_CONTINUAR: u64 = 1 << 20;
/// Bytes do cabeçalho do resultado, antes da rota.
pub const CABECALHO: usize = 25;

/// Domínio das coordenadas da instância.
pub const DOMINIO_INSTANCIA: &[u8] = dominio!("ROTAS-INSTANCIA-v1");
/// Domínio do estado inicial do gerador da partida.
pub const DOMINIO_PARTIDA: &[u8] = dominio!("ROTAS-PARTIDA-v1");

/// Uma cidade, com coordenadas inteiras em `[0, 10000)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cidade {
    /// Abscissa.
    pub x: u32,
    /// Ordenada.
    pub y: u32,
}

/// `dx² + dy²`. Com coordenadas abaixo de 10⁴ a soma fica abaixo de 2·10⁸:
/// nenhuma conta dá a volta.
fn quadrado(p: Cidade, q: Cidade) -> u64 {
    let dx = u64::from(p.x.abs_diff(q.x));
    let dy = u64::from(p.y.abs_diff(q.y));
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
}

/// `floor(sqrt(dx² + dy²))`, pela raiz quadrada inteira exata.
pub fn distancia(p: Cidade, q: Cidade) -> u64 {
    quadrado(p, q).isqrt()
}

/// As `n` cidades da instância: dois `u64` big-endian do XOF por cidade,
/// módulo 10000. O XOF é modo contador, então a instância de `n` cidades é o
/// começo da de `n + 1`.
pub fn coordenadas(instancia: u32, n: u32) -> Vec<Cidade> {
    let bruto = xof(&instancia.to_be_bytes(), (n as usize).saturating_mul(16), DOMINIO_INSTANCIA);
    let reduzir = |v: u64| u32::try_from(v.checked_rem(LADO).unwrap_or(0)).unwrap_or(0);
    bruto
        .as_chunks::<16>()
        .0
        .iter()
        .map(|bloco| {
            let (x, y) = bloco.split_at(8);
            let palavra = |s: &[u8]| <[u8; 8]>::try_from(s).map(u64::from_be_bytes).unwrap_or(0);
            Cidade { x: reduzir(palavra(x)), y: reduzir(palavra(y)) }
        })
        .collect()
}

/// Comprimento de uma rota fechada: a soma das `n` arestas, com a volta.
/// Cidade fora da instância conta como a origem (quem chama confere antes).
pub fn comprimento_da_rota(cidades: &[Cidade], rota: &[u16]) -> u64 {
    let em = |c: &u16| cidades.get(usize::from(*c)).copied().unwrap_or_default();
    rota.iter()
        .zip(rota.iter().cycle().skip(1))
        .fold(0u64, |soma, (p, q)| soma.saturating_add(distancia(em(p), em(q))))
}

/// xoshiro256** (Blackman e Vigna, 2018), tudo módulo 2^64.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gerador {
    s: [u64; 4],
}

impl Gerador {
    /// A partir de um estado; o estado todo zero vira `[1, 0, 0, 0]`.
    pub fn do_estado(estado: [u64; 4]) -> Self {
        let [a, b, c, d] = estado;
        Self { s: if estado == [0; 4] { [1, b, c, d] } else { [a, b, c, d] } }
    }

    /// Estado inicial nos quatro `u64` big-endian de
    /// `XOF(semente, 32, ROTAS-PARTIDA-v1)`.
    pub fn da_semente(semente: &[u8]) -> Self {
        let bruto = xof(semente, 32, DOMINIO_PARTIDA);
        let mut estado = [0u64; 4];
        for (e, c) in estado.iter_mut().zip(bruto.as_chunks::<8>().0) {
            *e = u64::from_be_bytes(*c);
        }
        Self::do_estado(estado)
    }

    /// O próximo número de 64 bits.
    pub fn proximo(&mut self) -> u64 {
        let [s0, s1, s2, s3] = self.s;
        let resultado = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s1.wrapping_shl(17);
        let s2 = s2 ^ s0;
        let s3 = s3 ^ s1;
        let s1 = s1 ^ s2;
        let s0 = s0 ^ s3;
        let s2 = s2 ^ t;
        let s3 = s3.rotate_left(45);
        self.s = [s0, s1, s2, s3];
        resultado
    }
}

/// A rota de partida: Fisher-Yates sobre `[0, n)` com o gerador da semente,
/// `j = proximo() mod (k + 1)` para `k` de `n − 1` descendo até 1.
pub fn partida(semente: &[u8], n: u32) -> Vec<u32> {
    let mut g = Gerador::da_semente(semente);
    let mut t: Vec<u32> = (0..n).collect();
    for k in (1..t.len()).rev() {
        let j = g.proximo().checked_rem((k as u64).wrapping_add(1)).unwrap_or(0) as usize;
        t.swap(k, j);
    }
    t
}

/// A forma canônica de um ciclo: começa na cidade 0 e `rota[1] < rota[n−1]`.
pub fn canonica(rota: &[u32]) -> Vec<u32> {
    let k = rota.iter().position(|&c| c == 0).unwrap_or(0);
    let mut r: Vec<u32> = rota.iter().skip(k).chain(rota.iter().take(k)).copied().collect();
    if r.len() > 2
        && r.get(1) > r.last()
        && let Some(resto) = r.get_mut(1..)
    {
        resto.reverse();
    }
    r
}

/// O resultado de uma unidade.
///
/// Bytes canônicos, big-endian, `25 + 2n`: `u64 comprimento || u64
/// comprimento inicial || u32 passadas || u32 melhorias || u8 ótimo local ||
/// n × u16 rota canônica`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resultado {
    /// Comprimento da rota final.
    pub comprimento: u64,
    /// Comprimento da partida sorteada.
    pub inicial: u64,
    /// Passadas feitas, contando a última sem troca.
    pub passadas: u32,
    /// Trocas aplicadas.
    pub melhorias: u32,
    /// A última passada não achou troca: a rota é ótimo local do 2-opt.
    /// Falso quer dizer que as passadas acabaram, e não se afirma nada.
    pub otimo_local: bool,
    /// A rota, na forma canônica.
    pub rota: Vec<u16>,
}

fn ler<const N: usize>(bytes: &[u8], de: usize) -> [u8; N] {
    bytes.get(de..de.saturating_add(N)).and_then(|s| <[u8; N]>::try_from(s).ok()).unwrap_or([0; N])
}

impl Resultado {
    /// Os bytes canônicos.
    pub fn codificar(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(CABECALHO.saturating_add(self.rota.len().saturating_mul(2)));
        v.extend_from_slice(&self.comprimento.to_be_bytes());
        v.extend_from_slice(&self.inicial.to_be_bytes());
        v.extend_from_slice(&self.passadas.to_be_bytes());
        v.extend_from_slice(&self.melhorias.to_be_bytes());
        v.push(u8::from(self.otimo_local));
        for c in &self.rota {
            v.extend_from_slice(&c.to_be_bytes());
        }
        v
    }

    /// O inverso de [`Self::codificar`] para `n` cidades. Só julga o tamanho
    /// e a marca de ótimo local; o resto é com [`certificar`].
    pub fn decodificar(bytes: &[u8], n: u32) -> Result<Self, Recusa> {
        let esperado = CABECALHO.saturating_add((n as usize).saturating_mul(2));
        if bytes.len() != esperado {
            return Err(Recusa(format!("resultado com {} bytes; {n} cidades pedem {esperado}", bytes.len())));
        }
        let marca = bytes.get(24).copied().unwrap_or(0);
        if marca > 1 {
            return Err(Recusa(format!("marca de ótimo local inválida: {marca}")));
        }
        Ok(Self {
            comprimento: u64::from_be_bytes(ler(bytes, 0)),
            inicial: u64::from_be_bytes(ler(bytes, 8)),
            passadas: u32::from_be_bytes(ler(bytes, 16)),
            melhorias: u32::from_be_bytes(ler(bytes, 20)),
            otimo_local: marca == 1,
            rota: bytes.get(CABECALHO..).unwrap_or(&[]).as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect(),
        })
    }
}

fn em(pos: &[Cidade], k: usize) -> Cidade {
    pos.get(k).copied().unwrap_or_default()
}

fn seguinte(k: usize, n: usize) -> usize {
    let s = k.wrapping_add(1);
    if s >= n { 0 } else { s }
}

/// Resolve a unidade: partida da semente e 2-opt até o ótimo local ou até
/// `passos` passadas. Devolve o resultado e as avaliações de distância feitas.
///
/// `continuar` recebe as avaliações em pedaços (ver [`BLOCO_CONTINUAR`]) e
/// pode interromper; a última chamada conta, mas não cancela.
pub fn resolver(
    n: u32,
    passos: u32,
    instancia: u32,
    semente: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<(Resultado, u64), ErroDeTrabalho> {
    validar(n, passos, &[instancia])?;
    let cidades = coordenadas(instancia, n);
    let mut t = partida(semente, n);
    // coordenadas na ordem da rota: a varredura lê em sequência
    let mut pos: Vec<Cidade> = t.iter().map(|&c| cidades.get(c as usize).copied().unwrap_or_default()).collect();
    let tam = pos.len();
    let mut aresta: Vec<u64> = (0..tam).map(|k| distancia(em(&pos, k), em(&pos, seguinte(k, tam)))).collect();
    let mut ops = tam as u64;
    let mut pendente = ops;
    let inicial = aresta.iter().fold(0u64, |s, &d| s.saturating_add(d));
    let mut comprimento = inicial;
    let (mut passadas, mut melhorias, mut otimo_local) = (0u32, 0u32, false);
    while passadas < passos {
        passadas = passadas.saturating_add(1);
        let mut melhorou = false;
        for i in 0..tam.saturating_sub(2) {
            let a = em(&pos, i);
            let mut b = em(&pos, i.wrapping_add(1));
            let mut dab = aresta.get(i).copied().unwrap_or(0);
            // (0, n−1) são arestas vizinhas pela volta
            let fim = if i == 0 { tam.saturating_sub(2) } else { tam.saturating_sub(1) };
            for j in i.wrapping_add(2)..=fim {
                let c = em(&pos, j);
                let limite = dab.wrapping_add(aresta.get(j).copied().unwrap_or(0));
                ops = ops.wrapping_add(1);
                pendente = pendente.wrapping_add(1);
                // isqrt(s) >= L exatamente quando s >= L²: a raiz só é tirada
                // quando a troca ainda pode encurtar
                let sac = quadrado(a, c);
                if sac >= limite.wrapping_mul(limite) {
                    continue;
                }
                let dac = sac.isqrt();
                ops = ops.wrapping_add(1);
                pendente = pendente.wrapping_add(1);
                let dbd = distancia(b, em(&pos, seguinte(j, tam)));
                let novo = dac.wrapping_add(dbd);
                if novo < limite {
                    let meio = i.wrapping_add(1);
                    if let Some(s) = t.get_mut(meio..=j) {
                        s.reverse();
                    }
                    if let Some(s) = pos.get_mut(meio..=j) {
                        s.reverse();
                    }
                    if let Some(s) = aresta.get_mut(meio..j) {
                        s.reverse();
                    }
                    if let Some(e) = aresta.get_mut(i) {
                        *e = dac;
                    }
                    if let Some(e) = aresta.get_mut(j) {
                        *e = dbd;
                    }
                    comprimento = comprimento.saturating_sub(limite.wrapping_sub(novo));
                    melhorias = melhorias.saturating_add(1);
                    melhorou = true;
                    b = c;
                    dab = dac;
                }
            }
            if pendente >= BLOCO_CONTINUAR {
                if !continuar(pendente) {
                    return Err(ErroDeTrabalho::Cancelado);
                }
                pendente = 0;
            }
        }
        if !melhorou {
            otimo_local = true;
            break;
        }
    }
    // a última chamada conta, mas não cancela: o trabalho já acabou
    let _ = continuar(pendente);
    let rota = canonica(&t).into_iter().map(|c| u16::try_from(c).unwrap_or(u16::MAX)).collect();
    Ok((Resultado { comprimento, inicial, passadas, melhorias, otimo_local, rota }, ops))
}

/// A primeira troca 2-opt que encurta a rota dada, na ordem da varredura:
/// `(i, j, ganho)`. `None` quer dizer ótimo local.
pub fn troca_que_melhora(cidades: &[Cidade], rota: &[u16]) -> Option<(usize, usize, u64)> {
    let pos: Vec<Cidade> = rota.iter().map(|&c| cidades.get(usize::from(c)).copied().unwrap_or_default()).collect();
    procurar_troca(&pos, &mut |_| true).ok().flatten()
}

fn procurar_troca(
    pos: &[Cidade],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Option<(usize, usize, u64)>, ErroDeTrabalho> {
    let tam = pos.len();
    let aresta: Vec<u64> = (0..tam).map(|k| distancia(em(pos, k), em(pos, seguinte(k, tam)))).collect();
    let mut pendente = tam as u64;
    for i in 0..tam.saturating_sub(2) {
        let a = em(pos, i);
        let b = em(pos, i.wrapping_add(1));
        let dab = aresta.get(i).copied().unwrap_or(0);
        let fim = if i == 0 { tam.saturating_sub(2) } else { tam.saturating_sub(1) };
        for j in i.wrapping_add(2)..=fim {
            let limite = dab.wrapping_add(aresta.get(j).copied().unwrap_or(0));
            pendente = pendente.wrapping_add(1);
            let sac = quadrado(a, em(pos, j));
            if sac >= limite.wrapping_mul(limite) {
                continue;
            }
            pendente = pendente.wrapping_add(1);
            let novo = sac.isqrt().wrapping_add(distancia(b, em(pos, seguinte(j, tam))));
            if novo < limite {
                return Ok(Some((i, j, limite.wrapping_sub(novo))));
            }
        }
        if pendente >= BLOCO_CONTINUAR {
            if !continuar(pendente) {
                return Err(ErroDeTrabalho::Cancelado);
            }
            pendente = 0;
        }
    }
    let _ = continuar(pendente);
    Ok(None)
}

fn instancia_de(esp: &Especificacao) -> Option<u32> {
    if esp.tipo() == TipoDeTrabalho::Rotas { esp.parametros().first().copied() } else { None }
}

/// Certificado O(n): formato, permutação canônica, comprimento exato e
/// cabeçalho coerente. Devolve o comprimento.
///
/// Prova que existe uma rota com aquele comprimento nesta instância (um
/// limite superior do ótimo). **Não** prova que a rota saiu desta semente,
/// que o 2-opt rodou, nem nada sobre otimalidade.
pub fn certificar(esp: &Especificacao, resultado: &[u8]) -> Result<u64, Recusa> {
    let Some(instancia) = instancia_de(esp) else {
        return Err(Recusa("a especificação não é de rotas".into()));
    };
    let (n, passos) = (esp.tamanho(), esp.passos());
    let r = Resultado::decodificar(resultado, n)?;
    if !(1..=passos).contains(&r.passadas) {
        return Err(Recusa(format!("{} passadas, fora de 1 a {passos}", r.passadas)));
    }
    if !r.otimo_local && r.passadas != passos {
        return Err(Recusa("parou antes do teto de passadas sem declarar ótimo local".into()));
    }
    let mut visto = vec![false; n as usize];
    for &c in &r.rota {
        match visto.get_mut(usize::from(c)) {
            None => return Err(Recusa(format!("a cidade {c} não existe"))),
            Some(true) => return Err(Recusa(format!("a cidade {c} aparece duas vezes"))),
            Some(v) => *v = true,
        }
    }
    if r.rota.first() != Some(&0) || r.rota.get(1) > r.rota.last() {
        return Err(Recusa("rota fora da forma canônica".into()));
    }
    let medido = comprimento_da_rota(&coordenadas(instancia, n), &r.rota);
    if medido != r.comprimento {
        return Err(Recusa(format!("comprimento declarado {}, a rota mede {medido}", r.comprimento)));
    }
    if r.comprimento > r.inicial {
        return Err(Recusa(format!("a rota final ({}) é mais longa que a partida ({})", r.comprimento, r.inicial)));
    }
    let queda = r.inicial.wrapping_sub(r.comprimento);
    if (r.melhorias == 0) != (queda == 0) || u64::from(r.melhorias) > queda {
        return Err(Recusa(format!("{} melhorias não fecham com a queda de {queda}", r.melhorias)));
    }
    Ok(r.comprimento)
}

/// Certificado O(n²): o de [`certificar`] e, além dele, que nenhuma troca
/// 2-opt encurta a rota. Devolve o comprimento. Mesma convenção de
/// [`crate::trabalho::verificar_controlado`]: `Err(Cancelado)` é ninguém
/// julgou; `Ok(Err(recusa))` traz a troca que melhora como evidência.
///
/// Prova otimalidade **local** na vizinhança do 2-opt, com a métrica
/// inteira. **Não** prova otimalidade global nem a origem da rota.
pub fn certificar_otimo_local(
    esp: &Especificacao,
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<u64, Recusa>, ErroDeTrabalho> {
    let comprimento = match certificar(esp, resultado) {
        Ok(c) => c,
        Err(r) => return Ok(Err(r)),
    };
    let cidades = coordenadas(instancia_de(esp).unwrap_or(0), esp.tamanho());
    let rota = bytes_da_rota(resultado);
    let pos: Vec<Cidade> = rota.iter().map(|&c| cidades.get(usize::from(c)).copied().unwrap_or_default()).collect();
    Ok(match procurar_troca(&pos, continuar)? {
        None => Ok(comprimento),
        Some((i, j, ganho)) => Err(Recusa(format!("a troca 2-opt das arestas {i} e {j} encurta a rota em {ganho}"))),
    })
}

fn bytes_da_rota(resultado: &[u8]) -> Vec<u16> {
    resultado.get(CABECALHO..).unwrap_or(&[]).as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect()
}

/// Confere as faixas e o teto de custo ([`TETO_AVALIACOES`]).
pub fn validar(tamanho: u32, passos: u32, parametros: &[u32]) -> Result<(), ErroDeTrabalho> {
    if parametros.len() != PARAMETROS {
        return Err(ErroDeTrabalho::Parametros(format!("rotas leva {PARAMETROS} parâmetro, vieram {}", parametros.len())));
    }
    if !(TAMANHO.0..=TAMANHO.1).contains(&tamanho) {
        return Err(ErroDeTrabalho::Parametros(format!("cidades fora da faixa ({} a {}): {tamanho}", TAMANHO.0, TAMANHO.1)));
    }
    if !(PASSOS.0..=PASSOS.1).contains(&passos) {
        return Err(ErroDeTrabalho::Parametros(format!("passadas fora da faixa ({} a {}): {passos}", PASSOS.0, PASSOS.1)));
    }
    let custo = teto(tamanho, passos);
    if custo > TETO_AVALIACOES {
        return Err(ErroDeTrabalho::Parametros(format!(
            "custo de {custo} avaliações passa do teto de {TETO_AVALIACOES}"
        )));
    }
    Ok(())
}

/// `n + passos · n · (n − 3)`: `n` no comprimento inicial e, por passada, até
/// duas avaliações em cada um dos `n(n − 3)/2` candidatos.
fn teto(n: u32, passos: u32) -> u64 {
    let n = u64::from(n);
    n.saturating_add(u64::from(passos).saturating_mul(n).saturating_mul(n.saturating_sub(3)))
}

/// Operações da execução: o **teto** de avaliações de distância (todas as
/// passadas até o fim, duas avaliações por candidato). A execução de verdade
/// costuma parar muito antes, e [`Execucao::operacoes`] traz o que ela fez.
pub fn operacoes(esp: &Especificacao) -> u64 {
    teto(esp.tamanho(), esp.passos())
}

/// Operações da verificação: o certificado (`n` avaliações) e a recomputação.
pub fn operacoes_de_verificacao(esp: &Especificacao) -> u64 {
    operacoes(esp).saturating_add(u64::from(esp.tamanho()))
}

/// Memória no pico, execução e verificação somadas, em bytes.
///
/// Por cidade, na execução: o XOF das coordenadas (16), as cidades (8), a
/// rota (4), as coordenadas na ordem da rota (8), as arestas (8), a rota
/// canônica (4 e 2) e o resultado (2), 52 no total; a verificação guarda o
/// resultado alegado (2), decodifica a rota (2), marca as visitadas (1) e
/// refaz tudo (52). Arredondado para 128 por cidade, mais 64 KiB de folga.
pub fn memoria_bytes(esp: &Especificacao) -> u64 {
    u64::from(esp.tamanho()).saturating_mul(128).saturating_add(64 * 1024)
}

/// Texto curto para o painel.
pub fn resumo(esp: &Especificacao) -> String {
    format!(
        "rotas: {} cidades da instância {}, até {} passadas do 2-opt",
        esp.tamanho(),
        esp.parametros().first().copied().unwrap_or(0),
        esp.passos()
    )
}

/// Executa a unidade. [`Execucao::operacoes`] são as avaliações de
/// distância feitas, iguais em todo worker honesto.
pub fn executar(esp: &Especificacao, semente: &[u8], continuar: &mut dyn FnMut(u64) -> bool) -> Result<Execucao, ErroDeTrabalho> {
    let instancia = instancia_de(esp).ok_or_else(|| ErroDeTrabalho::Parametros("a especificação não é de rotas".into()))?;
    let (r, operacoes) = resolver(esp.tamanho(), esp.passos(), instancia, semente, continuar)?;
    Ok(Execucao { resultado: r.codificar(), operacoes, curva: Vec::new() })
}

/// Confere um resultado: o certificado O(n) primeiro, depois a recomputação.
/// Mesma convenção de [`crate::trabalho::verificar_controlado`].
pub fn verificar(
    esp: &Especificacao,
    semente: &[u8],
    resultado: &[u8],
    continuar: &mut dyn FnMut(u64) -> bool,
) -> Result<Result<(), Recusa>, ErroDeTrabalho> {
    if let Err(recusa) = certificar(esp, resultado) {
        return Ok(Err(recusa));
    }
    if !continuar(u64::from(esp.tamanho())) {
        return Err(ErroDeTrabalho::Cancelado);
    }
    let refeito = executar(esp, semente, continuar)?.resultado;
    if refeito == resultado {
        return Ok(Ok(()));
    }
    let campos = [
        ("comprimento", 0..8),
        ("comprimento inicial", 8..16),
        ("passadas", 16..20),
        ("melhorias", 20..24),
        ("marca de ótimo local", 24..25),
    ];
    for (nome, faixa) in campos {
        if refeito.get(faixa.clone()) != resultado.get(faixa) {
            return Ok(Err(Recusa(format!("{nome} difere da recomputação"))));
        }
    }
    let k = bytes_da_rota(&refeito).iter().zip(bytes_da_rota(resultado)).position(|(a, b)| *a != b);
    Ok(Err(Recusa(match k {
        Some(k) => format!("a rota difere da recomputação na posição {k}"),
        None => "o resultado difere da recomputação".into(),
    })))
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

    use super::*;
    use crate::job::{Dominio, EspecificacaoDeJob, Nivel, PedidoDeJob, unidade};

    fn esp(n: u32, passos: u32, instancia: u32) -> Especificacao {
        Especificacao::nova_com(TipoDeTrabalho::Rotas, n, passos, &[instancia]).unwrap()
    }

    fn semente(i: u32) -> [u8; 64] {
        hyurax_crypto::sha512(&i.to_be_bytes())
    }

    /// O ótimo por força bruta: a cidade 0 fixa, todas as ordens das outras.
    fn otimo(cidades: &[Cidade]) -> u64 {
        fn permutar(resto: &mut Vec<u16>, k: usize, cidades: &[Cidade], melhor: &mut u64) {
            if k == resto.len() {
                let mut rota = vec![0u16];
                rota.extend_from_slice(resto);
                *melhor = (*melhor).min(comprimento_da_rota(cidades, &rota));
                return;
            }
            for m in k..resto.len() {
                resto.swap(k, m);
                permutar(resto, k + 1, cidades, melhor);
                resto.swap(k, m);
            }
        }
        let mut resto: Vec<u16> = (1..cidades.len() as u16).collect();
        let mut melhor = u64::MAX;
        permutar(&mut resto, 0, cidades, &mut melhor);
        melhor
    }

    #[test]
    fn gerador_bate_com_a_referencia_do_xoshiro() {
        // saídas do código de referência de Blackman e Vigna com o estado {1, 2, 3, 4}
        let mut g = Gerador::do_estado([1, 2, 3, 4]);
        let feito: Vec<u64> = (0..4).map(|_| g.proximo()).collect();
        assert_eq!(feito, [11_520, 0, 1_509_978_240, 1_215_971_899_390_074_240]);
        assert_eq!(Gerador::do_estado([0; 4]), Gerador::do_estado([1, 0, 0, 0]));
    }

    #[test]
    fn distancia_e_a_raiz_inteira_exata() {
        let cidades = coordenadas(3, 300);
        assert!(cidades.iter().all(|c| u64::from(c.x) < LADO && u64::from(c.y) < LADO));
        for par in cidades.windows(2) {
            let (p, q) = (par[0], par[1]);
            let d = distancia(p, q);
            let s = quadrado(p, q);
            assert!(d * d <= s && s < (d + 1) * (d + 1));
            assert_eq!(d, distancia(q, p));
        }
        assert_eq!(distancia(Cidade { x: 0, y: 0 }, Cidade { x: 3, y: 4 }), 5);
        assert_eq!(distancia(Cidade { x: 0, y: 0 }, Cidade { x: 9_999, y: 9_999 }), 14_140);
        // instância de n cidades é o começo da de n + 1
        assert_eq!(coordenadas(3, 100)[..], cidades[..100]);
    }

    #[test]
    fn partida_e_permutacao_e_canonica_normaliza() {
        let t = partida(&semente(1), 50);
        let mut ordenada = t.clone();
        ordenada.sort_unstable();
        assert_eq!(ordenada, (0..50).collect::<Vec<_>>());
        assert_ne!(t, partida(&semente(2), 50));
        let c = canonica(&t);
        assert_eq!(c[0], 0);
        assert!(c[1] < c[49]);
        // girar ou inverter o ciclo não muda a forma canônica
        let mut girada = t.clone();
        girada.rotate_left(17);
        assert_eq!(canonica(&girada), c);
        girada.reverse();
        assert_eq!(canonica(&girada), c);
    }

    #[test]
    fn instancias_pequenas_contra_a_forca_bruta() {
        let mut no_otimo = 0;
        let mut casos = 0;
        for n in 4..=8u32 {
            for instancia in 0..4u32 {
                let cidades = coordenadas(instancia, n);
                let melhor = otimo(&cidades);
                for s in 0..3u32 {
                    let e = esp(n, PASSOS.1, instancia);
                    let (r, ops) = resolver(n, PASSOS.1, instancia, &semente(s), &mut |_| true).unwrap();
                    assert!(r.otimo_local, "com passadas de sobra, o 2-opt converge");
                    assert!(r.comprimento >= melhor, "nada fica abaixo do ótimo");
                    assert!(r.comprimento <= r.inicial);
                    assert_eq!(comprimento_da_rota(&cidades, &r.rota), r.comprimento);
                    assert!(troca_que_melhora(&cidades, &r.rota).is_none(), "ótimo local");
                    assert!(ops <= operacoes(&e));
                    let bytes = r.codificar();
                    assert_eq!(certificar(&e, &bytes), Ok(r.comprimento));
                    assert_eq!(certificar_otimo_local(&e, &bytes, &mut |_| true), Ok(Ok(r.comprimento)));
                    no_otimo += usize::from(r.comprimento == melhor);
                    casos += 1;
                }
            }
        }
        // o 2-opt não garante o ótimo, mas em até 8 cidades quase sempre chega
        assert!(no_otimo * 10 >= casos * 7, "{no_otimo} de {casos} no ótimo");
    }

    #[test]
    fn melhora_muito_a_partida() {
        let e = esp(80, PASSOS.1, 11);
        let exec = executar(&e, &semente(5), &mut |_| true).unwrap();
        let r = Resultado::decodificar(&exec.resultado, 80).unwrap();
        assert!(r.otimo_local);
        assert!(r.comprimento * 3 < r.inicial, "{} contra {}", r.comprimento, r.inicial);
        assert!(r.melhorias > 0 && u64::from(r.melhorias) <= r.inicial - r.comprimento);
        assert!(exec.operacoes <= e.operacoes_fixas().unwrap());
        assert_eq!(verificar(&e, &semente(5), &exec.resultado, &mut |_| true), Ok(Ok(())));
    }

    #[test]
    fn passadas_esgotadas_nao_afirmam_otimo_local() {
        let e = esp(60, 1, 2);
        let exec = executar(&e, &semente(0), &mut |_| true).unwrap();
        let r = Resultado::decodificar(&exec.resultado, 60).unwrap();
        assert_eq!((r.passadas, r.otimo_local), (1, false));
        assert_eq!(certificar(&e, &exec.resultado), Ok(r.comprimento));
        // uma passada não basta: ainda há troca que melhora, e o certificado acusa
        let local = certificar_otimo_local(&e, &exec.resultado, &mut |_| true).unwrap();
        assert!(local.unwrap_err().0.contains("encurta"));
    }

    #[test]
    fn adulteracoes_sao_recusadas() {
        let e = esp(12, 50, 4);
        let s = semente(9);
        let honesto = executar(&e, &s, &mut |_| true).unwrap().resultado;
        let recusa = |bytes: &[u8]| {
            let motivo = certificar(&e, bytes).unwrap_err().0;
            let pela_verificacao = verificar(&e, &s, bytes, &mut |_| true).unwrap().unwrap_err().0;
            assert_eq!(motivo, pela_verificacao, "a verificação recusa pelo certificado, sem recomputar");
            motivo
        };

        // cidade repetida
        let mut repetida = honesto.clone();
        repetida[CABECALHO + 2 * 5..CABECALHO + 2 * 6].copy_from_slice(&honesto[CABECALHO + 2 * 4..CABECALHO + 2 * 5]);
        assert!(recusa(&repetida).contains("duas vezes"));
        // cidade que não existe
        let mut fantasma = honesto.clone();
        fantasma[CABECALHO + 2 * 3..CABECALHO + 2 * 4].copy_from_slice(&12u16.to_be_bytes());
        assert!(recusa(&fantasma).contains("não existe"));
        // comprimento mentiroso, para menos
        let mut mentira = honesto.clone();
        let c = u64::from_be_bytes(mentira[..8].try_into().unwrap());
        mentira[..8].copy_from_slice(&(c - 1).to_be_bytes());
        assert!(recusa(&mentira).contains("a rota mede"));
        // fora da forma canônica: a mesma rota, girada
        let mut girada = honesto.clone();
        girada[CABECALHO..].rotate_left(2);
        assert!(recusa(&girada).contains("canônica"));
        // marca inválida, tamanho errado e cabeçalho incoerente
        let mut marca = honesto.clone();
        marca[24] = 2;
        assert!(recusa(&marca).contains("marca"));
        assert!(recusa(&honesto[1..]).contains("bytes"));
        let mut melhorias = honesto.clone();
        melhorias[20..24].copy_from_slice(&0u32.to_be_bytes());
        assert!(recusa(&melhorias).contains("melhorias"));
        assert!(recusa(&honesto[..0]).contains("bytes"));
    }

    #[test]
    fn certificado_nao_prova_a_origem_e_a_recomputacao_prova() {
        // o ótimo local de outra semente: certificável, e mesmo assim não é
        // o que esta semente produz
        let e = esp(40, PASSOS.1, 6);
        let a = executar(&e, &semente(1), &mut |_| true).unwrap().resultado;
        let b = executar(&e, &semente(2), &mut |_| true).unwrap().resultado;
        assert_ne!(a, b);
        assert!(certificar(&e, &b).is_ok());
        assert!(certificar_otimo_local(&e, &b, &mut |_| true).unwrap().is_ok());
        let recusa = verificar(&e, &semente(1), &b, &mut |_| true).unwrap().unwrap_err();
        assert!(recusa.0.contains("recomputação"), "{recusa}");

        // a partida canônica, com o comprimento honesto: certificável, mas não
        // é ótimo local
        let cidades = coordenadas(6, 40);
        let rota: Vec<u16> = canonica(&partida(&semente(1), 40)).into_iter().map(|c| c as u16).collect();
        let comprimento = comprimento_da_rota(&cidades, &rota);
        let partida_crua = Resultado { comprimento, inicial: comprimento, passadas: PASSOS.1, melhorias: 0, otimo_local: false, rota };
        let bytes = partida_crua.codificar();
        assert_eq!(certificar(&e, &bytes), Ok(comprimento));
        assert!(certificar_otimo_local(&e, &bytes, &mut |_| true).unwrap().is_err());
    }

    #[test]
    fn cancelar_para_e_verificacao_interrompida_nao_e_recusa() {
        let e = esp(1_500, 1_000, 1);
        let mut chamadas = 0;
        let r = executar(&e, &semente(3), &mut |_| {
            chamadas += 1;
            chamadas < 2
        });
        assert_eq!(r, Err(ErroDeTrabalho::Cancelado));
        assert_eq!(chamadas, 2);

        let mut pedacos = Vec::new();
        let exec = executar(&e, &semente(3), &mut |ops| {
            pedacos.push(ops);
            true
        })
        .unwrap();
        assert!(pedacos.len() >= 3, "a unidade vem em pedaços: {pedacos:?}");
        assert_eq!(pedacos.iter().sum::<u64>(), exec.operacoes);
        // nenhum pedaço do meio passa de um bloco mais uma linha da varredura
        assert!(pedacos.iter().all(|&p| p < BLOCO_CONTINUAR + 2 * 1_500));

        let mut n = 0;
        let v = verificar(&e, &semente(3), &exec.resultado, &mut |_| {
            n += 1;
            n < 3
        });
        assert_eq!(v, Err(ErroDeTrabalho::Cancelado), "parar não é recusar");
        assert_eq!(verificar(&e, &semente(3), &exec.resultado, &mut |_| true), Ok(Ok(())));
    }

    #[test]
    fn unidades_do_mesmo_job_resolvem_a_mesma_instancia() {
        let job = EspecificacaoDeJob::nova(PedidoDeJob {
            dominio: Dominio::Logistica,
            modelo: esp(30, 100, 77),
            unidades: 10,
            nivel: Nivel::Reexecucao,
            redundancia: 1,
            prazo_s: 0,
            orcamento_milicreditos: 0,
            descricao: "entregas numa instância sorteada".into(),
        })
        .unwrap();
        let id = job.id().unwrap();
        let unidades: Vec<_> = (0..3).map(|i| unidade(&job, &id, i).unwrap()).collect();
        let resultados: Vec<Vec<u8>> =
            unidades.iter().map(|u| executar(&u.especificacao, &u.semente, &mut |_| true).unwrap().resultado).collect();
        for (u, r) in unidades.iter().zip(&resultados) {
            assert_eq!(&u.especificacao, job.modelo(), "a mesma especificação");
            assert_eq!(coordenadas(instancia_de(&u.especificacao).unwrap(), 30), coordenadas(77, 30));
            // o resultado de uma unidade se certifica na especificação de qualquer outra
            assert!(certificar(&unidades[0].especificacao, r).is_ok());
        }
        assert_ne!(partida(&unidades[0].semente, 30), partida(&unidades[1].semente, 30), "partidas diferentes");
        assert_ne!(resultados[0][8..16], resultados[1][8..16], "comprimentos iniciais diferentes");
    }

    #[test]
    fn faixas_e_teto() {
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 3, 1, &[0]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 2_001, 1, &[0]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 10, 0, &[0]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 10, 10_001, &[0]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 10, 1, &[]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 10, 1, &[0, 1]).is_err());
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 4, 1, &[u32::MAX]).is_ok());
        // o teto: com 2000 cidades cabem 1251 passadas, não 1252
        assert!(Especificacao::nova_com(TipoDeTrabalho::Rotas, 2_000, 1_251, &[0]).is_ok());
        assert!(matches!(
            Especificacao::nova_com(TipoDeTrabalho::Rotas, 2_000, 1_252, &[0]),
            Err(ErroDeTrabalho::Parametros(m)) if m.contains("teto")
        ));
        let e = esp(2_000, 1_251, 0);
        assert!(operacoes(&e) <= TETO_AVALIACOES);
        assert!(memoria_bytes(&e) < 1 << 20);
        assert!(resumo(&e).contains("2000 cidades"));
        // outro tipo não se certifica aqui
        let matriz = Especificacao::nova(TipoDeTrabalho::Matriz, 4, 0).unwrap();
        assert!(certificar(&matriz, &[0; 33]).is_err());
    }
}
