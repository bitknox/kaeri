// The AudioWorklet that plays decoded PCM in step with the video element's clock. The main thread
// sends the clock (video time at a context time, rate, playing); every render quantum works out
// the media time it should be playing and steers towards it: small drift by resampling up to
// 0.5 % faster or slower, which can't be heard, and large jumps (a seek, a stall) at once.

interface Chunk {
	pts: number;
	rate: number;
	data: Float32Array; // interleaved stereo
	frames: number;
}

declare const currentTime: number;
declare const sampleRate: number;
declare function registerProcessor(name: string, ctor: unknown): void;
declare class AudioWorkletProcessor {
	readonly port: MessagePort;
}

/** Drift beyond this many seconds is fixed by jumping; below it, by resampling. */
const HARD = 0.08;
/** Drift below this is left alone. */
const DEAD = 0.004;
const MAX_TRIM = 0.005;
/** Drift beyond this that lasts half a second is fixed by jumping. */
const SOFT = 0.025;
/** Seconds after a start or a seek in which drift is fixed by jumping at once. */
const SETTLE = 1;

class KaeriPcm extends AudioWorkletProcessor {
	private chunks: Chunk[] = [];
	/** Read position in the first chunk, in its frames (fractional). */
	private offset = 0;
	private gen = 0;
	private clock = { media: 0, ctx: 0, rate: 1, playing: false };
	private decoder: MessagePort | null = null;
	private lastReport = 0;
	private drift = 0;
	private worst = 0;
	private jumps = 0;
	private starved = 0;
	private waits = 0;
	/** Nothing has played since a seek or a stall: line up exactly rather than steering. */
	private fresh = true;
	/** Until this context time after a start, drift is fixed by jumping at a small threshold. */
	private settleUntil = 0;
	/** Since when drift has stayed beyond SOFT. */
	private overSince: number | null = null;

	constructor() {
		super();
		this.port.onmessage = (e) => {
			const m = e.data;
			if (m.type === 'clock') {
				this.clock = { media: m.media, ctx: m.ctx, rate: m.rate, playing: m.playing };
			} else if (m.type === 'flush') {
				if (m.gen > this.gen) this.reset(m.gen);
			} else if (m.type === 'decoder') {
				this.decoder = m.port as MessagePort;
				this.decoder.onmessage = (d) => this.onPcm(d.data);
			}
		};
	}

	/** A new start (a seek): what is queued goes, and the next audio lines up exactly. */
	private reset(gen: number) {
		this.gen = gen;
		this.chunks = [];
		this.offset = 0;
		this.worst = 0;
		this.fresh = true;
	}

	private onPcm(m: { type: string; gen: number; pts: number; rate: number; data: Float32Array }) {
		// The decoder's audio for a seek can come before the main thread's flush does.
		if (m.type !== 'pcm' || m.gen < this.gen) return;
		if (m.gen > this.gen) this.reset(m.gen);
		this.chunks.push({ pts: m.pts, rate: m.rate, data: m.data, frames: m.data.length / 2 });
	}

	/** The media time of the next frame to play. */
	private position(): number | null {
		const c = this.chunks[0];
		return c ? c.pts + this.offset / c.rate : null;
	}

	/** Drops frames up to media time `t`. */
	private skipTo(t: number) {
		while (this.chunks.length) {
			const c = this.chunks[0];
			const end = c.pts + c.frames / c.rate;
			if (end <= t) {
				this.chunks.shift();
				this.offset = 0;
				continue;
			}
			this.offset = Math.max(this.offset, (t - c.pts) * c.rate);
			return;
		}
	}

	process(_inputs: Float32Array[][], outputs: Float32Array[][]): boolean {
		const out = outputs[0];
		const left = out[0];
		const right = out[1] ?? out[0];
		const n = left.length;
		left.fill(0);
		if (right !== left) right.fill(0);
		const k = this.clock;
		const expected = k.media + (currentTime - k.ctx) * k.rate;
		this.report(expected);
		if (!k.playing) return true;
		let pos = this.position();
		if (pos == null) {
			this.starved++;
			this.fresh = true;
			return true;
		}
		let diff = expected - pos;
		if (this.fresh) this.settleUntil = currentTime + SETTLE;
		this.fresh = false;
		// Drift that resampling doesn't fix within a moment is fixed by jumping too.
		if (Math.abs(diff) > SOFT) this.overSince ??= currentTime;
		else this.overSince = null;
		const stuck = this.overSince != null && currentTime - this.overSince > 0.5;
		const hard = currentTime < this.settleUntil ? DEAD * 2 : stuck ? SOFT : HARD;
		if (diff > hard) {
			// Behind: catch up at once.
			this.skipTo(expected);
			this.jumps++;
			this.overSince = null;
			pos = this.position();
			if (pos == null) return true;
			diff = expected - pos;
		} else if (diff < -hard) {
			// Ahead (the video stalled or went back a little): wait in silence.
			if (-diff > n / sampleRate) {
				this.waits++;
				return true;
			}
		}
		this.drift = diff;
		this.worst = Math.max(this.worst, Math.abs(diff));
		const trim = Math.abs(diff) < DEAD ? 0 : Math.max(-MAX_TRIM, Math.min(MAX_TRIM, diff * 0.5));
		for (let i = 0; i < n; i++) {
			let c = this.chunks[0];
			if (!c) {
				this.starved++;
				this.fresh = true;
				break;
			}
			while (this.offset >= c.frames) {
				this.offset -= c.frames;
				const next = this.chunks[1];
				this.chunks.shift();
				if (!next) break;
				// A gap or overlap between chunks: start the next one where its own time says.
				const gap = next.pts - (c.pts + c.frames / c.rate);
				if (Math.abs(gap) > 0.02) this.offset = Math.max(0, (pos! + (i * k.rate) / sampleRate - next.pts) * next.rate);
				c = next;
			}
			if (!this.chunks.length) break;
			c = this.chunks[0];
			const j = Math.floor(this.offset);
			const f = this.offset - j;
			const a = j * 2;
			const b = j + 1 < c.frames ? a + 2 : a;
			left[i] = c.data[a] + (c.data[b] - c.data[a]) * f;
			if (right !== left) right[i] = c.data[a + 1] + (c.data[b + 1] - c.data[a + 1]) * f;
			this.offset += (c.rate / sampleRate) * k.rate * (1 + trim);
		}
		return true;
	}

	private report(expected: number) {
		if (currentTime - this.lastReport < 0.1) return;
		this.lastReport = currentTime;
		const pos = this.position();
		let queued = 0;
		for (const c of this.chunks) queued += c.frames / c.rate;
		queued -= this.chunks[0] ? this.offset / this.chunks[0].rate : 0;
		const status = { type: 'position', gen: this.gen, time: pos ?? expected, expected, drift: this.drift, worst: this.worst, jumps: this.jumps, waits: this.waits, starved: this.starved, queued, at: currentTime };
		this.decoder?.postMessage(status);
		this.port.postMessage(status);
	}
}

registerProcessor('kaeri-pcm', KaeriPcm);
