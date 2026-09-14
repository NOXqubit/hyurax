// ✝ Provérbios 13:11 — “A riqueza de procedência vã diminuirá, mas quem a ajunta com o próprio trabalho a aumentará.”
//! Nó do Auron: carteira com senha, mineração, envio de AUR de teste e saldo,
//! numa cadeia gravada no disco e sincronizada com a rede.
//!
//! ```text
//! auron-no carteira nova    --arquivo carteira.txt
//! auron-no carteira ver     --arquivo carteira.txt
//! auron-no carteira cifrar  --arquivo carteira.txt
//! auron-no minerar          --rede testnet --pasta dados --endereco HEX [--blocos N] [--linhas L]
//! auron-no enviar           --rede testnet --pasta dados --arquivo carteira.txt --para HEX --valor AUR
//! auron-no no               --rede testnet --pasta dados --porta P
//! auron-no estado           --rede testnet --pasta dados [--endereco HEX]
//! ```
//!
//! Cada bloco minerado ou recebido passa pela validação completa, e a cadeia é
//! gravada no disco a cada mudança. A conexão entre nós é cifrada (Noise XX), e
//! o nó guarda a própria identidade em `PASTA/no.chave`.

mod carteira;
mod senha;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, Instant};

use auron_block::Block;
use auron_chain::Chain;
use auron_consensus::{ParametrosRede, block_reward};
use auron_crypto::{ADDRESS_LEN, SECRET_LEN};
use auron_net::entropia::{entropia_do_sistema, preencher};
use auron_net::{Identidade, No, Rede};
use auron_pow::ConfigMineracao;
use auron_store::{load_chain, save_chain};
use auron_tx::{AUR, Output, sign_transfer_outputs};

const AJUDA: &str = "\
auron-no — nó do Auron (rede de TESTE)

  auron-no carteira nova --arquivo carteira.txt
      Cria uma chave nova, cifrada com senha, e mostra o endereço.
      Recusa sobrescrever arquivo.

  auron-no carteira ver --arquivo carteira.txt
      Mostra o endereço de uma carteira existente (não pede senha).

  auron-no carteira cifrar --arquivo carteira.txt
      Converte uma carteira antiga, com o segredo em texto, para o formato com senha.

  auron-no enviar --rede testnet --pasta dados --arquivo carteira.txt --para HEX --valor AUR
                  [--taxa AUR] [--semente IP:PORTA,...] [--porta P]
      Assina uma transferência com a carteira (pede a senha) e manda para a rede.

  auron-no minerar --rede testnet --pasta dados --endereco HEX [--blocos N] [--linhas L]
                   [--porta P] [--semente IP:PORTA,...] [--pausa-ms X]
      Minera N blocos (padrão 1; 0 = sem parar) e grava a cadeia depois de cada um.
      Com --porta e/ou --semente, entra na rede: sincroniza e propaga o que minerar.

  auron-no no --rede testnet --pasta dados [--porta P] [--semente IP:PORTA,...] [--exportar estado.json]
      Só roda o nó: escuta, sincroniza, serve e propaga. Sem minerar.
      --exportar grava um resumo público da cadeia em JSON (explorador de blocos).
      Sem --semente, a testnet usa as sementes embutidas; --sem-sementes-padrao desliga.

  auron-no estado --rede testnet --pasta dados [--endereco HEX]
      Mostra a altura da cadeia e, com --endereco, o saldo.

Dois aparelhos na mesma rede local, por exemplo:
  no PC:      auron-no no --porta 8790 --pasta dados
  no celular: auron-no minerar --porta 8790 --semente IP_DO_PC:8790 --endereco SEU_ENDERECO --blocos 0

Redes: mainnet (difícil: 16 bits de trabalho por bloco), testnet (8 bits), regtest (quase nada).
Senha em script: variável AURON_SENHA (conveniente, e menos segura que digitar).
Lembrete: a rede pública não existe e o AUR não tem valor. Isto é teste.
";

struct Opcoes {
    rede: ParametrosRede,
    pasta: PathBuf,
    arquivo: Option<PathBuf>,
    endereco: Option<[u8; ADDRESS_LEN]>,
    blocos: u64,
    linhas: u32,
    porta: u16,
    sementes: Vec<String>,
    pausa_ms: u64,
    para: Option<[u8; ADDRESS_LEN]>,
    valor: Option<u64>,
    taxa: u64,
    exportar: Option<PathBuf>,
    sem_sementes_padrao: bool,
}

