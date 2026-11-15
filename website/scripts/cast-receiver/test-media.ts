// Makes the receiver's test media with FFmpeg: every container, audio codec and subtitle type the
// receiver unpacks, from FFmpeg's own test patterns and tones. Nothing here is downloaded.
//
//   bun scripts/cast-receiver/test-media.ts [out dir]   (default: .cast-media/, ignored by git)
//
// The image subtitles have no FFmpeg encoder from text, so this writes a PGS stream itself
// (pgs() below) and FFmpeg turns that into VobSub.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const out = process.argv[2] ?? join(import.meta.dir, '../../.cast-media');
mkdirSync(out, { recursive: true });

const W = 320;
const H = 180;

function ff(args: string[], file: string) {
	const target = join(out, file);
	if (existsSync(target)) return;
	mkdirSync(join(target, '..'), { recursive: true });
	const r = spawnSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...args, target], {
		stdio: ['ignore', 'inherit', 'inherit'],
	});
	if (r.status !== 0) throw new Error(`ffmpeg failed for ${file}`);
}

const video = (seconds: number) => ['-f', 'lavfi', '-i', `testsrc2=size=${W}x${H}:rate=24:duration=${seconds}`];
// A 440 Hz tone with a short 1 kHz tick on every whole second.
const tone = (seconds: number, channels = 2) => [
	'-f',
	'lavfi',
	'-i',
	`aevalsrc='0.3*sin(2*PI*440*t)+if(lt(mod(t\\,1)\\,0.02)\\,0.5*sin(2*PI*1000*t)\\,0)':s=48000:d=${seconds}`,
	'-ac',
	String(channels),
];
const x264 = ['-c:v', 'libx264', '-preset', 'veryfast', '-pix_fmt', 'yuv420p', '-g', '48', '-bf', '2'];
const x265 = ['-c:v', 'libx265', '-preset', 'ultrafast', '-tag:v', 'hvc1', '-x265-params', 'log-level=none:keyint=48'];
const vp9 = ['-c:v', 'libvpx-vp9', '-deadline', 'realtime', '-cpu-used', '8', '-g', '48', '-b:v', '300k'];
const vp8 = ['-c:v', 'libvpx', '-deadline', 'realtime', '-cpu-used', '8', '-g', '48', '-b:v', '300k'];
const av1 = ['-c:v', 'libsvtav1', '-preset', '12', '-g', '48'];
const audio: Record<string, string[]> = {
	aac: ['-c:a', 'aac', '-b:a', '128k'],
	ac3: ['-c:a', 'ac3', '-b:a', '384k'],
	eac3: ['-c:a', 'eac3', '-b:a', '384k'],
	dts: ['-c:a', 'dca', '-strict', '-2'],
	truehd: ['-c:a', 'truehd', '-strict', '-2'],
	flac: ['-c:a', 'flac'],
	mp3: ['-c:a', 'libmp3lame', '-b:a', '128k'],
	mp2: ['-c:a', 'mp2', '-b:a', '192k'],
	opus: ['-c:a', 'libopus', '-b:a', '96k'],
	vorbis: ['-c:a', 'vorbis', '-strict', '-2', '-ac', '2'],
	pcm: ['-c:a', 'pcm_s16le'],
};
// Surround where the codec is usually surround, so the downmix is exercised.
const channels: Record<string, number> = { ac3: 6, eac3: 6, dts: 6, truehd: 6 };

function av(file: string, v: string[], a: string, seconds = 8, extra: string[] = []) {
	ff([...video(seconds), ...tone(seconds, channels[a] ?? 2), ...v, ...audio[a], ...extra, '-shortest'], file);
}

// Matroska and WebM
for (const a of ['aac', 'ac3', 'eac3', 'dts', 'truehd', 'flac', 'mp3', 'mp2', 'opus', 'pcm']) av(`mkv-h264-${a}.mkv`, x264, a);
av('mkv-hevc-ac3.mkv', x265, 'ac3');
av('mkv-av1-opus.mkv', av1, 'opus');
av('webm-vp9-opus.webm', vp9, 'opus');
av('webm-vp8-vorbis.webm', vp8, 'vorbis');
av('webm-av1-opus.webm', av1, 'opus');
// Two audio tracks, the second one selected by language.
ff(
	[
		...video(8),
		...tone(8),
		...tone(8, 6),
		'-map', '0:v', '-map', '1:a', '-map', '2:a',
		...x264,
		'-c:a:0', 'aac', '-c:a:1', 'ac3',
		'-metadata:s:a:0', 'language=jpn', '-metadata:s:a:1', 'language=eng',
		'-shortest',
	],
	'mkv-two-audio.mkv',
);

