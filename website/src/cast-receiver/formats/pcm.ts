// Audio the TV's MediaSource doesn't take, decoded in WebAssembly and played through an
// AudioWorklet on the video element's clock. The decoders load only when a stream needs one.

import type { Sample, Track } from './types';

/** The decoder family file and FFmpeg decoder name for a codec. Null when there is none. */
export function decoderFor(t: Track): { family: string; decoder: string } | null {
	const c = t.codec;
	if (c === 'ac3' || c === 'eac3') return { family: 'ac3', decoder: c };
	if (c === 'dts') return { family: 'dts', decoder: 'dca' };
	if (c === 'truehd' || c === 'mlp') return { family: 'truehd', decoder: c };
	if (c === 'flac') return { family: 'flac', decoder: 'flac' };
	const more: Record<string, string> = {
		aac: 'aac',
		aac_latm: 'aac_latm',
		mp3: 'mp3float',
		mp2: 'mp2float',
		mp1: 'mp1float',
		vorbis: 'vorbis',
		opus: 'opus',
		alac: 'alac',
	};
	if (more[c]) return { family: 'more', decoder: more[c] };
	if (/^pcm_(bluray|dvd|s16be|s16le|s24be|s24le|s32le|f32le|f64le|u8)$/.test(c)) return { family: 'more', decoder: c };
	return null;
}

/**
 * The video's clock and the audio context's clock as offsets from performance.now(), each the
 * mean of the middle half of the last 40 readings (about 3 s), with the video and context clocks
 * read on the main thread.
 *
 * The context time steered by is AudioContext.currentTime less the reported output latency. It
 * moves in steps of the output buffer but never strays further. getOutputTimestamp() would also
 * count the latency the browser doesn't report, but on the Chromecast TV it strays by up to 700 ms,
 * so it only calibrates, and only when the middle half of the last 200 differences between the two
 * lies within TRUSTED: their mean is the latency left over, and the clock moves towards it by at
 * most RAMP (3 ms a second), which the worklet follows by resampling without a gap or a skip.
 *
 * A reading far off the median (a seek, a stall caught up) starts the window again. Until a window
 * holds SETTLED readings, `settling` tells the worklet to leave drift under 120 ms alone, so the
 * start never steers by an estimate made from a handful of readings.
 */
export class ClockFilter {
	private v: number[] = [];
	private c: number[] = [];
	/** currentTime-based context time minus the output timestamp's, for the latency left over. */
	private d: number[] = [];
	private rate = 1;
	private playing = false;
	/** The extra latency applied so far (seconds), and when it was last moved. */
	private lat = 0;
	private latAt = 0;
	/** Spreads (max - min) of the video and context offsets and of the output timestamp's, the highest video offset above the estimate, and the extra latency applied. */
	spread = { video: 0, videoHigh: 0, ctx: 0, ts: 0, latency: 0 };

	constructor(private readonly keep = 40) {}

	/**
	 * One reading at performance time `now` (seconds): the video time, the context time from
	 * currentTime less the reported latency, and the one from getOutputTimestamp() when there is one.
	 */
	read(now: number, media: number, ctx: number, rate: number, playing: boolean, ts: number | null = null): { media: number; ctx: number; settling: boolean } {
		if (!playing || rate !== this.rate || !this.playing) this.v = [];
		this.rate = rate;
		this.playing = playing;
		const vo = media - now * rate;
		const co = ctx - now;
		if (this.v.length >= 3 && Math.abs(vo - median(this.v)) > JUMP) this.v = [];
		if (this.c.length >= 3 && Math.abs(co - median(this.c)) > CTX_JUMP) this.c = [];
		// A new window (start, seek, rate change, a real jump) starts both lists together.
		if (!this.v.length || !this.c.length) {
			this.v = [];
			this.c = [];
		}
		push(this.v, vo, this.keep);
		push(this.c, co, this.keep);
		if (ts != null && Number.isFinite(ts)) push(this.d, ctx - ts, CALIBRATE);
		const settling = this.v.length < SETTLED;
		// The leftover latency moves only in steady play, and slowly.
		if (!settling && this.d.length >= SETTLED) {
			// An output timestamp that strays more than this tells nothing: ease back to none.
			const want = quartiles(this.d) < TRUSTED ? Math.max(-MAX_LATENCY, Math.min(MAX_LATENCY, middle(this.d))) : 0;
			const step = RAMP * Math.max(0, Math.min(1, now - this.latAt));
			this.lat += Math.max(-step, Math.min(step, want - this.lat));
		}
		this.latAt = now;
		const mv = middle(this.v);
		const mc = middle(this.c);
		this.spread = { video: spread(this.v), videoHigh: Math.max(...this.v) - mv, ctx: spread(this.c), ts: spread(this.d), latency: this.lat };
		return { media: mv + now * rate, ctx: mc + now - this.lat, settling };
	}
}

