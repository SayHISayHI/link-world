[CmdletBinding()]
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [string]$OutputPath = '',
  [switch]$SkipClippy,
  [switch]$KeepFixture
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:Redactions = @()

function Convert-LogLine {
  param([AllowEmptyString()][string]$Line)

  $sanitized = $Line
  foreach ($redaction in $script:Redactions) {
    if ($redaction.value) {
      $sanitized = $sanitized.Replace($redaction.value, $redaction.replacement)
    }
  }
  return $sanitized
}

function New-StepResult {
  param(
    [string]$Name,
    [datetime]$StartedAt,
    [int]$ExitCode,
    [string[]]$OutputLines,
    [AllowNull()][string]$ErrorText
  )

  $finishedAt = Get-Date
  $tailStart = [Math]::Max(0, $OutputLines.Count - 80)
  $tail = if ($OutputLines.Count -eq 0) { @() } else { $OutputLines[$tailStart..($OutputLines.Count - 1)] }
  return [pscustomobject]@{
    name = $Name
    status = if ($ExitCode -eq 0) { 'passed' } else { 'failed' }
    exitCode = $ExitCode
    startedAt = $StartedAt.ToString('o')
    finishedAt = $finishedAt.ToString('o')
    durationMs = [int][Math]::Round(($finishedAt - $StartedAt).TotalMilliseconds)
    logTail = $tail
    error = $ErrorText
  }
}

function Invoke-CommandStep {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$WorkingDirectory,
    [Parameter(Mandatory = $true)][string[]]$Command
  )

  $startedAt = Get-Date
  $lines = @()
  $exitCode = 0
  $errorText = $null
  try {
    $executable = $Command[0]
    $arguments = if ($Command.Count -gt 1) { $Command[1..($Command.Count - 1)] } else { @() }
    Push-Location $WorkingDirectory
    try {
      $lines = @(& $executable @arguments 2>&1 | ForEach-Object { Convert-LogLine $_.ToString() })
      $exitCode = if ($null -eq $global:LASTEXITCODE) { 0 } else { $global:LASTEXITCODE }
    }
    finally {
      Pop-Location
    }
  }
  catch {
    $exitCode = 1
    $errorText = Convert-LogLine $_.Exception.Message
    $lines += $errorText
  }
  return New-StepResult -Name $Name -StartedAt $startedAt -ExitCode $exitCode -OutputLines $lines -ErrorText $errorText
}

function Invoke-AssertionStep {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][scriptblock]$Body
  )

  $startedAt = Get-Date
  $lines = @()
  $exitCode = 0
  $errorText = $null
  try {
    $lines = @(& $Body | ForEach-Object { Convert-LogLine $_.ToString() })
  }
  catch {
    $exitCode = 1
    $errorText = Convert-LogLine $_.Exception.Message
    $lines += $errorText
  }
  return New-StepResult -Name $Name -StartedAt $startedAt -ExitCode $exitCode -OutputLines $lines -ErrorText $errorText
}

function Invoke-CliJson {
  param(
    [Parameter(Mandatory = $true)][string]$CliPath,
    [Parameter(Mandatory = $true)][string[]]$Arguments,
    [int]$ExpectedExitCode = 0
  )

  $raw = @(& $CliPath --output json @Arguments 2>&1)
  $exitCode = if ($null -eq $global:LASTEXITCODE) { 0 } else { $global:LASTEXITCODE }
  if ($exitCode -ne $ExpectedExitCode) {
    throw "CLI exit code $exitCode did not match expected $ExpectedExitCode."
  }
  if ($raw.Count -ne 1) {
    throw "CLI JSON mode emitted $($raw.Count) lines instead of one JSON document."
  }
  return ($raw[0].ToString() | ConvertFrom-Json)
}

