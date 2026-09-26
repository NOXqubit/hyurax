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
    /// `WORKER_ID`: a chave pública da identidade do nó.
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

    /// Assina com a chave secreta da identidade do nó. Recusa se a chave não
    /// for a do `worker` do registro: ninguém assina em nome de outro.
    pub fn assinar(&self, segredo: &[u8; SECRET_LEN]) -> Option<[u8; SIGNATURE_LEN]> {
        (ed25519_public_key(segredo) == self.worker).then(|| ed25519_sign(segredo, &self.mensagem()))
    }

    /// Confere a assinatura contra o `worker` do registro.
    pub fn assinatura_confere(&self, assinatura: &[u8]) -> bool {
        ed25519_verify(&self.worker, &self.mensagem(), assinatura)
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
    fn nao_assina_em_nome_de_outro() {
        let r = registro(&[7; 32]);
        assert!(r.assinar(&[9; 32]).is_none());
    }
}
