<#
.SYNOPSIS
  Fast hot-reload watch loop for the deadsync `deadsync-screens` cdylib that
  bypasses cargo's per-edit graph/fingerprint overhead (~0.9s) by re-running the
  exact `rustc` invocation directly on each edit.

.DESCRIPTION
  Measured on this repo (warm target/hot, edit src/screens/menu/render.rs):

      cargo build  -p deadsync-screens --profile hot   ~1.99s / edit
      rustc-direct (link.exe)                          ~1.14s / edit
      rustc-direct + rust-lld                          ~0.74s / edit   (~2.7x)

  The ~0.9s cargo reclaims is pure workspace fingerprinting; the actual codegen
  of render.rs (~0.07s) and the DLL link (link.exe 0.81s / lld ~0.4s) are all
  that remain. This script removes cargo from the loop:

    1. Runs `cargo build -p <crate> --profile <profile> -v` ONCE to (a) bring the
       engine rlib + deps up to date under the chosen RUSTFLAGS and (b) capture
       the exact `rustc` command line cargo uses to build the cdylib.
    2. Optionally appends `-C linker=<bundled rust-lld as lld-link.exe>`.
    3. Watches the cdylib's own sources (crates/deadsync-screens/src/lib.rs and
       the #[path]-included src/screens/menu/render.rs) and, on change, re-runs
       the captured command directly (no cargo), then copies the freshly linked
       deps/deadsync_screens.dll (+ .pdb) up to target/<profile>/ where the host
       reloader watches it.

  CORRECTNESS NOTES
    * BUILD_HASH / LAYOUT_HASH are baked into the ENGINE RLIB (src/hot reads them
      via env! at rlib-compile time from build.rs). The cdylib only references
      those consts, so re-linking it against the same rlib reproduces the exact
      handshake the host expects. This is why rustc-direct is sound.
    * That handshake folds in `-C prefer-dynamic` (DEADSYNC_SHARED_ALLOC). The
      HOST and this loop MUST use identical RUSTFLAGS, or the host rejects the
      cdylib at load (build_hash mismatch). Default here is "-C prefer-dynamic"
      to match the shared-allocator branch. The linker choice is NOT part of the
      hash, so adding lld is safe.
    * Only the two cdylib sources are watched. Editing ENGINE code changes the
      rlib (and the statically-linked host), which this fast loop does NOT
      rebuild -- do a normal `cargo run` + restart the host for engine edits.
    * After this loop, a plain `cargo build` simply re-fingerprints once and
      (re)builds the cdylib normally; nothing is corrupted.

.PARAMETER WorktreeRoot
  Repo root to build in. Defaults to the repo containing this script (its parent dir).

.PARAMETER Rustflags
  RUSTFLAGS for the capture build. MUST match the host. Default "-C prefer-dynamic".

.PARAMETER Profile
  Cargo profile. Default "hot".

.PARAMETER NoLld
  Disable rust-lld and use the default MSVC linker (link.exe).

.EXAMPLE
  # Terminal A (host) -- same RUSTFLAGS as the watcher:
  $env:RUSTFLAGS = "-C prefer-dynamic"
  cargo run --profile hot --bin deadsync --features hot

  # Terminal B (this watcher):
  pwsh -File hot-watch.ps1
#>
[CmdletBinding()]
param(
    [string]$WorktreeRoot = "",
    [string]$Rustflags    = "-C prefer-dynamic",
    [string]$Profile      = "hot",
    [string]$Crate        = "deadsync-screens",
    [switch]$NoLld,
    [int]$PollMs          = 120
)

$ErrorActionPreference = "Continue"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
# Default the worktree root to the repo containing this script (scripts\..).
# Resolved here rather than in the param block because $PSScriptRoot is not
# populated during param default-value evaluation in Windows PowerShell 5.1.
if (-not $WorktreeRoot) { $WorktreeRoot = Split-Path -Parent $scriptDir }

function Resolve-LldLink {
    # Bundled rust-lld, copied to a name lld auto-detects as MSVC ("lld-link").
    $sysroot = (& rustc --print sysroot).Trim()
    $host3   = (& rustc -vV | Select-String '^host:\s*(.+)$').Matches[0].Groups[1].Value.Trim()
    $rustLld = Join-Path $sysroot "lib\rustlib\$host3\bin\rust-lld.exe"
    if (-not (Test-Path $rustLld)) { throw "rust-lld not found at $rustLld" }
    $shim = Join-Path $WorktreeRoot "target\.hot-watch\lld-link.exe"
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $shim) | Out-Null
    if (-not (Test-Path $shim) -or (Get-Item $shim).Length -ne (Get-Item $rustLld).Length) {
        Copy-Item $rustLld $shim -Force
    }
    return $shim
}

