// Plays every container, audio codec and subtitle type the receiver unpacks itself in headless
// Chrome, with the stand-in for the Cast SDK, and checks what plays and how: MediaSource or
// WebAssembly audio, the subtitles drawn, seeking, and audio kept in step through pause, rate
// changes and seeks.
//
//   bun scripts/cast-receiver.ts && bun scripts/cast-receiver-formats-check.ts [--only <name>]
//
// The media comes from scripts/cast-receiver/test-media.ts (FFmpeg test patterns and tones, made
// here when missing). Needs Google Chrome and ffmpeg. Desktop Chrome has no AC-3 or E-AC-3, so
// those play through the WebAssembly decoders here, as on a TV without them.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const site = join(here, '..');
const receiver = join(site, 'public/cast/receiver');
const media = join(site, '.cast-media');
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const onlyArg = process.argv.indexOf('--only');
const only = onlyArg > 0 ? process.argv[onlyArg + 1] : null;
const report = process.env.FORMATS_REPORT ?? null;

if (!existsSync(join(receiver, 'app.js')) || !existsSync(join(receiver, 'pcm/worker.js')))
	throw new Error('build the receiver first: bun scripts/cast-receiver.ts');
if (!existsSync(join(media, 'sync-h264-ac3.mkv'))) {
	const r = spawnSync('bun', [join(here, 'cast-receiver/test-media.ts'), media], { stdio: 'inherit' });
	if (r.status !== 0) throw new Error('making the test media failed');
}

// ---------- Server ----------

const stub = readFileSync(join(here, 'cast-receiver/caf-stub.js'), 'utf8');
const types: Record<string, string> = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript',
	'.css': 'text/css',
	'.json': 'application/json',
	'.wasm': 'application/wasm',
	'.woff2': 'font/woff2',
	'.mp4': 'video/mp4',
	'.m4s': 'video/iso.segment',
	'.mkv': 'video/x-matroska',
	'.webm': 'video/webm',
	'.ts': 'video/mp2t',
	'.m3u8': 'application/vnd.apple.mpegurl',
	'.mpd': 'application/dash+xml',
	'.sup': 'application/octet-stream',
};
let requests = 0;

function serveFile(path: string, req: Request): Response {
	if (!existsSync(path)) return new Response('not found', { status: 404 });
	const file = Bun.file(path);
	const headers = { 'Content-Type': types[extname(path)] ?? 'application/octet-stream', 'Accept-Ranges': 'bytes', 'Access-Control-Allow-Origin': '*' };
	const range = req.headers.get('range')?.match(/bytes=(\d*)-(\d*)/);
	if (range) {
		const size = file.size;
		const start = range[1] ? Number(range[1]) : size - Number(range[2]);
		const end = Math.min(size - 1, range[1] && range[2] ? Number(range[2]) : size - 1);
		return new Response(file.slice(start, end + 1), { status: 206, headers: { ...headers, 'Content-Range': `bytes ${start}-${end}/${size}` } });
	}
	return new Response(file, { headers });
}

const server = Bun.serve({
	port: 0,
	idleTimeout: 0,
	hostname: '127.0.0.1',
	fetch(req) {
		const path = decodeURIComponent(new URL(req.url).pathname);
		if (path === '/cast/receiver/' || path === '/cast/receiver/index.html') {
			const html = readFileSync(join(receiver, 'index.html'), 'utf8').replace(
				/<script src="https:\/\/www\.gstatic\.com[^"]+"><\/script>/,
				'<script src="caf-stub.js"></script>',
			);
			return new Response(html, { headers: { 'Content-Type': types['.html'] } });
		}
		if (path === '/cast/receiver/caf-stub.js') return new Response(stub, { headers: { 'Content-Type': types['.js'] } });
		if (path.startsWith('/media/')) requests++;
		// A URL that names no format: the receiver reads it from the bytes.
		if (path === '/media/stream') return serveFile(join(media, 'mkv-h264-aac.mkv'), req);
		const [root, rest] = path.startsWith('/media/') ? [media, path.slice(7)] : path.startsWith('/cast/receiver/') ? [receiver, path.slice(15)] : [null, ''];
		if (!root || normalize(rest).startsWith('..')) return new Response('not found', { status: 404 });
		return serveFile(join(root, rest), req);
	},
});
const origin = `http://127.0.0.1:${server.port}`;
const at = (p: string) => `${origin}/media/${p}`;

