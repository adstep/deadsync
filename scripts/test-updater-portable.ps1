<#
.SYNOPSIS
  End-to-end smoke test for the in-app updater (Windows portable mode).

.DESCRIPTION
  Builds two release-mode binaries with different versions, packages them
  into separate portable-layout directories, computes a SHA-256 sidecar for
  the "new" archive, and writes a release.json that mimics the GitHub API
  response.  Then starts a static HTTP server in a background job and prints
  the env var + command needed to launch the "old" binary so it sees the
  "new" release as an update.

.PARAMETER OldVersion
  Cargo.toml version to build the *running* (old) binary at.

.PARAMETER NewVersion
  Cargo.toml version to build the *upgraded* (new) binary at.

.PARAMETER Stage
  Working directory.  Wiped on each run.

.EXAMPLE
  .\scripts\test-updater-portable.ps1 -OldVersion 0.3.870 -NewVersion 0.3.875
#>
param(
  [Parameter(Mandatory)] [string] $OldVersion,
  [Parameter(Mandatory)] [string] $NewVersion,
  [string] $Stage = "$env:TEMP\deadsync-updater-e2e",
  [int]    $Port  = 8765
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
  $cargoToml = Join-Path $repoRoot 'Cargo.toml'
  $originalToml = Get-Content $cargoToml -Raw

  function Build-At {
    param([string] $Version, [string] $OutputDir)
    Write-Host "==> Building deadsync v$Version" -ForegroundColor Cyan
    $patched = $originalToml -replace '(?m)^version\s*=\s*"[^"]*"', "version = `"$Version`""
    Set-Content -Path $cargoToml -Value $patched -NoNewline
    cargo build --release --quiet
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed at version $Version" }
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
    Copy-Item 'target\release\deadsync.exe' $OutputDir -Force
    foreach ($d in 'assets','songs','courses') {
      if (Test-Path $d) { Copy-Item -Recurse -Force $d $OutputDir }
    }
    foreach ($f in 'README.md','LICENSE') {
      if (Test-Path $f) { Copy-Item -Force $f $OutputDir }
    }
    New-Item -ItemType File -Force -Path (Join-Path $OutputDir 'portable.txt') | Out-Null
  }

  Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $Stage | Out-Null

  $installDir  = Join-Path $Stage 'install'
  $serverDir   = Join-Path $Stage 'server'
  $newStageDir = Join-Path $Stage 'new-stage'
  New-Item -ItemType Directory -Path $serverDir, $newStageDir | Out-Null

  # 1. Build the OLD binary into the install dir.
  Build-At -Version $OldVersion -OutputDir $installDir

  # 2. Build the NEW binary into a staging dir, then zip it.
  Build-At -Version $NewVersion -OutputDir (Join-Path $newStageDir 'deadsync')
  $arch        = 'x86_64'
  $newTag      = "v$NewVersion"
  $archiveName = "deadsync-$newTag-$arch-windows.zip"
  $archivePath = Join-Path $serverDir $archiveName
  Compress-Archive -Path (Join-Path $newStageDir 'deadsync') `
                   -DestinationPath $archivePath -Force

  # 3. SHA-256 sidecar (GNU coreutils format).
  $hash = (Get-FileHash -Path $archivePath -Algorithm SHA256).Hash.ToLower()
  Set-Content -Path "$archivePath.sha256" -Value "$hash  $archiveName`n" -NoNewline

  # 4. Synthetic release.json that mimics the GitHub API shape.
  $size = (Get-Item $archivePath).Length
  $base = "http://localhost:$Port"
  $json = [pscustomobject]@{
    tag_name     = $newTag
    html_url     = "$base/release.html"
    body         = "Local-fixture release for e2e testing."
    published_at = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    assets       = @(
      [pscustomobject]@{
        name                 = $archiveName
        browser_download_url = "$base/$archiveName"
        size                 = $size
      },
      [pscustomobject]@{
        name                 = "$archiveName.sha256"
        browser_download_url = "$base/$archiveName.sha256"
        size                 = (Get-Item "$archivePath.sha256").Length
      }
    )
  } | ConvertTo-Json -Depth 6
  Set-Content -Path (Join-Path $serverDir 'release.json') -Value $json -NoNewline

  # 5. Restore the original Cargo.toml so the source tree is left clean.
  Set-Content -Path $cargoToml -Value $originalToml -NoNewline

  # 6. Spin up a quick static server.  Prefer python; fall back to a
  #    PowerShell HttpListener if python is missing.
  Write-Host "==> Starting static server on $base (serving $serverDir)" -ForegroundColor Cyan
  $py = Get-Command python -ErrorAction SilentlyContinue
  if ($py) {
    $serverJob = Start-Job -ArgumentList $serverDir, $Port -ScriptBlock {
      param($dir, $port)
      Set-Location $dir
      & python -m http.server $port
    }
  } else {
    throw "python not found on PATH; install Python or run a static file server in $serverDir on port $Port manually."
  }
  Start-Sleep -Seconds 1

  Write-Host ""
  Write-Host "Fixture ready." -ForegroundColor Green
  Write-Host ""
  Write-Host "  Install dir : $installDir"
  Write-Host "  Server dir  : $serverDir  -> $base"
  Write-Host "  Server job  : $($serverJob.Id)  (Stop-Job $($serverJob.Id); Remove-Job $($serverJob.Id))"
  Write-Host ""
  Write-Host "Run the OLD binary against the local 'release':" -ForegroundColor Yellow
  Write-Host "  `$env:DEADSYNC_UPDATER_RELEASE_URL = '$base/release.json'"
  Write-Host "  cd '$installDir'; .\deadsync.exe"
  Write-Host ""
  Write-Host "On the menu: banner appears within ~1s.  Press Start to confirm,"
  Write-Host "Start again to download, Start once more in 'Ready' to apply."
  Write-Host "After relaunch, the menu version line should read v$NewVersion."
}
finally {
  # Always restore Cargo.toml even on failure.
  if ($null -ne $originalToml) {
    Set-Content -Path $cargoToml -Value $originalToml -NoNewline
  }
  Pop-Location
}
