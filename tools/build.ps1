<#
.SYNOPSIS
    Build IEM-Tool-rs from source (for developers).

.DESCRIPTION
    Checks the toolchain, makes sure the measurement library is extracted, then
    runs the Tauri build (or dev server with -Dev). Requires Rust and the Tauri
    v2 prerequisites: https://tauri.app/start/prerequisites/

.PARAMETER Dev
    Run `cargo tauri dev` (hot-reload window) instead of a release build.

.PARAMETER Wasm
    Also rebuild the WebAssembly core into app-files\wasm\.

.EXAMPLE
    .\tools\build.ps1

.EXAMPLE
    .\tools\build.ps1 -Dev
#>
[CmdletBinding()]
param(
    [switch]$Dev,
    [switch]$Wasm
)

$ErrorActionPreference = 'Stop'

function Write-Step { param([string]$Message) Write-Host "==> $Message" -ForegroundColor Cyan }

$repoRoot = Split-Path -Parent $PSScriptRoot
$rustDir = Join-Path $repoRoot 'rust'
$appFiles = Join-Path $repoRoot 'app-files'

Write-Step 'Checking toolchain ...'
foreach ($tool in @('cargo', 'rustup')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "'$tool' not found. Install Rust from https://rustup.rs and re-run."
    }
}
Write-Host "    $(cargo --version)" -ForegroundColor DarkGray

if (-not (Get-Command 'cargo-tauri' -ErrorAction SilentlyContinue)) {
    Write-Step 'Installing the Tauri CLI (one-time) ...'
    cargo install tauri-cli --version "^2" --locked
}

# The app needs the measurement library expanded next to index.html.
$dataDir = Join-Path $appFiles 'data'
$dataZip = Join-Path $appFiles 'data.zip'
if ((Test-Path $dataZip) -and -not (Test-Path $dataDir)) {
    Write-Step 'Extracting the measurement library (data.zip) ...'
    Expand-Archive -Path $dataZip -DestinationPath $appFiles -Force
}

if ($Wasm) {
    Write-Step 'Rebuilding the WebAssembly core ...'
    Push-Location $rustDir
    try {
        rustup target add wasm32-unknown-unknown | Out-Null
        cargo rustc -p iem-core --release --target wasm32-unknown-unknown --crate-type cdylib
        Copy-Item (Join-Path $rustDir 'target\wasm32-unknown-unknown\release\iem_core.wasm') `
                  (Join-Path $appFiles 'wasm\iem_core.wasm') -Force
        Write-Host '    updated app-files\wasm\iem_core.wasm' -ForegroundColor DarkGray
    } finally { Pop-Location }
}

Push-Location $rustDir
try {
    if ($Dev) {
        Write-Step 'cargo tauri dev'
        cargo tauri dev
    } else {
        Write-Step 'cargo tauri build'
        cargo tauri build
        $bundle = Join-Path $rustDir 'target\release\bundle'
        Write-Step "Installers written to: $bundle"
        if (Test-Path $bundle) {
            Get-ChildItem -Path $bundle -Recurse -Include *.exe, *.msi |
                ForEach-Object { Write-Host "    $($_.FullName)" -ForegroundColor Green }
        }
    }
} finally { Pop-Location }
