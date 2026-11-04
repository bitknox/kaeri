#!/usr/bin/env bash
# Inside the build box: records the sync picker's motion (list, crossfade to the browser wait,
# then closed) on a virtual display, once with animations and once with them off, as mp4s in
# /out/motion.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh desktop/linux/scripts/sync-motion.sh
set -euo pipefail
mkdir -p "$HOME/.config/gtk-4.0" /out/motion
cargo build --manifest-path desktop/linux/Cargo.toml --features fixtures 2>&1 \
  | grep -E '^(error|warning)|-->' || true
bin=/target/debug/kaeri-desktop
[ -x "$bin" ] || { echo "the app did not build" >&2; exit 1; }
Xvfb :97 -screen 0 1280x800x24 -nolisten tcp >/out/motion/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X97 ] && break; sleep 0.1; done
export GTK_A11Y=none DISPLAY=:97 GDK_BACKEND=x11 GSK_RENDERER=ngl LIBGL_ALWAYS_SOFTWARE=1 NO_AT_BRIDGE=1
for mode in normal reduced; do
  anim=true; [ "$mode" = reduced ] && anim=false
  printf '[Settings]\ngtk-font-name=Inter 11\ngtk-enable-animations=%s\n' "$anim" >"$HOME/.config/gtk-4.0/settings.ini"
  data="/tmp/kaeri-sync-motion-$mode"; rm -rf "$data"
  ffmpeg -hide_banner -loglevel error -y -f x11grab -framerate 30 -video_size 1280x800 -i :97 \
    -c:v libx264 -preset veryfast -pix_fmt yuv420p "/tmp/raw-$mode.mp4" &
  rec=$!
  dbus-run-session -- timeout 60 "$bin" --fixtures --data "$data" --showcase --hwdec no --ao null \
    --size 1280x800 --shots /tmp/sm-$mode --prefix "m-" --shot sync-motion >"/out/motion/$mode.log" 2>&1 || true
  kill -INT $rec 2>/dev/null || true
  wait $rec || true
  # Cut from the first frame that is not black, where the window came up.
  start=$(ffmpeg -hide_banner -i "/tmp/raw-$mode.mp4" -vf blackdetect=d=0.2:pix_th=0.05 -an -f null - 2>&1 \
    | grep -oE 'black_end:[0-9.]+' | head -1 | cut -d: -f2)
  ffmpeg -hide_banner -loglevel error -y -ss "${start:-0}" -i "/tmp/raw-$mode.mp4" -t 14 \
    -c:v libx264 -preset veryfast -pix_fmt yuv420p "/out/motion/sync-picker-$mode.mp4"
done
ls -1 /out/motion
