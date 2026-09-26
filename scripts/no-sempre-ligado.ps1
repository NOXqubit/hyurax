# Deixa um nó do Hyurax ligado neste Windows, para ele servir de semente da
# rede de teste.
#
# O que faz:
#   1. acha o hyurax-no.exe;
#   2. cria uma tarefa que sobe o nó toda vez que você entra no Windows;
#   3. abre a porta no Firewall do Windows (se estiver como administrador);
#   4. mostra o seu endereço de fora e o que falta abrir no roteador.
#
# Uso:
#   powershell -ExecutionPolicy Bypass -File scripts\no-sempre-ligado.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\no-sempre-ligado.ps1 -Remover
#
# Nada aqui mexe em carteira: um nó semente não minera e não guarda dinheiro.

param(
    [string]$Programa,
    [string]$Pasta = "$env:APPDATA\Hyurax\semente",
    [int]$Porta = 8790,
    [switch]$Remover,
    [switch]$SemIpExterno
)

$ErrorActionPreference = 'Stop'
$NomeDaTarefa = 'Hyurax semente'

function Test-Administrador {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    (New-Object Security.Principal.WindowsPrincipal $id).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}

if ($Remover) {
    try { Unregister-ScheduledTask -TaskName $NomeDaTarefa -Confirm:$false; Write-Host 'Tarefa removida.' }
    catch { Write-Host 'Não havia tarefa para remover.' }
    if (Test-Administrador) {
        try { Remove-NetFirewallRule -DisplayName $NomeDaTarefa; Write-Host 'Regra do Firewall removida.' }
        catch { Write-Host 'Não havia regra no Firewall.' }
    } else {
        Write-Host 'Sem ser administrador: a regra do Firewall, se existir, continua lá.'
    }
    exit 0
}

# ---- 1. achar o programa ----
if (-not $Programa) {
    $candidatos = @(
        "$PSScriptRoot\..\.target\release\hyurax-no.exe",
        "$PSScriptRoot\..\.target\debug\hyurax-no.exe",
        "$PSScriptRoot\hyurax-no.exe",
        "$env:USERPROFILE\Downloads\hyurax-no.exe"
    )
    $Programa = $candidatos | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $Programa -or -not (Test-Path $Programa)) {
    Write-Error @'
Não achei o hyurax-no.exe. Baixe o pacote de terminal da página de lançamentos
do projeto, descompacte, e rode de novo apontando o caminho:

  powershell -ExecutionPolicy Bypass -File scripts\no-sempre-ligado.ps1 -Programa C:\caminho\hyurax-no.exe
'@
    exit 1
}
$Programa = (Resolve-Path $Programa).Path
if (-not (Test-Path $Pasta)) { New-Item -ItemType Directory -Path $Pasta | Out-Null }

Write-Host "Programa: $Programa"
Write-Host "Pasta de dados: $Pasta"
Write-Host "Porta: $Porta"
Write-Host ''

# ---- 2. tarefa que sobe o nó no logon ----
$argumentos = "no --rede testnet --pasta `"$Pasta`" --porta $Porta"
$acao = New-ScheduledTaskAction -Execute $Programa -Argument $argumentos -WorkingDirectory (Split-Path $Programa)
$gatilho = New-ScheduledTaskTrigger -AtLogOn
# Sem limite de tempo e sem parar por causa de bateria: um semente serve para ficar.
$opcoes = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1)
try { Unregister-ScheduledTask -TaskName $NomeDaTarefa -Confirm:$false -ErrorAction SilentlyContinue } catch {}
Register-ScheduledTask -TaskName $NomeDaTarefa -Action $acao -Trigger $gatilho -Settings $opcoes -Description 'Nó semente da rede de teste do Hyurax' | Out-Null
Start-ScheduledTask -TaskName $NomeDaTarefa
Write-Host "Tarefa `"$NomeDaTarefa`" criada e iniciada. Ela volta sozinha a cada logon."

# ---- 3. Firewall do Windows ----
if (Test-Administrador) {
    try { Remove-NetFirewallRule -DisplayName $NomeDaTarefa -ErrorAction SilentlyContinue } catch {}
    New-NetFirewallRule -DisplayName $NomeDaTarefa -Direction Inbound -Action Allow -Protocol TCP -LocalPort $Porta -Profile Any | Out-Null
    Write-Host "Firewall do Windows: entrada liberada na porta $Porta."
} else {
    Write-Host "AVISO: sem ser administrador, não dá para abrir a porta $Porta no Firewall."
    Write-Host '       Rode este script como administrador, ou libere na mão em'
    Write-Host '       "Firewall do Windows Defender > Regras de Entrada".'
}

# ---- 4. endereços ----
$local = (Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Where-Object { $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*' } |
    Select-Object -First 1).IPAddress
Write-Host ''
Write-Host "Endereço na sua rede local: ${local}:$Porta"

if (-not $SemIpExterno) {
    # Uma pergunta só, a um serviço público, para descobrir como a internet te
    # vê. Nada do nó vai junto. Com -SemIpExterno, nem isso sai daqui.
    try {
        $externo = (curl.exe -fsS --max-time 10 https://api.ipify.org)
        Write-Host "Endereço de fora (visto pela internet): ${externo}:$Porta"
    } catch {
        Write-Host 'Não consegui descobrir o endereço de fora (sem internet?).'
    }
}

Write-Host ''
Write-Host 'Falta o roteador: redirecione a porta TCP' $Porta 'para' $local
Write-Host '(procure por "Port Forwarding", "Encaminhamento de portas" ou "Virtual Server").'
Write-Host ''
Write-Host 'Depois, teste DE FORA — pelo 4G do celular, por exemplo:'
Write-Host "  hyurax-no no --rede testnet --pasta teste --semente SEU_IP_DE_FORA:$Porta"
Write-Host ''
Write-Host 'Se aparecer "1 par(es)", o seu nó está no ar: mande o endereço para entrar'
Write-Host 'em rede/sementes-testnet.txt. Veja docs/LANCAR-A-REDE.md.'