// MP4: moov at the end, moov first, fragmented
av('mp4-h264-aac.mp4', x264, 'aac');
av('mp4-h264-aac-faststart.mp4', x264, 'aac', 8, ['-movflags', '+faststart']);
av('mp4-hevc-eac3.mp4', x265, 'eac3');
av('mp4-h264-ac3.mp4', x264, 'ac3');
av('mp4-h264-flac.mp4', x264, 'flac', 8, ['-strict', '-2']);
av('mp4-h264-opus.mp4', x264, 'opus');
av('mp4-frag-h264-aac.mp4', x264, 'aac', 8, ['-movflags', 'frag_keyframe+empty_moov+default_base_moof']);

// MPEG-TS
for (const a of ['aac', 'ac3', 'mp2', 'dts', 'mp3']) av(`ts-h264-${a}.ts`, x264, a);
av('ts-hevc-eac3.ts', x265, 'eac3');

// HLS: TS segments; fMP4 segments with the audio as its own rendition
ff([...video(12), ...tone(12), ...x264, ...audio.aac, '-f', 'hls', '-hls_time', '2', '-hls_playlist_type', 'vod', '-hls_segment_filename', join(out, 'hls-ts/seg%03d.ts')], 'hls-ts/index.m3u8');
ff(
	[
		...video(12), ...tone(12, 6), ...x264, ...audio.ac3,
		'-map', '0:v', '-map', '1:a',
		'-f', 'hls', '-hls_time', '2', '-hls_playlist_type', 'vod', '-hls_segment_type', 'fmp4',
		'-var_stream_map', 'v:0,agroup:aud a:0,agroup:aud,language:eng',
		'-master_pl_name', 'master.m3u8',
		'-hls_segment_filename', join(out, 'hls-fmp4/%v/seg%03d.m4s'),
	],
	'hls-fmp4/%v/index.m3u8',
);

// DASH
ff([...video(12), ...tone(12), ...x264, ...audio.aac, '-map', '0:v', '-map', '1:a', '-f', 'dash', '-seg_duration', '2', '-use_timeline', '1', '-use_template', '1'], 'dash/manifest.mpd');
ff([...video(12), ...tone(12, 6), ...x264, ...audio.eac3, '-map', '0:v', '-map', '1:a', '-f', 'dash', '-seg_duration', '2', '-use_timeline', '0', '-use_template', '1'], 'dash-eac3/manifest.mpd');

// A longer one for the audio clock: AC-3, which a desktop browser's MediaSource refuses.
av('sync-h264-ac3.mkv', x264, 'ac3', 40);
av('sync-h264-aac.mkv', x264, 'aac', 40);

// Subtitles
const cues: [number, number, string][] = [
	[0, 2.0, 'First line of the test'],
	[2.5, 4.0, '<i>Second</i> line'],
	[4.5, 6.5, 'Third line'],
];
const srt = cues
	.map(([a, b, t], i) => `${i + 1}\n${stamp(a, ',')} --> ${stamp(b, ',')}\n${t}\n`)
	.join('\n');
writeFileSync(join(out, 'subs.srt'), srt);
const ass = `[Script Info]
ScriptType: v4.00+
PlayResX: ${W}
PlayResY: ${H}

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Sans,16,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,1,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
${cues.map(([a, b, t]) => `Dialogue: 0,${assStamp(a)},${assStamp(b)},Default,,0,0,0,,${t.replace(/<\/?i>/g, '')}`).join('\n')}
`;
writeFileSync(join(out, 'subs.ass'), ass);
writeFileSync(join(out, 'subs.sup'), pgs());

const withSub = (sub: string, codec: string, file: string, container: string[] = []) =>
	ff([...video(8), '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=48000:duration=8', '-i', join(out, sub), '-map', '0:v', '-map', '1:a', '-map', '2:s', ...x264, ...audio.aac, '-c:s', codec, ...container, '-shortest'], file);
