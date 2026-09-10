# =============================================================================
# Auron — prepara o ambiente do terminal.
#
# As variáveis já estão gravadas no perfil do usuário, então um terminal novo
# já vem pronto. Este script existe para dois casos:
#
#   1. Terminal que já estava aberto antes da instalação e não pegou as
#      variáveis novas.
#   2. Conferir rapidamente se tudo está no lugar.
#
# Uso:
#   . "D:\auron\scripts\env.ps1"
#
# O ponto e o espaço no começo importam: eles fazem o script rodar na sua
# sessão, e não numa sessão filha que morre no fim.
# =============================================================================

$AURON_ROOT = "D:\auron"
$TOOLCHAIN  = "$AURON_ROOT\.toolchain"

$env:RUSTUP_HOME = "$TOOLCHAIN\rustup"
$env:CARGO_HOME  = "$TOOLCHAIN\cargo"

# CARGO_TARGET_DIR de propósito NÃO é definido aqui. O .cargo/config.toml do
# projeto define `target-dir = ".target"`, que é relativo e portátil entre as
# três máquinas. Variável de ambiente sobrescreveria isso e criaria duas
# fontes de verdade.
Remove-Item Env:\CARGO_TARGET_DIR -ErrorAction SilentlyContinue

# Cache do pip tambem no HD externo. Nada do projeto escreve no C:.
$env:PIP_CACHE_DIR = "$TOOLCHAIN\pip-cache"

foreach ($p in @("$TOOLCHAIN\cargo\bin", "$TOOLCHAIN\git\cmd",
                 "$TOOLCHAIN\python", "$TOOLCHAIN\python\Scripts")) {
    if ($env:PATH -notlike "*$p*") { $env:PATH = "$p;$env:PATH" }
}

Write-Host ""
Write-Host "=== Ambiente Auron ===" -ForegroundColor Cyan

function Mostrar($nome, $comando) {
    $exe = Get-Command $comando -ErrorAction SilentlyContinue
    if ($exe) {
        # O rustup escreve uma nota informativa no stderr, o que o PowerShell
        # trata como erro. Engolir aqui evita alarme falso.
        $v = $null
        try { $v = (& $comando --version 2>$null | Select-Object -First 1) } catch { }
        if (-not $v) { $v = "(presente)" }
        Write-Host ("  {0,-8} {1}" -f $nome, $v) -ForegroundColor Green
    } else {
        Write-Host ("  {0,-8} AUSENTE" -f $nome) -ForegroundColor Red
    }
}

Mostrar "rustc"  "rustc"
Mostrar "cargo"  "cargo"
Mostrar "rustup" "rustup"
Mostrar "git"    "git"
Mostrar "python" "python"

Write-Host ""
Write-Host "  RUSTUP_HOME  $env:RUSTUP_HOME"
Write-Host "  CARGO_HOME   $env:CARGO_HOME"
Write-Host "  projeto      $AURON_ROOT"

$livre = [math]::Round((Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='D:'").FreeSpace / 1GB, 1)
$os = Get-CimInstance Win32_OperatingSystem
$ramLivre = [math]::Round($os.FreePhysicalMemory / 1MB, 1)
$vmTotal = [math]::Round($os.TotalVirtualMemorySize / 1MB, 1)

Write-Host ""
Write-Host ("  D: livre     {0} GB" -f $livre)
Write-Host ("  RAM livre    {0} GB de 3,4 GB" -f $ramLivre)
Write-Host ("  memoria virtual total  {0} GB" -f $vmTotal)

if ($vmTotal -lt 10) {
    Write-Host ""
    Write-Host "  AVISO: memoria virtual baixa para compilar Rust." -ForegroundColor Yellow
    Write-Host "  Rode como administrador: scripts\setup-pagefile.ps1"
}

Write-Host ""

# Script dot-sourced: nao usar `exit`, que fecharia a sessao do usuario.
# Zerar o codigo evita que a nota do rustup pareca falha.
$global:LASTEXITCODE = 0