/** The FLAC form decoded sound goes to the TV's MediaSource in, when the TV takes it. */
export const FLAC_MIME = 'audio/mp4; codecs="flac"';

/** Whether this TV's MediaSource takes FLAC in MP4. */
export function flacSupported(): boolean {
	// The checks set this to play decoded sound through the worklet on a browser that takes FLAC.
	if ((globalThis as { __kaeriWorklet?: boolean }).__kaeriWorklet) return false;
	try {
		return !!globalThis.MediaSource?.isTypeSupported?.(FLAC_MIME);
	} catch {
		return false;
	}
}

/** A decoder worker made ahead of use, with some decoder families fetched and compiled. */
interface SpareWorker {
	base: string;
	worker: Worker;
	made: number;
}

/** An audio context (running) with the worklet loaded, made ahead of use. */
interface SpareAudio {
	base: string;
	ctx: AudioContext;
	worklet: Promise<void>;
	made: number;
}

let spareWorker: SpareWorker | null = null;
let spareAudio: SpareAudio | null = null;

function makeWorker(base: string, families: string[]): SpareWorker {
	const worker = new Worker(new URL('pcm/worker.js', base).href, { type: 'module' });
	for (const family of families) worker.postMessage({ type: 'prepare', base, family });
	return { base, worker, made: performance.now() };
}

function makeAudio(base: string): SpareAudio {
	const ctx = new AudioContext({ latencyHint: 'playback' });
	const worklet = ctx.audioWorklet.addModule(new URL('pcm/worklet.js', base).href);
	worklet.catch(() => {});
	return { base, ctx, worklet, made: performance.now() };
}

/**
 * The decoder families made ready ahead: MPEG audio, AAC and the rest ('more', which MP2 always
 * needs), and AC-3 only on a TV whose MediaSource refuses it.
 */
function familiesAhead(): string[] {
	const out = ['more'];
	try {
		if (!globalThis.MediaSource?.isTypeSupported?.('audio/mp4; codecs="ac-3"')) out.push('ac3');
	} catch {
		out.push('ac3');
	}
	return out;
}

/**
 * Makes what decoded sound needs before a load does: a decoder worker that has fetched and compiled
 * the common decoder families, and, on a TV whose MediaSource doesn't take FLAC, the audio context
 * (running, so its clock is steady by the time sound starts) with the worklet. The receiver calls it
 * when it starts, and again after each use.
 */
export function warmPcm(base: string) {
	if (typeof Worker === 'undefined') return;
	try {
		spareWorker ??= makeWorker(base, familiesAhead());
		if (!flacSupported() && typeof AudioContext !== 'undefined') spareAudio ??= makeAudio(base);
	} catch {
		// made when a load needs it
	}
}

function takeWorker(base: string, family: string): SpareWorker {
	let p = spareWorker;
	spareWorker = null;
	if (p && p.base !== base) {
		p.worker.terminate();
		p = null;
	}
	p ??= makeWorker(base, []);
	p.worker.postMessage({ type: 'prepare', base, family });
	return p;
}

function takeAudio(base: string): SpareAudio {
	let p = spareAudio;
	spareAudio = null;
	if (p && p.base !== base) {
		void p.ctx.close().catch(() => {});
		p = null;
	}
	return p ?? makeAudio(base);
}

