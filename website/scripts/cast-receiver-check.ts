// Drives the built Cast receiver through every screen in headless Chrome with a stand-in for the
// Cast SDK, checks its texts and the messages it sends, and saves a shot of each screen.
//
//   bun run build && bun scripts/cast-receiver-check.ts [--shots <dir>]
//
// The media is made here by ffmpeg (a test pattern, a tone, made-up subtitles); nothing is fetched
// from outside. Needs Google Chrome and ffmpeg.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const site = join(here, '..');
const receiver = join(site, 'dist/cast/receiver');
const shotsArg = process.argv.indexOf('--shots');
const shots = shotsArg > 0 ? process.argv[shotsArg + 1] : mkdtempSync(join(tmpdir(), 'receiver-shots-'));
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

if (!existsSync(join(receiver, 'app.js'))) throw new Error('build the site first: bun run build');
mkdirSync(shots, { recursive: true });

// ---------- Media ----------

const work = mkdtempSync(join(tmpdir(), 'receiver-check-'));
const media = join(work, 'media');
mkdirSync(media);
function ffmpeg(...args: string[]) {
	const r = spawnSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...args], { cwd: media });
	if (r.status !== 0) throw new Error(`ffmpeg ${args.at(-1)}: ${r.stderr}`);
}
ffmpeg(
	'-f', 'lavfi', '-i', 'testsrc2=size=1280x720:rate=24:duration=40',
	'-f', 'lavfi', '-i', 'sine=frequency=330:duration=40',
	'-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-preset', 'veryfast', '-c:a', 'aac', '-b:a', '64k', '-movflags', '+faststart', 'ep.mp4',
);
ffmpeg('-f', 'lavfi', '-i', 'sine=frequency=220:duration=30', '-c:a', 'aac', '-movflags', '+faststart', 'sound.mp4');
ffmpeg('-f', 'lavfi', '-i', 'gradients=s=1280x720:c0=0x2a3a6b:c1=0x7a4a6b:seed=3:duration=1', '-frames:v', '1', 'art.png');
ffmpeg('-f', 'lavfi', '-i', 'gradients=s=1280x720:c0=0x2f6b62:c1=0xc9a46a:seed=9:duration=1', '-frames:v', '1', 'still.png');
const font = readdirSync(join(receiver, 'fonts')).find((f) => /zen-kaku-gothic-new-latin-700/.test(f))!;
writeFileSync(join(media, 'font.woff2'), readFileSync(join(receiver, 'fonts', font)));
writeFileSync(
	join(media, 'ep.ass'),
	`[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Zen Kaku Gothic New,72,&H00FFFFFF,&H000000FF,&H00201010,&H80000000,-1,0,0,0,100,100,0,0,1,4,1,2,80,80,60,1
Style: Sign,Zen Kaku Gothic New,64,&H0060D0F0,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,2,-6,1,3,0,8,80,80,120,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:40.00,Default,,0,0,0,,The ferry leaves at first light.
Dialogue: 0,0:00:00.00,0:00:40.00,Sign,,0,0,0,,{\\fad(300,300)}QUIET HARBOR
`,
);
writeFileSync(join(media, 'ep.vtt'), 'WEBVTT\n\n00:00.000 --> 00:40.000\nThe ferry leaves at first light.\n');

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
	'.png': 'image/png',
	'.ass': 'text/plain; charset=utf-8',
	'.vtt': 'text/vtt',
	'.txt': 'text/plain',
};

