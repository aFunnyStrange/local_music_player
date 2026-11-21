# Windows developer setup. Requires Rustup and MSVC C++ Build Tools / Windows SDK.
[CmdletBinding()]
param([switch]$SkipFetch)
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

function Initialize-Shiyin {
    $root = Split-Path $PSScriptRoot -Parent
    if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        throw 'Install Rustup (Windows MSVC) from https://rustup.rs and Visual Studio C++ Build Tools first. End users should use the executable release.'
    }
    Push-Location $root
    try {
        $channel = [regex]::Match((Get-Content rust-toolchain.toml -Raw), 'channel\s*=\s*"([^"]+)"').Groups[1].Value
        if (-not $channel) { throw 'Missing pinned Rust toolchain' }
        & rustup toolchain install $channel --profile minimal --component rustfmt --component clippy
        if ($LASTEXITCODE -ne 0) { throw 'Rust toolchain setup failed' }
        if (-not $SkipFetch) {
            & cargo fetch --locked
            if ($LASTEXITCODE -ne 0) { throw 'Dependency fetch failed' }
        }
        $manifest = Get-Content toolchain.json -Raw | ConvertFrom-Json
        $runtime = Join-Path $root ('.runtime/' + $manifest.vlc.directory)
        if (-not (Test-Path -LiteralPath (Join-Path $runtime 'libvlc.dll'))) {
            $archive = Get-VerifiedDownload $manifest.vlc (Join-Path $root '.runtime/downloads')
            Add-Type -AssemblyName System.IO.Compression.FileSystem
            [IO.Compression.ZipFile]::ExtractToDirectory($archive, (Join-Path $root '.runtime'))
        }
        Write-Host 'Ready. Run cargo run --locked, or scripts/build_release.ps1.'
    } finally { Pop-Location }
}
if ($MyInvocation.InvocationName -ne '.') { Initialize-Shiyin }