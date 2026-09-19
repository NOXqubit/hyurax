# Toca um WAV do Éter no alto-falante (ou no transmissor FM ligado na saída de som).
#
# Uso:
#   & "D:\hyurax\scripts\eter-tocar.ps1" saida\eter-123.wav
#
# Sem argumento, toca o WAV mais novo da pasta "saida".
param([string]$Arquivo)

if (-not $Arquivo) {
    $mais_novo = Get-ChildItem -Path 'saida' -Filter '*.wav' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $mais_novo) { Write-Error 'Nenhum WAV em .\saida'; exit 1 }
    $Arquivo = $mais_novo.FullName
}
$caminho = (Resolve-Path $Arquivo).Path
$player = New-Object System.Media.SoundPlayer $caminho
Write-Host "Tocando $caminho ..."
$player.PlaySync()
Write-Host 'Pronto.'