/// Nós semente da rede de teste pública, embutidos no programa. Quem não passa
/// `--semente` conecta neles. Vazio enquanto nenhum semente estiver no ar: a
/// lista só ganha um endereço depois de ele responder de verdade (ver
/// docs/NO-SEMENTE.md).
const SEMENTES_TESTNET: &[&str] = &[];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn de_hex<const N: usize>(texto: &str) -> Option<[u8; N]> {
    let texto = texto.trim();
    if texto.len() != N.checked_mul(2)? {
        return None;
    }
    let mut saida = [0u8; N];
    for (i, byte) in saida.iter_mut().enumerate() {
        *byte = u8::from_str_radix(texto.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(saida)
}

fn ler_opcoes(args: &[String]) -> Result<Opcoes, String> {
    let nucleos = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut o = Opcoes {
        rede: ParametrosRede::TESTNET,
        pasta: PathBuf::from("dados-auron"),
        arquivo: None,
        endereco: None,
        blocos: 1,
        linhas: u32::try_from((nucleos / 2).max(1)).unwrap_or(1),
        porta: 0,
        sementes: Vec::new(),
        pausa_ms: 0,
        para: None,
        valor: None,
        taxa: 0,
        exportar: None,
        sem_sementes_padrao: false,
    };
    let mut it = args.iter();
    while let Some(nome) = it.next() {
        if nome == "--sem-sementes-padrao" {
            o.sem_sementes_padrao = true;
            continue;
        }
        let valor = it.next().ok_or_else(|| format!("{nome} precisa de um valor"))?;
        match nome.as_str() {
            "--rede" => {
                o.rede = ParametrosRede::da_rede(valor).ok_or(format!("rede desconhecida: {valor}"))?;
            }
            "--pasta" => o.pasta = PathBuf::from(valor),
            "--arquivo" => o.arquivo = Some(PathBuf::from(valor)),
            "--endereco" => {
                o.endereco = Some(de_hex(valor).ok_or("--endereco precisa de 40 dígitos hexadecimais")?);
            }
            "--blocos" => o.blocos = valor.parse().map_err(|_| "--blocos precisa ser número")?,
            "--porta" => o.porta = valor.parse().map_err(|_| "--porta precisa ser número")?,
            "--semente" => {
                o.sementes.extend(valor.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));
            }
            "--pausa-ms" => o.pausa_ms = valor.parse().map_err(|_| "--pausa-ms precisa ser número")?,
            "--para" => o.para = Some(de_hex(valor).ok_or("--para precisa de 40 dígitos hexadecimais")?),
            "--valor" => o.valor = Some(unidades_de_aur(valor)?),
            "--taxa" => o.taxa = unidades_de_aur(valor)?,
            "--exportar" => o.exportar = Some(PathBuf::from(valor)),
            "--linhas" => {
                o.linhas = valor
                    .parse::<u32>()
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or("--linhas precisa ser pelo menos 1")?;
            }
            _ => return Err(format!("opção desconhecida: {nome}")),
        }
    }
    if o.sementes.is_empty() && !o.sem_sementes_padrao && o.rede.nome == ParametrosRede::TESTNET.nome {
        o.sementes = SEMENTES_TESTNET.iter().map(|s| (*s).to_string()).collect();
    }
    Ok(o)
}

/// 1 AUR = 100 000 000 unidades (seção 1 da especificação).
const AUR_UNIDADE: u128 = 100_000_000;

fn aur(unidades: u128) -> String {
    format!("{}.{:08}", unidades / AUR_UNIDADE, unidades % AUR_UNIDADE)
}

/// "1.5" vira 150 000 000 unidades. No máximo 8 casas; nunca ponto flutuante.
fn unidades_de_aur(texto: &str) -> Result<u64, String> {
    let erro = || format!("valor inválido: {texto} (use ponto, até 8 casas, por exemplo 1.5)");
    let (inteiro, fracao) = texto.trim().split_once('.').unwrap_or((texto.trim(), ""));
    if inteiro.is_empty() || fracao.len() > 8 || !inteiro.chars().chain(fracao.chars()).all(|c| c.is_ascii_digit()) {
        return Err(erro());
    }
    let inteiro: u64 = inteiro.parse().map_err(|_| erro())?;
    let fracao: u64 = format!("{fracao:0<8}").parse().map_err(|_| erro())?;
    inteiro.checked_mul(100_000_000).and_then(|v| v.checked_add(fracao)).ok_or_else(erro)
}

// -- carteira --

fn pedir_senha_nova() -> Result<String, String> {
    let senha = senha::ler("Senha nova da carteira (mínimo 10 caracteres)")?;
    carteira::senha_aceitavel(&senha)?;
    if std::env::var(senha::VARIAVEL).is_err() && senha::ler("Repita a senha")? != senha {
        return Err("as duas senhas não são iguais".into());
    }
    Ok(senha)
}

/// Grava num arquivo temporário e troca de nome: nunca deixa carteira pela metade.
fn gravar_carteira(arquivo: &Path, conteudo: &str) -> Result<(), String> {
    let temporario = arquivo.with_extension("tmp");
    std::fs::write(&temporario, conteudo).map_err(|e| format!("não consegui gravar: {e}"))?;
    std::fs::rename(&temporario, arquivo).map_err(|e| format!("não consegui gravar: {e}"))
}

fn cifrar_segredo(segredo: &[u8; SECRET_LEN], senha: &str) -> Result<String, String> {
    let mut aleatorio = [0u8; 28];
    preencher(&mut aleatorio)?;
    carteira::cifrar(segredo, senha, &aleatorio)
}

fn ler_arquivo(arquivo: &Path) -> Result<String, String> {
    std::fs::read_to_string(arquivo).map_err(|e| format!("não consegui ler {}: {e}", arquivo.display()))
}

fn comando_carteira(args: &[String]) -> Result<(), String> {
    let (acao, resto) = args.split_first().ok_or("use: carteira nova|ver|cifrar --arquivo ARQUIVO")?;
    let o = ler_opcoes(resto)?;
    let arquivo = o.arquivo.ok_or("falta --arquivo")?;
    match acao.as_str() {
        "nova" => {
            if arquivo.exists() {
                return Err(format!("{} já existe; não sobrescrevo carteira", arquivo.display()));
            }
            let senha = pedir_senha_nova()?;
            // A chave vem direto do sistema operacional, não do gerador derivado.
            let segredo = entropia_do_sistema()?;
            let conteudo = cifrar_segredo(&segredo, &senha)?;
            gravar_carteira(&arquivo, &conteudo)?;
            println!("Carteira criada em {}", arquivo.display());
            println!("Endereço: {}", hex(&carteira::endereco(&conteudo)?));
            println!("Guarde uma cópia do arquivo e NÃO esqueça a senha: sem os dois, o saldo fica perdido.");
            Ok(())
        }
        "ver" => {
            let texto = ler_arquivo(&arquivo)?;
            println!("Endereço: {}", hex(&carteira::endereco(&texto)?));
            if carteira::e_formato_antigo(&texto) {
                println!("Aviso: esta carteira guarda o segredo em texto. Proteja com: auron-no carteira cifrar --arquivo {}", arquivo.display());
            }
            Ok(())
        }
        "cifrar" => {
            let texto = ler_arquivo(&arquivo)?;
            if !carteira::e_formato_antigo(&texto) {
                return Err("esta carteira já está cifrada".into());
            }
            let segredo = carteira::abrir(&texto, "")?;
            let senha = pedir_senha_nova()?;
            let conteudo = cifrar_segredo(&segredo, &senha)?;
            gravar_carteira(&arquivo, &conteudo)?;
            println!("Carteira {} agora está cifrada com senha.", arquivo.display());
            println!("Se existir cópia antiga do arquivo em outro lugar, apague: ela continua sem senha.");
            Ok(())
        }
        outro => Err(format!("ação desconhecida: {outro}")),
    }
}

// -- cadeia --

fn arquivo_da_cadeia(o: &Opcoes) -> PathBuf {
    o.pasta.join(format!("{}.cadeia", o.rede.nome))
}

fn abrir_cadeia(o: &Opcoes) -> Result<Chain, String> {
    let arquivo = arquivo_da_cadeia(o);
    if arquivo.exists() {
        // o próprio disco: pula só o Argon2id, e revalida todo o resto
        load_chain(&arquivo, true, Some(o.rede)).map_err(|e| e.to_string())
    } else {
        Chain::nova(o.rede).map_err(|e| e.to_string())
    }
}

/// A identidade do nó na cifra da rede: `PASTA/no.chave`. Criada na primeira
/// vez; depois, a mesma a cada execução. Apagar o arquivo dá identidade nova.
fn identidade_do_no(o: &Opcoes) -> Result<Identidade, String> {
    let arquivo = o.pasta.join("no.chave");
    if arquivo.exists() {
        let texto = ler_arquivo(&arquivo)?;
        let segredo: [u8; 32] = texto
            .lines()
            .find_map(|l| l.trim().strip_prefix("segredo="))
            .and_then(de_hex)
            .ok_or_else(|| format!("{} inválido; apague para gerar outro", arquivo.display()))?;
        return Identidade::de_segredo(segredo).map_err(|e| e.to_string());
    }
    let identidade = Identidade::de_segredo(entropia_do_sistema()?).map_err(|e| e.to_string())?;
    let conteudo = format!(
        "# Identidade deste nó na rede Auron (chave da cifra entre nós).\n\
         # Não é carteira e não guarda saldo. Apagar gera uma identidade nova.\n\
         segredo={}\npublica={}\n",
        hex(identidade.segredo()),
        hex(&identidade.publica())
    );
    gravar_carteira(&arquivo, &conteudo)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&arquivo, std::fs::Permissions::from_mode(0o600));
    }
    Ok(identidade)
}

