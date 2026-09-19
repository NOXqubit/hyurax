// ✝ Isaías 52:7 — “Como são formosos sobre os montes os pés do que anuncia as boas novas.”
//! Auron — transporte por qualquer meio (`AURON-TRANSPORTE-v1`).
//!
//! Esta camada existe para uma frase: **o Auron define a mensagem, o meio é
//! trocável** (documento mestre, §24 e §68). O nó não fala "Wi-Fi" nem
//! "Bluetooth": ele entrega bytes a um [`Meio`], e o mesmo objeto pode sair
//! pedaço a pedaço por meios diferentes, ao mesmo tempo.
//!
//! Um objeto (um bloco, um arquivo, uma foto) vira:
//!
//! ```text
//! Manifesto   quem é o objeto: identidade, tamanho, quantos pedaços, raiz de Merkle
//! Fragmento   um pedaço numerado, com a prova de que pertence àquela raiz
//! Faltando    a lista do que ainda não chegou, para pedir de novo
//! ```
//!
//! Três propriedades sustentam o resto:
//!
//! 1. **Cada fragmento se prova sozinho.** Com a raiz do manifesto, um pedaço
//!    que chegou pelo Bluetooth é conferido sem depender dos que vieram pelo
//!    Wi-Fi. Lixo é descartado na hora, não no fim.
//! 2. **A identidade é o conteúdo.** O `id` é o SHA-512 do objeto inteiro, e a
//!    montagem só termina se o que foi remontado tiver aquele hash. Mentira na
//!    raiz ou no total não passa: a conta final não fecha.
//! 3. **A ordem não importa.** Fragmentos chegam fora de ordem, repetidos, por
//!    caminhos diferentes, com horas de diferença. O [`Montador`] aceita o que
//!    é novo e válido, ignora o resto e sabe dizer o que falta.
//!
//! O que esta camada **não** faz: não decide rota, não guarda fila entre
//! execuções e não conhece socket nenhum. Isso é de quem implementa [`Meio`].

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt;

use auron_codec::{
    CodecError, HASH_LEN, Reader, Writer, merkle_raiz_e_caminhos, merkle_verify_path,
};
use auron_crypto::sha512;

pub mod meios;

/// Marca de todo quadro deste transporte.
pub const MAGIC: [u8; 4] = *b"AURT";

/// Versão do formato.
pub const VERSAO: u16 = 1;

const TIPO_MANIFESTO: u8 = 1;
const TIPO_FRAGMENTO: u8 = 2;
const TIPO_FALTANDO: u8 = 3;

/// Profundidade máxima da árvore, e portanto do caminho de prova.
///
/// Um objeto com mais de 2³² pedaços não existe na prática; o limite está aqui
/// para o cálculo do tamanho do pedaço terminar sempre.
const PROFUNDIDADE_MAX: u32 = 32;

/// Bytes de um quadro de fragmento sem contar os dados nem o caminho.
///
/// `magic(4) + versão(2) + tipo(1) + id(64) + índice(4) + tamanho dos dados(4)
/// + tamanho do caminho(4)`.
const CUSTO_DO_QUADRO: usize = 4 + 2 + 1 + HASH_LEN + 4 + 4 + 4;

/// Teto de um objeto: 1 GiB. Acima disso, quem chama deve dividir antes.
pub const TAMANHO_MAX: u64 = 1024 * 1024 * 1024;

/// Identidade de um objeto: o SHA-512 do conteúdo inteiro.
pub type ObjetoId = [u8; HASH_LEN];

