[CmdletBinding()]
param(
  [string]$DatabasePath = (Join-Path $HOME '.cargo/advisory-db'),
  [string]$RepositoryUrl = 'https://github.com/RustSec/advisory-db.git'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
  throw 'Git is required to update the RustSec advisory database.'
}

if (-not (Test-Path -LiteralPath $DatabasePath)) {
  $parent = Split-Path -Parent $DatabasePath
  if ($parent) {
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
  }
  & git clone --filter=blob:none $RepositoryUrl $DatabasePath
  if ($LASTEXITCODE -ne 0) {
    throw 'Failed to clone the RustSec advisory database.'
  }
}

if (-not (Test-Path -LiteralPath (Join-Path $DatabasePath '.git'))) {
  throw "RustSec advisory database path is not a Git repository: $DatabasePath"
}

$dirty = (& git -C $DatabasePath status --porcelain)
if (-not [string]::IsNullOrWhiteSpace(($dirty -join "`n"))) {
  throw 'RustSec advisory database has local modifications; refusing to replace them.'
}

$originUrl = ((& git -C $DatabasePath remote get-url origin) -join '').Trim()
if ($LASTEXITCODE -ne 0 -or $originUrl -ne $RepositoryUrl) {
  throw "Unexpected RustSec advisory database origin: $originUrl"
}

& git -C $DatabasePath fetch --depth=1 origin main
if ($LASTEXITCODE -ne 0) {
  throw 'Failed to fetch the RustSec advisory database.'
}

$fetchedCommit = ((& git -C $DatabasePath rev-parse FETCH_HEAD) -join '').Trim()
if ($LASTEXITCODE -ne 0 -or $fetchedCommit -notmatch '^[0-9a-f]{40}$') {
  throw 'RustSec advisory database fetch did not produce a valid commit.'
}

& git -C $DatabasePath checkout --detach $fetchedCommit
if ($LASTEXITCODE -ne 0) {
  throw 'Failed to check out the fetched RustSec advisory database commit.'
}

$checkedOutCommit = ((& git -C $DatabasePath rev-parse HEAD) -join '').Trim()
if ($checkedOutCommit -ne $fetchedCommit) {
  throw 'RustSec advisory database HEAD does not match the fetched commit.'
}

Write-Output "RustSec advisory database updated: $checkedOutCommit"
