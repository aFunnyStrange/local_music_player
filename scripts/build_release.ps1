# One-shot Windows packaging; the player itself never launches helper processes.
[CmdletBinding()]
param([ValidateRange(1, 32)][int]$BuildJobs = 2)
$ErrorActionPreference = 'Stop'
$rustRoot = Split-Path $PSScriptRoot -Parent
$projectRoot = $rustRoot
$manifest = Get-Content -LiteralPath (Join-Path $projectRoot 'toolchain.json') -Raw | ConvertFrom-Json
. (Join-Path $projectRoot 'scripts\setup.ps1')
$runtime = Join-Path $projectRoot ('.runtime\' + $manifest.vlc.directory)
if (-not (Test-Path -LiteralPath (Join-Path $runtime 'libvlc.dll'))) {
    $archive = Get-VerifiedDownload $manifest.vlc (Join-Path $projectRoot '.runtime\downloads')
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory($archive, (Join-Path $projectRoot '.runtime'))
}
Push-Location $rustRoot
try {
    # Keep packaging separate from a developer's running target/release binary.
    $packageTarget = Join-Path $rustRoot 'target\package'
    & cargo build --release --locked --jobs $BuildJobs --target-dir $packageTarget
    if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed' }
    $metadata = (& cargo metadata --locked --format-version 1 | ConvertFrom-Json)
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $package = $metadata.packages | Where-Object { $_.name -eq 'shiyin-rs' } | Select-Object -First 1
    $dist = Join-Path $rustRoot 'dist'
    $bundle = Join-Path $dist 'Shiyin'
    New-Item -ItemType Directory -Force -Path $bundle | Out-Null
    Copy-Item -LiteralPath (Join-Path $packageTarget 'x86_64-pc-windows-msvc\release\shiyin.exe') -Destination (Join-Path $bundle 'Shiyin.exe') -Force
    if (-not (Test-Path -LiteralPath (Join-Path $bundle 'vlc'))) {
        Copy-Item -LiteralPath $runtime -Destination (Join-Path $bundle 'vlc') -Recurse
    }
    foreach ($file in @('README.md', 'README.en.md')) { Copy-Item -LiteralPath (Join-Path $rustRoot $file) -Destination $bundle -Force }
    Copy-Item -LiteralPath (Join-Path $rustRoot 'docs') -Destination $bundle -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $bundle -Force
    Copy-Item -LiteralPath (Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md') -Destination $bundle -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'create_shortcut.ps1') -Destination $bundle -Force
    Copy-Item -LiteralPath (Join-Path $rustRoot 'Cargo.lock') -Destination $bundle -Force
    $licenses = Join-Path $bundle 'licenses'
    New-Item -ItemType Directory -Force -Path $licenses | Out-Null
    Copy-Item -LiteralPath (Join-Path $projectRoot 'assets/licenses/VLC-LGPL-2.1.txt') -Destination $licenses -Force
    $sysroot = (& rustc --print sysroot).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot locate Rust standard-library notices' }
    $rustNotices = Join-Path $sysroot 'share/doc/rust'
    $rustLicenseTarget = Join-Path $licenses 'rust-toolchain'
    New-Item -ItemType Directory -Force -Path $rustLicenseTarget | Out-Null
    Copy-Item -LiteralPath (Join-Path $rustNotices 'COPYRIGHT-library.html') -Destination $rustLicenseTarget -Force
    Copy-Item -LiteralPath (Join-Path $rustNotices 'licenses') -Destination $rustLicenseTarget -Recurse -Force
    $dependencies = @()
    foreach ($dependency in $metadata.packages) {
        if ($dependency.name -eq 'shiyin-rs') { continue }
        $dependencies += [pscustomobject]@{name=$dependency.name; version=$dependency.version; license=$dependency.license; repository=$dependency.repository}
        $source = Split-Path $dependency.manifest_path -Parent
        $destination = Join-Path $licenses ($dependency.name + '-' + $dependency.version)
        $files = Get-ChildItem -LiteralPath $source -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE|COPYRIGHT)' }
        foreach ($file in $files) {
            New-Item -ItemType Directory -Force -Path $destination | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
        }
    }
    $dependencies | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 (Join-Path $licenses 'dependencies.json')
    $report = Join-Path $dist 'verification.json'
    $oldVlc = $env:SHIYIN_VLC_DIR
    $oldPath = $env:PATH
    try {
        $env:SHIYIN_VLC_DIR = Join-Path $bundle 'vlc'
        $env:PATH = Join-Path $env:SystemRoot 'System32'
        $errorLog = Join-Path $dist 'verification-stderr.txt'
        $process = Start-Process -FilePath (Join-Path $bundle 'Shiyin.exe') -ArgumentList @('--self-test', ('"' + $report + '"')) -PassThru -WindowStyle Hidden -RedirectStandardError $errorLog
        if (-not $process.WaitForExit(60000)) { $process.Kill(); throw 'Packaged verification timed out' }
        if ($process.ExitCode -ne 0) {
            if (Test-Path -LiteralPath $errorLog) { Get-Content -LiteralPath $errorLog | Write-Host }
            throw "Packaged verification failed (exit $($process.ExitCode))"
        }
        if (-not (Get-Content -LiteralPath $report -Raw | ConvertFrom-Json).passed) { throw 'Verification did not pass' }
    } finally { $env:SHIYIN_VLC_DIR = $oldVlc; $env:PATH = $oldPath }
    $zip = Join-Path $dist ('Shiyin-' + $package.version + '-windows-x64.zip')
    # Some crate license files use reproducible Unix-epoch timestamps; ZIP starts in 1980.
    Get-ChildItem -LiteralPath $bundle -Recurse -File | Where-Object { $_.LastWriteTime.Year -lt 1980 } | ForEach-Object { $_.LastWriteTime = [datetime]'1980-01-01' }
    Compress-Archive -LiteralPath $bundle -DestinationPath $zip -Force
    ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLower() + '  ' + (Split-Path $zip -Leaf)) | Set-Content -Encoding ascii ($zip + '.sha256')
    Write-Host "Verified Windows bundle: $zip"
} finally { Pop-Location }
