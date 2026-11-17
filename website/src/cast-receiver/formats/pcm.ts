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
 * mean of the middle half of the last 40 readings (about 3 s). On a TV both readings stray by tens of milliseconds
 * (the video time moves in steps, the context's output timestamp wanders), while the offsets
 * themselves stay put during steady playback. A reading far off the median (a seek, a stall caught
 * up) starts the window again.
 */
export class ClockFilter {
	private v: number[] = [];
	private c: number[] = [];
	private rate = 1;
	private playing = false;
	/** Spread of the video readings (max - min), and how far the highest sits above the median. */
	spread = { video: 0, videoHigh: 0, ctx: 0 };

	constructor(private readonly keep = 40) {}

	/** One reading at performance time `now` (seconds); returns the clock to steer by. */
	read(now: number, media: number, ctx: number, rate: number, playing: boolean): { media: number; ctx: number } {
		if (!playing || rate !== this.rate || !this.playing) {
			this.v = [];
			this.c = [];
		}
		this.rate = rate;
		this.playing = playing;
		const vo = media - now * rate;
		const co = ctx - now;
		if (this.v.length >= 3 && Math.abs(vo - median(this.v)) > JUMP) this.v = [];
		if (this.c.length >= 3 && Math.abs(co - median(this.c)) > JUMP) this.c = [];
		push(this.v, vo, this.keep);
		push(this.c, co, this.keep);
		const mv = middle(this.v);
		const mc = middle(this.c);
		this.spread = { video: spread(this.v), videoHigh: Math.max(...this.v) - mv, ctx: spread(this.c) };
		return { media: mv + now * rate, ctx: mc + now };
	}
}

/** A clock reading this far from the median is a real jump (a seek, a stall caught up). */
const JUMP = 0.4;

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
}

export class PcmAudio {
	stats: PcmStats = { drift: 0, worst: 0, jumps: 0, waits: 0, starved: 0, jitter: 0, queued: 0, time: 0 };
	private gen = 0;
	private timer = 0;
	private gain: GainNode;
	private listeners: [string, () => void][] = [];
	private lastPts = -Infinity;
	private filter = new ClockFilter();

	private constructor(
		private readonly ctx: AudioContext,
		private readonly node: AudioWorkletNode,
		private readonly worker: Worker,
		private readonly video: HTMLVideoElement,
	) {
		this.gain = ctx.createGain();
		node.connect(this.gain).connect(ctx.destination);
		node.port.onmessage = (e) => {
			if (e.data?.type === 'position' && e.data.gen === this.gen) this.stats = { ...e.data, clock: this.filter.spread };
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
		const ctx = new AudioContext({ latencyHint: 'playback' });
		const worker = new Worker(new URL('pcm/worker.js', base).href, { type: 'module' });
		try {
			await ctx.audioWorklet.addModule(new URL('pcm/worklet.js', base).href);
			const node = new AudioWorkletNode(ctx, 'kaeri-pcm', { numberOfInputs: 0, outputChannelCount: [2] });
			const channel = new MessageChannel();
			node.port.postMessage({ type: 'decoder', port: channel.port2 }, [channel.port2]);
			await new Promise<void>((resolve, reject) => {
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
						port: channel.port1,
					},
					[channel.port1],
				);
			});
			worker.onmessage = null;
			const pcm = new PcmAudio(ctx, node, worker, video);
			pcm.seek(start);
			if (ctx.state !== 'running') void ctx.resume().catch(() => {});
			return pcm;
		} catch (e) {
			worker.terminate();
			void ctx.close();
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

	/** Starts again at `time` (a seek, or a new load position). */
	seek(time: number) {
		this.gen++;
		this.lastPts = -Infinity;
		this.worker.postMessage({ type: 'flush', gen: this.gen, time });
		this.node.port.postMessage({ type: 'flush', gen: this.gen });
		this.clock();
	}

	private clock() {
		const v = this.video;
		const playing = !v.paused && !v.seeking && !v.ended && v.readyState >= 3;
		let ctxNow = this.ctx.currentTime;
		const ts = this.ctx.getOutputTimestamp?.();
		if (ts && ts.contextTime != null && ts.performanceTime != null && ts.performanceTime > 0)
			ctxNow = ts.contextTime + (performance.now() - ts.performanceTime) / 1000;
		const k = this.filter.read(performance.now() / 1000, v.currentTime, ctxNow, v.playbackRate, playing);
		this.node.port.postMessage({ type: 'clock', media: k.media, ctx: k.ctx, rate: v.playbackRate, playing });
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
		clearInterval(this.timer);
		for (const [ev, fn] of this.listeners) this.video.removeEventListener(ev, fn);
		this.worker.postMessage({ type: 'close' });
		setTimeout(() => this.worker.terminate(), 1000);
		this.node.disconnect();
		void this.ctx.close();
	}
}

/** Filters a list to its first occurrences. */
function unique<T>() {
	const seen = new Set<T>();
	return (v: T) => (seen.has(v) ? false : (seen.add(v), true));
}
