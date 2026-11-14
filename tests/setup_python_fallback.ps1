# Exercise the no-Python path without installing or altering system software.
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
. (Join-Path $project 'scripts\setup.ps1')
$manifest = Get-Content -LiteralPath (Join-Path $project 'toolchain.json') -Raw | ConvertFrom-Json
$expected = Join-Path $env:LOCALAPPDATA ('Programs\Python\' + $manifest.python.installDirectory + '\python.exe')
$script:installed = $false
$script:exitCode = 0
$script:launches = 0
function Get-Command { param($Name, $ErrorAction) return $null }
function Get-ChildItem { param($LiteralPath, $ErrorAction) return @() }
function Test-Python {
    param([string]$Executable, [string]$Venv = '')
    return $script:installed -and $Executable -eq $expected
}
function Get-VerifiedDownload {
    param($Artifact, [string]$Cache)
    if ($Artifact.url -notlike 'https://www.python.org/*') { throw 'Unexpected installer origin' }
    return 'verified-python-installer.exe'
}
function Start-Process {
    param($FilePath, $ArgumentList, [switch]$Wait, [switch]$PassThru, $WindowStyle)
    if ($FilePath -ne 'verified-python-installer.exe' -or $WindowStyle -ne 'Hidden' -or -not $Wait) {
        throw 'Incorrect installer invocation'
    }
    foreach ($required in @('/quiet', 'InstallAllUsers=0', 'Include_tcltk=1', 'Include_pip=1', 'PrependPath=0')) {
        if ($ArgumentList -notlike ('*' + $required + '*')) { throw "Missing installer option: $required" }
    }
    $script:launches++
    $script:installed = $script:exitCode -eq 0
    return [pscustomobject]@{ExitCode=$script:exitCode}
}
$result = Find-Python -Preferred '' -Artifact $manifest.python -Cache 'unused-cache'
if ($result -ne $expected -or $script:launches -ne 1) { throw 'Missing-Python fallback failed' }
$script:installed = $false
$script:exitCode = 1603
$rejected = $false
try { Find-Python -Preferred '' -Artifact $manifest.python -Cache 'unused-cache' | Out-Null } catch { $rejected = $true }
if (-not $rejected) { throw 'Installer error was not propagated' }
Write-Host 'PYTHON FALLBACK PASS (mock installer): per-user flags, Tk/pip, rediscovery, installer failure.'
