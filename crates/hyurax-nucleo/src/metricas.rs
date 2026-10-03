//! Métricas da máquina, com a origem de cada número.
//!
//! Regra: nenhum número sem origem. Cada valor sai com um destes rótulos:
//!
//! | rótulo | quer dizer |
//! |---|---|
//! | `REAL` | medido agora, por um contador do sistema operacional |
//! | `DERIVADO` | conta exata sobre um valor real |
//! | `ESTIMADO` | depende de um parâmetro que ninguém mediu (por exemplo, watts por núcleo) |
//! | `PENDENTE` | o programa ainda não sabe medir isto nesta máquina; o motivo vai junto |
//!
//! **Windows:** um processo auxiliar do PowerShell mantém abertos os
//! contadores de desempenho do sistema (`System.Diagnostics.PerformanceCounter`,
//! pelos nomes em inglês, que valem em qualquer idioma do Windows) e manda uma
//! linha JSON a cada 2 segundos. Ele sai sozinho quando o Hyurax fecha. Nada
//! de biblioteca nova e nada de código `unsafe`.
//!
//! **Linux:** `/proc/stat`, `/proc/meminfo` e `/proc/self/stat`.
//!
//! Temperatura: PENDENTE em todo lugar. No Windows não há leitura confiável
//! sem administrador (a zona térmica ACPI, quando existe, não é a da CPU).
//!
//! GPU: o uso é o do **adaptador inteiro** (todos os programas), pelo contador
//! "GPU Engine" do Windows 10 1709 ou mais novo. O quanto disso é do ULTRAX
//! vem do próprio backend da GPU (ciclo de trabalho medido), em
//! [`crate::ultrax`].

use std::collections::VecDeque;
#[cfg(windows)]
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::util::agora_ms;

/// Quantas leituras ficam guardadas (uma a cada 2 s: 4 minutos).
const HISTORICO: usize = 120;

/// De onde veio um número.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origem {
    /// Medido agora.
    Real,
    /// Conta exata sobre um valor real.
    Derivado,
    /// Depende de parâmetro não medido.
    Estimado,
    /// Conta real sobre entrada inventada.
    Simulado,
    /// Só visual.
    Mock,
    /// Ainda não existe.
    Pendente,
}

impl Origem {
    /// O rótulo, em maiúsculas.
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Real => "REAL",
            Self::Derivado => "DERIVADO",
            Self::Estimado => "ESTIMADO",
            Self::Simulado => "SIMULADO",
            Self::Mock => "MOCK",
            Self::Pendente => "PENDENTE",
        }
    }
}

/// Um valor com a origem e a fonte.
pub fn valor(v: Option<f64>, unidade: &str, origem: Origem, fonte: &str) -> Value {
    json!({ "valor": v, "unidade": unidade, "origem": origem.rotulo(), "fonte": fonte })
}

/// Tipo de uma placa de vídeo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoDeGpu {
    /// Dentro do processador (Intel HD/UHD/Iris, AMD Radeon Graphics das APUs).
    Integrada,
    /// Placa própria (NVIDIA GeForce/RTX/Quadro, AMD Radeon RX/Pro, Intel Arc).
    Dedicada,
    /// O nome não diz.
    Desconhecida,
}

impl TipoDeGpu {
    /// Nome para a tela.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Integrada => "integrada",
            Self::Dedicada => "dedicada",
            Self::Desconhecida => "não classificada",
        }
    }
}

/// Classifica a placa pelo nome do adaptador. É dedução pelo nome (DERIVADO),
/// não pergunta ao hardware.
pub fn tipo_de_gpu(nome: &str) -> TipoDeGpu {
    let n = nome.to_ascii_lowercase();
    let tem = |s: &str| n.contains(s);
    if tem("nvidia") || tem("geforce") || tem("quadro") || tem("rtx") || tem("radeon rx") || tem("radeon pro") || tem(" arc") || n.starts_with("arc") {
        TipoDeGpu::Dedicada
    } else if tem("intel") || tem("radeon(tm) graphics") || tem("radeon graphics") || tem("vega") {
        TipoDeGpu::Integrada
    } else {
        TipoDeGpu::Desconhecida
    }
}

/// Uma placa de vídeo vista pelo sistema.
#[derive(Clone, Debug, PartialEq)]
pub struct Gpu {
    /// Nome do adaptador.
    pub nome: String,
    /// Memória informada pelo driver, em MiB (o Windows corta em 4 GiB).
    pub memoria_mib: Option<u64>,
    /// Integrada ou dedicada (pelo nome).
    pub tipo: TipoDeGpu,
}

