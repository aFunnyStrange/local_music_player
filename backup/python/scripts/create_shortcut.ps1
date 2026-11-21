# Create or update a desktop launcher with the project's own icon.
param([string]$ShortcutPath = '', [string]$ExecutablePath = '')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $ExecutablePath) {
    $candidates = @((Join-Path $PSScriptRoot 'Shiyin.exe'), (Join-Path $projectRoot 'dist\Shiyin\Shiyin.exe'))
    $ExecutablePath = $candidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
}
if (-not $ExecutablePath -or -not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
    throw 'Supply -ExecutablePath pointing to the extracted Shiyin.exe, or build a release first.'
}
$ExecutablePath = [IO.Path]::GetFullPath($ExecutablePath)
if (-not $ShortcutPath) {
    $ShortcutPath = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Shiyin.lnk'
}
if ([IO.Path]::GetExtension($ShortcutPath) -ne '.lnk') { throw 'ShortcutPath must end in .lnk.' }
$shell = New-Object -ComObject WScript.Shell
try {
    $shortcut = $shell.CreateShortcut([IO.Path]::GetFullPath($ShortcutPath))
    $shortcut.TargetPath = $ExecutablePath
    $shortcut.Arguments = ''
    $shortcut.WorkingDirectory = Split-Path $ExecutablePath -Parent
    $shortcut.IconLocation = $ExecutablePath + ',0'
    $shortcut.Description = 'Shiyin local music player'
    $shortcut.Save()
    Write-Host "Shortcut saved: $ShortcutPath"
} finally {
    if ($shortcut) { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($shortcut) }
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($shell)
}
