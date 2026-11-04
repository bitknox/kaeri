# Fetches libmpv for the Windows app (and the player spike) into desktop/windows/.libmpv/<arch>/, at the pinned
# URL below, and refuses it unless its SHA-256 matches.
#
#   powershell -File desktop/windows/scripts/libmpv.ps1 [-Arch arm64|x64]
#
# The zips are Kaeri's own LGPL builds, with Kaeri's mpv patches (the demuxer audio tap among
# them), made by .github/workflows/libmpv-windows.yml with libmpv-build.sh and kept on the
# pre-release `deps-libmpv` (never the latest release) beside the source they were built from.
# A change to what the build is made from runs that workflow, which builds, checks and uploads
# the new zips, prints their SHA-256 values and fails until $build and the two values below name
# them. Old assets stay so older commits still build.
# docs/desktop-windows-build.md says why the LGPL build.
param([ValidateSet('arm64', 'x64')][string]$Arch = 'arm64')
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$build = 'v0.41.0-kaeri-9f2ab280214a'
$pins = @{
  arm64 = @{ name = "libmpv-$build-aarch64-w64-mingw32-lgpl.zip"; sha256 = '229c2556e75fd12b9effb80ae0fd2bd824000655232b2f57f83b42c525465e8c' }
  x64   = @{ name = "libmpv-$build-x86_64-w64-mingw32-lgpl.zip"; sha256 = 'd27f8fd5ab87b9770bcb80d169281287fabdfd3ac0b8c9d346ac40cf4df6a82c' }
}
# The source of this build (libmpv-$build-sources.tar on deps-libmpv), which
# packaging/apps/sources.sh puts beside a release.
$sources = 'adcec2968c1494c2b9170eb06dac0450e3c53f85250eaaff9829b5370334488f'
$pin = $pins[$Arch]
$url = "https://github.com/bitknox/kaeri/releases/download/deps-libmpv/$($pin.name)"

$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root ".libmpv\$Arch"
$stamp = Join-Path $out 'stamp'
if ((Test-Path $stamp) -and ((Get-Content $stamp -Raw).Trim() -eq $pin.sha256)) {
  Write-Host "libmpv.ps1: libmpv $build ($Arch) is in $out"
  exit 0
}

$zip = Join-Path ([IO.Path]::GetTempPath()) $pin.name
Write-Host "libmpv.ps1: fetching $url"
Invoke-WebRequest $url -OutFile $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
if ($hash -ne $pin.sha256) {
  Remove-Item $zip
  throw "libmpv.ps1: $($pin.name) has SHA-256 $hash, expected $($pin.sha256)"
}
if (Test-Path $out) { Remove-Item -Recurse -Force $out }
Expand-Archive $zip $out
Remove-Item $zip
Set-Content $stamp $pin.sha256
Write-Host "libmpv.ps1: wrote $out"
