# Developer acceptance harness. Starts player instances sequentially; the player has no worker processes.
[CmdletBinding()]
param(
    [string]$Executable = (Join-Path $PSScriptRoot '../dist/Shiyin/Shiyin.exe'),
    [ValidateRange(30, 86400)][int]$SoakSeconds = 600,
    [switch]$SkipSoak
)
$ErrorActionPreference = 'Stop'
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
$projectRoot = Split-Path $PSScriptRoot -Parent
$runRoot = Join-Path $projectRoot ('.runtime/rust-acceptance-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $runRoot | Out-Null

function Invoke-PlayerCheck([string[]]$Arguments, [string]$Report, [int]$TimeoutSeconds) {
    # Windows paths cannot contain quotes. Quote every argument so spaces remain intact.
    $quoted = @($Arguments | ForEach-Object { '"' + $_ + '"' })
    $process = Start-Process -FilePath $executablePath -ArgumentList $quoted -PassThru -WindowStyle Hidden
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $samples = @()
    try {
        while (-not $process.WaitForExit(1000)) {
            if ($timer.Elapsed.TotalSeconds -gt $TimeoutSeconds) { throw "Timed out: $Arguments" }
            $process.Refresh()
            if (-not $process.HasExited) {
                $samples += [pscustomobject]@{
                    seconds = [math]::Round($timer.Elapsed.TotalSeconds, 1)
                    working_set_bytes = $process.WorkingSet64
                    private_bytes = $process.PrivateMemorySize64
                    handles = $process.HandleCount
                    threads = $process.Threads.Count
                    cpu_seconds = $process.TotalProcessorTime.TotalSeconds
                }
            }
        }
        if ($process.ExitCode -ne 0) { throw "Player failed with exit code $($process.ExitCode): $Arguments" }
        if (-not (Get-Content -LiteralPath $Report -Raw | ConvertFrom-Json).passed) { throw "Failed report: $Report" }
    } finally {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $samples | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath ($Report + '.resources.json') -Encoding utf8
        $process.Dispose()
    }
}

if (-not $SkipSoak) {
    $report = Join-Path $runRoot 'soak.json'
    Invoke-PlayerCheck @('--soak-test', $report, "$SoakSeconds") $report ($SoakSeconds + 90)
}
$fixture = Join-Path $runRoot 'fixture'
$report = Join-Path $runRoot 'scan-exit.json'
Invoke-PlayerCheck @('--lifecycle-test', 'create', $fixture, $report) $report 150
for ($number = 1; $number -le 3; $number++) {
    $report = Join-Path $runRoot "restart-$number.json"
    Invoke-PlayerCheck @('--lifecycle-test', 'restart', $fixture, $report) $report 150
}
Write-Host "Acceptance reports and temporary fixtures: $runRoot"
