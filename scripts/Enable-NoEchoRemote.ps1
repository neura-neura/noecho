param([Parameter(Mandatory=$true)][string]$ProgramPath)
$ErrorActionPreference = 'Stop'
try {
    if (-not (Test-Path -LiteralPath $ProgramPath -PathType Leaf)) { throw 'NoEcho executable is missing.' }
    Get-NetFirewallRule -Name 'NoEcho-Network-Control' -ErrorAction SilentlyContinue | Remove-NetFirewallRule
    New-NetFirewallRule -Name 'NoEcho-Network-Control' -DisplayName 'NoEcho network control' -Direction Inbound -Action Allow -Protocol TCP -LocalPort 47832 -Program $ProgramPath -RemoteAddress LocalSubnet -Profile Any | Out-Null
    exit 0
} catch { Write-Error $_; exit 1 }
