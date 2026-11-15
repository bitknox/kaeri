#!/usr/bin/env bash
# Generates the Cast receiver's test media matrix: short, self-made clips (ffmpeg's test pattern
# and sine tones, a drawn PGS stream, made-up subtitle lines), nothing taken from anywhere.
#
#   tools/cast-dev/media.sh [--force] [--only <id prefix>]
#
# Output: tools/cast-dev/media/ (git-ignored, or $KAERI_CAST_MEDIA), which tools/cast-dev/serve.ts
# serves at /media/. matrix.tsv there lists every entry for `cast_dev --matrix`:
#
#   id <TAB> how <TAB> path <TAB> sidecar subtitle or - <TAB> what it tests
#
# `how` is `file` (cast as a file on this device, through the relay, as a download is) or `url`
# (HLS and DASH, fetched by the TV from the server's /media/ through the tunnel).
#
# Each clip is 12 s of 1280x720 at 30 fps: long enough for the matrix's 8 s of play.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
out="${KAERI_CAST_MEDIA:-$here/media}"
force=0
only=""
while (( $# )); do
  case "$1" in
    --force) force=1 ;;
    --only) only="$2"; shift ;;
    *) echo "usage: media.sh [--force] [--only <id prefix>]" >&2; exit 2 ;;
  esac
  shift
done

command -v ffmpeg >/dev/null || { echo "ffmpeg is missing" >&2; exit 1; }
font="$repo/website/node_modules/@fontsource/zen-kaku-gothic-new/files/zen-kaku-gothic-new-latin-400-normal.woff2"
[[ -f "$font" ]] || { echo "run 'bun install --frozen-lockfile' in website/ first (the ASS clip attaches an OFL font from it)" >&2; exit 1; }

mkdir -p "$out/src"
tsv="$out/matrix.tsv"
: >"$tsv.new"

ff() { ffmpeg -hide_banner -loglevel error -y "$@"; }
D=12
V="testsrc2=size=1280x720:rate=30:duration=$D"
A1="sine=frequency=440:sample_rate=48000:duration=$D"
A2="sine=frequency=660:sample_rate=48000:duration=$D"
X264=(-c:v libx264 -preset veryfast -profile:v high -pix_fmt yuv420p -g 60 -keyint_min 60 -sc_threshold 0)
AAC=(-c:a aac -b:a 128k)

# entry <id> <how> <path> <sidecar> <what>: listed, and made unless it is there already.
want() {
  local id="$1"
  [[ -n "$only" && "$id" != "$only"* ]] && return 1
  [[ $force == 0 && -e "$out/$3" ]] && return 1
  echo "making $id" >&2
  return 0
}
list() {
  printf '%s\t%s\t%s\t%s\t%s\n' "$@" >>"$tsv.new"
}

# Shared sources: picture as lossless-ish intermediates, two tones as WAV.
[[ -f "$out/src/picture.mkv" ]] || ff -f lavfi -i "$V" -c:v libx264 -qp 0 -preset ultrafast -pix_fmt yuv444p "$out/src/picture.mkv"
[[ -f "$out/src/tone-a.wav" ]] || ff -f lavfi -i "$A1" -ac 2 "$out/src/tone-a.wav"
[[ -f "$out/src/tone-b.wav" ]] || ff -f lavfi -i "$A2" -ac 2 "$out/src/tone-b.wav"
[[ -f "$out/src/tone-6.wav" ]] || ff -f lavfi -i "$A1" -af "pan=5.1|c0=c0|c1=c0|c2=c0|c3=c0|c4=c0|c5=c0" "$out/src/tone-6.wav"
P=(-i "$out/src/picture.mkv")
TA=(-i "$out/src/tone-a.wav")

# Subtitle sources: made-up lines.
cat >"$out/src/lines.srt" <<'SRT'
1
00:00:01,000 --> 00:00:05,000
Test line one, plain text.

2
00:00:06,000 --> 00:00:10,000
Test line two, <i>italic</i>.
SRT
cat >"$out/src/lines.vtt" <<'VTT'
WEBVTT