withSub('subs.srt', 'subrip', 'mkv-srt.mkv');
withSub('subs.ass', 'ass', 'mkv-ass.mkv');
withSub('subs.srt', 'webvtt', 'mkv-webvtt.mkv');
withSub('subs.sup', 'copy', 'mkv-pgs.mkv');
// FFmpeg drops the converted VobSub packets when muxing beside the test pattern, so convert first.
ff(['-fix_sub_duration', '-i', join(out, 'subs.sup'), '-c:s', 'dvdsub'], 'subs-vobsub.mkv');
ff(['-i', join(out, 'mkv-h264-aac.mkv'), '-i', join(out, 'subs-vobsub.mkv'), '-map', '0', '-map', '1', '-c', 'copy'], 'mkv-vobsub.mkv');
withSub('subs.srt', 'mov_text', 'mp4-tx3g.mp4');

console.log(`test media in ${out}`);

function stamp(s: number, sep: string) {
	const ms = Math.round(s * 1000);
	const p = (n: number, w = 2) => String(n).padStart(w, '0');
	return `${p(Math.floor(ms / 3600000))}:${p(Math.floor(ms / 60000) % 60)}:${p(Math.floor(ms / 1000) % 60)}${sep}${p(ms % 1000, 3)}`;
}
function assStamp(s: number) {
	const cs = Math.round(s * 100);
	return `${Math.floor(cs / 360000)}:${String(Math.floor(cs / 6000) % 60).padStart(2, '0')}:${String(Math.floor(cs / 100) % 60).padStart(2, '0')}.${String(cs % 100).padStart(2, '0')}`;
}

/** A PGS (.sup) stream with one bitmap per cue: a light bar with a dark band, bottom centre. */
function pgs(): Uint8Array {
	const parts: number[] = [];
	const u16 = (a: number[], v: number) => a.push((v >> 8) & 255, v & 255);
	const seg = (pts: number, type: number, body: number[]) => {
		const t = Math.round(pts * 90000);
		parts.push(0x50, 0x47, (t >>> 24) & 255, (t >>> 16) & 255, (t >>> 8) & 255, t & 255, 0, 0, 0, 0, type);
		u16(parts, body.length);
		parts.push(...body);
	};
	const ow = 160;
	const oh = 24;
	const x = (W - ow) >> 1;
	const y = H - oh - 12;
	let n = 0;
	for (const [start, end] of cues) {
		// Presentation composition: epoch start, one object
		const pcs: number[] = [];
		u16(pcs, W);
		u16(pcs, H);
		pcs.push(0x10);
		u16(pcs, n++);
		pcs.push(0x80, 0, 0, 1);
		u16(pcs, 0);
		pcs.push(0, 0);
		u16(pcs, x);
		u16(pcs, y);
		seg(start, 0x16, pcs);
		const wds: number[] = [1, 0];
		u16(wds, x);
		u16(wds, y);
		u16(wds, ow);
		u16(wds, oh);
		seg(start, 0x17, wds);
		// Palette: 1 light, 2 dark (Y, Cr, Cb, A)
		seg(start, 0x14, [0, 0, 1, 235, 128, 128, 255, 2, 16, 128, 128, 255]);
		const rle: number[] = [];
		for (let row = 0; row < oh; row++) {
			const color = row >= 8 && row < 16 ? 2 : 1;
			rle.push(0, 0x80 | 0x40 | ((ow >> 8) & 0x3f), ow & 255, color, 0, 0);
		}
		const ods: number[] = [];
		u16(ods, 0);
		ods.push(0, 0xc0);
		const len = rle.length + 4;
		ods.push((len >> 16) & 255, (len >> 8) & 255, len & 255);
		u16(ods, ow);
		u16(ods, oh);
		ods.push(...rle);
		seg(start, 0x15, ods);
		seg(start, 0x80, []);
		// Clear
		const clear: number[] = [];
		u16(clear, W);
		u16(clear, H);
		clear.push(0x10);
		u16(clear, n++);
		clear.push(0, 0, 0, 0);
		seg(end, 0x16, clear);
		seg(end, 0x17, wds);
		seg(end, 0x80, []);
	}
	return new Uint8Array(parts);
}
