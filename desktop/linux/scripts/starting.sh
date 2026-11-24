#!/usr/bin/env bash
# Inside the build box: the starting screen's shots, and recordings of its motion, from the
# engine's made-up starts. Shots go to /out, recordings to /out/motion.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh desktop/linux/scripts/starting.sh
#
# SHOTS picks the shots as start-<state>-<seconds> (the run takes the usual ones by default),
# REDUCED=1 turns animations off as GNOME's Reduce Animation does, and RECORD picks recordings
# as state:seconds pairs (a lossless 60 fps video each, with a contact sheet of stills).
#
# The shots are stills (--still): each start's made-up events step the engine's still clock to
# the shot's moment, so start-slow-24 says 24 s on every run and takes about 2 s, not 24. STILL=0
# takes them on the wall clock instead. Recordings always run on the wall clock.
#
# A step that fails (a run that exits non-zero, a shot that did not save, a recording that did
# not encode) is said and the rest still run; the script then exits 1.
set -euo pipefail
failed=0
mkdir -p "$HOME/.config/gtk-4.0" /out/motion
anim=true
[ "${REDUCED:-}" = 1 ] && anim=false
printf '[Settings]\ngtk-font-name=Inter 11\ngtk-enable-animations=%s\n' "$anim" \
  >"$HOME/.config/gtk-4.0/settings.ini"

# A failed build stops the run: an older binary in /target would otherwise be shot.
if ! cargo build --manifest-path desktop/linux/Cargo.toml --features fixtures >/tmp/kaeri-build.log 2>&1; then
  grep -E '^error|-->' /tmp/kaeri-build.log | head -40 >&2
  echo "the app did not build" >&2
  exit 1
fi
grep -E '^warning|-->' /tmp/kaeri-build.log || true
bin=/target/debug/kaeri-desktop

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
  still=(--still)
  [ "${STILL:-1}" = 0 ] && still=()
  dbus-run-session -- timeout 400 "$bin" --fixtures --data "$data" --media "$media" --showcase \
    --hwdec no --ao null --size "$size" --shots /out --prefix "$prefix" --shot "$shots" "${still[@]}" \
    >/out/starting.log 2>&1 || { echo "the run exited with $?" >&2; failed=1; }
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
    >"/out/motion/$name.log" 2>&1 || { echo "the $name run exited with $?" >&2; failed=1; }
  # ffmpeg ends on the interrupt with a status of its own; the encode below says whether the
  # recording holds.
  kill -INT $rec; wait $rec 2>/dev/null || true
  if ffmpeg -hide_banner -loglevel error -y -i "$video" -c:v libx264 -crf 16 -pix_fmt yuv420p "/out/motion/$name.mp4"; then
    echo "recorded $name.mp4"
  else
    echo "the $name recording did not encode" >&2
    failed=1
  fi
  rm -f "$video"
done
ls -1 /out/*.png /out/motion/*.mp4 2>/dev/null || true
[ "$failed" = 0 ] || echo "a step failed: see above" >&2
exit "$failed"
