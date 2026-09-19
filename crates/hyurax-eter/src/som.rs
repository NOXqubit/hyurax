//! Modem de som: bytes viram tons, tons voltam a ser bytes.
//!
//! É o meio que atravessa **o ar e o rádio FM** com o mesmo código. O som que
//! este módulo gera pode sair por um alto-falante e entrar num microfone do
//! outro lado da sala, ou entrar num transmissor FM e sair em qualquer rádio
//! sintonizado. Para o Éter, os dois são o mesmo meio: um fio de áudio.
//!
//! A modulação é FSK de fase contínua, com cada byte em moldura de UART
//! (um bit de partida, oito de dado do menos para o mais significativo, dois de
//! parada). Cada byte ressincroniza o relógio, então a diferença entre o
//! relógio de quem toca e o de quem grava (44,1 kHz contra 48 kHz, cristais
//! imprecisos) não se acumula ao longo do quadro.
//!
//! | Perfil | Velocidade | Tons (marca / espaço) | Para quê |
//! |---|---|---|---|
//! | [`Perfil::FM`] | 1200 bit/s | 1200 / 2200 Hz | Rádio FM e cabo; é o Bell 202 do APRS |
//! | [`Perfil::AR`] | 300 bit/s | 1200 / 1800 Hz | Alto-falante para microfone, ambiente com ruído |
//!
//! Sem biblioteca de áudio: o módulo lê e grava WAV, e tocar ou gravar fica com
//! o sistema (`scripts/eter-tocar.ps1`, `termux-media-player`, gravador do
//! celular). Assim ele roda igual no PC, no celular e no servidor.

use std::f64::consts::TAU;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::enquadramento::{Desemoldurador, emoldurar};
use crate::{EterError, Meio};

/// Como os bits viram som.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perfil {
    /// Bits por segundo.
    pub baud: u32,
    /// Tom do bit 1 (e do silêncio entre bytes), em Hz.
    pub marca_hz: f64,
    /// Tom do bit 0, em Hz.
    pub espaco_hz: f64,
}

impl Perfil {
    /// Bell 202, 1200 bit/s: o padrão do APRS no rádio amador. Para FM e cabo.
    pub const FM: Self = Self {
        baud: 1200,
        marca_hz: 1200.0,
        espaco_hz: 2200.0,
    };

    /// 300 bit/s, tons afastados um múltiplo exato da velocidade. Para o ar.
    pub const AR: Self = Self {
        baud: 300,
        marca_hz: 1200.0,
        espaco_hz: 1800.0,
    };

    /// Procura um perfil pelo nome (`fm` ou `ar`).
    pub fn pelo_nome(nome: &str) -> Option<Self> {
        match nome.to_ascii_lowercase().as_str() {
            "fm" => Some(Self::FM),
            "ar" => Some(Self::AR),
            _ => None,
        }
    }

    /// Bytes por segundo úteis: cada byte custa 11 bits (partida, 8, 2 paradas).
    pub fn bytes_por_segundo(&self) -> u64 {
        u64::from(self.baud) / BITS_POR_BYTE
    }
}

const BITS_POR_BYTE: u64 = 11;
/// Taxa de amostragem do que este módulo gera.
pub const TAXA_PADRAO: u32 = 48_000;
const AMPLITUDE: f64 = 0.6;
/// Marca antes do primeiro byte: dá tempo ao controle de volume de quem grava.
const PREAMBULO_S: f64 = 0.35;
/// Marca depois do último byte.
const CAUDA_S: f64 = 0.10;
/// Silêncio nas pontas, com rampa, para não estalar no alto-falante.
const SILENCIO_S: f64 = 0.15;
const RAMPA_S: f64 = 0.01;

// ---------------------------------------------------------------------------
// Modular
// ---------------------------------------------------------------------------

