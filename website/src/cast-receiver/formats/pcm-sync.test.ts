// The WebAssembly audio's worklet (pcm-worklet.ts) run against a simulated TV: a video clock read
// on the main thread with the jitter a TV's currentTime has, PCM sent the way the decoder worker
// sends it, and render quanta at 48 kHz. Steady playback must neither skip nor wait.

import { beforeAll, describe, expect, test } from 'bun:test';
import { ClockFilter } from './pcm';

const RATE = 48000;
const QUANTUM = 128;
const FRAME = 1024;

type Processor = {
	port: { onmessage: ((e: { data: unknown }) => void) | null; postMessage: (m: unknown) => void };
	process(inputs: Float32Array[][], outputs: Float32Array[][]): boolean;
};
let Ctor: new () => Processor;

const g = globalThis as unknown as Record<string, unknown>;

beforeAll(async () => {
	g.sampleRate = RATE;
	g.currentTime = 0;
	g.AudioWorkletProcessor = class {
		port = { onmessage: null, postMessage: () => {} };
	};
	g.registerProcessor = (_: string, c: new () => Processor) => (Ctor = c);
	await import('./pcm-worklet');
});

/** Counts after the start has lined up (the first second, or `settle`). */
interface Run {
	jumps: number;
	waits: number;
	starved: number;
	/** Output samples that break the tone (a skip, a repeat or silence), after the first second. */
	breaks: number;
	worst: number;
	/** Drift at the end, and the leftover latency the filter settled on. */
	drift: number;
	latency: number;
}

type Jitter = (t: number, rnd: () => number) => number;

/**
 * Plays `seconds` of a 440 Hz tone. `jitter(t)` is the error in the video time read at `t`,
 * `ctxJitter(t)` the error in the context time from currentTime (with `unreported` latency the
 * browser doesn't report on top), `tsJitter(t)` the error in the one from getOutputTimestamp();
 * with `filter`, the readings go through the main thread's ClockFilter first, as PcmAudio sends
 * them. `settle` is when counting starts (1 s by default).
 */
function play(
	seconds: number,
	jitter: Jitter,
	o: { ctxJitter?: Jitter; tsJitter?: Jitter; unreported?: number; filter?: boolean; seed?: number; settle?: number } = {},
): Run {
	const seed = o.seed ?? 7;
	const settle = o.settle ?? 1;
	const filter = o.filter ? new ClockFilter() : null;
	let s = seed;
	const rnd = () => ((s = (s * 16807) % 2147483647) / 2147483647);
	const p = new Ctor();
	const decoder = { onmessage: null as ((e: { data: unknown }) => void) | null, postMessage: (m: { type: string; time: number }) => (playhead = m.time) };
	let playhead = 0;
	let decodedTo = 0;
	p.port.onmessage!({ data: { type: 'decoder', port: decoder } });
	p.port.onmessage!({ data: { type: 'flush', gen: 1 } });
	// The worker: whole AAC frames, up to 4 s ahead of the worklet's position.
	const feed = () => {
		while (decodedTo < playhead + 4) {
			const data = new Float32Array(FRAME * 2);
			const first = Math.round(decodedTo * RATE);
			for (let i = 0; i < FRAME; i++) data[i * 2] = data[i * 2 + 1] = Math.sin((2 * Math.PI * 440 * (first + i)) / RATE);
			// Matroska times are whole milliseconds.
			decoder.onmessage!({ data: { type: 'pcm', gen: 1, pts: Math.round(decodedTo * 1000) / 1000, rate: RATE, data } });
			decodedTo = (first + FRAME) / RATE;
		}
	};
	feed();
	const left = new Float32Array(QUANTUM);
	const right = new Float32Array(QUANTUM);
	let last: number | null = null;
	let prev: number | null = null;
	let breaks = 0;
	// The counts when the start has lined up.
	let settled = { jumps: 0, waits: 0, starved: 0 };
	let nextClock = 0;
	const quanta = Math.round((seconds * RATE) / QUANTUM);
	// Output runs 50 ms ahead of what is heard; the main thread maps heard context time to video time.
	const latency = 0.05;
	for (let q = 0; q < quanta; q++) {
		const ctx = (q * QUANTUM) / RATE;
		g.currentTime = ctx;
		if (ctx >= nextClock) {
			const heard = Math.max(0, ctx - latency);
			const media = heard + jitter(heard, rnd);
			const at = heard + (o.unreported ?? 0) + (o.ctxJitter?.(heard, rnd) ?? 0);
			const ts = o.tsJitter ? heard + o.tsJitter(heard, rnd) : null;
			const k = filter ? filter.read(heard, media, at, 1, true, ts) : { media, ctx: at, settling: false };
			p.port.onmessage!({ data: { type: 'clock', media: k.media, ctx: k.ctx, rate: 1, playing: true, settling: k.settling } });
			nextClock += 0.1;
		}
		p.process([], [[left, right]]);
		feed();
		if (ctx <= settle) {
			const st = p as unknown as Record<string, number>;
			settled = { jumps: st.jumps, waits: st.waits, starved: st.starved };
		}
		const step = Math.cos((2 * Math.PI * 440) / RATE);
		for (let i = 0; i < QUANTUM; i++) {
			const v = left[i];
			// A continuous sine: x[n] = 2 cos(w) x[n-1] - x[n-2], within interpolation error.
			if (ctx > settle && last != null && prev != null && Math.abs(v - (2 * step * last - prev)) > 0.02) breaks++;
			prev = last;
			last = v;
		}
	}
	const st = p as unknown as Record<string, number>;
	return { jumps: st.jumps - settled.jumps, waits: st.waits - settled.waits, starved: st.starved - settled.starved, breaks, worst: st.worst, drift: st.drift, latency: filter?.spread.latency ?? 0 };
}