00:00:01.000 --> 00:00:05.000
Test line one, plain text.

00:00:06.000 --> 00:00:10.000
Test line two, <i>italic</i>.
VTT
cat >"$out/src/typeset.ass" <<'ASS'
[Script Info]
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720
WrapStyle: 0

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Zen Kaku Gothic New,48,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,3,1,2,40,40,40,1
Style: Sign,Zen Kaku Gothic New,64,&H0000E0FF,&H000000FF,&H00402000,&H00000000,1,0,0,0,100,100,2,0,1,4,0,8,40,40,60,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Typeset test line one
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,Typeset test line two
Dialogue: 1,0:00:01.00,0:00:10.00,Sign,,0,0,0,,{\pos(640,120)\frz-6\fad(300,300)}SIGN TEST
Dialogue: 1,0:00:02.00,0:00:09.00,Sign,,0,0,0,,{\move(200,600,1080,600)\fs40}Moving sign
ASS
[[ -f "$out/src/pgs.sup" ]] || python3 "$here/pgs.py" "$out/src/pgs.sup"
cp "$font" "$out/src/ZenKakuGothicNew.woff2"

# H.264 + AAC-LC, the base most entries copy from.
base="$out/src/base.mkv"
[[ -f "$base" && ( $force == 0 || -n "$only" ) ]] || ff "${P[@]}" "${TA[@]}" "${X264[@]}" -b:v 3M "${AAC[@]}" -shortest "$base"

# Containers.
want c-mp4 file c-mp4.mp4 && ff -i "$base" -c copy -movflags +faststart "$out/c-mp4.mp4"
list c-mp4 file c-mp4.mp4 - "MP4, H.264 High 8-bit, AAC-LC"
want c-mkv file c-mkv.mkv && ff -i "$base" -c copy "$out/c-mkv.mkv"
list c-mkv file c-mkv.mkv - "MKV, H.264 High 8-bit, AAC-LC"
want c-webm file c-webm.webm && ff "${P[@]}" "${TA[@]}" -c:v libvpx-vp9 -b:v 2M -deadline realtime -cpu-used 8 -row-mt 1 -pix_fmt yuv420p -c:a libopus -b:a 128k -shortest "$out/c-webm.webm"
list c-webm file c-webm.webm - "WebM, VP9 profile 0, Opus"
if want c-hls-fmp4 url c-hls-fmp4/index.m3u8; then
  mkdir -p "$out/c-hls-fmp4"
  ff -i "$base" -c copy -f hls -hls_time 4 -hls_playlist_type vod -hls_segment_type fmp4 -hls_fmp4_init_filename init.mp4 -hls_segment_filename "$out/c-hls-fmp4/seg%03d.m4s" "$out/c-hls-fmp4/index.m3u8"
fi
list c-hls-fmp4 url c-hls-fmp4/index.m3u8 - "HLS, fMP4 segments, H.264, AAC-LC"
if want c-hls-ts url c-hls-ts/index.m3u8; then
  mkdir -p "$out/c-hls-ts"
  ff -i "$base" -c copy -bsf:v h264_mp4toannexb -f hls -hls_time 4 -hls_playlist_type vod -hls_segment_filename "$out/c-hls-ts/seg%03d.ts" "$out/c-hls-ts/index.m3u8"
fi
list c-hls-ts url c-hls-ts/index.m3u8 - "HLS, MPEG-TS segments, H.264, AAC-LC"
if want c-dash url c-dash/index.mpd; then
  mkdir -p "$out/c-dash"
  ff -i "$base" -c copy -f dash -seg_duration 4 -use_template 1 -use_timeline 0 "$out/c-dash/index.mpd"
fi
list c-dash url c-dash/index.mpd - "DASH, fMP4, H.264, AAC-LC"

