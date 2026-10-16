//! Contas: cadastro, entrada, sessão, senha, recuperação, papéis e chaves de
//! API (seções 6, 7 e 30 do documento da V1).

use serde_json::{Value, json};

use super::{Erro, Plataforma, R};
use crate::auth;
use crate::dinheiro::Moeda;
use crate::eventos::Evento;
use crate::modelo::{Escopo, Id, Papeis, texto_curto};
use crate::util::hex;

/// Sessão vale no máximo isso.
pub const SESSAO_MAX_MS: u64 = 7 * 86_400_000;
/// Sessão parada por isso cai.
pub const SESSAO_OCIOSA_MS: u64 = 24 * 3_600_000;
/// Falhas de senha antes de travar o e-mail.
const FALHAS_POR_EMAIL: u32 = 5;
/// Falhas de senha antes de travar o IP.
const FALHAS_POR_IP: u32 = 20;
/// Tempo da trava.
const TRAVA_MS: u64 = 15 * 60_000;
/// Chaves de API ativas por usuário.
const CHAVES_MAX: usize = 10;

/// Quem fez o pedido, já autenticado.
#[derive(Clone, Debug)]
pub struct Quem {
    /// Usuário.
    pub usuario: Id,
    /// Papéis.
    pub papeis: Papeis,
    /// Hash do token de sessão, quando veio por sessão.
    pub sessao: Option<String>,
    /// Escopos da chave de API, quando veio por chave.
    pub escopos: Option<Vec<Escopo>>,
    /// Moeda da conta.
    pub moeda: Moeda,
}

impl Quem {
    /// Exige um escopo (sessão de navegador pode tudo do próprio usuário).
    pub fn pode(&self, escopo: Escopo) -> R<()> {
        match &self.escopos {
            Some(e) if !e.contains(&escopo) => Err(Erro::proibido(format!("a chave de API não tem o escopo {}", escopo.nome()))),
            _ => Ok(()),
        }
    }

    /// Exige o papel de cliente.
    pub fn cliente(&self) -> R<()> {
        if self.papeis.cliente { Ok(()) } else { Err(Erro::proibido("ative o papel de cliente na sua conta")) }
    }

    /// Exige o papel de fornecedor (e sessão: chave de API não mexe em nós).
    pub fn fornecedor(&self) -> R<()> {
        if self.escopos.is_some() {
            return Err(Erro::proibido("chave de API não administra computadores"));
        }
        if self.papeis.fornecedor { Ok(()) } else { Err(Erro::proibido("ative o papel de fornecedor na sua conta")) }
    }

    /// Exige o papel de administrador, por sessão.
    pub fn admin(&self) -> R<()> {
        if self.papeis.admin && self.escopos.is_none() { Ok(()) } else { Err(Erro::proibido("só o administrador")) }
    }
}

/// O que vem do formulário de cadastro.
#[derive(Clone, Debug, Default)]
pub struct Cadastro {
    /// E-mail.
    pub email: String,
    /// Nome.
    pub nome: String,
    /// Senha.
    pub senha: String,
    /// Moeda (`BRL` ou `USD`).
    pub moeda: String,
    /// País.
    pub pais: String,
    /// Quer comprar computação.
    pub cliente: bool,
    /// Quer fornecer computação.
    pub fornecedor: bool,
    /// Aceitou os termos.
    pub aceite: bool,
}

/// O que volta de uma entrada bem-sucedida.
#[derive(Clone, Debug)]
pub struct Entrada {
    /// Usuário.
    pub usuario: Id,
    /// Token de sessão (vai para o cookie).
    pub token: String,
    /// Códigos de recuperação (só no cadastro).
    pub codigos: Vec<String>,
}

