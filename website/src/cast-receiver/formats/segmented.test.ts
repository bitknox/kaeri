import { afterAll, beforeAll, describe, expect, test } from 'bun:test';
import { createMuxer } from './mux';
import { SegmentedStream, type Segment } from './segmented';
import type { Sample } from './types';

// An HLS stream of AAC in MPEG-TS whose audio PES packets run on from one segment into the next,
// as a segmenter writes them when it cuts at video keyframes on packet boundaries.

const RATE = 48000;
const FRAMES = 120;
const PER_PES = 8;
const PID = 0x101;
const PMT = 0x1000;
/** 1024 samples at 48 kHz on the 90 kHz clock. */
const TICKS = 1920;
const FIRST = 900000;

function adts(i: number): Uint8Array {
	const size = 7 + 180 + (i % 7) * 13;
	const f = new Uint8Array(size);
	const ch = 2;
	f.set([0xff, 0xf1, (1 << 6) | (3 << 2) | (ch >> 2), ((ch & 3) << 6) | (size >> 11), (size >> 3) & 0xff, ((size & 7) << 5) | 0x1f, 0xfc]);
	for (let k = 7; k < size; k++) f[k] = (i * 31 + k) & 0x7f;
	return f;
}

function pes(frames: Uint8Array[], pts: number): Uint8Array {
	const body = frames.reduce((n, f) => n + f.length, 0);
	const out = new Uint8Array(14 + body);
	const len = 8 + body;
	out.set([0, 0, 1, 0xc0, len >> 8, len & 0xff, 0x80, 0x80, 5]);
	out.set([0x21 | (Math.floor(pts / 2 ** 29) & 0x0e), (pts >> 22) & 0xff, ((pts >> 14) & 0xfe) | 1, (pts >> 7) & 0xff, ((pts << 1) & 0xfe) | 1], 9);
	let at = 14;
	for (const f of frames) {
		out.set(f, at);
		at += f.length;
	}
	return out;
}

const counters = new Map<number, number>();
function packets(pid: number, payload: Uint8Array): Uint8Array[] {
	const out: Uint8Array[] = [];
	for (let at = 0; at < payload.length; ) {
		const p = new Uint8Array(188).fill(0xff);
		const cc = counters.get(pid) ?? 0;
		counters.set(pid, (cc + 1) & 15);
		const n = Math.min(184, payload.length - at);
		p[0] = 0x47;
		p[1] = (at === 0 ? 0x40 : 0) | (pid >> 8);
		p[2] = pid & 0xff;
		if (n < 184) {
			// Stuffing in an adaptation field fills the last packet.
			p[3] = 0x30 | cc;
			p[4] = 183 - n;
			if (p[4] > 0) p[5] = 0;
		} else p[3] = 0x10 | cc;
		p.set(payload.subarray(at, at + n), 188 - n);
		at += n;
		out.push(p);
	}
	return out;
}

const pat = () => packets(0, new Uint8Array([0, 0x00, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xe0 | (PMT >> 8), PMT & 0xff, 0, 0, 0, 0]))[0];
const pmt = () =>
	packets(PMT, new Uint8Array([0, 0x02, 0xb0, 18, 0, 1, 0xc1, 0, 0, 0xe0 | (PID >> 8), PID & 0xff, 0xf0, 0, 0x0f, 0xe0 | (PID >> 8), PID & 0xff, 0xf0, 0, 0, 0, 0, 0]))[0];

