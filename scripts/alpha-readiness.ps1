[CmdletBinding()]
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [string]$OutputPath = '',
  [switch]$IncludeSprintGates,
  [switch]$IncludeTauriBuild,
  [switch]$IncludeNetworkAudits,
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
    [Parameter(Mandatory = $true)][string[]]$Command,
    [bool]$Optional = $false
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
  $tailStart = [Math]::Max(0, $outputLines.Count - 100)
  $tail = if ($outputLines.Count -eq 0) { @() } else { $outputLines[$tailStart..($outputLines.Count - 1)] }
  $status = if ($exitCode -eq 0) {
    'passed'
  } elseif ($Optional) {
    'warning'
  } else {
    'failed'
  }

  [pscustomobject]@{
    name = $Name
    command = ($Command -join ' ')
    workingDirectory = '.'
    optional = $Optional
    status = $status
    exitCode = $exitCode
    startedAt = $startedAt.ToString('o')
    finishedAt = $finishedAt.ToString('o')
    durationMs = [int][Math]::Round(($finishedAt - $startedAt).TotalMilliseconds)
    logTail = $tail
    error = $errorText
  }
}

function Read-JsonFile {
  param([Parameter(Mandatory = $true)][string]$Path)

  $raw = Get-Content -LiteralPath $Path -Raw
  return $raw | ConvertFrom-Json
}

$repo = (Resolve-Path $RepoRoot).Path
$cargoManifest = 'src-tauri/Cargo.toml'
$script:ReadinessRedactions = @(
  @{ value = $repo; replacement = '<repo>' },
  @{ value = [Environment]::GetFolderPath('UserProfile'); replacement = '<user-profile>' }
)

if (-not $OutputPath) {
  $stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')
  $OutputPath = Join-Path ([System.IO.Path]::GetTempPath()) "node-tide-alpha-readiness-$stamp.json"
}

$packageJson = Read-JsonFile (Join-Path $repo 'package.json')
$tauriConfig = Read-JsonFile (Join-Path $repo 'src-tauri/tauri.conf.json')
$migrationFiles = @(Get-ChildItem -LiteralPath (Join-Path $repo 'src-tauri/migrations') -Filter '*.sql' | Sort-Object Name)
$schemaVersion = if ($migrationFiles.Count -eq 0) {
  0
} else {
  [int]($migrationFiles[-1].BaseName.Split('_')[0])
}

$commitSha = (& git -C $repo rev-parse HEAD 2>$null)
$branch = (& git -C $repo rev-parse --abbrev-ref HEAD 2>$null)
$dirtyStatus = (& git -C $repo status --porcelain 2>$null)
$isDirty = -not [string]::IsNullOrWhiteSpace(($dirtyStatus -join "`n"))