function serveFile(path: string, req: Request): Response {
	if (!existsSync(path)) return new Response('not found', { status: 404 });
	const file = Bun.file(path);
	const type = types[extname(path)] ?? 'application/octet-stream';
	const range = req.headers.get('range')?.match(/bytes=(\d*)-(\d*)/);
	if (range) {
		const size = file.size;
		const start = range[1] ? Number(range[1]) : size - Number(range[2]);
		const end = range[1] && range[2] ? Number(range[2]) : size - 1;
		return new Response(file.slice(start, end + 1), {
			status: 206,
			headers: { 'Content-Type': type, 'Content-Range': `bytes ${start}-${end}/${size}`, 'Accept-Ranges': 'bytes' },
		});
	}
	return new Response(file, { headers: { 'Content-Type': type, 'Accept-Ranges': 'bytes' } });
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
			if (!html.includes('caf-stub.js')) throw new Error('the page no longer loads the SDK from gstatic');
			return new Response(html, { headers: { 'Content-Type': types['.html'] } });
		}
		if (path === '/cast/receiver/caf-stub.js') return new Response(stub, { headers: { 'Content-Type': types['.js'] } });
		const [root, rest] = path.startsWith('/media/') ? [media, path.slice(7)] : path.startsWith('/cast/receiver/') ? [receiver, path.slice(15)] : [null, ''];
		if (!root || normalize(rest).startsWith('..')) return new Response('not found', { status: 404 });
		return serveFile(join(root, rest), req);
	},
});
const origin = `http://127.0.0.1:${server.port}`;
const at = (p: string) => `${origin}/media/${p}`;

// ---------- Chrome over the DevTools protocol ----------

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
		// The page is the only one and must keep its timers at full rate.
		'--disable-background-timer-throttling',
		'--disable-renderer-backgrounding',
		'--disable-backgrounding-occluded-windows',
		'--hide-scrollbars',
		'--lang=en-US',
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

type Handler = (params: Record<string, unknown>) => void;
class Cdp {
	private id = 0;
	private waiting = new Map<number, { ok: (v: unknown) => void; fail: (e: Error) => void }>();
	private handlers = new Map<string, Handler[]>();
	constructor(private ws: WebSocket) {
		ws.addEventListener('message', (e) => {
			const m = JSON.parse(String(e.data));
			if (m.id != null) {
				const w = this.waiting.get(m.id);
				this.waiting.delete(m.id);
				if (m.error) w?.fail(new Error(`${m.error.message} ${m.error.data ?? ''}`));
				else w?.ok(m.result);
			} else for (const h of this.handlers.get(m.method) ?? []) h(m.params);
		});
	}
	static async open(url: string) {
		const ws = new WebSocket(url);
		await new Promise((ok, fail) => {
			ws.addEventListener('open', ok);
			ws.addEventListener('error', fail);
		});
		return new Cdp(ws);
	}
	send<T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}, sessionId?: string): Promise<T> {
		const id = ++this.id;
		this.ws.send(JSON.stringify({ id, method, params, sessionId }));
		return new Promise((ok, fail) => this.waiting.set(id, { ok: ok as (v: unknown) => void, fail }));
	}
	on(method: string, h: Handler) {
		this.handlers.set(method, [...(this.handlers.get(method) ?? []), h]);
	}
	close() {
		this.ws.close();
	}
}

const failures: string[] = [];
const consoleErrors: string[] = [];
function expect(ok: boolean, what: string) {
	console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
	if (!ok) failures.push(what);
}

