import { describe, expect, test } from 'bun:test';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { DecodeTimes, formatOf, sniff, videoRefusal } from './engine';
import { FileSource } from './file-source.test-helper';
import { Matroska } from './mkv';
import { Mp4File } from './mp4';
import { createMuxer } from './mux';
import { decoderFor } from './pcm';
import { supSamples } from './subs';
import { TsFile } from './ts';
import type { Demuxer, Sample, Track } from './types';

const media = join(import.meta.dir, '../../../.cast-media');

describe('format detection', () => {
	test('the load hint wins, then the type, then the path', () => {
		expect(formatOf('http://relay.example/u/1', 'video/mp4', 'mkv')).toBe('mkv');
		expect(formatOf('http://relay.example/u/1', 'application/x-mpegurl')).toBe('hls');
		expect(formatOf('http://relay.example/u/1', 'application/dash+xml')).toBe('dash');
		expect(formatOf('http://relay.example/a/ep.webm')).toBe('mkv');
		expect(formatOf('http://relay.example/a/ep.m2ts')).toBe('ts');
		expect(formatOf('http://relay.example/a/stream')).toBeNull();
	});

	test('the bytes name the format when nothing else does', () => {
		expect(sniff(new Uint8Array([0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0]))).toBe('mkv');
		expect(sniff(new Uint8Array([0, 0, 0, 0x20, 0x66, 0x74, 0x79, 0x70]))).toBe('mp4');
		const ts = new Uint8Array(400);
		ts[0] = ts[188] = 0x47;
		expect(sniff(ts)).toBe('ts');
		expect(sniff(new TextEncoder().encode('#EXTM3U\n#EXT-X-VERSION:3\n'))).toBe('hls');
		expect(sniff(new TextEncoder().encode('<?xml version="1.0"?>\n<MPD>'))).toBe('dash');
	});
});

describe('video the TV can take', () => {
	test('a codec with no MediaSource form is refused from its codec id', () => {
		for (const codec of ['mpeg2', 'mpeg4', 'vc1']) {
			const r = videoRefusal({ id: 1, kind: 'video', codec });
			expect(r?.part).toBe('video');
			expect(r?.codec).toBe(`video:${codec}`);
		}
		for (const codec of ['h264', 'hevc', 'vp9', 'av1', 'vp8']) expect(videoRefusal({ id: 1, kind: 'video', codec })).toBeNull();
	});
});

describe('decode times', () => {
	test('reordered frames decode before they show, in increasing order', () => {
		// I P B B: presentation 0, 3, 1, 2 (frames of 1/24 s)
		const f = 1 / 24;
		const order = [0, 3, 1, 2, 4, 7, 5, 6, 8, 11, 9, 10];
		const input: Sample[] = order.map((n, i) => ({ track: 1, pts: n * f, dts: n * f, duration: f, key: i % 4 === 0, data: new Uint8Array(1) }));
		const d = new DecodeTimes();
		const out = [...d.push(input), ...d.flush()];
		expect(out.length).toBe(input.length);
		for (let i = 0; i < out.length; i++) {
			expect(out[i].dts).toBeLessThanOrEqual(out[i].pts + 1e-6);
			if (i) expect(out[i].dts).toBeGreaterThan(out[i - 1].dts);
		}
	});
});

describe('audio routes', () => {
	test('every audio codec has a WebAssembly decoder', () => {
		for (const codec of ['ac3', 'eac3', 'dts', 'truehd', 'mlp', 'flac', 'aac', 'mp3', 'mp2', 'opus', 'vorbis', 'alac', 'pcm_s16le', 'pcm_bluray'])
			expect(decoderFor({ id: 1, kind: 'audio', codec })).not.toBeNull();
		expect(decoderFor({ id: 1, kind: 'audio', codec: 'wma' })).toBeNull();
	});

	test('the FLAC sample entry repeats STREAMINFO bits per sample', () => {
		const info = new Uint8Array(4 + 4 + 34);
		info.set([0x66, 0x4c, 0x61, 0x43, 0x80, 0, 0, 34]);
		// 48 kHz, 2 channels, 24 bits
		const body = 8;
		info[body + 10] = 0x0b;
		info[body + 11] = 0xb8;
		info[body + 12] = (0 << 4) | (1 << 1) | ((23 >> 4) & 1);
		info[body + 13] = (23 & 15) << 4;
		const t: Track = { id: 2, kind: 'audio', codec: 'flac', config: info, sampleRate: 48000, channels: 2 };
		const init = createMuxer(t)!.init();
		const at = findBox(init, 'fLaC');
		expect(at).toBeGreaterThan(0);
		// Box header (8), SampleEntry (8), reserved (8), channels (2): then the sample size
		expect((init[at + 26] << 8) | init[at + 27]).toBe(24);
	});
});

function findBox(b: Uint8Array, type: string) {
	for (let i = 4; i + 4 <= b.length; i++) if (String.fromCharCode(b[i], b[i + 1], b[i + 2], b[i + 3]) === type) return i - 4;
	return -1;
}

describe('side PGS files', () => {
	test('a .sup splits into display sets at each end segment', () => {
		const seg = (pts: number, type: number, body: number[]) => [0x50, 0x47, (pts >>> 24) & 255, (pts >>> 16) & 255, (pts >>> 8) & 255, pts & 255, 0, 0, 0, 0, type, 0, body.length, ...body];
		const bytes = new Uint8Array([...seg(90000, 0x16, [1, 2]), ...seg(90000, 0x80, []), ...seg(180000, 0x16, [3]), ...seg(180000, 0x80, [])]);
		const s = supSamples(bytes, 9);
		expect(s.map((x) => x.pts)).toEqual([1, 2]);
		expect(s[0].data.length).toBe(13 + 2 + 13);
	});
});

// The generated test media (scripts/cast-receiver/test-media.ts), when it is there.
const files: [string, (src: FileSource) => Demuxer, string, string | null][] = [
	['mkv-h264-ac3.mkv', (s) => new Matroska(s), 'h264', 'ac3'],
	['mkv-h264-truehd.mkv', (s) => new Matroska(s), 'h264', 'truehd'],
	['webm-vp9-opus.webm', (s) => new Matroska(s), 'vp9', 'opus'],
	['mp4-h264-aac.mp4', (s) => new Mp4File(s), 'h264', 'aac'],
	['mp4-frag-h264-aac.mp4', (s) => new Mp4File(s), 'h264', 'aac'],
	['mp4-hevc-eac3.mp4', (s) => new Mp4File(s), 'hevc', 'eac3'],
	['ts-h264-dts.ts', (s) => new TsFile(s), 'h264', 'dts'],
];

describe.skipIf(!existsSync(media))('the test media', () => {
	for (const [file, make, video, audio] of files) {
		test(`${file}: tracks, samples and a seek`, async () => {
			const d = make(new FileSource(join(media, file)));
			await d.open();
			expect(d.tracks.find((t) => t.kind === 'video')?.codec).toBe(video);
			expect(d.tracks.find((t) => t.kind === 'audio')?.codec ?? null).toBe(audio);
			expect(d.duration).toBeGreaterThan(5);
			let n = 0;
			let batch;
			while ((batch = await d.read()) && n < 200) n += batch.length;
			expect(n).toBeGreaterThan(50);
			const from = await d.seek(5);
			expect(from).toBeLessThanOrEqual(5.01);
			// A short TS file is read from its start; longer ones are bisected.
			if (!file.endsWith('.ts')) expect(from).toBeGreaterThan(2);
			const after = await d.read();
			expect(after?.some((s) => s.key && s.pts >= from - 0.01)).toBe(true);
			d.close();
		});
	}
});
