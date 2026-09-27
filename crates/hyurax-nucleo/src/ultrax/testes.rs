//! Testes do worker.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::*;

fn pasta(nome: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("hyurax-ultrax-teste-{nome}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn worker(nome: &str, partida: &Partida) -> (Arc<Ultrax>, PathBuf) {
    let p = pasta(nome);
    let u = Ultrax::abrir(&p, &[9; 32], 2, partida, Box::new(|_, _| {}));
    // ritmo baixo: o gerador faz tarefas pequenas, que o modo debug roda rápido
    *u.ritmo.lock().unwrap() = [1.0e5; 8];
    (u, p)
}

fn partida() -> Partida {
    Partida { ligado: true, linhas: 1, uso_cpu: 100, memoria_mib: MEMORIA_PADRAO_MIB, debug: false, gpu: false, gpu_uso: 50 }
}

#[test]
fn desafios_batem_com_os_vetores() {
    let ler = |nome: &str| {
        let caminho: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "vectors", nome].iter().collect();
        std::fs::read_to_string(caminho).unwrap()
    };
    let texto = ler("ultrax.json") + &ler("ia.json");
    for d in &DESAFIOS {
        let esp = Especificacao::nova(d.tipo, d.tamanho, d.passos).unwrap();
        let exec = trabalho::executar(&esp, &semente_do_desafio(&esp), &mut |_| true).unwrap();
        // o resultado calculado aqui está, em hexadecimal, dentro do arquivo do gabarito
        assert!(texto.contains(&hex(&exec.resultado)), "{} fora dos vetores", esp.resumo());
        assert_eq!(hex(&hash_do_resultado(&exec.resultado)), d.esperado, "{}", esp.resumo());
    }
}