/// O que não muda enquanto o programa roda.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sistema {
    /// Sistema operacional.
    pub so: String,
    /// Nome do processador.
    pub cpu: String,
    /// Núcleos físicos (quando o sistema diz).
    pub nucleos_fisicos: Option<u32>,
    /// Núcleos lógicos.
    pub nucleos_logicos: u32,
    /// Memória total, em MiB.
    pub ram_total_mib: Option<u64>,
    /// Placas de vídeo.
    pub gpus: Vec<Gpu>,
}

/// Uma leitura dos contadores.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Leitura {
    /// Milissegundos desde 1970.
    pub quando_ms: u64,
    /// CPU da máquina inteira, em %.
    pub cpu_total: Option<f64>,
    /// CPU deste programa, em % da máquina inteira.
    pub cpu_processo: Option<f64>,
    /// Memória livre da máquina, em MiB.
    pub ram_livre_mib: Option<f64>,
    /// Memória privada deste programa, em MiB.
    pub ram_processo_mib: Option<f64>,
    /// Uso dos motores 3D da GPU (todos os programas), em %.
    pub gpu_3d: Option<f64>,
    /// Uso dos motores de cálculo da GPU (todos os programas), em %.
    pub gpu_calculo: Option<f64>,
    /// Memória dedicada da GPU em uso (todos os programas), em MiB.
    pub gpu_dedicada_mib: Option<f64>,
    /// Memória compartilhada da GPU em uso (todos os programas), em MiB.
    pub gpu_compartilhada_mib: Option<f64>,
}

/// As métricas vivas.
pub struct Metricas {
    sistema: Mutex<Sistema>,
    leituras: Mutex<VecDeque<Leitura>>,
    fonte: &'static str,
    problema: Mutex<Option<String>>,
}

impl Metricas {
    /// Começa a medir numa linha própria.
    pub fn iniciar() -> Arc<Self> {
        let fonte = if cfg!(windows) {
            "contadores de desempenho do Windows"
        } else if cfg!(target_os = "linux") {
            "/proc do Linux"
        } else {
            "nenhuma"
        };
        let m = Arc::new(Self {
            sistema: Mutex::new(Sistema {
                nucleos_logicos: u32::try_from(std::thread::available_parallelism().map_or(1, |n| n.get())).unwrap_or(1),
                so: std::env::consts::OS.to_string(),
                ..Sistema::default()
            }),
            leituras: Mutex::new(VecDeque::new()),
            fonte,
            problema: Mutex::new(None),
        });
        let m2 = Arc::clone(&m);
        std::thread::spawn(move || m2.medir_para_sempre());
        m
    }

    // no Linux a medição lê /proc e não tem o que anotar
    #[cfg(not(target_os = "linux"))]
    fn anotar_problema(&self, p: String) {
        if let Ok(mut x) = self.problema.lock() {
            *x = Some(p);
        }
    }

    fn guardar(&self, l: Leitura) {
        if let Ok(mut v) = self.leituras.lock() {
            v.push_back(l);
            while v.len() > HISTORICO {
                v.pop_front();
            }
        }
    }

    #[cfg(windows)]
    fn medir_para_sempre(&self) {
        loop {
            if let Err(e) = self.medir_windows() {
                self.anotar_problema(e);
            }
            // o auxiliar caiu: tenta de novo daqui a pouco
            std::thread::sleep(Duration::from_secs(15));
        }
    }