/// Transforma bytes em amostras de áudio (de -1 a 1) na `taxa` pedida.
pub fn modular(bytes: &[u8], perfil: Perfil, taxa: u32) -> Vec<f32> {
    let mut bits: Vec<bool> = Vec::with_capacity(bytes.len().saturating_mul(11));
    for byte in bytes {
        bits.push(false);
        for k in 0..8 {
            bits.push((byte >> k) & 1 == 1);
        }
        bits.push(true);
        bits.push(true);
    }
    let taxa_f = f64::from(taxa.max(1));
    let por_bit = taxa_f / f64::from(perfil.baud.max(1));
    let preambulo = (PREAMBULO_S * taxa_f) as usize;
    let cauda = (CAUDA_S * taxa_f) as usize;
    let silencio = (SILENCIO_S * taxa_f) as usize;
    let rampa = ((RAMPA_S * taxa_f) as usize).max(1);

    let mut saida: Vec<f32> = vec![0.0; silencio];
    let mut fase = 0.0f64;
    let mut tom = |saida: &mut Vec<f32>, hz: f64, n: usize| {
        let passo = TAU * hz / taxa_f;
        for _ in 0..n {
            saida.push((AMPLITUDE * fase.sin()) as f32);
            fase = (fase + passo) % TAU;
        }
    };
    let comeco_tom = saida.len();
    tom(&mut saida, perfil.marca_hz, preambulo);
    // Cada bit ocupa o intervalo [round(k·T), round((k+1)·T)): sem erro acumulado.
    let mut feito = 0usize;
    for (k, bit) in bits.iter().enumerate() {
        let fim = ((k as f64 + 1.0) * por_bit).round() as usize;
        let n = fim.saturating_sub(feito);
        feito = fim;
        tom(&mut saida, if *bit { perfil.marca_hz } else { perfil.espaco_hz }, n);
    }
    tom(&mut saida, perfil.marca_hz, cauda);
    let fim_tom = saida.len();
    // Rampas de entrada e saída do tom.
    for i in 0..rampa {
        let g = i as f32 / rampa as f32;
        if let Some(a) = saida.get_mut(comeco_tom.saturating_add(i)) {
            *a *= g;
        }
        if let Some(a) = fim_tom.checked_sub(i.saturating_add(1)).and_then(|j| saida.get_mut(j)) {
            *a *= g;
        }
    }
    saida.extend(std::iter::repeat_n(0.0, silencio));
    saida
}

// ---------------------------------------------------------------------------
// Demodular
// ---------------------------------------------------------------------------

/// Correlação de um tom sobre uma janela, com somas acumuladas: a energia de
/// qualquer janela sai em tempo constante, sem depender da fase de quem tocou.
struct Tom {
    cos: Vec<f64>,
    sen: Vec<f64>,
}

impl Tom {
    fn novo(amostras: &[f32], hz: f64, taxa: f64) -> Self {
        let passo = TAU * hz / taxa;
        let mut cos = Vec::with_capacity(amostras.len().saturating_add(1));
        let mut sen = Vec::with_capacity(amostras.len().saturating_add(1));
        let (mut c, mut s) = (0.0f64, 0.0f64);
        cos.push(0.0);
        sen.push(0.0);
        for (i, x) in amostras.iter().enumerate() {
            // Fase recomputada pelo índice: nada de erro acumulado em gravação longa.
            let fase = (passo * i as f64) % TAU;
            c += f64::from(*x) * fase.cos();
            s += f64::from(*x) * fase.sin();
            cos.push(c);
            sen.push(s);
        }
        Self { cos, sen }
    }

    fn energia(&self, de: usize, ate: usize) -> f64 {
        let c = self.cos.get(ate).copied().unwrap_or(0.0) - self.cos.get(de).copied().unwrap_or(0.0);
        let s = self.sen.get(ate).copied().unwrap_or(0.0) - self.sen.get(de).copied().unwrap_or(0.0);
        c * c + s * s
    }
}

