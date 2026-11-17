// The FLAC encoder against the reference tools: its frames, written as a .flac file, must pass
// `flac -t` (every frame's CRC and the decode) and decode with FFmpeg to exactly the 16-bit samples
// that went in. Skipped where the tools aren't installed.

import { describe, expect, test } from 'bun:test';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { encodeFrame, MAX_BLOCK, streamInfo } from './flac-encode';

const have = (tool: string) => Bun.spawnSync(['which', tool]).exitCode === 0;
const tools = have('flac') && have('ffmpeg');

/** Encodes interleaved stereo `pcm` in blocks of the given sizes, as the worker does. */
function encode(pcm: Float32Array, blocks: number[], rate = 48000, start = 0): Uint8Array {
	const parts: Uint8Array[] = [streamInfo(rate, 2)];
	let at = 0;
	for (const size of blocks) {
		for (let i = 0; i < size; i += MAX_BLOCK) {
			const n = Math.min(MAX_BLOCK, size - i);
			parts.push(encodeFrame(pcm.subarray((at + i) * 2, (at + i + n) * 2), n, 2, start + at + i));
		}
		at += size;
	}
	const out = new Uint8Array(parts.reduce((a, p) => a + p.length, 0));
	let o = 0;
	for (const p of parts) {
		out.set(p, o);
		o += p.length;
	}
	return out;
}

function int16(pcm: Float32Array): Int16Array {
	return Int16Array.from(pcm, (v) => Math.max(-32768, Math.min(32767, Math.round(v * 32767))));
}

/** Writes the stream, checks it with flac -t and returns FFmpeg's decode. */
function roundTrip(file: Uint8Array): { ok: boolean; said: string; pcm: Int16Array } {
	const dir = mkdtempSync(join(tmpdir(), 'kaeri-flac-'));
	try {
		const path = join(dir, 'a.flac');
		writeFileSync(path, file);
		const t = Bun.spawnSync(['flac', '-t', '-s', path]);
		const raw = join(dir, 'a.raw');
		Bun.spawnSync(['ffmpeg', '-v', 'error', '-i', path, '-f', 's16le', '-y', raw]);
		const bytes = readFileSync(raw);
		// The stream carries no MD5 (it is made as it plays), which flac -t says.
		const said = t.stderr.toString().split('\n').filter((l) => l.trim() && !/MD5 signature/.test(l)).join('\n');
		return { ok: t.exitCode === 0, said, pcm: new Int16Array(bytes.buffer, bytes.byteOffset, bytes.length >> 1) };
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

function signal(frames: number, f: (i: number, c: number) => number): Float32Array {
	const pcm = new Float32Array(frames * 2);
	for (let i = 0; i < frames; i++) for (let c = 0; c < 2; c++) pcm[i * 2 + c] = f(i, c);
	return pcm;
}

let seed = 3;
const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647) * 2 - 1;

const cases: { name: string; pcm: Float32Array; blocks: number[]; rate?: number; start?: number }[] = [
	{ name: 'a tone in MP2-sized blocks', pcm: signal(1152 * 20, (i, c) => 0.5 * Math.sin((2 * Math.PI * (440 + c * 110) * i) / 48000)), blocks: Array(20).fill(1152) },
	{ name: 'noise at full scale', pcm: signal(1024 * 6, () => rnd()), blocks: Array(6).fill(1024) },
	{ name: 'silence', pcm: signal(1536 * 3, () => 0), blocks: Array(3).fill(1536) },
	{ name: 'clipping and steps', pcm: signal(4000, (i) => (i % 200 < 100 ? 1.2 : -1.2)), blocks: [4000] },
	{ name: 'odd and tiny blocks', pcm: signal(300 + 17 + 999 + 256 + 257, (i) => 0.3 * Math.sin(i / 7) + 0.01 * rnd()), blocks: [300, 17, 999, 256, 257] },
	{ name: 'a block longer than one frame', pcm: signal(10000, (i, c) => 0.4 * Math.sin(i / (13 + c))), blocks: [10000] },
	{ name: '24 kHz', pcm: signal(1152 * 4, (i) => 0.5 * Math.sin(i / 5)), blocks: Array(4).fill(1152), rate: 24000 },
	{ name: '44.1 kHz with sample numbers past 2^21', pcm: signal(4608 * 2, (i) => 0.2 * Math.sin(i / 3)), blocks: [4608, 4608], rate: 44100, start: 3_000_000 },
	{ name: 'sample numbers past 2^26 (a day of sound at 48 kHz is 2^32)', pcm: signal(1152 * 2, (i) => 0.2 * Math.sin(i / 3)), blocks: [1152, 1152], start: 100_000_000 },
];

describe('the FLAC encoder', () => {
	for (const c of cases)
		test.skipIf(!tools)(`${c.name}: flac -t passes and FFmpeg decodes the same samples`, () => {
			const file = encode(c.pcm, c.blocks, c.rate, c.start);
			const r = roundTrip(file);
			expect(r.said).toBe('');
			expect(r.ok).toBe(true);
			const want = int16(c.pcm);
			expect(r.pcm.length).toBe(want.length);
			let diff = 0;
			for (let i = 0; i < want.length; i++) if (r.pcm[i] !== want[i]) diff++;
			expect(diff).toBe(0);
		});

	test('compresses a tone to well under half of 16-bit PCM', () => {
		const pcm = signal(1152 * 50, (i) => 0.5 * Math.sin((2 * Math.PI * 440 * i) / 48000));
		const file = encode(pcm, Array(50).fill(1152));
		expect(file.length).toBeLessThan(pcm.length);
	});
});
