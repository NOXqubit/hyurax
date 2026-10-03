# Hyurax / Ultrax: teste de execução longa (Documento Mestre §15).
#
#   powershell -ExecutionPolicy Bypass -File scripts\teste-longo.ps1 -Programa .target\release\hyurax-no.exe -Minutos 120
#
# Roda o ULTRAX (carga LAB, conferida) sem parar numa pasta isolada e mede, a
# cada 30 s, a memória do processo (privada e conjunto de trabalho), os
# handles e as threads. No fim, para o processo, roda a auditoria do
# histórico e compara o começo com o fim:
#  - memória privada que cresce mais de 50% entre os primeiros e os últimos
#    5 minutos (depois do aquecimento) é vazamento;
#  - handles ou threads que crescem sem parar também;
#  - a auditoria precisa sair limpa (TASK_ID, assinaturas, placar, refeitas).
# As medidas ficam em CSV ao lado, para ler depois.

param(
    [Parameter(Mandatory = $true)][string]$Programa,
    [int]$Minutos = 30,
    [int]$Linhas = 2
)
$ErrorActionPreference = "Stop"
$Programa = (Resolve-Path $Programa).Path
$pasta = Join-Path ([System.IO.Path]::GetTempPath()) ("hyurax-teste-longo-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
New-Item -ItemType Directory -Force $pasta | Out-Null
$csv = Join-Path $pasta "medidas.csv"
"segundos,privada_mib,conjunto_mib,handles,threads" | Set-Content -Encoding ascii $csv

$p = Start-Process -FilePath $Programa -ArgumentList @("ultrax", "lab", "--pasta", "`"$pasta`"", "--tarefas", "0", "--linhas", "$Linhas") -PassThru -WindowStyle Hidden -RedirectStandardOutput (Join-Path $pasta "saida.txt")
Write-Host "rodando $Minutos min (processo $($p.Id), pasta $pasta)"
$inicio = Get-Date
$medidas = @()
try {
    while (((Get-Date) - $inicio).TotalMinutes -lt $Minutos) {
        Start-Sleep -Seconds 30
        $x = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
        if (-not $x) { throw "o processo parou sozinho antes do fim (veja $pasta\saida.txt)" }
        $s = [int]((Get-Date) - $inicio).TotalSeconds
        $m = [pscustomobject]@{ s = $s; privada = [math]::Round($x.PrivateMemorySize64 / 1MB, 1); conjunto = [math]::Round($x.WorkingSet64 / 1MB, 1); handles = $x.HandleCount; threads = $x.Threads.Count }
        $medidas += $m
        "$($m.s),$($m.privada),$($m.conjunto),$($m.handles),$($m.threads)" | Add-Content -Encoding ascii $csv
    }
} finally {
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
}

# começo (depois de 5 min de aquecimento) × fim (últimos 5 min)
$aquecido = $medidas | Where-Object { $_.s -ge 300 }
if ($aquecido.Count -lt 4) { $aquecido = $medidas }
$n = [math]::Max(1, [math]::Min(10, [math]::Floor($aquecido.Count / 3)))
$comeco = $aquecido | Select-Object -First $n
$fim = $aquecido | Select-Object -Last $n
$media = { param($l, $c) ($l | Measure-Object -Property $c -Average).Average }
$privadaC = & $media $comeco "privada"; $privadaF = & $media $fim "privada"
$handlesC = & $media $comeco "handles"; $handlesF = & $media $fim "handles"
$threadsC = & $media $comeco "threads"; $threadsF = & $media $fim "threads"
Write-Host ("memória privada: {0:N1} -> {1:N1} MiB" -f $privadaC, $privadaF)
Write-Host ("handles: {0:N0} -> {1:N0}   threads: {2:N0} -> {3:N0}" -f $handlesC, $handlesF, $threadsC, $threadsF)

$falhas = 0
if ($privadaF -gt $privadaC * 1.5 + 20) { Write-Host "  FALHOU  a memória privada cresceu demais"; $falhas++ }
if ($handlesF -gt $handlesC + 200) { Write-Host "  FALHOU  os handles cresceram sem parar"; $falhas++ }
if ($threadsF -gt $threadsC + 10) { Write-Host "  FALHOU  as threads cresceram sem parar"; $falhas++ }

$auditoria = & $Programa ultrax auditar --pasta $pasta --amostra 20 2>&1 | Out-String
Write-Host $auditoria
if ($LASTEXITCODE -ne 0) { Write-Host "  FALHOU  a auditoria do histórico achou problema"; $falhas++ }
Write-Host "medidas em $csv"
if ($falhas -gt 0) { exit 1 }
Write-Host "execução longa: tudo certo"
