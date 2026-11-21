#!/bin/sh
# Runs an AppImage desktop/linux/scripts/appimage.sh made on a distribution other than the one it
# was built on, as a person would: in a fresh container of it, as root, with only what a desktop
# has installed, and without network, as the AppImage catalog tests it. The apps-release
# workflow's linux-check job runs it on each AppImage.
#
#   desktop/linux/scripts/appimage-check.sh --deps                  install, with network
#   desktop/linux/scripts/appimage-check.sh <AppImage> <version>    then run it, without
#
# --deps installs the libraries the AppImage leaves to the system (the graphics stack, X11's and
# Wayland's client libraries, fontconfig, FreeType, HarfBuzz, FriBidi and ALSA), the CA
# certificates and the MIME database, as every desktop has them, with Xvfb and xdotool. Then
# the run checks that `--version` says <version>, and that the app shows a window on a virtual
# display within 30 seconds, is still running 5 seconds later, and logged none of the failures a
# broken bundle shows: GTK without its own resources, icons it cannot load, or a player that did
# not start. Without FUSE in a container, the AppImage extracts itself and runs
# (APPIMAGE_EXTRACT_AND_RUN).
set -eu

fail() { echo "appimage-check.sh: $*" >&2; exit 1; }
. /etc/os-release
echo "appimage-check.sh: $PRETTY_NAME"

if [ "${1:-}" = --deps ]; then
  if command -v dnf >/dev/null 2>&1; then
    dnf install -y -q mesa-libEGL libglvnd-gles mesa-libgbm mesa-dri-drivers libdrm libX11 libX11-xcb libxcb \
      libwayland-client fontconfig freetype harfbuzz fribidi alsa-lib xorg-x11-server-Xvfb xdotool \
      dejavu-sans-fonts ca-certificates shared-mime-info >/dev/null
  elif command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    # ALSA's library has a new name since Ubuntu 24.04 and Debian 13.
    asound=libasound2t64
    apt-cache show libasound2t64 >/dev/null 2>&1 || asound=libasound2
    apt-get install -y -qq --no-install-recommends libegl1 libgles2 libegl-mesa0 libgbm1 libgl1-mesa-dri \
      libdrm2 libx11-6 libx11-xcb1 libxcb1 libwayland-client0 libfontconfig1 libfreetype6 \
      libharfbuzz0b libfribidi0 "$asound" xvfb xauth xdotool fonts-dejavu-core ca-certificates \
      libcom-err2 shared-mime-info >/dev/null
  else
    fail "no dnf or apt-get on $PRETTY_NAME"
  fi
  exit 0
fi

[ $# -eq 2 ] || { echo "usage: desktop/linux/scripts/appimage-check.sh --deps | <AppImage> <version>" >&2; exit 2; }
appimage="$1" version="$2"
echo "appimage-check.sh: glibc $(getconf GNU_LIBC_VERSION 2>/dev/null | cut -d' ' -f2)"

export APPIMAGE_EXTRACT_AND_RUN=1 HOME="${TMPDIR:-/tmp}/kaeri-check-home" NO_AT_BRIDGE=1
mkdir -p "$HOME"
cd "$HOME"

said="$("$appimage" --version)"
[ "$said" = "Kaeri $version" ] || fail "--version says '$said', not 'Kaeri $version'"
echo "appimage-check.sh: $said"

# The first run registered kaeri:// links with this copy.
entry="$HOME/.local/share/applications/app.kaeri.desktop-appimage.desktop"
grep -qxF "Exec=\"$(readlink -f "$appimage")\" %u" "$entry" ||
  { cat "$entry" 2>/dev/null; fail "the first run did not register kaeri:// with this AppImage"; }
grep -qxF 'MimeType=x-scheme-handler/kaeri;' "$entry" || fail "$entry takes no kaeri:// links"

Xvfb :91 -screen 0 1280x800x24 -nolisten tcp >"$HOME/xvfb.log" 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
i=0
while [ ! -e /tmp/.X11-unix/X91 ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
[ -e /tmp/.X11-unix/X91 ] || { cat "$HOME/xvfb.log"; fail "Xvfb did not start"; }

DISPLAY=:91 GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 "$appimage" >"$HOME/app.log" 2>&1 &
app=$!
shown=""
i=0
while [ $i -lt 30 ]; do
  sleep 1
  i=$((i + 1))
  kill -0 $app 2>/dev/null || break
  if DISPLAY=:91 xdotool search --onlyvisible --name . >/dev/null 2>&1; then shown=$i; break; fi
done
if ! kill -0 $app 2>/dev/null; then
  tail -40 "$HOME/app.log"
  fail "the app exited within $i seconds instead of showing a window"
fi
[ -n "$shown" ] || { tail -40 "$HOME/app.log"; fail "the app showed no window within 30 seconds"; }
sleep 5
if ! kill -0 $app 2>/dev/null; then
  tail -40 "$HOME/app.log"
  fail "the app exited 5 seconds after showing its window"
fi
kill $app
if grep -E 'Cannot find resource|resource at .* does not exist|Unrecognized image file format|Failed to load icon|mpv did not start|engine did not start|symbol lookup error|undefined symbol' "$HOME/app.log"; then
  fail "the app logged the failures above"
fi
echo "appimage-check.sh: the app showed its window after $shown seconds on $PRETTY_NAME and was still running 5 seconds later"
