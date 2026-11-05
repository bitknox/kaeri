# Building the Windows app

## libmpv

mpv's releases ship `mpv.exe` statically linked, with no libmpv DLL, and the app needs mpv
patched for skip detection's demuxer audio tap anyway, so Kaeri builds its own libmpv for Windows,
the way mpv's own development builds are made.

- `desktop/windows/scripts/libmpv-build.sh` runs mpv's `ci/build-mingw64-full.sh`, at the commit
  of the development build the app used before, in BtbN's FFmpeg-Builds image for the target
  (`win64-lgpl-9.0`, `winarm64-lgpl-9.0`, pinned by digest), on the release mpv and FFmpeg
  archives Android pins, with the patches in `android/mpv/patches/mpv`.
  `desktop/windows/libmpv/` holds its changes to mpv's script, the mujs wrap, and fixes from mpv's
  master and FFmpeg's release branch that the images' toolchain needs. It builds on Linux with
  Docker, in 10 to 15 minutes per architecture on a GitHub runner.
- The app uses the LGPL build, so its MIT code stays separate from libmpv as a DLL. The GPL build
  adds nothing the app needs. The script refuses a build whose FFmpeg or mpv says GPL, or whose
  DLL needs any DLL Windows does not have.
- `.github/workflows/libmpv-windows.yml` runs it for x86_64 and aarch64 when anything the build
  is made from changes, checks the tap on an x64 and an Arm64 Windows runner with
  `desktop/windows/libmpv/tapcheck.c`, and uploads the zips and `libmpv-<build>-sources.tar`, all
  it was built from, to this repository's pre-release `deps-libmpv`, which is never the latest
  release. The build's name is a hash of its inputs, so a name on the pre-release is not built
  again and nothing there is replaced.
- `desktop/windows/scripts/libmpv.ps1` fetches the zip from there and checks its SHA-256; the
  script holds the current pin. The workflow prints the new SHA-256 values and fails until
  `$build` and the two values name its build. Leave old assets in place so older commits still
  build.
- It is a mingw build. Its DLL works from .NET as it is. `kaeri-mpv-host` loads it at run time
  and finds `mpv_set_audio_tap` with `GetProcAddress`; with another libmpv the app runs and
  Settings says finding skip times is not available.
- The DLL is about 67 MB for arm64 and 114 MB for x64.

## Toolchain

All of it installs over SSH.

| Tool | How |
|---|---|
| winget | `Add-AppxProvisionedPackage` of the GitHub release with its dependencies, if the image has no App Installer |
| Git for Windows | `winget install Git.Git` |
| .NET SDK | `winget install Microsoft.DotNet.SDK.10` |
| VS 2022 Build Tools | `vs_buildtools.exe` through `Start-Process -Wait`, see below |
| NASM | `winget install NASM.NASM` |
| CMake | `winget install Kitware.CMake --architecture arm64` |
| rustup | `winget install Rustlang.Rustup`, after the Build Tools, so it does not offer to install Visual Studio |
| Windows App SDK | NuGet, in the project |

The Build Tools components are `Microsoft.VisualStudio.Workload.VCTools`, `...VC.Tools.ARM64`,
`...VC.Tools.x86.x64`, `...Windows11SDK.26100`, `...VC.CMake.Project` (for Ninja) and
`...VC.Llvm.Clang`, with `--includeRecommended`. `winget install
Microsoft.VisualStudio.2022.BuildTools` hangs over SSH, so run the installer as
`Start-Process -FilePath vs_buildtools.exe -ArgumentList '--quiet --wait --norestart --nocache --add ...' -Wait`.
It returns 3010, which means done with a restart recommended.

Turn on long paths
(`HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled = 1`) and run
`git config --global core.longpaths true`.

## Build

From Git Bash, with cargo, dotnet, NASM and CMake on the PATH:

```bash
desktop/windows/scripts/bindings.sh release        # kaeri_ffi.dll and the C# bindings
desktop/windows/scripts/test.sh                    # the C# tests
powershell -File desktop/windows/scripts/libmpv.ps1 -Arch arm64
```

`docs/desktop-windows-bindings.md` covers the C# bindings and what each uniffi bump costs.