/// Transforma áudio de volta em bytes, na ordem em que foram tocados.
///
/// O áudio pode ter sido gravado em outra taxa, com outro volume, com ruído e
/// com silêncio em volta: o que não for byte bem formado é ignorado. Os bytes
/// saem crus; quem separa os quadros é a moldura (e o CRC dela).
pub fn demodular(amostras: &[f32], perfil: Perfil, taxa: u32) -> Vec<u8> {
    let taxa_f = f64::from(taxa.max(1));
    let por_bit = taxa_f / f64::from(perfil.baud.max(1));
    let janela = (por_bit.round() as usize).max(2);
    let n = amostras.len();
    if n <= janela.saturating_mul(12) {
        return Vec::new();
    }
    let meia = janela / 2;

    // Discriminador por amostra: +1 é marca pura, -1 é espaço puro. Calculado
    // em blocos, para uma gravação longa não virar centenas de MB de somas.
    let mut disc: Vec<f32> = vec![0.0; n];
    let mut forca: Vec<f32> = vec![0.0; n];
    let bloco = 1usize << 16;
    let mut b = 0usize;
    while b < n {
        // O bloco cobre as janelas centradas em [b, b + bloco).
        let de_bloco = b.saturating_sub(meia);
        let ate_bloco = b.saturating_add(bloco).saturating_add(janela).min(n);
        let trecho = amostras.get(de_bloco..ate_bloco).unwrap_or_default();
        let marca = Tom::novo(trecho, perfil.marca_hz, taxa_f);
        let espaco = Tom::novo(trecho, perfil.espaco_hz, taxa_f);
        let ultimo = b.saturating_add(bloco).min(n.saturating_sub(janela.saturating_sub(meia)));
        for i in b.max(meia)..ultimo {
            let de = i.saturating_sub(meia).saturating_sub(de_bloco);
            let ate = de.saturating_add(janela);
            let m = marca.energia(de, ate);
            let e = espaco.energia(de, ate);
            let soma = m + e;
            if let Some(d) = disc.get_mut(i) {
                *d = if soma > 0.0 { ((m - e) / soma) as f32 } else { 0.0 };
            }
            if let Some(f) = forca.get_mut(i) {
                *f = soma as f32;
            }
        }
        b = b.saturating_add(bloco);
    }
    // Há sinal onde a energia dos tons passa de 1/1000 da maior (30 dB abaixo).
    let pico = forca.iter().copied().fold(0.0f32, f32::max);
    let piso = pico * 1e-3;
    let tem_sinal = |i: usize| forca.get(i).is_some_and(|f| *f > piso);
    let ler = |i: f64| -> Option<f32> {
        let j = i.round();
        if j < 0.0 {
            return None;
        }
        disc.get(j as usize).copied()
    };

    let mut bytes = Vec::new();
    let mut i = 1usize;
    let limiar = 0.1f32;
    while i < n {
        let antes = disc.get(i.saturating_sub(1)).copied().unwrap_or(0.0);
        let agora = disc.get(i).copied().unwrap_or(0.0);
        // Partida: a marca (positivo) cai para espaço (negativo), com sinal presente.
        if !(antes > 0.0 && agora <= 0.0 && tem_sinal(i)) {
            i = i.saturating_add(1);
            continue;
        }
        let borda = i as f64;
        let meio = |k: f64| borda + (k + 0.5) * por_bit;
        let partida_ok = ler(meio(0.0)).is_some_and(|d| d < -limiar);
        let parada_ok = ler(meio(9.0)).is_some_and(|d| d > limiar);
        if !(partida_ok && parada_ok) {
            i = i.saturating_add(1);
            continue;
        }
        let mut byte = 0u8;
        let mut duvida = false;
        for k in 0..8u32 {
            match ler(meio(f64::from(k) + 1.0)) {
                Some(d) if d > 0.0 => byte |= 1 << k,
                Some(_) => {}
                None => duvida = true,
            }
        }
        if duvida {
            break;
        }
        bytes.push(byte);
        // Continua a procurar a próxima partida a partir do meio da primeira parada.
        i = meio(9.0).round() as usize;
    }
    bytes
}

// ---------------------------------------------------------------------------
// WAV
// ---------------------------------------------------------------------------

/// Grava áudio mono em WAV PCM de 16 bits.
pub fn gravar_wav(caminho: impl AsRef<Path>, amostras: &[f32], taxa: u32) -> Result<(), EterError> {
    fs::write(caminho, wav_em_bytes(amostras, taxa)).map_err(falha)
}