/// Sobe a rede: abre a cadeia do disco, escuta (se `--porta`) e conecta às
/// sementes (`--semente`, separadas por vírgula).
fn subir_rede(o: &Opcoes) -> Result<Arc<Rede>, String> {
    std::fs::create_dir_all(&o.pasta).map_err(|e| format!("não consegui criar a pasta: {e}"))?;
    let cadeia = abrir_cadeia(o)?;
    let identidade = identidade_do_no(o)?;
    println!("Identidade do nó: {}", hex(&identidade.publica()));
    let rede = Rede::com_identidade(No::novo(cadeia), identidade);

    if o.porta > 0 {
        let porta = rede
            .escutar(("0.0.0.0", o.porta))
            .map_err(|e| format!("não consegui escutar na porta {}: {e}", o.porta))?;
        println!("Escutando na porta {porta}");
    }
    for semente in o.sementes.iter().filter(|s| !s.is_empty()) {
        rede.semear(semente);
        match rede.conectar(semente.as_str()) {
            Ok(()) => println!("Conectando em {semente}"),
            Err(e) => println!("  aviso: não consegui conectar em {semente}: {e}"),
        }
    }
    Ok(rede)
}

/// Grava a cadeia no disco.
fn salvar(rede: &Arc<Rede>, o: &Opcoes) -> Result<(), String> {
    let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
    save_chain(&no.chain, &arquivo_da_cadeia(o)).map_err(|e| e.to_string())?;
    Ok(())
}

