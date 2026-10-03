# Hyurax / Ultrax — monta o programa e o instalador para Windows.
#
# O mesmo que o GitHub Actions faz (.github/workflows/lancamento.yml), para
# rodar numa máquina com Windows:
#
#   powershell -ExecutionPolicy Bypass -File scripts\empacotar-windows.ps1 [-Alvo x86_64-pc-windows-gnu]
#
# O alvo padrão é o do lançamento (MSVC). Numa máquina só com o Rust GNU,
# passe -Alvo x86_64-pc-windows-gnu.
#
# Saída em dist\: Hyurax\ (o programa solto), hyurax-windows-x86_64.zip,
# hyurax-instalador-windows-x86_64.exe e a soma SHA-256 de cada um.
#
# Reprodutível: `--locked` (as versões do Cargo.lock), caminhos do disco
# trocados por nomes fixos (--remap-path-prefix) e o ligador da Microsoft sem
# carimbo de hora (/Brepro). Duas montagens do mesmo commit, com o mesmo Rust,
# dão o mesmo .exe.

param([string]$Alvo = "x86_64-pc-windows-msvc")
$ErrorActionPreference = "Stop"
$raiz = Split-Path -Parent $PSScriptRoot
Set-Location $raiz

$alvo = $Alvo
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$fixos = "--remap-path-prefix=$raiz=hyurax --remap-path-prefix=$cargoHome=cargo"
if ($alvo -like "*-msvc") {
    # crt-static: o .exe leva a biblioteca de C da Microsoft dentro (sem VCRUNTIME140.dll)
    $env:RUSTFLAGS = "-C target-feature=+crt-static -C link-arg=/Brepro $fixos"
} else {
    $env:RUSTFLAGS = "-C link-arg=-Wl,--no-insert-timestamp $fixos"
}

$versao = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Hyurax / Ultrax $versao — montando para $alvo"

# o destino dos binários segue o .cargo/config.toml (target-dir)
$destinoCargo = Join-Path $raiz ".target\$alvo\release"

cargo build --release --locked --target $alvo -p hyurax-app --bin hyurax
if ($LASTEXITCODE -ne 0) { throw "falhou a montagem do programa" }
cargo build --release --locked --target $alvo -p hyurax-no
if ($LASTEXITCODE -ne 0) { throw "falhou a montagem do hyurax-no" }

$dist = Join-Path $raiz "dist"
$pasta = Join-Path $dist "Hyurax"
if (Test-Path $dist) { Remove-Item -LiteralPath $dist -Recurse -Force }
New-Item -ItemType Directory -Force $pasta | Out-Null
Copy-Item (Join-Path $destinoCargo "hyurax.exe") (Join-Path $pasta "Hyurax.exe")
# a ponte para o WebView2 (redistribuível da Microsoft, vem no webview2-com-sys)
$dll = Get-ChildItem -Path (Join-Path $destinoCargo "build") -Recurse -Filter WebView2Loader.dll | Where-Object { $_.FullName -match '\\x64\\' } | Select-Object -First 1
if (-not $dll) { throw "não achei a WebView2Loader.dll de 64 bits" }
Copy-Item $dll.FullName (Join-Path $pasta "WebView2Loader.dll")
Copy-Item LICENSE-MIT, LICENSE-APACHE $pasta
# LEIA-ME e termos para o Bloco de Notas: UTF-8 com marca e fim de linha do Windows
$utf8 = New-Object System.Text.UTF8Encoding $true
foreach ($par in @(@("crates\hyurax-instalador\LEIA-ME.txt", "LEIA-ME.txt"), @("docs\TERMOS-DE-USO.md", "TERMOS-DE-USO.txt"))) {
    $texto = (Get-Content -Raw -Encoding UTF8 $par[0]) -replace "`r?`n", "`r`n"
    [System.IO.File]::WriteAllText((Join-Path $pasta $par[1]), $texto, $utf8)
}
Copy-Item (Join-Path $destinoCargo "hyurax-no.exe") $dist

$zip = Join-Path $dist "hyurax-windows-x86_64.zip"
Compress-Archive -Path $pasta -DestinationPath $zip

# o instalador leva dentro o mesmo Hyurax.exe e a mesma DLL do zip
$env:HYURAX_CARGA_EXE = Join-Path $pasta "Hyurax.exe"
$env:HYURAX_CARGA_DLL = Join-Path $pasta "WebView2Loader.dll"
cargo build --release --locked --target $alvo -p hyurax-instalador
if ($LASTEXITCODE -ne 0) { throw "falhou a montagem do instalador" }
$instalador = Join-Path $dist "hyurax-instalador-windows-x86_64.exe"
Copy-Item (Join-Path $destinoCargo "hyurax-instalador.exe") $instalador
if ($alvo -notlike "*-msvc") {
    # Com o alvo GNU, o webview2-com-sys liga a WebView2Loader.dll em vez do
    # carregador estático (que só existe para MSVC): este instalador precisa
    # da DLL ao lado dele. Serve para testar aqui; o lançamento usa MSVC, que
    # dá um instalador de um arquivo só.
    Copy-Item (Join-Path $pasta "WebView2Loader.dll") $dist
    Write-Host "AVISO: alvo $alvo - o instalador precisa da WebView2Loader.dll ao lado (o de MSVC não precisa)"
}

foreach ($a in @($zip, $instalador, (Join-Path $dist "hyurax-no.exe"))) {
    $h = (Get-FileHash -Algorithm SHA256 $a).Hash.ToLower()
    "$h  $(Split-Path -Leaf $a)" | Set-Content -Encoding ascii "$a.sha256"
    Write-Host "$h  $(Split-Path -Leaf $a)"
}
Write-Host "Pronto: $dist"
