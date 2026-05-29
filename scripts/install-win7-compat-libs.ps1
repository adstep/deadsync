#Requires -Version 5.1
<#
.SYNOPSIS
    Downloads and extracts Chuyu-Team's YY-Thunks and VC-LTL5 binaries so that
    deadsync can be built against the Windows 7 system ABI.

.DESCRIPTION
    The Rust standard library imports `ProcessPrng` from `bcryptprimitives.dll`
    which does not exist on Windows 7 / Windows 7 Embedded. Linking against the
    matching YY-Thunks object file plus VC-LTL5 redirects the missing imports
    to APIs that do exist on Win7 (RtlGenRandom et al), letting the executable
    actually start on those systems.

    This script writes the resolved paths to GITHUB_OUTPUT (when running inside
    a GitHub Actions job) so the build step can forward them to cargo via the
    DEADSYNC_WIN7_THUNK_OBJ and DEADSYNC_WIN7_VC_LTL_LIB_DIR environment
    variables that build.rs looks for.

.PARAMETER Arch
    Target architecture. Supported values: x86_64 (default), x86.

.PARAMETER OutputDir
    Where to place the extracted binaries. Defaults to a `vendor\win7-compat`
    directory under the current working directory.

.PARAMETER YYThunksVersion
    Version of YY-Thunks to download. Defaults to a known-good release that
    ships a Win7 object compatible with Rust >= 1.78.

.PARAMETER VcLtlVersion
    Version of VC-LTL5 to download.
#>

param(
    [ValidateSet('x86_64', 'x86')]
    [string]$Arch = 'x86_64',

    [string]$OutputDir,

    [string]$YYThunksVersion = '1.1.7',

    [string]$VcLtlVersion = '5.2.2'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Resolve-SevenZip {
    $candidate = Get-Command 7z.exe -ErrorAction SilentlyContinue
    if ($candidate) { return $candidate.Source }

    $defaultPaths = @(
        'C:\Program Files\7-Zip\7z.exe',
        'C:\Program Files (x86)\7-Zip\7z.exe'
    )
    foreach ($p in $defaultPaths) {
        if (Test-Path $p) { return $p }
    }
    Write-Error '7z.exe not found. Install 7-Zip or add it to PATH.'
    exit 1
}

function Download-File {
    param(
        [Parameter(Mandatory)] [string]$Url,
        [Parameter(Mandatory)] [string]$Destination
    )
    Write-Host "Downloading $Url"
    $tmp = "$Destination.partial"
    if (Test-Path $tmp) { Remove-Item $tmp -Force }
    Invoke-WebRequest -Uri $Url -OutFile $tmp -UseBasicParsing
    Move-Item -Force $tmp $Destination
}

if (-not $OutputDir) {
    $OutputDir = Join-Path (Get-Location) 'vendor\win7-compat'
}
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

$sevenZip = Resolve-SevenZip

# ---------------------------------------------------------------------------
# YY-Thunks: provides Win7 implementations of newer APIs (ProcessPrng, etc.)
# ---------------------------------------------------------------------------
$yyThunksZipUrl  = "https://github.com/Chuyu-Team/YY-Thunks/releases/download/v$YYThunksVersion/YY-Thunks-Objs.zip"
$yyThunksZipPath = Join-Path $OutputDir "YY-Thunks-$YYThunksVersion-Objs.zip"
$yyThunksDir     = Join-Path $OutputDir "YY-Thunks-$YYThunksVersion"

if (-not (Test-Path $yyThunksDir)) {
    if (-not (Test-Path $yyThunksZipPath)) {
        Download-File -Url $yyThunksZipUrl -Destination $yyThunksZipPath
    }
    Write-Host "Extracting YY-Thunks to $yyThunksDir"
    & $sevenZip x -aoa "-o$yyThunksDir" $yyThunksZipPath | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to extract $yyThunksZipPath"
        exit 1
    }
}

$yyArch = if ($Arch -eq 'x86') { 'x86' } else { 'x64' }
$yyThunkObj = Join-Path $yyThunksDir "objs\$yyArch\YY_Thunks_for_Win7.obj"
if (-not (Test-Path $yyThunkObj)) {
    Write-Error "Missing expected YY-Thunks object: $yyThunkObj"
    exit 1
}

# ---------------------------------------------------------------------------
# VC-LTL5: redirects the MSVC C runtime to the Win7-compatible system CRT.
# Provides the `lib\<arch>` directory rustc/link.exe needs to find first.
# ---------------------------------------------------------------------------
$vcLtlArchiveUrl = "https://github.com/Chuyu-Team/VC-LTL5/releases/download/v$VcLtlVersion/VC-LTL-Binary.7z"
$vcLtlArchive    = Join-Path $OutputDir "VC-LTL-$VcLtlVersion-Binary.7z"
$vcLtlDir        = Join-Path $OutputDir "VC-LTL-$VcLtlVersion"

if (-not (Test-Path $vcLtlDir)) {
    if (-not (Test-Path $vcLtlArchive)) {
        Download-File -Url $vcLtlArchiveUrl -Destination $vcLtlArchive
    }
    Write-Host "Extracting VC-LTL5 to $vcLtlDir"
    & $sevenZip x -aoa "-o$vcLtlDir" $vcLtlArchive | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to extract $vcLtlArchive"
        exit 1
    }
}

# Win7 (and Vista) share the 6.0.6000.0 platform target in VC-LTL5.
$vcLtlArch    = if ($Arch -eq 'x86') { 'Win32' } else { 'x64' }
$vcLtlLibDir  = Join-Path $vcLtlDir "TargetPlatform\6.0.6000.0\lib\$vcLtlArch"
if (-not (Test-Path $vcLtlLibDir)) {
    Write-Error "Missing expected VC-LTL5 lib directory: $vcLtlLibDir"
    exit 1
}

Write-Host ''
Write-Host "YY-Thunks object  : $yyThunkObj"
Write-Host "VC-LTL5 lib dir   : $vcLtlLibDir"

if ($env:GITHUB_OUTPUT) {
    "thunk_obj=$yyThunkObj"     | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
    "vc_ltl_lib_dir=$vcLtlLibDir" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
}

if ($env:GITHUB_ENV) {
    "DEADSYNC_WIN7_THUNK_OBJ=$yyThunkObj"        | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
    "DEADSYNC_WIN7_VC_LTL_LIB_DIR=$vcLtlLibDir"  | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
}
