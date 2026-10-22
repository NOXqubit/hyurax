// ✝ Provérbios 27:17 — “Como o ferro com o ferro se afia, assim o homem afia o rosto do seu amigo.”
//! Temporada de ataques, rodada interna: fuzzing de todo decodificador que
//! recebe bytes de outro computador (o fio, os blocos, as transações, a malha,
//! o ULTRAX, a nuvem, os vouchers e o Éter).
//!
//! Cada alvo parte de bytes válidos (os vetores do gabarito Python, ou objetos
//! montados aqui) e recebe mutações: bit trocado, byte sorteado, corte,
//! inserção, remoção, pedaço repetido e campo de tamanho inflado (u32 e u64
//! nos valores que derrubam decodificador ingênuo). Duas regras:
//!
//! 1. **Nada derruba.** Nenhuma entrada pode dar pânico nem demorar.
//! 2. **Uma codificação só.** O que for aceito tem que voltar idêntico ao ser
//!    recodificado; senão o mesmo bloco ou a mesma transação circularia com
//!    bytes diferentes.
//!
//! Determinístico: a mesma semente sorteia as mesmas mutações, então qualquer
//! falha se reproduz. Para rodar mais pesado:
//! `HYX_FUZZ=200000 HYX_FUZZ_SEMENTE=7 cargo test -p hyurax-nucleo --test fuzz`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use hyurax_block::{Block, BlockHeader};
use hyurax_crypto::ed25519_public_key;
use hyurax_net::malha::{AnuncioVizinho, MensagemMalha, Pacote, Prefixo};
use hyurax_nuvem::mensagem::MensagemNuvem;
use hyurax_nuvem::plano::Voucher;
use hyurax_ultrax::job::EspecificacaoDeJob;
use hyurax_ultrax::prova::RegistroDeProva;
use hyurax_ultrax::rede::MensagemUltrax;
use hyurax_ultrax::trabalho::{Especificacao, MetodoDeVerificacao, TipoDeTrabalho};
use hyurax_usefulpow::UsefulWorkProof;
use serde_json::Value;

/// splitmix64: pequeno, rápido e igual em toda máquina.
struct Sorteio(u64);

