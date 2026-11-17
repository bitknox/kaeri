// The receiver's audio on a real TV: casts a set of AAC variants and one codec that needs the
// WebAssembly decoders to the dev receiver, reads the receiver's status lines from the cast-dev
// server's log, and fails when an AAC variant leaves MediaSource or when WebAssembly sound skips,
// pauses or runs dry during steady playback.
//
//   bun scripts/cast-receiver-audio-check.ts --log <cast-dev server.log> [--seconds 30]
//       [--only <id prefix>] [--make-only] [--cast-dev "<command>"] [--media-base <url>]
//
// Media: made here with FFmpeg (test patterns and tones, AudioToolbox's aac_at for HE-AAC, so
// macOS) into <cast-dev media>/audio/, which tools/cast-dev/serve.ts serves at /media/audio/.
// Each clip goes to the TV as an HLS URL, so the receiver reads it as it is, with no sender-side
// remux; one Matroska clip also goes as a file, through the relay and its remux, as a download.
// When the tunnel's server runs from another checkout, serve this one's media on another port
// (bun tools/cast-dev/serve.ts --port 8792 --no-watch) and pass --media-base
// http://127.0.0.1:8792/media/; the log stays the tunnel server's.
//
// Casting: runs cast_dev's matrix mode (crates/kaeri-cast/examples/cast_dev.rs), by default with
// `cargo run -q -p kaeri-cast --example cast_dev --`. Run it as cast_dev asks: under the HOME
// guard, with KAERI_CAST_RECEIVER_APP set to the dev receiver, the cast-dev server and tunnel
// serving this checkout's receiver build, and the TV taken under its lock. cast_dev mutes the TV
// and puts the sound back at the end.
//
// The receiver sends its lines only on the dev receiver's host (debug.ts): "receiver setup ..."
// once per change, with the audio path, MIME and config, and "receiver status ..." each second,
// with the WebAssembly sound's jumps, waits, starved and jitter.
import { spawn, spawnSync } from 'node:child_process';
import { closeSync, copyFileSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, readSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../..');
const mediaRoot = resolve(process.env.KAERI_CAST_MEDIA ?? join(repo, 'tools/cast-dev/media'));
const out = join(mediaRoot, 'audio');

const argv = process.argv.slice(2);
const flag = (name: string) => argv.includes(name);
const option = (name: string) => {
	const i = argv.indexOf(name);
	return i >= 0 ? argv[i + 1] : undefined;
};
const seconds = Number(option('--seconds') ?? 30);
const only = option('--only');
const logPath = option('--log') ?? process.env.CAST_DEV_LOG;
const castDev = option('--cast-dev') ?? 'cargo run -q -p kaeri-cast --example cast_dev --';
/** Where cast_dev finds the HLS clips: a server of this checkout's media (cast_dev's default otherwise). */
const mediaBase = option('--media-base');

/** Steady playback starts this many status lines (seconds) after the setup line. */
const SETTLE_LINES = 6;

interface Clip {
	id: string;
	what: string;
	how: 'url' | 'file';
	path: string;
	/** Where the sound must play. */
	expect: 'mse' | 'wasm';
	make: () => void;
}

// ---------- Media ----------

const D = Math.max(20, seconds + 15);

function ff(args: string[]) {
	const r = spawnSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...args], { stdio: ['ignore', 'inherit', 'inherit'] });
	if (r.status !== 0) throw new Error(`ffmpeg failed: ${args.join(' ')}`);
}

const picture = join(out, 'src/picture.mkv');
const tone = join(out, 'src/tone.wav');
const tone6 = join(out, 'src/tone-6.wav');

function sources() {
	mkdirSync(join(out, 'src'), { recursive: true });
	if (!existsSync(picture))
		ff(['-f', 'lavfi', '-i', `testsrc2=size=1280x720:rate=24:duration=${D}`, '-c:v', 'libx264', '-preset', 'veryfast', '-profile:v', 'high', '-pix_fmt', 'yuv420p', '-g', '48', '-b:v', '2M', picture]);
	// A 440 Hz tone with a 1 kHz tick on every whole second: a skip or a pause is easy to hear.
	const src = `aevalsrc='0.3*sin(2*PI*440*t)+if(lt(mod(t\\,1)\\,0.02)\\,0.5*sin(2*PI*1000*t)\\,0)':s=48000:d=${D}`;
	if (!existsSync(tone)) ff(['-f', 'lavfi', '-i', src, '-ac', '2', tone]);
	if (!existsSync(tone6)) ff(['-i', tone, '-af', 'pan=5.1|c0=c0|c1=c1|c2=c0|c3=c0|c4=c0|c5=c1', tone6]);
}

