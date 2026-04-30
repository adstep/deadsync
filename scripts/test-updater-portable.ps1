<#
.SYNOPSIS
  Build & stage a single deadsync version for in-app updater testing.

.DESCRIPTION
  Builds *one* release-mode binary at -Version, lays it out as a portable
  install, and packages a matching .zip into the staging dir so it can be
  used as either the install target or the source for the
  DEADSYNC_UPDATER_FAKE_DOWNLOAD shortcut.

  No HTTP server is started.  Point the running binary at a real GitHub
  release on your fork via DEADSYNC_UPDATER_RELEASE_URL, and (optionally)
  use DEADSYNC_UPDATER_FAKE_DOWNLOAD to skip the network on the download
  step.

.PARAMETER Version
  Cargo.toml version to build at.  The resulting binary will be staged as
  a portable install plus packaged into a zip named
  `deadsync-v<Version>-x86_64-windows.zip`.

.PARAMETER Stage
  Staging directory.  Wiped and recreated.  Defaults to
  $env:TEMP\deadsync-updater-e2e.

.EXAMPLE
  .\scripts\test-updater-portable.ps1 -Version 0.3.876
#>
param(
  [Parameter(Mandatory)] [string] $Version,
  [string] $Stage = "$env:TEMP\deadsync-updater-e2e"
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

$originalToml = $null
try {
  $cargoToml = Join-Path $repoRoot 'Cargo.toml'
  $originalToml = Get-Content $cargoToml -Raw

  Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $Stage | Out-Null

  $installDir  = Join-Path $Stage 'install'
  $archiveDir  = Join-Path $Stage 'archive'
  $payloadDir  = Join-Path $archiveDir 'deadsync'
  New-Item -ItemType Directory -Path $installDir, $payloadDir | Out-Null

  Write-Host "==> Building deadsync v$Version (release)" -ForegroundColor Cyan
  $patched = $originalToml -replace '(?m)^version\s*=\s*"[^"]*"', "version = `"$Version`""
  Set-Content -Path $cargoToml -Value $patched -NoNewline
  # Touch main.rs so cargo invalidates the binary even if only Cargo.toml
  # changed (incremental builds don't always rebuild on a bare version bump).
  (Get-Item 'src\main.rs').LastWriteTime = Get-Date
  Remove-Item 'target\release\deadsync.exe' -ErrorAction SilentlyContinue
  cargo build --release --quiet
  if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed at version $Version" }
  if (-not (Test-Path 'target\release\deadsync.exe')) {
    throw "cargo build --release at version $Version did not produce target\release\deadsync.exe"
  }
  # Restore Cargo.toml immediately so the source tree is clean for editing.
  Set-Content -Path $cargoToml -Value $originalToml -NoNewline

  function Lay-Out-Portable {
    param([string] $Dir)
    Copy-Item 'target\release\deadsync.exe' $Dir -Force
    foreach ($d in 'assets','songs','courses') {
      if (Test-Path $d) { Copy-Item -Recurse -Force $d $Dir }
    }
    foreach ($f in 'README.md','LICENSE') {
      if (Test-Path $f) { Copy-Item -Force $f $Dir }
    }
    New-Item -ItemType File -Force -Path (Join-Path $Dir 'portable.txt') | Out-Null
  }

  Lay-Out-Portable -Dir $installDir
  Lay-Out-Portable -Dir $payloadDir

  $tag         = "v$Version"
  $archiveName = "deadsync-$tag-x86_64-windows.zip"
  $archivePath = Join-Path $archiveDir $archiveName
  Compress-Archive -Path $payloadDir -DestinationPath $archivePath -Force

  Write-Host ""
  Write-Host "Stage ready." -ForegroundColor Green
  Write-Host ""
  Write-Host "  Install dir : $installDir"
  Write-Host "  Archive     : $archivePath"
  Write-Host "  Built tag   : $tag"
  Write-Host ""
  Write-Host "Point the binary at a real GitHub release on your fork:" -ForegroundColor Yellow
  Write-Host "  `$env:DEADSYNC_UPDATER_RELEASE_URL = 'https://api.github.com/repos/<owner>/deadsync/releases/latest'"
  Write-Host "  `$env:DEADSYNC_VERSION_OVERRIDE   = '<an older semver>'"
  Write-Host ""
  Write-Host "Skip the network download (use the staged archive locally):" -ForegroundColor Yellow
  Write-Host "  `$env:DEADSYNC_UPDATER_FAKE_DOWNLOAD      = '$archivePath'"
  Write-Host "  `$env:DEADSYNC_UPDATER_FAKE_DOWNLOAD_SECS = '5'"
  Write-Host ""
  Write-Host "Then:"
  Write-Host "  cd '$installDir'; .\deadsync.exe"
}
finally {
  if ($null -ne $originalToml) {
    Set-Content -Path $cargoToml -Value $originalToml -NoNewline
  }
  Pop-Location
}