/// O que pode dar errado no transporte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransporteError {
    /// Bytes que não são um quadro deste transporte.
    MagicErrado,
    /// Versão que este programa não conhece.
    VersaoDesconhecida(u16),
    /// Tipo de quadro desconhecido.
    TipoDesconhecido(u8),
    /// Erro de codificação vindo do `auron-codec`.
    Codec(CodecError),
    /// O objeto passa do teto.
    ObjetoGrandeDemais(u64),
    /// Objeto vazio: não há o que transportar.
    ObjetoVazio,
    /// O meio não comporta nem um pedaço deste objeto com a prova junto.
    MeioPequenoDemais {
        /// Quantos bytes o meio aceita por quadro.
        mtu: usize,
        /// Quantos bytes seriam necessários.
        preciso: usize,
    },
    /// Manifesto incoerente: a quantidade não bate com tamanho e pedaço.
    ManifestoIncoerente,
    /// O fragmento é de outro objeto.
    OutroObjeto,
    /// Índice além da quantidade declarada.
    IndiceForaDaFaixa(u32),
    /// O pedaço não bate com a raiz de Merkle do manifesto.
    ProvaInvalida(u32),
    /// Pedaço com tamanho errado para a posição que diz ocupar.
    TamanhoDoPedaco {
        /// O índice do pedaço.
        indice: u32,
        /// Quantos bytes vieram.
        veio: usize,
        /// Quantos bytes eram esperados.
        esperado: usize,
    },
    /// Ainda falta pedaço para montar.
    Incompleto {
        /// Quantos já chegaram.
        tenho: u32,
        /// Quantos são no total.
        total: u32,
    },
    /// O objeto remontado não tem o hash prometido.
    HashNaoConfere,
    /// Falha do meio (disco, rádio, socket).
    Meio(String),
}

impl fmt::Display for TransporteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MagicErrado => write!(f, "não é um quadro do transporte Auron"),
            Self::VersaoDesconhecida(v) => write!(f, "versão {v} desconhecida"),
            Self::TipoDesconhecido(t) => write!(f, "tipo de quadro {t} desconhecido"),
            Self::Codec(e) => write!(f, "{e}"),
            Self::ObjetoGrandeDemais(n) => write!(f, "objeto de {n} bytes passa do teto"),
            Self::ObjetoVazio => write!(f, "objeto vazio"),
            Self::MeioPequenoDemais { mtu, preciso } => {
                write!(f, "o meio leva {mtu} bytes por vez e seriam precisos {preciso}")
            }
            Self::ManifestoIncoerente => write!(f, "manifesto incoerente"),
            Self::OutroObjeto => write!(f, "fragmento de outro objeto"),
            Self::IndiceForaDaFaixa(i) => write!(f, "pedaço {i} fora da faixa"),
            Self::ProvaInvalida(i) => write!(f, "o pedaço {i} não bate com a raiz"),
            Self::TamanhoDoPedaco { indice, veio, esperado } => {
                write!(f, "pedaço {indice} veio com {veio} bytes e devia ter {esperado}")
            }
            Self::Incompleto { tenho, total } => write!(f, "faltam pedaços: {tenho} de {total}"),
            Self::HashNaoConfere => write!(f, "o objeto remontado não tem o hash prometido"),
            Self::Meio(e) => write!(f, "falha do meio: {e}"),
        }
    }
}

impl core::error::Error for TransporteError {}

impl From<CodecError> for TransporteError {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

// ---------------------------------------------------------------------------
// Manifesto
// ---------------------------------------------------------------------------

/// Quem é o objeto. É a primeira coisa que viaja, e a mais barata: cabe até
/// num quadro de rádio, então serve como o **aviso** de que algo existe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifesto {
    /// SHA-512 do conteúdo inteiro.
    pub id: ObjetoId,
    /// Tamanho em bytes.
    pub tamanho: u64,
    /// Tamanho de cada pedaço; o último pode ser menor.
    pub pedaco: u32,
    /// Quantos pedaços.
    pub quantidade: u32,
    /// Raiz de Merkle dos pedaços (RFC 6962).
    pub raiz: [u8; HASH_LEN],
    /// Nome de origem, se houver. Só informação: nunca vira caminho de arquivo
    /// sem quem recebe decidir.
    pub nome: String,
}

impl Manifesto {
    /// Confere que tamanho, pedaço e quantidade contam a mesma história.
    pub fn coerente(&self) -> bool {
        if self.tamanho == 0 || self.pedaco == 0 || self.quantidade == 0 {
            return false;
        }
        if self.tamanho > TAMANHO_MAX {
            return false;
        }
        let pedaco = u64::from(self.pedaco);
        let esperado = self.tamanho.div_ceil(pedaco);
        esperado == u64::from(self.quantidade)
    }