impl Plataforma {
    /// Cria uma conta e já abre a sessão.
    pub fn cadastrar(&self, c: Cadastro, ip: &str, agente: &str) -> R<Entrada> {
        let email = auth::validar_email(&c.email).ok_or("e-mail inválido")?;
        let nome = texto_curto(&c.nome, 80);
        if nome.chars().count() < 2 {
            return Err(Erro::pedido("informe o seu nome"));
        }
        if !c.aceite {
            return Err(Erro::pedido("é preciso aceitar os termos de uso e a política de privacidade"));
        }
        auth::validar_senha(&c.senha, &email)?;
        let moeda = Moeda::de_codigo(&c.moeda).ok_or("moeda: use BRL ou USD")?;
        let pais = c.pais.trim().to_ascii_uppercase();
        if pais.len() != 2 || !pais.chars().all(|x| x.is_ascii_uppercase()) {
            return Err(Erro::pedido("país: use o código de duas letras (BR, US, PT...)"));
        }
        if !c.cliente && !c.fornecedor {
            return Err(Erro::pedido("escolha pelo menos um uso: comprar ou fornecer computação"));
        }
        let (aberto, por_ip) = self.ler(|e| (e.config.cadastro_aberto, e.config.politica.cadastros_por_ip_dia))?;
        if !aberto {
            return Err(Erro::proibido("o cadastro está fechado no momento"));
        }
        self.conferir_cadastros_do_ip(ip, por_ip)?;
        // a conta cara (Argon2id) sai antes da trava
        let senha = auth::hash_da_senha(&c.senha).map_err(|e| Erro::interno(&e))?;
        let (codigos, hashes) = auth::codigos_de_recuperacao().map_err(|e| Erro::interno(&e))?;
        let (token, hash_token) = auth::token_de_sessao().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        if n.estado.por_email.contains_key(&email) {
            return Err(Erro::conflito("já existe uma conta com esse e-mail"));
        }
        let t = self.agora();
        let id = n.estado.seq.usuario;
        let termos = n.estado.config.termos_versao;
        self.registrar(
            &mut n,
            vec![
                Evento::UserCreated {
                    id,
                    email,
                    nome,
                    papeis: Papeis { cliente: c.cliente, fornecedor: c.fornecedor, admin: false },
                    moeda,
                    pais,
                    senha,
                    recuperacao: hashes,
                    termos,
                    ip: ip.to_string(),
                    t,
                },
                Evento::Login {
                    sessao: hash_token.clone(),
                    usuario: id,
                    expira: t.saturating_add(SESSAO_MAX_MS),
                    ip: ip.to_string(),
                    agente: texto_curto(agente, 120),
                    t,
                },
            ],
        )?;
        drop(n);
        if let Ok(mut v) = self.vivo() {
            v.atividade.insert(hash_token, t);
            v.cadastros.entry(ip.to_string()).or_default().push(t);
        }
        Ok(Entrada { usuario: id, token, codigos })
    }

    /// Cria o primeiro administrador (pela linha de comando). Devolve a senha
    /// gerada, que aparece uma vez só.
    pub fn criar_admin(&self, email: &str, nome: &str) -> R<String> {
        let email = auth::validar_email(email).ok_or("e-mail inválido")?;
        let b: [u8; 18] = crate::util::aleatorio().map_err(|e| Erro::interno(&e))?;
        let senha = crate::util::base32(&b);
        let hash = auth::hash_da_senha(&senha).map_err(|e| Erro::interno(&e))?;
        let (_, hashes) = auth::codigos_de_recuperacao().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        let t = self.agora();
        if let Some(&id) = n.estado.por_email.get(&email) {
            let mut papeis = n.estado.usuarios.get(&id).map(|u| u.papeis).unwrap_or_default();
            papeis.admin = true;
            self.registrar(
                &mut n,
                vec![
                    Evento::RoleGranted { usuario: id, papeis, por: 0, t },
                    Evento::PasswordChanged { usuario: id, senha: hash, como: "admin-cli".into(), por: 0, t },
                ],
            )?;
        } else {
            let id = n.estado.seq.usuario;
            let termos = n.estado.config.termos_versao;
            self.registrar(
                &mut n,
                vec![Evento::UserCreated {
                    id,
                    email,
                    nome: texto_curto(nome, 80),
                    papeis: Papeis { cliente: true, fornecedor: true, admin: true },
                    moeda: Moeda::Brl,
                    pais: "BR".into(),
                    senha: hash,
                    recuperacao: hashes,
                    termos,
                    ip: "linha-de-comando".into(),
                    t,
                }],
            )?;
        }
        Ok(senha)
    }

    fn conferir_cadastros_do_ip(&self, ip: &str, limite: u32) -> R<()> {
        let agora = self.agora();
        let excedeu = {
            let mut v = self.vivo()?;
            let lista = v.cadastros.entry(ip.to_string()).or_default();
            lista.retain(|&t| agora.saturating_sub(t) < 86_400_000);
            lista.len() >= usize::try_from(limite).unwrap_or(usize::MAX)
        };
        if excedeu {
            self.alertar_uma_vez("fraude:multiplas-contas", "media", &format!("ip:{ip}"), &format!("mais de {limite} cadastros em 24 h do mesmo IP"))?;
            return Err(Erro::limite("muitas contas criadas a partir desta rede hoje; tente amanhã ou fale com o suporte"));
        }
        Ok(())
    }

