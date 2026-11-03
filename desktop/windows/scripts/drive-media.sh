#!/usr/bin/env bash
# Makes the clips desktop/windows/Kaeri.Drive plays: generated test patterns with a tone, no real
# media. Skips any clip already there.
#
#   desktop/windows/scripts/drive-media.sh <dir>
#
#   sdr-1080p24.mkv   1920x1080 H.264 testsrc2 at 24 fps, stereo tone, English subtitles, 180 s
#   hevc-2160p60.mkv  3840x2160 HEVC 10-bit testsrc2 at 60 fps, stereo tone, 60 s: the decoder's load
#
# Needs ffmpeg on the PATH, or named in FFMPEG.
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: desktop/windows/scripts/drive-media.sh <dir>" >&2; exit 2; }
mkdir -p "$1"
out="$(cd "$1" && pwd)"
ff=("${FFMPEG:-ffmpeg}" -hide_banner -loglevel error -y)
tone() { echo "sine=frequency=440:sample_rate=48000:duration=$1,aformat=channel_layouts=stereo"; }

if [[ ! -f "$out/sdr-1080p24.mkv" ]]; then
  subs="$out/sdr-1080p24.en.srt"
  : >"$subs"
  for ((i = 0; i < 60; i++)); do
    printf '%d\n00:%02d:%02d,000 --> 00:%02d:%02d,500\nLine %d of the test pattern\n\n' \
      $((i + 1)) $((i * 3 / 60)) $((i * 3 % 60)) $((i * 3 / 60)) $((i * 3 % 60 + 2)) $((i + 1)) >>"$subs"
  done
  "${ff[@]}" -f lavfi -i "testsrc2=size=1920x1080:rate=24:duration=180" -f lavfi -i "$(tone 180)" -i "$subs" \
    -map 0:v -map 1:a -map 2:s -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p -c:a aac -b:a 128k \
    -c:s srt -metadata:s:s:0 language=eng "$out/sdr-1080p24.mkv"
  echo "drive-media.sh: wrote sdr-1080p24.mkv"
fi

if [[ ! -f "$out/hevc-2160p60.mkv" ]]; then
  "${ff[@]}" -f lavfi -i "testsrc2=size=3840x2160:rate=60:duration=60,format=yuv420p10le" -f lavfi -i "$(tone 60)" \
    -c:v libx265 -preset ultrafast -crf 24 -x265-params log-level=error -c:a aac -b:a 128k "$out/hevc-2160p60.mkv"
  echo "drive-media.sh: wrote hevc-2160p60.mkv"
fi