    /// Quantos bytes tem o pedaço `indice`. O último costuma ser menor.
    pub fn tamanho_do_pedaco(&self, indice: u32) -> Option<usize> {
        if indice >= self.quantidade {
            return None;
        }
        let pedaco = u64::from(self.pedaco);
        let ja = u64::from(indice).checked_mul(pedaco)?;
        let resto = self.tamanho.checked_sub(ja)?;
        usize::try_from(resto.min(pedaco)).ok()
    }

    fn escrever(&self, w: &mut Writer) -> Result<(), TransporteError> {
        w.fixed(&self.id);
        w.u64(self.tamanho);
        w.u32(self.pedaco);
        w.u32(self.quantidade);
        w.fixed(&self.raiz);
        w.string(&self.nome)?;
        Ok(())
    }

    fn ler(r: &mut Reader<'_>) -> Result<Self, TransporteError> {
        Ok(Self {
            id: r.fixed()?,
            tamanho: r.u64()?,
            pedaco: r.u32()?,
            quantidade: r.u32()?,
            raiz: r.fixed()?,
            nome: r.string()?.to_owned(),
        })
    }
}

// ---------------------------------------------------------------------------
// Quadro
// ---------------------------------------------------------------------------

/// Um pedaço numerado, com a prova de que pertence à raiz do manifesto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragmento {
    /// De qual objeto ele é.
    pub id: ObjetoId,
    /// A posição dele, de 0 a `quantidade - 1`.
    pub indice: u32,
    /// Os bytes.
    pub dados: Vec<u8>,
    /// Caminho de auditoria até a raiz, de baixo para cima.
    pub caminho: Vec<[u8; HASH_LEN]>,
}

/// O que viaja pelo meio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Quadro {
    /// O anúncio do objeto.
    Manifesto(Manifesto),
    /// Um pedaço.
    Fragmento(Fragmento),
    /// O que ainda falta, para quem tiver retransmitir.
    Faltando {
        /// De qual objeto.
        id: ObjetoId,
        /// Os índices que faltam.
        indices: Vec<u32>,
    },
}

impl Quadro {
    /// Serializa o quadro para entregar a um meio.
    pub fn codificar(&self) -> Result<Vec<u8>, TransporteError> {
        let mut w = Writer::new();
        w.fixed(&MAGIC);
        w.u16(VERSAO);
        match self {
            Self::Manifesto(m) => {
                w.u8(TIPO_MANIFESTO);
                m.escrever(&mut w)?;
            }
            Self::Fragmento(frag) => {
                w.u8(TIPO_FRAGMENTO);
                w.fixed(&frag.id);
                w.u32(frag.indice);
                w.var_bytes(&frag.dados)?;
                w.list(&frag.caminho, |w, h| {
                    w.fixed(h);
                    Ok::<(), CodecError>(())
                })?;
            }
            Self::Faltando { id, indices } => {
                w.u8(TIPO_FALTANDO);
                w.fixed(id);
                w.list(indices, |w, i| {
                    w.u32(*i);
                    Ok::<(), CodecError>(())
                })?;
            }
        }
        Ok(w.into_bytes())
    }

    /// Lê um quadro que chegou por um meio qualquer.
    ///
    /// Bytes de outro protocolo, ou truncados, viram erro — nunca pânico.
    pub fn decodificar(dados: &[u8]) -> Result<Self, TransporteError> {
        let mut r = Reader::new(dados);
        let magic: [u8; 4] = r.fixed()?;
        if magic != MAGIC {
            return Err(TransporteError::MagicErrado);
        }
        let versao = r.u16()?;
        if versao != VERSAO {
            return Err(TransporteError::VersaoDesconhecida(versao));
        }
        let quadro = match r.u8()? {
            TIPO_MANIFESTO => Self::Manifesto(Manifesto::ler(&mut r)?),
            TIPO_FRAGMENTO => Self::Fragmento(Fragmento {
                id: r.fixed()?,
                indice: r.u32()?,
                dados: r.var_bytes()?.to_vec(),
                caminho: r.read_list(|r| r.fixed::<HASH_LEN>())?,
            }),
            TIPO_FALTANDO => Self::Faltando {
                id: r.fixed()?,
                indices: r.read_list(Reader::u32)?,
            },
            outro => return Err(TransporteError::TipoDesconhecido(outro)),
        };
        r.finish()?;
        Ok(quadro)
    }
}