// ---------- Chrome over the DevTools protocol ----------

const work = mkdtempSync(join(tmpdir(), 'receiver-formats-'));
const profile = join(work, 'chrome');
const browser = spawn(
	chrome,
	[
		'--headless=new',
		'--remote-debugging-port=0',
		`--user-data-dir=${profile}`,
		'--no-first-run',
		'--no-default-browser-check',
		'--autoplay-policy=no-user-gesture-required',
		'--mute-audio',
		'--enable-precise-memory-info',
		'about:blank',
	],
	{ stdio: 'ignore' },
);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function devtoolsUrl(): Promise<string> {
	const file = join(profile, 'DevToolsActivePort');
	for (let i = 0; i < 100; i++) {
		if (existsSync(file)) {
			const [port, path] = readFileSync(file, 'utf8').trim().split('\n');
			if (port && path) return `ws://127.0.0.1:${port}${path}`;
		}
		await sleep(100);
	}
	throw new Error('Chrome did not start');
}

const ws = new WebSocket(await devtoolsUrl());
await new Promise((ok, fail) => {
	ws.addEventListener('open', ok);
	ws.addEventListener('error', fail);
});
let nextId = 0;
const waiting = new Map<number, { ok: (v: unknown) => void; fail: (e: Error) => void }>();
const logs: string[] = [];
ws.addEventListener('message', (e) => {
	const m = JSON.parse(String(e.data));
	if (m.id != null) {
		const w = waiting.get(m.id);
		waiting.delete(m.id);
		if (m.error) w?.fail(new Error(m.error.message));
		else w?.ok(m.result);
	} else if (m.method === 'Runtime.consoleAPICalled') {
		logs.push(`${m.params.type}: ${(m.params.args as { value?: unknown; description?: string }[]).map((a) => a.value ?? a.description).join(' ')}`);
	} else if (m.method === 'Runtime.exceptionThrown') {
		logs.push(`exception: ${m.params.exceptionDetails?.exception?.description ?? m.params.exceptionDetails?.text}`);
	}
});
function send<T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}, sessionId?: string): Promise<T> {
	const id = ++nextId;
	ws.send(JSON.stringify({ id, method, params, sessionId }));
	return new Promise((ok, fail) => waiting.set(id, { ok: ok as (v: unknown) => void, fail }));
}
const { targetId } = await send<{ targetId: string }>('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await send<{ sessionId: string }>('Target.attachToTarget', { targetId, flatten: true });
const page = <T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}) => send<T>(method, params, sessionId);
await page('Runtime.enable');
await page('Emulation.setDeviceMetricsOverride', { width: 1280, height: 720, deviceScaleFactor: 1.5, mobile: false });

async function js<T = unknown>(expression: string): Promise<T> {
	const r = await page<{ result: { value?: T }; exceptionDetails?: { text: string; exception?: { description?: string } } }>('Runtime.evaluate', {
		expression,
		awaitPromise: true,
		returnByValue: true,
	});
	if (r.exceptionDetails) throw new Error(`${expression}: ${r.exceptionDetails.exception?.description ?? r.exceptionDetails.text}`);
	return r.result.value as T;
}
async function until(expression: string, timeout = 10_000): Promise<boolean> {
	const end = Date.now() + timeout;
	while (Date.now() < end) {
		if (await js<boolean>(expression).catch(() => false)) return true;
		await sleep(100);
	}
	return false;
}

const failures: string[] = [];
function expect(ok: boolean, what: string) {
	console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
	if (!ok) failures.push(what);
}

