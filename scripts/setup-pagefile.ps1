# =============================================================================
# Auron — cria arquivo de paginação no HD externo (D:)
#
# PRECISA SER EXECUTADO COMO ADMINISTRADOR.
#
# Por que: a máquina tem 3,4 GB de RAM e sobra pouco livre. Compilar Rust pede
# de 0,5 a 1 GB por processo. Sem paginação suficiente, o compilador é morto
# pelo Windows no meio do build, com erro que não explica a causa.
#
# O C: já tem 2,6 GB de paginação e só 5,7 GB livres, então não cabe crescer
# lá. O D: tem 298 GB livres.
#
# RISCO, dito na cara: com paginação ativa no HD externo, desconectar o HD com
# o computador ligado deixa o Windows instável até reiniciar. Não desconecte
# com a máquina ligada. Se precisar tirar o HD de vez, rode este script com
# -Remover antes.
#
# Como rodar:
#   1. Menu Iniciar, digite "powershell"
#   2. Clique com o botão direito, "Executar como administrador"
#   3. Cole:
#        & "D:\Nova pasta\auron\scripts\setup-pagefile.ps1"
#
# A mudança só vale depois de REINICIAR o computador.
# =============================================================================

[CmdletBinding()]
param(
    [int]$InicialMB = 4096,
    [int]$MaximoMB  = 8192,
    [switch]$Remover
)

$ErrorActionPreference = "Stop"

$admin = ([Security.Principal.WindowsPrincipal] `
    [Security.Principal.WindowsIdentity]::GetCurrent()
).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $admin) {
    Write-Host ""
    Write-Host "  ERRO: este script precisa de privilegio de administrador." -ForegroundColor Red
    Write-Host "  Abra o PowerShell com 'Executar como administrador' e rode de novo."
    Write-Host ""
    exit 1
}

if (-not (Test-Path "D:\")) {
    Write-Host "  ERRO: a unidade D: nao esta disponivel. O HD externo esta conectado?" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "=== Paginacao — estado atual ===" -ForegroundColor Cyan
Get-CimInstance Win32_PageFileSetting |
    Select-Object Name, InitialSize, MaximumSize | Format-Table -AutoSize

$existente = Get-CimInstance Win32_PageFileSetting |
    Where-Object { $_.Name -like "D:*" }

if ($Remover) {
    if ($existente) {
        Remove-CimInstance -InputObject $existente
        Write-Host "  Paginacao do D: removida. REINICIE para valer." -ForegroundColor Yellow
    } else {
        Write-Host "  Nao havia paginacao no D:. Nada a fazer."
    }
    exit 0
}

# A gestao automatica precisa estar desligada, senao o Windows ignora a
# configuracao manual por unidade.
$cs = Get-CimInstance Win32_ComputerSystem
if ($cs.AutomaticManagedPagefile) {
    Write-Host "  Desligando gestao automatica de paginacao ..."
    Set-CimInstance -InputObject $cs -Property @{ AutomaticManagedPagefile = $false }
}

$livreGB = [math]::Round((Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='D:'").FreeSpace / 1GB, 1)
if ($livreGB -lt ($MaximoMB / 1024 + 5)) {
    Write-Host "  ERRO: D: tem apenas $livreGB GB livres, insuficiente." -ForegroundColor Red
    exit 1
}

if ($existente) {
    Write-Host "  Ajustando paginacao existente no D: para $InicialMB / $MaximoMB MB ..."
    Set-CimInstance -InputObject $existente -Property @{
        InitialSize = $InicialMB
        MaximumSize = $MaximoMB
    }
} else {
    Write-Host "  Criando D:\pagefile.sys com $InicialMB / $MaximoMB MB ..."
    New-CimInstance -ClassName Win32_PageFileSetting -Property @{
        Name        = "D:\pagefile.sys"
        InitialSize = $InicialMB
        MaximumSize = $MaximoMB
    } | Out-Null
}

Write-Host ""
Write-Host "=== Paginacao — depois ===" -ForegroundColor Cyan
Get-CimInstance Win32_PageFileSetting |
    Select-Object Name, InitialSize, MaximumSize | Format-Table -AutoSize

Write-Host "  Feito. REINICIE o computador para a mudanca valer." -ForegroundColor Green
Write-Host "  Depois de reiniciar, a memoria virtual total sai de 6,1 GB para cerca de 14 GB."
Write-Host ""
