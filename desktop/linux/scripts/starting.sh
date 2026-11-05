#!/usr/bin/env bash
# Inside the build box: the starting screen's shots, and recordings of its motion, from the
# engine's made-up starts. Shots go to /out, recordings to /out/motion.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh desktop/linux/scripts/starting.sh
#
# SHOTS picks the shots as start-<state>-<seconds> (the run takes the usual ones by default),
# REDUCED=1 turns animations off as GNOME's Reduce Animation does, and RECORD picks recordings
# as state:seconds pairs (a lossless 60 fps video each, with a contact sheet of stills).
set -euo pipefail
mkdir -p "$HOME/.config/gtk-4.0" /out/motion
anim=true
[ "${REDUCED:-}" = 1 ] && anim=false
printf '[Settings]\ngtk-font-name=Inter 11\ngtk-enable-animations=%s\n' "$anim" \
  >"$HOME/.config/gtk-4.0/settings.ini"

cargo build --manifest-path desktop/linux/Cargo.toml --features fixtures 2>&1 \
  | grep -E '^(error|warning)|-->' || true
bin=/target/debug/kaeri-desktop
[ -x "$bin" ] || { echo "the app did not build" >&2; exit 1; }

size=${SIZE:-960x600}
Xvfb :97 -screen 0 ${size}x24 -nolisten tcp >/out/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X97 ] && break; sleep 0.1; done
export GTK_A11Y=none DISPLAY=:97 GDK_BACKEND=x11 GSK_RENDERER=ngl LIBGL_ALWAYS_SOFTWARE=1 NO_AT_BRIDGE=1

media=/tmp/signal-test.mkv
[ -f "$media" ] || ffmpeg -hide_banner -loglevel error -f lavfi -i "testsrc2=size=640x360:rate=24" \
  -t 5 -c:v libx264 -preset veryfast -pix_fmt yuv420p "$media"

prefix=${PREFIX:-}
if [ -n "${SHOTS-x}" ]; then
  shots=${SHOTS:-start-title-lookup-0.8,start-found-2,start-source-missed-2.4,start-slow-10,start-slow-24,start-another-stream-10,start-asking-first-1.5,start-peers-known-10,start-buffering-torrent-9,start-buffering-direct-5,start-offline-1,start-failed-no-episode-5,start-failed-no-match-3,start-failed-no-sources-1,start-failed-not-aired-2}
  data=/tmp/kaeri-scene-start
  rm -rf "$data"
  dbus-run-session -- timeout 400 "$bin" --fixtures --data "$data" --media "$media" --showcase \
    --hwdec no --ao null --size "$size" --shots /out --prefix "$prefix" --shot "$shots" \
    >/out/starting.log 2>&1 || echo "the run exited with $?"
fi

for pair in ${RECORD:-}; do
  state=${pair%%:*} secs=${pair#*:}
  name="$prefix$state"
  video="/tmp/start-$name.mkv"
  ffmpeg -hide_banner -nostats -f x11grab -draw_mouse 0 -framerate 60 -video_size "$size" -i :97 \
    -c:v libx264rgb -crf 0 -preset ultrafast "$video" 2>"/tmp/start-$name.ffmpeg" &
  rec=$!
  sleep 0.5
  rm -rf "/tmp/kaeri-scene-rec"
  dbus-run-session -- timeout 120 "$bin" --fixtures --data /tmp/kaeri-scene-rec --media "$media" --showcase \
    --hwdec no --ao null --size "$size" --shots /out/motion --prefix "rec-$name-" --shot "start-$state-$secs" \
    >"/out/motion/$name.log" 2>&1 || echo "the $name run exited with $?"
  kill -INT $rec; wait $rec 2>/dev/null || true
  ffmpeg -hide_banner -loglevel error -y -i "$video" -c:v libx264 -crf 16 -pix_fmt yuv420p "/out/motion/$name.mp4"
  rm -f "$video"
  echo "recorded $name.mp4"
done
ls -1 /out/*.png /out/motion/*.mp4 2>/dev/null || true
