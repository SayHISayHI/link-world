[CmdletBinding()]
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [string]$OutputPath = '',
  [switch]$IncludeFrontend,
  [switch]$SkipClippy
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

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
      $outputLines = @(& $executable @arguments 2>&1 | ForEach-Object { $_.ToString() })
      $exitCode = if ($null -eq $global:LASTEXITCODE) { 0 } else { $global:LASTEXITCODE }
    }
    finally {
      Pop-Location
    }
  }
  catch {
    $exitCode = 1
    $errorText = $_.Exception.Message
    $outputLines += $errorText
  }

  $finishedAt = Get-Date
  $tailStart = [Math]::Max(0, $outputLines.Count - 80)
  $tail = if ($outputLines.Count -eq 0) { @() } else { $outputLines[$tailStart..($outputLines.Count - 1)] }

  [pscustomobject]@{
    name = $Name
    command = ($Command -join ' ')
    workingDirectory = $WorkingDirectory
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
$cargoManifest = Join-Path $repo 'src-tauri/Cargo.toml'

if (-not $OutputPath) {
  $stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')
  $OutputPath = Join-Path ([System.IO.Path]::GetTempPath()) "node-tide-sprint3-readiness-$stamp.json"
}

$steps = @(
  @{
    Name = 'rustfmt check'
    WorkingDirectory = $repo
    Command = @('cargo', 'fmt', '--manifest-path', $cargoManifest, '--', '--check')
  },
  @{
    Name = 'capture parsing failure isolation and redaction'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::capture::tests')
  },
  @{
    Name = 'startup background job convergence'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'repositories::jobs::tests')
  },
  @{
    Name = 'AI job failure classification'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, 'services::ai::tests')
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
  repoRoot = $repo
  scope = 'sprint3-capture-background-job-readiness'
  frontendIncluded = [bool]$IncludeFrontend
  clippySkipped = [bool]$SkipClippy
  status = if ($failed.Count -eq 0) { 'passed' } else { 'failed' }
  results = $results
  limitations = @(
    'This automation covers deterministic capture parsing, failure classification, secret redaction, job isolation, startup convergence and AI failure mapping.',
    'Real offline and DNS failures, external HTTP behavior, Windows process termination and concurrent capture still require the manual Sprint 3 fault matrix.'
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
Write-Host "Sprint 3 readiness report: $OutputPath"

if ($failed.Count -gt 0) {
  exit 1
}