describe('WebAssembly audio against the video clock', () => {
	test('a steady clock plays straight through', () => {
		const r = play(20, () => 0);
		console.info('steady', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	test('a video time read in whole frames (24 fps) and late by up to 30 ms plays straight through', () => {
		const r = play(30, (t, rnd) => Math.floor(t * 24) / 24 - t - rnd() * 0.03);
		console.info('frame steps', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	test('a video time with 60 ms of random error plays straight through', () => {
		const r = play(30, (_, rnd) => (rnd() - 0.5) * 0.06);
		console.info('random 60 ms', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	test('a clock that swings 80 ms either way, as the failing TV stream showed, plays straight through', () => {
		const r = play(30, (t, rnd) => (rnd() - 0.5) * 0.16 + 0.04 * Math.sin(t * 3));
		console.info('swing 80 ms', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	test('a context time that moves in 85 ms steps plays straight through', () => {
		const r = play(30, (t) => t - Math.floor(t / 0.085) * 0.085);
		console.info('85 ms steps', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	// The filter needs its window (40 readings) before the estimate stops moving; for the first seconds the
	// worklet lines up in a few short corrections.
	test('the TV: video time in 300 ms steps and a context time off by up to 100 ms, through the main thread filter', () => {
		const r = play(40, (t) => -(t % 0.3), { ctxJitter: (_, rnd) => (rnd() - 0.5) * 0.2, filter: true, settle: 6 });
		console.info('tv-like, filtered', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	test('the same clocks without the filter chop, as the TV did', () => {
		const r = play(40, (t) => -(t % 0.3), { ctxJitter: (_, rnd) => (rnd() - 0.5) * 0.2, settle: 6 });
		console.info('tv-like, unfiltered', r);
		expect(r.breaks).toBeGreaterThan(0);
	});

	// What the Chromecast TV showed on 17 Nov: the video offset steady, the output timestamp's
	// context time off by up to 350 ms either way. currentTime moves in steps of the output buffer.
	test('the TV from 0.5 s: currentTime in 85 ms steps and an output timestamp off by 200 ms either way', () => {
		const r = play(40, () => 0, { ctxJitter: (t) => -(t % 0.085), tsJitter: (_, rnd) => (rnd() - 0.5) * 0.4, filter: true, settle: 0.5 });
		console.info('tv from the start', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	// The TV on 17 Nov: the context offset spread 311 ms in the first seconds (the context starting),
	// then 70 to 90 ms. Readings from a context that isn't running are left out on the main thread;
	// one that runs but reads 300 ms off at first costs one skip when it settles, and no more.
	test('a context time that settles 300 ms later in the first 0.4 s costs one skip at most', () => {
		const r = play(30, () => 0, { ctxJitter: (t) => (t < 0.4 ? 0.3 : 0) - (t % 0.085), tsJitter: (_, rnd) => (rnd() - 0.5) * 2, filter: true, settle: 0.5 });
		console.info('context starting', r);
		expect(r.jumps).toBeLessThanOrEqual(1);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBeLessThanOrEqual(2);
	});

	test('latency the browser does not report (60 ms) is taken up by resampling, without a gap', () => {
		const r = play(60, () => 0, { unreported: 0.06, ctxJitter: (t) => -(t % 0.085), tsJitter: (_, rnd) => (rnd() - 0.5) * 0.04, filter: true, settle: 0.5 });
		console.info('unreported latency', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
		// currentTime in 85 ms steps reads 42 ms early on average, so 60 ms less that is left over.
		expect(Math.abs(r.latency - (0.06 - 0.085 / 2))).toBeLessThan(0.025);
		expect(Math.abs(r.drift)).toBeLessThan(0.015);
	});

	// On the Chromecast TV the output timestamp spread over a second: it must not move the clock.
	test('an output timestamp that strays by a second calibrates nothing', () => {
		const r = play(40, () => 0, { ctxJitter: (t) => -(t % 0.085), tsJitter: (_, rnd) => (rnd() - 0.3) * 1.2, filter: true, settle: 0.5 });
		console.info('wild output timestamp', r);
		expect(r.latency).toBe(0);
		expect(r.jumps + r.waits + r.breaks).toBe(0);
	});

	test('a currentTime that strays 60 ms too makes at most one correction after 0.5 s', () => {
		const r = play(40, () => 0, { ctxJitter: (_, rnd) => (rnd() - 0.5) * 0.12, tsJitter: (_, rnd) => (rnd() - 0.5) * 0.4, filter: true, settle: 0.5 });
		console.info('noisy currentTime', r);
		expect(r.jumps + Math.min(r.waits, 1)).toBeLessThanOrEqual(1);
		// One gap: its start and its end, each two samples off the tone.
		expect(r.breaks).toBeLessThanOrEqual(4);
	});

	test('a video clock that runs 0.2 % slow is followed without chops', () => {
		const r = play(40, (t) => -t * 0.002, { ctxJitter: (_, rnd) => (rnd() - 0.5) * 0.2, filter: true, settle: 6 });
		console.info('slow video, filtered', r);
		expect(r.jumps).toBe(0);
		expect(r.waits).toBe(0);
		expect(r.breaks).toBe(0);
	});

	test('a seek-sized jump goes through the main thread filter at once', () => {
		const r = play(20, (t) => (t > 10 ? 0.6 : 0), { filter: true });
		expect(r.jumps).toBe(1);
	});

	test('a real jump in the video (a stall caught up) is followed at once', () => {
		const r = play(20, (t) => (t > 10 ? 0.5 : 0));
		console.info('jump', r);
		expect(r.jumps).toBe(1);
	});
});
