//! Codificação canônica binária e árvore de Merkle.
//!
//! Tradução de `reference/auron/codec.py`. Cada leitura, escrita e prova é
//! conferida contra `vectors/codec.json` e `vectors/codec_edge.json`, gerados
//! pela implementação Python, inclusive a mensagem de cada recusa.
//!
//! Regras da codificação:
//!
//! - Inteiros são big-endian, de largura fixa e explícita.
//! - Bytes de tamanho variável levam prefixo de tamanho `u32` big-endian.
//! - Uma estrutura é a concatenação dos campos, na ordem declarada, sem nomes
//!   e sem separadores.
//! - Não existe representação alternativa do mesmo valor: sobra de bytes no
//!   fim é recusada.
//!
//! O prefixo de tamanho não é detalhe. Em 06/09/2026 a rede Liquid perdeu 95%
//! das reservas porque uma chave de cache era calculada sobre campos de
//! tamanho variável sem prefixo: dava para mover bytes de um campo para o
//! outro e cair na mesma chave.

#![forbid(unsafe_code)]
// Em teste, entrar em pânico é como se sinaliza falha. Os lints que proíbem
// isso valem para o código de produção, não para a suíte.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]

use core::fmt;

use auron_crypto::sha512;
pub use auron_crypto::HASH_LEN;

/// Maior tamanho de bytes variáveis que o prefixo `u32` consegue declarar.
pub const MAX_BYTES_LEN: u32 = u32::MAX;

/// Maior número de itens que o prefixo `u32` de uma lista consegue declarar.
pub const MAX_LIST_LEN: u32 = u32::MAX;

// O prefixo `u32` precisa caber em `usize` sem perda. Num alvo de 16 bits o
// build quebra aqui, em vez de um prefixo grande ser truncado em silêncio.
const _: () = assert!(usize::BITS >= 32);

/// Motivo pelo qual uma codificação ou leitura foi recusada.
///
/// O texto de cada variante é idêntico ao da mensagem do Python. Os vetores
/// comparam as duas, então mudar uma exige mudar a outra e regerar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Bytes variáveis maiores do que o prefixo `u32` consegue declarar.
    BytesLongosDemais,
    /// Lista com mais itens do que o prefixo `u32` consegue declarar.
    ListaLongaDemais,
    /// A leitura pediu mais bytes do que restam na entrada.
    FimInesperado {
        /// Bytes que a leitura pediu.
        pedidos: usize,
        /// Bytes que restavam na entrada.
        restantes: usize,
    },
    /// Os bytes de uma string não são UTF-8 válido.
    Utf8Invalido,
    /// A entrada terminou de ser lida e ainda sobraram bytes.
    BytesSobrando {
        /// Quantos bytes sobraram.
        quantidade: usize,
    },
    /// Índice de folha fora da árvore de Merkle.
    IndiceForaDaFaixa,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BytesLongosDemais => f.write_str("bytes longos demais"),
            Self::ListaLongaDemais => f.write_str("lista longa demais"),
            Self::FimInesperado { pedidos, restantes } => {
                write!(f, "fim inesperado: pediu {pedidos} bytes, restam {restantes}")
            }
            Self::Utf8Invalido => f.write_str("string UTF-8 inválida"),
            Self::BytesSobrando { quantidade } => {
                write!(f, "{quantidade} bytes não consumidos no fim")
            }
            Self::IndiceForaDaFaixa => f.write_str("índice de folha fora da faixa"),
        }
    }
}

impl core::error::Error for CodecError {}

/// Converte um tamanho no prefixo `u32`, recusando o que não cabe.
///
/// Nunca `as u32`: acima de 4 GiB ele truncaria em silêncio e o prefixo
/// mentiria sobre o conteúdo.
fn prefixo(tamanho: usize, erro: CodecError) -> Result<u32, CodecError> {
    u32::try_from(tamanho).map_err(|_| erro)
}

// ---------------------------------------------------------------------------
// Escrita
// ---------------------------------------------------------------------------

/// Escritor da codificação canônica.
///
/// Depois de um `Err`, o conteúdo acumulado é indefinido e o escritor deve
/// ser descartado, do mesmo jeito que no Python a exceção não devolve nada.
#[derive(Debug, Default, Clone)]
pub struct Writer {
    saida: Vec<u8>,
}

