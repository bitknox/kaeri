// The AudioWorklet that plays decoded PCM in step with the video element's clock. The main thread
// sends the clock (video time at a context time, rate, playing); every render quantum works out
// the media time it should be playing and steers towards it: small drift by resampling up to
// 0.5 % faster or slower, which can't be heard, and large jumps (a seek, a stall) at once.
//
// A TV reports the video time coarsely: in whole frames, and late by however long the main
// thread took to read it, tens of milliseconds either way. Steering towards each reading as it
// comes skips and pauses the sound all the time. So the readings are averaged: each one moves the
// clock a little, and only a jump beyond JUMP (a seek, a stall caught up) moves it at once.

/** A correction the worklet made: where (media time), by how much (seconds) and which rule. */
interface SyncEvent {
	seq: number;
	kind: 'start' | 'jump' | 'wait';
	at: number;
	by: number;
	why: string;
	/** Quanta in a wait run. */
	n: number;
}

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
const SETTLE = 0.5;
/** The jump threshold while settling. */
const SETTLE_HARD = 0.02;
/** The jump threshold while the main thread's clock filter fills its window after the start. */
const WIDE = 0.12;
/** A clock reading this far from the clock is a real jump, taken at once. */
const JUMP = 0.25;
/** The least weight a clock reading gets (about the last 20 readings, 2 s, count). */
const WEIGHT = 0.05;

