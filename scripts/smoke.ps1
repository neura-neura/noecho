param()
$ErrorActionPreference = 'Stop'
Write-Host '== NoEcho smoke ==' -ForegroundColor Cyan
cargo run -q -p tech-probe -- smoke
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host 'OK' -ForegroundColor Green