/// O WAV PCM de 16 bits, mono, em memória.
pub fn wav_em_bytes(amostras: &[f32], taxa: u32) -> Vec<u8> {
    let dados = u32::try_from(amostras.len().saturating_mul(2)).unwrap_or(u32::MAX);
    let mut w = Vec::with_capacity(44usize.saturating_add(dados as usize));
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&dados.saturating_add(36).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&taxa.to_le_bytes());
    w.extend_from_slice(&taxa.saturating_mul(2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&dados.to_le_bytes());
    for a in amostras {
        let v = (f64::from(*a).clamp(-1.0, 1.0) * 32767.0).round() as i16;
        w.extend_from_slice(&v.to_le_bytes());
    }
    w
}

/// Lê um WAV: PCM de 8, 16, 24 ou 32 bits, ou ponto flutuante de 32 bits.
///
/// Com mais de um canal, usa o primeiro. Devolve as amostras e a taxa.
pub fn ler_wav(caminho: impl AsRef<Path>) -> Result<(Vec<f32>, u32), EterError> {
    let bytes = fs::read(caminho).map_err(falha)?;
    wav_de_bytes(&bytes)
}

/// Mesmo que [`ler_wav`], a partir dos bytes do arquivo.
pub fn wav_de_bytes(bytes: &[u8]) -> Result<(Vec<f32>, u32), EterError> {
    let ruim = |m: &str| EterError::Meio(format!("WAV inválido: {m}"));
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(ruim("sem cabeçalho RIFF/WAVE"));
    }
    let u16_em = |i: usize| {
        bytes
            .get(i..i.saturating_add(2))
            .and_then(|b| <[u8; 2]>::try_from(b).ok())
            .map(u16::from_le_bytes)
    };
    let u32_em = |i: usize| {
        bytes
            .get(i..i.saturating_add(4))
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map(u32::from_le_bytes)
    };
    let mut pos = 12usize;
    let mut formato: Option<(u16, u16, u32, u16)> = None;
    while let (Some(id), Some(tam)) = (bytes.get(pos..pos.saturating_add(4)), u32_em(pos.saturating_add(4))) {
        let corpo = pos.saturating_add(8);
        let tam = tam as usize;
        if id == b"fmt " {
            let codigo = u16_em(corpo).ok_or_else(|| ruim("fmt curto"))?;
            let canais = u16_em(corpo.saturating_add(2)).ok_or_else(|| ruim("fmt curto"))?;
            let taxa = u32_em(corpo.saturating_add(4)).ok_or_else(|| ruim("fmt curto"))?;
            let bits = u16_em(corpo.saturating_add(14)).ok_or_else(|| ruim("fmt curto"))?;
            // WAVE_FORMAT_EXTENSIBLE: o formato de verdade está no subtipo.
            let codigo = if codigo == 0xFFFE {
                u16_em(corpo.saturating_add(24)).ok_or_else(|| ruim("fmt extensível curto"))?
            } else {
                codigo
            };
            formato = Some((codigo, canais, taxa, bits));
        } else if id == b"data" {
            let (codigo, canais, taxa, bits) = formato.ok_or_else(|| ruim("data antes de fmt"))?;
            let fim = corpo.saturating_add(tam).min(bytes.len());
            let dados = bytes.get(corpo..fim).unwrap_or_default();
            let largura = usize::from(bits / 8);
            let passo = largura.saturating_mul(usize::from(canais.max(1)));
            if largura == 0 || passo == 0 {
                return Err(ruim("bits por amostra"));
            }
            let mut amostras = Vec::with_capacity(dados.len().checked_div(passo).unwrap_or(0));
            for quadro in dados.chunks_exact(passo) {
                // Até 4 bytes da amostra, completados com zero.
                let mut a = [0u8; 4];
                for (d, o) in a.iter_mut().zip(quadro.iter().take(largura)) {
                    *d = *o;
                }
                let [b0, b1, b2, _] = a;
                let v = match (codigo, bits) {
                    (1, 8) => (f32::from(b0) - 128.0) / 128.0,
                    (1, 16) => f32::from(i16::from_le_bytes([b0, b1])) / 32768.0,
                    (1, 24) => (i32::from_le_bytes([0, b0, b1, b2]) >> 8) as f32 / 8_388_608.0,
                    (1, 32) => i32::from_le_bytes(a) as f32 / 2_147_483_648.0,
                    (3, 32) => f32::from_le_bytes(a),
                    _ => return Err(ruim("formato de amostra não suportado")),
                };
                amostras.push(v);
            }
            return Ok((amostras, taxa));
        }
        // Blocos têm tamanho par: o byte de enchimento conta.
        pos = corpo.saturating_add(tam).saturating_add(tam & 1);
    }
    Err(ruim("sem bloco data"))
}