/** Opens the decoder for `track` in `worker`, its sound going to `out`. */
function openDecoder(worker: Worker, base: string, track: Track, out: 'worklet' | 'flac', port?: MessagePort): Promise<void> {
	const d = decoderFor(track);
	if (!d) return Promise.reject(new Error(`no decoder for ${track.codec}`));
	return new Promise<void>((resolve, reject) => {
		worker.onmessage = (e) => {
			if (e.data?.type === 'open') resolve();
			else if (e.data?.type === 'error') reject(new Error(e.data.detail));
		};
		worker.onerror = (e) => reject(new Error(e.message || 'decoder worker failed'));
		worker.postMessage(
			{
				type: 'open',
				base,
				family: d.family,
				decoder: d.decoder,
				config: track.config,
				rate: track.sampleRate ?? 0,
				channels: track.channels ?? 0,
				unframed: !!track.unframed,
				out,
				port,
			},
			port ? [port] : [],
		);
	}).finally(() => {
		worker.onmessage = null;
	});
}

/** FLAC frames from the decoder: where each starts (media seconds), its samples, and the frame. */
export interface FlacFrame {
	pts: number;
	samples: number;
	data: Uint8Array;
}

/**
 * Sound decoded in WebAssembly and handed back as FLAC, for an audio SourceBuffer next to the
 * video's: the TV's own pipeline then keeps sound and picture together, as for any other stream.
 */
export class FlacAudio {
	private gen = 0;
	private lastPts = -Infinity;
	private draining: { gen: number; done: () => void }[] = [];
	/** How the setup went, for the timing line. */
	setup = '';
	/** performance.now() when the current start's first frames came back. */
	firstFrames: number | null = null;

	private constructor(
		private readonly worker: Worker,
		private readonly base: string,
		private readonly onFrames: (rate: number, frames: FlacFrame[]) => void,
	) {
		worker.onmessage = (e) => this.onMessage(e.data);
	}

	/** Loads the decoder for `track`. `onFrames` gets the FLAC frames of the current start, in order. */
	static async create(base: string, track: Track, start: number, onFrames: (rate: number, frames: FlacFrame[]) => void): Promise<FlacAudio> {
		const d = decoderFor(track);
		if (!d) throw new Error(`no decoder for ${track.codec}`);
		const t0 = performance.now();
		const spare = takeWorker(base, d.family);
		const age = Math.round(t0 - spare.made);
		try {
			await openDecoder(spare.worker, base, track, 'flac');
		} catch (e) {
			spare.worker.terminate();
			throw e;
		}
		const a = new FlacAudio(spare.worker, base, onFrames);
		a.setup = `worker ${age ? `${age} ms old` : 'new'}, decoder ${Math.round(performance.now() - t0)} ms`;
		a.seek(start);
		return a;
	}

	private onMessage(m: { type: string; gen: number; rate: number; frames: FlacFrame[] }) {
		if (m?.gen !== this.gen) return;
		if (m.type === 'flac') {
			if (!m.frames.length) return;
			this.firstFrames ??= performance.now();
			this.onFrames(m.rate, m.frames);
		} else if (m.type === 'drained') {
			const done = this.draining.filter((d) => d.gen === m.gen);
			this.draining = this.draining.filter((d) => d.gen !== m.gen);
			for (const d of done) d.done();
		}
	}

	/** The media time the compressed audio reaches. */
	get bufferedTo() {
		return this.lastPts;
	}

	push(samples: Sample[]) {
		if (!samples.length) return;
		const packets = samples.map((s) => ({ pts: s.pts, data: s.data }));
		this.lastPts = samples[samples.length - 1].pts;
		this.worker.postMessage({ type: 'packets', gen: this.gen, packets }, packets.map((p) => p.data.buffer as ArrayBuffer).filter(unique()));
	}

	/** Starts again at `time`: frames of the old start that are still on their way are dropped. */
	seek(time: number) {
		this.gen++;
		this.lastPts = -Infinity;
		this.firstFrames = null;
		for (const d of this.draining) d.done();
		this.draining = [];
		this.worker.postMessage({ type: 'flush', gen: this.gen, time });
	}

	/** Resolves once every packet pushed so far came back as frames (the end of the stream). */
	drain(): Promise<void> {
		return new Promise((done) => {
			this.draining.push({ gen: this.gen, done });
			this.worker.postMessage({ type: 'drain', gen: this.gen });
		});
	}

