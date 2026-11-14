# Create or update a desktop launcher with the project's own icon.
param([string]$ShortcutPath = '')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $ShortcutPath) {
    $ShortcutPath = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Shiyin.lnk'
}
if ([IO.Path]::GetExtension($ShortcutPath) -ne '.lnk') { throw 'ShortcutPath must end in .lnk.' }
$shell = New-Object -ComObject WScript.Shell
try {
    $shortcut = $shell.CreateShortcut([IO.Path]::GetFullPath($ShortcutPath))
    $shortcut.TargetPath = Join-Path $projectRoot 'run.cmd'
    $shortcut.Arguments = ''
    $shortcut.WorkingDirectory = $projectRoot
    $shortcut.IconLocation = (Join-Path $projectRoot 'assets\logo.ico') + ',0'
    $shortcut.Description = 'Shiyin local music player'
    $shortcut.Save()
    Write-Host "Shortcut saved: $ShortcutPath"
} finally {
    if ($shortcut) { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($shortcut) }
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($shell)
}
