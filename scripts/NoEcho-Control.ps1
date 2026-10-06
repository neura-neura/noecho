param(
  [ValidateSet('status','apps','devices','config','meters','start','stop','exclude','include','set-exclusions','monitor','voice','channel')]
  [string]$Command = 'status',
  [string[]]$Apps = @(),
  [string]$Value,
  [string]$BaseUrl = 'http://127.0.0.1:47832',
  [string]$Computer
)
$ErrorActionPreference = 'Stop'
try {
  if ($Computer) { $BaseUrl = "http://${Computer}:47832" }
  $body = @{ command = $Command; apps = @($Apps) }
  if ($PSBoundParameters.ContainsKey('Value')) {
    if ($Command -eq 'voice') {
      if ($Value -notin @('true','false')) { throw 'Voice accepts true or false.' }
      $body.value = $Value -eq 'true'
    } elseif ($Command -eq 'channel' -and $Value -eq 'automatic') { $body.value = $null }
    else { $body.value = $Value }
  }
  $response = Invoke-RestMethod -Uri "$BaseUrl/v1/command" -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes(($body | ConvertTo-Json -Depth 5))) -TimeoutSec 30
  $response | ConvertTo-Json -Depth 12
} catch {
  [Console]::Error.WriteLine($_.ToString())
  exit 1
}
