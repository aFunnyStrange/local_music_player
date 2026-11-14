# Real installation into a disposable project with spaces and no prebuilt environment.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
. (Join-Path $project 'scripts\setup.ps1')
$testParent = Join-Path $project '.runtime\setup-tests'
New-Item -ItemType Directory -Force -Path $testParent | Out-Null
$fixture = Assert-ChildPath (Join-Path $testParent ('new device ' + [guid]::NewGuid().ToString('N'))) $testParent
New-Item -ItemType Directory -Path $fixture | Out-Null
try {
    foreach ($name in @('settings.py', 'requirements.txt', 'toolchain.json', 'player', 'scripts')) {
        Copy-Item -LiteralPath (Join-Path $project $name) -Destination $fixture -Recurse
    }
    $manifest = Get-Content -LiteralPath (Join-Path $project 'toolchain.json') -Raw | ConvertFrom-Json
    $cache = Join-Path $fixture '.runtime\downloads'
    New-Item -ItemType Directory -Force -Path $cache | Out-Null
    $existingZip = Join-Path $project ('.runtime\' + $manifest.vlc.file)
    if (Test-Hash $existingZip $manifest.vlc.sha256) {
        Copy-Item -LiteralPath $existingZip -Destination $cache
    }
    # Preserve user data even when rebuilding a broken virtual environment.
    New-Item -ItemType Directory -Path (Join-Path $fixture 'data') | Out-Null
    $sentinel = Join-Path $fixture 'data\user-data.txt'
    [IO.File]::WriteAllText($sentinel, 'must remain unchanged')
    New-Item -ItemType Directory -Path (Join-Path $fixture '.venv') | Out-Null
    [IO.File]::WriteAllText((Join-Path $fixture '.venv\old-marker'), 'preserve broken venv')
    Invoke-Setup -ProjectRoot $fixture
    $venvPython = Join-Path $fixture '.venv\Scripts\python.exe'
    if (-not (Test-Python $venvPython (Join-Path $fixture '.venv'))) { throw 'Fresh venv is not usable' }
    if ([IO.File]::ReadAllText($sentinel) -ne 'must remain unchanged') { throw 'User data was modified' }
    if (@(Get-ChildItem -LiteralPath (Join-Path $fixture '.runtime\backups') -Filter 'old-marker' -Recurse).Count -ne 1) {
        throw 'Broken venv was not preserved'
    }
    $marker = Join-Path $fixture '.venv\repeat-marker'
    [IO.File]::WriteAllText($marker, 'idempotency')
    Invoke-Setup -ProjectRoot $fixture
    if (-not (Test-Path -LiteralPath $marker)) { throw 'Repeated setup unnecessarily replaced the venv' }
    Write-Host 'SETUP SYSTEM PASS: discovery, broken venv backup, new venv, packages, VLC extraction/load, data preservation, repeat run.'
} finally {
    $safeFixture = Assert-ChildPath $fixture $testParent
    Remove-Item -LiteralPath $safeFixture -Recurse -Force
}
