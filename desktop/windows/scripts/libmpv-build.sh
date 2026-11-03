#!/usr/bin/env bash
# Builds libmpv for the Windows app with Kaeri's mpv patches, the way mpv makes its own Windows
# development builds: mpv's ci/build-mingw64-full.sh in the FFmpeg-Builds image for the target,
# the LGPL variant. So the DLL has the same dependencies and the same LGPL configuration as the
# development build the app used before, plus the demuxer audio tap for skip detection
# (android/mpv/patches/mpv, mpv_set_audio_tap).
#
#   desktop/windows/scripts/libmpv-build.sh name                     the build's name, below
#   desktop/windows/scripts/libmpv-build.sh build <arch> <out> [--sources]
#
# <arch> is x86_64 or aarch64. build runs in Docker with the image pinned below and writes into
# <out>:
#
#   libmpv-<name>-<arch>-w64-mingw32-lgpl.zip   libmpv-2.dll, its import library, mpv's headers,
#                                               libbluray's jars and LICENSE.LGPL, as mpv's own
#                                               zips have them, and BUILD.txt
#   libmpv-<name>-<arch>-BUILD.txt              what it was made from, FFmpeg's licence line and
#                                               configure options, and how long it took
#   libmpv-<name>-<arch>-tapcheck.exe           desktop/windows/libmpv/tapcheck.c against it
#   libmpv-<name>-sources.tar                   with --sources: the source of everything in the
#                                               DLL, as the LGPL asks (SOURCES.txt inside says
#                                               what each archive is)
#
# The name is mpv's version and a hash of what the build is made from: this script, mpv's script
# and Kaeri's changes to it (desktop/windows/libmpv), the mpv patches and Android's FFmpeg and
# mpv pins. .github/workflows/libmpv-windows.yml builds both architectures when the name changes,
# runs tapcheck on Windows, and keeps the files on the deps-libmpv pre-release, from which
# desktop/windows/scripts/libmpv.ps1 fetches them by SHA-256.
#
# The pins:
#
#   mpv, FFmpeg   Android's archives, from android/scripts/mpv.sh: mpv v0.41.0 with the patches,
#                 and FFmpeg n9.0.2, the release the images below are made for
#   the script    mpv's ci/build-mingw64-full.sh at the commit mpv's development builds were made
#                 from before, with desktop/windows/libmpv/build-mingw64-full.patch
#   the images    BtbN's FFmpeg-Builds win64-lgpl-9.0 and winarm64-lgpl-9.0 by digest: the
#                 cross toolchain and every library beneath FFmpeg and mpv, static, each pinned by
#                 commit in FFmpeg-Builds' scripts.d at the commit they were made from
#   LuaJIT, libsixel, mujs
#                 archives at a commit, and mujs's wrap (desktop/windows/libmpv/mujs.wrap), where
#                 mpv's script takes the newest
#
# Nothing here deletes Docker data: build runs the image with --rm and leaves the image pulled.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

# --- Pins ------------------------------------------------------------------------------------------

mpv_script_commit=703e59b0ecf659ba8062ecb876fb8ec25f73726f
mpv_script_sum=fac6eea27bc065b675cd5c76b9f1d8dad4b778d4fb2f2bfcfe8fe9d34b915e92
declare -A images=(
  [x86_64]=ghcr.io/btbn/ffmpeg-builds/win64-lgpl-9.0@sha256:3dc7a24eb7a47963eb8ae75bd6d1f9ca3ee68e620005ae86039e927fcc3e1b7d
  [aarch64]=ghcr.io/btbn/ffmpeg-builds/winarm64-lgpl-9.0@sha256:264f78f0ab8b1bb5129587107ffa10e4f00e00a679ec07fb1bb81b66b8d8cc6b
)
ffmpeg_builds_commit=5ece62897c650b863c1ddda7b6964845344aef2d
ffmpeg_builds_sum=3399050a6fb3d93fffb6e6a565889ff02c483fe7b8aea09e95a777043309b1af
luajit_commit=ea9eb7edb5eac4a9d175d74523db34fac0fa5180
luajit_sum=338a5640be00b89fd0a2a027a7433fcbbb4d006be840d216d84151386b5bcdf7
libsixel_commit=f0b705a365563c56f977afaca73997ca68669c62
libsixel_sum=f53d0f3b06c221e88d25724d63c8cf9915f3f7a7308b7133b1b26fb5d5df2707