function Capture-RustcCmd {
    param([string]$primaryWatch)
    $cn = $Crate -replace '-','_'
    # Iterate native output lines directly. Do NOT use Out-String: it wraps long
    # lines at the host width, splitting the 2000+ char rustc command and breaking
    # the match. Capture whatever is between the backticks (works with or without
    # the sccache rustc-wrapper prefix).
    $find = {
        cargo build -p $Crate --profile $Profile -v 2>&1 | ForEach-Object { "$_" } |
            Where-Object { $_ -match 'Running' -and $_ -match "--crate-name $cn\b" } | Select-Object -First 1
    }
    $line = & $find
    if (-not $line) {
        (Get-Item $primaryWatch).LastWriteTime = Get-Date
        $line = & $find
    }
    if (-not $line) { throw "could not capture rustc command for $Crate (no 'Running ... --crate-name $cn' line)" }
    $m = [regex]::Match($line, 'Running\s+`(.+)`\s*$')
    if (-not $m.Success) { throw "found build line but could not extract command between backticks:`n$line" }
    return $m.Groups[1].Value.Trim()
}

Set-Location $WorktreeRoot
$env:RUSTFLAGS = $Rustflags
# Belt-and-suspenders: reproduce the Cargo compile-time env for the cdylib crate
# so a future env!/option_env! in the hot unit behaves identically under a bare
# rustc rerun (today the hot unit uses none; this guards against regressions).
$crateDir = Join-Path $WorktreeRoot "crates\deadsync-screens"
$env:CARGO_MANIFEST_DIR = $crateDir
$env:CARGO_PKG_NAME     = $Crate
$env:CARGO_CRATE_NAME   = ($Crate -replace '-','_')

$render = Join-Path $WorktreeRoot "src\screens\menu\render.rs"
$cdylibLib = Join-Path $WorktreeRoot "crates\deadsync-screens\src\lib.rs"
$watch = @($render, $cdylibLib) | Where-Object { Test-Path $_ }

$depsDll = Join-Path $WorktreeRoot "target\$Profile\deps\deadsync_screens.dll"
$topDll  = Join-Path $WorktreeRoot "target\$Profile\deadsync_screens.dll"

function Snapshot-File([string]$p) {
    if (-not (Test-Path $p)) { return $null }
    $i = Get-Item $p
    return "$($i.Length):$($i.LastWriteTimeUtc.Ticks)"
}

Write-Host "== deadsync hot-watch ==" -ForegroundColor Cyan
Write-Host "  worktree : $WorktreeRoot"
Write-Host "  RUSTFLAGS: $Rustflags   (the HOST must use the SAME)"
Write-Host "  profile  : $Profile"

$cmd = Capture-RustcCmd -primaryWatch $render

# Fail closed if the scraped command doesn't look like the cdylib build.
foreach ($needle in @('--crate-type cdylib', "--crate-name $($Crate -replace '-','_')", '--out-dir', '--extern deadsync=')) {
    if ($cmd -notmatch [regex]::Escape($needle)) {
        throw "captured rustc command missing '$needle' -- refusing to run a wrong/partial build.`n$cmd"
    }
}
if ($cmd.Length -gt 7000) { throw "captured rustc command is $($cmd.Length) chars; too close to cmd.exe limit -- aborting." }

# Snapshot the engine rlib this cdylib links against. If a concurrent host
# `cargo build` rebuilds it, our direct relink would link a NEWER engine than
# the running host statically contains -> a silently incompatible cdylib. We
# abort instead. (BUILD_HASH is folded from git HEAD, so a dirty-source rebuild
# would NOT bump it; the only safe invariant is "same rlib bytes".)
$rlibMatch = [regex]::Match($cmd, '--extern\s+deadsync="?([^"\s]+\.rlib)"?')
$rlibPath  = if ($rlibMatch.Success) { $rlibMatch.Groups[1].Value } else { $null }
$rlibSnap  = Snapshot-File $rlibPath

