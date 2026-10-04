//! Os ajustes do dono, em `ajustes.txt` (pasta de configuração).
//!
//! Texto simples, uma chave por linha. Chave desconhecida é ignorada (sobra
//! de versão antiga), valor fora da faixa volta ao padrão. Apagar o arquivo
//! volta tudo ao padrão.

use std::fmt::Write as _;
use std::path::Path;

use crate::{arquivos, maquinas, sementes, ultrax};

/// Nome do arquivo.
pub const ARQUIVO: &str = "ajustes.txt";
/// Watts de um núcleo trabalhando, para a estimativa de energia (o dono ajusta).
pub const WATTS_NUCLEO_PADRAO: u32 = 12;
/// Preço do kWh, em centavos (o dono ajusta).
pub const CENTAVOS_KWH_PADRAO: u32 = 90;

/// Os ajustes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ajustes {
    /// Minerar ao abrir.
    pub minerar: bool,
    /// Linhas da mineração.
    pub linhas: Option<u32>,
    /// Limite de CPU da mineração, por linha (10 a 100).
    pub limite_cpu: u32,
    /// Watts de um núcleo trabalhando (estimativa de energia).
    pub watts_nucleo: u32,
    /// Preço do kWh, em centavos.
    pub centavos_kwh: u32,
    /// Deixar o painel ser lido por outros aparelhos da rede local.
    pub na_rede: bool,
    /// Avisar quando um bloco deste nó entrar, e com som.
    pub avisar_bloco: bool,
    /// Som no aviso de bloco.
    pub som_bloco: bool,
    /// ULTRAX ligado ao abrir.
    pub ultrax: bool,
    /// Linhas do ULTRAX.
    pub ultrax_linhas: u32,
    /// Limite de CPU do ULTRAX, por linha.
    pub ultrax_limite_cpu: u32,
    /// Teto de memória do ULTRAX, em MiB.
    pub ultrax_memoria_mib: u32,
    /// Registro detalhado do ULTRAX.
    pub ultrax_debug: bool,
    /// GPU no ULTRAX.
    pub ultrax_gpu: bool,
    /// Limite de uso da GPU pelo ULTRAX.
    pub ultrax_gpu_limite: u32,
    /// Calcular unidades que outros nós pedirem.
    pub aceitar_rede: bool,
    /// A API externa (contas de cliente mandam JOBs de fora).
    pub api_externa: bool,
    /// A API das carteiras de celular (`/api/v1/leve/`).
    pub carteiras_leves: bool,
    /// Sementes do dono.
    pub sementes: Vec<String>,
    /// As outras máquinas do dono.
    pub maquinas: Vec<String>,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            minerar: false,
            linhas: None,
            limite_cpu: 100,
            watts_nucleo: WATTS_NUCLEO_PADRAO,
            centavos_kwh: CENTAVOS_KWH_PADRAO,
            na_rede: false,
            avisar_bloco: true,
            som_bloco: false,
            ultrax: false,
            ultrax_linhas: 1,
            ultrax_limite_cpu: 100,
            ultrax_memoria_mib: ultrax::MEMORIA_PADRAO_MIB,
            ultrax_debug: false,
            ultrax_gpu: false,
            ultrax_gpu_limite: 50,
            aceitar_rede: false,
            api_externa: false,
            carteiras_leves: false,
            sementes: Vec::new(),
            maquinas: Vec::new(),
        }
    }
}

impl Ajustes {
    /// Lê da pasta de configuração; sem arquivo, o padrão.
    pub fn ler(pasta: &Path) -> Self {
        let mut a = Self::default();
        let Ok(texto) = std::fs::read_to_string(pasta.join(ARQUIVO)) else { return a };
        for linha in texto.lines() {
            let Some((chave, valor)) = linha.trim().split_once('=') else { continue };
            let v = valor.trim();
            let sim = v == "1";
            let faixa = |min: u32, max: u32| v.parse::<u32>().ok().filter(|n| (min..=max).contains(n));
            match chave.trim() {
                "minerar" => a.minerar = sim,
                "linhas" => a.linhas = faixa(1, 1024),
                "uso_cpu" | "limite_cpu" => a.limite_cpu = faixa(10, 100).unwrap_or(a.limite_cpu),
                "watts_nucleo" => a.watts_nucleo = faixa(1, 200).unwrap_or(a.watts_nucleo),
                "centavos_kwh" => a.centavos_kwh = faixa(1, 99_999).unwrap_or(a.centavos_kwh),
                "na_rede" => a.na_rede = sim,
                "avisar_bloco" => a.avisar_bloco = sim,
                "som_bloco" => a.som_bloco = sim,
                "ultrax" => a.ultrax = sim,
                "ultrax_linhas" => a.ultrax_linhas = faixa(1, 1024).unwrap_or(a.ultrax_linhas),
                "ultrax_uso_cpu" | "ultrax_limite_cpu" => a.ultrax_limite_cpu = faixa(10, 100).unwrap_or(a.ultrax_limite_cpu),
                "ultrax_memoria_mib" => {
                    a.ultrax_memoria_mib = faixa(ultrax::MEMORIA_MIN_MIB, ultrax::MEMORIA_MAX_MIB).unwrap_or(a.ultrax_memoria_mib);
                }
                "ultrax_debug" => a.ultrax_debug = sim,
                "ultrax_gpu" => a.ultrax_gpu = sim,
                "ultrax_gpu_uso" | "ultrax_gpu_limite" => a.ultrax_gpu_limite = faixa(10, 100).unwrap_or(a.ultrax_gpu_limite),
                "aceitar_rede" => a.aceitar_rede = sim,
                "api_externa" => a.api_externa = sim,
                "carteiras_leves" => a.carteiras_leves = sim,
                "semente" if sementes::valida(v) && a.sementes.len() < sementes::MAXIMO => a.sementes.push(v.to_string()),
                "maquina" if sementes::valida(v) && a.maquinas.len() < maquinas::MAXIMO => a.maquinas.push(v.to_string()),
                _ => {}
            }
        }
        a
    }

