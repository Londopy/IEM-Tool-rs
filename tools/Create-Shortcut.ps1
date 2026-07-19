<#
.SYNOPSIS
    Create optional .lnk shortcuts for IEM-Tool-rs on your Desktop.

.DESCRIPTION
    Generates Windows shortcuts that launch the local PowerShell scripts:
      * "Build IEM-Tool-rs"   -> tools\build.ps1   (build from source)
      * "Install IEM-Tool-rs" -> install.ps1       (download the latest release)

    WHY THIS GENERATES THE .lnk LOCALLY INSTEAD OF SHIPPING ONE:
    A .lnk file *downloaded from the internet* that launches PowerShell matches a
    well-known malware delivery pattern, so Windows Defender / SmartScreen often
    block or quarantine it outright. A shortcut you generate on your own machine
    from local files carries no Mark-of-the-Web, so it just works - same
    convenience, none of the security warnings. That's why no .lnk is attached
    to the GitHub release.

.PARAMETER For
    Which shortcut(s) to create: build (default), install, or both.

.PARAMETER Destination
    Where to write the shortcut(s). Defaults to your Desktop.

.EXAMPLE
    .\tools\Create-Shortcut.ps1

.EXAMPLE
    .\tools\Create-Shortcut.ps1 -For both
#>
[CmdletBinding()]
param(
    [ValidateSet('build', 'install', 'both')]
    [string]$For = 'build',
    [string]$Destination = [Environment]::GetFolderPath('Desktop')
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$iconPath = Join-Path $repoRoot 'icon.ico'

if (-not (Test-Path $Destination)) {
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
}

$shell = New-Object -ComObject WScript.Shell

function New-AppShortcut {
    param(
        [string]$Name,
        [string]$ScriptPath,
        [string]$Description
    )

    if (-not (Test-Path $ScriptPath)) {
        Write-Warning "Skipping '$Name' - script not found: $ScriptPath"
        return
    }

    $linkPath = Join-Path $Destination "$Name.lnk"
    $shortcut = $shell.CreateShortcut($linkPath)
    $shortcut.TargetPath = $powershell
    # -NoExit keeps the window open so you can read the output.
    $shortcut.Arguments = "-NoExit -ExecutionPolicy Bypass -File `"$ScriptPath`""
    $shortcut.WorkingDirectory = $repoRoot
    $shortcut.Description = $Description
    if (Test-Path $iconPath) { $shortcut.IconLocation = $iconPath }
    $shortcut.Save()

    Write-Host "Created: $linkPath" -ForegroundColor Green
}

if ($For -eq 'build' -or $For -eq 'both') {
    New-AppShortcut -Name 'Build IEM-Tool-rs' `
        -ScriptPath (Join-Path $PSScriptRoot 'build.ps1') `
        -Description 'Build IEM-Tool-rs from source with Tauri'
}

if ($For -eq 'install' -or $For -eq 'both') {
    New-AppShortcut -Name 'Install IEM-Tool-rs' `
        -ScriptPath (Join-Path $repoRoot 'install.ps1') `
        -Description 'Download and install the latest IEM-Tool-rs release'
}

Write-Host ''
Write-Host 'Done. These shortcuts run local scripts only - nothing is fetched at click time' -ForegroundColor DarkGray
Write-Host 'except by install.ps1, which downloads the release and verifies its SHA-256.' -ForegroundColor DarkGray