class KaeriPcm extends AudioWorkletProcessor {
	private chunks: Chunk[] = [];
	/** Read position in the first chunk, in its frames (fractional). */
	private offset = 0;
	private gen = 0;
	private clock = { media: 0, ctx: 0, rate: 1, playing: false };
	/** The main thread's clock estimate is still young: leave moderate drift alone. */
	private settling = false;
	/** Readings averaged into the clock since it last moved at once. */
	private readings = 0;
	/** Mean square of the readings' error against the clock, for the stats. */
	private noise = 0;
	private decoder: MessagePort | null = null;
	private lastReport = 0;
	private drift = 0;
	private worst = 0;
	private jumps = 0;
	private starved = 0;
	private waits = 0;
	/** Jumps and runs of waits since the last start, with their size and the rule behind them. */
	private events: SyncEvent[] = [];
	private seq = 0;
	/** The wait run going on now, extended while the next quanta wait too. */
	private waitRun: SyncEvent | null = null;
	/** Sound has played since the last start. */
	private sounded = false;
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
				this.onClock(m);
			} else if (m.type === 'flush') {
				if (m.gen > this.gen) this.reset(m.gen);
			} else if (m.type === 'decoder') {
				this.decoder = m.port as MessagePort;
				this.decoder.onmessage = (d) => this.onPcm(d.data);
			}
		};
	}

	/** A clock reading: averaged in while playing steadily, else taken as it is. */
	private onClock(m: { media: number; ctx: number; rate: number; playing: boolean; settling?: boolean }) {
		this.settling = !!m.settling;
		const k = this.clock;
		const now = { media: m.media, ctx: m.ctx, rate: m.rate, playing: m.playing };
		if (!m.playing || !k.playing || m.rate !== k.rate || m.ctx < k.ctx) {
			this.clock = now;
			this.readings = 0;
			return;
		}
		const predicted = k.media + (m.ctx - k.ctx) * k.rate;
		const error = m.media - predicted;
		if (Math.abs(error) > JUMP) {
			this.clock = now;
			this.readings = 0;
			return;
		}
		this.readings++;
		this.noise += (error * error - this.noise) * 0.05;
		const weight = Math.max(WEIGHT, 1 / (this.readings + 1));
		this.clock = { ...now, media: predicted + error * weight };
	}

	/** A new start (a seek): what is queued goes, and the next audio lines up exactly. */
	private reset(gen: number) {
		this.gen = gen;
		this.chunks = [];
		this.offset = 0;
		this.worst = 0;
		this.fresh = true;
		this.sounded = false;
		this.waitRun = null;
	}

	private queued(): number {
		let queued = 0;
		for (const c of this.chunks) queued += c.frames / c.rate;
		return queued - (this.chunks[0] ? this.offset / this.chunks[0].rate : 0);
	}

	private event(kind: SyncEvent['kind'], at: number, by: number, why: string) {
		const e = { seq: ++this.seq, kind, at, by, why, n: 1 };
		this.events.push(e);
		if (this.events.length > 16) this.events.shift();
		return e;
	}

	private onPcm(m: { type: string; gen: number; pts: number; rate: number; data: Float32Array }) {
		// The decoder's audio for a seek can come before the main thread's flush does.
		if (m.type !== 'pcm' || m.gen < this.gen) return;
		if (m.gen > this.gen) this.reset(m.gen);
		this.chunks.push({ pts: m.pts, rate: m.rate, data: m.data, frames: m.data.length / 2 });
		// Until the sound starts, the main thread waits for it (the start gate): tell it at once.
		if (!this.sounded) this.port.postMessage({ type: 'queued', gen: this.gen, queued: this.queued() });
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
		const first = !this.sounded;
		if (this.fresh) this.settleUntil = currentTime + SETTLE;
		this.fresh = false;
		// Drift that resampling doesn't fix within a moment is fixed by jumping too.
		// While the main thread's estimate is young, only a wide drift counts: the start keeps the
		// line-up it made in its first SETTLE seconds.
		const young = this.settling && currentTime >= this.settleUntil;
		if (Math.abs(diff) > SOFT && !young) this.overSince ??= currentTime;
		else this.overSince = null;
		const stuck = this.overSince != null && currentTime - this.overSince > 0.5;
		const hard = currentTime < this.settleUntil ? SETTLE_HARD : young ? WIDE : stuck ? SOFT : HARD;
		if (diff > hard) {
			// Behind: catch up at once. Before any sound has played this only picks where it starts.
			this.skipTo(expected);
			if (first) this.event('start', expected, diff, 'line-up');
			else {
				this.jumps++;
				this.event('jump', expected, diff, currentTime < this.settleUntil ? 'settle' : young ? 'young' : stuck ? 'stuck' : 'hard');
			}
			this.overSince = null;
			pos = this.position();
			if (pos == null) return true;
			diff = expected - pos;
		} else if (diff < -hard) {
			// Ahead (the video stalled or went back a little): wait in silence.
			if (-diff > n / sampleRate) {
				this.waits++;
				if (this.waitRun && this.waitRun.seq === this.seq) this.waitRun.n++;
				else this.waitRun = this.event('wait', expected, diff, currentTime < this.settleUntil ? 'settle' : young ? 'young' : stuck ? 'stuck' : 'hard');
				return true;
			}
		}
		this.waitRun = null;
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
			// The last frame of a chunk leans towards the first of the next one.
			const after = j + 1 < c.frames ? c : (this.chunks[1] ?? c);
			const b = after === c ? Math.min(a + 2, c.data.length - 2) : 0;
			left[i] = c.data[a] + (after.data[b] - c.data[a]) * f;
			if (right !== left) right[i] = c.data[a + 1] + (after.data[b + 1] - c.data[a + 1]) * f;
			this.offset += (c.rate / sampleRate) * k.rate * (1 + trim);
		}
		if (!this.sounded) {
			this.sounded = true;
			this.port.postMessage({ type: 'sounding', gen: this.gen, ctx: currentTime });
		}
		return true;
	}

	private report(expected: number) {
		if (currentTime - this.lastReport < 0.1) return;
		this.lastReport = currentTime;
		const pos = this.position();
		const queued = this.queued();
		const events = this.events.map((e) => ({ ...e }));
		const status = { type: 'position', gen: this.gen, time: pos ?? expected, expected, drift: this.drift, worst: this.worst, jumps: this.jumps, waits: this.waits, starved: this.starved, jitter: Math.sqrt(this.noise), queued, at: currentTime, events };
		this.decoder?.postMessage(status);
		this.port.postMessage(status);
	}
}

registerProcessor('kaeri-pcm', KaeriPcm);
