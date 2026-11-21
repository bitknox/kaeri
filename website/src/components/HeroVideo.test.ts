import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

// HeroVideo's script, run against a stand-in page: one clip, a clock the test moves a second at a
// time (the stall watchdog's interval), and a video element that reports what a browser reports
// while the clip loads, plays, waits for data and stalls.
const component = readFileSync(new URL('./HeroVideo.astro', import.meta.url), 'utf8');
const script = new Bun.Transpiler({ loader: 'ts' }).transformSync(component.match(/<script>([\s\S]*?)<\/script>/)![1]);

const MEDIA = {
	HAVE_NOTHING: 0,
	HAVE_METADATA: 1,
	HAVE_CURRENT_DATA: 2,
	HAVE_FUTURE_DATA: 3,
	HAVE_ENOUGH_DATA: 4,
	NETWORK_EMPTY: 0,
	NETWORK_IDLE: 1,
	NETWORK_LOADING: 2,
	NETWORK_NO_SOURCE: 3,
};
const WEBM = 'https://kaeri.example/features/mac-home.webm';
const MP4 = 'https://kaeri.example/features/mac-home.mp4';
/** The clip's length in seconds. */
const LENGTH = 10.284;

function page() {
	let now = 0;
	let tick = () => {};
	let seen: (entries: { target: unknown; isIntersecting: boolean }[]) => void = () => {};
	const listeners = new Map<string, (() => void)[]>();
	const emit = (type: string) => listeners.get(type)?.forEach((listener) => listener());
	// preload="none" until the script asks for the clip, so the browser holds nothing and fetches nothing.
	const video = {
		...MEDIA,
		paused: true,
		preload: 'none',
		readyState: MEDIA.HAVE_NOTHING,
		networkState: MEDIA.NETWORK_IDLE,
		seeking: false,
		currentTime: 0,
		duration: Number.NaN,
		currentSrc: WEBM,
		src: '',
		addEventListener: (type: string, listener: () => void) => listeners.set(type, [...(listeners.get(type) ?? []), listener]),
		play() {
			video.paused = false;
			emit('play');
			return Promise.resolve();
		},
		pause() {
			video.paused = true;
			emit('pause');
		},
		querySelector: (selector: string) => (selector.includes('video/mp4') ? { src: MP4 } : null),
	};
	const button = { hidden: true, dataset: {}, addEventListener() {}, setAttribute() {} };
	const clip = { classList: { add() {}, remove() {} }, querySelector: (selector: string) => (selector === 'video' ? video : button) };
	const window = {
		matchMedia: () => ({ matches: false, addEventListener() {} }),
		addEventListener() {},
		setInterval: (callback: () => void) => {
			tick = callback;
		},
	};
	const document = { readyState: 'complete', hidden: false, querySelectorAll: () => [clip], addEventListener() {} };
	class IntersectionObserver {
		constructor(callback: typeof seen) {
			seen = callback;
		}
		observe() {}
	}
	new Function('window', 'document', 'IntersectionObserver', 'performance', 'HTMLMediaElement', script)(
		window,
		document,
		IntersectionObserver,
		{ now: () => now },
		MEDIA,
	);

	return {
		video,
		/** The clip scrolls into view, which plays it. */
		show: () => seen([{ target: clip, isIntersecting: true }]),
		/** Seconds pass with a watchdog tick each; a clip that plays moves a second and says so. */
		pass(seconds: number, { playing = false } = {}) {
			for (let i = 0; i < seconds; i++) {
				now += 1000;
				if (playing) {
					video.currentTime = Math.min(video.currentTime + 1, LENGTH);
					emit('timeupdate');
				}
				tick();
			}
		},
	};
}

/** A clip on screen that has played its first three seconds of the WebM. */
function playing() {
	const clip = page();
	clip.show();
	Object.assign(clip.video, { networkState: MEDIA.NETWORK_LOADING, readyState: MEDIA.HAVE_ENOUGH_DATA, duration: LENGTH });
	clip.pass(3, { playing: true });
	return clip;
}

