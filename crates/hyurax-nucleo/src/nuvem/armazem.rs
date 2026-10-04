//! Armazenamento distribuído: os dois lados.
//!
//! - [`Guarda`]: este nó guarda fragmentos de outros, até a cota que o dono
//!   ofereceu. Só a identidade (Noise) que mandou o fragmento busca, desafia
//!   ou apaga. Os fragmentos chegam cifrados: quem guarda nunca vê o
//!   conteúdo em claro.
//! - [`Manifesto`]: um arquivo deste nó guardado na rede. Fica só aqui (tem
//!   a chave do arquivo), com o hash de cada fragmento, quem guarda cada um,
//!   e desafios pré-calculados para conferir a guarda sem baixar tudo.
//!
//! O envio, a busca, os desafios e o reparo (reconstruir com `k` fragmentos
//! e mandar o que se perdeu para outro nó) são coordenados em [`super`].

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, Tag};
use hyurax_crypto::{HASH_LEN, sha512};
use hyurax_nuvem::FRAGMENTO_MAX;
use hyurax_nuvem::codigo::prova;
use hyurax_nuvem::mensagem::{Alvo, MensagemNuvem};

use crate::util::{de_hex, hex};

/// Maior arquivo que o dono guarda na rede.
pub const ARQUIVO_MAX: usize = 64 * 1024 * 1024;
/// Envios pela metade ao mesmo tempo (este nó guardando).
pub const PENDENTES_MAX: usize = 8;
/// Um envio pela metade que parou some depois disso.
pub const PENDENTE_VALE_S: u64 = 600;
/// Fragmentos guardados por este nó, no máximo.
pub const ITENS_MAX: usize = 10_000;
/// Desafios pré-calculados por fragmento.
pub const DESAFIOS_POR_FRAGMENTO: usize = 8;

// ------------------------------------------------------------------ guarda

/// Um fragmento guardado aqui.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Tamanho, em bytes.
    pub tamanho: u32,
    /// SHA-512.
    pub hash: [u8; HASH_LEN],
    /// Quando ficou completo.
    pub criado: u64,
}

struct Pendente {
    tamanho: u32,
    hash: [u8; HASH_LEN],
    recebido: u32,
    inicio: u64,
}

/// Os fragmentos que este nó guarda para outros.
pub struct Guarda {
    pasta: PathBuf,
    /// Cota oferecida, em MiB (0: não guarda nada).
    pub cota_mib: u64,
    /// O que está guardado: (dono, alvo) → item.
    pub itens: BTreeMap<([u8; 32], Alvo), Item>,
    pendentes: BTreeMap<([u8; 32], Alvo), Pendente>,
}

fn nome_do_fragmento(dono: &[u8; 32], alvo: &Alvo) -> String {
    format!("{}/{}-{}", hex(dono.get(..16).unwrap_or_default()), hex(&alvo.arquivo), alvo.indice)
}

fn recusa(alvo: Alvo, motivo: &str) -> MensagemNuvem {
    MensagemNuvem::Guardado { alvo, ok: false, motivo: motivo.chars().take(150).collect() }
}

/// SHA-512 de um arquivo, lendo aos pedaços.
fn hash_do_arquivo(caminho: &Path) -> Option<[u8; HASH_LEN]> {
    let mut f = std::fs::File::open(caminho).ok()?;
    let mut tudo = Vec::new();
    f.read_to_end(&mut tudo).ok()?;
    Some(sha512(&tudo))
}