impl Writer {
    /// Escritor vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Um byte.
    pub fn u8(&mut self, valor: u8) {
        self.saida.push(valor);
    }

    /// Inteiro de 2 bytes, big-endian.
    pub fn u16(&mut self, valor: u16) {
        self.saida.extend_from_slice(&valor.to_be_bytes());
    }

    /// Inteiro de 4 bytes, big-endian.
    pub fn u32(&mut self, valor: u32) {
        self.saida.extend_from_slice(&valor.to_be_bytes());
    }

    /// Inteiro de 8 bytes, big-endian.
    pub fn u64(&mut self, valor: u64) {
        self.saida.extend_from_slice(&valor.to_be_bytes());
    }

    /// Bytes de tamanho fixo, sem prefixo. Usado em hash e endereço.
    ///
    /// O tamanho está no tipo, então o erro de tamanho errado do Python nem
    /// se representa aqui.
    pub fn fixed<const N: usize>(&mut self, dados: &[u8; N]) {
        self.saida.extend_from_slice(dados);
    }

    /// Bytes de tamanho variável: prefixo `u32` de tamanho e o conteúdo.
    pub fn var_bytes(&mut self, dados: &[u8]) -> Result<(), CodecError> {
        let tamanho = prefixo(dados.len(), CodecError::BytesLongosDemais)?;
        self.u32(tamanho);
        self.saida.extend_from_slice(dados);
        Ok(())
    }

    /// String em UTF-8, com prefixo `u32` de tamanho em bytes.
    pub fn string(&mut self, texto: &str) -> Result<(), CodecError> {
        self.var_bytes(texto.as_bytes())
    }

    /// Lista: prefixo `u32` com o número de itens e cada item em sequência.
    ///
    /// O tamanho é conferido antes de escrever qualquer coisa, como no
    /// Python. O erro do item é propagado, e por isso o tipo de erro é
    /// genérico.
    pub fn list<T, E: From<CodecError>>(
        &mut self,
        itens: &[T],
        mut item: impl FnMut(&mut Self, &T) -> Result<(), E>,
    ) -> Result<(), E> {
        let quantidade = prefixo(itens.len(), CodecError::ListaLongaDemais)?;
        self.u32(quantidade);
        for valor in itens {
            item(self, valor)?;
        }
        Ok(())
    }

    /// Os bytes escritos.
    pub fn into_bytes(self) -> Vec<u8> {
        self.saida
    }
}

// ---------------------------------------------------------------------------
// Leitura
// ---------------------------------------------------------------------------

/// Leitor posicional da codificação canônica.
///
/// Guarda só o que falta ler, não uma posição. Não existe soma de posição que
/// possa estourar, e nada é alocado a partir de um prefixo de tamanho: o que
/// resta é conferido antes, como em `codec.py`.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    resto: &'a [u8],
}

impl<'a> Reader<'a> {
    /// Leitor sobre a entrada inteira.
    pub fn new(dados: &'a [u8]) -> Self {
        Self { resto: dados }
    }

    /// Bytes que ainda não foram lidos.
    pub fn remaining(&self) -> usize {
        self.resto.len()
    }

