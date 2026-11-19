import { describe, expect, test } from 'bun:test';
import { Matroska, codecFrameTime } from './mkv';
import type { ByteSource } from './source';
import type { Sample } from './types';

// A small Matroska file built in memory: one audio track and laced blocks, the way mkvmerge
// writes audio (several frames per SimpleBlock, no BlockDuration).

class MemorySource implements ByteSource {
	size: number;
	constructor(private readonly data: Uint8Array) {
		this.size = data.length;
	}
	async bytes(offset: number, length: number) {
		return this.data.subarray(offset, Math.min(this.size, offset + length));
	}
	prefetch() {}
	close() {}
}

const cat = (...parts: (Uint8Array | number[])[]) => {
	const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
	let at = 0;
	for (const p of parts) {
		out.set(p, at);
		at += p.length;
	}
	return out;
};

const beBytes = (n: number) => {
	const out: number[] = [];
	do {
		out.unshift(n & 0xff);
		n = Math.floor(n / 256);
	} while (n > 0);
	return out;
};

function size(n: number): number[] {
	let len = 1;
	while (n >= 2 ** (7 * len) - 1) len++;
	const out: number[] = [];
	for (let i = 0; i < len; i++) out.unshift(Math.floor(n / 256 ** i) & 0xff);
	out[0] |= 0x80 >> (len - 1);
	return out;
}

const el = (id: number, ...body: (Uint8Array | number[])[]) => {
	const b = cat(...body);
	return cat(beBytes(id), size(b.length), b);
};
const uint = (id: number, n: number) => el(id, beBytes(n));
const str = (id: number, s: string) => el(id, [...new TextEncoder().encode(s)]);
const float = (id: number, x: number) => {
	const b = new Uint8Array(8);
	new DataView(b.buffer).setFloat64(0, x);
	return el(id, b);
};

/** A block of track 1 at `ms`, its frames Xiph-laced when there are several. */
function block(ms: number, frames: Uint8Array[]): Uint8Array {
	const head = [0x81, (ms >> 8) & 0xff, ms & 0xff];
	if (frames.length === 1) return cat(head, [0x80], frames[0]);
	const sizes = frames.slice(0, -1).map((f) => f.length);
	return cat(head, [0x80 | 0x02, frames.length - 1], sizes, ...frames);
}

const frame = (n: number) => new Uint8Array(20).fill(n);
const frames = (n: number) => Array.from({ length: n }, (_, i) => frame(i + 1));

interface Audio {
	codec: string;
	rate: number;
	/** DefaultDuration in ns, when the track gives one. */
	defaultDuration?: number;
}

function file(audio: Audio, cluster: Uint8Array[]): Uint8Array {
	const entry = el(
		0xae,
		uint(0xd7, 1),
		uint(0x83, 2),
		str(0x86, audio.codec),
		...(audio.codec === 'A_AAC' ? [el(0x63a2, [0x11, 0x90])] : []),
		...(audio.defaultDuration ? [uint(0x23e383, audio.defaultDuration)] : []),
		el(0xe1, float(0xb5, audio.rate), uint(0x9f, 2)),
	);
	const segment = el(
		0x18538067,
		el(0x1549a966, uint(0x2ad7b1, 1_000_000), float(0x4489, 2000)),
		el(0x1654ae6b, entry),
		el(0x1f43b675, uint(0xe7, 0), ...cluster),
	);
	return cat(el(0x1a45dfa3, str(0x4282, 'matroska')), segment);
}

const simple = (b: Uint8Array) => el(0xa3, b);
const group = (b: Uint8Array, duration: number) => el(0xa0, el(0xa1, b), uint(0x9b, duration));

async function samples(bytes: Uint8Array): Promise<Sample[]> {
	const m = new Matroska(new MemorySource(bytes));
	await m.open();
	const out: Sample[] = [];
	let batch;
	while ((batch = await m.read())) out.push(...batch);
	return out;
}

