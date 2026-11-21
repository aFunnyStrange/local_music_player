# Source materials are separate from the end-user bundle. Requires PowerShell 7.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'setup.ps1')
$catalog = Get-Content (Join-Path $root 'packaging/native-sources.json') -Raw | ConvertFrom-Json
$cache = Join-Path $root '.runtime/native-sources'
New-Item -ItemType Directory -Force -Path $cache | Out-Null
$vlcArchive = Get-VerifiedDownload $catalog.vlc (Join-Path $root '.runtime/downloads')
$stage = Join-Path $root ('.runtime/source-bundle-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage | Out-Null
& tar -xf $vlcArchive -C $stage
if ($LASTEXITCODE -ne 0) { throw 'VLC source extraction failed' }
$tarballs = Join-Path $stage 'vlc-3.0.21/contrib/tarballs'
New-Item -ItemType Directory -Force -Path $tarballs | Out-Null
# Stream concurrent downloads to disk with bounded concurrency and timeouts.
$config = @()
foreach ($item in $catalog.contrib) {
    $path = Assert-ChildPath (Join-Path $cache $item.file) $cache
    if ($item.PSObject.Properties.Name -contains 'generated_from') {
        $patch = Get-Content -LiteralPath (Join-Path $stage $item.generated_from)
        $lines = @($patch | Where-Object { $_.StartsWith('+') -and -not $_.StartsWith('+++') } | ForEach-Object { $_.Substring(1) })
        [IO.File]::WriteAllText($path, ($lines -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
    } elseif (-not ((Test-Path -LiteralPath $path) -and (Get-FileHash -LiteralPath $path -Algorithm SHA512).Hash -eq $item.sha512)) {
        if ($item.url -notlike 'https://*' -or $item.url.Contains('"') -or $item.file -match '[/\\"]') { throw 'Invalid source URL or filename' }
        $config += 'url = "' + $item.url + '"'
        $config += @('fail', 'location', 'retry = 3', 'max-time = 300', 'connect-timeout = 30')
        $config += 'output = "' + $path.Replace('\', '/') + '"'
        $config += 'next'
    }
}
if ($config.Count) {
    $configPath = Join-Path $root ('.runtime/downloads-' + [guid]::NewGuid().ToString('N') + '.curl')
    [IO.File]::WriteAllLines($configPath, $config[0..($config.Count-2)])
    & curl.exe --parallel --parallel-max 4 --fail --location --retry 3 --max-time 300 --connect-timeout 30 --config $configPath
    if ($LASTEXITCODE -ne 0) { throw 'Source downloads failed; nothing will be published' }
}
foreach ($item in $catalog.contrib) {
    $path = Join-Path $cache $item.file
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA512).Hash -ne $item.sha512) { throw "Source hash mismatch: $($item.file)" }
    Copy-Item -LiteralPath $path -Destination $tarballs
}
Push-Location $root
try {
    $metadata = & cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $version = ($metadata.packages | Where-Object name -eq 'shiyin-rs').version
    $rustSources = Join-Path $stage 'rust'
    New-Item -ItemType Directory -Path $rustSources | Out-Null
    & cargo vendor --locked --versioned-dirs (Join-Path $rustSources 'vendor') | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Rust source collection failed' }
    @'
[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "vendor"
'@ | Set-Content (Join-Path $rustSources 'config.toml') -Encoding utf8
    Copy-Item -LiteralPath (Join-Path $root 'Cargo.lock') -Destination $rustSources
    Copy-Item -LiteralPath (Join-Path $root 'packaging/native-sources.json') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $root 'docs/third-party-sources.md') -Destination (Join-Path $stage 'README.md')
    $dist = Join-Path $root 'dist'
    New-Item -ItemType Directory -Force -Path $dist | Out-Null
    $zip = Join-Path $dist "Shiyin-$version-third-party-sources.zip"
    & tar -a -cf $zip -C $stage .
    if ($LASTEXITCODE -ne 0) { throw 'Source archive creation failed' }
    ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLower() + '  ' + (Split-Path $zip -Leaf)) | Set-Content -Encoding ascii ($zip + '.sha256')
    Write-Host "Verified source material bundle: $zip"
} finally { Pop-Location }
