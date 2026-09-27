//! O histórico em texto: uma linha por trecho de ciclo encerrado, e a leitura de volta.

#[allow(clippy::wildcard_imports)]
use super::*;

// ---------------------------------------------------------------------------
// Histórico em texto: uma linha por trecho de ciclo encerrado
// ---------------------------------------------------------------------------

/// Texto sem espaço nem quebra, para caber numa linha `chave=valor`.
pub(super) fn escapar(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len());
    for c in texto.chars() {
        match c {
            '%' => s.push_str("%25"),
            ' ' => s.push_str("%20"),
            '\n' => s.push_str("%0A"),
            '\r' => s.push_str("%0D"),
            '\t' => s.push_str("%09"),
            outro => s.push(outro),
        }
    }
    s
}

pub(super) fn desescapar(texto: &str) -> String {
    texto.replace("%20", " ").replace("%0A", "\n").replace("%0D", "\r").replace("%09", "\t").replace("%25", "%")
}

pub(super) fn linha_do_historico(
    item: &NaFila,
    registro: Option<(&RegistroDeProva, Option<[u8; 64]>)>,
    veredito: Option<[u8; 64]>,
    ms_calculo: u64,
    nota: &str,
) -> String {
    let t = &item.tarefa;
    let esp = t.especificacao;
    let mut l = String::with_capacity(1024);
    let _ = write!(
        l,
        "v=1 modo={MODO} selo={} numero={} id={} estado={} tipo={} tamanho={} passos={} metodo={} instancia={} desafio={} \
         origem={} prioridade={} criada={} prazo={}",
        escapar(SELO),
        t.numero(),
        hex(&t.id),
        t.estado.nome(),
        esp.tipo().codigo(),
        esp.tamanho(),
        esp.passos(),
        t.metodo.codigo(),
        t.instancia.codigo(),
        u8::from(item.desafio.is_some()),
        hex(&item.origem),
        t.prioridade,
        t.criada_ms,
        t.prazo_ms,
    );
    if !esp.parametros().is_empty() {
        let lista: Vec<String> = esp.parametros().iter().map(u32::to_string).collect();
        let _ = write!(l, " parametros={}", lista.join(","));
    }
    if let Some((job, indice)) = item.job {
        let _ = write!(l, " job={} indice={indice}", hex(&job));
    }
    if let Some((r, assinatura)) = registro {
        let _ = write!(
            l,
            " entrada={} resultado={} operacoes={} worker={} inicio={} fim={} assinatura={} veredito={}",
            hex(&r.entrada),
            hex(&r.resultado),
            r.operacoes,
            hex(&r.worker),
            r.inicio_ms,
            r.fim_ms,
            assinatura.map(|a| hex(&a)).unwrap_or_default(),
            veredito.map(|a| hex(&a)).unwrap_or_default(),
        );
    }
    let eventos: Vec<String> = t.eventos.iter().map(|e| format!("{}@{}", e.estado.nome(), e.instante_ms)).collect();
    let _ = write!(l, " ms_calculo={ms_calculo} eventos={} nota={}", eventos.join(","), escapar(nota));
    l
}

/// Uma linha do histórico, lida de volta.
#[derive(Clone, Debug)]
pub(super) struct Registro {
    pub(super) id: [u8; HASH_LEN],
    pub(super) estado: Estado,
    pub(super) especificacao: Especificacao,
    pub(super) metodo: MetodoDeVerificacao,
    pub(super) instancia: Instancia,
    pub(super) desafio: bool,
    pub(super) origem: Vec<u8>,
    pub(super) prioridade: u8,
    pub(super) criada_ms: u64,
    pub(super) prazo_ms: u64,
    pub(super) entrada: Option<[u8; HASH_LEN]>,
    pub(super) resultado: Option<[u8; HASH_LEN]>,
    pub(super) operacoes: u64,
    pub(super) worker: Option<[u8; PUBKEY_LEN]>,
    pub(super) inicio_ms: Option<u64>,
    pub(super) fim_ms: Option<u64>,
    pub(super) assinatura: Option<[u8; 64]>,
    pub(super) veredito: Option<[u8; 64]>,
    pub(super) ms_calculo: u64,
    pub(super) nota: String,
    pub(super) job: Option<[u8; HASH_LEN]>,
    pub(super) indice: Option<u64>,
}

pub(super) fn estado_de_nome(nome: &str) -> Option<Estado> {
    use Estado::*;
    [Criada, NaFila, Atribuida, Executando, Enviada, Verificando, Verificada, Recusada, Liquidada, Cancelada, Expirada]
        .into_iter()
        .find(|e| e.nome() == nome)
}

impl Registro {
    pub(super) fn ler(linha: &str) -> Option<Self> {
        let campo = |nome: &str| {
            linha.split(' ').find_map(|par| par.split_once('=').filter(|(k, _)| *k == nome).map(|(_, v)| v))
        };
        let num = |nome: &str| campo(nome).and_then(|v| v.parse::<u64>().ok());
        if campo("v") != Some("1") {
            return None;
        }
        let tipo = TipoDeTrabalho::de_codigo(u8::try_from(num("tipo")?).ok()?)?;
        let parametros: Vec<u32> = match campo("parametros") {
            Some(lista) => lista.split(',').map(|v| v.parse::<u32>().ok()).collect::<Option<Vec<u32>>>()?,
            None => Vec::new(),
        };
        let especificacao = Especificacao::nova_com(
            tipo,
            u32::try_from(num("tamanho")?).ok()?,
            u32::try_from(num("passos")?).ok()?,
            &parametros,
        )
        .ok()?;
        Some(Self {
            id: de_hex(campo("id")?)?,
            estado: estado_de_nome(campo("estado")?)?,
            especificacao,
            metodo: MetodoDeVerificacao::de_codigo(u8::try_from(num("metodo")?).ok()?)?,
            instancia: Instancia::de_codigo(u8::try_from(num("instancia")?).ok()?)?,
            desafio: num("desafio")? == 1,
            origem: decodificar_hex(campo("origem")?)?,
            prioridade: u8::try_from(num("prioridade")?).ok()?,
            criada_ms: num("criada")?,
            prazo_ms: num("prazo")?,
            entrada: campo("entrada").and_then(de_hex),
            resultado: campo("resultado").and_then(de_hex),
            operacoes: num("operacoes").unwrap_or(0),
            worker: campo("worker").and_then(de_hex),
            inicio_ms: num("inicio"),
            fim_ms: num("fim"),
            assinatura: campo("assinatura").and_then(de_hex),
            veredito: campo("veredito").and_then(de_hex),
            ms_calculo: num("ms_calculo").unwrap_or(0),
            nota: campo("nota").map(desescapar).unwrap_or_default(),
            job: campo("job").and_then(de_hex),
            indice: num("indice"),
        })
    }

    pub(super) fn numero(&self) -> u32 {
        u32::from_be_bytes(self.id.first_chunk::<4>().copied().unwrap_or_default()) % 100_000_000
    }

    pub(super) fn prova(&self) -> Option<RegistroDeProva> {
        Some(RegistroDeProva {
            tarefa: self.id,
            entrada: self.entrada?,
            especificacao: self.especificacao,
            metodo: self.metodo,
            resultado: self.resultado?,
            operacoes: self.operacoes,
            worker: self.worker?,
            inicio_ms: self.inicio_ms?,
            fim_ms: self.fim_ms?,
        })
    }
}
