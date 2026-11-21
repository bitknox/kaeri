// Drives the built site in headless Chrome and checks what only a browser shows: how the pages
// answer clicks, keys and focus, and how their colours come out once every rule has applied.
//
//   bun run build && bun scripts/site-check.ts [--only <check>,<check>] [--shots <dir>]
//
// The site is served from dist/ as the host serves it, with 404.html for a page that isn't there,
// and Chrome reaches nothing else. Every check prints an ok or FAIL line per expectation, and the
// run exits 1 when any failed. Needs Google Chrome (CHROME names another binary).
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dist = join(here, '..', 'dist');
const option = (name: string) => {
	const at = process.argv.indexOf(name);
	return at > 0 ? process.argv[at + 1] : undefined;
};
const only = option('--only')?.split(',');
const shots = option('--shots');
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';

if (!existsSync(join(dist, '404.html'))) throw new Error('build the site first: bun run build');
if (shots) mkdirSync(shots, { recursive: true });

// ---------- The site, as the host serves it ----------

const isFile = (path: string) => existsSync(path) && statSync(path).isFile();
const server = Bun.serve({
	port: 0,
	hostname: '127.0.0.1',
	fetch(req) {
		const path = decodeURIComponent(new URL(req.url).pathname);
		const at = normalize(join(dist, path));
		if (at !== dist && !at.startsWith(dist + sep)) return new Response('not found', { status: 404 });
		if (isFile(at)) return new Response(Bun.file(at));
		if (isFile(join(at, 'index.html'))) {
			return path.endsWith('/') ? new Response(Bun.file(join(at, 'index.html'))) : new Response(null, { status: 308, headers: { Location: `${path}/` } });
		}
		return new Response(Bun.file(join(dist, '404.html')), { status: 404 });
	},
});
const origin = `http://127.0.0.1:${server.port}`;

// ---------- Chrome over the DevTools protocol ----------

