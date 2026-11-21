# Windows PowerShell 5.1 bootstrap. No Python, Git, admin rights or package manager required.
[CmdletBinding()]
param([string]$PythonPath)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Assert-ChildPath {
    param([string]$Path, [string]$Parent)
    $full = [IO.Path]::GetFullPath($Path)
    $base = [IO.Path]::GetFullPath($Parent).TrimEnd('\') + '\'
    if (-not $full.StartsWith($base, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing operation outside intended directory: $full"
    }
    return $full
}

function Test-Hash {
    param([string]$Path, [string]$Sha256)
    return (Test-Path -LiteralPath $Path -PathType Leaf) -and
        ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -eq $Sha256)
}

function Get-VerifiedDownload {
    param($Artifact, [string]$Cache)
    if ($Artifact.sha256 -notmatch '^[a-fA-F0-9]{64}$' -or $Artifact.url -notlike 'https://*') {
        throw 'Toolchain artifacts require HTTPS and a pinned SHA-256 hash.'
    }
    New-Item -ItemType Directory -Force -Path $Cache | Out-Null
    $target = Assert-ChildPath (Join-Path $Cache $Artifact.file) $Cache
    if (Test-Hash $target $Artifact.sha256) { return $target }
    $partial = Assert-ChildPath ($target + '.part') $Cache
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        try {
            Write-Host "Downloading $($Artifact.file) (attempt $attempt/3)..."
            Invoke-WebRequest -UseBasicParsing -Uri $Artifact.url -OutFile $partial -TimeoutSec 180
            if (-not (Test-Hash $partial $Artifact.sha256)) {
                throw "SHA-256 mismatch: $($Artifact.file). No downloaded code was executed."
            }
            Move-Item -LiteralPath $partial -Destination $target -Force
            return $target
        } catch {
            if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial -Force }
            if ($attempt -eq 3) { throw }
            Write-Warning $_.Exception.Message
        }
    }
}

function Test-Python {
    param([string]$Executable, [string]$Venv = '')
    if (-not $Executable -or -not (Test-Path -LiteralPath $Executable -PathType Leaf)) { return $false }
    if ($Executable -like '*\Microsoft\WindowsApps\*') { return $false }
    try {
        & $Executable (Join-Path $PSScriptRoot 'probe_python.py') $Venv 2>$null | Out-Null
        return $LASTEXITCODE -eq 0
    } catch { return $false }
}

function Find-Python {
    param([string]$Preferred, $Artifact, [string]$Cache, [switch]$AfterInstall)
    if ($Preferred) {
        $Preferred = [IO.Path]::GetFullPath($Preferred)
        if (-not (Test-Python $Preferred)) { throw "Not a usable 64-bit Python 3.11-3.13 with Tk: $Preferred" }
        return $Preferred
    }
    $candidates = @()
    $command = Get-Command python.exe -ErrorAction SilentlyContinue
    if ($command) { $candidates += $command.Source }
    foreach ($hive in @('HKCU:\Software\Python\PythonCore', 'HKLM:\Software\Python\PythonCore')) {
        foreach ($key in @(Get-ChildItem -LiteralPath $hive -ErrorAction SilentlyContinue)) {
            $install = Get-Item -LiteralPath ($key.PSPath + '\InstallPath') -ErrorAction SilentlyContinue
            if ($install) {
                $directory = $install.GetValue('')
                if ($directory) { $candidates += Join-Path $directory 'python.exe' }
            }
        }
    }
    $userDirectory = Join-Path $env:LOCALAPPDATA ('Programs\Python\' + $Artifact.installDirectory)
    $candidates += Join-Path $userDirectory 'python.exe'
    foreach ($candidate in ($candidates | Select-Object -Unique)) {
        if (Test-Python $candidate) { return $candidate }
    }
    if ($AfterInstall) {
        throw 'Python setup completed but no usable interpreter with Tcl/Tk was found. Restart Windows if requested, or pass -PythonPath to a working interpreter.'
    }
    $installer = Get-VerifiedDownload $Artifact $Cache
    Write-Host "Installing official Python $($Artifact.version) for the current user (no PATH changes)..."
    $arguments = '/quiet InstallAllUsers=0 Include_pip=1 Include_tcltk=1 Include_launcher=0 Include_test=0 Include_doc=0 Shortcuts=0 AssociateFiles=0 PrependPath=0'
    $process = Start-Process -FilePath $installer -ArgumentList $arguments -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -notin @(0, 3010)) { throw "Python installer failed: exit $($process.ExitCode)" }
    return Find-Python -Preferred '' -Artifact $Artifact -Cache $Cache -AfterInstall
}