    /// O texto do arquivo.
    pub fn texto(&self) -> String {
        let b = |x: bool| u8::from(x);
        let mut t = String::from("# Ajustes do Hyurax. Pode apagar: volta tudo ao padrão.\n");
        let _ = writeln!(t, "minerar={}", b(self.minerar));
        if let Some(l) = self.linhas {
            let _ = writeln!(t, "linhas={l}");
        }
        let _ = writeln!(t, "limite_cpu={}", self.limite_cpu);
        let _ = writeln!(t, "watts_nucleo={}", self.watts_nucleo);
        let _ = writeln!(t, "centavos_kwh={}", self.centavos_kwh);
        let _ = writeln!(t, "na_rede={}", b(self.na_rede));
        let _ = writeln!(t, "avisar_bloco={}", b(self.avisar_bloco));
        let _ = writeln!(t, "som_bloco={}", b(self.som_bloco));
        let _ = writeln!(t, "ultrax={}", b(self.ultrax));
        let _ = writeln!(t, "ultrax_linhas={}", self.ultrax_linhas);
        let _ = writeln!(t, "ultrax_limite_cpu={}", self.ultrax_limite_cpu);
        let _ = writeln!(t, "ultrax_memoria_mib={}", self.ultrax_memoria_mib);
        let _ = writeln!(t, "ultrax_debug={}", b(self.ultrax_debug));
        let _ = writeln!(t, "ultrax_gpu={}", b(self.ultrax_gpu));
        let _ = writeln!(t, "ultrax_gpu_limite={}", self.ultrax_gpu_limite);
        let _ = writeln!(t, "aceitar_rede={}", b(self.aceitar_rede));
        let _ = writeln!(t, "api_externa={}", b(self.api_externa));
        let _ = writeln!(t, "carteiras_leves={}", b(self.carteiras_leves));
        for s in &self.sementes {
            let _ = writeln!(t, "semente={s}");
        }
        for m in &self.maquinas {
            let _ = writeln!(t, "maquina={m}");
        }
        t
    }

    /// Grava na pasta de configuração.
    ///
    /// # Errors
    /// Sem permissão ou disco cheio.
    pub fn gravar(&self, pasta: &Path) -> Result<(), String> {
        arquivos::gravar_atomico(&pasta.join(ARQUIVO), self.texto().as_bytes())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod testes {
    use super::*;

    #[test]
    fn grava_e_le_igual_e_aceita_as_chaves_da_0x() {
        let p = std::env::temp_dir().join(format!("hyurax-ajustes-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        let a = Ajustes {
            minerar: true,
            linhas: Some(3),
            limite_cpu: 55,
            ultrax: true,
            ultrax_gpu: true,
            aceitar_rede: true,
            sementes: vec!["203.0.113.7:8790".into()],
            maquinas: vec!["192.168.0.9:8800".into()],
            ..Ajustes::default()
        };
        a.gravar(&p).unwrap();
        assert_eq!(Ajustes::ler(&p), a);
        // arquivo de uma 0.x: chaves antigas continuam valendo, as que saíram são ignoradas
        std::fs::write(p.join(ARQUIVO), "uso_cpu=40\nultrax_uso_cpu=30\nmercado=1\npaineis=inicio,ultrax\nlinhas=0\n").unwrap();
        let b = Ajustes::ler(&p);
        assert_eq!((b.limite_cpu, b.ultrax_limite_cpu, b.linhas), (40, 30, None));
        let _ = std::fs::remove_dir_all(p);
    }
}