// ---------------------------------------------------------------------------
// Fatiar
// ---------------------------------------------------------------------------

/// Quantos bytes de dados cabem por fragmento num meio de `mtu` bytes.
///
/// O caminho de prova cresce com o número de pedaços, e o número de pedaços
/// cresce quando o pedaço encolhe. A conta é feita por tentativa, subindo a
/// profundidade até a escolha se sustentar.
pub fn pedaco_para_o_meio(mtu: usize, tamanho: u64) -> Result<u32, TransporteError> {
    if tamanho == 0 {
        return Err(TransporteError::ObjetoVazio);
    }
    if tamanho > TAMANHO_MAX {
        return Err(TransporteError::ObjetoGrandeDemais(tamanho));
    }
    for profundidade in 0..=PROFUNDIDADE_MAX {
        let prova = (profundidade as usize).saturating_mul(HASH_LEN);
        let gasto = CUSTO_DO_QUADRO.saturating_add(prova);
        let Some(espaco) = mtu.checked_sub(gasto).filter(|e| *e > 0) else {
            continue;
        };
        let quantidade = tamanho.div_ceil(espaco as u64);
        if altura_da_arvore(quantidade) <= profundidade {
            let cabe = u32::try_from(espaco).unwrap_or(u32::MAX);
            return Ok(cabe);
        }
    }
    // Nem com a árvore mais rasa o meio comporta um pedaço útil.
    Err(TransporteError::MeioPequenoDemais {
        mtu,
        preciso: CUSTO_DO_QUADRO.saturating_add(1),
    })
}

/// Altura da árvore de Merkle com `folhas` folhas: quantos irmãos o caminho tem.
fn altura_da_arvore(folhas: u64) -> u32 {
    let mut altura: u32 = 0;
    let mut n = 1u64;
    while n < folhas {
        n = n.saturating_mul(2);
        altura = altura.saturating_add(1);
        if altura >= PROFUNDIDADE_MAX {
            break;
        }
    }
    altura
}

/// Corta um objeto em fragmentos prontos para viajar.
///
/// Devolve o manifesto e os fragmentos na ordem, cada um já com a sua prova.
pub fn fatiar(
    dados: &[u8],
    nome: &str,
    pedaco: u32,
) -> Result<(Manifesto, Vec<Fragmento>), TransporteError> {
    if dados.is_empty() {
        return Err(TransporteError::ObjetoVazio);
    }
    let tamanho = u64::try_from(dados.len()).unwrap_or(u64::MAX);
    if tamanho > TAMANHO_MAX {
        return Err(TransporteError::ObjetoGrandeDemais(tamanho));
    }
    let passo = usize::try_from(pedaco).unwrap_or(usize::MAX);
    if passo == 0 {
        return Err(TransporteError::ManifestoIncoerente);
    }

    let folhas: Vec<&[u8]> = dados.chunks(passo).collect();
    let quantidade = u32::try_from(folhas.len()).map_err(|_| TransporteError::ManifestoIncoerente)?;
    // Uma passada só pela árvore: a prova de cada pedaço sai junto com a raiz.
    let (raiz, caminhos) = merkle_raiz_e_caminhos(&folhas);
    let manifesto = Manifesto {
        id: sha512(dados),
        tamanho,
        pedaco,
        quantidade,
        raiz,
        nome: nome.to_owned(),
    };

    let mut fragmentos = Vec::with_capacity(folhas.len());
    for (posicao, pedacinho) in folhas.iter().enumerate() {
        let indice = u32::try_from(posicao).map_err(|_| TransporteError::ManifestoIncoerente)?;
        let caminho = caminhos
            .get(posicao)
            .cloned()
            .ok_or(TransporteError::ManifestoIncoerente)?;
        fragmentos.push(Fragmento {
            id: manifesto.id,
            indice,
            dados: pedacinho.to_vec(),
            caminho,
        });
    }
    Ok((manifesto, fragmentos))
}

