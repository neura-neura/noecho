$ErrorActionPreference = 'Stop'
try {
  $install = Join-Path $env:LOCALAPPDATA 'Programs\UnifiedAudio'
  $session = (Get-Process -Id $PID).SessionId
  $apps = @(Get-Process -ErrorAction SilentlyContinue | Where-Object {
    $_.SessionId -eq $session -and $_.Path -and (
      [IO.Path]::GetDirectoryName($_.Path) -eq $install -or
      [IO.Path]::GetFileName($_.Path) -ieq 'parsecd.exe'
    )
  })
  # Give apps a chance to save settings before stopping tray processes.
  foreach ($app in $apps) { [void]$app.CloseMainWindow() }
  foreach ($app in $apps) {
    if (-not $app.HasExited -and -not $app.WaitForExit(5000)) {
      Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
      if (-not $app.WaitForExit(5000)) { throw "No se pudo cerrar $($app.ProcessName)." }
    }
  }
  Write-Output 'UnifiedAudio y Parsec cerrados. Puedes abrirlos de nuevo al terminar.'
} catch {
  $errorDir = Join-Path $env:LOCALAPPDATA 'NoEcho'
  New-Item -ItemType Directory -Path $errorDir -Force | Out-Null
  [IO.File]::WriteAllText((Join-Path $errorDir 'close-apps-error.txt'), $_.Exception.Message)
  [Console]::Error.WriteLine($_.Exception.Message)
  exit 1
}