/** An HLS stream of the picture and `audio` (a file already encoded, copied) or a tone encoded with `codec`. */
function hls(dir: string, segments: 'mpegts' | 'fmp4', audio: { file: string } | { codec: string[]; source?: string }) {
	const target = join(out, dir, 'index.m3u8');
	if (existsSync(target)) return;
	mkdirSync(join(out, dir), { recursive: true });
	const input = 'file' in audio ? ['-i', audio.file] : ['-i', audio.source ?? tone];
	const codec = 'file' in audio ? ['-c:a', 'copy'] : audio.codec;
	ff([
		'-i', picture, ...input, '-map', '0:v', '-map', '1:a', '-c:v', 'copy', ...codec, '-shortest',
		'-f', 'hls', '-hls_time', '4', '-hls_playlist_type', 'vod', '-hls_segment_type', segments,
		'-hls_segment_filename', join(out, dir, segments === 'fmp4' ? 'seg%03d.m4s' : 'seg%03d.ts'),
		...(segments === 'fmp4' ? ['-hls_fmp4_init_filename', 'init.mp4'] : []),
		target,
	]);
}

/**
 * HE-AAC v2 in ADTS whose headers say AAC Main (profile 0) at a 24 kHz mono core, as some
 * packagers write it: the stream a TV refused as mp4a.40.1.
 */
function adtsMain(): string {
	const file = join(out, 'src/he-aac-v2-main.aac');
	if (existsSync(file)) return file;
	const plain = join(out, 'src/he-aac-v2.aac');
	ff(['-i', tone, '-c:a', 'aac_at', '-profile:a', '28', '-b:a', '32k', '-f', 'adts', plain]);
	const b = readFileSync(plain);
	let frames = 0;
	for (let at = 0; at + 7 <= b.length; ) {
		if (b[at] !== 0xff || (b[at + 1] & 0xf6) !== 0xf0) throw new Error(`no ADTS frame at ${at}`);
		b[at + 2] &= 0x3f;
		const size = ((b[at + 3] & 3) << 11) | (b[at + 4] << 3) | (b[at + 5] >> 5);
		if (size < 7) throw new Error(`bad ADTS frame at ${at}`);
		at += size;
		frames++;
	}
	if (!frames) throw new Error('no ADTS frames');
	writeFileSync(file, b);
	return file;
}

/**
 * A copy of an fMP4 HLS stream whose init segment says HE-AAC explicitly (object type 5 or 29),
 * where AudioToolbox writes the backward-compatible form (AAC-LC with an SBR extension). The new
 * AudioSpecificConfig is padded with zero bytes to the old one's length, so no box changes size.
 */
