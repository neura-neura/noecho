param([Parameter(Mandatory)][string]$Source)
$ErrorActionPreference = 'Stop'
try {
  $install = Join-Path $env:LOCALAPPDATA 'Programs\UnifiedAudio'
  $engine = Join-Path $install 'UnifiedAudio Engine Host.exe'
  if (-not (Test-Path -LiteralPath $engine)) { throw 'Motor no encontrado.' }
  $version = [Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $install 'UnifiedAudio.exe')).ProductVersion
  if ($version -notlike '0.1.6*') { throw "Esta actualización requiere UnifiedAudio 0.1.6; se encontró $version." }
  $busy = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and ([IO.Path]::GetDirectoryName($_.Path) -eq $install) }
  if ($busy) { throw 'Cierra UnifiedAudio desde su bandeja antes de actualizar.' }
  $backup = $engine + '.before-noecho'
  if (-not (Test-Path -LiteralPath $backup)) { Copy-Item -LiteralPath $engine -Destination $backup }
  Copy-Item -LiteralPath $Source -Destination $engine -Force
  Write-Output 'Motor actualizado. Tus perfiles, efectos y dispositivos se conservaron.'
} catch {
  [Console]::Error.WriteLine($_.Exception.Message)
  exit 1
}