const work = mkdtempSync(join(tmpdir(), 'site-check-'));
const profile = join(work, 'chrome');
const browser = spawn(
	chrome,
	[
		'--headless=new',
		'--remote-debugging-port=0',
		`--user-data-dir=${profile}`,
		'--no-first-run',
		'--no-default-browser-check',
		// No keychain, nothing past the local server, no looking for devices on the network.
		'--use-mock-keychain',
		'--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1',
		'--disable-background-networking',
		'--disable-component-update',
		'--disable-sync',
		'--disable-extensions',
		'--disable-features=MediaRouter,DialMediaRouteProvider,Translate,OptimizationHints',
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

const cdp = await Cdp.open(await devtoolsUrl());
const { targetId } = await cdp.send<{ targetId: string }>('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await cdp.send<{ sessionId: string }>('Target.attachToTarget', { targetId, flatten: true });
const page = <T = Record<string, unknown>>(method: string, params: Record<string, unknown> = {}) => cdp.send<T>(method, params, sessionId);
const pageErrors: string[] = [];
cdp.on('Runtime.exceptionThrown', (p) => pageErrors.push((p.exceptionDetails as { exception?: { description?: string }; text: string }).exception?.description ?? JSON.stringify(p)));
let loaded = () => {};
cdp.on('Page.loadEventFired', () => loaded());
await page('Page.bringToFront');
await page('Emulation.setFocusEmulationEnabled', { enabled: true });
await page('Runtime.enable');
await page('Page.enable');

async function js<T = unknown>(expression: string): Promise<T> {
	const r = await page<{ result: { value?: T }; exceptionDetails?: { text: string; exception?: { description?: string } } }>('Runtime.evaluate', {
		expression,
		awaitPromise: true,
		returnByValue: true,
	});
	if (r.exceptionDetails) throw new Error(`${expression}: ${r.exceptionDetails.exception?.description ?? r.exceptionDetails.text}`);
	return r.result.value as T;
}

let current = '';
const failed = new Set<string>();
function expect(ok: boolean, what: string) {
	console.log(`${ok ? 'ok  ' : 'FAIL'} ${current}: ${what}`);
	if (!ok) failed.add(current);
}
async function until(expression: string, what: string, timeout = 10_000) {
	const end = Date.now() + timeout;
	while (Date.now() < end) {
		if (await js<boolean>(expression).catch(() => false)) return true;
		await sleep(100);
	}
	expect(false, `${what} (waited for ${expression})`);
	return false;
}

interface View {
	width: number;
	height: number;
	/** A phone: the mobile viewport and touch. */
	mobile?: boolean;
	scale?: number;
	scheme?: 'light' | 'dark';
}
const wide: View = { width: 1280, height: 800 };
const phone: View = { width: 390, height: 844, mobile: true, scale: 2 };

/** Opens a page of the site at a size and in a theme, with motion reduced, once it has loaded. */
async function open(path: string, view: View) {
	await page('Emulation.setDeviceMetricsOverride', { width: view.width, height: view.height, deviceScaleFactor: view.scale ?? 1, mobile: !!view.mobile });
	await page('Emulation.setTouchEmulationEnabled', { enabled: !!view.mobile });
	await page('Emulation.setEmulatedMedia', {
		features: [
			{ name: 'prefers-color-scheme', value: view.scheme ?? 'light' },
			{ name: 'prefers-reduced-motion', value: 'reduce' },
		],
	});
	let timer: ReturnType<typeof setTimeout> | undefined;
	const done = new Promise<void>((ok, fail) => {
		loaded = ok;
		timer = setTimeout(() => fail(new Error(`${path} did not load in 15 s`)), 15_000);
	});
	await page('Page.navigate', { url: `${origin}${path}` });
	await done.finally(() => clearTimeout(timer));
}

const keys = {
	Enter: { code: 'Enter', keyCode: 13, text: '\r' },
	Escape: { code: 'Escape', keyCode: 27 },
	Tab: { code: 'Tab', keyCode: 9 },
} as const;
/** A key press, as Chrome gets it from a keyboard. */
async function press(key: keyof typeof keys, { shift = false } = {}) {
	const k: { code: string; keyCode: number; text?: string } = keys[key];
	const modifiers = shift ? 8 : 0;
	await page('Input.dispatchKeyEvent', { type: k.text ? 'keyDown' : 'rawKeyDown', key, code: k.code, windowsVirtualKeyCode: k.keyCode, modifiers, text: k.text });
	await page('Input.dispatchKeyEvent', { type: 'keyUp', key, code: k.code, windowsVirtualKeyCode: k.keyCode, modifiers });
}
/** A click with the mouse at a point of the window. */
async function click(x: number, y: number) {
	await page('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
	await page('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1 });
	await page('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', buttons: 0, clickCount: 1 });
}
async function clickOn(selector: string) {
	const [x, y] = await js<[number, number]>(
		`(() => { const e = document.querySelector(${JSON.stringify(selector)}); e.scrollIntoView({ block: 'center' }); const r = e.getBoundingClientRect(); return [r.x + r.width / 2, r.y + r.height / 2]; })()`,
	);
	await click(x, y);
}
async function shot(name: string) {
	if (!shots) return;
	await sleep(400);
	const { data } = await page<{ data: string }>('Page.captureScreenshot', { format: 'png' });
	writeFileSync(join(shots, `${name}.png`), Buffer.from(data, 'base64'));
}

// ---------- The checks ----------

const checks: { name: string; title: string; run: () => Promise<void> }[] = [];
const check = (name: string, title: string, run: () => Promise<void>) => checks.push({ name, title, run });

check('404-search', "The 404 page's Search button opens the search dialog, and it stays open", async () => {
	await open('/no-such-page/', wide);
	await until(
		`!!document.querySelector('[data-open-search]') && !!document.querySelector('site-search button[data-open-modal]:not([disabled])') && !!document.querySelector('site-search dialog input')`,
		'the 404 page and its search are ready',
	);
	const dialogOpen = () => js<boolean>(`document.querySelector('site-search dialog').open`);
	const focusInDialog = () => js<boolean>(`document.querySelector('site-search dialog').contains(document.activeElement)`);

	await clickOn('[data-open-search]');
	await sleep(300);
	expect(await dialogOpen(), 'a click on Search opens the search dialog, and it stays open');
	expect(await focusInDialog(), 'the search field has the focus');
	await shot('404-search');
	// Starlight's own rule still holds: a click outside the dialog, on its backdrop, shuts it.
	await click(16, wide.height - 16);
	await sleep(200);
	expect(!(await dialogOpen()), 'a click outside the dialog shuts it');

	await js(`document.querySelector('[data-open-search]').focus()`);
	await press('Enter');
	await sleep(300);
	expect(await dialogOpen(), 'Enter on Search opens the search dialog, and it stays open');
	await press('Escape');
	await sleep(200);
	expect(!(await dialogOpen()), 'Escape shuts it');
});

// The text in every element under `.theatre` that has text of its own: how it reads against what
// is painted behind it, with the backgrounds of the element and its ancestors laid over each other
// as the page lays them (on a canvas, so any colour syntax works).
const theatreText = String.raw`(() => {
	const ctx = Object.assign(document.createElement('canvas'), { width: 1, height: 1 }).getContext('2d', { willReadFrequently: true });
	const paint = (colors) => {
		ctx.clearRect(0, 0, 1, 1);
		for (const c of ['#fff', ...colors]) {
			ctx.fillStyle = c;
			ctx.fillRect(0, 0, 1, 1);
		}
		return [...ctx.getImageData(0, 0, 1, 1).data.slice(0, 3)];
	};
	const lin = (v) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
	const lum = ([r, g, b]) => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
	const hex = (c) => '#' + c.map((v) => v.toString(16).padStart(2, '0')).join('');
	const out = [];
	for (const el of document.querySelectorAll('.theatre *')) {
		if (![...el.childNodes].some((n) => n.nodeType === Node.TEXT_NODE && n.textContent.trim())) continue;
		const style = getComputedStyle(el);
		const box = el.getBoundingClientRect();
		if (style.visibility !== 'visible' || !box.width || !box.height) continue;
		const under = [];
		for (let e = el; e; e = e.parentElement) under.unshift(getComputedStyle(e).backgroundColor);
		const back = paint(under);
		const fore = paint([...under, style.color]);
		const [hi, lo] = [lum(fore), lum(back)].sort((a, b) => b - a);
		const size = parseFloat(style.fontSize);
		out.push({
			text: el.textContent.trim().replace(/\s+/g, ' ').slice(0, 40),
			pressed: !!el.closest('[aria-pressed="true"]'),
			ratio: (hi + 0.05) / (lo + 0.05),
			need: size >= 24 || (size >= 18.66 && Number(style.fontWeight) >= 700) ? 3 : 4.5,
			colours: hex(fore) + ' on ' + hex(back),
		});
	}
	return out;
})()`;

check('theatre-contrast', "Text in the player's dark room reads at 4.5:1 or more (3:1 when large), in either theme", async () => {
	for (const scheme of ['light', 'dark'] as const) {
		for (const path of ['/', '/features/']) {
			await open(path, { ...wide, scheme });
			expect((await js<string>('document.documentElement.dataset.theme')) === scheme, `${path} opens in the ${scheme} theme`);
			const texts = await js<{ text: string; pressed: boolean; ratio: number; need: number; colours: string }[]>(theatreText);
			const low = texts.filter((t) => t.ratio < t.need);
			const lowest = Math.min(...texts.map((t) => t.ratio));
			expect(texts.length > 0 && !low.length, `${path} in ${scheme}: ${texts.length} texts in the dark room, the lowest at ${lowest.toFixed(2)}:1`);
			for (const t of low) console.log(`     "${t.text}" reads at ${t.ratio.toFixed(2)}:1 (${t.colours}), under ${t.need}:1`);
			if (path === '/features/') expect(texts.some((t) => t.pressed), `${path} in ${scheme}: the pressed look button is among them`);
		}
	}
	await open('/features/', { ...wide, scale: 2 });
	await js(`document.querySelector('.compare-looks').scrollIntoView({ block: 'center' })`);
	await shot('theatre-contrast');
});

check('phone-menu', "On a phone the menu's links come right after the Menu button, and focus moving on past them shuts the menu", async () => {
	// Where the focus is, in the bar's terms.
	const focus = () =>
		js<string>(String.raw`(() => {
			const a = document.activeElement;
			const link = [...document.querySelectorAll('#bar-nav a')].indexOf(a);
			if (link >= 0) return 'link ' + (link + 1);
			if (a?.matches('[data-bar-menu]')) return 'Menu';
			if (a?.matches('site-search button')) return 'Search';
			if (a?.matches('[data-theme-toggle]')) return 'theme';
			return (a?.tagName.toLowerCase() ?? 'nothing') + (a?.closest('main') ? ' in main' : '');
		})()`);
	const menuOpen = () => js<boolean>(`document.querySelector('[data-bar-menu]').getAttribute('aria-expanded') === 'true' && document.getElementById('bar-nav').classList.contains('open')`);

	await open('/', phone);
	const count = await js<number>(`document.querySelectorAll('#bar-nav a').length`);
	const links = Array.from({ length: count }, (_, i) => `link ${i + 1}`);
	expect(
		await js<boolean>(String.raw`(() => {
			const menu = document.querySelector('[data-bar-menu]');
			const left = menu.getBoundingClientRect().left;
			return [...menu.parentElement.children].every((e) => e === menu || getComputedStyle(e).position === 'absolute' || e.getBoundingClientRect().right <= left + 1);
		})()`),
		'the Menu button is the last thing in the bar',
	);

	await js(`document.querySelector('[data-bar-menu]').focus()`);
	await press('Enter');
	expect(await menuOpen(), 'Enter on Menu opens the menu');
	const path: string[] = [];
	for (let i = 0; i < count; i++) {
		await press('Tab');
		path.push(await focus());
		if (i === 0) await shot('phone-menu');
	}
	expect(path.join() === links.join(), `Tab goes from Menu through the ${count} links in order (went to ${path.join(', ')})`);
	expect(await menuOpen(), 'the menu stays open while the focus is in it');
	await press('Tab');
	const after = await focus();
	expect(after === 'Search' && !(await menuOpen()), `Tab past the last link goes on to Search and shuts the menu (went to ${after})`);

	await js(`document.querySelector('[data-bar-menu]').focus()`);
	await press('Enter');
	await press('Tab');
	await press('Tab', { shift: true });
	const back = await focus();
	expect(back === 'Menu' && (await menuOpen()), `Shift-Tab from the first link goes back to Menu, and the menu stays open (went to ${back})`);
	await press('Escape');
	expect(!(await menuOpen()) && (await focus()) === 'Menu', 'Escape shuts the menu and leaves the focus on Menu');

	// A wide window has no menu: Tab goes through the bar in the order it shows.
	await open('/', wide);
	await js(`document.querySelector('.site-brand').focus()`);
	const bar: string[] = [];
	for (let i = 0; i < count + 2; i++) {
		await press('Tab');
		bar.push(await focus());
	}
	expect(bar.join() === [...links, 'Search', 'theme'].join(), `on a wide window Tab goes from the name through the links, Search and the theme switch (went to ${bar.join(', ')})`);
});

// ---------- Run ----------

const unknown = only?.filter((name) => !checks.some((c) => c.name === name)) ?? [];
if (unknown.length) throw new Error(`no check named ${unknown.join(', ')}; the checks are ${checks.map((c) => c.name).join(', ')}`);
const run = checks.filter((c) => !only || only.includes(c.name));
try {
	for (const c of run) {
		current = c.name;
		console.log(`\n${c.name}: ${c.title}`);
		try {
			await c.run();
		} catch (e) {
			expect(false, `stopped: ${e instanceof Error ? e.message : String(e)}`);
		}
	}
} finally {
	cdp.close();
	browser.kill();
	server.stop(true);
	await sleep(300);
	rmSync(work, { recursive: true, force: true });
}

if (pageErrors.length) console.log(`\nerrors the pages threw (not checked): ${pageErrors.join(' | ')}`);
if (shots) console.log(`\nshots in ${shots}`);
if (failed.size) {
	console.log(`\nsite-check: ${failed.size} of ${run.length} checks failed: ${[...failed].join(', ')}`);
	process.exit(1);
}
console.log(`\nsite-check: all ${run.length} checks passed`);
