// ✝ 1 Tessalonicenses 5:21 — “Examinai tudo. Retende o bem.”
//! O registro de prova: o que foi pedido, o que entrou, o que saiu, quem fez
//! e quando, assinado pelo worker.
//!
//! A assinatura tem uma função só: atribuir o resultado a um worker. É ela que
//! permite, no modo TESTNET, dizer "o worker X entregou este resultado" sem
//! confiar em quem repassou a mensagem, e alimentar a reputação de X. O estado
//! da tarefa (`STATUS`) não entra na assinatura, porque muda depois.

use hyurax_codec::{Reader, Writer};
use hyurax_crypto::{HASH_LEN, PUBKEY_LEN, SECRET_LEN, SIGNATURE_LEN, ed25519_public_key, ed25519_sign, ed25519_verify, sha512};

use crate::trabalho::{ErroDeTrabalho, Especificacao, MetodoDeVerificacao};

/// Domínio da mensagem assinada.
pub const DOMINIO_PROVA: &[u8] = dominio!("PROVA-v1");
/// Versão do formato do registro.
pub const VERSAO_PROVA: u16 = 1;
/// Domínio da assinatura do veredito.
pub const DOMINIO_VEREDITO: &[u8] = dominio!("VEREDITO-v1");
/// Domínio da derivação da chave do worker.
pub const DOMINIO_WORKER: &[u8] = dominio!("WORKER-v1");

/// A chave secreta Ed25519 do worker, derivada do segredo do nó.
///
/// A identidade do nó (`PASTA/no.chave`) é uma chave X25519 da cifra da rede.
/// Usar o mesmo segredo para assinar em Ed25519 seria a mesma chave em dois
/// algoritmos, o que não se faz. A derivação por hash com domínio próprio dá
/// outra chave, estável enquanto o nó for o mesmo: apagar `no.chave` troca as
/// duas juntas.
pub fn chave_do_worker(segredo_do_no: &[u8; SECRET_LEN]) -> [u8; SECRET_LEN] {
    let mut dados = DOMINIO_WORKER.to_vec();
    dados.extend_from_slice(segredo_do_no);
    sha512(&dados).first_chunk::<SECRET_LEN>().copied().unwrap_or([0; SECRET_LEN])
}

/// Um registro de prova.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistroDeProva {
    /// `TASK_ID`.
    pub tarefa: [u8; HASH_LEN],
    /// `INPUT_HASH`.
    pub entrada: [u8; HASH_LEN],
    /// `WORK_TYPE` e `PARAMETERS`.
    pub especificacao: Especificacao,
    /// `VERIFICATION_METHOD`.
    pub metodo: MetodoDeVerificacao,
    /// `RESULT_HASH`.
    pub resultado: [u8; HASH_LEN],
    /// Operações feitas, pelo modelo de custo do tipo.
    pub operacoes: u64,
    /// `WORKER_ID`: a chave pública Ed25519 do worker, derivada da identidade
    /// do nó por [`chave_do_worker`].
    pub worker: [u8; PUBKEY_LEN],
    /// Início da execução, em milissegundos desde 1970.
    pub inicio_ms: u64,
    /// Fim da execução.
    pub fim_ms: u64,
}

impl RegistroDeProva {
    /// A codificação canônica, que é o que se assina.
    pub fn codificar(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u16(VERSAO_PROVA);
        w.fixed(&self.tarefa);
        w.fixed(&self.entrada);
        self.especificacao.codificar(&mut w);
        w.u8(self.metodo.codigo());
        w.fixed(&self.resultado);
        w.u64(self.operacoes);
        w.fixed(&self.worker);
        w.u64(self.inicio_ms);
        w.u64(self.fim_ms);
        w.into_bytes()
    }

    /// O inverso de [`Self::codificar`], exigindo que não sobre byte.
    pub fn decodificar(dados: &[u8]) -> Result<Self, ErroDeTrabalho> {
        let mut r = Reader::new(dados);
        let versao = r.u16()?;
        if versao != VERSAO_PROVA {
            return Err(ErroDeTrabalho::TipoDesconhecido(u8::try_from(versao).unwrap_or(u8::MAX)));
        }
        let tarefa = r.fixed()?;
        let entrada = r.fixed()?;
        let especificacao = Especificacao::decodificar(&mut r)?;
        let codigo = r.u8()?;
        let metodo = MetodoDeVerificacao::de_codigo(codigo).ok_or(ErroDeTrabalho::TipoDesconhecido(codigo))?;
        let registro = Self {
            tarefa,
            entrada,
            especificacao,
            metodo,
            resultado: r.fixed()?,
            operacoes: r.u64()?,
            worker: r.fixed()?,
            inicio_ms: r.u64()?,
            fim_ms: r.u64()?,
        };
        r.finish()?;
        Ok(registro)
    }

    /// Hash do registro: identifica esta entrega.
    pub fn hash(&self) -> [u8; HASH_LEN] {
        sha512(&self.mensagem())
    }

