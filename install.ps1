<#
.SYNOPSIS
    Optional installer for IEM-Tool-rs.

.DESCRIPTION
    Downloads the latest Windows installer from the GitHub Releases page,
    verifies its SHA-256 against the release's SHA256SUMS.txt, and runs it.

    This is a convenience wrapper only - you can always just download the
    installer manually from the Releases page. The checksum verification means
    this script is arguably *safer* than a plain browser download.

.PARAMETER Repo
    owner/name of the GitHub repository. Defaults to Londopy/IEM-Tool-rs.

.PARAMETER Tag
    Release tag to install (e.g. v1.0.0). Defaults to the latest release.

.PARAMETER Arch
    x64 or x86. Defaults to auto-detecting your OS architecture.

.PARAMETER DownloadOnly
    Download and verify, but don't launch the installer.

.EXAMPLE
    .\install.ps1

.EXAMPLE
    irm https://raw.githubusercontent.com/Londopy/IEM-Tool-rs/main/install.ps1 | iex
#>
[CmdletBinding()]
param(
    [string]$Repo = 'Londopy/IEM-Tool-rs',
    [string]$Tag,
    [ValidateSet('auto', 'x64', 'x86')]
    [string]$Arch = 'auto',
    [switch]$DownloadOnly
)

$ErrorActionPreference = 'Stop'

function Write-Step { param([string]$Message) Write-Host "==> $Message" -ForegroundColor Cyan }

# Ensure TLS 1.2 on older Windows PowerShell versions.
try {
    [Net.ServicePointManager]::SecurityProtocol =
        [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
} catch { }

if ($Arch -eq 'auto') {
    $Arch = if ([Environment]::Is64BitOperatingSystem) { 'x64' } else { 'x86' }
}
Write-Step "Target architecture: $Arch"

$headers = @{
    'User-Agent' = 'IEM-Tool-rs-installer'
    'Accept'     = 'application/vnd.github+json'
}

$apiUrl = if ($Tag) {
    "https://api.github.com/repos/$Repo/releases/tags/$Tag"
} else {
    "https://api.github.com/repos/$Repo/releases/latest"
}

Write-Step "Looking up release from $Repo ..."
$release = Invoke-RestMethod -Uri $apiUrl -Headers $headers
Write-Step "Found release: $($release.tag_name)"

# Prefer the NSIS setup installer, fall back to the MSI.
$installer = $release.assets |
    Where-Object { $_.name -like "*$Arch*setup.exe" } |
    Select-Object -First 1
if (-not $installer) {
    $installer = $release.assets |
        Where-Object { $_.name -like "*$Arch*.msi" } |
        Select-Object -First 1
}
if (-not $installer) {
    throw "No $Arch Windows installer found in release $($release.tag_name)."
}

$sumsAsset = $release.assets |
    Where-Object { $_.name -eq 'SHA256SUMS.txt' } |
    Select-Object -First 1

$workDir = Join-Path $env:TEMP 'iem-tool-rs'
New-Item -ItemType Directory -Force -Path $workDir | Out-Null
$target = Join-Path $workDir $installer.name

Write-Step "Downloading $($installer.name) ..."
Invoke-WebRequest -Uri $installer.browser_download_url -OutFile $target -Headers $headers

if ($sumsAsset) {
    Write-Step 'Verifying SHA-256 ...'
    $sumsPath = Join-Path $workDir 'SHA256SUMS.txt'
    Invoke-WebRequest -Uri $sumsAsset.browser_download_url -OutFile $sumsPath -Headers $headers

    $entry = Select-String -Path $sumsPath -SimpleMatch $installer.name | Select-Object -First 1
    if (-not $entry) { throw "No checksum entry found for $($installer.name)." }

    $expected = (($entry.Line -split '\s+')[0]).ToLower()
    $actual = (Get-FileHash -Path $target -Algorithm SHA256).Hash.ToLower()

    if ($expected -ne $actual) {
        Remove-Item $target -Force -ErrorAction SilentlyContinue
        throw "CHECKSUM MISMATCH - download deleted.`n  expected: $expected`n  actual:   $actual"
    }
    Write-Host "    checksum OK ($actual)" -ForegroundColor Green
} else {
    Write-Warning 'SHA256SUMS.txt not present in this release - skipping verification.'
}

if ($DownloadOnly) {
    Write-Step "Downloaded (verified) to: $target"
    return
}

Write-Step 'Launching the installer ...'
Write-Host '    Windows may show a SmartScreen prompt for unsigned software:' -ForegroundColor DarkGray
Write-Host '    click "More info" -> "Run anyway".' -ForegroundColor DarkGray
Start-Process -FilePath $target -Wait
Write-Step 'Done.'