$repo = (Resolve-Path $RepoRoot).Path
$cargoManifest = 'src-tauri/Cargo.toml'
$stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')
$fixtureRoot = Join-Path ([System.IO.Path]::GetTempPath()) "拾海-cli-readiness-$stamp"
$script:Redactions = @(
  @{ value = $repo; replacement = '<repo>' },
  @{ value = [Environment]::GetFolderPath('UserProfile'); replacement = '<user-profile>' },
  @{ value = $fixtureRoot; replacement = '<fixture>' }
)

if (-not $OutputPath) {
  $OutputPath = Join-Path ([System.IO.Path]::GetTempPath()) "node-tide-cli-readiness-$stamp.json"
}

$steps = @(
  @{ Name = 'rustfmt check'; Command = @('cargo', 'fmt', '--manifest-path', $cargoManifest, '--', '--check') },
  @{ Name = 'CLI and all-target compile'; Command = @('cargo', 'check', '--manifest-path', $cargoManifest, '--all-targets') },
  @{ Name = 'CLI parser output and redaction tests'; Command = @('cargo', 'test', '--manifest-path', $cargoManifest, '--lib', 'cli::tests') },
  @{ Name = 'cross-process runtime lock tests'; Command = @('cargo', 'test', '--manifest-path', $cargoManifest, '--lib', 'runtime_lock::tests') },
  @{ Name = 'capture request id identity tests'; Command = @('cargo', 'test', '--manifest-path', $cargoManifest, '--lib', 'capture_request_id_is_idempotent_and_identity_bound') },
  @{ Name = 'AI request id identity tests'; Command = @('cargo', 'test', '--manifest-path', $cargoManifest, '--lib', 'manual_enrichment_request_id_reuses_terminal_operation_and_rejects_cross_object_use') },
  @{ Name = 'portable export privacy and format tests'; Command = @('cargo', 'test', '--manifest-path', $cargoManifest, '--lib', 'services::portable_export::tests') },
  @{ Name = 'CLI debug binary build'; Command = @('cargo', 'build', '--manifest-path', $cargoManifest, '--bin', 'node-tide-cli') }
)

if (-not $SkipClippy) {
  $steps += @{
    Name = 'CLI clippy warnings gate'
    Command = @('cargo', 'clippy', '--manifest-path', $cargoManifest, '--all-targets', '--', '-D', 'warnings')
  }
}

$results = @()
foreach ($step in $steps) {
  Write-Host "==> $($step.Name)"
  $result = Invoke-CommandStep -Name $step.Name -WorkingDirectory $repo -Command $step.Command
  $results += $result
  Write-Host "    $($result.status) in $($result.durationMs)ms"
}