function explicit(from: string, dir: string, objectType: 5 | 29) {
	const target = join(out, dir);
	if (existsSync(join(target, 'index.m3u8'))) return;
	mkdirSync(target, { recursive: true });
	for (const f of readdirSync(join(out, from))) copyFileSync(join(out, from, f), join(target, f));
	const init = readFileSync(join(target, 'init.mp4'));
	// The esds box's DecoderSpecificInfo (tag 5); descriptor lengths take one to four bytes.
	const esds = init.indexOf('esds');
	if (esds < 0) throw new Error(`no esds in ${from}/init.mp4`);
	let at = esds + 8;
	let len = 0;
	for (;;) {
		const tag = init[at++];
		len = 0;
		for (let k = 0; k < 4; k++) {
			const b = init[at++];
			len = (len << 7) | (b & 0x7f);
			if (!(b & 0x80)) break;
		}
		if (tag === 5) break;
		if (tag === 3) at += 3;
		else if (tag === 4) at += 13;
		else throw new Error(`unexpected descriptor ${tag} in ${from}/init.mp4`);
	}
	const old = init.subarray(at, at + len);
	const ot = old[0] >> 3;
	const sri = ((old[0] & 7) << 1) | (old[1] >> 7);
	const ch = (old[1] >> 3) & 15;
	if (ot !== 2 || len < 4) throw new Error(`unexpected config ${Buffer.from(old).toString('hex')}`);
	// object type, core rate index, channels, extension rate index (48 kHz), core object type (AAC-LC), GASpecificConfig 0.
	const bits = [objectType, 5, sri, 4, ch, 4, 3, 4, 2, 5, 0, 3];
	const asc = new Uint8Array(len);
	let pos = 0;
	for (let i = 0; i < bits.length; i += 2) {
		for (let k = bits[i + 1] - 1; k >= 0; k--, pos++) if ((bits[i] >> k) & 1) asc[pos >> 3] |= 0x80 >> (pos & 7);
	}
	init.set(asc, at);
	writeFileSync(join(target, 'init.mp4'), init);
}

const clips: Clip[] = [
	{ id: 'aac-lc-hls-ts', what: 'HLS MPEG-TS, AAC-LC 48 kHz stereo', how: 'url', path: 'audio/aac-lc-ts/index.m3u8', expect: 'mse', make: () => hls('aac-lc-ts', 'mpegts', { codec: ['-c:a', 'aac', '-b:a', '128k'] }) },
	{ id: 'aac-main-hls-ts', what: 'HLS MPEG-TS, HE-AAC v2 with ADTS saying AAC Main, 24 kHz mono core', how: 'url', path: 'audio/aac-main-ts/index.m3u8', expect: 'mse', make: () => hls('aac-main-ts', 'mpegts', { file: adtsMain() }) },
	{ id: 'he-aac-hls-ts', what: 'HLS MPEG-TS, HE-AAC v1 (implicit SBR in ADTS)', how: 'url', path: 'audio/he-aac-ts/index.m3u8', expect: 'mse', make: () => hls('he-aac-ts', 'mpegts', { codec: ['-c:a', 'aac_at', '-profile:a', '4', '-b:a', '64k'] }) },
	{ id: 'he-aac-hls-fmp4', what: 'HLS fMP4, HE-AAC v1 (explicit config)', how: 'url', path: 'audio/he-aac-fmp4/index.m3u8', expect: 'mse', make: () => hls('he-aac-fmp4', 'fmp4', { codec: ['-c:a', 'aac_at', '-profile:a', '4', '-b:a', '64k'] }) },
	{ id: 'he-aac-v2-hls-fmp4', what: 'HLS fMP4, HE-AAC v2 (explicit config)', how: 'url', path: 'audio/he-aac-v2-fmp4/index.m3u8', expect: 'mse', make: () => hls('he-aac-v2-fmp4', 'fmp4', { codec: ['-c:a', 'aac_at', '-profile:a', '28', '-b:a', '32k'] }) },
	{
		id: 'he-aac-explicit-hls-fmp4',
		what: 'HLS fMP4, HE-AAC v1 with an explicit config (mp4a.40.5)',
		how: 'url',
		path: 'audio/he-aac-explicit-fmp4/index.m3u8',
		expect: 'mse',
		make: () => {
			hls('he-aac-fmp4', 'fmp4', { codec: ['-c:a', 'aac_at', '-profile:a', '4', '-b:a', '64k'] });
			explicit('he-aac-fmp4', 'he-aac-explicit-fmp4', 5);
		},
	},
	{
		id: 'he-aac-v2-explicit-hls-fmp4',
		what: 'HLS fMP4, HE-AAC v2 with an explicit config (mp4a.40.29)',
		how: 'url',
		path: 'audio/he-aac-v2-explicit-fmp4/index.m3u8',
		expect: 'mse',
		make: () => {
			hls('he-aac-v2-fmp4', 'fmp4', { codec: ['-c:a', 'aac_at', '-profile:a', '28', '-b:a', '32k'] });
			explicit('he-aac-v2-fmp4', 'he-aac-v2-explicit-fmp4', 29);
		},
	},
	{ id: 'aac-51-hls-fmp4', what: 'HLS fMP4, AAC-LC 5.1', how: 'url', path: 'audio/aac-51-fmp4/index.m3u8', expect: 'mse', make: () => hls('aac-51-fmp4', 'fmp4', { codec: ['-c:a', 'aac', '-b:a', '384k'], source: tone6 }) },
	{
		id: 'aac-lc-mkv-file',
		what: 'MKV file through the relay, AAC-LC',
		how: 'file',
		path: 'audio/aac-lc.mkv',
		expect: 'mse',
		make: () => existsSync(join(out, 'aac-lc.mkv')) || ff(['-i', picture, '-i', tone, '-map', '0:v', '-map', '1:a', '-c:v', 'copy', '-c:a', 'aac', '-b:a', '128k', '-shortest', join(out, 'aac-lc.mkv')]),
	},
	{ id: 'ac3-hls-ts', what: 'HLS MPEG-TS, AC-3 stereo (WebAssembly)', how: 'url', path: 'audio/ac3-ts/index.m3u8', expect: 'wasm', make: () => hls('ac3-ts', 'mpegts', { codec: ['-c:a', 'ac3', '-b:a', '192k'] }) },
];