root="$(cd "$(dirname "$0")/../../.." && pwd)"
self="$root/desktop/windows/scripts/libmpv-build.sh"
android="$root/android/scripts/mpv.sh"
here="$root/desktop/windows/libmpv"
fail() { echo "libmpv-build.sh: $*" >&2; exit 1; }

# FFmpeg's and mpv's pins from Android's list: name | version | address | SHA-256.
pin_of() {
  sed -n '/^sources=(/,/^)/p' "$android" | sed -n "s/^  \"\($1|.*\)\"$/\1/p"
}
IFS='|' read -r _ ffmpeg_version ffmpeg_url ffmpeg_sum <<<"$(pin_of ffmpeg)"
IFS='|' read -r _ mpv_version mpv_url mpv_sum <<<"$(pin_of mpv)"
[[ -n "$ffmpeg_sum" && -n "$mpv_sum" ]] || fail "found no FFmpeg or mpv in $android's sources"
mpv_patches=()
for patch in "$root/android/mpv/patches/mpv/"*.patch; do
  [[ -f "$patch" ]] && mpv_patches+=("$patch")
done
# Then fixes from mpv's master that this toolchain needs, for Windows only.
for patch in "$here/mpv/"*.patch; do
  [[ -f "$patch" ]] && mpv_patches+=("$patch")
done
# FFmpeg fixes from its release/9.0 branch, after the tag: the images' libraries are the ones that
# branch builds with.
ffmpeg_patches=()
for patch in "$here/ffmpeg/"*.patch; do
  [[ -f "$patch" ]] && ffmpeg_patches+=("$patch")
done

sum() { sha256sum "$1" 2>/dev/null | cut -d' ' -f1 || shasum -a 256 "$1" | cut -d' ' -f1; }
hash_of() { { sha256sum 2>/dev/null || shasum -a 256; } | cut -d' ' -f1; }

# The build's name: mpv's version and what the build is made from. tapcheck.c is not part of it.
name() {
  local hash
  hash="$( {
    cat "$self" "$here/build-mingw64-full.patch" "$here/mujs.wrap"
    echo "$ffmpeg_url $ffmpeg_sum $mpv_url $mpv_sum"
    for patch in "${mpv_patches[@]}" "${ffmpeg_patches[@]}"; do basename "$patch"; cat "$patch"; done
  } | hash_of)"
  echo "$mpv_version-kaeri-${hash:0:12}"
}

case "${1:-}" in
  name)
    name
    exit 0
    ;;
  build)
    [[ $# -ge 3 ]] || fail "usage: libmpv-build.sh build <x86_64|aarch64> <out> [--sources]"
    arch="$2"
    [[ -n "${images[$arch]:-}" ]] || fail "no image for $arch: x86_64 or aarch64"
    mkdir -p "$3"
    out="$(cd "$3" && pwd)"
    echo "libmpv-build.sh: building $(name) for $arch in ${images[$arch]%@*}"
    exec docker run --rm -v "$root:/src:ro" -v "$out:/out" -e KAERI_BUILD="$(name)" \
      "${images[$arch]}" bash /src/desktop/windows/scripts/libmpv-build.sh inside "$arch" /out "${4:-}"
    ;;
  inside) ;;
  *)
    echo "usage: desktop/windows/scripts/libmpv-build.sh name | build <x86_64|aarch64> <out> [--sources]" >&2
    exit 2
    ;;
esac

# --- In the image ----------------------------------------------------------------------------------

arch="$2" out="$3" sources="${4:-}"
build="${KAERI_BUILD:?}"
: "${FFBUILD_PREFIX:?not in an FFmpeg-Builds image}"
[[ "$VARIANT" == lgpl ]] || fail "the image is the $VARIANT variant, not lgpl"
# The image's FFmpeg release and the archive agree: 9.0 and n9.0.2.
[[ "${ffmpeg_version#n}" == "$ADDINS_STR".* ]] ||
  fail "the image is made for FFmpeg $ADDINS_STR and the archive is $ffmpeg_version"