impl Guarda {
    /// Abre a guarda em `pasta` (lê o índice).
    pub fn abrir(pasta: PathBuf, cota_mib: u64) -> Self {
        let mut itens = BTreeMap::new();
        let texto = std::fs::read_to_string(pasta.join("guarda.txt")).unwrap_or_default();
        for l in texto.lines().filter(|l| !l.starts_with('#')) {
            let mut c = l.split(' ');
            let (Some(dono), Some(arquivo), Some(indice), Some(tamanho), Some(hash), Some(criado)) = (
                c.next().and_then(de_hex::<32>),
                c.next().and_then(de_hex::<32>),
                c.next().and_then(|v| v.parse::<u8>().ok()),
                c.next().and_then(|v| v.parse::<u32>().ok()),
                c.next().and_then(de_hex::<64>),
                c.next().and_then(|v| v.parse::<u64>().ok()),
            ) else {
                continue;
            };
            let alvo = Alvo { arquivo, indice };
            if pasta.join(format!("{}.frag", nome_do_fragmento(&dono, &alvo))).exists() {
                itens.insert((dono, alvo), Item { tamanho, hash, criado });
            }
        }
        Self { pasta, cota_mib, itens, pendentes: BTreeMap::new() }
    }

    fn caminho(&self, dono: &[u8; 32], alvo: &Alvo, ext: &str) -> PathBuf {
        self.pasta.join(format!("{}.{ext}", nome_do_fragmento(dono, alvo)))
    }

    fn gravar_indice(&self) {
        let mut t = String::from("# Hyurax: fragmentos guardados para outros nós (cifrados; este nó não tem a chave)\n");
        for ((dono, alvo), i) in &self.itens {
            t.push_str(&format!("{} {} {} {} {} {}\n", hex(dono), hex(&alvo.arquivo), alvo.indice, i.tamanho, hex(&i.hash), i.criado));
        }
        let _ = std::fs::create_dir_all(&self.pasta);
        let _ = crate::arquivos::gravar_atomico(&self.pasta.join("guarda.txt"), t.as_bytes());
    }

    /// Bytes em uso (guardados e chegando).
    pub fn usado(&self) -> u64 {
        let guardado: u64 = self.itens.values().map(|i| u64::from(i.tamanho)).sum();
        let chegando: u64 = self.pendentes.values().map(|p| u64::from(p.tamanho)).sum();
        guardado.saturating_add(chegando)
    }

    /// Alguém quer mandar um fragmento. `Some`: a recusa a responder.
    pub fn guardar(&mut self, dono: [u8; 32], alvo: Alvo, tamanho: u32, hash: [u8; HASH_LEN], agora: u64) -> Option<MensagemNuvem> {
        self.limpar(agora);
        if self.cota_mib == 0 {
            return Some(recusa(alvo, "este nó não oferece armazenamento"));
        }
        if tamanho == 0 || tamanho > FRAGMENTO_MAX {
            return Some(recusa(alvo, "fragmento de tamanho inválido"));
        }
        if self.itens.get(&(dono, alvo)).is_some_and(|i| i.hash == hash) {
            return Some(MensagemNuvem::Guardado { alvo, ok: true, motivo: "já guardado".into() });
        }
        if self.pendentes.len() >= PENDENTES_MAX || self.itens.len() >= ITENS_MAX {
            return Some(recusa(alvo, "envios demais ao mesmo tempo; tente de novo"));
        }
        let cota = self.cota_mib.saturating_mul(1024 * 1024);
        if self.usado().saturating_add(u64::from(tamanho)) > cota {
            return Some(recusa(alvo, "sem espaço na cota oferecida"));
        }
        let parcial = self.caminho(&dono, &alvo, "parcial");
        if let Some(p) = parcial.parent()
            && std::fs::create_dir_all(p).is_err()
        {
            return Some(recusa(alvo, "disco indisponível"));
        }
        if std::fs::File::create(&parcial).is_err() {
            return Some(recusa(alvo, "disco indisponível"));
        }
        self.pendentes.insert((dono, alvo), Pendente { tamanho, hash, recebido: 0, inicio: agora });
        None
    }

    /// Há um envio pela metade deste dono para este fragmento?
    pub fn esperando(&self, dono: &[u8; 32], alvo: &Alvo) -> bool {
        self.pendentes.contains_key(&(*dono, *alvo))
    }