// ---------------------------------------------------------------------------
// Montador
// ---------------------------------------------------------------------------

/// Junta os pedaços que chegam, venham de onde vierem, na ordem que vierem.
#[derive(Clone, Debug)]
pub struct Montador {
    manifesto: Manifesto,
    pedacos: Vec<Option<Vec<u8>>>,
    tenho: u32,
}

impl Montador {
    /// Abre a montagem de um objeto anunciado por um manifesto.
    pub fn novo(manifesto: Manifesto) -> Result<Self, TransporteError> {
        if !manifesto.coerente() {
            return Err(TransporteError::ManifestoIncoerente);
        }
        let quantidade =
            usize::try_from(manifesto.quantidade).map_err(|_| TransporteError::ManifestoIncoerente)?;
        Ok(Self {
            manifesto,
            pedacos: vec![None; quantidade],
            tenho: 0,
        })
    }

    /// O manifesto que rege esta montagem.
    pub fn manifesto(&self) -> &Manifesto {
        &self.manifesto
    }

    /// Aceita um fragmento. Devolve `true` se ele era novo.
    ///
    /// Recusa o que é de outro objeto, o que está fora da faixa, o que tem
    /// tamanho errado e o que não bate com a raiz. Repetido não é erro:
    /// devolve `false`, porque em rede com vários meios o mesmo pedaço chega
    /// duas vezes o tempo todo.
    pub fn aceitar(&mut self, fragmento: &Fragmento) -> Result<bool, TransporteError> {
        if fragmento.id != self.manifesto.id {
            return Err(TransporteError::OutroObjeto);
        }
        let posicao = usize::try_from(fragmento.indice)
            .map_err(|_| TransporteError::IndiceForaDaFaixa(fragmento.indice))?;
        let Some(vaga) = self.pedacos.get(posicao) else {
            return Err(TransporteError::IndiceForaDaFaixa(fragmento.indice));
        };
        if vaga.is_some() {
            return Ok(false);
        }
        let esperado = self
            .manifesto
            .tamanho_do_pedaco(fragmento.indice)
            .ok_or(TransporteError::IndiceForaDaFaixa(fragmento.indice))?;
        if fragmento.dados.len() != esperado {
            return Err(TransporteError::TamanhoDoPedaco {
                indice: fragmento.indice,
                veio: fragmento.dados.len(),
                esperado,
            });
        }
        let vale = merkle_verify_path(
            &fragmento.dados,
            u64::from(fragmento.indice),
            u64::from(self.manifesto.quantidade),
            &fragmento.caminho,
            &self.manifesto.raiz,
        );
        if !vale {
            return Err(TransporteError::ProvaInvalida(fragmento.indice));
        }
        if let Some(vaga) = self.pedacos.get_mut(posicao) {
            *vaga = Some(fragmento.dados.clone());
            self.tenho = self.tenho.saturating_add(1);
        }
        Ok(true)
    }

    /// Quantos pedaços já chegaram.
    pub fn tenho(&self) -> u32 {
        self.tenho
    }