    #[cfg(windows)]
    fn medir_windows(&self) -> Result<(), String> {
        use std::os::windows::process::CommandExt as _;
        const SEM_JANELA: u32 = 0x0800_0000;
        let script = SCRIPT_WINDOWS.replace("__PAI__", &std::process::id().to_string());
        let mut filho = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &crate::util::base64(&utf16le(&script))])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .creation_flags(SEM_JANELA)
            .spawn()
            .map_err(|e| format!("não consegui abrir o PowerShell para ler os contadores: {e}"))?;
        let saida = filho.stdout.take().ok_or("o PowerShell não abriu a saída")?;
        for linha in BufReader::new(saida).lines() {
            let Ok(linha) = linha else { break };
            let Ok(v) = serde_json::from_str::<Value>(&linha) else { continue };
            if v.get("sistema").is_some() {
                self.ler_sistema(&v);
            } else {
                self.guardar(leitura_de_json(&v));
                if let Ok(mut p) = self.problema.lock() {
                    *p = None;
                }
            }
        }
        let _ = filho.kill();
        let _ = filho.wait();
        Err("o leitor de contadores do Windows parou".into())
    }

    #[cfg(windows)]
    fn ler_sistema(&self, v: &Value) {
        let texto = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().trim().to_string();
        let numero = |k: &str| v.get(k).and_then(Value::as_u64);
        let gpus: Vec<Gpu> = v
            .get("gpus")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|g| {
                        let nome = g.get("nome").and_then(Value::as_str)?.trim().to_string();
                        let ram = g.get("ram").and_then(Value::as_u64).filter(|r| *r > 0).map(|r| r / (1024 * 1024));
                        Some(Gpu { tipo: tipo_de_gpu(&nome), nome, memoria_mib: ram })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Ok(mut s) = self.sistema.lock() {
            s.so = texto("so");
            s.cpu = texto("cpu");
            s.nucleos_fisicos = numero("fisicos").and_then(|n| u32::try_from(n).ok());
            if let Some(n) = numero("logicos").and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0) {
                s.nucleos_logicos = n;
            }
            s.ram_total_mib = numero("ram").map(|b| b / (1024 * 1024));
            s.gpus = gpus;
        }
    }

    #[cfg(target_os = "linux")]
    fn medir_para_sempre(&self) {
        self.ler_sistema_linux();
        let mut antes = linux::tempos();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let agora = linux::tempos();
            let (cpu_total, cpu_processo) = match (antes, agora) {
                (Some(a), Some(b)) => linux::uso(a, b),
                _ => (None, None),
            };
            antes = agora;
            let (_, livre) = linux::memoria();
            self.guardar(Leitura {
                quando_ms: agora_ms(),
                cpu_total,
                cpu_processo,
                ram_livre_mib: livre,
                ram_processo_mib: linux::memoria_do_processo(),
                ..Leitura::default()
            });
        }
    }

    #[cfg(target_os = "linux")]
    fn ler_sistema_linux(&self) {
        let cpu = std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.strip_prefix("model name").and_then(|r| r.split(':').nth(1)).map(|s| s.trim().to_string())))
            .unwrap_or_default();
        let (total, _) = linux::memoria();
        if let Ok(mut s) = self.sistema.lock() {
            s.cpu = cpu;
            s.ram_total_mib = total.map(|t| t as u64);
            s.so = "linux".into();
        }
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    fn medir_para_sempre(&self) {
        self.anotar_problema("medição de CPU, memória e GPU ainda não implementada neste sistema".into());
    }

    /// A última leitura.
    pub fn ultima(&self) -> Option<Leitura> {
        self.leituras.lock().ok().and_then(|v| v.back().copied())
    }

    /// As leituras guardadas, da mais velha para a mais nova.
    pub fn historico(&self) -> Vec<Leitura> {
        self.leituras.lock().map(|v| v.iter().copied().collect()).unwrap_or_default()
    }

    /// O que não muda.
    pub fn sistema(&self) -> Sistema {
        self.sistema.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Tudo em JSON, com a origem de cada número. `watts_nucleo` e
    /// `centavos_kwh` são os parâmetros do dono para a estimativa de energia.
    pub fn json(&self, watts_nucleo: u32, centavos_kwh: u32) -> Value {
        let s = self.sistema();
        let l = self.ultima();
        let fresca = l.filter(|l| agora_ms().saturating_sub(l.quando_ms) < 10_000);
        let problema = self.problema.lock().ok().and_then(|p| p.clone());
        let origem = |x: Option<f64>| if x.is_some() { Origem::Real } else { Origem::Pendente };
        let campo = |f: fn(&Leitura) -> Option<f64>| fresca.as_ref().and_then(f);
        let cpu_total = campo(|l| l.cpu_total);
        let cpu_processo = campo(|l| l.cpu_processo);
        let ram_livre = campo(|l| l.ram_livre_mib);
        let ram_proc = campo(|l| l.ram_processo_mib);
        let gpu_3d = campo(|l| l.gpu_3d);
        let gpu_calc = campo(|l| l.gpu_calculo);
        let gpu_ded = campo(|l| l.gpu_dedicada_mib);
        let gpu_comp = campo(|l| l.gpu_compartilhada_mib);
        let ram_usada = match (s.ram_total_mib, ram_livre) {
            (Some(t), Some(l)) => Some((t as f64 - l).max(0.0)),
            _ => None,
        };
        // energia: ESTIMADA pelo uso medido do programa e pelos watts por
        // núcleo que o dono informou; nunca medida
        let nucleos = f64::from(s.nucleos_logicos.max(1));
        let watts = cpu_processo.map(|p| p / 100.0 * nucleos * f64::from(watts_nucleo));
        let custo_mes = watts.map(|w| w * 24.0 * 30.0 / 1000.0 * f64::from(centavos_kwh) / 100.0);
        let fonte = self.fonte;
        let pendente = |motivo: &str| valor(None, "", Origem::Pendente, motivo);
        json!({
            "fonte": fonte,
            "problema": problema,
            "sistema": {
                "so": s.so,
                "cpu": s.cpu,
                "nucleos_fisicos": s.nucleos_fisicos,
                "nucleos_logicos": s.nucleos_logicos,
                "ram_total_mib": s.ram_total_mib,
                "gpus": s.gpus.iter().map(|g| json!({
                    "nome": g.nome,
                    "memoria_mib": g.memoria_mib,
                    "tipo": g.tipo.nome(),
                    "tipo_origem": "DERIVADO",
                    "tipo_fonte": "pelo nome do adaptador",
                })).collect::<Vec<_>>(),
            },
            "cpu_total": valor(cpu_total, "%", origem(cpu_total), "CPU da máquina inteira"),
            "cpu_processo": valor(cpu_processo, "%", origem(cpu_processo), "CPU deste programa, em % da máquina"),
            "ram_usada": valor(ram_usada, "MiB", if ram_usada.is_some() { Origem::Derivado } else { Origem::Pendente }, "total menos livre"),
            "ram_livre": valor(ram_livre, "MiB", origem(ram_livre), "memória livre da máquina"),
            "ram_processo": valor(ram_proc, "MiB", origem(ram_proc), "memória privada deste programa"),
            "gpu_3d": valor(gpu_3d, "%", origem(gpu_3d), "motores 3D da GPU, todos os programas"),
            "gpu_calculo": valor(gpu_calc, "%", origem(gpu_calc), "motores de cálculo da GPU, todos os programas"),
            "gpu_memoria_dedicada": valor(gpu_ded, "MiB", origem(gpu_ded), "memória dedicada da GPU em uso, todos os programas"),
            "gpu_memoria_compartilhada": valor(gpu_comp, "MiB", origem(gpu_comp), "memória compartilhada da GPU em uso, todos os programas"),
            "temperatura": pendente("sem leitura confiável de temperatura sem administrador; a zona térmica ACPI, quando existe, não é a da CPU"),
            "energia": valor(watts, "W", if watts.is_some() { Origem::Estimado } else { Origem::Pendente },
                &format!("CPU medida do programa × {} núcleos × {watts_nucleo} W por núcleo (valor informado, não medido)", s.nucleos_logicos)),
            "custo_mes": valor(custo_mes, "R$", if custo_mes.is_some() { Origem::Estimado } else { Origem::Pendente },
                &format!("energia estimada × 720 h × R$ {:.2} por kWh (valor informado)", f64::from(centavos_kwh) / 100.0)),
            "historico": self.historico().iter().map(|l| json!([l.quando_ms, l.cpu_total, l.cpu_processo, l.gpu_3d])).collect::<Vec<_>>(),
        })
    }
}