/** The stream's audio packets, cut into three segments in the middle of PES packets. */
function segments(): Uint8Array[] {
	const audio: Uint8Array[] = [];
	for (let i = 0; i < FRAMES; i += PER_PES) {
		const frames = Array.from({ length: Math.min(PER_PES, FRAMES - i) }, (_, k) => adts(i + k));
		audio.push(...packets(PID, pes(frames, FIRST + i * TICKS)));
	}
	const cuts = [0, Math.floor(audio.length / 3) + 3, Math.floor((audio.length * 2) / 3) + 5, audio.length];
	const out: Uint8Array[] = [];
	for (let k = 0; k < 3; k++) {
		const list = [pat(), pmt(), ...audio.slice(cuts[k], cuts[k + 1])];
		// Each cut lands inside a PES: the segment opens with a packet that starts none.
		if (k > 0) expect(list[2][1] & 0x40).toBe(0);
		const seg = new Uint8Array(list.length * 188);
		list.forEach((p, i) => seg.set(p, i * 188));
		out.push(seg);
	}
	return out;
}

const bodies = segments();
const realFetch = globalThis.fetch;
beforeAll(() => {
	globalThis.fetch = (async (url: string) => {
		const k = Number(String(url).match(/seg(\d)\.ts/)?.[1]);
		return new Response(bodies[k] as Uint8Array<ArrayBuffer>, { headers: { 'Content-Type': 'video/mp2t' } });
	}) as typeof fetch;
});
afterAll(() => {
	globalThis.fetch = realFetch;
});

function stream() {
	const each = (FRAMES * 1024) / RATE / 3;
	const segs: Segment[] = [0, 1, 2].map((k) => ({ url: `http://relay.example/seg${k}.ts`, start: k * each, duration: each, epoch: 0 }));
	return new SegmentedStream({ kind: 'video', segments: segs, format: 'ts', muxed: true }, [], [], each * 3);
}

async function readAll(s: SegmentedStream): Promise<Sample[]> {
	const out: Sample[] = [];
	let batch;
	while ((batch = await s.read())) out.push(...batch);
	return out;
}

describe('HLS segments cut inside an audio PES', () => {
	test('every audio frame comes through, back to back', async () => {
		const s = stream();
		await s.open();
		const audio = s.tracks.find((t) => t.kind === 'audio')!;
		expect(audio.codec).toBe('aac');
		const samples = (await readAll(s)).filter((x) => x.track === audio.id);
		expect(samples.length).toBe(FRAMES);
		samples.forEach((x, i) => {
			expect(x.pts).toBeCloseTo((i * 1024) / RATE, 6);
			expect(x.data.length).toBe(180 + (i % 7) * 13);
		});
	});

	test('the audio fragments give every frame 1024 ticks', async () => {
		const s = stream();
		await s.open();
		const audio = s.tracks.find((t) => t.kind === 'audio')!;
		const samples = (await readAll(s)).filter((x) => x.track === audio.id);
		const muxer = createMuxer(audio, samples[0])!;
		const durations = [...trun(muxer.segment(samples)!), ...trun(muxer.flush()!)];
		expect(durations.length).toBe(FRAMES);
		expect(durations.every((d) => d === 1024)).toBe(true);
	});

	test('after a seek the parser starts over at the segment', async () => {
		const s = stream();
		await s.open();
		const audio = s.tracks.find((t) => t.kind === 'audio')!;
		await s.seek(1.5);
		const samples = (await readAll(s)).filter((x) => x.track === audio.id);
		// Segment 1 opens inside a PES: its frames start with the first whole PES.
		expect(samples.length).toBeGreaterThan(0);
		expect(samples.length).toBeLessThan(FRAMES);
		samples.forEach((x, i) => i && expect(x.pts - samples[i - 1].pts).toBeCloseTo(1024 / RATE, 6));
		expect(Math.round(samples[0].pts * RATE) % (PER_PES * 1024)).toBe(0);
	});
});

/** The sample durations of a fragment's trun. */
function trun(b: Uint8Array): number[] {
	for (let i = 4; i + 4 <= b.length; i++) {
		if (String.fromCharCode(b[i], b[i + 1], b[i + 2], b[i + 3]) !== 'trun') continue;
		const v = new DataView(b.buffer, b.byteOffset + i + 4);
		const n = v.getUint32(4);
		return Array.from({ length: n }, (_, k) => v.getUint32(12 + k * 16));
	}
	return [];
}
