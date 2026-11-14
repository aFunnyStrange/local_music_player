# Deterministic integrity checks without downloading or executing any binary.
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
. (Join-Path $project 'scripts\setup.ps1')
$testParent = Join-Path $project '.runtime\setup-tests'
$fixture = Assert-ChildPath (Join-Path $testParent ('integrity-' + [guid]::NewGuid().ToString('N'))) $testParent
New-Item -ItemType Directory -Force -Path $fixture | Out-Null
try {
    $cached = Join-Path $fixture 'fixture.bin'
    [IO.File]::WriteAllText($cached, 'trusted fixture')
    $artifact = [pscustomobject]@{file='fixture.bin'; url='https://example.invalid/fixture'; sha256=(Get-FileHash $cached -Algorithm SHA256).Hash}
    $script:attempts = 0
    $script:payload = 'corrupted response'
    function Invoke-WebRequest {
        param([switch]$UseBasicParsing, [string]$Uri, [string]$OutFile, [int]$TimeoutSec)
        $script:attempts++
        [IO.File]::WriteAllText($OutFile, $script:payload)
    }
    $result = Get-VerifiedDownload $artifact $fixture
    if ($result -ne $cached -or $script:attempts -ne 0) { throw 'Valid cache unexpectedly used the network' }
    [IO.File]::WriteAllText($cached, 'invalid cache')
    $rejected = $false
    try { Get-VerifiedDownload $artifact $fixture | Out-Null } catch { $rejected = $true }
    if (-not $rejected -or $script:attempts -ne 3) { throw 'Corrupted download was not rejected with bounded retries' }
    if (Test-Path -LiteralPath ($cached + '.part')) { throw 'Partial download was left behind' }
    $script:payload = 'trusted fixture'
    $result = Get-VerifiedDownload $artifact $fixture
    if (-not (Test-Hash $result $artifact.sha256)) { throw 'Valid download did not replace the invalid cache' }
    $rejected = $false
    try { Assert-ChildPath (Join-Path $fixture '..\outside') $fixture | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Directory boundary was not enforced' }
    Write-Host 'SETUP INTEGRITY PASS: cache reuse, hash rejection, bounded retries, partial cleanup, directory boundaries.'
} finally {
    $safeFixture = Assert-ChildPath $fixture $testParent
    Remove-Item -LiteralPath $safeFixture -Recurse -Force
}
