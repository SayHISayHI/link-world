[CmdletBinding()]
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [string]$OutputDirectory = '',
  [string]$ReadinessReport = '',
  [switch]$AllowDirty
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-JsonFile {
  param([Parameter(Mandatory = $true)][string]$Path)
  return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Get-SignatureStatus {
  param([Parameter(Mandatory = $true)][string]$Path)

  $signature = Get-AuthenticodeSignature -LiteralPath $Path
  if ($signature.Status -eq 'Valid') {
    return 'signed_valid'
  }
  if ($signature.Status -eq 'NotSigned') {
    return 'unsigned_alpha'
  }
  return "signature_$($signature.Status.ToString().ToLowerInvariant())"
}

$repo = (Resolve-Path $RepoRoot).Path
$tauriConfig = Read-JsonFile (Join-Path $repo 'src-tauri/tauri.conf.json')
$migrationFiles = @(Get-ChildItem -LiteralPath (Join-Path $repo 'src-tauri/migrations') -Filter '*.sql' | Sort-Object Name)
$schemaVersion = if ($migrationFiles.Count -eq 0) { 0 } else { [int]($migrationFiles[-1].BaseName.Split('_')[0]) }
$commitSha = ((& git -C $repo rev-parse HEAD) -join '').Trim()
$shortCommit = ((& git -C $repo rev-parse --short=8 HEAD) -join '').Trim()
$branch = ((& git -C $repo rev-parse --abbrev-ref HEAD) -join '').Trim()
$dirtyStatus = (& git -C $repo status --porcelain)
$isDirty = -not [string]::IsNullOrWhiteSpace(($dirtyStatus -join "`n"))

if ($isDirty -and -not $AllowDirty) {
  throw 'Release packaging requires a clean worktree. Commit or explicitly use -AllowDirty for a non-release rehearsal.'
}

$bundleRoot = Join-Path $repo 'src-tauri/target/release/bundle'
$msiCandidates = @(Get-ChildItem -LiteralPath (Join-Path $bundleRoot 'msi') -Filter '*.msi' -File -ErrorAction SilentlyContinue)
$nsisCandidates = @(Get-ChildItem -LiteralPath (Join-Path $bundleRoot 'nsis') -Filter '*-setup.exe' -File -ErrorAction SilentlyContinue)
$cliCandidate = Join-Path $repo 'src-tauri/target/release/link-world-cli.exe'
$cliBuildMetadataPath = Join-Path $repo 'src-tauri/target/release/link-world-cli.build.json'
$cliInstallerScript = Join-Path $repo 'scripts/install-link-world-cli.ps1'
if ($msiCandidates.Count -ne 1 -or $nsisCandidates.Count -ne 1) {
  throw "Expected exactly one MSI and one NSIS artifact; found $($msiCandidates.Count) MSI and $($nsisCandidates.Count) NSIS files."
}
foreach ($candidate in @($msiCandidates[0], $nsisCandidates[0])) {
  if ($candidate.Name -notmatch [regex]::Escape($tauriConfig.version)) {
    throw "Artifact version does not match Tauri version $($tauriConfig.version): $($candidate.Name)"
  }
}
$sourceArtifacts = @(
  @{
    packageType = 'msi'
    path = $msiCandidates[0].FullName
    fileName = "link-world-$($tauriConfig.version)-windows-x64-$shortCommit.msi"
  },
  @{
    packageType = 'nsis'
    path = $nsisCandidates[0].FullName
    fileName = "link-world-$($tauriConfig.version)-windows-x64-$shortCommit-setup.exe"
  },
  @{
    packageType = 'cli'
    path = $cliCandidate
    fileName = 'link-world-cli.exe'
  },
  @{
    packageType = 'cli-installer'
    path = $cliInstallerScript
    fileName = 'install-link-world-cli.ps1'
  }
)

foreach ($artifact in $sourceArtifacts) {
  if (-not (Test-Path -LiteralPath $artifact.path -PathType Leaf)) {
    throw "Missing $($artifact.packageType) artifact. Run the Tauri and CLI release builds first."
  }
}

if (-not (Test-Path -LiteralPath $cliBuildMetadataPath -PathType Leaf)) {
  throw 'Missing CLI build metadata. Run npm run build:cli after the Tauri build.'
}
$cliBuildMetadata = Read-JsonFile $cliBuildMetadataPath
$cliHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $cliCandidate).Hash
if ($cliBuildMetadata.commitSha -ne $commitSha) {
  throw "CLI build metadata commit does not match HEAD: $($cliBuildMetadata.commitSha) != $commitSha"
}
if ($cliBuildMetadata.packageVersion -ne $tauriConfig.version) {
  throw "CLI build metadata version does not match Tauri version $($tauriConfig.version)."
}
if ($cliBuildMetadata.sha256 -ne $cliHash) {
  throw 'CLI binary no longer matches its build metadata. Run npm run build:cli after the Tauri build.'
}
if ([int64]$cliBuildMetadata.bytes -ne (Get-Item -LiteralPath $cliCandidate).Length) {
  throw 'CLI binary size no longer matches its build metadata.'
}