# Video codecs, each in MP4 and MKV with AAC-LC.
video() {
  local id="$1" what="$2"; shift 2
  local made="$out/src/v-$id.mkv"
  if want "v-$id" file "v-$id.mkv" || want "v-$id" file "v-$id.mp4"; then
    ff "${P[@]}" "${TA[@]}" "$@" "${AAC[@]}" -shortest "$made"
    ff -i "$made" -c copy "$out/v-$id.mkv"
    ff -i "$made" -c copy $([[ $id == hevc* ]] && echo "-tag:v hvc1") -movflags +faststart "$out/v-$id.mp4"
  fi
  list "v-$id-mp4" file "v-$id.mp4" - "MP4, $what, AAC-LC"
  list "v-$id-mkv" file "v-$id.mkv" - "MKV, $what, AAC-LC"
}
video h264-hi10 "H.264 High 10" -c:v libx264 -preset veryfast -profile:v high10 -pix_fmt yuv420p10le -b:v 3M
video hevc "HEVC Main" -c:v libx265 -preset fast -profile:v main -pix_fmt yuv420p -b:v 2M -x265-params log-level=error
video hevc-main10 "HEVC Main 10" -c:v libx265 -preset fast -profile:v main10 -pix_fmt yuv420p10le -b:v 2M -x265-params log-level=error
video vp9 "VP9 profile 0" -c:v libvpx-vp9 -b:v 2M -deadline realtime -cpu-used 8 -row-mt 1 -pix_fmt yuv420p
video av1 "AV1 Main 8-bit" -c:v libsvtav1 -preset 10 -b:v 2M -pix_fmt yuv420p
if want v-hevc-hls url v-hevc-hls/index.m3u8; then
  mkdir -p "$out/v-hevc-hls"
  ff -i "$out/src/v-hevc.mkv" -c copy -tag:v hvc1 -f hls -hls_time 4 -hls_playlist_type vod -hls_segment_type fmp4 -hls_fmp4_init_filename init.mp4 -hls_segment_filename "$out/v-hevc-hls/seg%03d.m4s" "$out/v-hevc-hls/index.m3u8"
fi
list v-hevc-hls url v-hevc-hls/index.m3u8 - "HLS fMP4, HEVC Main, AAC-LC"

# Audio codecs, each with H.264 in MKV, and in MP4 where MP4 carries it.
audio() {
  local id="$1" what="$2" mp4="$3" input="$4"; shift 4
  if want "a-$id" file "a-$id.mkv"; then
    # The audio on its own first: the TrueHD encoder crashes when it runs beside a copied picture.
    ff -i "$input" "$@" "$out/src/a-$id.mka"
    ff -i "$base" -i "$out/src/a-$id.mka" -map 0:v -map 1:a -c copy -shortest "$out/a-$id.mkv"
    [[ $mp4 == yes ]] && ff -i "$out/a-$id.mkv" -c copy -movflags +faststart "$out/a-$id.mp4"
  fi
  list "a-$id-mkv" file "a-$id.mkv" - "MKV, H.264, $what"
  [[ $mp4 == yes ]] && list "a-$id-mp4" file "a-$id.mp4" - "MP4, H.264, $what"
  return 0
}
tone="$out/src/tone-a.wav"
six="$out/src/tone-6.wav"
audio aac-lc "AAC-LC" no "$tone" -c:a aac -b:a 128k
audio he-aac "HE-AAC v1" yes "$tone" -c:a aac_at -profile:a 4 -b:a 64k
audio he-aac-v2 "HE-AAC v2" yes "$tone" -c:a aac_at -profile:a 28 -b:a 32k
audio opus "Opus" yes "$tone" -c:a libopus -b:a 128k
audio flac "FLAC" yes "$tone" -c:a flac
audio mp3 "MP3" yes "$tone" -c:a libmp3lame -b:a 192k
audio ac3 "AC-3 5.1" yes "$six" -c:a ac3 -b:a 384k
audio eac3 "E-AC-3 5.1" yes "$six" -c:a eac3 -b:a 384k
audio dts "DTS 5.1" no "$six" -strict -2 -c:a dca -b:a 768k
audio truehd "TrueHD 5.1" no "$six" -strict -2 -c:a truehd -sample_fmt s16p