$cliPath = Join-Path $repo 'src-tauri/target/debug/node-tide-cli.exe'
$previousDataDir = [Environment]::GetEnvironmentVariable('NODE_TIDE_DATA_DIR', 'Process')
try {
  New-Item -ItemType Directory -Path $fixtureRoot -Force | Out-Null
  [Environment]::SetEnvironmentVariable('NODE_TIDE_DATA_DIR', $fixtureRoot, 'Process')

  $results += Invoke-AssertionStep -Name 'version help and PowerShell completion avoid data initialization' -Body {
    $version = Invoke-CliJson -CliPath $cliPath -Arguments @('version')
    if (-not $version.ok -or $version.schemaVersion -ne 1 -or $version.command -ne 'version') {
      throw 'Version JSON envelope is invalid.'
    }
    $help = @(& $cliPath --help 2>&1)
    if ($global:LASTEXITCODE -ne 0 -or $help.Count -lt 5) {
      throw 'CLI help generation failed.'
    }
    $completion = @(& $cliPath completion powershell 2>&1)
    if ($global:LASTEXITCODE -ne 0 -or $completion.Count -lt 5) {
      throw 'PowerShell completion generation failed.'
    }
    if ((Test-Path -LiteralPath (Join-Path $fixtureRoot 'link-world.sqlite3')) -or
        (Test-Path -LiteralPath (Join-Path $fixtureRoot 'runtime'))) {
      throw 'Version/help/completion initialized local application data.'
    }
    'version and completion contracts passed'
  }

  $results += Invoke-AssertionStep -Name 'non-ASCII data directory diagnostics and path redaction' -Body {
    $status = Invoke-CliJson -CliPath $cliPath -Arguments @('status')
    $diagnostics = Invoke-CliJson -CliPath $cliPath -Arguments @('diagnostics', 'show')
    $objects = Invoke-CliJson -CliPath $cliPath -Arguments @('object', 'list', '--limit', '10')
    $index = Invoke-CliJson -CliPath $cliPath -Arguments @('search-index', 'check')
    if (-not $status.ok -or -not $diagnostics.ok -or -not $objects.ok -or -not $index.ok) {
      throw 'A read-only CLI command returned a failed envelope.'
    }
    $serialized = @($status, $diagnostics, $objects, $index) | ConvertTo-Json -Depth 20
    if ($serialized.Contains($fixtureRoot) -or $serialized.Contains([Environment]::GetFolderPath('UserProfile'))) {
      throw 'CLI output leaked a local absolute path.'
    }
    if (-not $serialized.Contains('<app-data>')) {
      throw 'Diagnostics did not expose the documented redacted app-data marker.'
    }
    'read-only JSON and path redaction passed'
  }

  $results += Invoke-AssertionStep -Name 'invalid arguments use stable JSON and exit code' -Body {
    $errorEnvelope = Invoke-CliJson -CliPath $cliPath -Arguments @('object', 'list', '--limit', '0') -ExpectedExitCode 2
    if ($errorEnvelope.ok -or $errorEnvelope.error.code -ne 'ERR_INVALID_ARGUMENT') {
      throw 'Invalid argument contract is not stable.'
    }
    'invalid argument contract passed'
  }

  $results += Invoke-AssertionStep -Name 'destructive commands require explicit non-interactive confirmation' -Body {
    $deleteDenied = Invoke-CliJson -CliPath $cliPath -Arguments @('object', 'delete', 'missing-object') -ExpectedExitCode 2
    $supportDenied = Invoke-CliJson -CliPath $cliPath -Arguments @('diagnostics', 'export') -ExpectedExitCode 2
    $exportDenied = Invoke-CliJson -CliPath $cliPath -Arguments @('export', 'library') -ExpectedExitCode 2
    foreach ($denied in @($deleteDenied, $supportDenied, $exportDenied)) {
      if ($denied.ok -or $denied.error.code -ne 'ERR_INVALID_ARGUMENT') {
        throw 'A destructive command bypassed explicit confirmation.'
      }
    }

    $confirmedMissing = Invoke-CliJson -CliPath $cliPath -Arguments @('object', 'delete', 'missing-object', '--yes') -ExpectedExitCode 3
    if ($confirmedMissing.ok -or $confirmedMissing.error.code -ne 'ERR_OBJECT_NOT_FOUND') {
      throw 'The --yes path did not pass confirmation and reach the shared object service.'
    }
    'destructive confirmation contract passed'
  }

  $results += Invoke-AssertionStep -Name 'live runtime lock contention fails closed' -Body {
    $lockPath = Join-Path $fixtureRoot 'runtime/link-world.lock'
    $stream = [System.IO.File]::Open(
      $lockPath,
      [System.IO.FileMode]::OpenOrCreate,
      [System.IO.FileAccess]::ReadWrite,
      [System.IO.FileShare]::None
    )
    try {
      $busy = Invoke-CliJson -CliPath $cliPath -Arguments @('status') -ExpectedExitCode 5
      if ($busy.ok -or $busy.error.code -ne 'ERR_RUNTIME_BUSY') {
        throw 'Runtime contention did not return ERR_RUNTIME_BUSY.'
      }
    }
    finally {
      $stream.Dispose()
    }
    'runtime contention contract passed'
  }

  $results += Invoke-AssertionStep -Name 'empty-library export and backup lifecycle' -Body {
    $export = Invoke-CliJson -CliPath $cliPath -Arguments @('--quiet', 'export', 'library', '--format', 'json', '--yes')
    if (-not $export.ok -or $export.data.format -ne 'json_directory') {
      throw 'Portable JSON export failed.'
    }
    $backup = Invoke-CliJson -CliPath $cliPath -Arguments @('backup', 'create')
    $listed = Invoke-CliJson -CliPath $cliPath -Arguments @('backup', 'list')
    $verified = Invoke-CliJson -CliPath $cliPath -Arguments @('backup', 'verify', $backup.data.backupId)
    if (-not $backup.ok -or -not $listed.ok -or -not $verified.data.valid) {
      throw 'Backup create/list/verify lifecycle failed.'
    }
    $serialized = @($export, $backup, $listed, $verified) | ConvertTo-Json -Depth 20
    if ($serialized.Contains($fixtureRoot)) {
      throw 'Maintenance output leaked its absolute fixture path.'
    }
    'export and backup lifecycle passed'
  }

  $results += Invoke-AssertionStep -Name 'user-level CLI install and removal script' -Body {
    $installer = Join-Path $repo 'scripts/install-node-tide-cli.ps1'
    $installDirectory = Join-Path $fixtureRoot 'cli-install'
    & pwsh -NoProfile -ExecutionPolicy Bypass -File $installer -SourcePath $cliPath -InstallDirectory $installDirectory | Out-Null
    if ($global:LASTEXITCODE -ne 0) {
      throw 'CLI installation script failed.'
    }
    $installedCli = Join-Path $installDirectory 'node-tide-cli.exe'
    $version = Invoke-CliJson -CliPath $installedCli -Arguments @('version')
    if (-not $version.ok) {
      throw 'Installed CLI did not run.'
    }
    & pwsh -NoProfile -ExecutionPolicy Bypass -File $installer -InstallDirectory $installDirectory -Remove | Out-Null
    if ($global:LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath $installedCli)) {
      throw 'CLI removal script failed.'
    }
    'CLI install and removal passed'
  }
}
finally {
  [Environment]::SetEnvironmentVariable('NODE_TIDE_DATA_DIR', $previousDataDir, 'Process')
  if (-not $KeepFixture -and (Test-Path -LiteralPath $fixtureRoot)) {
    $resolvedFixture = [System.IO.Path]::GetFullPath($fixtureRoot)
    $resolvedTemp = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if (-not $resolvedFixture.StartsWith($resolvedTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
      throw "Refusing to remove readiness fixture outside the system temp directory: $resolvedFixture"
    }
    Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
  }
}

$failed = @($results | Where-Object { $_.status -ne 'passed' })
$report = [pscustomobject]@{
  schemaVersion = 1
  generatedAt = (Get-Date).ToString('o')
  scope = 'node-tide-cli-readiness'
  status = if ($failed.Count -eq 0) { 'passed' } else { 'failed' }
  clippySkipped = [bool]$SkipClippy
  results = $results
  limitations = @(
    'This gate validates the debug CLI binary, stable JSON/exit contracts, non-ASCII local paths, privacy redaction, live lock contention, portable export, backup lifecycle and targeted service idempotency.',
    'It does not replace signed release-binary inspection, Windows installer/PATH testing, proxy/firewall capture tests, live model-provider tests, forced process termination, or Defender validation.'
  )
}

$outputDirectory = Split-Path -Parent $OutputPath
if ($outputDirectory) {
  New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}
$temporaryOutput = "$OutputPath.tmp"
[System.IO.File]::WriteAllText($temporaryOutput, ($report | ConvertTo-Json -Depth 10), [System.Text.UTF8Encoding]::new($false))
Move-Item -LiteralPath $temporaryOutput -Destination $OutputPath -Force
Write-Host "CLI readiness report: $OutputPath"

if ($failed.Count -gt 0) {
  exit 1
}
