// The Cast receiver's local test server: bun tools/cast-dev/serve.ts [--port 8787] [--no-watch]
//
// Serves two things and takes one, on the loopback interface only, for a tunnel to put in front
// of it (the dev receiver app's URL):
//   /           the receiver build, website/public/cast/receiver/ (index.html at /)
//   /media/     the test media tools/cast-dev/media.sh writes, with CORS and byte ranges
//   POST /log   a line from the dev receiver (status, audio setup, page errors), written to this
//               server's log as "receiver <line>"
// Any other path is 404. Directories are never listed. Nothing is cached, so a TV reloading the
// receiver gets the build that is on disk now.
//
// It rebuilds the receiver (website/scripts/cast-receiver.ts) at start and whenever a file under
// website/src/cast-receiver/ changes. The page's own files (index.html, receiver.css, i18n/) are
// served as they are, so a change to them needs no build: reload the receiver.
import { spawnSync } from 'node:child_process';
import { existsSync, statSync, watch } from 'node:fs';
import { join, normalize, resolve, sep } from 'node:path';

const repo = resolve(import.meta.dir, '../..');
const site = join(repo, 'website');
const receiverDir = join(site, 'public/cast/receiver');
const mediaDir = resolve(process.env.KAERI_CAST_MEDIA ?? join(repo, 'tools/cast-dev/media'));

const args = process.argv.slice(2);
const port = Number(args[args.indexOf('--port') + 1]) || Number(process.env.PORT) || 8787;
const watching = !args.includes('--no-watch');

const types: Record<string, string> = {
	html: 'text/html; charset=utf-8',
	js: 'text/javascript; charset=utf-8',
	mjs: 'text/javascript; charset=utf-8',
	css: 'text/css; charset=utf-8',
	json: 'application/json; charset=utf-8',
	wasm: 'application/wasm',
	woff2: 'font/woff2',
	woff: 'font/woff',
	ttf: 'font/ttf',
	otf: 'font/otf',
	txt: 'text/plain; charset=utf-8',
	m3u8: 'application/vnd.apple.mpegurl',
	mpd: 'application/dash+xml',
	ts: 'video/mp2t',
	m4s: 'video/iso.segment',
	mp4: 'video/mp4',
	m4a: 'audio/mp4',
	mkv: 'video/x-matroska',
	webm: 'video/webm',
	ass: 'text/x-ssa; charset=utf-8',
	ssa: 'text/x-ssa; charset=utf-8',
	srt: 'application/x-subrip; charset=utf-8',
	vtt: 'text/vtt; charset=utf-8',
	sup: 'application/octet-stream',
	sub: 'application/octet-stream',
	idx: 'text/plain; charset=utf-8',
};

const cors = {
	'access-control-allow-origin': '*',
	'access-control-allow-headers': 'range, content-type',
	'access-control-expose-headers': 'content-length, content-range, accept-ranges',
};

function log(line: string) {
	console.log(`${new Date().toISOString().slice(11, 19)} ${line}`);
}

/** The file `path` names under `root`, or null when it escapes `root` or isn't a file. */
function inside(root: string, path: string): string | null {
	let decoded: string;
	try {
		decoded = decodeURIComponent(path);
	} catch {
		return null;
	}
	if (decoded.includes('\0')) return null;
	const file = normalize(join(root, decoded));
	if (file !== root && !file.startsWith(root + sep)) return null;
	try {
		return statSync(file).isFile() ? file : null;
	} catch {
		return null;
	}
}

function serve(request: Request, file: string, media: boolean): Response {
	const extension = file.split('.').pop()!.toLowerCase();
	const headers: Record<string, string> = {
		'content-type': types[extension] ?? 'application/octet-stream',
		'cache-control': 'no-store',
		'accept-ranges': 'bytes',
		'x-content-type-options': 'nosniff',
		...(media ? cors : {}),
	};
	const body = Bun.file(file);
	const size = body.size;
	const range = request.headers.get('range')?.match(/^bytes=(\d*)-(\d*)$/);
	if (range && (range[1] || range[2])) {
		let start = range[1] ? Number(range[1]) : Math.max(0, size - Number(range[2]));
		let end = range[1] && range[2] ? Math.min(Number(range[2]), size - 1) : size - 1;
		if (start >= size || start > end) {
			return new Response(null, { status: 416, headers: { ...headers, 'content-range': `bytes */${size}` } });
		}
		headers['content-range'] = `bytes ${start}-${end}/${size}`;
		headers['content-length'] = String(end - start + 1);
		const slice = body.slice(start, end + 1);
		return new Response(request.method === 'HEAD' ? null : slice, { status: 206, headers });
	}
	headers['content-length'] = String(size);
	return new Response(request.method === 'HEAD' ? null : body, { status: 200, headers });
}

/** Logs a line the receiver posted: at most 2000 characters, control characters as spaces. */
async function receiverLine(request: Request): Promise<Response> {
	const text = (await request.text().catch(() => '')).slice(0, 2000).replace(/[\x00-\x1f\x7f]+/g, ' ').trim();
	if (text) log(`receiver ${text}`);
	return new Response(null, { status: 204, headers: { 'cache-control': 'no-store' } });
}

const server = Bun.serve({
	hostname: '127.0.0.1',
	port,
	fetch(request) {
		const url = new URL(request.url);
		const path = url.pathname;
		const media = path.startsWith('/media/');
		if (request.method === 'OPTIONS' && media) return new Response(null, { status: 204, headers: cors });
		// The dev receiver's status and error lines (debug.ts devLog), one log line each.
		if (path === '/log' && request.method === 'POST') return receiverLine(request);
		if (request.method !== 'GET' && request.method !== 'HEAD') return new Response(null, { status: 405 });
		const file = media
			? inside(mediaDir, path.slice('/media/'.length))
			: inside(receiverDir, path === '/' ? 'index.html' : path.slice(1));
		const status = file ? 200 : 404;
		if (!path.startsWith('/media/') || !/\.(ts|m4s)$/.test(path)) log(`${status} ${request.method} ${path}`);
		if (!file) return new Response('Not found\n', { status: 404, headers: { 'cache-control': 'no-store' } });
		return serve(request, file, media);
	},
});

function build(): boolean {
	const started = Date.now();
	const result = spawnSync('bun', ['scripts/cast-receiver.ts'], { cwd: site, stdio: ['ignore', 'pipe', 'pipe'] });
	if (result.status === 0) {
		log(`receiver built in ${Date.now() - started} ms`);
		return true;
	}
	log('receiver build failed:');
	process.stderr.write(result.stderr);
	return false;
}

if (!existsSync(join(site, 'node_modules'))) {
	console.error('website/node_modules is missing: run `bun install --frozen-lockfile` in website/');
	process.exit(1);
}
build();
if (watching) {
	let timer: ReturnType<typeof setTimeout> | undefined;
	watch(join(site, 'src/cast-receiver'), { recursive: true }, (_, name) => {
		if (name && /\.test\.ts$/.test(name)) return;
		clearTimeout(timer);
		timer = setTimeout(build, 200);
	});
	log('watching website/src/cast-receiver/');
}
log(`serving the receiver and ${mediaDir} on http://127.0.0.1:${server.port}`);