interface Stats {
	format: string;
	video: string | null;
	audio: string | null;
	audioPath: 'mse' | 'wasm' | null;
	text: string | null;
	readTo: number;
	seeks: number;
	pcm: { drift: number; worst: number; jumps: number; starved: number; queued: number; time: number } | null;
}
const stats = () => js<Stats | null>('__receiver.formats.current?.stats() ?? null');
const time = () => js<number>('document.getElementById("video").currentTime');
const errors = () => js<Record<string, unknown>[]>(`__caf.sent.filter((m) => m.body.type === 'error').map((m) => m.body)`);

interface Case {
	name: string;
	file: string;
	type?: string;
	container?: string;
	audio?: number;
	/** A subtitle track inside the file (index), or a side file (url). */
	subtitle?: { inside?: number; url?: string; format: string };
	format: string;
	video?: string;
	codec?: string;
	path?: 'mse' | 'wasm';
	text?: 'cues' | 'image' | 'typeset';
	/** Seek here once playing and check it plays on from there. */
	seek?: number;
}

const cases: Case[] = [
	// Matroska, every audio codec
	{ name: 'mkv h264 aac', file: 'mkv-h264-aac.mkv', format: 'mkv', video: 'avc1', codec: 'aac', path: 'mse', seek: 5 },
	{ name: 'mkv h264 ac3', file: 'mkv-h264-ac3.mkv', format: 'mkv', video: 'avc1', codec: 'ac3', path: 'wasm', seek: 5 },
	{ name: 'mkv h264 eac3', file: 'mkv-h264-eac3.mkv', format: 'mkv', video: 'avc1', codec: 'eac3', path: 'wasm' },
	{ name: 'mkv h264 dts', file: 'mkv-h264-dts.mkv', format: 'mkv', video: 'avc1', codec: 'dts', path: 'wasm' },
	{ name: 'mkv h264 truehd', file: 'mkv-h264-truehd.mkv', format: 'mkv', video: 'avc1', codec: 'truehd', path: 'wasm' },
	{ name: 'mkv h264 flac', file: 'mkv-h264-flac.mkv', format: 'mkv', video: 'avc1', codec: 'flac', path: 'mse' },
	{ name: 'mkv h264 mp3', file: 'mkv-h264-mp3.mkv', format: 'mkv', video: 'avc1', codec: 'mp3', path: 'wasm' },
	{ name: 'mkv h264 mp2', file: 'mkv-h264-mp2.mkv', format: 'mkv', video: 'avc1', codec: 'mp2', path: 'wasm' },
	{ name: 'mkv h264 opus', file: 'mkv-h264-opus.mkv', format: 'mkv', video: 'avc1', codec: 'opus', path: 'mse' },
	{ name: 'mkv h264 pcm', file: 'mkv-h264-pcm.mkv', format: 'mkv', video: 'avc1', path: 'wasm' },
	{ name: 'mkv hevc ac3', file: 'mkv-hevc-ac3.mkv', format: 'mkv', video: 'hvc1', codec: 'ac3', path: 'wasm' },
	{ name: 'mkv av1 opus', file: 'mkv-av1-opus.mkv', format: 'mkv', video: 'av01', codec: 'opus', path: 'mse' },
	{ name: 'mkv second audio track (ac3)', file: 'mkv-two-audio.mkv', audio: 1, format: 'mkv', video: 'avc1', codec: 'ac3', path: 'wasm' },
	// WebM
	{ name: 'webm vp9 opus', file: 'webm-vp9-opus.webm', format: 'mkv', video: 'vp09', codec: 'opus', path: 'mse', seek: 5 },
	{ name: 'webm vp8 vorbis', file: 'webm-vp8-vorbis.webm', format: 'mkv', video: 'vp8', codec: 'vorbis', path: 'mse' },
	{ name: 'webm av1 opus', file: 'webm-av1-opus.webm', format: 'mkv', video: 'av01', codec: 'opus', path: 'mse' },
	// MP4
	{ name: 'mp4 moov at the end', file: 'mp4-h264-aac.mp4', format: 'mp4', video: 'avc1', codec: 'aac', path: 'mse', seek: 5 },
	{ name: 'mp4 faststart', file: 'mp4-h264-aac-faststart.mp4', format: 'mp4', video: 'avc1', codec: 'aac', path: 'mse' },
	{ name: 'mp4 fragmented', file: 'mp4-frag-h264-aac.mp4', format: 'mp4', video: 'avc1', codec: 'aac', path: 'mse', seek: 5 },
	{ name: 'mp4 h264 ac3', file: 'mp4-h264-ac3.mp4', format: 'mp4', video: 'avc1', codec: 'ac3', path: 'wasm' },
	{ name: 'mp4 h264 flac', file: 'mp4-h264-flac.mp4', format: 'mp4', video: 'avc1', codec: 'flac', path: 'mse' },
	{ name: 'mp4 h264 opus', file: 'mp4-h264-opus.mp4', format: 'mp4', video: 'avc1', codec: 'opus', path: 'mse' },
	{ name: 'mp4 hevc eac3', file: 'mp4-hevc-eac3.mp4', format: 'mp4', video: 'hvc1', codec: 'eac3', path: 'wasm' },
	// MPEG-TS
	{ name: 'ts h264 aac', file: 'ts-h264-aac.ts', format: 'ts', video: 'avc1', codec: 'aac', path: 'mse', seek: 5 },
	{ name: 'ts h264 ac3', file: 'ts-h264-ac3.ts', format: 'ts', video: 'avc1', codec: 'ac3', path: 'wasm', seek: 5 },
	{ name: 'ts h264 mp2', file: 'ts-h264-mp2.ts', format: 'ts', video: 'avc1', codec: 'mp2', path: 'wasm' },
	{ name: 'ts h264 dts', file: 'ts-h264-dts.ts', format: 'ts', video: 'avc1', codec: 'dts', path: 'wasm' },
	{ name: 'ts h264 mp3', file: 'ts-h264-mp3.ts', format: 'ts', video: 'avc1', codec: 'mp3', path: 'wasm' },
	{ name: 'ts hevc eac3', file: 'ts-hevc-eac3.ts', format: 'ts', video: 'hvc1', codec: 'eac3', path: 'wasm' },
	// Playlists
	{ name: 'hls ts', file: 'hls-ts/index.m3u8', format: 'hls', video: 'avc1', codec: 'aac', path: 'mse', seek: 6 },
	{ name: 'hls fmp4 with ac3 rendition', file: 'hls-fmp4/master.m3u8', format: 'hls', video: 'avc1', codec: 'ac3', path: 'wasm', seek: 6 },
	{ name: 'dash timeline', file: 'dash/manifest.mpd', format: 'dash', video: 'avc1', codec: 'aac', path: 'mse', seek: 6 },
	{ name: 'dash numbered eac3', file: 'dash-eac3/manifest.mpd', format: 'dash', video: 'avc1', codec: 'eac3', path: 'wasm' },
	// A file with no extension or type: read from its bytes
	{ name: 'sniffed matroska', file: 'stream', type: 'application/octet-stream', format: 'mkv', video: 'avc1', codec: 'aac', path: 'mse' },
	// Subtitles inside the file, and a side PGS file
	{ name: 'subtitles srt', file: 'mkv-srt.mkv', subtitle: { inside: 0, format: 'srt' }, format: 'mkv', text: 'cues' },
	{ name: 'subtitles webvtt', file: 'mkv-webvtt.mkv', subtitle: { inside: 0, format: 'vtt' }, format: 'mkv', text: 'cues' },
	{ name: 'subtitles mp4 timed text', file: 'mp4-tx3g.mp4', subtitle: { inside: 0, format: 'srt' }, format: 'mp4', text: 'cues' },
	{ name: 'subtitles ass typeset', file: 'mkv-ass.mkv', subtitle: { inside: 0, format: 'ass' }, format: 'mkv', text: 'typeset' },
	{ name: 'subtitles pgs', file: 'mkv-pgs.mkv', subtitle: { inside: 0, format: 'pgs' }, format: 'mkv', text: 'image' },
	{ name: 'subtitles vobsub', file: 'mkv-vobsub.mkv', subtitle: { inside: 0, format: 'vobsub' }, format: 'mkv', text: 'image' },
	{ name: 'subtitles side pgs file', file: 'mkv-h264-aac.mkv', subtitle: { url: 'subs.sup', format: 'pgs' }, format: 'mkv', text: 'image' },
];