    /// Entra com e-mail e senha.
    pub fn entrar(&self, email: &str, senha: &str, ip: &str, agente: &str) -> R<Entrada> {
        let generico = Erro { status: 401, mensagem: "e-mail ou senha incorretos".into() };
        let email = auth::validar_email(email).unwrap_or_default();
        let agora = self.agora();
        for chave in [format!("e:{email}"), format!("i:{ip}")] {
            let v = self.vivo()?;
            if let Some(&(_, ate, _)) = v.falhas_login.get(&chave)
                && ate > agora
            {
                let min = ate.saturating_sub(agora).div_ceil(60_000);
                return Err(Erro::limite(format!("muitas tentativas; tente de novo em {min} min")));
            }
        }
        let alvo = self.ler(|e| e.por_email.get(&email).and_then(|id| e.usuarios.get(id)).map(|u| (u.id, u.senha.clone(), u.bloqueado)))?;
        let Some((id, hash, bloqueado)) = alvo else {
            auth::gastar_tempo_como_login(senha);
            self.falha_de_login(&email, ip)?;
            return Err(generico);
        };
        if !auth::senha_confere(senha, &hash) {
            self.falha_de_login(&email, ip)?;
            return Err(generico);
        }
        if bloqueado {
            return Err(Erro::proibido("esta conta está bloqueada; fale com o suporte"));
        }
        let (token, hash_token) = auth::token_de_sessao().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        let t = self.agora();
        self.registrar(
            &mut n,
            vec![Evento::Login {
                sessao: hash_token.clone(),
                usuario: id,
                expira: t.saturating_add(SESSAO_MAX_MS),
                ip: ip.to_string(),
                agente: texto_curto(agente, 120),
                t,
            }],
        )?;
        drop(n);
        let mut v = self.vivo()?;
        v.atividade.insert(hash_token, t);
        v.falhas_login.remove(&format!("e:{email}"));
        Ok(Entrada { usuario: id, token, codigos: Vec::new() })
    }

    fn falha_de_login(&self, email: &str, ip: &str) -> R<()> {
        let agora = self.agora();
        let mut travou = Vec::new();
        {
            let mut v = self.vivo()?;
            for (chave, limite) in [(format!("e:{email}"), FALHAS_POR_EMAIL), (format!("i:{ip}"), FALHAS_POR_IP)] {
                let f = v.falhas_login.entry(chave.clone()).or_insert((0, 0, agora));
                if agora.saturating_sub(f.2) > TRAVA_MS {
                    *f = (0, 0, agora);
                }
                f.0 = f.0.saturating_add(1);
                if f.0 >= limite {
                    f.1 = agora.saturating_add(TRAVA_MS);
                    f.0 = 0;
                    f.2 = agora;
                    travou.push(chave);
                }
            }
        }
        for chave in travou {
            self.alertar("seguranca:forca-bruta", "media", &chave.replacen("e:", "email:", 1).replacen("i:", "ip:", 1), "senhas erradas em sequência: entrada travada por 15 min")?;
        }
        Ok(())
    }