#[test]
fn uma_tarefa_passa_pelo_ciclo_inteiro() {
    let (u, p) = worker("ciclo", &partida());
    let item = u.gerar(agora_ms()).unwrap();
    let numero = item.tarefa.numero();
    u.processar(0, item);
    let placar = u.placar();
    assert_eq!(placar.liquidadas, 1);
    assert_eq!(placar.reputacao.verificadas, 1);
    assert!(placar.score.operacoes_verificadas > 0);
    let eventos: Vec<&str> = u.telemetria.lock().unwrap().iter().rev().filter(|m| m.tarefa == numero).map(|m| m.evento).collect();
    assert_eq!(
        eventos,
        [
            "TASK CREATED",
            "TASK QUEUED",
            "TASK ASSIGNED",
            "RESOURCE ALLOCATED",
            "WORK STARTED",
            "WORK COMPLETED",
            "RESULT SUBMITTED",
            "VERIFICATION STARTED",
            "VERIFICATION PASSED",
            "SETTLEMENT COMPLETED"
        ]
    );
    // o histórico foi gravado e a auditoria confere id, assinatura e refaz a conta
    let a = auditar(&p, &p, 10).unwrap();
    assert_eq!((a.registros, a.ids_certos, a.assinaturas_certas, a.refeitas_iguais), (1, 1, 1, 1), "{a:?}");
    assert!(a.problemas.is_empty(), "{:?}", a.problemas);
    // e o placar sobrevive a reabrir
    let reaberto = Ultrax::abrir(&p, &[9; 32], 2, &partida(), Box::new(|_, _| {}));
    assert_eq!(reaberto.placar().liquidadas, 1);
    assert_eq!(reaberto.historico.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn a_decima_tarefa_e_desafio_e_passa() {
    let (u, p) = worker("desafio", &partida());
    u.sequencia.store(DESAFIO_A_CADA - 1, Ordering::Relaxed);
    let item = u.gerar(agora_ms()).unwrap();
    assert!(item.desafio.is_some());
    assert_eq!(item.tarefa.metodo, MetodoDeVerificacao::ResultadoEsperado);
    u.processar(0, item);
    assert_eq!(u.placar().desafios_certos, 1);
    assert!(auditar(&p, &p, 1).unwrap().problemas.is_empty());
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn auditoria_pega_historico_alterado() {
    let (u, p) = worker("adulterado", &partida());
    let item = u.gerar(agora_ms()).unwrap();
    u.processar(0, item);
    let arquivo = p.join("ultrax").join("historico.txt");
    let texto = std::fs::read_to_string(&arquivo).unwrap();
    // inflar as operações para ganhar Work Score
    let operacoes = Registro::ler(texto.lines().next().unwrap()).unwrap().operacoes;
    std::fs::write(&arquivo, texto.replace(&format!("operacoes={operacoes}"), &format!("operacoes={}", operacoes * 10))).unwrap();
    let a = auditar(&p, &p, 0).unwrap();
    assert_eq!(a.assinaturas_certas, 0);
    assert!(a.problemas.iter().any(|x| x.contains("assinatura")), "{:?}", a.problemas);
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn teto_de_memoria_cancela_o_que_nao_cabe() {
    let (u, p) = worker("memoria", &Partida { memoria_mib: MEMORIA_MIN_MIB, ..partida() });
    // uma matriz 1024 pede 48 MiB, acima do teto de 32
    let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 1024, 0).unwrap();
    let mut tarefa = Tarefa::nova(esp, MetodoDeVerificacao::Freivalds, Instancia::PorWorker, b"x", 0, agora_ms(), agora_ms() + 60_000).unwrap();
    tarefa.avancar(Estado::NaFila, agora_ms(), "").unwrap();
    u.processar(0, NaFila { tarefa, desafio: None, recusas: 0, origem: vec![0; 32], job: None });
    let placar = u.placar();
    assert_eq!((placar.canceladas, placar.liquidadas), (1, 0));
    assert_eq!(u.reservada.load(Ordering::Relaxed), 0, "nada fica reservado");
    // e o gerador dimensiona dentro do teto
    let esp = u.dimensionar(TipoDeTrabalho::Matriz).unwrap();
    assert!(esp.memoria_bytes() <= u64::from(MEMORIA_MIN_MIB) * 1024 * 1024);
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn desligar_cancela_sem_julgar() {
    let (u, p) = worker("desligar", &partida());
    let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 256, 0).unwrap();
    let mut tarefa = Tarefa::nova(esp, MetodoDeVerificacao::Freivalds, Instancia::PorWorker, b"y", 0, agora_ms(), agora_ms() + 60_000).unwrap();
    tarefa.avancar(Estado::NaFila, agora_ms(), "").unwrap();
    u.ligado.store(false, Ordering::Relaxed);
    u.processar(0, NaFila { tarefa, desafio: None, recusas: 0, origem: vec![0; 32], job: None });
    let placar = u.placar();
    assert_eq!((placar.canceladas, placar.recusadas, placar.reputacao.recusadas), (1, 0, 0), "parar não é errar");
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn limite_de_cpu_descansa_na_proporcao() {
    let (u, p) = worker("cpu", &Partida { uso_cpu: 50, ..partida() });
    let feitas = AtomicU64::new(0);
    let mut c = Controle::novo(&u, 0, agora_ms() + 60_000, &feitas);
    let comeco = Instant::now();
    for _ in 0..20 {
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(10) {
            std::hint::spin_loop();
        }
        assert!(c.passo(1));
    }
    let parede = comeco.elapsed().as_secs_f64();
    let calculo = c.trabalhando.as_secs_f64();
    // 50%: o relógio anda perto do dobro do cálculo (com folga para o agendador)
    assert!(parede > calculo * 1.6, "parede {parede:.3} s, cálculo {calculo:.3} s");
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn difusao_com_pouca_memoria_por_linha_nao_trava() {
    let p = pasta("difusao");
    let partida = Partida { memoria_mib: MEMORIA_MIN_MIB, linhas: 16, ..partida() };
    let u = Ultrax::abrir(&p, &[9; 32], 16, &partida, Box::new(|_, _| {}));
    // máquina rápida: os passos batem no máximo, e 2 MiB por linha não cabem a grade 256
    *u.ritmo.lock().unwrap() = [1.0e10; 8];
    let esp = u.dimensionar(TipoDeTrabalho::Difusao).unwrap();
    assert!(esp.memoria_bytes() <= u64::from(MEMORIA_MIN_MIB) * 1024 * 1024 / 16, "{}", esp.resumo());
    let _ = std::fs::remove_dir_all(p);
}

/// Uma pasta com uma tarefa liquidada e a `no.chave` do worker que a fez.
fn pasta_auditavel(nome: &str) -> (PathBuf, PathBuf) {
    let (u, p) = worker(nome, &partida());
    std::fs::write(p.join("no.chave"), format!("segredo={}
", hex(&[9u8; 32]))).unwrap();
    let item = u.gerar(agora_ms()).unwrap();
    u.processar(0, item);
    let historico = p.join("ultrax").join("historico.txt");
    let a = auditar(&p, &p, 1).unwrap();
    assert!(a.problemas.is_empty(), "a pasta honesta passa: {:?}", a.problemas);
    assert!(a.placar_conferido);
    assert_eq!((a.vereditos_certos, a.refeitas_iguais), (1, 1));
    (p, historico)
}

fn trocar_no_historico(historico: &Path, de: &str, para: &str) {
    let texto = std::fs::read_to_string(historico).unwrap();
    assert!(texto.contains(de), "{de} não está no histórico");
    std::fs::write(historico, texto.replacen(de, para, 1)).unwrap();
}

#[test]
fn auditoria_pega_estado_trocado() {
    let (p, historico) = pasta_auditavel("estado");
    trocar_no_historico(&historico, "estado=SETTLED", "estado=REJECTED");
    let a = auditar(&p, &p, 0).unwrap();
    assert!(a.problemas.iter().any(|x| x.contains("veredito")), "{:?}", a.problemas);
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn auditoria_pega_desafio_falso_e_linha_repetida() {
    let (p, historico) = pasta_auditavel("desafio-falso");
    let linha = std::fs::read_to_string(&historico).unwrap();
    std::fs::write(&historico, format!("{linha}{linha}")).unwrap();
    trocar_no_historico(&historico, "desafio=0", "desafio=1");
    let a = auditar(&p, &p, 0).unwrap();
    assert!(a.problemas.iter().any(|x| x.contains("não é um desafio do gabarito")), "{:?}", a.problemas);
    assert!(a.problemas.iter().any(|x| x.contains("mais de uma vez")), "{:?}", a.problemas);
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn auditoria_pega_outro_worker_e_placar_inflado() {
    let (p, _) = pasta_auditavel("worker");
    std::fs::write(p.join("no.chave"), format!("segredo={}
", hex(&[8u8; 32]))).unwrap();
    let placar = p.join("ultrax").join("placar.txt");
    let texto = std::fs::read_to_string(&placar).unwrap();
    std::fs::write(&placar, texto.replace("liquidadas=1", "liquidadas=500")).unwrap();
    let a = auditar(&p, &p, 0).unwrap();
    assert!(a.problemas.iter().any(|x| x.contains("outro worker")), "{:?}", a.problemas);
    assert!(a.problemas.iter().any(|x| x.contains("placar.txt")), "{:?}", a.problemas);
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn contagem_de_recusa_dupla() {
    let (u, p) = worker("recusa", &partida());
    // um desafio com a resposta esperada errada faz esta máquina "errar" sempre
    let esp = Especificacao::nova(TipoDeTrabalho::Matriz, 8, 0).unwrap();
    let mut tarefa = Tarefa::nova(esp, MetodoDeVerificacao::ResultadoEsperado, Instancia::Compartilhada, b"z", 0, agora_ms(), agora_ms() + 60_000).unwrap();
    tarefa.avancar(Estado::NaFila, agora_ms(), "").unwrap();
    // desafio de índice 0 é a matriz 16; esta tarefa é 8, então a resposta nunca bate
    u.processar(0, NaFila { tarefa, desafio: Some(0), recusas: 0, origem: vec![0; 32], job: None });
    let item = u.fila.lock().unwrap().pop_front().expect("volta para a fila uma vez");
    u.processar(0, item);
    let placar = u.placar();
    assert_eq!(
        (placar.recusadas, placar.abandonadas, placar.canceladas, placar.desafios_errados, placar.liquidadas),
        (2, 1, 0, 1, 0),
        "{placar:?}"
    );
    assert_eq!(u.finais.load(Ordering::Relaxed), 1, "uma tarefa, um final");
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn treino_de_ia_guarda_o_melhor_modelo() {
    let (u, p) = worker("ia", &partida());
    // a quarta tarefa normal é a de IA
    u.sequencia.store(3, Ordering::Relaxed);
    let item = u.gerar(agora_ms()).unwrap();
    assert_eq!(item.tarefa.especificacao.tipo(), TipoDeTrabalho::Ia);
    u.processar(0, item);
    assert_eq!(u.placar().por_tipo[3], 1);
    let json = u.json();
    assert!(json.contains("\"treinos\":1"), "{json}");
    assert!(json.contains("\"previsto_mili\":-") || json.contains("\"previsto_mili\":"), "{json}");
    // e o modelo sobrevive a reabrir
    let reaberto = Ultrax::abrir(&p, &[9; 32], 2, &partida(), Box::new(|_, _| {}));
    assert!(reaberto.modelo.lock().unwrap().melhor.is_some());
    assert!(auditar(&p, &p, 1).unwrap().problemas.is_empty());
    let _ = std::fs::remove_dir_all(p);
}

/// A GPU de mentira: faz a conta na CPU e manda como a página mandaria.
fn fazer_na_gpu(u: &Ultrax, adulterar: bool) -> u32 {
    let (numero, n) = u.gpu_pegar("GPU de teste").unwrap();
    let entrada = u.gpu_entrada(numero).unwrap();
    let v: Vec<u64> = entrada.as_chunks::<4>().0.iter().map(|c| u64::from(u32::from_le_bytes(*c))).collect();
    let n = n as usize;
    let (a, b) = v.split_at(n * n);
    let mut c = vec![0u32; n * n];
    for i in 0..n {
        for k in 0..n {
            for j in 0..n {
                c[i * n + j] += (a[i * n + k] * b[k * n + j]) as u32;
            }
        }
    }
    if adulterar {
        c[7] ^= 1;
    }
    u.gpu_progresso(numero, n as u64);
    let bytes: Vec<u8> = c.iter().flat_map(|x| x.to_le_bytes()).collect();
    u.gpu_resultado(numero, &bytes).unwrap();
    numero
}

#[test]
fn gpu_honesta_e_creditada_e_adulterada_e_recusada() {
    let (u, p) = worker("gpu", &Partida { gpu: true, ..partida() });
    *u.ritmo_gpu.lock().unwrap() = 1.0e5;
    fazer_na_gpu(&u, false);
    assert_eq!(u.placar().liquidadas, 1);
    assert_eq!(u.reservada.load(Ordering::Relaxed), 0);
    fazer_na_gpu(&u, true);
    let placar = u.placar();
    assert_eq!(placar.recusadas, 1, "a CPU pegou o erro da GPU");
    assert!(auditar(&p, &p, 1).unwrap().problemas.is_empty());
    // desligada, não entrega tarefa
    u.ajustar_tudo(None, None, None, None, Some(false), None);
    assert!(u.gpu_pegar("x").is_err());
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn gpu_esquecida_vence_e_libera_a_memoria() {
    let (u, p) = worker("gpu-vence", &Partida { gpu: true, ..partida() });
    *u.ritmo_gpu.lock().unwrap() = 1.0e5;
    let (numero, _) = u.gpu_pegar("x").unwrap();
    u.gpu_cancelar(numero, "a janela fechou");
    assert_eq!(u.placar().canceladas, 1);
    assert_eq!(u.reservada.load(Ordering::Relaxed), 0);
    assert!(u.gpu_resultado(numero, &[0; 16]).is_err(), "encerrada não aceita resultado");
    let _ = std::fs::remove_dir_all(p);
}

#[test]
fn escapar_ida_e_volta() {
    let t = "a b%c\nd\te";
    assert_eq!(desescapar(&escapar(t)), t);
    assert!(!escapar(t).contains(' '));
}