function request(c: Case, start = 0) {
	const sub = c.subtitle;
	return {
		currentTime: start,
		media: {
			contentId: at(c.file),
			contentType: c.type ?? types[extname(c.file)] ?? 'video/mp4',
			streamType: 'BUFFERED',
			metadata: { metadataType: 2, seriesTitle: 'Lanterns of the Quiet Harbor', title: c.name },
			tracks: sub ? [{ trackId: 7, type: 'TEXT', trackContentId: sub.url ? at('subs.srt') : 'x', trackContentType: 'text/vtt', subtype: 'SUBTITLES', language: 'en' }] : [],
			customData: {
				kaeri: {
					version: 1,
					audio: c.audio != null ? { index: c.audio } : null,
					subtitles: sub ? [{ id: 7, format: sub.format, language: 'en', ...(sub.url ? { url: at(sub.url) } : { inside: sub.inside }) }] : [],
					subtitle: sub ? 7 : null,
					device: 'iphone',
				},
			},
		},
	};
}

/** Alpha drawn on the image subtitles' canvas. */
const imagePixels = () =>
	js<number>(`(() => {
		const c = document.getElementById('image-subs');
		if (!c || !c.width) return 0;
		const d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
		let n = 0;
		for (let i = 3; i < d.length; i += 4) if (d[i]) n++;
		return n;
	})()`);