# Subtitles, in MKV with H.264 and AAC-LC, and side-loaded beside an MP4.
if want s-ass-font file s-ass-font.mkv; then
  ff -i "$base" -i "$out/src/typeset.ass" -map 0 -map 1 -c copy -c:s ass -metadata:s:s:0 language=eng \
    -attach "$out/src/ZenKakuGothicNew.woff2" -metadata:s:t:0 mimetype=font/woff2 -metadata:s:t:0 filename=ZenKakuGothicNew.woff2 \
    "$out/s-ass-font.mkv"
fi
list s-ass-font file s-ass-font.mkv - "MKV, ASS track with an attached font (Zen Kaku Gothic New)"
want s-srt file s-srt.mkv && ff -i "$base" -i "$out/src/lines.srt" -map 0 -map 1 -c copy -c:s srt -metadata:s:s:0 language=eng "$out/s-srt.mkv"
list s-srt file s-srt.mkv - "MKV, SRT track"
want s-vtt file s-vtt.mkv && ff -i "$base" -i "$out/src/lines.vtt" -map 0 -map 1 -c copy -c:s webvtt -metadata:s:s:0 language=eng "$out/s-vtt.mkv"
list s-vtt file s-vtt.mkv - "MKV, WebVTT track"
want s-pgs file s-pgs.mkv && ff -i "$base" -i "$out/src/pgs.sup" -map 0 -map 1 -c copy -metadata:s:s:0 language=eng "$out/s-pgs.mkv"
list s-pgs file s-pgs.mkv - "MKV, PGS track (drawn boxes)"
want s-vobsub file s-vobsub.mkv && ff -i "$base" -i "$out/src/pgs.sup" -map 0 -map 1 -c:v copy -c:a copy -c:s dvdsub -metadata:s:s:0 language=eng "$out/s-vobsub.mkv"
list s-vobsub file s-vobsub.mkv - "MKV, VobSub track (drawn boxes)"
for kind in ass srt vtt; do
  src="$out/src/lines.$kind"; [[ $kind == ass ]] && src="$out/src/typeset.ass"
  if want "s-side-$kind" file "s-side-$kind.$kind"; then
    cp "$src" "$out/s-side-$kind.$kind"
  fi
  list "s-side-$kind" file c-mp4.mp4 "s-side-$kind.$kind" "MP4, side-loaded .$kind file"
done

# Two audio tracks: an HLS master with two renditions, and an MKV with two tracks.
if want m-hls-2audio url m-hls-2audio/master.m3u8; then
  mkdir -p "$out/m-hls-2audio"
  (cd "$out/m-hls-2audio" && ff -i "$base" -i "$out/src/tone-a.wav" -i "$out/src/tone-b.wav" \
    -map 0:v -map 1:a -map 2:a -c:v copy -c:a aac -b:a 128k -f hls -hls_time 4 -hls_playlist_type vod \
    -hls_segment_type fmp4 -hls_segment_filename "s%v_%03d.m4s" -hls_fmp4_init_filename "init_%v.mp4" \
    -master_pl_name master.m3u8 \
    -var_stream_map "v:0,agroup:aud a:0,agroup:aud,language:ja,name:tone-440,default:yes a:1,agroup:aud,language:en,name:tone-660" \
    "stream_%v.m3u8")
fi
list m-hls-2audio url m-hls-2audio/master.m3u8 - "HLS fMP4 master, two AAC renditions (440 Hz ja, 660 Hz en)"
want m-mkv-2audio file m-mkv-2audio.mkv && ff -i "$base" -i "$out/src/tone-b.wav" -map 0:v -map 0:a -map 1:a -c:v copy -c:a aac -b:a 128k \
  -metadata:s:a:0 language=jpn -metadata:s:a:0 title="Tone 440" -metadata:s:a:1 language=eng -metadata:s:a:1 title="Tone 660" -shortest "$out/m-mkv-2audio.mkv"
list m-mkv-2audio file m-mkv-2audio.mkv - "MKV, two AAC tracks (440 Hz ja, 660 Hz en)"

mv "$tsv.new" "$tsv"
echo "$(wc -l <"$tsv" | tr -d ' ') entries in $tsv"