    /// Um pedaço chegou. `Some`: a resposta final (guardado ou recusado).
    pub fn parte(&mut self, dono: [u8; 32], alvo: Alvo, deslocamento: u32, dados: &[u8]) -> Option<MensagemNuvem> {
        let chave = (dono, alvo);
        let (tamanho, hash, recebido) = {
            let p = self.pendentes.get(&chave)?;
            (p.tamanho, p.hash, p.recebido)
        };
        let fim = u64::from(recebido).saturating_add(dados.len() as u64);
        let parcial = self.caminho(&dono, &alvo, "parcial");
        if deslocamento != recebido || fim > u64::from(tamanho) {
            self.pendentes.remove(&chave);
            let _ = std::fs::remove_file(&parcial);
            return Some(recusa(alvo, "pedaço fora de ordem ou além do tamanho"));
        }
        let gravou = std::fs::OpenOptions::new().append(true).open(&parcial).and_then(|mut f| f.write_all(dados));
        if gravou.is_err() {
            self.pendentes.remove(&chave);
            let _ = std::fs::remove_file(&parcial);
            return Some(recusa(alvo, "disco indisponível"));
        }
        let novo = u32::try_from(fim).unwrap_or(u32::MAX);
        if let Some(p) = self.pendentes.get_mut(&chave) {
            p.recebido = novo;
        }
        if novo < tamanho {
            return None;
        }
        self.pendentes.remove(&chave);
        if hash_do_arquivo(&parcial) != Some(hash) {
            let _ = std::fs::remove_file(&parcial);
            return Some(recusa(alvo, "o hash do fragmento não confere"));
        }
        if std::fs::rename(&parcial, self.caminho(&dono, &alvo, "frag")).is_err() {
            let _ = std::fs::remove_file(&parcial);
            return Some(recusa(alvo, "disco indisponível"));
        }
        self.itens.insert(chave, Item { tamanho, hash, criado: crate::util::agora_unix() });
        self.gravar_indice();
        Some(MensagemNuvem::Guardado { alvo, ok: true, motivo: String::new() })
    }

    /// Lê um fragmento guardado (só do dono).
    pub fn ler(&self, dono: &[u8; 32], alvo: &Alvo) -> Option<Vec<u8>> {
        let item = self.itens.get(&(*dono, *alvo))?;
        let bytes = std::fs::read(self.caminho(dono, alvo, "frag")).ok()?;
        (bytes.len() == item.tamanho as usize).then_some(bytes)
    }

    /// Responde a um desafio do dono.
    pub fn provar(&self, dono: &[u8; 32], alvo: Alvo, nonce: [u8; 32]) -> MensagemNuvem {
        match self.ler(dono, &alvo) {
            Some(f) => MensagemNuvem::Prova { alvo, nonce, h: prova(&nonce, &f) },
            None => recusa(alvo, "não tenho este fragmento"),
        }
    }

    /// Apaga um fragmento do dono.
    pub fn apagar(&mut self, dono: &[u8; 32], alvo: &Alvo) -> bool {
        if self.itens.remove(&(*dono, *alvo)).is_none() {
            return false;
        }
        let _ = std::fs::remove_file(self.caminho(dono, alvo, "frag"));
        self.gravar_indice();
        true
    }

    /// Tira os envios pela metade que pararam.
    pub fn limpar(&mut self, agora: u64) {
        let velhos: Vec<([u8; 32], Alvo)> =
            self.pendentes.iter().filter(|(_, p)| agora.saturating_sub(p.inicio) > PENDENTE_VALE_S).map(|(k, _)| *k).collect();
        for (dono, alvo) in velhos {
            self.pendentes.remove(&(dono, alvo));
            let _ = std::fs::remove_file(self.caminho(&dono, &alvo, "parcial"));
        }
    }
}

// --------------------------------------------------------------- manifesto

/// Como está um fragmento de um arquivo deste nó.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoDoFragmento {
    /// Mandado, sem resposta ainda.
    Enviando,
    /// Guardado e conferido.
    Guardado,
    /// Perdido: o guardião sumiu, falhou nos desafios ou recusou.
    Perdido,
}

impl EstadoDoFragmento {
    /// Nome para o disco e a tela.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Enviando => "enviando",
            Self::Guardado => "guardado",
            Self::Perdido => "perdido",
        }
    }

    fn de_nome(n: &str) -> Option<Self> {
        match n {
            "enviando" => Some(Self::Enviando),
            "guardado" => Some(Self::Guardado),
            "perdido" => Some(Self::Perdido),
            _ => None,
        }
    }
}

