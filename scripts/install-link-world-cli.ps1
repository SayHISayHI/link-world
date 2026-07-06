[CmdletBinding(SupportsShouldProcess = $true)]
param(
  [string]$SourcePath = (Join-Path $PSScriptRoot 'link-world-cli.exe'),
  [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA 'LinkWorld\cli'),
  [switch]$AddToPath,
  [switch]$Remove
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-NormalizedPathEntry {
  param([Parameter(Mandatory = $true)][string]$Path)
  return [System.IO.Path]::GetFullPath($Path).TrimEnd('\')
}

function Update-UserPath {
  param(
    [Parameter(Mandatory = $true)][string]$Directory,
    [Parameter(Mandatory = $true)][bool]$Present
  )

  $normalized = Get-NormalizedPathEntry $Directory
  $current = [Environment]::GetEnvironmentVariable('Path', 'User')
  $entries = @(
    $current -split ';' |
      Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
      ForEach-Object { $_.Trim() }
  )
  $filtered = @($entries | Where-Object {
      try {
        (Get-NormalizedPathEntry $_) -ne $normalized
      }
      catch {
        $_ -ne $Directory
      }
    })
  if ($Present) {
    $filtered += $normalized
  }
  [Environment]::SetEnvironmentVariable('Path', ($filtered -join ';'), 'User')
}

$installRoot = Get-NormalizedPathEntry $InstallDirectory
$destination = Join-Path $installRoot 'link-world-cli.exe'

if ($Remove) {
  if (-not $PSCmdlet.ShouldProcess($destination, 'Remove Link World CLI')) {
    exit 0
  }
  if (Test-Path -LiteralPath $destination -PathType Leaf) {
    Remove-Item -LiteralPath $destination -Force
  }
  Update-UserPath -Directory $installRoot -Present $false
  if (Test-Path -LiteralPath $installRoot -PathType Container) {
    $remaining = @(Get-ChildItem -LiteralPath $installRoot -Force)
    if ($remaining.Count -eq 0) {
      Remove-Item -LiteralPath $installRoot -Force
    }
  }
  Write-Output "Link World CLI removed from: $destination"
  Write-Output 'Open a new terminal for PATH changes to take effect.'
  exit 0
}

$source = [System.IO.Path]::GetFullPath($SourcePath)
if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
  throw "CLI executable does not exist: $source"
}
if ([System.IO.Path]::GetFileName($source) -ne 'link-world-cli.exe') {
  throw 'SourcePath must point to link-world-cli.exe.'
}

if (-not $PSCmdlet.ShouldProcess($destination, 'Install Link World CLI')) {
  exit 0
}
New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
Copy-Item -LiteralPath $source -Destination $destination -Force
if ($AddToPath) {
  Update-UserPath -Directory $installRoot -Present $true
}

$hash = Get-FileHash -Algorithm SHA256 -LiteralPath $destination
Write-Output "Link World CLI installed: $destination"
Write-Output "SHA-256: $($hash.Hash)"
if ($AddToPath) {
  Write-Output 'User PATH updated. Open a new terminal before running link-world-cli.'
} else {
  Write-Output 'PATH was not changed. Re-run with -AddToPath to enable command discovery.'
}
