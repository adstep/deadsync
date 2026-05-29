# DeadSync — Windows 7 compatibility build

This archive contains a `deadsync.exe` built with Rust's Tier 3
`x86_64-win7-windows-msvc` target so it can run on Windows 7 SP1, Windows 7
Embedded / POSReady 7, and Windows 8.x. It is intended for users who hit:

> The procedure entry point `ProcessPrng` could not be located in the dynamic
> link library `bcryptprimitives.dll`.

That error appeared in Rust 1.78 (May 2024) when the standard library
switched to `ProcessPrng`, an API that only exists on Windows 10 1809 and
later. The Win7 target rebuilds the standard library to use `RtlGenRandom`
instead, which is available all the way back to Windows XP SP3.

## Requirements

- Windows 7 SP1 / Windows 7 Embedded SP1 / Windows 8 / 8.1 / 10 / 11
- The [Visual C++ 2015–2022 Redistributable](https://aka.ms/vs/17/release/vc_redist.x64.exe)
- A GPU with a working **Vulkan** or **OpenGL** driver. The Direct3D 12
  backend is not used on Windows < 10.
- Same disk space, audio, and input requirements as the standard build.

## Known limitations

- This is a Tier 3 Rust target, which means it is **not covered by Rust's CI
  guarantees** and is built with a nightly compiler + `-Z build-std`.
  Regressions can slip in between nightly toolchains.
- Some optional features that depend on Windows 10-only APIs may be disabled
  or degraded at runtime.

## If it still crashes on launch

1. Make sure Windows 7 SP1 + the Platform Update (KB2670838) are installed.
2. Install the latest Visual C++ 2015–2022 Redistributable.
3. Update your GPU driver and verify Vulkan or OpenGL is available.
4. Run `deadsync.exe` from a command prompt to capture the error message and
   attach it to a bug report.