    fn mensagem(&self) -> Vec<u8> {
        let mut m = DOMINIO_PROVA.to_vec();
        m.extend(self.codificar());
        m
    }

    /// Assina com a chave secreta do worker. Recusa se a chave não for a do
    /// `worker` do registro: ninguém assina em nome de outro.
    pub fn assinar(&self, segredo: &[u8; SECRET_LEN]) -> Option<[u8; SIGNATURE_LEN]> {
        (ed25519_public_key(segredo) == self.worker).then(|| ed25519_sign(segredo, &self.mensagem()))
    }

    /// Confere a assinatura contra o `worker` do registro.
    pub fn assinatura_confere(&self, assinatura: &[u8]) -> bool {
        ed25519_verify(&self.worker, &self.mensagem(), assinatura)
    }

    fn mensagem_do_veredito(&self, veredito: &str) -> Vec<u8> {
        let mut m = DOMINIO_VEREDITO.to_vec();
        m.extend_from_slice(&self.hash());
        m.extend_from_slice(veredito.as_bytes());
        m
    }

    /// Assina o veredito sobre esta entrega (`VERIFIED`, `REJECTED`,
    /// `SETTLED`...). O estado muda depois da entrega, então fica fora da
    /// assinatura do registro; esta segunda assinatura amarra o veredito ao
    /// registro, e quem trocar `REJECTED` por `SETTLED` depois é pego.
    pub fn assinar_veredito(&self, segredo: &[u8; SECRET_LEN], veredito: &str) -> Option<[u8; SIGNATURE_LEN]> {
        (ed25519_public_key(segredo) == self.worker).then(|| ed25519_sign(segredo, &self.mensagem_do_veredito(veredito)))
    }

    /// Confere a assinatura do veredito, feita pela mesma chave do registro.
    pub fn veredito_confere(&self, veredito: &str, assinatura: &[u8]) -> bool {
        ed25519_verify(&self.worker, &self.mensagem_do_veredito(veredito), assinatura)
    }
}

#[cfg(test)]
mod testes {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;
    use crate::trabalho::TipoDeTrabalho;

    fn registro(segredo: &[u8; 32]) -> RegistroDeProva {
        RegistroDeProva {
            tarefa: [1; 64],
            entrada: [2; 64],
            especificacao: Especificacao::nova(TipoDeTrabalho::Mochila, 40, 0).unwrap(),
            metodo: MetodoDeVerificacao::Recomputacao,
            resultado: [3; 64],
            operacoes: 12_345,
            worker: ed25519_public_key(segredo),
            inicio_ms: 10,
            fim_ms: 20,
        }
    }

    #[test]
    fn ida_e_volta() {
        let r = registro(&[7; 32]);
        assert_eq!(RegistroDeProva::decodificar(&r.codificar()).unwrap(), r);
        let mut sobra = r.codificar();
        sobra.push(0);
        assert!(RegistroDeProva::decodificar(&sobra).is_err());
    }

    #[test]
    fn assinatura_amarra_cada_campo() {
        let segredo = [7; 32];
        let r = registro(&segredo);
        let assinatura = r.assinar(&segredo).unwrap();
        assert!(r.assinatura_confere(&assinatura));

        let mut outro = r.clone();
        outro.resultado[0] ^= 1;
        assert!(!outro.assinatura_confere(&assinatura), "resultado trocado");
        let mut outro = r.clone();
        outro.operacoes += 1;
        assert!(!outro.assinatura_confere(&assinatura), "operações infladas");
        let mut outro = r.clone();
        outro.worker = ed25519_public_key(&[8; 32]);
        assert!(!outro.assinatura_confere(&assinatura), "outro worker");
    }

    #[test]
    fn chave_do_worker_e_outra_e_estavel() {
        let no = [5u8; 32];
        let w = chave_do_worker(&no);
        assert_ne!(w, no, "não reaproveita o segredo da cifra");
        assert_eq!(w, chave_do_worker(&no));
        assert_ne!(w, chave_do_worker(&[6u8; 32]));
    }

    #[test]
    fn veredito_amarrado_ao_registro() {
        let segredo = [7; 32];
        let r = registro(&segredo);
        let v = r.assinar_veredito(&segredo, "REJECTED").unwrap();
        assert!(r.veredito_confere("REJECTED", &v));
        assert!(!r.veredito_confere("SETTLED", &v), "trocar o veredito quebra a assinatura");
        let mut outro = r.clone();
        outro.resultado[0] ^= 1;
        assert!(!outro.veredito_confere("REJECTED", &v), "o veredito é deste registro");
        assert!(r.assinar_veredito(&[9; 32], "SETTLED").is_none());
    }

    #[test]
    fn nao_assina_em_nome_de_outro() {
        let r = registro(&[7; 32]);
        assert!(r.assinar(&[9; 32]).is_none());
    }
}