	/** Decoder and encoder CPU time so far (ms) and seconds decoded, from the worker. */
	cpu(): Promise<{ cpu: number; encode: number; decoded: number; heap: number }> {
		return new Promise((resolve) => {
			const prev = this.worker.onmessage;
			this.worker.onmessage = (e) => {
				if (e.data?.type !== 'stats') return prev?.call(this.worker, e);
				this.worker.onmessage = prev;
				resolve(e.data);
			};
			this.worker.postMessage({ type: 'stats' });
		});
	}

	destroy() {
		this.gen++;
		for (const d of this.draining) d.done();
		this.draining = [];
		this.worker.postMessage({ type: 'close' });
		const w = this.worker;
		setTimeout(() => w.terminate(), 1000);
		const base = this.base;
		setTimeout(() => warmPcm(base), 0);
	}
}

/** Why `ready()` resolved. */
export type Primed = 'sound' | 'timeout' | 'seek';

/** Seconds of decoded sound the worklet holds before the picture may start. */
export const PRIMED = 0.3;

/**
 * How the context's clock must keep pace with performance.now() before the picture goes: over the
 * last PACE_WINDOW ms, its highest offset (it moves in steps of the output buffer) no more than
 * PACE_SLIP below the highest in the PACE_WINDOW ms before. A context starved of time falls
 * behind, and its sound with it.
 */
const PACE_WINDOW = 250;
const PACE_SLIP = 0.015;
/** How often the context's clock is sampled while the start waits for it, ms. */
const PACE_EVERY = 20;

/** Whether the context kept pace, from [performance.now() ms, currentTime s] samples, oldest first. */
export function keptPace(samples: [number, number][], window = PACE_WINDOW, slip = PACE_SLIP): boolean {
	if (samples.length < 2) return false;
	const end = samples[samples.length - 1][0];
	if (end - samples[0][0] < window * 2) return false;
	let recent = -Infinity;
	let before = -Infinity;
	for (const [now, ctx] of samples) {
		const off = ctx - now / 1000;
		if (now > end - window) recent = Math.max(recent, off);
		else if (now > end - window * 2) before = Math.max(before, off);
	}
	return before > -Infinity && recent - before > -slip;
}

/** Readings in a window before the worklet steers by it (about 1.5 s). */
const SETTLED = 20;
/** Differences kept for the latency left over (about 15 s). */
const CALIBRATE = 200;
/** The most the leftover latency moves the clock, in seconds a second. */
const RAMP = 0.003;
/** The widest middle half of output timestamp differences that still calibrates. */
const TRUSTED = 0.05;
/** The largest leftover latency believed. */
const MAX_LATENCY = 0.3;

/** A clock reading this far from the median is a real jump (a seek, a stall caught up). */
const JUMP = 0.4;
/**
 * The same for the context offset, which strays only by the output buffer (under 90 ms on the
 * Chromecast TV): further off, the context has just started or stalled, and the window starts again.
 */
const CTX_JUMP = 0.15;

function push(list: number[], v: number, keep: number) {
	list.push(v);
	if (list.length > keep) list.shift();
}

