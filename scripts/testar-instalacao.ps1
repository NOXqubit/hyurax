# Hyurax / Ultrax: teste da instalação limpa, de ponta a ponta.
#
#   powershell -ExecutionPolicy Bypass -File scripts\testar-instalacao.ps1 -Instalador dist\hyurax-instalador-windows-x86_64.exe
#
# Tudo acontece numa pasta isolada: LOCALAPPDATA, APPDATA e USERPROFILE do
# processo apontam para dentro dela, e a chave de "Aplicativos" é uma chave de
# teste (--chave-de-teste). A instalação de verdade do dono não é tocada.
#
# Confere:
#  1. o instalador sem o aceite dos termos se recusa (saída 2);
#  2. instala: arquivos, manifesto, atalhos, chave no registro, termos aceitos;
#  3. instala de novo por cima (atualização no lugar);
#  4. o programa instalado abre sozinho, fora da árvore de desenvolvimento,
#     e responde na API local com a versão certa e a rede de teste;
#  5. desinstala: some o programa, os atalhos e a chave; a carteira e os dados
#     do usuário continuam lá.

param(
    [Parameter(Mandatory = $true)][string]$Instalador,
    [int]$Porta = 8800
)
$ErrorActionPreference = "Stop"
$Instalador = (Resolve-Path $Instalador).Path

