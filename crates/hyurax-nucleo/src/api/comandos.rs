//! Os comandos da API v1 (`POST`). Só chegam aqui pedidos deste computador,
//! com `Host` e `Origin` locais, e com o programa destrancado.

use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde_json::json;

use super::http::{Pedido, campo, responder, responder_json};
use crate::carteira::endereco;
use crate::servico::{Modo, Nucleo};
use crate::util::{hex, hyx};

/// Perfis prontos da mineração: (linhas, limite de CPU). Saem dos núcleos
/// que a máquina tem, então o mesmo perfil vale num Atom e num PC de jogo.
fn perfil(nucleos: u32, nome: &str) -> Option<(u32, u32)> {
    let n = nucleos.max(1);
    match nome {
        "leve" => Some(((n / 4).max(1), 35)),
        "equilibrado" => Some(((n / 2).max(1), 70)),
        "turbo" => Some((n, 100)),
        _ => None,
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn atender(s: &mut TcpStream, p: &Pedido, n: &Arc<Nucleo>) -> std::io::Result<()> {
    let c = p.campos();
    let texto = |nome: &str| campo(&c, nome).unwrap_or_default();
    let numero = |nome: &str| campo(&c, nome).and_then(|v| v.parse::<u32>().ok());
    let sim = |nome: &str| campo(&c, nome).map(|v| v == "1");
    let janela = n.modo == Modo::Janela;
    let ok = || Ok(json!({ "ok": true }));
    let rede = n.config.rede.nome;
    let r = match p.rota() {
        "/api/v1/mineracao" => {
            if let Some((linhas, limite)) = campo(&c, "perfil").and_then(|x| perfil(n.nucleos, &x)) {
                n.mineracao.ajustar(Some(linhas), Some(limite));
                n.barramento.registrar("mineracao", format!("perfil {}: {linhas} linha(s), limite de {limite}% da CPU por linha", texto("perfil")));
            }
            if numero("linhas").is_some() || numero("limite_cpu").is_some() {
                n.mineracao.ajustar(numero("linhas"), numero("limite_cpu"));
            }
            if let Some(ligar) = sim("ligar") {
                n.mineracao.ligada.store(ligar && n.carteira.endereco().is_some(), Ordering::Relaxed);
            }
            n.gravar_ajustes();
            ok()
        }
        "/api/v1/planos/ativar" => n.planos.ativar(&texto("voucher"), &n.ultrax.worker(), crate::util::agora_ms()).map(|v| {
            let nome = hyurax_nuvem::plano::do_catalogo(v.plano).map_or("?", |p| p.nome);
            n.barramento.registrar("planos", format!("plano {nome} ativado (voucher {}), até {}", v.serie, crate::util::data_curta(v.fim_ms)));
            json!({ "plano": v.plano, "fim_ms": v.fim_ms })
        }),
        "/api/v1/ultrax" => {
            n.ultrax.ajustar_tudo(numero("linhas"), numero("limite_cpu"), numero("memoria_mib"), sim("debug"), sim("gpu"), numero("gpu_limite"));
            if let Some(ligar) = sim("ligar") {
                n.ultrax.ligar(ligar);
            }
            n.gravar_ajustes();
            ok()
        }
        "/api/v1/carteira/nova" if janela => n.carteira.criar(&texto("senha"), &texto("senha2")).map(|e| {
            n.barramento.registrar("carteira", format!("carteira criada: {}", endereco::mostrar(&e, rede)));
            json!({ "endereco": endereco::mostrar(&e, rede) })
        }),
        "/api/v1/carteira/importar" if janela => n.carteira.importar(&texto("conteudo"), &texto("senha"), &texto("senha2")).map(|(e, antiga)| {
            n.barramento.registrar("carteira", format!("carteira importada: {}", endereco::mostrar(&e, rede)));
            if antiga {
                n.barramento.registrar("carteira", "a carteira veio sem senha (formato antigo) e foi cifrada com a senha nova");
            }
            json!({ "endereco": endereco::mostrar(&e, rede), "cifrada_agora": antiga })
        }),
        "/api/v1/carteira/cifrar" if janela => n.carteira.cifrar(&texto("senha"), &texto("senha2")).map(|()| {
            n.barramento.registrar("carteira", "carteira protegida com senha: o segredo não fica mais em texto no disco");
            json!({ "ok": true })
        }),
        "/api/v1/carteira/conferir" => n
            .carteira
            .conferir(&n.config, &texto("para"), &texto("valor"), &texto("taxa"))
            .map(|pedido| json!({ "conferido": true, "para": endereco::mostrar(&pedido.para, rede), "valor": hyx(u128::from(pedido.valor)), "taxa": hyx(u128::from(pedido.taxa)) })),
        "/api/v1/carteira/enviar" => n
            .carteira
            .conferir(&n.config, &texto("para"), &texto("valor"), &texto("taxa"))
            .and_then(|pedido| n.carteira.enviar(&n.rede, &n.config, &pedido, &texto("senha"), &texto("codigo")))
            .inspect(|v| {
                n.barramento.registrar(
                    "carteira",
                    format!("enviados {} HYX para {} (taxa {}, nonce {})", v["valor"].as_str().unwrap_or(""), v["para"].as_str().unwrap_or(""), v["taxa"].as_str().unwrap_or(""), v["nonce"]),
                );
            }),
        "/api/v1/seguranca/comecar" => n.carteira.seguranca_comecar(),
        "/api/v1/seguranca/confirmar" => n.carteira.seguranca_confirmar(&texto("codigo"), sim("trava") == Some(true)).map(|()| {
            n.barramento.registrar("seguranca", "segundo fator ligado: enviar HYX agora pede o código de 6 dígitos");
            json!({ "ok": true })
        }),
        "/api/v1/seguranca/mudar" => n
            .carteira
            .seguranca_mudar(&texto("codigo"), sim("desligar") == Some(true), sim("exige_envio").unwrap_or(true), sim("trava") == Some(true))
            .map(|desligou| {
                n.barramento.registrar("seguranca", if desligou { "segundo fator desligado" } else { "segundo fator ajustado" });
                json!({ "ok": true })
            }),
        "/api/v1/destravar" => n.carteira.destravar(&texto("codigo")).map(|()| {
            n.barramento.registrar("seguranca", "programa destrancado");
            json!({ "ok": true })
        }),
        "/api/v1/ciencia/rede" => {
            let aceitar = sim("aceitar") == Some(true);
            n.ciencia.aceitar_da_rede(aceitar);
            n.gravar_ajustes();
            n.barramento.registrar(
                "ciencia",
                if aceitar { "este nó passa a calcular unidades que outros nós pedirem (dentro dos limites do ULTRAX)" } else { "este nó não aceita mais trabalho de outros nós" },
            );
            Ok(json!({ "aceitar": aceitar }))
        }
        "/api/v1/ciencia/benchmark" => {
            let ja = n.ciencia.benchmark.lock().map(|mut b| std::mem::replace(&mut b.0, true)).unwrap_or(true);
            if !ja {
                let (ciencia, nucleos) = (Arc::clone(&n.ciencia), n.nucleos);
                n.barramento.registrar("ciencia", "ULTRA BENCHMARK começou: mede agendador, motores, escala e um JOB de ponta a ponta");
                std::thread::spawn(move || {
                    let r = crate::ciencia::bancada::rodar(&ciencia, nucleos, &mut |_| {});
                    if let Ok(mut b) = ciencia.benchmark.lock() {
                        *b = (false, r.unwrap_or_else(|e| json!({ "erro": e }).to_string()));
                    }
                });
            }
            Ok(json!({ "rodando": true, "ja_estava": ja }))
        }
        "/api/v1/ciencia/estimar" => crate::ciencia::pedido_do_formulario(&c)
            .and_then(|pedido| n.ciencia.estimar(pedido))
            .and_then(|j| serde_json::from_str(&j).map_err(|e| e.to_string())),
        "/api/v1/ciencia/submeter" => crate::ciencia::pedido_do_formulario(&c).and_then(|pedido| n.ciencia.submeter(pedido)).map(|id| {
            n.barramento.registrar("ciencia", format!("JOB submetido: {}", hex(&id)));
            json!({ "id": hex(&id) })
        }),
        r if r.starts_with("/api/v1/ciencia/job/") => match r.trim_start_matches("/api/v1/ciencia/job/").split_once('/') {
            Some((id_texto, acao)) => crate::ciencia::id_de_hex(id_texto)
                .ok_or_else(|| "JOB_ID inválido".to_string())
                .and_then(|id| n.ciencia.mudar(&id, acao))
                .map(|()| json!({ "ok": true })),
            None => Err("falta a ação: pausar, retomar ou cancelar".into()),
        },
        "/api/v1/ajustes" if janela => {
            if let Ok(mut a) = n.ajustes.lock() {
                if let Some(w) = numero("watts_nucleo") {
                    a.watts_nucleo = w.clamp(1, 200);
                }
                if let Some(k) = numero("centavos_kwh") {
                    a.centavos_kwh = k.clamp(1, 99_999);
                }
                if let Some(x) = sim("avisar_bloco") {
                    a.avisar_bloco = x;
                }
                if let Some(x) = sim("som_bloco") {
                    a.som_bloco = x;
                }
            }
            if let Some(na_rede) = sim("na_rede") {
                n.na_rede.store(na_rede, Ordering::Relaxed);
                n.barramento.registrar("painel", if na_rede { "painel visível na rede local (só leitura)" } else { "painel só neste computador" });
            }
            if let Some(ligar) = sim("api_externa") {
                n.api_externa.store(ligar, Ordering::Relaxed);
                n.barramento.registrar("painel", if ligar { "API externa ligada: contas de cliente mandam JOBs pela rede" } else { "API externa desligada" });
            }
            if let Some(ligar) = sim("carteiras_leves") {
                n.carteiras_leves.store(ligar, Ordering::Relaxed);
                n.barramento.registrar(
                    "painel",
                    if ligar { "carteiras de celular ligadas: o app pode ler saldo e mandar transações já assinadas por este nó" } else { "carteiras de celular desligadas" },
                );
            }
            n.gravar_ajustes();
            ok()
        }
        "/api/v1/contas/criar" => {
            let limite = numero("limite_milicreditos").map_or(0, u64::from);
            n.contas.criar(&texto("nome"), limite, crate::util::agora_unix()).map(|(c, chave)| {
                n.barramento.registrar("painel", format!("conta {} ({}) criada para a API externa", c.id, c.nome));
                json!({ "id": c.id, "nome": c.nome, "chave": chave, "aviso": "guarde a chave agora: ela não aparece de novo" })
            })
        }
        "/api/v1/contas/revogar" => numero("id").ok_or_else(|| "falta o id".to_string()).and_then(|id| n.contas.revogar(id)).map(|()| {
            n.barramento.registrar("painel", "conta da API externa revogada");
            json!({ "ok": true })
        }),
        "/api/v1/sementes" if janela => n.trocar_sementes(&texto("lista")).map(|l| json!({ "sementes": l })),
        "/api/v1/maquinas" => n.maquinas.trocar(&texto("lista")).map(|l| {
            n.gravar_ajustes();
            json!({ "maquinas": l })
        }),
        "/api/v1/termos/aceitar" if janela => crate::termos::aceitar(&n.config.pastas.config, "programa").map(|()| json!({ "ok": true })),
        "/api/v1/atualizacao/buscar" if janela => {
            let n = Arc::clone(n);
            std::thread::spawn(move || n.buscar_atualizacao());
            ok()
        }
        "/api/v1/atualizacao/instalar" if janela => n.instalar_atualizacao().map(|()| json!({ "ok": true })),
        "/api/v1/abrir-pasta" if janela => abrir_pasta(&n.config.pastas.dados).map(|()| json!({ "ok": true })),
        // o canal do backend da GPU (o worker WebGL 2 da janela)
        "/api/v1/nuvem/ajustes" => {
            let mut a = n.nuvem.ajustes.lock().map(|a| a.clone()).unwrap_or_default();
            let n64 = |nome: &str| campo(&c, nome).and_then(|v| v.parse::<u64>().ok());
            let pct = |nome: &str| campo(&c, nome).and_then(|v| v.parse::<u8>().ok()).filter(|x| *x <= 100);
            if let Some(v) = sim("anunciar") {
                a.anunciar = v;
            }
            if let Some(t) = campo(&c, "tipo").and_then(|v| match v.as_str() {
                "capacidade" => Some(hyurax_nuvem::anuncio::TipoDeAnuncio::Capacidade),
                "maquina_inteira" => Some(hyurax_nuvem::anuncio::TipoDeAnuncio::MaquinaInteira),
                "venda" => Some(hyurax_nuvem::anuncio::TipoDeAnuncio::Venda),
                _ => None,
            }) {
                a.tipo = t;
            }
            a.preco_credito_mili = n64("preco_credito_mili").unwrap_or(a.preco_credito_mili);
            a.preco_gb_mes_mili = n64("preco_gb_mes_mili").unwrap_or(a.preco_gb_mes_mili);
            a.preco_venda_centavos = n64("preco_venda_centavos").unwrap_or(a.preco_venda_centavos);
            if let Some(d) = campo(&c, "descricao") {
                a.descricao = d.chars().take(200).collect();
            }
            a.disco_mib = n64("disco_mib").filter(|x| *x <= 10_000_000).unwrap_or(a.disco_mib);
            a.provedor_pct = pct("provedor_pct").unwrap_or(a.provedor_pct);
            a.plataforma_pct = pct("plataforma_pct").unwrap_or(a.plataforma_pct);
            n.nuvem.mudar_ajustes(a).map(|()| {
                n.barramento.registrar("nuvem", "ajustes da nuvem gravados (anúncio refeito)");
                json!({ "ok": true })
            })
        }
        "/api/v1/nuvem/guardar" => {
            let nome = p.parametro("nome").and_then(super::http::decodificar).unwrap_or_else(|| "arquivo".into());
            let k = p.parametro("k").and_then(|v| v.parse::<u8>().ok()).unwrap_or(2);
            let m = p.parametro("m").and_then(|v| v.parse::<u8>().ok()).unwrap_or(1);
            n.nuvem.guardar_arquivo(&nome, &p.corpo, k, m).map(|id| json!({ "arquivo": hex(&id) }))
        }
        "/api/v1/nuvem/recuperar" => crate::nuvem::arquivo(&texto("arquivo")).ok_or_else(|| "arquivo inválido".to_string()).and_then(|a| n.nuvem.recuperar(&a)).map(|()| json!({ "ok": true })),
        "/api/v1/nuvem/apagar" => crate::nuvem::arquivo(&texto("arquivo")).ok_or_else(|| "arquivo inválido".to_string()).and_then(|a| n.nuvem.apagar_arquivo(&a)).map(|()| json!({ "ok": true })),
        "/api/v1/nuvem/alugar" => crate::util::de_hex::<32>(&texto("worker"))
            .ok_or_else(|| "worker inválido".to_string())
            .and_then(|w| n.nuvem.anuncio_para_alugar(&w))
            .and_then(|anuncio| {
                let mut pedido = crate::ciencia::pedido_do_formulario(&c)?;
                // aluguel: cada unidade calculada lá e refeita aqui (concordância 2 de 2)
                pedido.nivel = hyurax_ultrax::job::Nivel::Concordancia;
                pedido.redundancia = 2;
                let id = n.ciencia.submeter(pedido)?;
                n.ciencia.restringir_a(id, anuncio.worker);
                n.nuvem.registrar_aluguel(id, &anuncio);
                n.barramento.registrar("nuvem", format!("aluguel: JOB {}… só para o worker {}…, a {} milicréditos por crédito verificado", hex(id.get(..6).unwrap_or_default()), hex(anuncio.worker.get(..6).unwrap_or_default()), anuncio.preco_credito_mili));
                Ok(json!({ "job": hex(&id) }))
            }),
        "/api/v1/gpu/pegar" => n.ultrax.gpu_pegar(&texto("nome")).map(|(numero, lado, job)| {
            json!({ "numero": numero, "n": lado, "origem": if job.is_some() { "JOB" } else { "LAB" }, "job": job.map(|(j, _)| hex(&j)), "unidade": job.map(|(_, i)| i) })
        }),
        r if r.starts_with("/api/v1/gpu/progresso/") => {
            if let (Ok(numero), Some(linhas)) =
                (r.trim_start_matches("/api/v1/gpu/progresso/").parse::<u32>(), campo(&c, "linhas").and_then(|v| v.parse::<u64>().ok()))
            {
                n.ultrax.gpu_progresso(numero, linhas);
            }
            return responder(s, "204 No Content", "text/plain", b"");
        }
        r if r.starts_with("/api/v1/gpu/cancelar/") => {
            if let Ok(numero) = r.trim_start_matches("/api/v1/gpu/cancelar/").parse::<u32>() {
                let motivo: String = campo(&c, "motivo").unwrap_or_else(|| "a janela desistiu da tarefa".into()).chars().filter(|c| !c.is_control()).take(160).collect();
                n.ultrax.gpu_cancelar(numero, &motivo);
            }
            return responder(s, "204 No Content", "text/plain", b"");
        }
        r if r.starts_with("/api/v1/gpu/resultado/") => r
            .trim_start_matches("/api/v1/gpu/resultado/")
            .parse::<u32>()
            .map_err(|_| "número de tarefa inválido".to_string())
            .and_then(|numero| n.ultrax.gpu_resultado(numero, &p.corpo))
            .map(|()| json!({ "ok": true })),
        _ => return responder(s, "404 Not Found", "text/plain", b"nao existe"),
    };
    // código de 6 dígitos errado fica no registro (sem o código): quem tenta
    // os 10^6 por um script aparece no hyurax.log
    if let Err(e) = &r
        && matches!(p.rota(), "/api/v1/destravar" | "/api/v1/seguranca/mudar" | "/api/v1/carteira/enviar")
        && (e.contains("código") || e.contains("espere"))
    {
        n.barramento.registrar("seguranca", format!("código de 6 dígitos recusado em {}: {e}", p.rota()));
    }
    responder_json(s, r)
}

fn abrir_pasta(pasta: &std::path::Path) -> Result<(), String> {
    let programa = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(programa).arg(pasta).spawn().map(|_| ()).map_err(|e| format!("não consegui abrir {}: {e}", pasta.display()))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn perfis_saem_dos_nucleos_da_maquina() {
        assert_eq!(perfil(4, "leve"), Some((1, 35)));
        assert_eq!(perfil(4, "equilibrado"), Some((2, 70)));
        assert_eq!(perfil(4, "turbo"), Some((4, 100)));
        assert_eq!(perfil(1, "equilibrado"), Some((1, 70)));
        assert_eq!(perfil(4, "outro"), None);
    }
}
