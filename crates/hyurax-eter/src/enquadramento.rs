//! Moldura para meios que são um fio contínuo de bytes: som, Bluetooth serial,
//! rádio. Num fio assim não existe "um pacote": chega um rio de bytes, com
//! ruído no meio e sem aviso de onde um quadro começa. A moldura resolve isso:
//!
//! ```text
//! sincronia(4)  tamanho(2, big-endian)  quadro(tamanho)  CRC-32(4, big-endian)
//! E7 3C 5A C3
//! ```
//!
//! O CRC-32 cobre `tamanho + quadro`. Ele não é segurança (quem garante o
//! conteúdo é a prova de Merkle do próprio quadro do Éter); ele só separa o que
//! o fio estragou do que chegou inteiro, para o lixo nem chegar ao montador.

/// Quatro bytes que marcam o começo de um quadro no fio.
pub const SINCRONIA: [u8; 4] = [0xE7, 0x3C, 0x5A, 0xC3];

/// Bytes da moldura além do quadro: sincronia, tamanho e CRC.
pub const CUSTO_DA_MOLDURA: usize = 4 + 2 + 4;

/// Maior quadro que cabe numa moldura.
pub const QUADRO_MAX: usize = u16::MAX as usize;

const TABELA: [u32; 256] = tabela_crc();

// Laços com limites fixos (256 e 8): o índice nunca sai da tabela.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn tabela_crc() -> [u32; 256] {
    let mut tabela = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 == 1 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        tabela[i] = c;
        i += 1;
    }
    tabela
}

/// CRC-32 (IEEE 802.3, o mesmo do zip e do Ethernet).
pub fn crc32(dados: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in dados {
        let i = usize::from((c as u8) ^ b);
        c = TABELA.get(i).copied().unwrap_or(0) ^ (c >> 8);
    }
    !c
}

/// Põe a moldura em volta de um quadro. `None` se o quadro passa de [`QUADRO_MAX`].
pub fn emoldurar(quadro: &[u8]) -> Option<Vec<u8>> {
    let tamanho = u16::try_from(quadro.len()).ok()?;
    let mut saida = Vec::with_capacity(quadro.len().saturating_add(CUSTO_DA_MOLDURA));
    saida.extend_from_slice(&SINCRONIA);
    saida.extend_from_slice(&tamanho.to_be_bytes());
    saida.extend_from_slice(quadro);
    let crc = crc32(saida.get(SINCRONIA.len()..).unwrap_or_default());
    saida.extend_from_slice(&crc.to_be_bytes());
    Some(saida)
}

/// Tira quadros inteiros de um fio de bytes que chega aos poucos.
///
/// Aceita o fio em qualquer tamanho de pedaço, com lixo antes, no meio e
/// depois. Quadro com CRC errado é jogado fora e a busca continua no byte
/// seguinte, então um quadro estragado não leva o vizinho junto.
#[derive(Debug)]
pub struct Desemoldurador {
    fila: Vec<u8>,
    limite: usize,
    descartados: u64,
}

impl Desemoldurador {
    /// Aceita quadros de até `limite` bytes; maiores são tratados como lixo.
    pub fn novo(limite: usize) -> Self {
        Self {
            fila: Vec::new(),
            limite: limite.min(QUADRO_MAX),
            descartados: 0,
        }
    }

    /// Quantos candidatos a quadro foram jogados fora por CRC ou tamanho.
    pub fn descartados(&self) -> u64 {
        self.descartados
    }

    /// Junta mais bytes do fio e devolve os quadros que ficaram completos.
    pub fn empurrar(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.fila.extend_from_slice(bytes);
        let mut prontos = Vec::new();
        let mut inicio = 0usize;
        loop {
            let resto = self.fila.get(inicio..).unwrap_or_default();
            let Some(achou) = resto.windows(SINCRONIA.len()).position(|j| j == SINCRONIA) else {
                // Sem sincronia: guarda só a cauda que pode ser começo de uma.
                let guardar = SINCRONIA.len().saturating_sub(1);
                inicio = self.fila.len().saturating_sub(guardar).max(inicio);
                break;
            };
            let comeco = inicio.saturating_add(achou);
            let depois_sinc = comeco.saturating_add(SINCRONIA.len());
            let Some(tam) = self.fila.get(depois_sinc..depois_sinc.saturating_add(2)) else {
                inicio = comeco;
                break;
            };
            let tamanho = usize::from(u16::from_be_bytes([
                tam.first().copied().unwrap_or(0),
                tam.get(1).copied().unwrap_or(0),
            ]));
            if tamanho == 0 || tamanho > self.limite {
                self.descartados = self.descartados.saturating_add(1);
                inicio = comeco.saturating_add(1);
                continue;
            }
            let fim_quadro = depois_sinc.saturating_add(2).saturating_add(tamanho);
            let fim = fim_quadro.saturating_add(4);
            if self.fila.len() < fim {
                inicio = comeco;
                break;
            }
            let coberto = self.fila.get(depois_sinc..fim_quadro).unwrap_or_default();
            let veio = self.fila.get(fim_quadro..fim).unwrap_or_default();
            let confere = veio.len() == 4 && crc32(coberto).to_be_bytes() == veio;
            if confere {
                prontos.push(coberto.get(2..).unwrap_or_default().to_vec());
                inicio = fim;
            } else {
                self.descartados = self.descartados.saturating_add(1);
                inicio = comeco.saturating_add(1);
            }
        }
        self.fila.drain(..inicio.min(self.fila.len()));
        prontos
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod testes {
    use super::*;

    #[test]
    fn crc_do_vetor_classico() {
        // O "check" oficial do CRC-32/ISO-HDLC.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn quadros_saem_inteiros_do_meio_do_lixo_em_qualquer_picote() {
        let a = emoldurar(b"primeiro").unwrap();
        let b = emoldurar(&[0xE7; 300]).unwrap();
        let mut fio = vec![0x00, 0xE7, 0x3C, 0x5A];
        fio.extend(&a);
        fio.extend([0xFF, 0xE7, 0x3C]);
        fio.extend(&b);
        fio.extend([1, 2, 3]);
        for picote in [1, 2, 3, 7, 64, fio.len()] {
            let mut d = Desemoldurador::novo(1000);
            let mut saiu = Vec::new();
            for pedaco in fio.chunks(picote) {
                saiu.extend(d.empurrar(pedaco));
            }
            assert_eq!(saiu, vec![b"primeiro".to_vec(), vec![0xE7; 300]], "picote {picote}");
        }
    }

    #[test]
    fn um_bit_trocado_derruba_so_aquele_quadro() {
        let mut fio = emoldurar(b"estragado").unwrap();
        fio[8] ^= 0x10;
        fio.extend(emoldurar(b"inteiro").unwrap());
        let mut d = Desemoldurador::novo(1000);
        assert_eq!(d.empurrar(&fio), vec![b"inteiro".to_vec()]);
        assert!(d.descartados() >= 1);
    }

    #[test]
    fn tamanho_mentiroso_nao_trava_a_fila() {
        // Sincronia com tamanho enorme: tem de ser descartada, não esperada.
        let mut fio = SINCRONIA.to_vec();
        fio.extend([0xFF, 0xFF]);
        fio.extend(emoldurar(b"depois").unwrap());
        let mut d = Desemoldurador::novo(1000);
        assert_eq!(d.empurrar(&fio), vec![b"depois".to_vec()]);
    }
}