fn minerar(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let endereco = o.endereco.ok_or("falta --endereco (crie com: auron-no carteira nova --arquivo carteira.txt)")?;
    let rede = subir_rede(&o)?;
    // Minerar antes de alcançar a rede é minerar num ramo que vai ser jogado fora.
    if !o.sementes.is_empty() {
        println!("Sincronizando com a rede antes de minerar…");
        esperar_sincronizar(&rede, &o);
    }
    {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        println!(
            "Rede {} · altura {} · {} linha(s), {:.0} MiB",
            o.rede.nome,
            no.chain.height(),
            o.linhas,
            (u64::from(o.rede.pow.memoria_kib) * u64::from(o.linhas)) as f64 / 1024.0
        );
    }

    let mut feitos = 0u64;
    let mut perdidos = 0u64;
    while o.blocos == 0 || feitos < o.blocos {
        let inicio = Instant::now();

        // Monta o candidato com o cadeado, e minera SEM ele: a busca do nonce
        // demora, e travar o nó nesse tempo pararia a rede.
        let (candidato, pow) = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            let transfers = no.mempool_ordenado();
            let candidato = no
                .chain
                .build_candidate(endereco, transfers, None, Vec::new())
                .map_err(|e| e.to_string())?;
            (candidato, o.rede.pow)
        };

        let config = ConfigMineracao {
            linhas: o.linhas,
            nonce_inicial: 0,
            limite: Some(1u64 << 32),
            pausa: Duration::from_millis(o.pausa_ms),
        };
        let achado = auron_pow::minerar(
            &candidato.header.encode(),
            pow,
            config,
            &AtomicBool::new(false),
            &AtomicU64::new(0),
        )
        .map_err(|e| e.to_string())?
        .achado
        .ok_or("não achei nonce no limite de tentativas")?;

        let altura = candidato.header.height;
        let n = candidato.useful_proof.as_ref().map_or(0, |p| p.n);
        let bits = candidato.header.bits;
        let bloco = Block { header: candidato.header.with_nonce(achado.nonce), ..candidato };

        // Enquanto eu minerava, a rede pode ter achado outro bloco na mesma
        // altura. Aí este vira órfão e eu recomeço: é a corrida normal.
        match rede.submeter_bloco(bloco) {
            Ok(true) => {
                salvar(&rede, &o)?;
                feitos = feitos.saturating_add(1);
                println!(
                    "  bloco {altura} em {:.1} s · trabalho útil {n}×{n} · bits {bits:#010x} · recompensa {} AUR (libera em {} blocos) · {} par(es)",
                    inicio.elapsed().as_secs_f64(),
                    aur(u128::from(block_reward(altura, &o.rede))),
                    o.rede.coinbase_maturity,
                    rede.pares_conectados()
                );
            }
            Ok(false) | Err(_) => {
                perdidos = perdidos.saturating_add(1);
                println!("  bloco {altura} perdido na corrida (outro nó chegou primeiro); recomeçando");
            }
        }
    }
    if perdidos > 0 {
        println!("{feitos} bloco(s) meus, {perdidos} perdido(s) na corrida.");
    }
    salvar(&rede, &o)?;
    Ok(())
}