/// Um fragmento de um arquivo deste nó.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragmento {
    /// Índice (`0..k+m`).
    pub indice: u8,
    /// Tamanho, em bytes.
    pub tamanho: u32,
    /// SHA-512.
    pub hash: [u8; HASH_LEN],
    /// Identidade (Noise) de quem guarda.
    pub guardiao: [u8; 32],
    /// Estado.
    pub estado: EstadoDoFragmento,
    /// Última prova boa (segundos Unix).
    pub ultimo_ok: u64,
    /// Desde quando o guardião não está conectado (0: está).
    pub ausente_desde: u64,
    /// Desafios seguidos que falharam.
    pub falhas: u8,
    /// Desafios ainda não usados: (nonce, prova esperada).
    pub desafios: Vec<([u8; 32], [u8; HASH_LEN])>,
    /// Preço do guardião, milicréditos por GB por mês.
    pub preco_gb_mes_mili: u64,
    /// Até quando o armazenamento já foi lançado no livro.
    pub pago_ate: u64,
}

/// Um arquivo deste nó guardado na rede.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifesto {
    /// Identificador (sorteado).
    pub arquivo: [u8; 32],
    /// Nome dado pelo dono.
    pub nome: String,
    /// Tamanho original.
    pub tamanho: u64,
    /// Tamanho cifrado (com a etiqueta).
    pub cifrado: u64,
    /// Fragmentos de dados.
    pub k: u8,
    /// Fragmentos de paridade.
    pub m: u8,
    /// A chave do arquivo (só aqui).
    pub chave: [u8; 32],
    /// Quando foi guardado.
    pub criado: u64,
    /// Os fragmentos.
    pub fragmentos: Vec<Fragmento>,
}

/// Cifra com a chave do arquivo (nonce zero: a chave é única por arquivo) e o
/// identificador como dado associado. Devolve cifrado || etiqueta.
///
/// # Errors
/// Falha da cifra.
pub fn cifrar(chave: &[u8; 32], arquivo: &[u8; 32], dados: &[u8]) -> Result<Vec<u8>, String> {
    let mut buf = dados.to_vec();
    let etiqueta = ChaCha20Poly1305::new(&Key::from(*chave))
        .encrypt_in_place_detached(&Nonce::from([0u8; 12]), arquivo, &mut buf)
        .map_err(|_| "falha ao cifrar".to_string())?;
    buf.extend_from_slice(&etiqueta);
    Ok(buf)
}

/// Decifra e confere a etiqueta: qualquer byte trocado é recusado.
///
/// # Errors
/// Etiqueta que não confere.
pub fn decifrar(chave: &[u8; 32], arquivo: &[u8; 32], cifrado: &[u8]) -> Result<Vec<u8>, String> {
    let corte = cifrado.len().checked_sub(16).ok_or("cifrado curto demais")?;
    let (corpo, etiqueta) = cifrado.split_at(corte);
    let etiqueta: [u8; 16] = etiqueta.try_into().map_err(|_| "etiqueta")?;
    let mut buf = corpo.to_vec();
    ChaCha20Poly1305::new(&Key::from(*chave))
        .decrypt_in_place_detached(&Nonce::from([0u8; 12]), arquivo, &mut buf, &Tag::from(etiqueta))
        .map_err(|_| "o arquivo reconstruído não confere com a etiqueta (fragmento adulterado?)".to_string())?;
    Ok(buf)
}

/// Os desafios de um fragmento, derivados da semente.
pub fn desafios(semente: &[u8; 32], arquivo: &[u8; 32], indice: u8, rodada: u32, fragmento: &[u8]) -> Vec<([u8; 32], [u8; HASH_LEN])> {
    (0..DESAFIOS_POR_FRAGMENTO)
        .filter_map(|i| {
            let mut m = Vec::with_capacity(80);
            m.extend_from_slice(b"HYURAX-NUVEM-DESAFIO");
            m.extend_from_slice(semente);
            m.extend_from_slice(arquivo);
            m.push(indice);
            m.extend_from_slice(&rodada.to_be_bytes());
            m.extend_from_slice(&u32::try_from(i).unwrap_or(0).to_be_bytes());
            let nonce: [u8; 32] = sha512(&m).get(..32)?.try_into().ok()?;
            Some((nonce, prova(&nonce, fragmento)))
        })
        .collect()
}

