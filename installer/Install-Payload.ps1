param(
  [string]$Source = ""
)
$ErrorActionPreference = "Stop"
$dest = Join-Path $env:LOCALAPPDATA "NoEcho\payload"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
if (-not $Source) {
  $here = Split-Path -Parent $MyInvocation.MyCommand.Path
  $Source = Join-Path $here "payload"
}
if (-not (Test-Path $Source)) {
  Write-Host "No se encontro: $Source"
  exit 1
}
Copy-Item (Join-Path $Source "*") -Destination $dest -Force -Recurse
Write-Host "Preparativo copiado a $dest"
Write-Host "Abre NoEcho y pulsa: Preparar audio (solo una vez)"