/// Espera alcançar o trabalho que os pares anunciaram (no máximo 90 s).
fn esperar_sincronizar(rede: &Arc<Rede>, o: &Opcoes) {
    if o.sementes.is_empty() {
        return;
    }
    let inicio = Instant::now();
    while inicio.elapsed() < Duration::from_secs(90) {
        if rede.alcancou_os_pares() {
            return;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    println!("  aviso: não alcancei a rede em 90 s; seguindo com a cadeia que tenho");
}

fn enviar(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let arquivo = o.arquivo.clone().ok_or("falta --arquivo (a carteira que paga)")?;
    let para = o.para.ok_or("falta --para (endereço de destino, 40 dígitos hexadecimais)")?;
    let valor = o.valor.filter(|&v| v > 0).ok_or("falta --valor maior que zero (em AUR, por exemplo 1.5)")?;
    let texto = ler_arquivo(&arquivo)?;
    let origem = carteira::endereco(&texto)?;
    if origem == para {
        return Err("origem e destino são o mesmo endereço".into());
    }

    let rede = subir_rede(&o)?;
    println!("Sincronizando com a rede…");
    esperar_sincronizar(&rede, &o);
    salvar(&rede, &o)?;
    // Os pares mandam o mempool logo depois do aperto de mão. Esperar um pouco
    // evita escolher um nonce que já está numa transação pendente minha.
    if rede.pares_conectados() > 0 {
        std::thread::sleep(Duration::from_millis(1500));
    }
    let (nonce, saldo, altura) = {
        let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
        (no.proximo_nonce(&origem), no.chain.state.balance(&origem, &AUR), no.chain.height())
    };
    let total = valor.checked_add(o.taxa).ok_or("valor mais taxa estoura")?;
    if u128::from(total) > u128::from(saldo) {
        return Err(format!(
            "saldo gastável insuficiente na altura {altura}: {} AUR, precisa de {} AUR (a recompensa de mineração só libera depois de {} blocos)",
            aur(u128::from(saldo)),
            aur(u128::from(total)),
            o.rede.coinbase_maturity
        ));
    }

    let senha = if carteira::e_formato_antigo(&texto) { String::new() } else { senha::ler("Senha da carteira")? };
    let segredo = carteira::abrir(&texto, &senha)?;
    let saida = Output { recipient: para, asset_id: AUR, amount: valor };
    let tx = sign_transfer_outputs(&segredo, &o.rede.magic, origem, vec![saida], o.taxa, nonce).map_err(|e| e.to_string())?;
    let id = tx.txid().map_err(|e| e.to_string())?;
    match rede.submeter_tx(tx) {
        Ok(true) => {}
        Ok(false) => return Err("já existe uma transação com esse nonce esperando no mempool".into()),
        Err(e) => return Err(format!("a própria validação recusou: {}", e.0)),
    }
    println!("Transação {} assinada: {} AUR para {} (taxa {} AUR, nonce {nonce}).", hex(&id), aur(u128::from(valor)), hex(&para), aur(u128::from(o.taxa)));
    if rede.pares_conectados() == 0 {
        println!("Aviso: nenhum par conectado. A transação só existe neste nó; use --semente para mandar à rede.");
        return Ok(());
    }
    std::thread::sleep(Duration::from_secs(3));
    println!("Enviada para {} par(es). Ela entra num bloco quando algum minerador a incluir.", rede.pares_conectados());
    Ok(())
}

/// Só roda o nó: escuta, sincroniza, serve e propaga. Sem minerar.
fn servir_no(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let rede = subir_rede(&o)?;
    println!("Nó no ar. Ctrl+C para parar. A cadeia é gravada a cada mudança.");
    let mut ultima = (u64::MAX, usize::MAX);
    let mut ultimo_export = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(2));
        let (altura, pares, mempool) = {
            let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
            (no.chain.height(), rede.pares_conectados(), no.mempool_len())
        };
        let mudou = (altura, pares) != ultima;
        if altura != ultima.0 {
            salvar(&rede, &o)?;
            println!("  altura {altura} · {pares} par(es) · {mempool} no mempool");
        }
        if let Some(arquivo) = &o.exportar
            && (mudou || ultimo_export.elapsed() > Duration::from_secs(60))
        {
            if let Err(e) = exportar_estado(&rede, &o, arquivo) {
                println!("  aviso: não consegui exportar {}: {e}", arquivo.display());
            }
            ultimo_export = Instant::now();
        }
        ultima = (altura, pares);
    }
}

