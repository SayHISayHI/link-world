[CmdletBinding()]
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$manifestPath = Join-Path $RepoRoot 'src-tauri/Cargo.toml'
$treeOutput = @(& cargo tree --manifest-path $manifestPath -i rsa --prefix none --format '{p}' 2>&1)
if ($LASTEXITCODE -ne 0) {
  $treeOutput | ForEach-Object { Write-Error $_.ToString() }
  exit $LASTEXITCODE
}

$activeRsa = @($treeOutput | Where-Object { $_.ToString() -match '^rsa v' })
if ($activeRsa.Count -gt 0) {
  Write-Error 'RUSTSEC-2023-0071 waiver is invalid because rsa is present in the active Windows dependency graph.'
  exit 1
}

Write-Output 'RUSTSEC-2023-0071 waiver validated: rsa is lockfile-only and absent from the active Windows dependency graph.'