case "$arch" in
  x86_64) [[ "$TARGET" == win64 ]] ;;
  aarch64) [[ "$TARGET" == winarm64 ]] ;;
esac || fail "the image is for $TARGET, not $arch"

started=$SECONDS
step() { echo "libmpv-build.sh: $* ($((SECONDS - started)) s)"; }
work=/work
src="$work/mpv"
downloads="$work/downloads"
mkdir -p "$src" "$downloads"

fetch() { # <url> <sha256> <file>
  local file="$downloads/$3"
  if command -v curl >/dev/null; then
    curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 -o "$file" "$1"
  else
    wget -q --tries=5 -O "$file" "$1"
  fi
  [[ "$(sum "$file")" == "$2" ]] || fail "$1 has SHA-256 $(sum "$file"), not $2"
}
unpack() { # <file> <dir>
  mkdir -p "$2"
  tar -xf "$downloads/$1" -C "$2" --strip-components=1
}
apply() { # <dir> <patch>
  if command -v patch >/dev/null; then
    patch -d "$1" -p1 -N -s <"$2"
  else
    git -C "$1" apply -p1 "$2"
  fi || fail "$2 does not apply"
}

step "fetching mpv $mpv_version, FFmpeg $ffmpeg_version, LuaJIT, libsixel and mpv's script"
fetch "$mpv_url" "$mpv_sum" "mpv-$mpv_version.tar.gz"
fetch "$ffmpeg_url" "$ffmpeg_sum" "ffmpeg-${ffmpeg_version#n}.tar.xz"
fetch "https://github.com/LuaJIT/LuaJIT/archive/$luajit_commit.tar.gz" "$luajit_sum" "LuaJIT-$luajit_commit.tar.gz"
fetch "https://github.com/saitoha/libsixel/archive/$libsixel_commit.tar.gz" "$libsixel_sum" "libsixel-$libsixel_commit.tar.gz"
fetch "https://raw.githubusercontent.com/mpv-player/mpv/$mpv_script_commit/ci/build-mingw64-full.sh" \
  "$mpv_script_sum" build-mingw64-full.sh
unpack "mpv-$mpv_version.tar.gz" "$src"
for patch in "${mpv_patches[@]}"; do apply "$src" "$patch"; done
unpack "ffmpeg-${ffmpeg_version#n}.tar.xz" "$src/ffmpeg"
for patch in "${ffmpeg_patches[@]}"; do apply "$src/ffmpeg" "$patch"; done
unpack "LuaJIT-$luajit_commit.tar.gz" "$src/LuaJIT"
unpack "libsixel-$libsixel_commit.tar.gz" "$src/libsixel"
mkdir -p "$src/subprojects"
cp "$here/mujs.wrap" "$src/subprojects/mujs.wrap"
cp "$downloads/build-mingw64-full.sh" "$src/ci/build-mingw64-full.sh"
apply "$src" "$here/build-mingw64-full.patch"
chmod +x "$src/ci/build-mingw64-full.sh"

step "running mpv's ci/build-mingw64-full.sh"
log="$work/build.log"
(cd "$src" && ./ci/build-mingw64-full.sh) >"$log" 2>&1 || {
  grep -E 'error|Error|ERROR' "$log" | grep -v -- '-Werror' | tail -40
  tail -40 "$log"
  [[ -f "$src/build/meson-logs/meson-log.txt" ]] && tail -40 "$src/build/meson-logs/meson-log.txt"
  cp "$log" "$out/libmpv-$build-$arch-build.log"
  fail "the build failed: the log is in $out/libmpv-$build-$arch-build.log"
}

# --- Checks ----------------------------------------------------------------------------------------

package="$src/artifact-libmpv"
dll="$package/libmpv-2.dll"
[[ -f "$dll" ]] || fail "no libmpv-2.dll"
licence="$(sed -n 's/^License: //p' "$log" | head -1)"
[[ "$licence" == "LGPL version 3 or later" || "$licence" == "LGPL version 2.1 or later" ]] ||
  fail "FFmpeg's configure says its licence is '$licence'"
grep -q '^#define HAVE_GPL 0' "$src/build/config.h" || fail "mpv was configured with GPL code"
[[ -f "$package/LICENSE.LGPL" ]] || fail "the package has no LICENSE.LGPL"
"${FFBUILD_CROSS_PREFIX}objdump" -p "$dll" | grep -qw mpv_set_audio_tap ||
  fail "libmpv-2.dll does not export mpv_set_audio_tap"