$raiz = Join-Path ([System.IO.Path]::GetTempPath()) ("hyurax-teste-instalacao-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
$local = Join-Path $raiz "Local"
$roaming = Join-Path $raiz "Roaming"
$perfil = Join-Path $raiz "Perfil"
foreach ($p in @($local, $roaming, (Join-Path $perfil "Desktop"))) { New-Item -ItemType Directory -Force $p | Out-Null }
$chaveNome = "ci" + [guid]::NewGuid().ToString("N").Substring(0, 6)
$chave = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Hyurax-teste-$chaveNome"
$programa = Join-Path $local "Programs\Hyurax"
$falhas = 0

function Conferir([bool]$ok, [string]$o) {
    if ($ok) { Write-Host "  ok  $o" } else { Write-Host "  FALHOU  $o"; $script:falhas++ }
}

function Rodar([string]$exe, [string[]]$argumentos) {
    $i = New-Object System.Diagnostics.ProcessStartInfo $exe
    $i.Arguments = ($argumentos -join " ")
    $i.UseShellExecute = $false
    $i.EnvironmentVariables["LOCALAPPDATA"] = $local
    $i.EnvironmentVariables["APPDATA"] = $roaming
    $i.EnvironmentVariables["USERPROFILE"] = $perfil
    return [System.Diagnostics.Process]::Start($i)
}

Write-Host "pasta isolada: $raiz"
try {
    Write-Host "1. sem aceitar os termos"
    $p = Rodar $Instalador @("--silencioso", "--chave-de-teste", $chaveNome)
    $p.WaitForExit()
    Conferir ($p.ExitCode -eq 2) "o instalador recusa sem --aceito-os-termos (saída $($p.ExitCode))"
    Conferir (-not (Test-Path $programa)) "nada foi instalado"

    Write-Host "2. instalação"
    $p = Rodar $Instalador @("--silencioso", "--aceito-os-termos", "--area-de-trabalho", "--chave-de-teste", $chaveNome)
    $p.WaitForExit()
    Conferir ($p.ExitCode -eq 0) "instalou (saída $($p.ExitCode))"
    foreach ($a in @("Hyurax.exe", "WebView2Loader.dll", "TERMOS-DE-USO.txt", "LEIA-ME.txt", "instalacao.txt")) {
        Conferir (Test-Path (Join-Path $programa $a)) "$a na pasta do programa"
    }
    $manifesto = Get-Content -Raw (Join-Path $programa "instalacao.txt")
    Conferir ($manifesto -match "versao=\d+\.\d+\.\d+") "manifesto com a versão"
    Conferir ($manifesto -match "rede=testnet") "manifesto diz rede de teste"
    $menu = Join-Path $roaming "Microsoft\Windows\Start Menu\Programs\Hyurax.lnk"
    $mesa = Join-Path $perfil "Desktop\Hyurax.lnk"
    Conferir (Test-Path $menu) "atalho no Menu Iniciar"
    Conferir (Test-Path $mesa) "atalho na Área de Trabalho"
    $reg = Get-ItemProperty -Path $chave -ErrorAction SilentlyContinue
    Conferir ($null -ne $reg -and $reg.DisplayName -eq "Hyurax / Ultrax") "registrado em Aplicativos (chave de teste)"
    Conferir ((Get-ChildItem -Path (Join-Path $roaming "Hyurax") -Filter "termos*" -ErrorAction SilentlyContinue).Count -ge 1) "termos aceitos gravados na configuração do usuário"

    Write-Host "3. instalar por cima"
    $p = Rodar $Instalador @("--silencioso", "--aceito-os-termos", "--chave-de-teste", $chaveNome)
    $p.WaitForExit()
    Conferir ($p.ExitCode -eq 0) "reinstalou por cima (saída $($p.ExitCode))"

    Write-Host "4. o programa instalado abre e responde"
    $ocupada = $null
    try { $ocupada = Invoke-RestMethod -TimeoutSec 2 "http://127.0.0.1:$Porta/api/v1/resumo" } catch {}
    if ($ocupada) {
        Write-Host "  AVISO  a porta $Porta já tem um Hyurax aberto (o do dono?): pulei a abertura para não confundir os dois"
    } else {
        $app = Rodar (Join-Path $programa "Hyurax.exe") @()
        $resumo = $null
        for ($k = 0; $k -lt 90 -and -not $resumo; $k++) {
            Start-Sleep -Seconds 2
            try { $resumo = Invoke-RestMethod -TimeoutSec 3 "http://127.0.0.1:$Porta/api/v1/resumo" } catch {}
        }
        Conferir ($null -ne $resumo) "a API local respondeu"
        if ($resumo) {
            Conferir ($resumo.produto -eq "Hyurax / Ultrax") "produto Hyurax / Ultrax"
            Conferir ($resumo.versao -match "^\d+\.\d+\.\d+$") "versão $($resumo.versao)"
            Conferir ($resumo.rede -eq "hyurax-testnet" -or $resumo.rede -like "*test*") "rede de teste ($($resumo.rede))"
            try {
                $e = Invoke-RestMethod -TimeoutSec 5 "http://127.0.0.1:$Porta/api/v1/estado"
                Conferir ($e.rede.tipo -eq "TESTNET") "estado diz TESTNET"
                Conferir ($e.ajustes.pasta_dados -like "$local*") "dados na pasta isolada do usuário"
                Conferir ($e.ajustes.pasta_config -like "$roaming*") "configuração na pasta isolada do usuário"
            } catch { Conferir $false "leu o estado: $_" }
            $pagina = $null
            try { $pagina = Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 "http://127.0.0.1:$Porta/" } catch {}
            Conferir ($pagina -and $pagina.Content -match "HYURAX") "a interface embutida abre"
        }
        if (-not $app.HasExited) { Stop-Process -Id $app.Id -Force }
        Start-Sleep -Seconds 2
    }

    Write-Host "5. desinstalar"
    $carteira = Join-Path $roaming "Hyurax\carteira.txt"
    if (-not (Test-Path $carteira)) { "carteira de teste" | Set-Content $carteira }
    $p = Rodar (Join-Path $programa "Hyurax.exe") @("--desinstalar", "--silencioso")
    $p.WaitForExit()
    for ($k = 0; $k -lt 20 -and (Test-Path $programa); $k++) { Start-Sleep -Seconds 1 }
    Conferir (-not (Test-Path $programa)) "a pasta do programa saiu"
    Conferir (-not (Test-Path $menu)) "atalho do Menu Iniciar saiu"
    Conferir (-not (Test-Path $mesa)) "atalho da Área de Trabalho saiu"
    Conferir (-not (Test-Path $chave)) "a chave de Aplicativos saiu"
    Conferir (Test-Path $carteira) "a carteira continua lá"
    Conferir (Test-Path (Join-Path $local "Hyurax")) "os dados do usuário continuam lá"
} finally {
    if (Test-Path $chave) { Remove-Item -Path $chave -Recurse -Force -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $raiz -Recurse -Force -ErrorAction SilentlyContinue
}

if ($falhas -gt 0) { Write-Host "$falhas conferência(s) falharam"; exit 1 }
Write-Host "instalação limpa: tudo certo"