impl Sorteio {
    fn proximo(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn ate(&mut self, n: usize) -> usize {
        (self.proximo() % n.max(1) as u64) as usize
    }
}

/// Os valores que derrubam decodificador ingênuo num campo de tamanho.
const U32_FEIOS: [u32; 7] = [u32::MAX, 0x7FFF_FFFF, 0x8000_0000, 0x0100_0000, 0x0001_0000, 0xFFFF, 0];
const U64_FEIOS: [u64; 5] = [u64::MAX, i64::MAX as u64, 1 << 32, 1 << 63, 0];

fn mutar(s: &mut Sorteio, base: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    for _ in 0..=s.ate(4) {
        let n = v.len();
        match s.ate(9) {
            0 if n > 0 => {
                let i = s.ate(n);
                v[i] ^= 1 << s.ate(8);
            }
            1 if n > 0 => {
                let i = s.ate(n);
                v[i] = s.proximo() as u8;
            }
            2 if n > 0 => v.truncate(s.ate(n)),
            3 => {
                let i = s.ate(n + 1);
                v.insert(i, s.proximo() as u8);
            }
            4 if n > 0 => {
                v.remove(s.ate(n));
            }
            5 if n >= 4 => {
                let i = s.ate(n - 3);
                v[i..i + 4].copy_from_slice(&U32_FEIOS[s.ate(U32_FEIOS.len())].to_be_bytes());
            }
            6 if n >= 8 => {
                let i = s.ate(n - 7);
                v[i..i + 8].copy_from_slice(&U64_FEIOS[s.ate(U64_FEIOS.len())].to_be_bytes());
            }
            7 if n > 0 => {
                let i = s.ate(n);
                let j = (i + 1 + s.ate(64)).min(n);
                let pedaco = v[i..j].to_vec();
                let onde = s.ate(v.len() + 1);
                v.splice(onde..onde, pedaco);
            }
            _ => {
                for _ in 0..s.ate(16) {
                    v.push(s.proximo() as u8);
                }
            }
        }
    }
    v
}

/// Decodifica e, se aceitar, recodifica. `None` = recusou (o esperado para
/// quase toda mutação). `Some(bytes)` = aceitou, e estes são os bytes
/// canônicos do que foi lido.
type Prova = fn(&[u8]) -> Option<Vec<u8>>;

struct Alvo {
    nome: &'static str,
    corpus: Vec<Vec<u8>>,
    provar: Prova,
}

fn vetor(nome: &str) -> Value {
    let caminho = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vectors").join(format!("{nome}.json"));
    let texto = std::fs::read_to_string(&caminho).unwrap_or_else(|e| panic!("{}: {e}", caminho.display()));
    serde_json::from_str::<Value>(&texto).unwrap()["data"].clone()
}

fn hex(v: &Value) -> Vec<u8> {
    let t = v.as_str().unwrap_or_else(|| panic!("esperava hex: {v}"));
    (0..t.len()).step_by(2).map(|i| u8::from_str_radix(&t[i..i + 2], 16).unwrap()).collect()
}

/// Todos os valores de `campo` nos itens de uma lista.
fn de_lista(lista: &Value, campo: &str) -> Vec<Vec<u8>> {
    lista.as_array().unwrap().iter().map(|x| hex(&x[campo])).collect()
}

fn para_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

const MAGIC_REGTEST: [u8; 4] = *b"HYXR";

fn alvos() -> Vec<Alvo> {
    let fio = vetor("wire");
    let cadeia = vetor("chain");
    let genese = vetor("genesis");
    let txs = vetor("transactions");
    let txs_borda = vetor("transactions_edge");
    let util = vetor("usefulpow");
    let malha = vetor("malha");
    let ultrax = vetor("rede_ultrax");
    let nuvem = vetor("nuvem");
    let plano = vetor("plano");
    let job = vetor("job");
    assert_eq!(hex(&fio["magic"]), MAGIC_REGTEST, "os vetores do fio são da rede de teste local");

    let mut quadros = de_lista(&fio["valid"], "frame");
    quadros.extend(de_lista(&fio["invalid"], "frame"));
    let mut blocos = de_lista(&cadeia["blocks"], "block_bytes");
    blocos.extend(de_lista(&genese, "block_bytes"));
    let mut cabecalhos = de_lista(&cadeia["blocks"], "header_bytes");
    cabecalhos.extend(de_lista(&genese, "header_bytes"));
    let mut transacoes = de_lista(&txs["coinbases"], "encoded");
    transacoes.extend(de_lista(&txs["transfers"], "encoded"));
    transacoes.extend(de_lista(&txs_borda, "encoded"));
    let mut provas = de_lista(&util["proofs"], "encoded");
    provas.extend(de_lista(&util["rejections"], "encoded"));
    let mut msgs_malha = de_lista(&malha["mensagens"], "corpo");
    msgs_malha.extend(de_lista(&malha["recusados"], "corpo"));
    let prefixos = ["circuito", "reserva", "verificar"].iter().map(|k| hex(&malha["prefixos"][k])).collect();
    let mut msgs_ultrax = de_lista(&ultrax["mensagens"], "corpo");
    msgs_ultrax.extend(de_lista(&ultrax["recusados"], "corpo"));
    msgs_ultrax.push(hex(&ultrax["oferta_adulterada"]["corpo"]));
    // registros de prova como os que viajam dentro das mensagens de resultado
    let registros: Vec<Vec<u8>> = [(TipoDeTrabalho::Matriz, 8, 0), (TipoDeTrabalho::Mochila, 40, 0), (TipoDeTrabalho::Difusao, 4, 2), (TipoDeTrabalho::Ia, 4, 3)]
        .into_iter()
        .map(|(tipo, tamanho, passos)| {
            RegistroDeProva {
                tarefa: [1; 64],
                entrada: [2; 64],
                especificacao: Especificacao::nova(tipo, tamanho, passos).unwrap(),
                metodo: MetodoDeVerificacao::Recomputacao,
                resultado: [3; 64],
                operacoes: 12_345,
                worker: ed25519_public_key(&[7; 32]),
                inicio_ms: 10,
                fim_ms: 20,
            }
            .codificar()
        })
        .collect();
    let mut msgs_nuvem = de_lista(&nuvem["mensagens"], "corpo");
    msgs_nuvem.extend(de_lista(&nuvem["recusados"], "corpo"));
    let mut vouchers = de_lista(&plano["vouchers"], "bytes");
    vouchers.extend(de_lista(&plano["recusas"], "bytes"));
    vouchers.extend(de_lista(&plano["nao_confere"], "bytes"));
    let jobs = de_lista(&job["jobs"], "codificacao");
    let objeto: Vec<u8> = (0..3_000u32).map(|i| (i % 251) as u8).collect();
    let (manifesto, fragmentos) = hyurax_eter::fatiar(&objeto, "foto.jpg", 700).unwrap();
    let eter = [
        hyurax_eter::Quadro::Manifesto(manifesto.clone()),
        hyurax_eter::Quadro::Fragmento(fragmentos[1].clone()),
        hyurax_eter::Quadro::Faltando { id: manifesto.id, indices: vec![0, 3, 4] },
    ]
    .iter()
    .map(|q| q.codificar().unwrap())
    .collect();

    vec![
        Alvo {
            nome: "quadro do fio",
            corpus: quadros,
            provar: |m| {
                let q = hyurax_wire::decode_frame(&MAGIC_REGTEST, m).ok()?;
                let lido = m.len() - q.resto.len();
                // devolve o que foi consumido, recodificado, mais o resto como veio
                let mut v = hyurax_wire::encode_frame(&MAGIC_REGTEST, &q.message).ok()?;
                assert!(lido <= m.len());
                v.extend_from_slice(q.resto);
                Some(v)
            },
        },
        Alvo { nome: "bloco", corpus: blocos, provar: |m| Block::decode(m).ok().map(|b| b.encode().unwrap()) },
        Alvo { nome: "cabeçalho", corpus: cabecalhos, provar: |m| BlockHeader::decode(m).ok().map(|h| h.encode().to_vec()) },
        Alvo { nome: "transação", corpus: transacoes, provar: |m| hyurax_tx::decode_tx(m).ok().map(|t| t.encode().unwrap()) },
        Alvo { nome: "prova útil", corpus: provas, provar: |m| UsefulWorkProof::decode(m).ok().map(|p| p.encode().unwrap()) },
        Alvo { nome: "mensagem da malha", corpus: msgs_malha, provar: |m| MensagemMalha::decodificar(m).ok().map(|x| x.codificar().unwrap()) },
        Alvo { nome: "prefixo da malha", corpus: prefixos, provar: |m| Prefixo::ler(m).ok().map(|x| x.bytes()) },
        Alvo { nome: "anúncio de vizinho", corpus: vec![hex(&malha["vizinho"]["corpo"])], provar: |m| AnuncioVizinho::ler(m).ok().map(|x| x.bytes()) },
        Alvo {
            nome: "pacote do Éter (malha)",
            corpus: vec![hex(&malha["pacote"]["corpo"]), hex(&malha["pacote"]["recusado_sobra"])],
            provar: |m| Pacote::ler(m).ok().map(|x| x.bytes().unwrap()),
        },
        Alvo { nome: "mensagem do ULTRAX", corpus: msgs_ultrax, provar: |m| MensagemUltrax::decodificar(m).ok().map(|x| x.codificar().unwrap()) },
        Alvo { nome: "registro de prova", corpus: registros, provar: |m| RegistroDeProva::decodificar(m).ok().map(|x| x.codificar()) },
        Alvo { nome: "mensagem da nuvem", corpus: msgs_nuvem, provar: |m| MensagemNuvem::decodificar(m).ok().map(|x| x.codificar().unwrap()) },
        Alvo { nome: "voucher de plano", corpus: vouchers, provar: |m| Voucher::ler(m).ok().map(|x| x.bytes().unwrap()) },
        Alvo { nome: "especificação de JOB", corpus: jobs, provar: |m| EspecificacaoDeJob::decodificar(m).ok().map(|x| x.codificar().unwrap()) },
        Alvo { nome: "quadro do Éter", corpus: eter, provar: |m| hyurax_eter::Quadro::decodificar(m).ok().map(|x| x.codificar().unwrap()) },
    ]
}

/// Quanto uma decodificação pode levar, mesmo num Atom em modo de depuração.
const PRAZO: Duration = Duration::from_secs(2);

#[test]
fn nenhum_decodificador_de_fora_cai_nem_aceita_duas_codificacoes() {
    let rodadas: usize = std::env::var("HYX_FUZZ").ok().and_then(|v| v.parse().ok()).unwrap_or(1_500);
    let semente: u64 = std::env::var("HYX_FUZZ_SEMENTE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let alvos = alvos();
    // o pânico vira falha anotada com a entrada, sem poluir a saída
    std::panic::set_hook(Box::new(|_| {}));
    let mut falhas = Vec::new();
    let mut resumo = Vec::new();
    for alvo in alvos {
        // os exemplos válidos: os aceitos voltam idênticos
        let mut validos = 0;
        for c in &alvo.corpus {
            if let Ok(Some(v)) = catch_unwind(AssertUnwindSafe(|| (alvo.provar)(c))) {
                validos += 1;
                if &v != c {
                    falhas.push(format!("{}: exemplo do corpus não volta igual: {}", alvo.nome, para_hex(c)));
                }
            }
        }
        if validos == 0 {
            falhas.push(format!("{}: nenhum exemplo do corpus foi aceito (corpus errado?)", alvo.nome));
        }
        let mut s = Sorteio(semente ^ (alvo.nome.len() as u64).wrapping_mul(0x1000_0001));
        let (mut aceitos, mut nesta) = (0usize, 0usize);
        for _ in 0..rodadas {
            let base = &alvo.corpus[s.ate(alvo.corpus.len())];
            let m = mutar(&mut s, base);
            let inicio = Instant::now();
            let r = catch_unwind(AssertUnwindSafe(|| (alvo.provar)(&m)));
            let levou = inicio.elapsed();
            let problema = match r {
                Err(e) => {
                    let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string())).unwrap_or_default();
                    Some(format!("pânico ({msg})"))
                }
                Ok(Some(v)) => {
                    aceitos += 1;
                    (v != m).then(|| format!("aceitou bytes não canônicos (recodificado: {})", para_hex(&v)))
                }
                Ok(None) => None,
            };
            let problema = problema.or_else(|| (levou > PRAZO).then(|| format!("levou {levou:?}")));
            if let Some(p) = problema {
                if nesta < 3 {
                    falhas.push(format!("{}: {p} com a entrada {}", alvo.nome, para_hex(&m)));
                }
                nesta += 1;
            }
        }
        resumo.push(format!("{}: {rodadas} mutações, {aceitos} aceitas, {nesta} problema(s)", alvo.nome));
    }
    let _ = std::panic::take_hook();
    println!("semente {semente}\n{}", resumo.join("\n"));
    assert!(falhas.is_empty(), "{} problema(s):\n{}\n\n{}", falhas.len(), falhas.join("\n"), resumo.join("\n"));
}

