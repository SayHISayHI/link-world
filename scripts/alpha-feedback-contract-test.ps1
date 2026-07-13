[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$fixture = Join-Path $repo 'tests/fixtures/alpha-feedback-ready'
$validator = Join-Path $PSScriptRoot 'alpha-feedback-readiness.ps1'
$pwsh = (Get-Process -Id $PID).Path
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("node-tide-alpha-contract-" + [guid]::NewGuid().ToString('N'))

try {
  New-Item -ItemType Directory -Path $tempRoot | Out-Null

  & $pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -EvidenceDirectory $fixture -AllowSyntheticFixture -OutputPath (Join-Path $tempRoot 'ready.json') *> $null
  if ($LASTEXITCODE -ne 0) {
    throw 'The known-ready synthetic fixture did not pass.'
  }

  & $pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -EvidenceDirectory $fixture *> $null
  if ($LASTEXITCODE -eq 0) {
    throw 'Synthetic evidence was accepted without the explicit test-only switch.'
  }

  $sensitiveFixture = Join-Path $tempRoot 'sensitive'
  Copy-Item -LiteralPath $fixture -Destination $sensitiveFixture -Recurse
  $participantsPath = Join-Path $sensitiveFixture 'participants.json'
  $participants = Get-Content -LiteralPath $participantsPath -Raw | ConvertFrom-Json -Depth 100
  $participants.records[0] | Add-Member -NotePropertyName apiKey -NotePropertyValue 'sk-synthetic-not-a-real-secret'
  $participants | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $participantsPath -Encoding utf8

  & $pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -EvidenceDirectory $sensitiveFixture -AllowSyntheticFixture *> $null
  if ($LASTEXITCODE -eq 0) {
    throw 'Evidence containing a forbidden sensitive field was accepted.'
  }

  $unknownFixture = Join-Path $tempRoot 'unknown-field'
  Copy-Item -LiteralPath $fixture -Destination $unknownFixture -Recurse
  $unknownParticipantsPath = Join-Path $unknownFixture 'participants.json'
  $unknownParticipants = Get-Content -LiteralPath $unknownParticipantsPath -Raw | ConvertFrom-Json -Depth 100
  $unknownParticipants.records[0] | Add-Member -NotePropertyName notes -NotePropertyValue 'Synthetic free text'
  $unknownParticipants | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $unknownParticipantsPath -Encoding utf8

  & $pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -EvidenceDirectory $unknownFixture -AllowSyntheticFixture *> $null
  if ($LASTEXITCODE -eq 0) {
    throw 'An undeclared free-text field bypassed the fail-closed schema.'
  }

  $smallCohortFixture = Join-Path $tempRoot 'small-cohort'
  Copy-Item -LiteralPath $fixture -Destination $smallCohortFixture -Recurse
  $smallParticipantsPath = Join-Path $smallCohortFixture 'participants.json'
  $smallParticipants = Get-Content -LiteralPath $smallParticipantsPath -Raw | ConvertFrom-Json -Depth 100
  $smallParticipants.records = @($smallParticipants.records | Select-Object -First 4)
  $smallParticipants | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $smallParticipantsPath -Encoding utf8
  & $pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -EvidenceDirectory $smallCohortFixture -AllowSyntheticFixture *> $null
  if ($LASTEXITCODE -eq 0) {
    throw 'A cohort smaller than five participants was accepted.'
  }

  $p0Fixture = Join-Path $tempRoot 'open-p0'
  Copy-Item -LiteralPath $fixture -Destination $p0Fixture -Recurse
  $p0IssuesPath = Join-Path $p0Fixture 'issues.json'
  $p0Issues = Get-Content -LiteralPath $p0IssuesPath -Raw | ConvertFrom-Json -Depth 100
  $p0Issues.issues[0].severity = 'P0'
  $p0Issues | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $p0IssuesPath -Encoding utf8
  & $pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -EvidenceDirectory $p0Fixture -AllowSyntheticFixture *> $null
  if ($LASTEXITCODE -eq 0) {
    throw 'Evidence with an open P0 was accepted.'
  }

  $unownedP1Fixture = Join-Path $tempRoot 'unowned-p1'
  Copy-Item -LiteralPath $fixture -Destination $unownedP1Fixture -Recurse
  $p1IssuesPath = Join-Path $unownedP1Fixture 'issues.json'
  $p1Issues = Get-Content -LiteralPath $p1IssuesPath -Raw | ConvertFrom-Json -Depth 100
  $p1Issues.issues[0].severity = 'P1'
  $p1Issues.issues[0].PSObject.Properties.Remove('dueDate')
  $p1Issues | ConvertTo-Json -Depth 100 | Set-Content -LiteralPath $p1IssuesPath -Encoding utf8
  & $pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -EvidenceDirectory $unownedP1Fixture -AllowSyntheticFixture *> $null
  if ($LASTEXITCODE -eq 0) {
    throw 'An open P1 without a due date was accepted.'
  }

  Write-Output 'Alpha feedback contract tests passed.'
}
finally {
  if ((Test-Path -LiteralPath $tempRoot) -and $tempRoot.StartsWith([System.IO.Path]::GetTempPath(), [System.StringComparison]::OrdinalIgnoreCase)) {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force
  }
}