if (-not $OutputDirectory) {
  $stamp = (Get-Date).ToString('yyyyMMdd-HHmmss')
  $OutputDirectory = Join-Path ([System.IO.Path]::GetTempPath()) "link-world-alpha-$($tauriConfig.version)-$shortCommit-$stamp"
}

$outputFullPath = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $outputFullPath) {
  throw "Output directory already exists: $outputFullPath"
}
New-Item -ItemType Directory -Path $outputFullPath | Out-Null

$files = @()
foreach ($artifact in $sourceArtifacts) {
  $fileName = $artifact.fileName
  $destination = Join-Path $outputFullPath $fileName
  Copy-Item -LiteralPath $artifact.path -Destination $destination
  $hash = Get-FileHash -Algorithm SHA256 -LiteralPath $destination
  $file = Get-Item -LiteralPath $destination
  $files += [pscustomobject]@{
    fileName = $fileName
    packageType = $artifact.packageType
    bytes = $file.Length
    sha256 = $hash.Hash
    signatureStatus = Get-SignatureStatus $destination
  }
}

$readiness = $null
if ($ReadinessReport) {
  $reportPath = (Resolve-Path $ReadinessReport).Path
  $report = Read-JsonFile $reportPath
  if ($report.status -ne 'passed') {
    throw 'Readiness report is not passed.'
  }
  if ($report.app.commitSha -ne $commitSha) {
    throw "Readiness report commit does not match HEAD: $($report.app.commitSha) != $commitSha"
  }
  if ($report.app.dirtyWorktree -and -not $AllowDirty) {
    throw 'Readiness report was generated from a dirty worktree.'
  }

  $reportName = 'alpha-readiness.json'
  $reportDestination = Join-Path $outputFullPath $reportName
  Copy-Item -LiteralPath $reportPath -Destination $reportDestination
  $readiness = [pscustomobject]@{
    fileName = $reportName
    status = $report.status
    sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $reportDestination).Hash
  }
}

$signatureStatuses = @($files | Select-Object -ExpandProperty signatureStatus -Unique)
$manifest = [pscustomobject]@{
  schemaVersion = 1
  generatedAt = (Get-Date).ToString('o')
  channel = 'alpha'
  productName = $tauriConfig.productName
  packageVersion = $tauriConfig.version
  schemaMigrationVersion = $schemaVersion
  commitSha = $commitSha
  branch = $branch
  dirtyWorktree = $isDirty
  target = [pscustomobject]@{ os = 'windows'; arch = 'x64' }
  signatureStatus = if ($signatureStatuses.Count -eq 1) { $signatureStatuses[0] } else { 'mixed' }
  files = $files
  cliBuild = [pscustomobject]@{
    metadataSchemaVersion = $cliBuildMetadata.schemaVersion
    sha256 = $cliBuildMetadata.sha256
    bytes = [int64]$cliBuildMetadata.bytes
    dirtyWorktree = [bool]$cliBuildMetadata.dirtyWorktree
  }
  readinessReport = $readiness
  limitations = @(
    'Checksums prove artifact integrity, not publisher identity.',
    'Unsigned Alpha artifacts require an out-of-band trusted checksum.',
    'The CLI is distributed as a separate executable with an explicit user-level install/PATH script; it is not silently added to PATH by the desktop installer.',
    'This manifest does not replace the Windows installation, upgrade, uninstall, Credential Manager, proxy/firewall, non-ASCII profile, Defender, or user-feedback matrices.'
  )
}

$manifestPath = Join-Path $outputFullPath 'release-manifest.json'
[System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 8), [System.Text.UTF8Encoding]::new($false))

$checksumLines = @($files | Sort-Object fileName | ForEach-Object { "$($_.sha256)  $($_.fileName)" })
if ($readiness) {
  $checksumLines += "$($readiness.sha256)  $($readiness.fileName)"
}
$manifestHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $manifestPath).Hash
$checksumLines += "$manifestHash  release-manifest.json"
[System.IO.File]::WriteAllLines((Join-Path $outputFullPath 'SHA256SUMS.txt'), $checksumLines, [System.Text.UTF8Encoding]::new($false))

Write-Output "Alpha release package: $outputFullPath"
Write-Output "Signature status: $($manifest.signatureStatus)"