    /// Os índices que ainda faltam, em ordem.
    pub fn faltando(&self) -> Vec<u32> {
        self.pedacos
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_none())
            .filter_map(|(i, _)| u32::try_from(i).ok())
            .collect()
    }

    /// Já dá para montar?
    pub fn pronto(&self) -> bool {
        self.tenho == self.manifesto.quantidade
    }

    /// Monta o objeto e confere o hash prometido.
    ///
    /// Esta é a última trava: mesmo que alguém tenha mentido na raiz ou no
    /// total, o conteúdo remontado precisa ter exatamente o `id` do manifesto.
    pub fn montar(&self) -> Result<Vec<u8>, TransporteError> {
        if !self.pronto() {
            return Err(TransporteError::Incompleto {
                tenho: self.tenho,
                total: self.manifesto.quantidade,
            });
        }
        let cabe = usize::try_from(self.manifesto.tamanho).unwrap_or(usize::MAX);
        let mut inteiro = Vec::with_capacity(cabe);
        for pedaco in &self.pedacos {
            match pedaco {
                Some(bytes) => inteiro.extend_from_slice(bytes),
                None => {
                    return Err(TransporteError::Incompleto {
                        tenho: self.tenho,
                        total: self.manifesto.quantidade,
                    });
                }
            }
        }
        if sha512(&inteiro) != self.manifesto.id {
            return Err(TransporteError::HashNaoConfere);
        }
        Ok(inteiro)
    }
}

// ---------------------------------------------------------------------------
// Meio
// ---------------------------------------------------------------------------

/// Um caminho por onde bytes passam: Wi-Fi, Bluetooth, rádio, pendrive, QR.
///
/// O transporte não sabe o que está embaixo. Só precisa saber quanto cabe de
/// cada vez ([`Meio::mtu`]) e como entregar e recolher quadros.
pub trait Meio {
    /// Nome curto, para registro e diagnóstico.
    fn nome(&self) -> &str;

    /// Quantos bytes cabem em um quadro deste meio.
    fn mtu(&self) -> usize;

    /// Entrega um quadro já codificado.
    fn enviar(&mut self, quadro: &[u8]) -> Result<(), TransporteError>;

    /// Recolhe o que chegou desde a última chamada. Vazio é normal.
    fn receber(&mut self) -> Result<Vec<Vec<u8>>, TransporteError>;
}

/// Espalha um objeto pelos meios disponíveis.
///
/// Cada meio faz o que aguenta, e é isso que dá a mistura:
///
/// - **O manifesto vai por todos os meios que o comportem.** Ele é pequeno de
///   propósito: cabe num quadro de rádio, então o aviso "existe este objeto"
///   viaja longe mesmo quando o dado não pode.
/// - **O tamanho do pedaço é ditado pelo maior meio**, para não desperdiçar a
///   banda de quem tem banda.
/// - **Os fragmentos são repartidos em rodízio entre os meios que comportam
///   aquele quadro.** Um pedaço sai pelo Wi-Fi, o seguinte pelo Bluetooth, e
///   quem escuta só um dos dois nunca vê o objeto inteiro.
///
/// Um meio pequeno demais para o dado não é erro: ele fica com o aviso. Erro é
/// quando **nenhum** meio comporta um fragmento.
pub fn espalhar(
    dados: &[u8],
    nome: &str,
    meios: &mut [&mut dyn Meio],
) -> Result<Manifesto, TransporteError> {
    if meios.is_empty() {
        return Err(TransporteError::Meio("nenhum meio disponível".into()));
    }
    let tamanho = u64::try_from(dados.len()).unwrap_or(u64::MAX);
    let maior = meios.iter().map(|m| m.mtu()).max().unwrap_or(0);
    let pedaco = pedaco_para_o_meio(maior, tamanho)?;
    let (manifesto, fragmentos) = fatiar(dados, nome, pedaco)?;

    let anuncio = Quadro::Manifesto(manifesto.clone()).codificar()?;
    for meio in meios.iter_mut() {
        if anuncio.len() <= meio.mtu() {
            meio.enviar(&anuncio)?;
        }
    }

    // Quem tem espaço para o quadro de fragmento carrega dado; o resto só avisa.
    let quadros: Vec<Vec<u8>> = fragmentos
        .iter()
        .map(|f| Quadro::Fragmento(f.clone()).codificar())
        .collect::<Result<_, _>>()?;
    let maior_quadro = quadros.iter().map(Vec::len).max().unwrap_or(0);
    let carregadores: Vec<usize> = (0..meios.len())
        .filter(|i| meios.get(*i).is_some_and(|m| m.mtu() >= maior_quadro))
        .collect();
    if carregadores.is_empty() {
        return Err(TransporteError::MeioPequenoDemais {
            mtu: maior,
            preciso: maior_quadro,
        });
    }
    for (posicao, quadro) in quadros.iter().enumerate() {
        let vez = posicao.checked_rem(carregadores.len()).unwrap_or(0);
        let Some(escolhido) = carregadores.get(vez) else {
            continue;
        };
        if let Some(meio) = meios.get_mut(*escolhido) {
            meio.enviar(quadro)?;
        }
    }
    Ok(manifesto)
}