const results: string[] = [];

async function playCase(c: Case) {
	await js('__caf.sent.length = 0');
	logs.length = 0;
	await js(`__caf.load(${JSON.stringify(request(c))})`);
	const playing = await until(`document.getElementById('video').currentTime > 1.2`, 15_000);
	const s = await stats();
	const errs = await errors();
	const label = c.name;
	expect(playing, `${label}: plays`);
	expect(!!s && s.format === c.format, `${label}: unpacked as ${c.format} (${s?.format ?? 'not unpacked'})`);
	if (c.video) expect(s?.video === c.video, `${label}: video ${c.video} (${s?.video})`);
	if (c.codec) expect(s?.audio === c.codec, `${label}: audio ${c.codec} (${s?.audio})`);
	if (c.path) expect(s?.audioPath === c.path, `${label}: audio through ${c.path === 'mse' ? 'MediaSource' : 'WebAssembly'} (${s?.audioPath})`);
	expect(await js<boolean>('document.getElementById("video").videoWidth > 0'), `${label}: a picture`);
	if (c.path === 'mse') expect(await until('document.getElementById("video").webkitAudioDecodedByteCount > 0', 3000), `${label}: MediaSource decodes the audio`);
	if (c.path === 'wasm') {
		if (process.env.FORMATS_DEBUG)
			for (let i = 0; i < 12; i++) {
				console.log(JSON.stringify({ t: await time(), p: (await stats())?.pcm }));
				await sleep(300);
			}
		await sleep(800);
		const p = (await stats())?.pcm;
		expect(!!p && p.time > 1 && Math.abs(p.drift) < 0.05, `${label}: WebAssembly audio plays in step (at ${p?.time.toFixed(2)} s, drift ${((p?.drift ?? 0) * 1000).toFixed(1)} ms)`);
	}
	if (c.text === 'cues') {
		await js('document.getElementById("video").currentTime = 0.5');
		const ok = await until(`[...document.getElementById('video').textTracks].some((t) => t.label === 'kaeri' && t.mode === 'showing' && t.activeCues && t.activeCues.length > 0)`, 5000);
		expect(ok, `${label}: a cue shows`);
	} else if (c.text === 'image') {
		await js('document.getElementById("video").pause(); document.getElementById("video").currentTime = 0.8');
		await sleep(600);
		const shown = await imagePixels();
		await js('document.getElementById("video").currentTime = 2.2');
		await sleep(600);
		const gap = await imagePixels();
		expect(shown > 50 && gap === 0, `${label}: the picture shows in its cue and clears after (${shown} px, then ${gap} px)`);
		await js('document.getElementById("video").play()');
	} else if (c.text === 'typeset') {
		expect(await until('__receiver.typeset.active', 8000), `${label}: typeset by JASSUB`);
	}
	if (c.seek != null) {
		await js(`document.getElementById('video').currentTime = ${c.seek}`);
		const on = await until(`document.getElementById('video').currentTime > ${c.seek + 0.8} && !document.getElementById('video').seeking`, 10_000);
		expect(on, `${label}: plays on after a seek to ${c.seek} s`);
		if (c.path === 'wasm') {
			await sleep(600);
			const p = (await stats())?.pcm;
			expect(!!p && Math.abs(p.drift) < 0.05 && p.time > c.seek, `${label}: in step after the seek (drift ${((p?.drift ?? 0) * 1000).toFixed(1)} ms)`);
		}
	}
	const unexpected = errs.length ? errs : await errors();
	expect(unexpected.length === 0, `${label}: no error sent${unexpected.length ? ` (${JSON.stringify(unexpected[0])})` : ''}`);
	if (failures.length && logs.length) console.log(logs.filter((l) => !/^(log|debug):/.test(l)).slice(-6).map((l) => `     ${l}`).join('\n'));
	results.push(`${c.name}\t${s?.format}\t${s?.video}\t${s?.audio}\t${s?.audioPath}\t${playing ? 'plays' : 'no'}`);
}