const AAC_FRAME = 1024 / 48000;

function expectSteps(s: Sample[], from: number, step: number) {
	s.forEach((x, i) => {
		expect(x.pts).toBeCloseTo(from + i * step, 6);
		expect(x.dts).toBeCloseTo(x.pts, 9);
		expect(x.duration).toBeCloseTo(step, 6);
	});
}

describe('laced Matroska audio', () => {
	test('frames step by the track default duration', async () => {
		const s = await samples(file({ codec: 'A_AAC', rate: 48000, defaultDuration: 21_333_333 }, [simple(block(0, frames(8))), simple(block(171, frames(8)))]));
		expect(s.length).toBe(16);
		expectSteps(s.slice(0, 8), 0, 0.021333333);
		expectSteps(s.slice(8), 0.171, 0.021333333);
		// The frames keep their order and bytes.
		expect(s.slice(0, 8).map((x) => x.data[0])).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
	});

	test('without a default duration, AAC frames step by 1024 samples', async () => {
		const s = await samples(file({ codec: 'A_AAC', rate: 48000 }, [simple(block(0, frames(8))), simple(block(171, frames(8)))]));
		expectSteps(s.slice(0, 8), 0, AAC_FRAME);
		expectSteps(s.slice(8), 0.171, AAC_FRAME);
	});

	test('a block duration is shared among its frames', async () => {
		const s = await samples(file({ codec: 'A_AAC', rate: 48000, defaultDuration: 21_333_333 }, [group(block(0, frames(4)), 100)]));
		expectSteps(s, 0, 0.025);
	});

	test('frames of a codec with no fixed frame size spread to the next block', async () => {
		const s = await samples(file({ codec: 'A_OPUS', rate: 48000 }, [simple(block(0, frames(4))), simple(block(80, frames(4)))]));
		expect(s.length).toBe(8);
		expectSteps(s.slice(0, 4), 0, 0.02);
		// The last block has nothing after it: its frames take the last spread.
		expectSteps(s.slice(4), 0.08, 0.02);
	});

	test('an unlaced block keeps its time and leaves the duration to the muxer', async () => {
		const s = await samples(file({ codec: 'A_AAC', rate: 48000 }, [simple(block(0, [frame(1)])), simple(block(21, [frame(2)]))]));
		expect(s.map((x) => x.pts)).toEqual([0, 0.021]);
		expect(s.map((x) => x.duration)).toEqual([0, 0]);
	});

	test('no two laced frames share a time stamp', async () => {
		const blocks = Array.from({ length: 20 }, (_, i) => simple(block(Math.round(i * 8 * AAC_FRAME * 1000), frames(8))));
		const s = await samples(file({ codec: 'A_AAC', rate: 48000 }, blocks));
		for (let i = 1; i < s.length; i++) expect(s[i].pts).toBeGreaterThan(s[i - 1].pts);
	});
});

describe('codec frame times', () => {
	test('fixed frame sizes by codec and rate', () => {
		expect(codecFrameTime('aac', 48000)).toBeCloseTo(1024 / 48000, 9);
		expect(codecFrameTime('ac3', 48000)).toBeCloseTo(0.032, 9);
		expect(codecFrameTime('eac3', 48000)).toBeCloseTo(0.032, 9);
		expect(codecFrameTime('mp3', 44100)).toBeCloseTo(1152 / 44100, 9);
		expect(codecFrameTime('mp3', 24000)).toBeCloseTo(576 / 24000, 9);
		expect(codecFrameTime('dts', 48000)).toBeCloseTo(512 / 48000, 9);
	});

	test('no frame time for variable frames or an unknown rate', () => {
		expect(codecFrameTime('opus', 48000)).toBe(0);
		expect(codecFrameTime('vorbis', 44100)).toBe(0);
		expect(codecFrameTime('aac', 0)).toBe(0);
	});
});
