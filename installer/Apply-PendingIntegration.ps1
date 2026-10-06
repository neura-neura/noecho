param([Parameter(Mandatory)][string]$NoEchoExecutable)
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:LOCALAPPDATA 'NoEcho'
$pendingEngine = Join-Path $root 'pending-integration\engine.exe'
$pendingParsec = Join-Path $root 'parsec-pending'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$watcherLock = [Threading.Mutex]::new($false, 'Local\NoEchoPendingIntegration')
if (-not $watcherLock.WaitOne(0)) { $watcherLock.Dispose(); exit 0 }
try {
    if (-not ((Test-Path -LiteralPath $pendingEngine) -or (Test-Path -LiteralPath $pendingParsec))) { exit 0 }
    New-Item -Path $runKey -Force | Out-Null
    $command = 'powershell.exe -NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "' + $PSCommandPath + '" -NoEchoExecutable "' + $NoEchoExecutable + '"'
    Set-ItemProperty -Path $runKey -Name NoEchoPendingIntegration -Value $command
    while ((Test-Path -LiteralPath $pendingEngine) -or (Test-Path -LiteralPath $pendingParsec)) {
        if (-not (Test-Path -LiteralPath $NoEchoExecutable)) { break }
        try {
            if (Test-Path -LiteralPath $pendingEngine) {
                $install = Join-Path $env:LOCALAPPDATA 'Programs\UnifiedAudio'
                $engine = Join-Path $install 'UnifiedAudio Engine Host.exe'
                $busy = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and ([IO.Path]::GetDirectoryName($_.Path) -eq $install) }
                if (-not $busy) {
                    if (-not (Test-Path -LiteralPath ($engine + '.before-noecho'))) { Copy-Item -LiteralPath $engine -Destination ($engine + '.before-noecho') }
                    Copy-Item -LiteralPath $pendingEngine -Destination $engine -Force
                    Remove-Item -LiteralPath $pendingEngine
                }
            }
            if (Test-Path -LiteralPath $pendingParsec) {
                $process = Start-Process -FilePath $NoEchoExecutable -ArgumentList '--prepare-parsec' -WindowStyle Hidden -Wait -PassThru
                if ($process.ExitCode -ne 0) { throw 'Parsec preparation failed.' }
            }
            Remove-Item -LiteralPath (Join-Path $root 'pending-integration-error.txt') -ErrorAction SilentlyContinue
        } catch {
            [IO.File]::WriteAllText((Join-Path $root 'pending-integration-error.txt'), $_.Exception.Message)
        }
        if ((Test-Path -LiteralPath $pendingEngine) -or (Test-Path -LiteralPath $pendingParsec)) { Start-Sleep -Seconds 10 }
    }
} finally {
    if (-not (Test-Path -LiteralPath $NoEchoExecutable) -or -not ((Test-Path -LiteralPath $pendingEngine) -or (Test-Path -LiteralPath $pendingParsec))) {
        Remove-ItemProperty -Path $runKey -Name NoEchoPendingIntegration -ErrorAction SilentlyContinue
    }
    $watcherLock.ReleaseMutex(); $watcherLock.Dispose()
}