#[cfg(windows)]
fn leitura_de_json(v: &Value) -> Leitura {
    let n = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite());
    let nucleos = n("logicos").unwrap_or(1.0).max(1.0);
    Leitura {
        quando_ms: agora_ms(),
        cpu_total: n("cpu").map(|x| x.clamp(0.0, 100.0)),
        // o contador do processo soma os núcleos (4 núcleos cheios = 400%)
        cpu_processo: n("proc_cpu").map(|x| (x / nucleos).clamp(0.0, 100.0)),
        ram_livre_mib: n("livre_mb"),
        ram_processo_mib: n("proc_ram").map(|b| b / 1_048_576.0),
        gpu_3d: n("gpu_3d").map(|x| x.clamp(0.0, 100.0)),
        gpu_calculo: n("gpu_calculo").map(|x| x.clamp(0.0, 100.0)),
        gpu_dedicada_mib: n("gpu_dedicada").map(|b| b / 1_048_576.0),
        gpu_compartilhada_mib: n("gpu_compartilhada").map(|b| b / 1_048_576.0),
    }
}

#[cfg(windows)]
fn utf16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// O leitor de contadores do Windows. `__PAI__` vira o PID do Hyurax: o
/// auxiliar sai quando ele some.
#[cfg(windows)]
const SCRIPT_WINDOWS: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$pai = __PAI__
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$so = Get-CimInstance Win32_OperatingSystem
$cs = Get-CimInstance Win32_ComputerSystem
$gpus = @(Get-CimInstance Win32_VideoController | ForEach-Object { @{ nome = $_.Name; ram = [uint64]$_.AdapterRAM } })
$logicos = [Environment]::ProcessorCount
@{ sistema = 1; so = $so.Caption; cpu = $cpu.Name; fisicos = $cpu.NumberOfCores; logicos = $logicos; ram = [uint64]$cs.TotalPhysicalMemory; gpus = $gpus } | ConvertTo-Json -Compress -Depth 4
[Console]::Out.Flush()
function Contador($cat, $nome, $inst) { try { $c = New-Object System.Diagnostics.PerformanceCounter($cat, $nome, $inst, $true); [void]$c.NextValue(); $c } catch { $null } }
$total = Contador 'Processor' '% Processor Time' '_Total'
$livre = Contador 'Memory' 'Available MBytes' ''
function Instancia() {
  $cat = New-Object System.Diagnostics.PerformanceCounterCategory('Process')
  foreach ($n in $cat.GetInstanceNames()) {
    try { $c = New-Object System.Diagnostics.PerformanceCounter('Process', 'ID Process', $n, $true); if ([int]$c.RawValue -eq $pai) { return $n } } catch {}
  }
  return $null
}
function Gpus() {
  $l = @()
  try {
    $cat = New-Object System.Diagnostics.PerformanceCounterCategory('GPU Engine')
    foreach ($n in $cat.GetInstanceNames()) {
      if ($n -like '*engtype_3D' -or $n -like '*engtype_Compute*') { $c = Contador 'GPU Engine' 'Utilization Percentage' $n; if ($c) { $l += ,@($n, $c) } }
    }
  } catch {}
  return ,$l
}
function Memorias() {
  $l = @()
  try {
    $cat = New-Object System.Diagnostics.PerformanceCounterCategory('GPU Adapter Memory')
    foreach ($n in $cat.GetInstanceNames()) {
      $d = Contador 'GPU Adapter Memory' 'Dedicated Usage' $n; $s = Contador 'GPU Adapter Memory' 'Shared Usage' $n
      $l += ,@($d, $s)
    }
  } catch {}
  return ,$l
}
$volta = 0
while ($true) {
  if (-not (Get-Process -Id $pai)) { exit }
  if ($volta % 15 -eq 0) {
    $inst = Instancia
    $pc = if ($inst) { Contador 'Process' '% Processor Time' $inst } else { $null }
    $pr = if ($inst) { Contador 'Process' 'Working Set - Private' $inst } else { $null }
    $motores = Gpus
    $mems = Memorias
    Start-Sleep -Milliseconds 500
  }
  $g3 = $null; $gc = $null
  foreach ($m in $motores) { try { $v = $m[1].NextValue(); if ($m[0] -like '*engtype_3D') { $g3 += $v } else { $gc += $v } } catch {} }
  $gd = $null; $gs = $null
  foreach ($m in $mems) { try { if ($m[0]) { $gd += $m[0].NextValue() }; if ($m[1]) { $gs += $m[1].NextValue() } } catch {} }
  $x = @{ logicos = $logicos }
  if ($total) { $x.cpu = $total.NextValue() }
  if ($livre) { $x.livre_mb = $livre.NextValue() }
  if ($pc) { $x.proc_cpu = $pc.NextValue() }
  if ($pr) { $x.proc_ram = $pr.NextValue() }
  if ($motores.Count -gt 0) { $x.gpu_3d = [double]$g3; $x.gpu_calculo = [double]$gc }
  if ($mems.Count -gt 0) { $x.gpu_dedicada = [double]$gd; $x.gpu_compartilhada = [double]$gs }
  $x | ConvertTo-Json -Compress
  [Console]::Out.Flush()
  $volta++
  Start-Sleep -Seconds 2
}
"#;