/** Seek, pause, rate and drift with AC-3 in WebAssembly, over 40 s of media. */
async function syncCase() {
	const label = 'sync ac3 (WebAssembly)';
	await js('__caf.sent.length = 0');
	await js(`__caf.load(${JSON.stringify(request({ name: label, file: 'sync-h264-ac3.mkv', format: 'mkv' }))})`);
	expect(await until(`document.getElementById('video').currentTime > 3`, 15_000), `${label}: plays`);
	const drift = async () => (await stats())?.pcm;
	await sleep(3000);
	let p = await drift();
	expect(!!p && Math.abs(p.drift) < 0.02, `${label}: drift after 6 s of play ${((p?.drift ?? 0) * 1000).toFixed(1)} ms (worst ${((p?.worst ?? 0) * 1000).toFixed(1)} ms)`);

	await js('document.getElementById("video").pause()');
	await sleep(400);
	const pausedAt = (await drift())?.time ?? 0;
	await sleep(1500);
	const later = (await drift())?.time ?? 0;
	expect(Math.abs(later - pausedAt) < 0.02, `${label}: audio holds while paused (moved ${((later - pausedAt) * 1000).toFixed(1)} ms)`);
	await js('document.getElementById("video").play()');
	await sleep(2000);
	p = await drift();
	expect(!!p && Math.abs(p.drift) < 0.02, `${label}: in step after resuming (${((p?.drift ?? 0) * 1000).toFixed(1)} ms)`);

	await js('document.getElementById("video").playbackRate = 1.5');
	await sleep(4000);
	p = await drift();
	expect(!!p && Math.abs(p.drift) < 0.03, `${label}: in step at 1.5x (${((p?.drift ?? 0) * 1000).toFixed(1)} ms)`);
	await js('document.getElementById("video").playbackRate = 0.75');
	await sleep(3000);
	p = await drift();
	expect(!!p && Math.abs(p.drift) < 0.03, `${label}: in step at 0.75x (${((p?.drift ?? 0) * 1000).toFixed(1)} ms)`);
	await js('document.getElementById("video").playbackRate = 1');

	for (const target of [30, 4, 22]) {
		await js(`document.getElementById("video").currentTime = ${target}`);
		await until(`document.getElementById('video').currentTime > ${target + 0.5} && !document.getElementById('video').seeking`, 10_000);
		await sleep(1500);
		p = await drift();
		const t = await time();
		if (process.env.FORMATS_DEBUG) console.log(JSON.stringify({ target, t, p, s: await stats() }));
		expect(!!p && Math.abs(p.drift) < 0.02 && Math.abs(p.time - t) < 0.3, `${label}: in step after a seek to ${target} s (${((p?.drift ?? 0) * 1000).toFixed(1)} ms)`);
	}
	p = await drift();
	const cpu = await js<{ cpu: number; decoded: number; heap: number } | null>('__receiver.formats.current?.pcmCpu() ?? null');
	const heap = await js<number>('performance.memory ? performance.memory.usedJSHeapSize : 0');
	const line = `sync: jumps ${p?.jumps}, starved ${p?.starved}, decoder CPU ${cpu ? ((cpu.cpu / 1000 / Math.max(1, cpu.decoded)) * 100).toFixed(2) : '?'} % of real time, decoder heap ${cpu ? (cpu.heap / 1048576).toFixed(0) : '?'} MB, page JS heap ${(heap / 1048576).toFixed(1)} MB`;
	console.log(`     ${line}`);
	results.push(line);
	expect((await errors()).length === 0, `${label}: no error sent`);
}

