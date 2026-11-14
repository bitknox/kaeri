#!/usr/bin/env bash
# Fails when a binary in a folder needs the Visual C++ runtime, which a clean Windows does not
# have: vcruntime140.dll, msvcp140.dll and the rest of the Redistributable. The engine links it
# in (.cargo/config.toml), and the .NET runtime and the Windows App SDK carry what they need.
#
#   desktop/windows/scripts/vc-runtime-check.sh [--list] <folder>
#
# Reads the imports and delay-load imports of every .dll and .exe under <folder> with Visual
# Studio's dumpbin. With --list it prints each binary that needs one, and passes. Run from Git Bash.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

list=0
if [[ "${1:-}" == "--list" ]]; then list=1; shift; fi
[[ $# -eq 1 && -d "$1" ]] || { echo "usage: desktop/windows/scripts/vc-runtime-check.sh [--list] <folder>" >&2; exit 2; }
folder="$1"

vs="$("/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' \
  -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | tr -d '\r')"
[[ -n "$vs" ]] || vs="$("/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" -latest \
  -products '*' -property installationPath | tr -d '\r')"
dumpbin=""
if [[ -n "$vs" ]]; then
  for candidate in "$(cygpath -u "$vs")"/VC/Tools/MSVC/*/bin/Host{x64,ARM64,x86}/*/dumpbin.exe; do
    if [[ -f "$candidate" ]]; then dumpbin="$candidate"; break; fi
  done
fi
[[ -n "$dumpbin" ]] || { echo "vc-runtime-check.sh: no dumpbin in Visual Studio's MSVC tools" >&2; exit 1; }

shopt -s globstar nullglob nocaseglob
cd "$folder"
binaries=(**/*.dll **/*.exe)
[[ ${#binaries[@]} -gt 0 ]] || { echo "vc-runtime-check.sh: no binaries in $folder" >&2; exit 1; }

# dumpbin names each file, then lists what it imports, indented; the runtime's DLLs by name. It
# skips a file it cannot read with a warning, so only a run that read nothing counts as failed.
dump="$("$dumpbin" -nologo -dependents "${binaries[@]}" | tr -d '\r')" || true
read_count="$(grep -c '^Dump of file ' <<<"$dump" || true)"
[[ "$read_count" -gt 0 ]] || { echo "vc-runtime-check.sh: dumpbin read none of the binaries in $folder" >&2; exit 1; }
needs="$(awk '
  /^Dump of file / { file = substr($0, 14); next }
  tolower($1) ~ /^(vcruntime140(_1|_threads)?|msvcp140(_[0-9a-z_]+)?|concrt140|vccorlib140|vcomp140|vcamp140)\.dll$/ {
    print file ": " $1
  }' <<<"$dump")"
if [[ -z "$needs" ]]; then
  echo "vc-runtime-check.sh: none of the $read_count binaries dumpbin read in $folder needs the Visual C++ runtime"
  exit 0
fi
echo "$needs" | sed 's/^/vc-runtime-check.sh: needs the Visual C++ runtime: /'
((list)) && exit 0
echo "vc-runtime-check.sh: a clean Windows has none of these; link the runtime in or carry it" >&2
exit 1