impl Manifesto {
    /// O texto do arquivo do manifesto (tem a chave: gravar privado).
    pub fn texto(&self) -> String {
        let mut t = format!(
            "# Hyurax: manifesto de um arquivo guardado na rede. TEM A CHAVE do arquivo: não compartilhe.\n\
             arquivo={}\nnome={}\ntamanho={}\ncifrado={}\nk={}\nm={}\nchave={}\ncriado={}\n",
            hex(&self.arquivo),
            hex(self.nome.as_bytes()),
            self.tamanho,
            self.cifrado,
            self.k,
            self.m,
            hex(&self.chave),
            self.criado
        );
        for f in &self.fragmentos {
            let d: Vec<String> = f.desafios.iter().map(|(n, h)| format!("{}:{}", hex(n), hex(h))).collect();
            t.push_str(&format!(
                "frag={} {} {} {} {} {} {} {} {} {} {}\n",
                f.indice,
                f.tamanho,
                hex(&f.hash),
                hex(&f.guardiao),
                f.estado.nome(),
                f.ultimo_ok,
                f.ausente_desde,
                f.falhas,
                f.preco_gb_mes_mili,
                f.pago_ate,
                if d.is_empty() { "-".to_string() } else { d.join(",") }
            ));
        }
        t
    }

    /// Lê o texto de um manifesto.
    pub fn ler(texto: &str) -> Option<Self> {
        let campo = |nome: &str| texto.lines().find_map(|l| l.strip_prefix(&format!("{nome}=")).map(str::trim));
        let nome_hex = campo("nome")?;
        let bytes: Vec<u8> = (0..nome_hex.len() / 2).filter_map(|i| nome_hex.get(i * 2..i * 2 + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
        let mut fragmentos = Vec::new();
        for l in texto.lines().filter_map(|l| l.strip_prefix("frag=")) {
            let c: Vec<&str> = l.split(' ').collect();
            let [indice, tamanho, hash, guardiao, estado, ultimo_ok, ausente, falhas, preco, pago, ds] = c.as_slice() else { return None };
            let desafios = if *ds == "-" {
                Vec::new()
            } else {
                ds.split(',').filter_map(|p| p.split_once(':').and_then(|(n, h)| Some((de_hex::<32>(n)?, de_hex::<64>(h)?)))).collect()
            };
            fragmentos.push(Fragmento {
                indice: indice.parse().ok()?,
                tamanho: tamanho.parse().ok()?,
                hash: de_hex(hash)?,
                guardiao: de_hex(guardiao)?,
                estado: EstadoDoFragmento::de_nome(estado)?,
                ultimo_ok: ultimo_ok.parse().ok()?,
                ausente_desde: ausente.parse().ok()?,
                falhas: falhas.parse().ok()?,
                desafios,
                preco_gb_mes_mili: preco.parse().ok()?,
                pago_ate: pago.parse().ok()?,
            });
        }
        Some(Self {
            arquivo: de_hex(campo("arquivo")?)?,
            nome: String::from_utf8(bytes).ok()?,
            tamanho: campo("tamanho")?.parse().ok()?,
            cifrado: campo("cifrado")?.parse().ok()?,
            k: campo("k")?.parse().ok()?,
            m: campo("m")?.parse().ok()?,
            chave: de_hex(campo("chave")?)?,
            criado: campo("criado")?.parse().ok()?,
            fragmentos,
        })
    }

    /// Fragmentos guardados (contam para reconstruir).
    pub fn guardados(&self) -> usize {
        self.fragmentos.iter().filter(|f| f.estado == EstadoDoFragmento::Guardado).count()
    }

    /// A saúde: "integro" (todos guardados), "degradado" (dá para
    /// reconstruir, falta reparar), "em risco" (menos de k) ou "enviando".
    pub fn saude(&self) -> &'static str {
        let g = self.guardados();
        let total = usize::from(self.k).saturating_add(usize::from(self.m));
        if self.fragmentos.iter().any(|f| f.estado == EstadoDoFragmento::Enviando) {
            "enviando"
        } else if g >= total {
            "integro"
        } else if g >= usize::from(self.k) {
            "degradado"
        } else {
            "em_risco"
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hyurax-guarda-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn guarda_confere_hash_cota_e_dono() {
        let p = pasta("a");
        let mut g = Guarda::abrir(p.clone(), 1);
        let dono = [1u8; 32];
        let alvo = Alvo { arquivo: [2; 32], indice: 0 };
        let frag = vec![9u8; 1000];
        let h = sha512(&frag);
        assert!(g.guardar(dono, alvo, 1000, h, 10).is_none());
        assert!(g.parte(dono, alvo, 0, frag.get(..600).unwrap()).is_none());
        let fim = g.parte(dono, alvo, 600, frag.get(600..).unwrap()).unwrap();
        assert!(matches!(fim, MensagemNuvem::Guardado { ok: true, .. }));
        assert_eq!(g.ler(&dono, &alvo), Some(frag.clone()));
        assert_eq!(g.ler(&[3; 32], &alvo), None, "outro dono não lê");
        let nonce = [5u8; 32];
        assert_eq!(g.provar(&dono, alvo, nonce), MensagemNuvem::Prova { alvo, nonce, h: prova(&nonce, &frag) });
        // reabre: o índice volta
        let g2 = Guarda::abrir(p.clone(), 1);
        assert_eq!(g2.ler(&dono, &alvo), Some(frag));
        // hash errado é recusado; cota estourada é recusada; sem cota, nada
        let outro = Alvo { arquivo: [4; 32], indice: 1 };
        assert!(g.guardar(dono, outro, 10, [0; 64], 10).is_none());
        assert!(matches!(g.parte(dono, outro, 0, &[1; 10]), Some(MensagemNuvem::Guardado { ok: false, .. })));
        assert!(g.guardar(dono, Alvo { arquivo: [5; 32], indice: 0 }, 2 * 1024 * 1024, [0; 64], 10).is_some());
        let mut sem = Guarda::abrir(pasta("b"), 0);
        assert!(sem.guardar(dono, alvo, 10, [0; 64], 10).is_some());
        assert!(g.apagar(&dono, &alvo));
        assert_eq!(g.ler(&dono, &alvo), None);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn cifra_e_manifesto_vao_e_voltam() {
        let chave = [7u8; 32];
        let arquivo = [8u8; 32];
        let c = cifrar(&chave, &arquivo, b"segredo do dono").unwrap();
        assert_eq!(decifrar(&chave, &arquivo, &c).unwrap(), b"segredo do dono");
        let mut torto = c.clone();
        if let Some(b) = torto.first_mut() {
            *b ^= 1;
        }
        assert!(decifrar(&chave, &arquivo, &torto).is_err());
        assert!(decifrar(&chave, &[9; 32], &c).is_err(), "o identificador entra na conta");
        let m = Manifesto {
            arquivo,
            nome: "relatório final.pdf".into(),
            tamanho: 15,
            cifrado: 31,
            k: 2,
            m: 1,
            chave,
            criado: 5,
            fragmentos: vec![Fragmento {
                indice: 0,
                tamanho: 16,
                hash: [1; 64],
                guardiao: [2; 32],
                estado: EstadoDoFragmento::Guardado,
                ultimo_ok: 3,
                ausente_desde: 0,
                falhas: 0,
                desafios: desafios(&[3; 32], &arquivo, 0, 0, b"x"),
                preco_gb_mes_mili: 10,
                pago_ate: 3,
            }],
        };
        assert_eq!(Manifesto::ler(&m.texto()), Some(m.clone()));
        assert_eq!(m.saude(), "em_risco");
    }
}