/** Switching audio in place: from AC-3 (WebAssembly) to AAC. */
async function switchCase() {
	const label = 'audio switch';
	await js('__caf.sent.length = 0');
	await js(`__caf.load(${JSON.stringify(request({ name: label, file: 'mkv-two-audio.mkv', format: 'mkv', audio: 1 }))})`);
	expect(await until(`document.getElementById('video').currentTime > 1.5`, 15_000), `${label}: plays`);
	expect((await stats())?.audio === 'ac3', `${label}: starts on the second track`);
	await js(`__caf.message({ type: 'audio', version: 1, language: 'jpn' })`);
	expect(await until(`__receiver.formats.current?.stats().audio === 'aac'`, 5000), `${label}: switches to the first track`);
	const t = await time();
	expect(await until(`document.getElementById('video').currentTime > ${t + 1}`, 6000), `${label}: plays on`);
}

/** A video codec MediaSource can't take is reported as video, naming it. */
async function refusedVideoCase() {
	const label = 'unplayable video';
	const file = 'mkv-mpeg2.mkv';
	if (!existsSync(join(media, file)))
		spawnSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', 'testsrc2=size=320x180:rate=24:duration=3', '-c:v', 'mpeg2video', join(media, file)]);
	await js('__caf.sent.length = 0');
	await js(`__caf.load(${JSON.stringify(request({ name: label, file, format: 'mkv' }))})`);
	expect(await until(`__caf.sent.some((m) => m.body.type === 'error')`, 8000), `${label}: an error is sent`);
	const e = (await errors())[0];
	expect(e?.part === 'video' && e?.codec === 'mpeg2', `${label}: part video, codec mpeg2 (${JSON.stringify(e)})`);
}

try {
	await page('Page.navigate', { url: `${origin}/cast/receiver/` });
	if (!(await until('!!window.__receiver?.formats'))) throw new Error('the receiver did not start');
	await js(`__caf.connect('sender-1', 'Mozilla/5.0 (iPhone)')`);
	for (const c of cases) if (!only || c.name.includes(only)) await playCase(c);
	if (!only || 'sync'.includes(only)) await syncCase();
	if (!only || 'switch'.includes(only)) await switchCase();
	if (!only || 'refused'.includes(only)) await refusedVideoCase();
} finally {
	await js('__caf.load({ media: { contentId: "about:blank" } })').catch(() => {});
	ws.close();
	browser.kill();
	server.stop(true);
	await sleep(300);
	rmSync(work, { recursive: true, force: true });
}

if (report) writeFileSync(report, `${results.join('\n')}\n`);
console.log(`\n${failures.length ? `${failures.length} failed` : 'all passed'} (${requests} media requests)`);
process.exit(failures.length ? 1 : 0);
