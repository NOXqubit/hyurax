# Gera a narração do vídeo como arquivos de áudio, com a voz do Windows.
#
# Por que arquivo, e não a voz do navegador: a voz do navegador muda de aparelho
# para aparelho, some quando o idioma não está instalado, e NÃO entra na
# gravação do vídeo. Um arquivo toca igual para todo mundo, entra na gravação e
# ainda dá o nível do som para mexer a boca do modelo 3D.
#
# Rodar da raiz do repositório:  python site/tools/build_narracao.py
#
# Saída: site/assets/voz/pt-BR/01.wav .. 10.wav  (fora do Git; é gerado)
import json
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
LOCALE = RAIZ / "site" / "locales" / "pt-BR.js"
SAIDA = RAIZ / "site" / "assets" / "voz" / "pt-BR"
VOZ = "Maria"   # Microsoft Maria, pt-BR, feminina

POWERSHELL = r"""
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
    $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and
    $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Esperar($op, $tipo) {
    $metodo = $asTask.MakeGenericMethod($tipo)
    $tarefa = $metodo.Invoke($null, @($op))
    $tarefa.Wait(-1) | Out-Null
    $tarefa.Result
}
[Windows.Media.SpeechSynthesis.SpeechSynthesizer, Windows.Media, ContentType = WindowsRuntime] | Out-Null
[Windows.Storage.Streams.DataReader, Windows.Storage, ContentType = WindowsRuntime] | Out-Null

$falas = Get-Content -Raw -Encoding UTF8 'ENTRADA' | ConvertFrom-Json
$sintetizador = New-Object Windows.Media.SpeechSynthesis.SpeechSynthesizer
$voz = [Windows.Media.SpeechSynthesis.SpeechSynthesizer]::AllVoices |
    Where-Object { $_.DisplayName -like '*VOZ*' } | Select-Object -First 1
if (-not $voz) { throw 'voz nao encontrada' }
$sintetizador.Voice = $voz
# Um pouco mais devagar: é explicação, não propaganda.
$sintetizador.Options.SpeakingRate = 0.92
$sintetizador.Options.AudioVolume = 1.0

$n = 0
foreach ($fala in $falas) {
    $n++
    $fluxo = Esperar $sintetizador.SynthesizeTextToStreamAsync($fala) ([Windows.Media.SpeechSynthesis.SpeechSynthesisStream])
    $leitor = New-Object Windows.Storage.Streams.DataReader($fluxo.GetInputStreamAt(0))
    Esperar $leitor.LoadAsync([uint32]$fluxo.Size) ([uint32]) | Out-Null
    $bytes = New-Object byte[] $fluxo.Size
    $leitor.ReadBytes($bytes)
    $leitor.Dispose()
    $arquivo = Join-Path 'SAIDA' ("{0:d2}.wav" -f $n)
    [IO.File]::WriteAllBytes($arquivo, $bytes)
    "{0:d2}.wav  {1} bytes" -f $n, $bytes.Length
}
$sintetizador.Dispose()
"""


def falas_do_locale() -> list[str]:
    texto = LOCALE.read_text(encoding="utf-8")
    bloco = re.search(r"  edit: \{.*?\n  \},\n", texto, re.S)
    if not bloco:
        sys.exit("não achei o bloco edit em pt-BR.js")
    return re.findall(r'fala: "((?:[^"\\]|\\.)*)"', bloco.group(0))


def main() -> None:
    falas = [f.replace('\\"', '"') for f in falas_do_locale()]
    if not falas:
        sys.exit("nenhuma fala encontrada")
    SAIDA.mkdir(parents=True, exist_ok=True)
    entrada = SAIDA / "falas.json"
    entrada.write_text(json.dumps(falas, ensure_ascii=False), encoding="utf-8")

    script = (POWERSHELL
              .replace("ENTRADA", str(entrada).replace("\\", "\\\\"))
              .replace("SAIDA", str(SAIDA).replace("\\", "\\\\"))
              .replace("VOZ", VOZ))
    caminho = SAIDA / "_gerar.ps1"
    caminho.write_text(script, encoding="utf-8-sig")
    r = subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(caminho)],
                       capture_output=True, text=True)
    print(r.stdout.strip())
    if r.returncode != 0:
        print(r.stderr.strip(), file=sys.stderr)
        sys.exit(r.returncode)
    caminho.unlink()
    entrada.unlink()

    total = sum(p.stat().st_size for p in SAIDA.glob("*.wav"))
    print(f"{len(falas)} falas · {total / 1048576:.1f} MB em {SAIDA}")


if __name__ == "__main__":
    main()