    /// Fecha a sessão.
    pub fn sair(&self, hash_sessao: &str) -> R<()> {
        let mut n = self.nucleo()?;
        if !n.estado.sessoes.contains_key(hash_sessao) {
            return Ok(());
        }
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::Logout { sessao: hash_sessao.to_string(), motivo: "saiu".into(), t }])
    }

    /// Autentica um token de sessão.
    pub fn por_sessao(&self, token: &str) -> R<Quem> {
        let h = auth::hash_do_segredo(token);
        let agora = self.agora();
        let (usuario, papeis, moeda, expira, bloqueado) = self
            .ler(|e| e.sessoes.get(&h).and_then(|s| e.usuarios.get(&s.usuario).map(|u| (u.id, u.papeis, u.moeda, s.expira_ms, u.bloqueado))))?
            .ok_or_else(Erro::sem_sessao)?;
        if bloqueado {
            return Err(Erro::proibido("esta conta está bloqueada"));
        }
        let ociosa = {
            let mut v = self.vivo()?;
            let ultima = *v.atividade.entry(h.clone()).or_insert(agora);
            let ociosa = agora.saturating_sub(ultima) > SESSAO_OCIOSA_MS;
            if !ociosa {
                v.atividade.insert(h.clone(), agora);
            }
            ociosa
        };
        if expira <= agora || ociosa {
            let mut n = self.nucleo()?;
            if n.estado.sessoes.contains_key(&h) {
                self.registrar(&mut n, vec![Evento::Logout { sessao: h, motivo: "expirou".into(), t: agora }])?;
            }
            return Err(Erro::sem_sessao());
        }
        Ok(Quem { usuario, papeis, sessao: Some(h), escopos: None, moeda })
    }

    /// Autentica uma chave de API (`Authorization: Bearer hyx_...`).
    pub fn por_chave(&self, chave: &str) -> R<Quem> {
        let h = auth::hash_do_segredo(chave.trim());
        let achada = self.ler(|e| {
            let id = e.chave_por_hash.get(&h)?;
            let c = e.chaves.get(id)?;
            let u = e.usuarios.get(&c.usuario)?;
            Some((c.id, c.escopos.clone(), c.limite_por_min, u.id, u.papeis, u.moeda, u.bloqueado))
        })?;
        let Some((id, escopos, limite, usuario, papeis, moeda, bloqueado)) = achada else {
            return Err(Erro { status: 401, mensagem: "chave de API inválida ou revogada".into() });
        };
        if bloqueado {
            return Err(Erro::proibido("esta conta está bloqueada"));
        }
        if !self.ritmo(&format!("chave:{id}"), limite) {
            return Err(Erro::limite(format!("limite da chave: {limite} pedidos por minuto")));
        }
        Ok(Quem { usuario, papeis, sessao: None, escopos: Some(escopos), moeda })
    }

    /// Os dados da conta.
    pub fn eu(&self, q: &Quem) -> R<Value> {
        let ambiente_teste = self.opcoes.modo_teste;
        self.ler(|e| {
            let u = e.usuarios.get(&q.usuario)?;
            Some(json!({
                "id": u.id,
                "email": u.email,
                "nome": u.nome,
                "papeis": u.papeis.nomes(),
                "moeda": u.moeda.codigo(),
                "pais": u.pais,
                "verificado": u.verificado,
                "criado_ms": u.criado_ms,
                "codigos_restantes": u.recuperacao.iter().filter(|c| !c.is_empty()).count(),
                "ambiente_teste": ambiente_teste,
            }))
        })?
        .ok_or_else(Erro::sem_sessao)
    }

    /// Troca a senha (exige a atual). Derruba as outras sessões e devolve um
    /// token novo para esta.
    pub fn trocar_senha(&self, q: &Quem, atual: &str, nova: &str, ip: &str, agente: &str) -> R<String> {
        let (email, hash) = self.ler(|e| e.usuarios.get(&q.usuario).map(|u| (u.email.clone(), u.senha.clone())))?.ok_or_else(Erro::sem_sessao)?;
        if !auth::senha_confere(atual, &hash) {
            return Err(Erro::proibido("a senha atual não confere"));
        }
        auth::validar_senha(nova, &email)?;
        let novo = auth::hash_da_senha(nova).map_err(|e| Erro::interno(&e))?;
        let (token, h) = auth::token_de_sessao().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        let t = self.agora();
        self.registrar(
            &mut n,
            vec![
                Evento::PasswordChanged { usuario: q.usuario, senha: novo, como: "troca".into(), por: q.usuario, t },
                Evento::Login { sessao: h.clone(), usuario: q.usuario, expira: t.saturating_add(SESSAO_MAX_MS), ip: ip.into(), agente: texto_curto(agente, 120), t },
            ],
        )?;
        drop(n);
        self.vivo()?.atividade.insert(h, t);
        Ok(token)
    }

    /// Recupera a conta com um código de recuperação.
    pub fn recuperar(&self, email: &str, codigo: &str, nova: &str) -> R<()> {
        let generico = Erro::proibido("e-mail ou código de recuperação incorretos");
        let email = auth::validar_email(email).ok_or_else(|| generico.clone())?;
        auth::validar_senha(nova, &email)?;
        let h = auth::hash_do_segredo(&codigo.trim().to_lowercase());
        let achado = self.ler(|e| {
            let u = e.por_email.get(&email).and_then(|id| e.usuarios.get(id))?;
            // confere todos, sem parar no primeiro: o tempo não diz qual bateu
            let mut indice = None;
            for (i, c) in u.recuperacao.iter().enumerate() {
                if !c.is_empty() && crate::util::iguais(c.as_bytes(), h.as_bytes()) {
                    indice = Some(i);
                }
            }
            Some((u.id, indice?))
        })?;
        let Some((id, indice)) = achado else { return Err(generico) };
        let novo = auth::hash_da_senha(nova).map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        let t = self.agora();
        self.registrar(
            &mut n,
            vec![
                Evento::RecoveryCodeUsed { usuario: id, indice: u32::try_from(indice).unwrap_or(u32::MAX), t },
                Evento::PasswordChanged { usuario: id, senha: novo, como: "recuperacao".into(), por: id, t },
            ],
        )
    }

    /// Gera códigos de recuperação novos (exige a senha).
    pub fn novos_codigos(&self, q: &Quem, senha: &str) -> R<Vec<String>> {
        let hash = self.ler(|e| e.usuarios.get(&q.usuario).map(|u| u.senha.clone()))?.ok_or_else(Erro::sem_sessao)?;
        if !auth::senha_confere(senha, &hash) {
            return Err(Erro::proibido("a senha não confere"));
        }
        let (codigos, hashes) = auth::codigos_de_recuperacao().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::RecoveryCodesIssued { usuario: q.usuario, recuperacao: hashes, t }])?;
        Ok(codigos)
    }

    /// Ativa o papel de cliente ou de fornecedor na própria conta.
    pub fn ativar_papel(&self, q: &Quem, papel: &str) -> R<Value> {
        let mut n = self.nucleo()?;
        let mut papeis = n.estado.usuarios.get(&q.usuario).map(|u| u.papeis).ok_or_else(Erro::sem_sessao)?;
        match papel {
            "cliente" | "CUSTOMER" => papeis.cliente = true,
            "fornecedor" | "PROVIDER" => papeis.fornecedor = true,
            _ => return Err(Erro::pedido("papel: cliente ou fornecedor")),
        }
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::RoleGranted { usuario: q.usuario, papeis, por: q.usuario, t }])?;
        Ok(json!({ "papeis": papeis.nomes() }))
    }

    /// As sessões abertas da conta.
    pub fn sessoes(&self, q: &Quem) -> R<Value> {
        self.ler(|e| {
            let mut v: Vec<Value> = e
                .sessoes
                .iter()
                .filter(|(_, s)| s.usuario == q.usuario)
                .map(|(h, s)| {
                    json!({ "id": h.get(..16).unwrap_or(""), "criada_ms": s.criada_ms, "expira_ms": s.expira_ms, "ip": s.ip, "navegador": s.agente, "esta": q.sessao.as_deref() == Some(h.as_str()) })
                })
                .collect();
            v.sort_by_key(|x| std::cmp::Reverse(x["criada_ms"].as_u64().unwrap_or(0)));
            json!({ "sessoes": v })
        })
    }

    /// Encerra as outras sessões da conta.
    pub fn encerrar_outras_sessoes(&self, q: &Quem) -> R<u32> {
        let mut n = self.nucleo()?;
        let t = self.agora();
        let eventos: Vec<Evento> = n
            .estado
            .sessoes
            .iter()
            .filter(|(h, s)| s.usuario == q.usuario && q.sessao.as_deref() != Some(h.as_str()))
            .map(|(h, _)| Evento::Logout { sessao: h.clone(), motivo: "encerrada pelo dono".into(), t })
            .collect();
        let quantas = u32::try_from(eventos.len()).unwrap_or(u32::MAX);
        self.registrar(&mut n, eventos)?;
        Ok(quantas)
    }

    /// As chaves de API (sem a chave: só o começo).
    pub fn chaves(&self, q: &Quem) -> R<Value> {
        self.ler(|e| {
            let v: Vec<Value> = e
                .chaves
                .values()
                .filter(|c| c.usuario == q.usuario)
                .map(|c| {
                    json!({
                        "id": c.id, "nome": c.nome, "prefixo": c.prefixo,
                        "escopos": c.escopos.iter().map(|s| s.nome()).collect::<Vec<_>>(),
                        "limite_por_min": c.limite_por_min, "criada_ms": c.criada_ms, "revogada_ms": c.revogada_ms,
                    })
                })
                .collect();
            json!({ "chaves": v })
        })
    }

    /// Cria uma chave de API. A chave inteira só aparece nesta resposta.
    pub fn criar_chave(&self, q: &Quem, nome: &str, escopos: &[String], limite_por_min: u32) -> R<Value> {
        if q.escopos.is_some() {
            return Err(Erro::proibido("chave de API não cria outra chave"));
        }
        let nome = texto_curto(nome, 60);
        if nome.is_empty() {
            return Err(Erro::pedido("dê um nome à chave (onde ela vai ser usada)"));
        }
        let mut lista: Vec<Escopo> = Vec::new();
        for s in escopos {
            let e = Escopo::ler(s).ok_or_else(|| Erro::pedido(format!("escopo desconhecido: {s}")))?;
            if !lista.contains(&e) {
                lista.push(e);
            }
        }
        if lista.is_empty() {
            return Err(Erro::pedido("escolha pelo menos um escopo"));
        }
        lista.sort();
        let limite = limite_por_min.clamp(1, 600);
        let (chave, prefixo, hash) = auth::chave_de_api().map_err(|e| Erro::interno(&e))?;
        let mut n = self.nucleo()?;
        let ativas = n.estado.chaves.values().filter(|c| c.usuario == q.usuario && c.revogada_ms.is_none()).count();
        if ativas >= CHAVES_MAX {
            return Err(Erro::conflito(format!("já há {CHAVES_MAX} chaves ativas; revogue uma antes")));
        }
        let id = n.estado.seq.chave;
        let t = self.agora();
        self.registrar(
            &mut n,
            vec![Evento::ApiKeyCreated { id, usuario: q.usuario, nome: nome.clone(), prefixo: prefixo.clone(), hash, escopos: lista.clone(), limite_por_min: limite, t }],
        )?;
        Ok(json!({
            "id": id, "nome": nome, "prefixo": prefixo, "chave": chave,
            "escopos": lista.iter().map(|s| s.nome()).collect::<Vec<_>>(), "limite_por_min": limite,
            "aviso": "guarde a chave agora: ela não aparece de novo",
        }))
    }

    /// Revoga uma chave de API.
    pub fn revogar_chave(&self, q: &Quem, id: Id) -> R<()> {
        let mut n = self.nucleo()?;
        let c = n.estado.chaves.get(&id).filter(|c| c.usuario == q.usuario || q.papeis.admin).ok_or_else(|| Erro::nao_achado("chave não encontrada"))?;
        if c.revogada_ms.is_some() {
            return Ok(());
        }
        let t = self.agora();
        self.registrar(&mut n, vec![Evento::ApiKeyRevoked { id, por: q.usuario, t }])
    }

    /// Rotação: cria uma chave igual (nome, escopos, limite) e revoga a velha.
    pub fn rotacionar_chave(&self, q: &Quem, id: Id) -> R<Value> {
        let velha = self
            .ler(|e| e.chaves.get(&id).filter(|c| c.usuario == q.usuario && c.revogada_ms.is_none()).cloned())?
            .ok_or_else(|| Erro::nao_achado("chave não encontrada ou já revogada"))?;
        self.revogar_chave(q, id)?;
        let escopos: Vec<String> = velha.escopos.iter().map(|s| s.nome().to_string()).collect();
        self.criar_chave(q, &velha.nome, &escopos, velha.limite_por_min)
    }

    /// Emite um alerta de segurança.
    pub fn alertar(&self, tipo: &str, severidade: &str, alvo: &str, detalhe: &str) -> R<()> {
        let mut n = self.nucleo()?;
        let id = n.estado.seq.alerta;
        let t = self.agora();
        self.registrar(
            &mut n,
            vec![Evento::SecurityAlert { id, tipo: tipo.into(), severidade: severidade.into(), alvo: alvo.into(), detalhe: texto_curto(detalhe, 400), t }],
        )
    }

    /// Emite o alerta só se não houver um igual aberto (mesmo tipo e alvo).
    pub fn alertar_uma_vez(&self, tipo: &str, severidade: &str, alvo: &str, detalhe: &str) -> R<()> {
        let aberto = self.ler(|e| e.alertas.values().any(|a| a.resolvido.is_none() && a.tipo == tipo && a.alvo == alvo))?;
        if aberto { Ok(()) } else { self.alertar(tipo, severidade, alvo, detalhe) }
    }

    pub(crate) fn encerrar_sessoes_vencidas(&self) -> R<()> {
        let agora = self.agora();
        let mut n = self.nucleo()?;
        let eventos: Vec<Evento> = n
            .estado
            .sessoes
            .iter()
            .filter(|(_, s)| s.expira_ms <= agora)
            .take(200)
            .map(|(h, _)| Evento::Logout { sessao: h.clone(), motivo: "expirou".into(), t: agora })
            .collect();
        self.registrar(&mut n, eventos)
    }
}

/// Um identificador curto de sessão, para logs (nunca o token).
pub fn rotulo_da_sessao(hash: &str) -> String {
    hex(hash.as_bytes().get(..8).unwrap_or_default())
}
