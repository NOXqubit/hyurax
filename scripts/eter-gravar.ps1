# Grava o microfone (ou a entrada de linha, vinda de um rádio FM) num WAV,
# para o "eter receber --som" ler. Usa o gravador que já vem no Windows (MCI),
# sem instalar nada.
#
# Uso:
#   & "D:\hyurax\scripts\eter-gravar.ps1" -Segundos 30 -Saida gravacoes\ar.wav
#
# Teste de eco (o mesmo computador toca e ouve, pelo ar):
#   & "D:\hyurax\scripts\eter-gravar.ps1" -Tocar saida\eter-123.wav -Saida gravacoes\eco.wav
param(
    [int]$Segundos = 20,
    [string]$Saida = 'gravacoes\gravacao.wav',
    [string]$Tocar
)

Add-Type -Namespace Eter -Name Mci -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("winmm.dll", CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern int mciSendString(string comando, System.Text.StringBuilder resposta, int tamanho, System.IntPtr janela);
'@

function Mci([string]$comando) {
    $resposta = New-Object System.Text.StringBuilder 256
    $codigo = [Eter.Mci]::mciSendString($comando, $resposta, 256, [IntPtr]::Zero)
    if ($codigo -ne 0) { throw "O gravador do Windows recusou '$comando' (código $codigo)" }
}

$pasta = Split-Path -Parent $Saida
if (-not $pasta) { $pasta = '.' }
New-Item -ItemType Directory -Force -Path $pasta | Out-Null
$destino = Join-Path (Resolve-Path $pasta).Path (Split-Path -Leaf $Saida)
# O MCI não aceita caminho comprido: grava num nome curto e depois move.
# Mover no fim também garante que o "eter receber" só vê o arquivo completo.
$curto = Join-Path $env:TEMP 'eter-grav.wav'

Mci 'open new type waveaudio alias eter'
try {
    # Muitos microfones de notebook só gravam em estéreo; o Éter lê o primeiro canal.
    Mci 'set eter bitspersample 16 channels 2 samplespersec 44100'
    Mci 'record eter'
    if ($Tocar) {
        Write-Host "Gravando e tocando $Tocar ..."
        Start-Sleep -Milliseconds 500
        (New-Object System.Media.SoundPlayer (Resolve-Path $Tocar).Path).PlaySync()
        Start-Sleep -Milliseconds 700
    } else {
        Write-Host "Gravando $Segundos s ..."
        Start-Sleep -Seconds $Segundos
    }
    Mci 'stop eter'
    Mci "save eter `"$curto`""
} finally {
    Mci 'close eter'
}
Move-Item -Force $curto $destino
Write-Host "Gravado: $destino"