/// Recolhe de vários meios até o objeto ficar inteiro.
///
/// Devolve o objeto quando fecha, ou `None` se ainda falta. Quem chama decide
/// quando desistir e quando pedir o que falta.
#[derive(Debug, Default)]
pub struct Recepcao {
    montador: Option<Montador>,
    vistos: BTreeSet<u32>,
}

impl Recepcao {
    /// Começa vazia, à espera de um manifesto.
    pub fn nova() -> Self {
        Self::default()
    }

    /// Processa um quadro cru vindo de qualquer meio.
    ///
    /// Quadro estragado não derruba a recepção: vira erro e a vida segue.
    pub fn quadro(&mut self, cru: &[u8]) -> Result<Option<Vec<u8>>, TransporteError> {
        match Quadro::decodificar(cru)? {
            Quadro::Manifesto(m) => {
                if self.montador.is_none() {
                    self.montador = Some(Montador::novo(m)?);
                }
            }
            Quadro::Fragmento(frag) => {
                let Some(montador) = self.montador.as_mut() else {
                    // Pedaço antes do anúncio: guarda o número e espera o manifesto.
                    self.vistos.insert(frag.indice);
                    return Ok(None);
                };
                montador.aceitar(&frag)?;
                if montador.pronto() {
                    return montador.montar().map(Some);
                }
            }
            Quadro::Faltando { .. } => {}
        }
        Ok(None)
    }

    /// O que ainda falta, se já houver manifesto.
    pub fn faltando(&self) -> Vec<u32> {
        self.montador.as_ref().map(Montador::faltando).unwrap_or_default()
    }

