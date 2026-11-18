#!/usr/bin/env bash
# Builds the receiver's WebAssembly audio decoders into public/cast/receiver/pcm/.
#
# Each family is its own pair of files (a small ES module and its .wasm), loaded only when a
# stream's audio needs it. They are FFmpeg's decoders (LGPL 2.1 or later, configured without any
# GPL or nonfree part) behind decoder.c. The output is committed, because the website build has
# no Emscripten; run this again after changing decoder.c or the FFmpeg version.
#
#   EMSDK=/path/to/emsdk scripts/cast-receiver/pcm/build.sh [work dir]
#
# Needs Emscripten (tested with 4.0.10) and curl. The FFmpeg source comes from ffmpeg.org,
# checked against the SHA-256 below.
set -euo pipefail

FFMPEG_VERSION=7.1.1
FFMPEG_SHA256=733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1

here=$(cd "$(dirname "$0")" && pwd)
site=$(cd "$here/../../.." && pwd)
out="$site/public/cast/receiver/pcm"
work=${1:-$(mktemp -d)}
: "${EMSDK:?set EMSDK to an emsdk checkout}"
# shellcheck disable=SC1091
source "$EMSDK/emsdk_env.sh" >/dev/null 2>&1

mkdir -p "$work" "$out"
cd "$work"
tarball="ffmpeg-$FFMPEG_VERSION.tar.xz"
if [ ! -f "$tarball" ]; then
	curl -sSfLo "$tarball" "https://ffmpeg.org/releases/$tarball"
fi
echo "$FFMPEG_SHA256  $tarball" | shasum -a 256 -c - >/dev/null
[ -d "ffmpeg-$FFMPEG_VERSION" ] || tar xf "$tarball"
src="$work/ffmpeg-$FFMPEG_VERSION"

# family: decoders | parsers
families=(
	"ac3:ac3,eac3:ac3"
	"dts:dca:dca"
	"truehd:truehd,mlp:mlp"
	"flac:flac:flac"
	"more:aac,aac_latm,mp1float,mp2float,mp3float,vorbis,opus,alac,pcm_bluray,pcm_dvd,pcm_s16be,pcm_s16le,pcm_s24be,pcm_s24le,pcm_s32le,pcm_f32le,pcm_f64le,pcm_u8:aac,aac_latm,mpegaudio,opus,vorbis"
)

for entry in "${families[@]}"; do
	IFS=: read -r name decoders parsers <<<"$entry"
	build="$work/build-$name"
	if [ ! -f "$build/libavcodec/libavcodec.a" ]; then
		rm -rf "$build"
		mkdir -p "$build"
		(
			cd "$build"
			emconfigure "$src/configure" \
				--cc=emcc --cxx=em++ --ar=emar --ranlib=emranlib --nm=emnm \
				--target-os=none --arch=x86_32 --enable-cross-compile \
				--disable-asm --disable-inline-asm --disable-x86asm --disable-stripping \
				--disable-programs --disable-doc --disable-debug --disable-runtime-cpudetect \
				--disable-autodetect --disable-pthreads --disable-network --disable-everything \
				--disable-avdevice --disable-avformat --disable-swscale --disable-swresample \
				--disable-avfilter --disable-iconv \
				--disable-gpl --disable-nonfree \
				--enable-decoder="$decoders" --enable-parser="$parsers" \
				--extra-cflags="-Oz" >/dev/null
			emmake make -j"$(sysctl -n hw.ncpu 2>/dev/null || nproc)" libavcodec/libavcodec.a libavutil/libavutil.a >/dev/null
		)
	fi
	emcc -Oz -flto "$here/decoder.c" -I"$src" -I"$build" \
		"$build/libavcodec/libavcodec.a" "$build/libavutil/libavutil.a" \
		-o "$out/$name.js" \
		-sMODULARIZE=1 -sEXPORT_ES6=1 -sENVIRONMENT=worker -sFILESYSTEM=0 \
		-sALLOW_MEMORY_GROWTH=1 -sINITIAL_MEMORY=24MB -sSTACK_SIZE=256KB \
		-sEXPORTED_FUNCTIONS=_kd_open,_kd_decode,_kd_out,_kd_rate,_kd_channels,_kd_flush,_kd_close,_malloc,_free \
		-sEXPORTED_RUNTIME_METHODS=HEAPU8,HEAPF32,stringToNewUTF8 \
		-sINCOMING_MODULE_JS_API=locateFile,instantiateWasm
	echo "$name: $(wc -c <"$out/$name.wasm" | tr -d ' ') bytes"
done

cp "$here/NOTICE.txt" "$out/NOTICE.txt"
cp "$src/COPYING.LGPLv2.1" "$out/COPYING.LGPLv2.1"
