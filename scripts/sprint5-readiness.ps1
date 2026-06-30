[CmdletBinding()]
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [string]$OutputPath = '',
  [switch]$IncludeFrontend,
  [switch]$SkipClippy
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:ReadinessRedactions = @()

function Convert-ReadinessLogLine {
  param([AllowEmptyString()][string]$Line)

  $sanitized = $Line
  foreach ($redaction in $script:ReadinessRedactions) {
    if ($redaction.value) {
      $sanitized = $sanitized.Replace($redaction.value, $redaction.replacement)
    }
  }

  return $sanitized
}

function Invoke-ReadinessStep {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$WorkingDirectory,
    [Parameter(Mandatory = $true)][string[]]$Command
  )

  $startedAt = Get-Date
  $outputLines = @()
  $exitCode = 0
  $errorText = $null

  try {
    $executable = $Command[0]
    $arguments = @()
    if ($Command.Count -gt 1) {
      $arguments = $Command[1..($Command.Count - 1)]
    }

    Push-Location $WorkingDirectory
    try {
      $outputLines = @(& $executable @arguments 2>&1 | ForEach-Object { Convert-ReadinessLogLine $_.ToString() })
      $exitCode = if ($null -eq $global:LASTEXITCODE) { 0 } else { $global:LASTEXITCODE }
    }
    finally {
      Pop-Location
    }
  }
  catch {
    $exitCode = 1
    $errorText = Convert-ReadinessLogLine $_.Exception.Message
    $outputLines += $errorText
  }

  $finishedAt = Get-Date
  $tailStart = [Math]::Max(0, $outputLines.Count - 80)
  $tail = if ($outputLines.Count -eq 0) { @() } else { $outputLines[$tailStart..($outputLines.Count - 1)] }

  [pscustomobject]@{
    name = $Name
    command = ($Command -join ' ')
    workingDirectory = '.'
    status = if ($exitCode -eq 0) { 'passed' } else { 'failed' }
    exitCode = $exitCode
    startedAt = $startedAt.ToString('o')
    finishedAt = $finishedAt.ToString('o')
    durationMs = [int][Math]::Round(($finishedAt - $startedAt).TotalMilliseconds)
    logTail = $tail
    error = $errorText
  }
}

$repo = (Resolve-Path $RepoRoot).Path
$cargoManifest = 'src-tauri/Cargo.toml'
$script:ReadinessRedactions = @(
  @{ value = $repo; replacement = '<repo>' },
  @{ value = [Environment]::GetFolderPath('UserProfile'); replacement = '<user-profile>' }
)

if (-not $OutputPath) {
  $stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')
  $OutputPath = Join-Path ([System.IO.Path]::GetTempPath()) "link-world-sprint5-readiness-$stamp.json"
}

$steps = @(
  @{
    Name = 'rustfmt check'
    WorkingDirectory = $repo
    Command = @('cargo', 'fmt', '--manifest-path', $cargoManifest, '--', '--check')
  },
  @{
    Name = 'local diagnostics health and redaction'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::system::tests')
  },
  @{
    Name = 'bounded structured log validation and rotation'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'telemetry::tests')
  },
  @{
    Name = 'support bundle confirmation privacy and atomic export'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::support_bundle::tests')
  },
  @{
    Name = 'AI enrichment correlation and event redaction'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::ai::tests')
  },
  @{
    Name = 'search maintenance correlation cancellation and redaction'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'search::tests')
  },
  @{
    Name = 'capture lifecycle correlation and sanitized failure evidence'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::capture::tests')
  },
  @{
    Name = 'startup migration correlation and redacted failure evidence'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::migration::tests')
  },
  @{
    Name = 'restore restart correlation rollback and redaction'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::restore::tests')
  }
)

if (-not $SkipClippy) {
  $steps += @{
    Name = 'rust clippy warnings gate'
    WorkingDirectory = $repo
    Command = @(
      'cargo',
      'clippy',
      '--manifest-path',
      $cargoManifest,
      '--all-targets',
      '--',
      '-D',
      'warnings',
      '-A',
      'clippy::needless-return',
      '-A',
      'clippy::unnecessary-map-or'
    )
  }
}

if ($IncludeFrontend) {
  $steps += @{
    Name = 'frontend typecheck'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'typecheck')
  }
  $steps += @{
    Name = 'frontend tests'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'test')
  }
}

$results = @()
foreach ($step in $steps) {
  Write-Host "==> $($step.Name)"
  $result = Invoke-ReadinessStep -Name $step.Name -WorkingDirectory $step.WorkingDirectory -Command $step.Command
  $results += $result
  Write-Host "    $($result.status) in $($result.durationMs)ms"
}

$failed = @($results | Where-Object { $_.status -ne 'passed' })
$report = [pscustomobject]@{
  schemaVersion = 1
  generatedAt = (Get-Date).ToString('o')
  scope = 'sprint5-local-observability-support-readiness'
  frontendIncluded = [bool]$IncludeFrontend
  clippySkipped = [bool]$SkipClippy
  status = if ($failed.Count -eq 0) { 'passed' } else { 'failed' }
  results = $results
  limitations = @(
    'This automation covers deterministic local diagnostics, redaction, bounded log validation and rotation, support bundle privacy, capture/AI/search correlation boundaries, plus startup migration and restore restart/rollback correlation/failure redaction.',
    'It does not prove user-confirmation usability, installed Windows paths, live log rotation under process interruption, large failed-job UI performance, or manual inspection of a release-candidate support bundle.',
    'Structured log coverage includes the planned capture submit/fetch, AI enrichment, search rebuild/reindex, startup migration and restore lifecycles; backup catalog reads, diagnostics reads and portable export are not modeled as correlated lifecycle workflows.'
  )
}

$outputDirectory = Split-Path -Parent $OutputPath
if ($outputDirectory) {
  New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}

$reportJson = $report | ConvertTo-Json -Depth 8
$temporaryOutput = "$OutputPath.tmp"
[System.IO.File]::WriteAllText($temporaryOutput, $reportJson, [System.Text.UTF8Encoding]::new($false))
Move-Item -LiteralPath $temporaryOutput -Destination $OutputPath -Force
Write-Host "Sprint 5 readiness report: $OutputPath"

if ($failed.Count -gt 0) {
  exit 1
}