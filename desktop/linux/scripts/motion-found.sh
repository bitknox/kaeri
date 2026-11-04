#!/usr/bin/env bash
# Inside the build box: records the found skip times' motion on a virtual display, once with the
# system's animations on and once with them off (Reduce Animation), as mp4s in /out/motion-found:
# the card answering by crossfade, Adjust opening the review, and Settings' paused line.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh desktop/linux/scripts/motion-found.sh
#
# SCENES picks the recordings (default: yes never adjust settings).
set -euo pipefail
mkdir -p "$HOME/.config/gtk-4.0" /out/motion-found

cargo build --manifest-path desktop/linux/Cargo.toml --features fixtures 2>&1 \
  | grep -E '^(error|warning)|-->' || true
bin=/target/debug/kaeri-desktop
[ -x "$bin" ] || { echo "the app did not build" >&2; exit 1; }

size=960x600
Xvfb :97 -screen 0 ${size}x24 -nolisten tcp >/out/motion-found/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X97 ] && break; sleep 0.1; done
export GTK_A11Y=none DISPLAY=:97 GDK_BACKEND=x11 GSK_RENDERER=ngl LIBGL_ALWAYS_SOFTWARE=1 NO_AT_BRIDGE=1

# A synthetic test pattern with a tone, as the player's shots use.
media=/tmp/signal-test.mkv
[ -f "$media" ] || ffmpeg -hide_banner -loglevel error \
  -f lavfi -i "testsrc2=size=1280x720:rate=24" -f lavfi -i "sine=frequency=440:sample_rate=48000" \
  -t 60 -c:v libx264 -preset veryfast -pix_fmt yuv420p -c:a aac "$media"

for variant in motion reduced; do
  animations=true
  [ "$variant" = reduced ] && animations=false
  printf '[Settings]\ngtk-font-name=Inter 11\ngtk-enable-animations=%s\n' "$animations" \
    >"$HOME/.config/gtk-4.0/settings.ini"
  for scene in ${SCENES:-yes never adjust settings}; do
    name="found-$scene"
    [ "$variant" = reduced ] && name="$name-reduced"
    data="/tmp/kaeri-motion-$name"
    rm -rf "$data"
    ffmpeg -hide_banner -nostats -f x11grab -draw_mouse 0 -framerate 30 -video_size "$size" -i :97 \
      -c:v libx264rgb -crf 0 -preset ultrafast "/tmp/$name.mkv" 2>"/tmp/$name.ffmpeg" &
    rec=$!
    sleep 0.5
    echo "record: $name"
    # The recording runs from the app's window; the card's own timers start with the clip.
    dbus-run-session -- timeout 120 "$bin" --fixtures --showcase --data "$data" \
      --media "$media" --hwdec no --ao null --size "$size" \
      --shots /out/motion-found --prefix "$name-" --shot "motion-found-$scene" \
      >"/out/motion-found/$name.log" 2>&1 || echo "the $name run exited with $?"
    kill -INT $rec; wait $rec 2>/dev/null || true
    ffmpeg -hide_banner -loglevel error -y -i "/tmp/$name.mkv" -vf "format=yuv420p" \
      -c:v libx264 -crf 18 -preset slow -movflags +faststart "/out/motion-found/$name.mp4"
    rm -f "/tmp/$name.mkv"
  done
done