# Redirect incremental to a watcher-private dir so we never race Cargo's
# incremental cache (the source of the earlier "Access is denied (os error 5)").
$privInc = Join-Path $WorktreeRoot "target\$Profile\hot-watch-incremental"
New-Item -ItemType Directory -Force -Path $privInc | Out-Null
$cmd = [regex]::Replace($cmd, '-C\s+incremental=("[^"]*"|\S+)', "-C incremental=`"$privInc`"")

if (-not $NoLld) {
    $lld = Resolve-LldLink
    $cmd = "$cmd -C linker=`"$lld`""
    Write-Host "  linker   : rust-lld ($lld)"
} else {
    Write-Host "  linker   : default (link.exe)"
}
if ($rlibPath) { Write-Host "  rlib     : $rlibPath" }
Write-Host "  watching :"; $watch | ForEach-Object { Write-Host "             $_" }
Write-Host ""
Write-Host "  Start the host in another terminal with the SAME RUSTFLAGS:" -ForegroundColor Yellow
Write-Host "      `$env:RUSTFLAGS = `"$Rustflags`"" -ForegroundColor Yellow
Write-Host "      cargo run --profile $Profile --bin deadsync --features hot" -ForegroundColor Yellow
Write-Host "  (prefer-dynamic needs the toolchain's std-*.dll on PATH; `cargo run` handles this." -ForegroundColor DarkGray
Write-Host "   If you launch target\$Profile\deadsync.exe directly, add <sysroot>\bin to PATH.)" -ForegroundColor DarkGray
Write-Host ""

function Invoke-Relink {
    # Guard: the engine rlib must be byte-identical to the one the running host
    # statically links. If it changed (e.g. a concurrent host rebuild), abort --
    # linking a newer engine into the cdylib than the host contains is UB.
    $now = Snapshot-File $rlibPath
    if ($rlibSnap -and $now -ne $rlibSnap) {
        Write-Host ("[{0:HH:mm:ss}] ENGINE RLIB CHANGED -- stop & restart host + watcher (stale-host hazard)" -f (Get-Date)) -ForegroundColor Red
        return
    }
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $out = cmd /c "$cmd 2>&1"
    $code = $LASTEXITCODE
    $sw.Stop()
    if ($code -ne 0) {
        Write-Host ("[{0:HH:mm:ss}] BUILD FAILED ({1:N2}s)" -f (Get-Date), $sw.Elapsed.TotalSeconds) -ForegroundColor Red
        $out | Where-Object { $_ -match 'error' } | Select-Object -First 12 | ForEach-Object { Write-Host "   $_" -ForegroundColor Red }
        return
    }
    # Atomic publish: write a temp sibling then rename over the watched path so
    # the host's poll never observes a half-written dll. (Replicates cargo's
    # deps -> target/<profile>/ copy that the reloader watches.)
    $tmpDll = "$topDll.tmp"
    Copy-Item $depsDll $tmpDll -Force
    $depsPdb = [IO.Path]::ChangeExtension($depsDll, ".pdb")
    if (Test-Path $depsPdb) { Copy-Item $depsPdb ([IO.Path]::ChangeExtension($topDll, ".pdb")) -Force }
    for ($try = 0; $try -lt 5; $try++) {
        try { Move-Item $tmpDll $topDll -Force; break }
        catch { Start-Sleep -Milliseconds 30; if ($try -eq 4) { throw } }
    }
    Write-Host ("[{0:HH:mm:ss}] reloaded in {1:N2}s" -f (Get-Date), $sw.Elapsed.TotalSeconds) -ForegroundColor Green
}

# Prime the top-level dll so the host's first load matches this loop's output.
Invoke-Relink

# Robust polling watcher (no FileSystemWatcher event plumbing).
$last = @{}
foreach ($f in $watch) { $last[$f] = (Get-Item $f).LastWriteTimeUtc }
Write-Host "Watching for edits (Ctrl-C to stop)..." -ForegroundColor Cyan
while ($true) {
    Start-Sleep -Milliseconds $PollMs
    $changed = $false
    foreach ($f in $watch) {
        $t = (Get-Item $f).LastWriteTimeUtc
        if ($t -ne $last[$f]) { $last[$f] = $t; $changed = $true }
    }
    if ($changed) {
        Start-Sleep -Milliseconds 40   # let the editor finish writing
        foreach ($f in $watch) { $last[$f] = (Get-Item $f).LastWriteTimeUtc }
        Invoke-Relink
    }
}