const cdp = await Cdp.open(await devtoolsUrl());
const { targetId } = await cdp.send<{ targetId: string }>('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await cdp.send<{ sessionId: string }>('Target.attachToTarget', { targetId, flatten: true });
const page = <T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}) => cdp.send<T>(method, params, sessionId);
cdp.on('Runtime.exceptionThrown', (p) => consoleErrors.push(JSON.stringify((p.exceptionDetails as { exception?: { description?: string } }).exception?.description ?? p)));
cdp.on('Runtime.consoleAPICalled', (p) => {
	if (p.type === 'error') consoleErrors.push((p.args as { value?: unknown; description?: string }[]).map((a) => a.value ?? a.description).join(' '));
});
await page('Page.bringToFront');
await page('Emulation.setFocusEmulationEnabled', { enabled: true });
await page('Runtime.enable');
await page('Page.enable');
// A 1080p Chromecast gives the page 1280 x 720 CSS px at 1.5x.
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
async function until(expression: string, what: string, timeout = 8000) {
	const end = Date.now() + timeout;
	while (Date.now() < end) {
		if (await js<boolean>(expression).catch(() => false)) return true;
		await sleep(100);
	}
	expect(false, `${what} (waited for ${expression})`);
	return false;
}
/** Shots of the same screen at 1920 x 1080 (the TV's pixels) and at 1280 x 720. */
async function shotBoth(name: string) {
	await shot(`${name}-1080`);
	await page('Emulation.setDeviceMetricsOverride', { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false });
	await shot(`${name}-720`);
	await page('Emulation.setDeviceMetricsOverride', { width: 1280, height: 720, deviceScaleFactor: 1.5, mobile: false });
}
/** The Up next card stands clear of the bottom fifth, where subtitles show. */
const upNextClear = () => js<boolean>(`(() => { const r = document.getElementById('upnext').getBoundingClientRect(); return innerHeight - r.bottom >= 168 * Math.min(innerWidth / 1280, innerHeight / 720) - 1; })()`);
async function shot(name: string) {
	await sleep(350); // the crossfade in
	const { data } = await page<{ data: string }>('Page.captureScreenshot', { format: 'png' });
	writeFileSync(join(shots, `${name}.png`), Buffer.from(data, 'base64'));
}
/** The #ui layers that show, and the text of an element. */
const shown = () => js<string[]>(`[...document.querySelectorAll('#ui > .on')].map((e) => e.id)`);
const textOf = (id: string) => js<string>(`document.getElementById(${JSON.stringify(id)}).textContent`);
const sent = () => js<{ to: string | null; body: Record<string, unknown> }[]>('__caf.sent');
const lastSent = async (type: string) => (await sent()).filter((m) => m.body.type === type).at(-1)?.body;
const phase = () => js<string>('document.getElementById("ui").dataset.phase');

const episode = (extra: Record<string, unknown> = {}, media: Record<string, unknown> = {}) => ({
	media: {
		contentId: at('ep.mp4'),
		contentType: 'video/mp4',
		streamType: 'BUFFERED',
		metadata: {
			metadataType: 2,
			seriesTitle: 'Lanterns of the Quiet Harbor',
			title: 'The Ferry at Dawn',
			episode: 3,
			images: [{ url: at('art.png') }],
		},
		tracks: [{ trackId: 1, type: 'TEXT', trackContentId: at('ep.vtt'), trackContentType: 'text/vtt', subtype: 'SUBTITLES', language: 'en' }],
		customData: {
			kaeri: {
				episode: 3,
				still: at('still.png'),
				container: 'mp4',
				video: { codec: 'h264', height: 720 },
				audio: { index: 0, language: 'en', codec: 'aac' },
				subtitles: [{ id: 1, url: at('ep.ass'), format: 'ass', language: 'en', title: 'Full' }],
				subtitle: null,
				fonts: [at('font.woff2')],
				device: 'iphone',
				...extra,
			},
		},
		...media,
	},
});
const load = (request: unknown, play = true) => js(`__caf.load(${JSON.stringify(request)}, { play: ${play} })`);
const say = (data: unknown) => js(`__caf.message(${JSON.stringify(data)})`);