// ---------- Reading the log ----------

interface Status {
	at: string;
	path: string;
	codec: string;
	time: number;
	pcm: { queued: number; drift: number; jumps: number; waits: number; starved: number; jitter: number } | null;
}

function parseStatus(line: string): Status | null {
	const m = line.match(/^(\d\d:\d\d:\d\d) receiver status audio (\S+) (\S*) .*? at ([\d.]+) s(.*)$/);
	if (!m) return null;
	const p = m[5].match(/pcm queued ([\d.]+) s drift ([-+\d]+) ms worst [-+\d]+ ms jumps (\d+) waits (\d+) starved (\d+)(?: jitter ([-+\d]+) ms)?/);
	return {
		at: m[1],
		path: m[2],
		codec: m[3],
		time: Number(m[4]),
		pcm: p ? { queued: Number(p[1]), drift: Number(p[2]), jumps: Number(p[3]), waits: Number(p[4]), starved: Number(p[5]), jitter: Number(p[6] ?? 0) } : null,
	};
}

/** What the log says about one clip's window. Null verdict: passes. */
function judge(c: Clip, lines: string[]): { verdict: string | null; summary: string } {
	const setups = lines.filter((l) => / receiver setup /.test(l)).map((l) => l.replace(/^\d\d:\d\d:\d\d receiver setup /, ''));
	const statuses = lines.map(parseStatus).filter((s): s is Status => !!s && s.time > 0);
	const setup = setups.at(-1) ?? 'no setup line';
	if (!statuses.length) return { verdict: 'no status lines (is the receiver build the one with debug.ts setup lines, and the log the tunnel server\'s?)', summary: setup };
	const path = statuses.at(-1)!.path;
	const summary = `${setup}`;
	if (c.expect === 'mse' && path !== 'mse') return { verdict: `audio went to ${path}, expected MediaSource`, summary };
	if (c.expect === 'wasm' && path !== 'wasm') return { verdict: `audio went to ${path}, expected WebAssembly`, summary };
	const moved = statuses.at(-1)!.time - statuses[0].time;
	if (moved < seconds * 0.5) return { verdict: `video moved ${moved.toFixed(1)} s in ${seconds} s`, summary };
	if (path !== 'wasm') return { verdict: null, summary };
	const steady = statuses.slice(SETTLE_LINES).filter((s) => s.pcm);
	if (steady.length < 5) return { verdict: `only ${steady.length} steady status lines with PCM stats`, summary };
	const a = steady[0].pcm!;
	const b = steady.at(-1)!.pcm!;
	const span = steady.at(-1)!.time - steady[0].time;
	const d = { jumps: b.jumps - a.jumps, waits: b.waits - a.waits, starved: b.starved - a.starved };
	const worstDrift = Math.max(...steady.map((s) => Math.abs(s.pcm!.drift)));
	const jitter = Math.max(...steady.map((s) => s.pcm!.jitter));
	const queued = Math.min(...steady.map((s) => s.pcm!.queued));
	const stats = `over ${span.toFixed(0)} s steady: jumps +${d.jumps}, waits +${d.waits}, starved +${d.starved}, |drift| up to ${worstDrift} ms, jitter up to ${jitter} ms, queued at least ${queued.toFixed(1)} s`;
	const bad: string[] = [];
	// One correction is allowed (a hiccup); climbing counts are the chop.
	if (d.jumps > 1) bad.push('jumps climb');
	if (d.waits > 4) bad.push('waits climb');
	if (d.starved > 0) bad.push('starved climbs');
	if (worstDrift > 60) bad.push('drift beyond 60 ms');
	return { verdict: bad.length ? `${bad.join(', ')} (${stats})` : null, summary: `${summary} | ${stats}` };
}