# Everything but Windows is inside: no MinGW runtime or other library beside it.
needs="$("${FFBUILD_CROSS_PREFIX}objdump" -p "$dll" | sed -n 's/^\s*DLL Name: //p' | sort -u | tr '\n' ' ')"
[[ -n "$needs" ]] || fail "found no imports in libmpv-2.dll"
for need in $needs; do
  case "${need,,}" in
    lib*.dll | *mingw*.dll | *pthread*.dll | *c++*.dll | *unwind*.dll | *gcc*.dll | *ffmpeg*.dll | *mpv*.dll | \
      avcodec*.dll | avformat*.dll | avutil*.dll | avfilter*.dll | avdevice*.dll | swscale*.dll | swresample*.dll)
      fail "libmpv-2.dll needs $need, which Windows does not have" ;;
  esac
done

step "building tapcheck"
"$CC" -O2 -o "$work/tapcheck.exe" "$here/tapcheck.c" -I"$package/include" "$package/libmpv.dll.a" ||
  fail "tapcheck does not build"

# --- The package -----------------------------------------------------------------------------------

{
  echo "build $build"
  echo "arch $arch"
  echo "image ${images[$arch]}"
  echo "ffmpeg-builds $ffmpeg_builds_commit"
  echo "mpv $mpv_version $mpv_url $mpv_sum"
  echo "mpv-patches$(for patch in "${mpv_patches[@]}"; do printf ' %s' "$(basename "$patch")"; done)"
  echo "mpv-script $mpv_script_commit ci/build-mingw64-full.sh $mpv_script_sum, with build-mingw64-full.patch"
  echo "ffmpeg $ffmpeg_version $ffmpeg_url $ffmpeg_sum"
  echo "luajit $luajit_commit"
  echo "libsixel $libsixel_commit"
  echo "mujs $(sed -n 's/^wrapdb_version = //p' "$here/mujs.wrap")"
  echo "ffmpeg License: $licence"
  echo "ffmpeg configure: $FF_CONFIGURE"
  echo "dll-imports $needs"
  echo "seconds $((SECONDS - started))"
} >"$out/libmpv-$build-$arch-BUILD.txt"
cp "$out/libmpv-$build-$arch-BUILD.txt" "$package/BUILD.txt"
zip="$out/libmpv-$build-$arch-w64-mingw32-lgpl.zip"
python3 - "$package" "$zip" <<'EOF'
import os, sys, zipfile
root, target = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for folder, _, files in sorted(os.walk(root)):
        for name in sorted(files):
            path = os.path.join(folder, name)
            z.write(path, os.path.relpath(path, root))
EOF
cp "$work/tapcheck.exe" "$out/libmpv-$build-$arch-tapcheck.exe"
step "wrote $(basename "$zip") ($(( $(stat -c %s "$zip") / 1048576 )) MiB)"

# --- The source ------------------------------------------------------------------------------------

[[ "$sources" == --sources ]] || exit 0
step "collecting the source"
name="libmpv-$build-sources"
stage="$work/$name"
mkdir -p "$stage/sources" "$stage/build/patches"
listing="$work/listing"
: >"$listing"
record() { printf '%s\t%s\t%s\n' "$1" "$(sum "$stage/sources/$1")" "$2" >>"$listing"; }

