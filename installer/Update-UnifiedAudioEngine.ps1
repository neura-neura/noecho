param([Parameter(Mandatory)][string]$Source)
$ErrorActionPreference = 'Stop'
try {
  $install = Join-Path $env:LOCALAPPDATA 'Programs\UnifiedAudio'
  $engine = Join-Path $install 'UnifiedAudio Engine Host.exe'
  if (-not (Test-Path -LiteralPath $engine)) { throw 'Motor no encontrado.' }
  if ((Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -eq (Get-FileHash -LiteralPath $engine -Algorithm SHA256).Hash) {
    $oldPending = Join-Path $env:LOCALAPPDATA 'NoEcho\pending-integration\engine.exe'
    if (Test-Path -LiteralPath $oldPending) { Remove-Item -LiteralPath $oldPending }
    Write-Output 'UnifiedAudio ya tiene el motor compatible. Puede permanecer abierto.'
    exit 0
  }
  $version = [Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $install 'UnifiedAudio.exe')).ProductVersion
  if ($version -notlike '0.1.6*') { throw "Esta actualización requiere UnifiedAudio 0.1.6; se encontró $version." }
  $busy = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and ([IO.Path]::GetDirectoryName($_.Path) -eq $install) }
  if ($busy) {
    $pending = Join-Path $env:LOCALAPPDATA 'NoEcho\pending-integration'
    New-Item -ItemType Directory -Path $pending -Force | Out-Null
    $temporary = Join-Path $pending 'engine.exe.tmp'
    Copy-Item -LiteralPath $Source -Destination $temporary -Force
    Move-Item -LiteralPath $temporary -Destination (Join-Path $pending 'engine.exe') -Force
    Write-Output 'Actualizacion del motor pendiente. Se aplicara cuando UnifiedAudio deje de usarlo. Puedes continuar trabajando.'
    exit 0
  }
  $backup = $engine + '.before-noecho'
  if (-not (Test-Path -LiteralPath $backup)) { Copy-Item -LiteralPath $engine -Destination $backup }
  Copy-Item -LiteralPath $Source -Destination $engine -Force
  Write-Output 'Motor actualizado. Tus perfiles, efectos y dispositivos se conservaron.'
} catch {
  $errorDir = Join-Path $env:LOCALAPPDATA 'NoEcho'
  New-Item -ItemType Directory -Path $errorDir -Force | Out-Null
  [IO.File]::WriteAllText((Join-Path $errorDir 'integration-update-error.txt'), $_.Exception.Message)
  [Console]::Error.WriteLine($_.Exception.Message)
  exit 1
}