    /// O manifesto em curso, se já chegou.
    pub fn manifesto(&self) -> Option<&Manifesto> {
        self.montador.as_ref().map(Montador::manifesto)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod testes {
    use super::*;

    fn objeto(n: usize) -> Vec<u8> {
        (0..n).map(|i| u8::try_from(i % 251).unwrap_or(0)).collect()
    }

    #[test]
    fn fatia_e_monta_de_volta() {
        let dados = objeto(10_000);
        let (manifesto, fragmentos) = fatiar(&dados, "foto.jpg", 1024).unwrap();
        assert_eq!(manifesto.quantidade, 10);
        assert!(manifesto.coerente());

        let mut montador = Montador::novo(manifesto).unwrap();
        for f in &fragmentos {
            assert!(montador.aceitar(f).unwrap());
        }
        assert_eq!(montador.montar().unwrap(), dados);
    }

    #[test]
    fn ordem_embaralhada_e_repeticao_nao_atrapalham() {
        let dados = objeto(5_000);
        let (manifesto, mut fragmentos) = fatiar(&dados, "", 512).unwrap();
        fragmentos.reverse();
        let mut montador = Montador::novo(manifesto).unwrap();
        for f in &fragmentos {
            montador.aceitar(f).unwrap();
        }
        // O mesmo pedaço de novo: não é erro, e não conta duas vezes.
        assert!(!montador.aceitar(&fragmentos[0]).unwrap());
        assert!(montador.pronto());
        assert_eq!(montador.montar().unwrap(), dados);
    }

    #[test]
    fn pedaco_adulterado_e_recusado() {
        let dados = objeto(4_096);
        let (manifesto, fragmentos) = fatiar(&dados, "", 1024).unwrap();
        let mut montador = Montador::novo(manifesto).unwrap();
        let mut mentiroso = fragmentos[2].clone();
        mentiroso.dados[0] ^= 0xff;
        assert_eq!(
            montador.aceitar(&mentiroso),
            Err(TransporteError::ProvaInvalida(2))
        );
        assert_eq!(montador.tenho(), 0);
    }

    #[test]
    fn fragmento_de_outro_objeto_e_recusado() {
        let (m1, _) = fatiar(&objeto(2_000), "a", 1024).unwrap();
        let (_, f2) = fatiar(b"outro conteudo qualquer", "b", 1024).unwrap();
        let mut montador = Montador::novo(m1).unwrap();
        assert_eq!(montador.aceitar(&f2[0]), Err(TransporteError::OutroObjeto));
    }

    #[test]
    fn falta_dizer_o_que_falta() {
        let dados = objeto(3_000);
        let (manifesto, fragmentos) = fatiar(&dados, "", 1000).unwrap();
        let mut montador = Montador::novo(manifesto).unwrap();
        montador.aceitar(&fragmentos[0]).unwrap();
        assert_eq!(montador.faltando(), vec![1, 2]);
        assert!(matches!(
            montador.montar(),
            Err(TransporteError::Incompleto { tenho: 1, total: 3 })
        ));
    }

    #[test]
    fn quadros_vao_e_voltam_iguais() {
        let (manifesto, fragmentos) = fatiar(&objeto(2_500), "nota.txt", 700).unwrap();
        for quadro in [
            Quadro::Manifesto(manifesto.clone()),
            Quadro::Fragmento(fragmentos[1].clone()),
            Quadro::Faltando { id: manifesto.id, indices: vec![0, 3, 7] },
        ] {
            let cru = quadro.codificar().unwrap();
            assert_eq!(Quadro::decodificar(&cru).unwrap(), quadro);
        }
    }

    #[test]
    fn bytes_de_outro_protocolo_nao_derrubam() {
        assert_eq!(Quadro::decodificar(b"HTTP/1.1 200 OK"), Err(TransporteError::MagicErrado));
        assert!(Quadro::decodificar(&[]).is_err());
        let (m, _) = fatiar(&objeto(100), "", 50).unwrap();
        let mut cru = Quadro::Manifesto(m).codificar().unwrap();
        cru.truncate(20);
        assert!(Quadro::decodificar(&cru).is_err());
    }

    #[test]
    fn o_pedaco_cabe_no_meio() {
        // Wi-Fi: pedaço grande.
        let wifi = pedaco_para_o_meio(16 * 1024, 5 * 1024 * 1024).unwrap();
        // Bluetooth, objeto pequeno: pedaço menor, mas ainda útil.
        let bt = pedaco_para_o_meio(512, 2_000).unwrap();
        assert!(wifi > bt);

        for (mtu, tamanho) in [(16 * 1024usize, 5 * 1024 * 1024u64), (512, 2_000), (1024, 300)] {
            let pedaco = pedaco_para_o_meio(mtu, tamanho).unwrap();
            let dados = objeto(usize::try_from(tamanho).unwrap());
            let (_, fragmentos) = fatiar(&dados, "", pedaco).unwrap();
            let maior = fragmentos
                .iter()
                .map(|f| Quadro::Fragmento(f.clone()).codificar().unwrap().len())
                .max()
                .unwrap();
            assert!(maior <= mtu, "quadro de {maior} bytes não cabe em {mtu}");
        }
    }

    #[test]
    fn meio_minusculo_avisa_em_vez_de_travar() {
        // Um quadro de rádio de 100 bytes não carrega nem o cabeçalho.
        assert!(matches!(
            pedaco_para_o_meio(100, 1_000_000),
            Err(TransporteError::MeioPequenoDemais { .. })
        ));
    }

    #[test]
    fn manifesto_mentiroso_e_recusado() {
        let (mut manifesto, _) = fatiar(&objeto(3_000), "", 1000).unwrap();
        manifesto.quantidade = 7; // não bate com tamanho e pedaço
        assert_eq!(
            Montador::novo(manifesto).err(),
            Some(TransporteError::ManifestoIncoerente)
        );
    }
}