function Move-ToBackup {
    param([string]$Path, [string]$ProjectRoot)
    $source = Assert-ChildPath $Path $ProjectRoot
    $backups = Join-Path $ProjectRoot '.runtime\backups'
    New-Item -ItemType Directory -Force -Path $backups | Out-Null
    $destination = Assert-ChildPath (Join-Path $backups ((Split-Path $source -Leaf) + '-' + [guid]::NewGuid().ToString('N'))) $ProjectRoot
    Write-Host "Preserving previous runtime at $destination"
    Move-Item -LiteralPath $source -Destination $destination
}

function Test-Vlc {
    param([string]$Python, [string]$Directory)
    if (-not (Test-Path -LiteralPath (Join-Path $Directory 'libvlc.dll'))) { return $false }
    if (-not (Test-Path -LiteralPath (Join-Path $Directory 'plugins') -PathType Container)) { return $false }
    try {
        & $Python -c 'import sys; from pathlib import Path; from player.playback import VlcBackend; b=VlcBackend(Path(sys.argv[1])); b.close()' $Directory 2>$null | Out-Null
        return $LASTEXITCODE -eq 0
    } catch { return $false }
}

function Install-Vlc {
    param([string]$Python, $Artifact, [string]$ProjectRoot, [string]$Cache)
    $runtime = Join-Path $ProjectRoot '.runtime'
    $destination = Assert-ChildPath (Join-Path $runtime $Artifact.directory) $runtime
    if (Test-Vlc $Python $destination) {
        Write-Host 'VLC runtime is already working.'
        return
    }
    $archive = Get-VerifiedDownload $Artifact $Cache
    $staging = Assert-ChildPath (Join-Path $runtime ('staging-' + [guid]::NewGuid().ToString('N'))) $runtime
    New-Item -ItemType Directory -Path $staging | Out-Null
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        [IO.Compression.ZipFile]::ExtractToDirectory($archive, $staging)
        $extracted = Assert-ChildPath (Join-Path $staging $Artifact.directory) $staging
        if (-not (Test-Vlc $Python $extracted)) { throw 'Downloaded VLC did not pass its load check.' }
        if (Test-Path -LiteralPath $destination) { Move-ToBackup $destination $ProjectRoot }
        Move-Item -LiteralPath $extracted -Destination $destination
    } finally {
        $safeStaging = Assert-ChildPath $staging $runtime
        if (Test-Path -LiteralPath $safeStaging) { Remove-Item -LiteralPath $safeStaging -Recurse -Force }
    }
}

function Invoke-Setup {
    param([string]$ProjectRoot, [string]$PreferredPython = '')
    if (-not [Environment]::Is64BitOperatingSystem) { throw '64-bit Windows is required.' }
    if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') {
        throw 'This bootstrap targets Windows x64. ARM64 is not supported.'
    }
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $ProjectRoot = [IO.Path]::GetFullPath($ProjectRoot)
    $runtime = Join-Path $ProjectRoot '.runtime'
    New-Item -ItemType Directory -Force -Path $runtime | Out-Null
    $lock = $null
    Push-Location $ProjectRoot
    try {
        $lock = [IO.File]::Open((Join-Path $runtime 'setup.lock'), 'OpenOrCreate', 'ReadWrite', 'None')
        $manifest = Get-Content -LiteralPath (Join-Path $ProjectRoot 'toolchain.json') -Raw | ConvertFrom-Json
        $cache = Join-Path $runtime 'downloads'
        $venv = Join-Path $ProjectRoot '.venv'
        $python = Join-Path $venv 'Scripts\python.exe'
        if (-not (Test-Python $python $venv)) {
            $basePython = Find-Python $PreferredPython $manifest.python $cache
            if (Test-Path -LiteralPath $venv) { Move-ToBackup $venv $ProjectRoot }
            Write-Host "Creating project virtual environment with $basePython ..."
            & $basePython -m venv $venv
            if ($LASTEXITCODE -ne 0 -or -not (Test-Python $python $venv)) { throw 'Virtual environment creation failed.' }
        }
        Write-Host 'Installing pinned packages from requirements.txt ...'
        & $python -m pip install --disable-pip-version-check --retries 2 --timeout 30 --only-binary=:all: -r (Join-Path $ProjectRoot 'requirements.txt')
        if ($LASTEXITCODE -ne 0) { throw 'pip failed. Check network/proxy settings and rerun scripts/setup.ps1.' }
        Install-Vlc $python $manifest.vlc $ProjectRoot $cache
        & $python -m scripts.verify_runtime
        if ($LASTEXITCODE -ne 0) { throw 'Final runtime verification failed.' }
        Write-Host 'Setup complete. Run .venv\Scripts\python.exe main.py to launch from source.' -ForegroundColor Green
    } finally {
        if ($lock) { $lock.Dispose() }
        Pop-Location
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        Invoke-Setup -ProjectRoot (Split-Path $PSScriptRoot -Parent) -PreferredPython $PythonPath
    } catch {
        Write-Host ('Setup failed: ' + $_.Exception.Message) -ForegroundColor Red
        exit 1
    }
}
