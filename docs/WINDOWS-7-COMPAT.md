# DeadSync — Windows 7 compatibility build

This archive contains a `deadsync.exe` built with extra link-time shims that
let it run on Windows 7, Windows 7 Embedded / POSReady 7, and Windows 8.x. It
is intended for users who hit:

> The procedure entry point `ProcessPrng` could not be located in the dynamic
> link library `bcryptprimitives.dll`.

That error is a Rust standard library requirement introduced in Rust 1.78
(May 2024). `ProcessPrng` only exists on Windows 10 1809 and later, so any
modern Rust binary crashes on launch on older systems. This build works
around it by linking against:

- [YY-Thunks](https://github.com/Chuyu-Team/YY-Thunks) — provides a Win7
  implementation of `ProcessPrng` (and other newer APIs) that falls back to
  `RtlGenRandom`.
- [VC-LTL5](https://github.com/Chuyu-Team/VC-LTL5) — redirects the MSVC C
  runtime to the Win7-compatible system CRT so the binary does not need a
  newer `vcruntime140.dll` / `ucrtbase.dll`.

## Requirements

- Windows 7 SP1 / Windows 7 Embedded SP1 / Windows 8 / 8.1 / 10 / 11
- A GPU with a working **Vulkan** or **OpenGL** driver. The Direct3D 12
  backend will be skipped automatically on Windows < 10.
- Same disk space, audio, and input requirements as the standard build.

## Known limitations

- This is a best-effort port. We do not run a Windows 7 CI matrix, so
  regressions can slip in. Please report issues against the upstream
  repository and mention that you are on the Win7-compat build.
- Some optional features that depend on Win10-only APIs may be disabled or
  degraded at runtime.
- Anti-cheat / DRM solutions that hook system DLLs may behave oddly with the
  thunked imports. None ship with DeadSync today, but third-party overlays
  can interfere.

## If it still crashes on launch

1. Make sure Windows 7 SP1 + the Platform Update (KB2670838) are installed.
2. Install the latest Visual C++ runtime redistributables.
3. Update your GPU driver and verify Vulkan / OpenGL is available.
4. Run `deadsync.exe` from a command prompt to capture the error message and
   attach it to a bug report.