function median(list: number[]): number {
	const s = [...list].sort((a, b) => a - b);
	const m = s.length >> 1;
	return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

/**
 * The mean of the middle half. A median of readings that fall on a few steps (a video time that
 * moves a frame or a decode batch at a time) hops between steps as the window slides; this moves
 * smoothly and still leaves out the stray readings at both ends.
 */
function middle(list: number[]): number {
	const s = [...list].sort((a, b) => a - b);
	const cut = s.length >> 2;
	const kept = s.slice(cut, s.length - cut);
	return kept.reduce((a, b) => a + b, 0) / kept.length;
}

/** The range of the middle half. */
function quartiles(list: number[]): number {
	const s = [...list].sort((a, b) => a - b);
	return s[Math.floor((s.length * 3) / 4)] - s[Math.floor(s.length / 4)];
}

function spread(list: number[]): number {
	return list.length ? Math.max(...list) - Math.min(...list) : 0;
}

export interface PcmStats {
	/** Audio minus video position at the last report, seconds (positive: audio behind). */
	drift: number;
	/** The largest drift since the last seek. */
	worst: number;
	/** Times the audio was behind and skipped forward. */
	jumps: number;
	/** Render quanta left silent because the audio was ahead of the video. */
	waits: number;
	/** Render quanta with nothing decoded to play. */
	starved: number;
	/** How far the video time readings stray from the averaged clock (RMS, seconds). */
	jitter: number;
	/** Seconds of PCM waiting in the worklet. */
	queued: number;
	time: number;
	/** The main thread's clock readings: spread of the video and context offsets (seconds). */
	clock?: ClockFilter['spread'];
	/** The AudioContext's state when the worklet last reported. */
	context?: string;
	/** The worklet's corrections since the start: jumps, runs of waits, and the start's line-up. */
	events?: { seq: number; kind: 'start' | 'jump' | 'wait'; at: number; by: number; why: string; n: number }[];
}

export class PcmAudio {
	stats: PcmStats = { drift: 0, worst: 0, jumps: 0, waits: 0, starved: 0, jitter: 0, queued: 0, time: 0 };
	private gen = 0;
	private timer = 0;
	private gain: GainNode;
	private listeners: [string, () => void][] = [];
	private lastPts = -Infinity;
	private filter = new ClockFilter();
	/** Sound is queued for the current start; until then, `ready()` callers wait. */
	private primed = false;
	private waiting: ((how: Primed) => void)[] = [];
	/** performance.now() when the current start's first sound was decoded, and when it was heard. */
	timing: { decoded: number | null; primed: number | null; paced: number | null; sound: number | null } = { decoded: null, primed: null, paced: null, sound: null };
	/** How the setup went: how old the ready-made parts were, and ms to the worklet and the open decoder. */
	setup = '';
	/** Clock readings in the first seconds after a start (see clock()), for the dev log. */
	trace: string[] = [];
	private traceFrom: number | null = null;
	private trace0: { v: number; c: number } | null = null;
	private dropped0: number | null = null;
	private created = performance.now();
	private base = '';

	/** The output latency the browser reports (outputLatency + baseLatency), in seconds. */
	get latency() {
		return { output: this.ctx.outputLatency || 0, base: this.ctx.baseLatency || 0 };
	}

	private constructor(
		private readonly ctx: AudioContext,
		private readonly node: AudioWorkletNode,
		private readonly worker: Worker,
		private readonly video: HTMLVideoElement,
	) {
		this.gain = ctx.createGain();
		node.connect(this.gain).connect(ctx.destination);
		node.port.onmessage = (e) => {
			const m = e.data;
			if (m?.gen !== this.gen) return;
			if (m.type === 'queued') {
				this.timing.decoded ??= performance.now();
				if (m.queued >= PRIMED) this.timing.primed ??= performance.now();
				if (!this.primed && m.queued >= PRIMED) this.primeWhenPaced();
			} else if (m.type === 'sounding') {
				// When the first sound is heard: rendered at context time `ctx`, out after the latency.
				const l = this.latency;
				this.timing.sound = performance.now() + (m.ctx + l.output + l.base - this.ctx.currentTime) * 1000;
			} else if (m.type === 'position') {
				this.stats = { ...m, clock: this.filter.spread, context: this.ctx.state };
				if (!this.primed && this.stats.queued >= PRIMED) this.primeWhenPaced();
			}
		};
		const clock = () => this.clock();
		for (const ev of ['play', 'playing', 'pause', 'seeking', 'seeked', 'ratechange', 'waiting', 'timeupdate', 'ended', 'stalled'])
			this.listen(ev, clock);
		this.listen('volumechange', () => this.volume());
		this.volume();
		this.timer = window.setInterval(clock, 100);
		this.clock();
	}

	private listen(ev: string, fn: () => void) {
		this.video.addEventListener(ev, fn);
		this.listeners.push([ev, fn]);
	}

	/** Loads the decoder for `track` and starts the worklet. Rejects when no decoder opens. */
	static async create(base: string, track: Track, video: HTMLVideoElement, start: number): Promise<PcmAudio> {
		const d = decoderFor(track);
		if (!d) throw new Error(`no decoder for ${track.codec}`);
		const t0 = performance.now();
		const spare = takeWorker(base, d.family);
		const audio = takeAudio(base);
		const { ctx } = audio;
		const worker = spare.worker;
		const ms = () => Math.round(performance.now() - t0);
		const age = Math.round(t0 - Math.max(spare.made, audio.made));
		try {
			if (ctx.state !== 'running') void ctx.resume().catch(() => {});
			await audio.worklet;
			const worklet = ms();
			const node = new AudioWorkletNode(ctx, 'kaeri-pcm', { numberOfInputs: 0, outputChannelCount: [2] });
			const channel = new MessageChannel();
			node.port.postMessage({ type: 'decoder', port: channel.port2 }, [channel.port2]);
			await openDecoder(worker, base, track, 'worklet', channel.port1);
			const pcm = new PcmAudio(ctx, node, worker, video);
			pcm.created = audio.made;
			pcm.base = base;
			pcm.setup = `parts ${age ? `${age} ms old` : 'new'} ${ctx.state}, worklet ${worklet} ms, decoder ${ms()} ms`;
			pcm.seek(start);
			if (ctx.state !== 'running') void ctx.resume().catch(() => {});
			return pcm;
		} catch (e) {
			worker.terminate();
			void ctx.close().catch(() => {});
			throw e;
		}
	}

	/** The media time the compressed audio reaches. */
	get bufferedTo() {
		return this.lastPts;
	}

	push(samples: Sample[]) {
		if (!samples.length) return;
		const packets = samples.map((s) => ({ pts: s.pts, data: s.data }));
		this.lastPts = samples[samples.length - 1].pts;
		this.worker.postMessage({ type: 'packets', gen: this.gen, packets }, packets.map((p) => p.data.buffer as ArrayBuffer).filter(unique()));
	}

	/**
	 * Resolves once the worklet holds PRIMED seconds of sound for the current start, or after
	 * `timeout` ms. The engine holds the first video back until then: on a slow TV the decoder
	 * starts later than the picture, and the sound would begin with a gap and then skip to catch up.
	 */
	ready(timeout = 2500): Promise<Primed> {
		if (this.primed) return Promise.resolve('sound');
		return new Promise((resolve) => {
			const timer = setTimeout(() => this.prime('timeout'), timeout);
			this.waiting.push((how) => {
				clearTimeout(timer);
				resolve(how);
			});
		});
	}

	get isPrimed() {
		return this.primed;
	}

	/** Samples of the context's clock while the start waits for it. */
	private pace: [number, number][] = [];
	private paceTimer = 0;

	/** Sound is queued: the picture goes once the context's clock keeps pace (see keptPace). */
	private primeWhenPaced() {
		if (this.paceTimer) return;
		const check = () => {
			if (this.primed) return this.stopPace();
			const now = performance.now();
			if (this.ctx.state === 'running') this.pace.push([now, this.ctx.currentTime]);
			while (this.pace.length && this.pace[0][0] < now - PACE_WINDOW * 3) this.pace.shift();
			if (this.ctx.state === 'running' && keptPace(this.pace)) {
				this.timing.paced = now;
				this.stopPace();
				this.prime();
			}
		};
		this.paceTimer = window.setInterval(check, PACE_EVERY);
		check();
	}

	private stopPace() {
		clearInterval(this.paceTimer);
		this.paceTimer = 0;
	}

	private prime(how: Primed = 'sound') {
		if (how !== 'seek') this.primed = true;
		const waiting = this.waiting;
		this.waiting = [];
		for (const w of waiting) w(how);
	}

	/** Starts again at `time` (a seek, or a new load position). */
	seek(time: number) {
		// Whoever waits for the old start gives up: the packets it pushed are flushed.
		this.prime('seek');
		this.primed = false;
		this.timing = { decoded: null, primed: null, paced: null, sound: null };
		this.stopPace();
		this.trace = [];
		this.traceFrom = null;
		this.dropped0 = null;
		this.gen++;
		this.lastPts = -Infinity;
		this.worker.postMessage({ type: 'flush', gen: this.gen, time });
		this.node.port.postMessage({ type: 'flush', gen: this.gen });
		this.clock();
	}

	private clock() {
		const v = this.video;
		const playing = !v.paused && !v.seeking && !v.ended && v.readyState >= 3;
		const now = performance.now();
		const latency = (this.ctx.outputLatency || 0) + (this.ctx.baseLatency || 0);
		const heard = this.ctx.currentTime - latency;
		const out = this.ctx.getOutputTimestamp?.();
		const ts =
			out && out.contextTime != null && out.performanceTime != null && out.performanceTime > 0
				? out.contextTime + (now - out.performanceTime) / 1000
				: null;
		// A context that isn't running yet has a clock standing still: no reading of it counts.
		const running = this.ctx.state === 'running';
		const k = this.filter.read(now / 1000, v.currentTime, heard, v.playbackRate, playing && running, ts);
		// The first 4 s of readings after a start, for the dev log: when, the raw offsets (video
		// time less heard context time, and the output timestamp's), and the one steered by.
		if (playing && running) {
			if (this.traceFrom == null) {
				this.traceFrom = now;
				this.trace0 = { v: v.currentTime - now / 1000, c: heard - now / 1000 };
				this.trace.push(`context ${Math.round(now - this.created)} ms old, at ${this.ctx.currentTime.toFixed(3)} s`);
			}
			// ms since playing: video offset / context offset / output timestamp offset, each against
			// its first value, > the video-to-context offset steered by.
			const z = this.trace0!;
			const d = (x: number) => Math.round(x * 1000);
			// Then the video buffered ahead (ms) and the frames dropped since playing began.
			if (now - this.traceFrom < 4000) {
				let ahead = 0;
				for (let i = 0; i < v.buffered.length; i++)
					if (v.buffered.start(i) <= v.currentTime + 0.05 && v.buffered.end(i) > v.currentTime) ahead = v.buffered.end(i) - v.currentTime;
				const dropped = v.getVideoPlaybackQuality?.().droppedVideoFrames ?? 0;
				this.dropped0 ??= dropped;
				this.trace.push(
					`${Math.round(now - this.traceFrom)}:${d(v.currentTime - now / 1000 - z.v)}/${d(heard - now / 1000 - z.c)}/${ts == null ? '-' : d(ts - now / 1000 - z.c)}>${d(k.media - k.ctx)}${k.settling ? 's' : ''} b${d(ahead)} d${dropped - this.dropped0}`,
				);
			}
		}
		this.node.port.postMessage({ type: 'clock', media: k.media, ctx: k.ctx, rate: v.playbackRate, playing, settling: k.settling });
		if (playing && this.ctx.state === 'suspended') void this.ctx.resume().catch(() => {});
	}

	private volume() {
		this.gain.gain.value = this.video.muted ? 0 : this.video.volume;
	}

	private tap: AnalyserNode | null = null;

	/** The decoded sound before the volume, for the testing build's level meter. */
	analyser(): AnalyserNode {
		if (!this.tap) {
			this.tap = this.ctx.createAnalyser();
			this.tap.fftSize = 2048;
			this.node.connect(this.tap);
		}
		return this.tap;
	}

	/** Decoder CPU time so far (ms) and seconds decoded, from the worker. */
	cpu(): Promise<{ cpu: number; decoded: number; heap: number }> {
		return new Promise((resolve) => {
			const prev = this.worker.onmessage;
			this.worker.onmessage = (e) => {
				if (e.data?.type !== 'stats') return;
				this.worker.onmessage = prev;
				resolve(e.data);
			};
			this.worker.postMessage({ type: 'stats' });
		});
	}

	destroy() {
		this.prime('seek');
		clearInterval(this.timer);
		for (const [ev, fn] of this.listeners) this.video.removeEventListener(ev, fn);
		this.worker.postMessage({ type: 'close' });
		setTimeout(() => this.worker.terminate(), 1000);
		this.node.disconnect();
		void this.ctx.close();
		// Ready for the next load that needs it.
		const base = this.base;
		if (base) setTimeout(() => warmPcm(base), 0);
	}
}

/** Filters a list to its first occurrences. */
function unique<T>() {
	const seen = new Set<T>();
	return (v: T) => (seen.has(v) ? false : (seen.add(v), true));
}