$steps = @(
  @{
    Name = 'Node.js runtime compatibility'
    WorkingDirectory = $repo
    Command = @(
      'node',
      '-e',
      "const [major, minor] = process.versions.node.split('.').map(Number); const supported = (major === 20 && minor >= 19) || (major === 22 && minor >= 13) || major >= 24; if (!supported) { console.error('Node.js 20.19+, 22.13+, or >=24 is required; found ' + process.version); process.exit(1); }"
    )
  },
  @{
    Name = 'frontend lint'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'lint')
  },
  @{
    Name = 'frontend typecheck'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'typecheck')
  },
  @{
    Name = 'frontend tests'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'test')
  },
  @{
    Name = 'Alpha feedback evidence contract'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'test:alpha-feedback-contract')
  },
  @{
    Name = 'frontend production build'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'build')
  },
  @{
    Name = 'browser E2E smoke'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'test:e2e')
  },
  @{
    Name = 'rustfmt check'
    WorkingDirectory = $repo
    Command = @('cargo', 'fmt', '--manifest-path', $cargoManifest, '--', '--check')
  },
  @{
    Name = 'rust check'
    WorkingDirectory = $repo
    Command = @('cargo', 'check', '--manifest-path', $cargoManifest)
  },
  @{
    Name = 'rust tests'
    WorkingDirectory = $repo
    Command = @('cargo', 'test', '--manifest-path', $cargoManifest, '--', '--test-threads=1')
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

$steps += @{
  Name = 'npm dependency inventory'
  WorkingDirectory = $repo
  Command = @('npm', 'ls', '--omit=dev', '--json')
  Optional = $true
}

$steps += @{
  Name = 'cargo dependency tree'
  WorkingDirectory = $repo
  Command = @('cargo', 'tree', '--manifest-path', $cargoManifest)
  Optional = $true
}

if ($IncludeSprintGates) {
  $steps += @{
    Name = 'Sprint 2 readiness gate'
    WorkingDirectory = $repo
    Command = @('pwsh', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/sprint2-readiness.ps1')
  }
  $steps += @{
    Name = 'Sprint 3 readiness gate'
    WorkingDirectory = $repo
    Command = @('pwsh', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/sprint3-readiness.ps1')
  }
  $steps += @{
    Name = 'Sprint 5 readiness gate'
    WorkingDirectory = $repo
    Command = @('pwsh', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/sprint5-readiness.ps1', '-IncludeFrontend')
  }
  $steps += @{
    Name = 'CLI readiness gate'
    WorkingDirectory = $repo
    Command = @('pwsh', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/cli-readiness.ps1')
  }
}

if ($IncludeTauriBuild) {
  $steps += @{
    Name = 'Tauri package build'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'tauri:build')
  }
  $steps += @{
    Name = 'CLI release build'
    WorkingDirectory = $repo
    Command = @('npm', 'run', 'build:cli')
  }
}

if ($IncludeNetworkAudits) {
  $steps += @{
    Name = 'RustSec waiver validation'
    WorkingDirectory = $repo
    Command = @('pwsh', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', 'scripts/rustsec-waiver-check.ps1')
  }
  $steps += @{
    Name = 'npm audit production dependencies'
    WorkingDirectory = $repo
    Command = @('npm', 'audit', '--omit=dev', '--audit-level=high', '--registry=https://registry.npmjs.org')
  }
  $steps += @{
    Name = 'cargo audit runtime dependencies'
    WorkingDirectory = (Join-Path $repo 'src-tauri')
    Command = @('cargo', 'audit', '--ignore', 'RUSTSEC-2023-0071')
  }
}

$results = @()
foreach ($step in $steps) {
  Write-Host "==> $($step.Name)"
  $optional = if ($step.ContainsKey('Optional')) { [bool]$step.Optional } else { $false }
  $result = Invoke-ReadinessStep -Name $step.Name -WorkingDirectory $step.WorkingDirectory -Command $step.Command -Optional $optional
  $results += $result
  Write-Host "    $($result.status) in $($result.durationMs)ms"
}

$failed = @($results | Where-Object { $_.status -eq 'failed' })
$warnings = @($results | Where-Object { $_.status -eq 'warning' })
$manualEvidenceRequired = @(
  'W9-01 clean Windows 11 install smoke',
  'W9-02 Windows 10 compatibility smoke',
  'W9-03 previous Alpha in-place upgrade with Credential Manager',
  'W9-04 interrupted upgrade/startup convergence',
  'W9-05 uninstall and reinstall with data retained',
  'W9-07 installer source/version/checksum verification',
  'W9-08 provider credential create/edit/delete/restart/upgrade',
  'W9-09 proxy/firewall/offline network matrix',
  'W9-10 non-ASCII Windows user directory',
  'W9-13 security/license review disposition',
  'Week 10 invitation records for 5-15 target users',
  'Week 10 at least one install-to-real-task observation',
  'Week 10 P0/P1 triage and next-stage decision record'
)

$report = [pscustomobject]@{
  schemaVersion = 1
  generatedAt = (Get-Date).ToString('o')
  scope = 'windows-alpha-release-readiness'
  status = if ($failed.Count -eq 0) { 'passed' } else { 'failed' }
  app = [pscustomobject]@{
    packageName = $packageJson.name
    packageVersion = $packageJson.version
    productName = $tauriConfig.productName
    tauriVersion = $tauriConfig.version
    identifier = $tauriConfig.identifier
    schemaVersion = $schemaVersion
    branch = ($branch -join '').Trim()
    commitSha = ($commitSha -join '').Trim()
    dirtyWorktree = $isDirty
  }
  toolchain = [pscustomobject]@{
    nodeVersion = ((& node --version 2>$null) -join '').Trim()
    npmVersion = ((& npm --version 2>$null) -join '').Trim()
  }
  options = [pscustomobject]@{
    sprintGatesIncluded = [bool]$IncludeSprintGates
    tauriBuildIncluded = [bool]$IncludeTauriBuild
    networkAuditsIncluded = [bool]$IncludeNetworkAudits
    clippySkipped = [bool]$SkipClippy
  }
  results = $results
  warnings = $warnings
  manualEvidenceRequired = $manualEvidenceRequired
  limitations = @(
    'This report proves only local static, test, build and dependency-inventory gates that were selected for this run.',
    'It does not prove Windows installer behavior, signed artifact provenance, Credential Manager behavior across installer upgrades, real proxy/firewall/offline behavior, non-ASCII profile behavior, Defender interference, uninstall data retention, or Alpha user feedback.',
    'Week 9 completion still requires docs/windows_alpha_release_matrix.md evidence. Week 10 completion still requires docs/alpha_feedback_playbook.md evidence.'
  )
}

$outputDirectory = Split-Path -Parent $OutputPath
if ($outputDirectory) {
  New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}

$reportJson = $report | ConvertTo-Json -Depth 9
$temporaryOutput = "$OutputPath.tmp"
[System.IO.File]::WriteAllText($temporaryOutput, $reportJson, [System.Text.UTF8Encoding]::new($false))
Move-Item -LiteralPath $temporaryOutput -Destination $OutputPath -Force
Write-Host "Alpha readiness report: $OutputPath"

if ($failed.Count -gt 0) {
  exit 1
}