fn numero(v: &Value) -> u32 {
    u32::try_from(v.as_u64().unwrap()).unwrap()
}

fn numeros(v: &Value) -> Vec<u32> {
    v.as_array().map(|a| a.iter().map(numero).collect()).unwrap_or_default()
}

/// Os 8 motores, cada um com uma unidade pequena de verdade: especificação,
/// semente e se o resultado é único (qualquer outro byte tem que ser recusado).
fn unidades() -> Vec<(Especificacao, Vec<u8>, bool)> {
    let semente = vec![5u8; 64];
    let gene = vetor("genetica");
    let mel = vetor("melhoramento");
    let rot = vetor("rotas");
    let tri = vetor("triagem");
    let g = &gene["casos"][0];
    let m = &mel["casos"][0];
    // a maior rota pequena dos vetores
    let r = rot["casos"].as_array().unwrap().iter().filter(|c| numero(&c["tamanho"]) <= 60).max_by_key(|c| numero(&c["tamanho"])).unwrap();
    let t = &tri["casos"][0];
    vec![
        (Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap(), semente.clone(), true),
        // na mochila vale qualquer escolha ótima, não só a do worker honesto
        (Especificacao::nova(TipoDeTrabalho::Mochila, 20, 0).unwrap(), semente.clone(), false),
        (Especificacao::nova(TipoDeTrabalho::Difusao, 6, 3).unwrap(), semente.clone(), true),
        (Especificacao::nova(TipoDeTrabalho::Ia, 4, 3).unwrap(), semente.clone(), true),
        (Especificacao::nova_com(TipoDeTrabalho::Genetica, numero(&g["tamanho"]), numero(&g["passos"]), &numeros(&g["parametros"])).unwrap(), hex(&gene["semente"]), true),
        (Especificacao::nova_com(TipoDeTrabalho::Melhoramento, numero(&m["tamanho"]), numero(&m["passos"]), &numeros(&m["parametros"])).unwrap(), hex(&mel["semente"]), true),
        // na rota, o certificado aceita outra rota que também seja ótimo local
        (Especificacao::nova_com(TipoDeTrabalho::Rotas, numero(&r["tamanho"]), numero(&r["passos"]), &[numero(&r["instancia"])]).unwrap(), hex(&r["semente"]), false),
        (Especificacao::nova_com(TipoDeTrabalho::Triagem, numero(&t["tamanho"]), 0, &numeros(&t["parametros"])).unwrap(), Vec::new(), true),
    ]
}