try {
	await page('Page.navigate', { url: `${origin}/cast/receiver/` });
	await until('!!window.__receiver', 'the receiver starts');
	expect(await js<boolean>('__caf.context.started'), 'the receiver starts CAF');
	expect(
		await js<boolean>(`JSON.stringify(__caf.context.options.customNamespaces) === '{"urn:x-cast:app.kaeri.cast":"JSON"}'`),
		'the receiver listens on urn:x-cast:app.kaeri.cast',
	);
	expect(await js<boolean>('__caf.context.options.disableIdleTimeout === true'), "CAF's own idle close is off");

	// Ready
	await js(`__caf.connect('sender-1', 'Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X)')`);
	const ready = await lastSent('ready');
	expect(ready?.version === 1 && typeof ready?.build === 'string', 'ready carries version 1 and the build');
	const caps = ready?.caps as { typeset?: boolean; audioSwitch?: boolean; codecs?: unknown[]; screen?: number[] } | undefined;
	expect(caps?.typeset === true && caps.audioSwitch === true && Array.isArray(caps.codecs) && caps.codecs.length === 0 && caps.screen?.length === 2, 'ready caps: typeset, audioSwitch, no codec list, screen');
	const readies = (await sent()).filter((m) => m.body.type === 'ready').length;
	await say({ type: 'hello', version: 1, language: 'en-US', device: 'iphone' });
	await sleep(400);
	expect((await sent()).filter((m) => m.body.type === 'ready').length === readies + 1, 'hello is answered with ready');
	expect((await shown()).join() === 'ready', 'Ready shows the wordmark and its line only');
	expect((await textOf('ready-line')) === 'Ready. Choose an episode on the iPhone.', 'Ready names the iPhone');
	expect(await js<boolean>(`getComputedStyle(document.getElementById('ready')).animationName === 'drift'`), 'Ready drifts against burn-in');
	await shot('01-ready');
	await say({ type: 'hello', version: 1, language: 'ja', device: 'mac' });
	await until(`document.getElementById('ready-line').textContent.includes('Mac')`, 'Ready in Japanese');
	await shot('01b-ready-ja');
	await say({ type: 'hello', version: 1, language: 'en', device: 'iphone' });
	await until(`document.documentElement.lang === 'en'`, 'back to English');

	// Starting
	await load(episode(), false);
	await sleep(900);
	expect((await phase()) === 'loading', 'a load starts with Starting');
	expect((await textOf('starting-text')) === 'Starting' && (await shown()).includes('art'), 'Starting over the art');
	expect((await textOf('ov-show')) === 'Lanterns of the Quiet Harbor' && (await textOf('ov-episode')) === 'Episode 3 · The Ferry at Dawn', 'Starting names the show and the episode');
	await shot('02-starting');

	// Playing: the overlay, then typeset subtitles
	await js('__caf.play()');
	await until(`document.getElementById('ui').dataset.phase === 'playing'`, 'the episode plays');
	await sleep(1200);
	expect((await shown()).includes('ov'), 'the overlay shows when playing starts');
	expect(/^0:0\d$/.test(await textOf('ov-at')) && (await textOf('ov-left')).startsWith('−0:3'), 'the overlay times');
	await shot('03-overlay');
	await js('__caf.editTracks([1])');
	await until(`__receiver.typeset.active && document.documentElement.classList.contains('typeset')`, 'typeset subtitles start', 10_000);
	await until(`!document.getElementById('ov').classList.contains('on')`, 'the overlay hides after 4 s');
	await sleep(800);
	expect(await js<boolean>(`!!document.querySelector('#stage canvas, video + canvas')`), 'JASSUB draws on its canvas');
	await shot('04-typeset');

	// Paused
	await js(`document.getElementById('video').pause()`);
	await sleep(300);
	expect((await shown()).includes('paused') && (await textOf('paused-text')) === 'Paused', 'Paused');
	await shot('05-paused');
	await js(`document.getElementById('video').play()`);
	await until(`!document.getElementById('paused').classList.contains('on')`, 'playing again');

	// Waiting for the stream
	await js('__caf.buffering(true)');
	await until(`document.getElementById('spin').classList.contains('on')`, 'the spinner after 600 ms', 3000);
	const early = await js<number>('performance.now() - __receiver.S.bufferingAt');
	expect(early >= 600 && !(await shown()).includes('waiting'), `the spinner first, the count not yet (${Math.round(early)} ms)`);
	await sleep(4300 - early);
	expect((await shown()).includes('waiting') && /^Waiting for the stream · [45] s$/.test(await textOf('waiting')), 'then the count');
	await shot('06-waiting');
	await js('__caf.buffering(false)');
	await sleep(400);

	// Skipped and Up next
	await say({ type: 'skipped', version: 1, part: 'opening' });
	await sleep(300);
	expect((await textOf('skipped-text')) === 'Skipped opening', 'Skipped opening');
	await shot('07-skipped');
	await until(`!document.getElementById('skipped').classList.contains('on')`, 'Skipped hides after 3 s', 5000);
	await say({ type: 'up-next', version: 1, number: 4, title: 'Lights on the Water', still: at('still.png'), seconds: 15 });
	await sleep(400);
	expect((await textOf('upnext-title')) === 'Episode 4 · Lights on the Water', 'Up next names the episode');
	expect(/^Up next · plays in 1[45] s$/.test(await textOf('upnext-count')), 'Up next counts down');
	expect(await upNextClear(), 'Up next stands clear of typeset subtitles');
	await shotBoth('08-up-next-typeset');
	await page('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
	await page('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
	await sleep(100);
	expect((await lastSent('up-next'))?.action === 'play', 'select on the remote plays the next episode now');
	await say({ type: 'up-next', version: 1 });
	await sleep(400);
	expect(!(await shown()).includes('upnext'), 'an empty up-next hides it');

	// Plain subtitles: the renderer fell behind
	await js(`__receiver.plainSubtitles('slow')`);
	await sleep(300);
	const plain = await lastSent('subtitles-plain');
	expect(plain?.version === 1 && plain.reason === 'slow', 'subtitles-plain goes to the sender');
	expect(!(await js<boolean>(`document.documentElement.classList.contains('typeset')`)) && !(await js<boolean>('__receiver.typeset.active')), 'the WebVTT twin shows instead');
	expect((await textOf('plain')) === 'Subtitles show as plain text on this TV.', 'the plain subtitles notice');
	await shot('09-plain-subs');
	await say({ type: 'up-next', version: 1, number: 4, title: 'Lights on the Water', still: at('still.png'), seconds: 12 });
	await sleep(400);
	expect(await js<boolean>(`[...document.getElementById('video').textTracks].some((t) => t.mode === 'showing')`), 'the WebVTT twin is on');
	expect(await upNextClear(), 'Up next stands clear of WebVTT subtitles');
	await shotBoth('09b-up-next-webvtt');
	await say({ type: 'up-next', version: 1 });
	await sleep(300);

	// The device went away, and came back
	await say({ type: 'sender-away', version: 1, device: 'iphone' });
	await sleep(400);
	expect((await phase()) === 'gone' && (await textOf('gone-title')) === 'Waiting for the iPhone', 'Waiting for the iPhone');
	expect((await textOf('gone-hint')) === 'Open Kaeri on the iPhone to keep watching.', 'the hint names the app and the device');
	await shot('10-gone');
	await sleep(1100);
	await say({ type: 'hello', version: 1, language: 'en', device: 'iphone' });
	await sleep(200);
	const back = await lastSent('sender-gone');
	expect(typeof back?.waited === 'number' && (back.waited as number) >= 1, 'sender-gone tells how long it waited');
	expect((await phase()) === 'playing', 'the episode goes on');

	// The relay stopped answering mid-episode
	await js(`__caf.error({ detailedErrorCode: 314, error: { shakaErrorCode: 1002, shakaErrorData: ['http://relay.example/s7.ts', {}, 1] } })`);
	await sleep(900);
	const lost = await lastSent('error');
	expect(lost?.part === 'network' && lost.fetched === 'segment' && lost.status === 0 && !('codec' in lost) && typeof lost.detail === 'string', 'a lost segment is a network error');
	expect((await phase()) === 'gone', 'a lost stream waits with the picture held');

	// A subtitle file that does not load is not fatal
	await load(episode({ subtitles: [{ id: 1, url: at('missing.ass'), format: 'ass' }] }));
	await until(`document.getElementById('ui').dataset.phase === 'playing'`, 'the next load plays');
	await js('__caf.editTracks([1])');
	await sleep(1000);
	const subs = await lastSent('error');
	expect(subs?.part === 'subtitles' && subs.fetched === 'subtitle', 'a missing typeset file is a subtitles error');
	expect((await phase()) === 'playing', 'the episode plays on without typeset subtitles');

	// Sound with no picture
	await load(episode({ subtitles: [] }, { contentId: at('sound.mp4') }));
	await until(`document.getElementById('ui').dataset.phase === 'playing'`, 'the soundtrack plays');
	await until(`__caf.sent.some((m) => m.body.type === 'error' && m.body.part === 'video')`, 'sound without picture is a video error', 8000);
	const silent = await lastSent('error');
	expect(/sound without picture/.test(String(silent?.detail)), 'the video error says why');

	// Couldn't play
	await load(episode({}, { contentId: at('missing.mp4') }));
	await until(`document.getElementById('ui').dataset.phase === 'error'`, "Couldn't play");
	expect((await textOf('error-title')) === "Couldn't play episode 3", "Couldn't play episode 3");
	expect((await textOf('error-hint')) === 'Choose another source on the iPhone.', 'the error hint');
	const failed = await lastSent('error');
	expect(failed?.version === 1 && ['part', 'detail', 'position'].every((k) => k in failed), 'the error carries part and detail');
	await shot('11-error');
	await load(episode({ audio: { index: 0, codec: 'truehd' } }, { contentId: at('missing.mp4') }));
	await until(`document.getElementById('ui').dataset.phase === 'error'`, "Couldn't play again");
	const refusedAudio = await lastSent('error');
	expect(refusedAudio?.part === 'audio' && refusedAudio.codec === 'truehd', 'an audio codec the TV refuses is an audio error, never video');
	await say({ type: 'hello', version: 1, language: 'de', device: 'tablet' });
	await until(`document.documentElement.lang === 'de'`, 'German');
	await shot('11b-error-de');
	await say({ type: 'hello', version: 1, language: 'en', device: 'iphone' });
	await until(`document.documentElement.lang === 'en'`, 'English again');

	// Finished
	await load(episode({ subtitles: [] }));
	await until(`document.getElementById('ui').dataset.phase === 'playing'`, 'the last episode plays');
	await say({ type: 'up-next', version: 1, number: 4, title: 'Lights on the Water', still: at('still.png'), seconds: 12 });
	await sleep(400);
	const why = await js<string>(`JSON.stringify({ typeset: __receiver.typeset.active, tracks: [...document.getElementById('video').textTracks].map((t) => t.mode) })`);
	expect(!(await js<boolean>(`document.getElementById('upnext').classList.contains('over-subs')`)), `without subtitles Up next keeps its place on the board ${why}`);
	await shot('08b-up-next-no-subs');
	await say({ type: 'up-next', version: 1 });
	await js(`document.getElementById('video').currentTime = 38`);
	await until(`document.getElementById('ui').dataset.phase === 'finished'`, 'Finished', 8000);
	expect((await textOf('finished-line')) === 'Finished episode 3', 'Finished episode 3');
	await shot('12-finished');

	// Ten minutes with nothing to play
	await js(`__receiver.S.since = performance.now() - 600_001; __receiver.S.phase = 'idle'`);
	await sleep(1300);
	expect(await js<boolean>('__caf.context.stopped'), 'the receiver closes after ten idle minutes');
} catch (e) {
	failures.push(String(e));
	console.error(e);
} finally {
	expect(consoleErrors.length === 0, `no console errors${consoleErrors.length ? `: ${consoleErrors.join(' | ')}` : ''}`);
	cdp.close();
	browser.kill();
	server.stop(true);
	await sleep(300);
	rmSync(work, { recursive: true, force: true });
}

console.log(`\nshots in ${shots}`);
if (failures.length) {
	console.log(`${failures.length} failed`);
	process.exit(1);
}
console.log('all screens checked');
