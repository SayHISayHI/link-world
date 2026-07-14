[CmdletBinding()]
param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-HostTarget {
  $os = if ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
    'windows'
  } elseif ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::OSX)) {
    'macos'
  } elseif ([System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Linux)) {
    'linux'
  } else {
    throw 'Unsupported release host OS.'
  }

  $arch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
    'X64' { 'x64' }
    'Arm64' { 'arm64' }
    default { [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant() }
  }

  [pscustomobject]@{
    os = $os
    arch = $arch
    target = "$os-$arch"
    executableSuffix = if ($os -eq 'windows') { '.exe' } else { '' }
  }
}

$repo = (Resolve-Path $RepoRoot).Path
$manifestPath = Join-Path $repo 'src-tauri/Cargo.toml'
$tauriConfig = Get-Content -LiteralPath (Join-Path $repo 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
$target = Get-HostTarget
& cargo build --manifest-path $manifestPath --release --bin node-tide-cli
if ($LASTEXITCODE -ne 0) {
  throw "CLI release build failed with exit code $LASTEXITCODE."
}

$binaryName = "node-tide-cli$($target.executableSuffix)"
$binaryPath = Join-Path $repo "src-tauri/target/release/$binaryName"
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
  throw "CLI release build did not produce: $binaryPath"
}

$commitSha = ((& git -C $repo rev-parse HEAD) -join '').Trim()
$dirtyStatus = (& git -C $repo status --porcelain)
$isDirty = -not [string]::IsNullOrWhiteSpace(($dirtyStatus -join "`n"))
$file = Get-Item -LiteralPath $binaryPath
$hash = Get-FileHash -Algorithm SHA256 -LiteralPath $binaryPath
$metadata = [pscustomobject]@{
  schemaVersion = 1
  generatedAt = (Get-Date).ToString('o')
  commitSha = $commitSha
  dirtyWorktree = $isDirty
  packageVersion = $tauriConfig.version
  target = $target.target
  fileName = $binaryName
  bytes = $file.Length
  sha256 = $hash.Hash
}

$metadataPath = Join-Path $repo 'src-tauri/target/release/node-tide-cli.build.json'
$temporaryPath = "$metadataPath.tmp"
[System.IO.File]::WriteAllText($temporaryPath, ($metadata | ConvertTo-Json -Depth 4), [System.Text.UTF8Encoding]::new($false))
Move-Item -LiteralPath $temporaryPath -Destination $metadataPath -Force
Write-Output "CLI release binary: $binaryPath"
Write-Output "CLI build metadata: $metadataPath"