/** Lines appended to the log since `from` (a byte offset). */
function readFrom(file: string, from: number): { text: string; to: number } {
	const size = statSync(file).size;
	if (size <= from) return { text: '', to: size };
	const fd = openSync(file, 'r');
	const buf = Buffer.alloc(size - from);
	readSync(fd, buf, 0, buf.length, from);
	closeSync(fd);
	return { text: buf.toString('utf8'), to: size };
}

// ---------- Run ----------

const chosen = clips.filter((c) => !only || c.id.startsWith(only));
if (!chosen.length) throw new Error(`no clip starts with ${only}`);
sources();
for (const c of chosen) {
	console.log(`media ${c.id}`);
	c.make();
}
const tsv = join(mediaRoot, 'audio-matrix.tsv');
writeFileSync(tsv, chosen.map((c) => `${c.id}\t${c.how}\t${c.path}\t-\t${c.what}`).join('\n') + '\n');
console.log(`matrix written to ${tsv}`);
if (flag('--make-only')) process.exit(0);
if (!logPath || !existsSync(logPath)) throw new Error('name the cast-dev server log: --log <server.log> (or CAST_DEV_LOG)');

let offset = statSync(logPath).size;
const windows = new Map<string, string[]>();
let current: string | null = null;
const collect = () => {
	const r = readFrom(logPath, offset);
	offset = r.to;
	if (current) for (const l of r.text.split('\n')) if (/ receiver (setup|status) /.test(l)) windows.get(current)!.push(l);
};
const poll = setInterval(collect, 500);

const [cmd, ...rest] = castDev.split(/\s+/);
const child = spawn(cmd, [...rest, '--matrix', tsv, '--seconds', String(seconds), ...(mediaBase ? ['--media-base', mediaBase] : [])], { cwd: repo, stdio: ['ignore', 'pipe', 'inherit'] });
let buffered = '';
child.stdout.on('data', (chunk: Buffer) => {
	buffered += chunk.toString('utf8');
	let i: number;
	while ((i = buffered.search(/\r?\n/)) >= 0) {
		const line = buffered.slice(0, i);
		buffered = buffered.slice(i + (buffered[i] === '\r' ? 2 : 1));
		console.log(`  cast_dev: ${line}`);
		const start = line.match(/^\[\d+\/\d+\] (\S+)/);
		if (start) {
			collect();
			current = start[1];
			windows.set(current, []);
		} else if (/^\s*-> /.test(line)) {
			collect();
			current = null;
		}
	}
});
const code: number = await new Promise((r) => child.on('close', (c) => r(c ?? 1)));
// The last lines reach the log a moment after the cast.
await new Promise((r) => setTimeout(r, 1500));
collect();
clearInterval(poll);

let failed = code !== 0 ? 1 : 0;
if (code !== 0) console.log(`cast_dev exited with ${code}`);
console.log('\nresults');
for (const c of chosen) {
	const lines = windows.get(c.id);
	if (!lines) {
		console.log(`FAIL ${c.id}: not cast`);
		failed++;
		continue;
	}
	const { verdict, summary } = judge(c, lines);
	console.log(`${verdict ? 'FAIL' : 'ok  '} ${c.id} (${c.what}): ${verdict ?? 'as expected'}\n     ${summary}`);
	if (verdict) failed++;
}
console.log(failed ? `\n${failed} failed` : '\nall passed');
process.exit(failed ? 1 : 0);