#[cfg(target_os = "linux")]
mod linux {
    /// (tempo total da máquina, tempo ocioso, tempo deste processo), em tiques.
    pub fn tempos() -> Option<(u64, u64, u64)> {
        let stat = std::fs::read_to_string("/proc/stat").ok()?;
        let campos: Vec<u64> = stat.lines().next()?.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
        let total: u64 = campos.iter().sum();
        let ocioso = campos.get(3).copied()?.saturating_add(campos.get(4).copied().unwrap_or(0));
        let meu = std::fs::read_to_string("/proc/self/stat").ok()?;
        let depois = meu.rsplit_once(')')?.1;
        let v: Vec<u64> = depois.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        // campos 14 e 15 do stat (utime, stime), contados depois do ')'
        let proc = v.get(10).copied()?.saturating_add(v.get(11).copied()?);
        Some((total, ocioso, proc))
    }

    /// (% da máquina, % deste processo) entre duas leituras.
    pub fn uso(a: (u64, u64, u64), b: (u64, u64, u64)) -> (Option<f64>, Option<f64>) {
        let dt = b.0.saturating_sub(a.0) as f64;
        if dt <= 0.0 {
            return (None, None);
        }
        let ocioso = b.1.saturating_sub(a.1) as f64;
        let proc = b.2.saturating_sub(a.2) as f64;
        (Some(((dt - ocioso) / dt * 100.0).clamp(0.0, 100.0)), Some((proc / dt * 100.0).clamp(0.0, 100.0)))
    }

