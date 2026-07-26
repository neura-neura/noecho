param(
  [switch]$SkipFrontend,
  [switch]$SkipTauriBuild
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
Write-Host "== NoEcho instalador de un solo clic ==" -ForegroundColor Cyan
$payloadDir = Join-Path $Root "installer\payload"
$hasPayload = Get-ChildItem -Path $payloadDir -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '\.(exe|zip)$' -and $_.Name -notmatch '^README' }
if (-not $hasPayload) {
  Write-Host "AVISO: no hay paquete de audio en installer\payload\" -ForegroundColor Yellow
} else {
  Write-Host "Paquete de audio detectado:" -ForegroundColor Green
  $hasPayload | ForEach-Object { Write-Host " - $($_.Name)" }
}
if (-not $SkipFrontend) {
  npm install
  npm run build
}
if (-not $SkipTauriBuild) {
  npx tauri build
}
$bundle = Join-Path $Root "src-tauri\target\release\bundle\nsis"
$fallback = Join-Path $Root "target\release\bundle\nsis"
$nsisDir = $null
if (Test-Path $bundle) { $nsisDir = $bundle }
elseif (Test-Path $fallback) { $nsisDir = $fallback }
$dist = Join-Path $Root "dist-installer"
New-Item -ItemType Directory -Force -Path $dist | Out-Null
if ($nsisDir) {
  Get-ChildItem $nsisDir -Filter *.exe | ForEach-Object {
    Copy-Item $_.FullName -Destination (Join-Path $dist $_.Name) -Force
    Write-Host "Instalador: $($_.Name)" -ForegroundColor Green
  }
} else {
  Write-Host "No se encontro el instalador NSIS." -ForegroundColor Yellow
}
$packPayload = Join-Path $dist "payload"
New-Item -ItemType Directory -Force -Path $packPayload | Out-Null
Copy-Item (Join-Path $payloadDir "*") -Destination $packPayload -Force -ErrorAction SilentlyContinue
$copyScript = @"
param([string]`$From = ".\payload")
`$ErrorActionPreference = "Stop"
`$dest = Join-Path `$env:LOCALAPPDATA "NoEcho\payload"
New-Item -ItemType Directory -Force -Path `$dest | Out-Null
if (-not (Test-Path `$From)) { Write-Host "No existe: `$From"; exit 1 }
Copy-Item (Join-Path `$From "*") -Destination `$dest -Force -Recurse
Write-Host "Listo. Abre NoEcho y pulsa Preparar audio (solo una vez)."
"@
Set-Content -Encoding UTF8 (Join-Path $dist "Copiar-Preparativo-Audio.ps1") -Value $copyScript
$leeme = @"
NoEcho - instalacion simple
1. Ejecuta el instalador NoEcho_...-setup.exe
2. Abre NoEcho
3. Si pide preparativo, pulsa Preparar audio (solo una vez)
4. Marca Discord (u otra app) y pulsa Ocultar del remoto
"@
Set-Content -Encoding UTF8 (Join-Path $dist "LEEME.txt") -Value $leeme
Write-Host "Paquete listo en: $dist" -ForegroundColor Cyan