cp "$downloads/mpv-$mpv_version.tar.gz" "$stage/sources/"
record "mpv-$mpv_version.tar.gz" "$mpv_url"
cp "$downloads/ffmpeg-${ffmpeg_version#n}.tar.xz" "$stage/sources/"
record "ffmpeg-${ffmpeg_version#n}.tar.xz" "$ffmpeg_url"
cp "$downloads/LuaJIT-$luajit_commit.tar.gz" "$stage/sources/"
record "LuaJIT-$luajit_commit.tar.gz" "https://github.com/LuaJIT/LuaJIT at $luajit_commit"
cp "$downloads/libsixel-$libsixel_commit.tar.gz" "$stage/sources/"
record "libsixel-$libsixel_commit.tar.gz" "https://github.com/saitoha/libsixel at $libsixel_commit"
for file in "$src"/subprojects/packagecache/*; do
  cp "$file" "$stage/sources/"
  record "$(basename "$file")" "mujs and its meson build, from the wrap in build/mujs.wrap"
done
# C++/WinRT and the Windows metadata its headers are generated from, as mpv's script fetched them
# at the versions it pins.
for dir in "$src"/cppwinrt-* "$src/windows-rs"; do
  [[ -d "$dir" ]] || fail "found no $(basename "$dir") from mpv's script"
  tar -czf "$stage/sources/$(basename "$dir").tar.gz" --exclude=.git --exclude=build -C "$src" "$(basename "$dir")"
  record "$(basename "$dir").tar.gz" "fetched by mpv's ci/build-mingw64-full.sh (C++/WinRT, and windows-rs's crates/libs/bindgen/default)"
done
fetch "https://github.com/BtbN/FFmpeg-Builds/archive/$ffmpeg_builds_commit.tar.gz" "$ffmpeg_builds_sum" \
  "FFmpeg-Builds-$ffmpeg_builds_commit.tar.gz"
cp "$downloads/FFmpeg-Builds-$ffmpeg_builds_commit.tar.gz" "$stage/sources/"
record "FFmpeg-Builds-$ffmpeg_builds_commit.tar.gz" "https://github.com/BtbN/FFmpeg-Builds at $ffmpeg_builds_commit, whose scripts made the images"

# Every library in the images, by the script that built it: its repository and commit.
builds="$work/FFmpeg-Builds"
unpack "FFmpeg-Builds-$ffmpeg_builds_commit.tar.gz" "$builds"
{
  echo "The libraries in the FFmpeg-Builds images libmpv-2.dll was built in (${images[x86_64]%@*} and"
  echo "${images[aarch64]%@*}), from FFmpeg-Builds at $ffmpeg_builds_commit. Each was built by its"
  echo "script in scripts.d from the repository and commit named; a script's ffbuild_enabled says"
  echo "for which targets and variants. Only the lgpl variant's are in the images used."
  echo
  find "$builds/scripts.d" -name '*.sh' | sort | while read -r script; do
    pins="$(grep -E '^SCRIPT_(REPO|COMMIT|REV|BRANCH)[0-9]*=' "$script" | tr '\n' ' ')"
    [[ -n "$pins" ]] && printf '%s\t%s\n' "${script#"$builds/"}" "$pins"
  done
} >"$stage/DEPENDENCIES.txt"

cp "$self" "$here/build-mingw64-full.patch" "$here/mujs.wrap" "$here/tapcheck.c" "$stage/build/"
cp "$downloads/build-mingw64-full.sh" "$stage/build/build-mingw64-full.sh.orig"
cp "${mpv_patches[@]}" "$stage/build/patches/"
mkdir -p "$stage/build/patches/ffmpeg"
cp "${ffmpeg_patches[@]}" "$stage/build/patches/ffmpeg/"
{
  title="Source of libmpv $build for Windows"
  echo "$title"
  echo "${title//?/=}"
  cat <<EOF

libmpv-2.dll in Kaeri's Windows app is an LGPL build of mpv $mpv_version and FFmpeg $ffmpeg_version,
with the libraries beneath them linked statically into the one DLL, which the app loads as a
library of its own and which can be replaced with one built from modified sources.

  build/libmpv-build.sh            the script that built it, in the images it names by digest
  build/build-mingw64-full.sh.orig mpv's own script for its Windows builds, at mpv commit
                                   $mpv_script_commit
  build/build-mingw64-full.patch   Kaeri's changes to that script
  build/patches/                   Kaeri's changes to mpv, and fixes from mpv's master branch
                                   for this toolchain
  build/patches/ffmpeg/            fixes from FFmpeg's release/9.0 branch, applied to the tag
  build/mujs.wrap                  the mujs release and meson build used
  sources/                         each archive, by its SHA-256 below
  DEPENDENCIES.txt                 every library in the build images, by repository and commit

archive	SHA-256	from
EOF
  sort "$listing"
} >"$stage/SOURCES.txt"
tar -cf "$out/$name.tar" -C "$work" "$name"
step "wrote $name.tar ($(( $(stat -c %s "$out/$name.tar") / 1048576 )) MiB)"