    /// (total, disponível), em MiB.
    pub fn memoria() -> (Option<f64>, Option<f64>) {
        let Ok(t) = std::fs::read_to_string("/proc/meminfo") else { return (None, None) };
        let kb = |nome: &str| {
            t.lines().find_map(|l| l.strip_prefix(nome)).and_then(|r| r.split_whitespace().next()).and_then(|x| x.parse::<f64>().ok()).map(|k| k / 1024.0)
        };
        (kb("MemTotal:"), kb("MemAvailable:"))
    }

    /// Memória residente deste processo, em MiB.
    pub fn memoria_do_processo() -> Option<f64> {
        let t = std::fs::read_to_string("/proc/self/status").ok()?;
        t.lines().find_map(|l| l.strip_prefix("VmRSS:")).and_then(|r| r.split_whitespace().next()).and_then(|x| x.parse::<f64>().ok()).map(|k| k / 1024.0)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod testes {
    use super::*;

    #[test]
    fn classifica_as_placas_pelo_nome() {
        assert_eq!(tipo_de_gpu("Intel(R) HD Graphics"), TipoDeGpu::Integrada);
        assert_eq!(tipo_de_gpu("Intel(R) UHD Graphics 620"), TipoDeGpu::Integrada);
        assert_eq!(tipo_de_gpu("AMD Radeon(TM) Graphics"), TipoDeGpu::Integrada);
        assert_eq!(tipo_de_gpu("NVIDIA GeForce GTX 1650"), TipoDeGpu::Dedicada);
        assert_eq!(tipo_de_gpu("AMD Radeon RX 6600"), TipoDeGpu::Dedicada);
        assert_eq!(tipo_de_gpu("Intel(R) Arc(TM) A770 Graphics"), TipoDeGpu::Dedicada);
        assert_eq!(tipo_de_gpu("Microsoft Basic Display Adapter"), TipoDeGpu::Desconhecida);
    }

    #[test]
    fn sem_leitura_tudo_e_pendente_e_energia_nunca_e_real() {
        let m = Metricas {
            sistema: Mutex::new(Sistema { nucleos_logicos: 4, ..Sistema::default() }),
            leituras: Mutex::new(VecDeque::new()),
            fonte: "teste",
            problema: Mutex::new(None),
        };
        let j = m.json(12, 90);
        assert_eq!(j["cpu_total"]["origem"], "PENDENTE");
        assert_eq!(j["temperatura"]["origem"], "PENDENTE");
        m.guardar(Leitura { quando_ms: agora_ms(), cpu_total: Some(40.0), cpu_processo: Some(25.0), ..Leitura::default() });
        let j = m.json(12, 90);
        assert_eq!(j["cpu_total"]["origem"], "REAL");
        assert_eq!(j["cpu_total"]["valor"], 40.0);
        assert_eq!(j["energia"]["origem"], "ESTIMADO", "energia nunca é medida");
        // 25% da máquina × 4 núcleos × 12 W = 12 W
        assert_eq!(j["energia"]["valor"], 12.0);
        assert_eq!(j["gpu_3d"]["origem"], "PENDENTE", "sem contador de GPU, nada de número");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_le_o_proc() {
        let a = linux::tempos().expect("stat");
        std::thread::sleep(Duration::from_millis(200));
        let b = linux::tempos().expect("stat");
        let (t, p) = linux::uso(a, b);
        assert!(t.is_some() && p.is_some());
        assert!(linux::memoria().0.is_some());
    }
}