    /// Os próximos `n` bytes. Se não houver, o leitor não avança.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        let (pedaco, resto) =
            self.resto
                .split_at_checked(n)
                .ok_or(CodecError::FimInesperado {
                    pedidos: n,
                    restantes: self.resto.len(),
                })?;
        self.resto = resto;
        Ok(pedaco)
    }

    /// Os próximos `N` bytes, de tamanho fixo.
    pub fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let (bloco, resto) =
            self.resto
                .split_first_chunk::<N>()
                .ok_or(CodecError::FimInesperado {
                    pedidos: N,
                    restantes: self.resto.len(),
                })?;
        self.resto = resto;
        Ok(*bloco)
    }

    /// Um byte.
    pub fn u8(&mut self) -> Result<u8, CodecError> {
        self.fixed::<1>().map(u8::from_be_bytes)
    }

    /// Inteiro de 2 bytes, big-endian.
    pub fn u16(&mut self) -> Result<u16, CodecError> {
        self.fixed::<2>().map(u16::from_be_bytes)
    }

    /// Inteiro de 4 bytes, big-endian.
    pub fn u32(&mut self) -> Result<u32, CodecError> {
        self.fixed::<4>().map(u32::from_be_bytes)
    }

    /// Inteiro de 8 bytes, big-endian.
    pub fn u64(&mut self) -> Result<u64, CodecError> {
        self.fixed::<8>().map(u64::from_be_bytes)
    }

    /// Bytes de tamanho variável, sem cópia.
    pub fn var_bytes(&mut self) -> Result<&'a [u8], CodecError> {
        let tamanho = self.u32()?;
        // `usize` tem pelo menos 32 bits (conferido na compilação), então a
        // conversão não perde nada. Se perdesse, `usize::MAX` ainda cairia em
        // `FimInesperado`, nunca numa leitura errada.
        self.take(usize::try_from(tamanho).unwrap_or(usize::MAX))
    }

    /// String em UTF-8 estrito.
    ///
    /// O tamanho é conferido antes do UTF-8, como no Python: um prefixo além
    /// do fim dá `FimInesperado`, não `Utf8Invalido`.
    pub fn string(&mut self) -> Result<&'a str, CodecError> {
        core::str::from_utf8(self.var_bytes()?).map_err(|_| CodecError::Utf8Invalido)
    }

    /// Lista: prefixo `u32` com o número de itens e cada item em sequência.
    ///
    /// A contagem vem de quem mandou os bytes, então nunca vira
    /// `with_capacity`: quatro bytes maliciosos pediriam memória para bilhões
    /// de itens. A lista cresce item a item, e uma entrada curta acaba em
    /// `FimInesperado` logo no primeiro item que falta.
    pub fn read_list<T, E: From<CodecError>>(
        &mut self,
        mut item: impl FnMut(&mut Self) -> Result<T, E>,
    ) -> Result<Vec<T>, E> {
        let quantidade = self.u32()?;
        let mut itens = Vec::new();
        for _ in 0..quantidade {
            itens.push(item(self)?);
        }
        Ok(itens)
    }

    /// Exige que a entrada tenha sido consumida por inteiro.
    ///
    /// Sobra de bytes é recusada de propósito: aceitar sobra deixa duas
    /// sequências diferentes decodificarem para a mesma estrutura, e aí o
    /// hash da estrutura deixa de identificá-la de forma única.
    pub fn finish(self) -> Result<(), CodecError> {
        if self.resto.is_empty() {
            Ok(())
        } else {
            Err(CodecError::BytesSobrando {
                quantidade: self.resto.len(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Árvore de Merkle — RFC 6962
// ---------------------------------------------------------------------------

/// Prefixo de folha. Impede que o hash de uma folha seja confundido com o de
/// uma subárvore.
const PREFIXO_FOLHA: u8 = 0x00;

/// Prefixo de nó interno.
const PREFIXO_NO: u8 = 0x01;

/// Hash de uma folha: `H(0x00 || dados)`.
pub fn merkle_leaf_hash(dados: &[u8]) -> [u8; HASH_LEN] {
    let mut entrada = Vec::with_capacity(dados.len().saturating_add(1));
    entrada.push(PREFIXO_FOLHA);
    entrada.extend_from_slice(dados);
    sha512(&entrada)
}

/// Hash de um nó interno: `H(0x01 || esquerda || direita)`.
fn no(esquerda: &[u8; HASH_LEN], direita: &[u8; HASH_LEN]) -> [u8; HASH_LEN] {
    let mut entrada = Vec::with_capacity(HASH_LEN.saturating_mul(2).saturating_add(1));
    entrada.push(PREFIXO_NO);
    entrada.extend_from_slice(esquerda);
    entrada.extend_from_slice(direita);
    sha512(&entrada)
}

/// Maior potência de 2 estritamente menor que `n`: é o laço de
/// `codec.py:198-200`. `None` com menos de duas folhas.
fn ponto_de_divisao(n: usize) -> Option<usize> {
    1_usize.checked_shl(n.checked_sub(1)?.checked_ilog2()?)
}

/// O mesmo que `ponto_de_divisao`, em `u64`, para a verificação de prova.
fn ponto_de_divisao_u64(n: u64) -> Option<u64> {
    1_u64.checked_shl(n.checked_sub(1)?.checked_ilog2()?)
}

/// Divide as folhas no ponto da RFC 6962. `None` com menos de duas.
fn dividir<T>(itens: &[T]) -> Option<(&[T], &[T])> {
    itens.split_at_checked(ponto_de_divisao(itens.len())?)
}

/// Raiz de Merkle no formato da RFC 6962 (Certificate Transparency).
///
/// Em contagem ímpar, o último elemento é promovido, não duplicado. Duplicar
/// o último é o bug clássico do Bitcoin (CVE-2012-2459), em que duas listas
/// diferentes produzem a mesma raiz.
///
/// A árvore vazia tem raiz `H("")`, sem prefixo, como no Python.
pub fn merkle_root<F: AsRef<[u8]>>(folhas: &[F]) -> [u8; HASH_LEN] {
    if let Some((esquerda, direita)) = dividir(folhas) {
        return no(&merkle_root(esquerda), &merkle_root(direita));
    }
    match folhas.first() {
        Some(unica) => merkle_leaf_hash(unica.as_ref()),
        None => sha512(b""),
    }
}

/// Caminho de auditoria da folha `indice` até a raiz, de baixo para cima.
pub fn merkle_path<F: AsRef<[u8]>>(
    folhas: &[F],
    indice: usize,
) -> Result<Vec<[u8; HASH_LEN]>, CodecError> {
    if indice >= folhas.len() {
        return Err(CodecError::IndiceForaDaFaixa);
    }
    let mut caminho = Vec::new();
    caminho_ate(folhas, indice, &mut caminho);
    Ok(caminho)
}

/// Desce até a folha e empilha os irmãos na volta, de baixo para cima.
fn caminho_ate<F: AsRef<[u8]>>(folhas: &[F], indice: usize, caminho: &mut Vec<[u8; HASH_LEN]>) {
    // Uma folha só: o caminho acaba aqui.
    let Some((esquerda, direita)) = dividir(folhas) else {
        return;
    };
    match indice.checked_sub(esquerda.len()) {
        None => {
            caminho_ate(esquerda, indice, caminho);
            caminho.push(merkle_root(direita));
        }
        Some(na_direita) => {
            caminho_ate(direita, na_direita, caminho);
            caminho.push(merkle_root(esquerda));
        }
    }
}

/// Confere uma prova de inclusão sem precisar da lista inteira.
///
/// Devolve `false` para qualquer prova inválida, nunca erro.
///
/// Índice e total são `u64`, e não `usize`, para o resultado não depender da
/// largura do ponteiro: um total vindo da rede precisa dar o mesmo veredito
/// num nó de 32 bits e num de 64.
///
/// **A prova não amarra o total.** O oráculo aceita uma prova de 3 folhas
/// conferida com total 4, porque as duas árvores têm a mesma forma no caminho
/// daquela folha. Quem confere precisa obter o total de uma fonte
/// autenticada.
///
/// O Python aceita irmão de qualquer tamanho; aqui o caminho é de hashes de 64
/// bytes. O veredito é o mesmo, porque sem colisão de SHA-512 só um irmão de
/// 64 bytes reproduz a entrada `0x01 || 64 || 64` de cada nível. Um formato de
/// prova na rede, quando existir, precisa usar `fixed(64)`.
pub fn merkle_verify_path(
    folha: &[u8],
    indice: u64,
    total: u64,
    caminho: &[[u8; HASH_LEN]],
    raiz: &[u8; HASH_LEN],
) -> bool {
    // Com `u64`, isso também cobre `total < 1`.
    if indice >= total {
        return false;
    }

    // A divisão da árvore só é conhecida de cima para baixo, e o caminho vem
    // de baixo para cima. Então a descida é feita primeiro, registrando de
    // que lado a folha cai em cada nível.
    let mut a_esquerda: Vec<bool> = Vec::new();
    let (mut idx, mut tamanho) = (indice, total);
    while let Some(k) = ponto_de_divisao_u64(tamanho) {
        match idx.checked_sub(k) {
            None => {
                a_esquerda.push(true);
                tamanho = k;
            }
            Some(na_direita) => {
                a_esquerda.push(false);
                idx = na_direita;
                let Some(restante) = tamanho.checked_sub(k) else {
                    return false;
                };
                tamanho = restante;
            }
        }
    }

    if a_esquerda.len() != caminho.len() {
        return false;
    }

    let calculada = a_esquerda
        .iter()
        .rev()
        .zip(caminho)
        .fold(merkle_leaf_hash(folha), |atual, (&esquerda, irmao)| {
            if esquerda {
                no(&atual, irmao)
            } else {
                no(irmao, &atual)
            }
        });
    &calculada == raiz
}

#[cfg(test)]
mod testes {
    use super::*;

    fn folhas(n: usize) -> Vec<Vec<u8>> {
        (0..n).map(|i| format!("folha-{i}").into_bytes()).collect()
    }

    #[test]
    fn inteiros_sao_big_endian() {
        let mut w = Writer::new();
        w.u8(0);
        w.u8(255);
        w.u32(1);
        w.u64(1);
        w.u16(0x0102);
        assert_eq!(
            w.into_bytes(),
            [&[0x00, 0xff][..], &[0, 0, 0, 1], &[0, 0, 0, 0, 0, 0, 0, 1], &[1, 2]].concat()
        );
    }

    #[test]
    fn ida_e_volta() {
        let mut w = Writer::new();
        w.u64(1 << 63);
        w.var_bytes(&[0, 1, 2]).unwrap();
        w.string("acentuacao: ção").unwrap();
        w.fixed(&[b'A'; 20]);
        w.list(&[1_u32, 2, 3], |w, v| {
            w.u32(*v);
            Ok::<(), CodecError>(())
        })
        .unwrap();
        let blob = w.into_bytes();

        let mut r = Reader::new(&blob);
        assert_eq!(r.u64().unwrap(), 1 << 63);
        assert_eq!(r.var_bytes().unwrap(), &[0, 1, 2]);
        assert_eq!(r.string().unwrap(), "acentuacao: ção");
        assert_eq!(r.fixed::<20>().unwrap(), [b'A'; 20]);
        assert_eq!(r.read_list(|rd| rd.u32()).unwrap(), vec![1, 2, 3]);
        r.finish().unwrap();
    }

    #[test]
    fn sobra_de_bytes_recusada() {
        let mut entrada = 7_u32.to_be_bytes().to_vec();
        entrada.extend_from_slice(b"sobra");
        let mut r = Reader::new(&entrada);
        assert_eq!(r.u32().unwrap(), 7);
        assert_eq!(r.finish(), Err(CodecError::BytesSobrando { quantidade: 5 }));
    }

    #[test]
    fn entrada_truncada_recusada() {
        let erro = Reader::new(&[0, 0]).u32().unwrap_err();
        assert_eq!(erro, CodecError::FimInesperado { pedidos: 4, restantes: 2 });
        assert_eq!(erro.to_string(), "fim inesperado: pediu 4 bytes, restam 2");

        // Prefixo de tamanho que mente sobre o conteúdo.
        let mut entrada = 1000_u32.to_be_bytes().to_vec();
        entrada.extend_from_slice(b"curto");
        let erro = Reader::new(&entrada).var_bytes().unwrap_err();
        assert_eq!(erro, CodecError::FimInesperado { pedidos: 1000, restantes: 5 });
    }

    #[test]
    fn falha_nao_avanca_o_leitor() {
        let mut r = Reader::new(&[1, 2, 3]);
        assert!(r.u32().is_err());
        assert_eq!(r.remaining(), 3);
        assert_eq!(r.take(3).unwrap(), &[1, 2, 3]);
    }

    #[test]
    fn prefixo_de_4_gib_nao_aloca() {
        // Se o leitor alocasse a partir do prefixo, este teste pediria 4 GiB.
        let erro = Reader::new(&[0xff, 0xff, 0xff, 0xff, 0, 0, 0])
            .var_bytes()
            .unwrap_err();
        assert_eq!(
            erro,
            CodecError::FimInesperado { pedidos: 4_294_967_295, restantes: 3 }
        );

        // O mesmo para uma lista que declara 2^32-1 itens sem trazer nenhum.
        let erro = Reader::new(&[0xff, 0xff, 0xff, 0xff])
            .read_list(|rd| rd.u32())
            .unwrap_err();
        assert_eq!(erro, CodecError::FimInesperado { pedidos: 4, restantes: 0 });
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn prefixo_acima_de_u32_e_recusado() {
        let acima = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
        assert_eq!(
            prefixo(acima, CodecError::BytesLongosDemais),
            Err(CodecError::BytesLongosDemais)
        );
        assert_eq!(prefixo(acima - 1, CodecError::BytesLongosDemais), Ok(u32::MAX));
    }

    #[test]
    fn folha_nao_colide_com_no() {
        assert_eq!(merkle_root(&[b"a"]), merkle_leaf_hash(b"a"));
        assert_ne!(merkle_root(&[b"a"]), merkle_root(&[b"a", b"a"]));
        assert_eq!(merkle_root::<&[u8]>(&[]), sha512(b""));
    }

    #[test]
    fn ordem_importa() {
        assert_ne!(merkle_root(&[b"a", b"b"]), merkle_root(&[b"b", b"a"]));
    }

    #[test]
    fn sem_colisao_por_duplicar_o_ultimo() {
        // CVE-2012-2459: com a regra do Bitcoin, [a,b,c] e [a,b,c,c] dariam a
        // mesma raiz.
        assert_ne!(merkle_root(&[b"a", b"b", b"c"]), merkle_root(&[b"a", b"b", b"c", b"c"]));
        assert_ne!(merkle_root(&[b"a", b"b"]), merkle_root(&[b"a", b"b", b"a", b"b"]));
    }

    #[test]
    fn provas_de_inclusao_de_1_a_17() {
        for total in 1..18_usize {
            let lista = folhas(total);
            let raiz = merkle_root(&lista);
            let total_u64 = u64::try_from(total).unwrap();
            for (i, folha) in lista.iter().enumerate() {
                let caminho = merkle_path(&lista, i).unwrap();
                let i_u64 = u64::try_from(i).unwrap();
                assert!(
                    merkle_verify_path(folha, i_u64, total_u64, &caminho, &raiz),
                    "prova válida falhou (total={total}, i={i})"
                );
                assert!(
                    !merkle_verify_path(b"impostora", i_u64, total_u64, &caminho, &raiz),
                    "folha falsa passou (total={total}, i={i})"
                );
                if total > 1 {
                    let outro = (i_u64 + 1) % total_u64;
                    assert!(
                        !merkle_verify_path(folha, outro, total_u64, &caminho, &raiz),
                        "índice errado passou (total={total}, i={i})"
                    );
                }
            }
        }
    }

    #[test]
    fn divisao_igual_ao_laco_do_python() {
        fn laco_do_python(n: usize) -> usize {
            let mut k = 1;
            while k * 2 < n {
                k *= 2;
            }
            k
        }
        assert_eq!(ponto_de_divisao(0), None);
        assert_eq!(ponto_de_divisao(1), None);
        for n in 2..5000 {
            assert_eq!(ponto_de_divisao(n), Some(laco_do_python(n)), "n = {n}");
        }
        assert_eq!(ponto_de_divisao(usize::MAX), Some(1 << (usize::BITS - 1)));
        assert_eq!(ponto_de_divisao_u64(u64::MAX), Some(1 << 63));
    }

    #[test]
    fn verificacao_no_teto_do_u64_nao_estoura() {
        let raiz = merkle_root(&folhas(5));
        assert!(!merkle_verify_path(b"x", u64::MAX - 1, u64::MAX, &[], &raiz));
        assert!(!merkle_verify_path(b"x", u64::MAX, u64::MAX, &[raiz; 64], &raiz));
        assert!(!merkle_verify_path(b"x", 0, 0, &[], &raiz));
    }

    #[test]
    fn indice_fora_da_faixa() {
        assert_eq!(merkle_path(&folhas(0), 0), Err(CodecError::IndiceForaDaFaixa));
        assert_eq!(merkle_path(&folhas(3), 3), Err(CodecError::IndiceForaDaFaixa));
        assert!(merkle_path(&folhas(1), 0).unwrap().is_empty());
    }
}