describe('the hero clip on a slow network', () => {
	test('keeps its WebM while the first frame is on the way', () => {
		const clip = page();
		clip.show();
		expect(clip.video.paused).toBe(false);
		expect(clip.video.preload).toBe('auto');

		// Fetching, with nothing back yet.
		clip.video.networkState = MEDIA.NETWORK_LOADING;
		clip.pass(3);
		// The metadata is in, the first frame not yet.
		Object.assign(clip.video, { readyState: MEDIA.HAVE_METADATA, duration: LENGTH });
		clip.pass(3);
		expect(clip.video.src).toBe('');

		Object.assign(clip.video, { readyState: MEDIA.HAVE_ENOUGH_DATA });
		clip.pass(5, { playing: true });
		expect(clip.video.src).toBe('');
	});

	test('keeps its WebM while it waits for more data partway through', () => {
		const clip = playing();
		Object.assign(clip.video, { readyState: MEDIA.HAVE_CURRENT_DATA });
		clip.pass(5);
		expect(clip.video.src).toBe('');

		Object.assign(clip.video, { readyState: MEDIA.HAVE_ENOUGH_DATA });
		clip.pass(3, { playing: true });
		expect(clip.video.src).toBe('');
	});
});

describe('the hero clip when the WebM stalls', () => {
	test('plays its MP4 once it stops at its end instead of looping', () => {
		const clip = playing();
		// The whole file is in, and the clip stands on its last frame.
		Object.assign(clip.video, { networkState: MEDIA.NETWORK_IDLE, readyState: MEDIA.HAVE_CURRENT_DATA, currentTime: LENGTH });
		clip.pass(1);
		expect(clip.video.src).toBe('');
		clip.pass(1);
		expect(clip.video.src).toBe(MP4);
	});

	test('plays its MP4 when its loop never gets going again', () => {
		// The seek back to the start never ends.
		const seeking = playing();
		Object.assign(seeking.video, { networkState: MEDIA.NETWORK_IDLE, readyState: MEDIA.HAVE_METADATA, seeking: true, currentTime: 0 });
		seeking.pass(2);
		expect(seeking.video.src).toBe(MP4);

		// The seek ends, with the whole file in, and no frame follows.
		const stuck = playing();
		Object.assign(stuck.video, { networkState: MEDIA.NETWORK_IDLE, readyState: MEDIA.HAVE_CURRENT_DATA, currentTime: 0 });
		stuck.pass(2);
		expect(stuck.video.src).toBe(MP4);
	});

	test('plays its MP4 when it stops partway with frames ahead', () => {
		const clip = playing();
		clip.pass(2);
		expect(clip.video.src).toBe(MP4);
	});
});

describe('the pause button', () => {
	// An absolutely placed box with a fixed height keeps its top and drops its bottom when both are
	// set (CSS 2.1, 10.6.4), so a bottom that comes without top: auto never moves it.
	test('is placed from the top or the bottom of the clip, never both', () => {
		const css = component.match(/<style[^>]*>([\s\S]*?)<\/style>/)![1];
		const media = /@media([^{]*)\{((?:[^{}]*\{[^{}]*\})*)\s*\}/g;
		const button = (block: string): Record<string, string> =>
			Object.fromEntries(
				[...block.matchAll(/\.hero-video-toggle\s*\{([^}]*)\}/g)].flatMap(([, rule]) =>
					rule
						.split(';')
						.filter((declaration) => declaration.includes(':'))
						.map((declaration) => [declaration.slice(0, declaration.indexOf(':')).trim(), declaration.slice(declaration.indexOf(':') + 1).trim()]),
				),
			);
		const base = button(css.replace(media, ''));
		expect(base.position).toBe('absolute');

		const contexts: [string, Record<string, string>][] = [['everywhere', base]];
		for (const [, query, body] of css.matchAll(media)) contexts.push([`@media${query.trimEnd()}`, { ...base, ...button(body) }]);
		for (const [where, placed] of contexts) {
			if (!placed.height || placed.height === 'auto') continue;
			const pinned = ['top', 'bottom'].filter((side) => placed[side] && placed[side] !== 'auto');
			expect(pinned, where).toHaveLength(1);
		}
	});
});