fn falha(e: impl core::fmt::Display) -> EterError {
    EterError::Meio(e.to_string())
}

// ---------------------------------------------------------------------------
// O meio
// ---------------------------------------------------------------------------

/// O Éter pelo som: FM, cabo de áudio ou o ar entre um alto-falante e um microfone.
///
/// Enviar junta os quadros num áudio só; [`Meio::descarregar`] grava esse áudio
/// como um WAV na pasta de saída, pronto para tocar. Receber lê os WAV que
/// aparecem na pasta de entrada (gravados do microfone ou do rádio),
/// demodula, separa os quadros e move o arquivo para `lidos`.
pub struct MeioSom {
    nome: String,
    perfil: Perfil,
    taxa: u32,
    mtu: usize,
    saida: PathBuf,
    entrada: PathBuf,
    pendente: Vec<u8>,
    ultimo: Option<PathBuf>,
}

impl MeioSom {
    /// Abre o meio. `mtu` é o maior quadro do Éter por moldura de som.
    pub fn novo(
        nome: &str,
        perfil: Perfil,
        saida: impl AsRef<Path>,
        entrada: impl AsRef<Path>,
        mtu: usize,
    ) -> Result<Self, EterError> {
        let saida = saida.as_ref().to_path_buf();
        let entrada = entrada.as_ref().to_path_buf();
        fs::create_dir_all(&saida).map_err(falha)?;
        fs::create_dir_all(entrada.join("lidos")).map_err(falha)?;
        Ok(Self {
            nome: nome.to_owned(),
            perfil,
            taxa: TAXA_PADRAO,
            mtu: mtu.min(crate::enquadramento::QUADRO_MAX),
            saida,
            entrada,
            pendente: Vec::new(),
            ultimo: None,
        })
    }

    /// O último WAV gravado por [`Meio::descarregar`], para tocar.
    pub fn ultimo_wav(&self) -> Option<&Path> {
        self.ultimo.as_deref()
    }
}

impl Meio for MeioSom {
    fn nome(&self) -> &str {
        &self.nome
    }

    fn mtu(&self) -> usize {
        self.mtu
    }

    fn vazao(&self) -> u64 {
        self.perfil.bytes_por_segundo()
    }

    fn enviar(&mut self, quadro: &[u8]) -> Result<(), EterError> {
        if quadro.len() > self.mtu {
            return Err(EterError::MeioPequenoDemais {
                mtu: self.mtu,
                preciso: quadro.len(),
            });
        }
        let emoldurado = emoldurar(quadro).ok_or(EterError::MeioPequenoDemais {
            mtu: self.mtu,
            preciso: quadro.len(),
        })?;
        self.pendente.extend_from_slice(&emoldurado);
        Ok(())
    }

    fn descarregar(&mut self) -> Result<(), EterError> {
        if self.pendente.is_empty() {
            return Ok(());
        }
        let audio = modular(&self.pendente, self.perfil, self.taxa);
        let agora = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let arquivo = self.saida.join(format!("eter-{agora}.wav"));
        gravar_wav(&arquivo, &audio, self.taxa)?;
        self.pendente.clear();
        self.ultimo = Some(arquivo);
        Ok(())
    }

