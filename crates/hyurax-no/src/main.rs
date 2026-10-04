// ✝ Provérbios 13:11 — “A riqueza de procedência vã diminuirá, mas quem a ajunta com o próprio trabalho a aumentará.”
//! `hyurax-no`: o Hyurax / Ultrax pelo terminal (servidor, celular, testes).
//!
//! Usa o mesmo núcleo do programa com janela (`hyurax-nucleo`); aqui só se
//! lê a linha de comando e se imprime o resultado.

mod cadeia;
mod carteira;
mod ciencia;
mod contas;
mod copia;
mod pacote;
mod identidade;
mod opcoes;
mod lancamento;
mod painel;
mod planos;
mod ultrax;

use std::process::ExitCode;

const AJUDA: &str = "\
hyurax-no — Hyurax / Ultrax pelo terminal (rede de TESTE)

Pastas: sem --pasta, usa as mesmas do programa com janela (no Windows,
%APPDATA%\\Hyurax para configuração e %LOCALAPPDATA%\\Hyurax para dados).
Com --pasta P, tudo fica em P. Redes: testnet (padrão) e regtest (local).

  hyurax-no carteira nova|ver|cifrar --arquivo carteira.txt
      nova: cria uma chave nova, cifrada com senha (recusa sobrescrever).
      ver: mostra o endereço (não pede senha).
      cifrar: converte uma carteira com o segredo em texto para o formato com senha.

  hyurax-no enviar --arquivo carteira.txt --para ENDERECO --valor HYX [--taxa HYX] [--pacote ARQ]
      Assina uma transferência (pede a senha) e manda para a rede.
      --pacote também grava a transação num pacote do Éter, para levar sem
      internet (pendrive, Bluetooth, som) até um nó conectado.

  hyurax-no minerar --endereco ENDERECO [--blocos N] [--linhas L] [--pausa-ms X]
      Minera N blocos (padrão 1; 0 = sem parar).

  hyurax-no painel [--arquivo carteira.txt | --endereco ENDERECO]
                   [--painel-porta 8800] [--painel-rede] [--api-externa] [--carteiras]
      O núcleo inteiro com a interface em http://127.0.0.1:8800 no navegador.
      --painel-rede deixa o celular no mesmo Wi-Fi ver (sem poder mandar).
      --api-externa liga /api/v1/externa/ para as contas de cliente.
      --carteiras liga /api/v1/leve/ para o aplicativo do celular.

  hyurax-no contas criar --nome NOME [--creditos MILICREDITOS]
  hyurax-no contas listar | revogar --id N | limite --id N --creditos M
      Contas da API externa. A chave de acesso aparece uma vez, na criação;
      em disco fica só o hash. Créditos são contabilidade, não dinheiro.

  hyurax-no no [--exportar resumo.json]
      Só o nó: escuta, sincroniza, serve e propaga. Sem minerar.
      --exportar grava um resumo público da cadeia (explorador de blocos).
      --ws-porta P também escuta nós por WebSocket (rota /p2p; saúde em /saude),
      para semente atrás de HTTPS, como no Render. Sementes podem ser
      wss://nome/p2p além de IP:PORTA.

  hyurax-no estado [--endereco ENDERECO]
      Altura da cadeia e, com --endereco, o saldo.

  hyurax-no ultrax lab [--tarefas N] [--linhas L] [--uso-cpu P] [--memoria-mib M] [--debug]
  hyurax-no ultrax auditar [--amostra N]
      ULTRAX no modo LAB (carga de teste gerada nesta máquina) e a auditoria
      do histórico (refaz TASK_ID, assinaturas e uma amostra do zero).

  hyurax-no ciencia rodar|listar|relatorio|refazer|benchmark|no ...
      Computação científica: JOBs divididos em unidades conferidas, com
      relatório em JSON, CSV e PDF. `hyurax-no ciencia` mostra as opções.

  hyurax-no lancamento chave --saida ARQUIVO
  hyurax-no lancamento assinar --chave ARQUIVO --instalador EXE --versao X [--notas TEXTO] [--saida atualizacao.txt]
  hyurax-no lancamento conferir --arquivo atualizacao.txt [--instalador EXE]
      Para quem publica: a chave de lançamento (nasce fora do repositório) e
      o manifesto assinado que a atualização segura do programa confere.

  hyurax-no planos catalogo | chave --saida ARQUIVO
  hyurax-no planos emitir --chave ARQUIVO --plano pro|equipe|empresa --para WORKER --serie N [--dias 30]
  hyurax-no planos conferir --voucher hyurax-plano:...
      Para quem vende os planos (docs/MONETIZACAO.md): a chave de planos
      (nasce fora do repositório) e o voucher que o cliente cola na tela Planos.

  hyurax-no identidade ver|girar
      ver: a identidade deste nó na rede e o WORKER_ID do ULTRAX.
      girar: troca a identidade (com o programa fechado). A antiga fica
      guardada ao lado; os outros nós passam a ver uma identidade nova.

  hyurax-no pacote exportar --saida ARQ [--blocos N]
  hyurax-no pacote importar --arquivo ARQ
      Pacote do Éter: blocos e transações num arquivo. Importar confere tudo
      como se viesse de um par (com o programa fechado). Com o programa
      aberto, basta pôr o arquivo na pasta eter/entrada dos dados.

  hyurax-no copia criar --destino PASTA
  hyurax-no copia conferir --origem COPIA | restaurar --origem COPIA
      Cópia de segurança das pastas (com manifesto SHA-512). Restaurar só
      com o programa fechado, e nunca apaga: o que existia fica ao lado.

Opções de rede (comandos que sobem o nó):
  --rede testnet|regtest   --porta P   --semente IP:PORTA[,…]   --sem-sementes-padrao
  --sem-malha   sem anúncio na rede local e sem abrir a porta no roteador (UPnP)

Senha em script: variável HYURAX_SENHA (conveniente, e menos segura que digitar).
A rede pública ainda é de teste: o HYX não tem valor.
";

fn principal(args: &[String]) -> Result<(), String> {
    let (comando, resto) = args.split_first().ok_or(AJUDA)?;
    match comando.as_str() {
        "carteira" => carteira::comando(resto),
        "enviar" => cadeia::enviar(resto),
        "minerar" => cadeia::minerar(resto),
        "painel" => painel::comando(resto),
        "no" => cadeia::servir(resto),
        "estado" => cadeia::estado(resto),
        "ultrax" => ultrax::comando(resto),
        "ciencia" => ciencia::comando(resto),
        "lancamento" => lancamento::comando(resto),
        "planos" => planos::comando(resto),
        "identidade" => identidade::comando(resto),
        "contas" => contas::comando(resto),
        "copia" => copia::comando(resto),
        "pacote" => pacote::comando(resto),
        "versao" | "--versao" | "-V" => {
            println!("{} {} · rede de teste", hyurax_nucleo::PRODUTO, hyurax_nucleo::VERSAO);
            Ok(())
        }
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