/// Resumo público da cadeia em JSON, para um explorador de blocos estático.
/// Só dados que já são públicos: altura, hashes e contagens. Gravado num
/// arquivo temporário e renomeado, para quem lê nunca pegar pela metade.
fn exportar_estado(rede: &Arc<Rede>, o: &Opcoes, arquivo: &Path) -> Result<(), String> {
    use std::fmt::Write as _;
    let no = rede.no.lock().map_err(|_| "nó travado".to_string())?;
    let c = &no.chain;
    let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut j = String::new();
    let _ = write!(
        j,
        "{{\"formato\":\"auron-explorador-v1\",\"rede\":\"{}\",\"altura\":{},\"ponta\":\"{}\",\"trabalho\":\"{}\",\"emitido\":\"{}\",\"pares\":{},\"mempool\":{},\"atualizado\":{},\"blocos\":[",
        o.rede.nome,
        c.height(),
        hex(&c.tip_hash()),
        c.total_work().to_decimal(),
        aur(u128::from(c.state.total_emitted)),
        rede.pares_conectados(),
        no.mempool_len(),
        agora
    );
    for (i, e) in c.entries.iter().rev().take(20).enumerate() {
        let h = &e.block.header;
        let n = e.block.useful_proof.as_ref().map_or(0, |p| p.n);
        let _ = write!(
            j,
            "{}{{\"altura\":{},\"hash\":\"{}\",\"anterior\":\"{}\",\"horario\":{},\"bits\":\"{:#010x}\",\"transacoes\":{},\"trabalho_util_n\":{}}}",
            if i > 0 { "," } else { "" },
            h.height,
            hex(&e.block.block_hash()),
            hex(&h.prev_hash),
            h.timestamp,
            h.bits,
            e.block.transactions.len(),
            n
        );
    }
    j.push_str("]}
");
    drop(no);
    let temporario = arquivo.with_extension("tmp");
    std::fs::write(&temporario, j).map_err(|e| e.to_string())?;
    std::fs::rename(&temporario, arquivo).map_err(|e| e.to_string())
}

fn estado(args: &[String]) -> Result<(), String> {
    let o = ler_opcoes(args)?;
    let cadeia = abrir_cadeia(&o)?;
    println!("Rede:              {}", o.rede.nome);
    println!("Altura:            {}", cadeia.height());
    println!("Ponta:             {}", hex(&cadeia.tip_hash()));
    println!("Trabalho total:    {}", cadeia.total_work().to_decimal());
    println!("Emitido:           {} AUR", aur(u128::from(cadeia.state.total_emitted)));
    if let Some(endereco) = o.endereco {
        println!("Saldo gastável:    {} AUR", aur(u128::from(cadeia.state.balance(&endereco, &AUR))));
        println!("Esperando liberar: {} AUR", aur(cadeia.state.immature_balance(&endereco)));
    }
    Ok(())
}

fn principal(args: &[String]) -> Result<(), String> {
    let (comando, resto) = args.split_first().ok_or(AJUDA)?;
    match comando.as_str() {
        "carteira" => comando_carteira(resto),
        "enviar" => enviar(resto),
        "minerar" => minerar(resto),
        "no" => servir_no(resto),
        "estado" => estado(resto),
        "ajuda" | "--ajuda" | "-h" => {
            print!("{AJUDA}");
            Ok(())
        }
        outro => Err(format!("comando desconhecido: {outro}\n\n{AJUDA}")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match principal(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}
