# =============================================================================
# Auron - arquivo de paginacao para o build de Rust caber na maquina.
#
# PRECISA SER EXECUTADO COMO ADMINISTRADOR.
#
# Por que: a maquina tem 3,4 GB de RAM e sobra pouco livre. Compilar Rust pede
# de 0,5 a 1 GB por processo. Sem paginacao suficiente, o compilador e morto
# pelo Windows no meio do build, com erro que nao explica a causa.
#
# Onde: por padrao no C:. Depois da reinstalacao do Windows, o C: passou a ter
# cerca de 35 GB livres, entao cabe. Antes so tinha 5,7 GB, e era por isso que
# a versao anterior deste script mandava direto para o HD externo.
#
# Se um dia o C: apertar de novo, da para mandar para o D::
#
#     & "D:\auron\scripts\setup-pagefile.ps1" -Unidade D
#
# RISCO do HD externo, dito na cara: com paginacao ativa nele, desconectar o
# HD com o computador ligado deixa o Windows instavel ate reiniciar. Se
# precisar tirar o HD de vez, rode antes com -Remover -Unidade D.
#
# Como rodar:
#   1. Menu Iniciar, digite "powershell"
#   2. Clique com o botao direito, "Executar como administrador"
#   3. Cole:
#        & "D:\auron\scripts\setup-pagefile.ps1"
#
# A mudanca so vale depois de REINICIAR o computador.
# =============================================================================

[CmdletBinding()]
param(
    [ValidatePattern('^[A-Za-z]$')]
    [string]$Unidade   = "C",
    [int]   $InicialMB = 4096,
    [int]   $MaximoMB  = 8192,
    [switch]$Remover
)

$ErrorActionPreference = "Stop"

$U    = $Unidade.ToUpper()
$Raiz = "${U}:\"
$Arq  = "${U}:\pagefile.sys"

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

if (-not (Test-Path $Raiz)) {
    Write-Host "  ERRO: a unidade ${U}: nao esta disponivel." -ForegroundColor Red
    if ($U -ne "C") { Write-Host "  O HD externo esta conectado?" }
    exit 1
}

Write-Host ""
Write-Host "=== Paginacao - estado atual ===" -ForegroundColor Cyan
Get-CimInstance Win32_PageFileSetting |
    Select-Object Name, InitialSize, MaximumSize | Format-Table -AutoSize

$existente = Get-CimInstance Win32_PageFileSetting |
    Where-Object { $_.Name -like "${U}:*" }

if ($Remover) {
    if ($existente) {
        Remove-CimInstance -InputObject $existente
        Write-Host "  Paginacao do ${U}: removida. REINICIE para valer." -ForegroundColor Yellow
    } else {
        Write-Host "  Nao havia paginacao no ${U}:. Nada a fazer."
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

$livreGB = [math]::Round(
    (Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='${U}:'").FreeSpace / 1GB, 1)

if ($livreGB -lt ($MaximoMB / 1024 + 5)) {
    Write-Host "  ERRO: ${U}: tem apenas $livreGB GB livres, insuficiente." -ForegroundColor Red
    Write-Host "  Precisa de pelo menos $([math]::Round($MaximoMB / 1024 + 5, 1)) GB."
    exit 1
}

if ($U -ne "C") {
    Write-Host ""
    Write-Host "  AVISO: paginacao fora do disco do sistema." -ForegroundColor Yellow
    Write-Host "  Nao desconecte a unidade ${U}: com o computador ligado."
}

if ($existente) {
    Write-Host "  Ajustando paginacao existente no ${U}: para $InicialMB / $MaximoMB MB ..."
    Set-CimInstance -InputObject $existente -Property @{
        InitialSize = $InicialMB
        MaximumSize = $MaximoMB
    }
} else {
    Write-Host "  Criando $Arq com $InicialMB / $MaximoMB MB ..."
    New-CimInstance -ClassName Win32_PageFileSetting -Property @{
        Name        = $Arq
        InitialSize = $InicialMB
        MaximumSize = $MaximoMB
    } | Out-Null
}

Write-Host ""
Write-Host "=== Paginacao - depois ===" -ForegroundColor Cyan
Get-CimInstance Win32_PageFileSetting |
    Select-Object Name, InitialSize, MaximumSize | Format-Table -AutoSize

Write-Host "  Feito. REINICIE o computador para a mudanca valer." -ForegroundColor Green
Write-Host "  Confira depois com: scripts\env.ps1"
Write-Host ""