    fn receber(&mut self) -> Result<Vec<Vec<u8>>, EterError> {
        let lidos = self.entrada.join("lidos");
        let mut arquivos: Vec<PathBuf> = fs::read_dir(&self.entrada)
            .map_err(falha)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")))
            .collect();
        arquivos.sort();
        let mut quadros = Vec::new();
        for arquivo in arquivos {
            let Ok((amostras, taxa)) = ler_wav(&arquivo) else {
                continue; // ainda sendo gravado, ou não é WAV: tenta na próxima
            };
            let bytes = demodular(&amostras, self.perfil, taxa);
            let mut d = Desemoldurador::novo(self.mtu);
            quadros.extend(d.empurrar(&bytes));
            if let Some(nome) = arquivo.file_name() {
                let _ = fs::rename(&arquivo, lidos.join(nome));
            }
        }
        Ok(quadros)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]
mod testes {
    use super::*;

    fn bytes(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 37 % 256) as u8).collect()
    }

    /// Reamostra por interpolação linear: simula gravar noutra taxa, com o
    /// relógio de quem grava um pouco adiantado.
    fn reamostrar(x: &[f32], de: u32, para: u32, desvio_ppm: f64) -> Vec<f32> {
        let razao = f64::from(de) / f64::from(para) * (1.0 + desvio_ppm * 1e-6);
        let mut y = Vec::new();
        let mut t = 0.0f64;
        while (t as usize) + 1 < x.len() {
            let i = t as usize;
            let f = (t - i as f64) as f32;
            y.push(x[i] * (1.0 - f) + x[i + 1] * f);
            t += razao;
        }
        y
    }

    /// Ruído pseudoaleatório determinístico (xorshift), de -1 a 1.
    fn ruido(n: usize, semente: u64) -> Vec<f32> {
        let mut s = semente;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 40) as f32 / (1u64 << 23) as f32 - 1.0
            })
            .collect()
    }

    #[test]
    fn ida_e_volta_limpa_nos_dois_perfis() {
        let dados = bytes(200);
        for perfil in [Perfil::FM, Perfil::AR] {
            let audio = modular(&dados, perfil, TAXA_PADRAO);
            assert_eq!(demodular(&audio, perfil, TAXA_PADRAO), dados, "{perfil:?}");
        }
    }

    #[test]
    fn sobrevive_a_outra_taxa_relogio_torto_volume_baixo_e_ruido() {
        let dados = bytes(300);
        let quadro = emoldurar(&dados).unwrap();
        let audio = modular(&quadro, Perfil::FM, 48_000);
        // Gravado a 44,1 kHz, com o relógio 300 ppm fora, a 20% do volume e
        // com ruído branco de 1/3 da amplitude do sinal.
        let mut gravado = reamostrar(&audio, 48_000, 44_100, 300.0);
        let r = ruido(gravado.len(), 7);
        for (a, n) in gravado.iter_mut().zip(r) {
            *a = *a * 0.2 + n * 0.04;
        }
        let bytes = demodular(&gravado, Perfil::FM, 44_100);
        let mut d = Desemoldurador::novo(1000);
        assert_eq!(d.empurrar(&bytes), vec![dados]);
    }

    #[test]
    fn wav_ida_e_volta() {
        let audio = modular(b"ola", Perfil::FM, 22_050);
        let (lido, taxa) = wav_de_bytes(&wav_em_bytes(&audio, 22_050)).unwrap();
        assert_eq!(taxa, 22_050);
        assert_eq!(lido.len(), audio.len());
        assert_eq!(demodular(&lido, Perfil::FM, taxa), b"ola");
    }

    #[test]
    fn wav_estereo_de_24_bits_usa_o_primeiro_canal() {
        let audio = modular(b"xy", Perfil::FM, 8_000);
        let mut w = Vec::new();
        let dados = (audio.len() * 6) as u32;
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(dados + 36).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&8_000u32.to_le_bytes());
        w.extend_from_slice(&48_000u32.to_le_bytes());
        w.extend_from_slice(&6u16.to_le_bytes());
        w.extend_from_slice(&24u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&dados.to_le_bytes());
        for a in &audio {
            let v = (a * 8_000_000.0) as i32;
            let b = v.to_le_bytes();
            w.extend_from_slice(&b[..3]);
            w.extend_from_slice(&[0, 0, 0]); // o segundo canal é silêncio
        }
        let (lido, taxa) = wav_de_bytes(&w).unwrap();
        assert_eq!(demodular(&lido, Perfil::FM, taxa), b"xy");
    }
}