/// O resultado que chega de outro nó passa por `trabalho::verificar`. Ele não
/// pode cair nem travar com bytes de atacante, e nos motores de resultado
/// único só o resultado verdadeiro passa.
#[test]
fn conferencia_dos_motores_nao_cai_nem_aceita_resultado_falso() {
    use hyurax_ultrax::trabalho::{executar, verificar};
    let rodadas: usize = std::env::var("HYX_FUZZ").ok().and_then(|v| v.parse::<usize>().ok()).map_or(150, |n| (n / 10).max(1));
    let semente: u64 = std::env::var("HYX_FUZZ_SEMENTE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let unidades = unidades();
    std::panic::set_hook(Box::new(|_| {}));
    let mut falhas = Vec::new();
    let mut resumo = Vec::new();
    for (esp, semente_da_unidade, unico) in unidades {
        let nome = esp.resumo();
        let certo = executar(&esp, &semente_da_unidade, &mut |_| true).unwrap().resultado;
        let inicio = Instant::now();
        let ok = verificar(&esp, &semente_da_unidade, &certo);
        let normal = inicio.elapsed();
        if let Err(e) = ok {
            falhas.push(format!("{nome}: o resultado verdadeiro foi recusado: {e:?}"));
            continue;
        }
        // a conferência de lixo não pode custar muito mais que a do certo
        let prazo = PRAZO.max(normal * 20);
        let mut s = Sorteio(semente ^ u64::from(esp.tipo().codigo()).wrapping_mul(0x1000_0001));
        let (mut aceitos, mut nesta) = (0usize, 0usize);
        for _ in 0..rodadas {
            let m = mutar(&mut s, &certo);
            if m == certo {
                continue;
            }
            let inicio = Instant::now();
            let r = catch_unwind(AssertUnwindSafe(|| verificar(&esp, &semente_da_unidade, &m)));
            let levou = inicio.elapsed();
            let problema = match r {
                Err(e) => {
                    let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string())).unwrap_or_default();
                    Some(format!("pânico ({msg})"))
                }
                Ok(Ok(())) => {
                    aceitos += 1;
                    unico.then(|| "aceitou um resultado diferente do verdadeiro".to_string())
                }
                Ok(Err(_)) => None,
            };
            let problema = problema.or_else(|| (levou > prazo).then(|| format!("levou {levou:?} (o certo leva {normal:?})")));
            if let Some(p) = problema {
                if nesta < 3 {
                    falhas.push(format!("{nome}: {p} com {} bytes: {}", m.len(), para_hex(&m[..m.len().min(96)])));
                }
                nesta += 1;
            }
        }
        resumo.push(format!("{nome}: {rodadas} mutações, {aceitos} aceitas, {nesta} problema(s), conferência normal {normal:?}"));
    }
    let _ = std::panic::take_hook();
    println!("semente {semente}\n{}", resumo.join("\n"));
    assert!(falhas.is_empty(), "{} problema(s):\n{}\n\n{}", falhas.len(), falhas.join("\n"), resumo.join("\n"));
}
