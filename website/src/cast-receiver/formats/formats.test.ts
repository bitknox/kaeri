import { describe, expect, test } from 'bun:test';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { codecString } from './codecs';
import { DecodeTimes, audioBuffer, formatOf, sniff, videoRefusal } from './engine';
import { FileSource } from './file-source.test-helper';
import { Matroska } from './mkv';
import { Mp4File } from './mp4';
import { createMuxer, mimeChoices, mimeFor } from './mux';
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

// AudioSpecificConfig: object type (5 bits), rate index (4), channel config (4), then more.
const LC_48K_STEREO = [0x11, 0x90];
const LC_44K_STEREO = [0x12, 0x10];
const LC_48K_51 = [0x11, 0xb0];
const MAIN_48K_STEREO = [0x09, 0x90];
// Explicit HE-AAC: object type 5, core 24 kHz, stereo, SBR at 48 kHz, then AAC-LC.
const HE_V1 = [0x2b, 0x11, 0x88, 0x00];
// Explicit HE-AAC v2: object type 29, core 24 kHz, mono, SBR at 48 kHz, then AAC-LC.
const HE_V2 = [0xeb, 0x09, 0x88, 0x00];
// Backward-compatible SBR: AAC-LC 24 kHz stereo, then sync extension 0x2b7 with SBR at 48 kHz.
const LC_SBR_EXT = [0x13, 0x10, 0x56, 0xe5, 0x98];
// Escaped object type 31 + 11 = 42 (xHE-AAC, USAC).
const USAC = [0xf9, 0x46, 0x43, 0x00];

const aac = (config?: number[], more: Partial<Track> = {}): Track => ({
	id: 2,
	kind: 'audio',
	codec: 'aac',
	...(config ? { config: new Uint8Array(config) } : {}),
	...more,
});

/** A raw AAC frame with a 7-byte ADTS header: profile LC, rate index, channel config. */
function adtsFrame(sri: number, channels: number, payload = 16, profile = 1) {
	const size = 7 + payload;
	const f = new Uint8Array(size);
	f.set([0xff, 0xf1, (profile << 6) | (sri << 2) | (channels >> 2), ((channels & 3) << 6) | (size >> 11), (size >> 3) & 0xff, ((size & 7) << 5) | 0x1f, 0xfc]);
	for (let i = 7; i < size; i++) f[i] = i;
	return f;
}

describe('AAC for MediaSource', () => {
	test('the codec string follows the AudioSpecificConfig object type', () => {
		expect(codecString(aac(LC_48K_STEREO))).toBe('mp4a.40.2');
		expect(codecString(aac(LC_48K_51))).toBe('mp4a.40.2');
		expect(codecString(aac(MAIN_48K_STEREO))).toBe('mp4a.40.1');
		expect(codecString(aac(HE_V1))).toBe('mp4a.40.5');
		expect(codecString(aac(HE_V2))).toBe('mp4a.40.29');
		expect(codecString(aac(LC_SBR_EXT))).toBe('mp4a.40.2');
		expect(codecString(aac(USAC))).toBe('mp4a.40.42');
		expect(codecString(aac())).toBe('mp4a.40.2');
	});

	test('HE-AAC also offers the AAC-LC form, which every AAC decoder reads', () => {
		const m = (c: number[]) => mimeChoices(aac(c));
		expect(m(LC_48K_STEREO)).toEqual(['audio/mp4; codecs="mp4a.40.2"']);
		expect(m(HE_V1)).toEqual(['audio/mp4; codecs="mp4a.40.5"', 'audio/mp4; codecs="mp4a.40.2"']);
		expect(m(HE_V2)).toEqual(['audio/mp4; codecs="mp4a.40.29"', 'audio/mp4; codecs="mp4a.40.2"']);
		expect(m(MAIN_48K_STEREO)).toEqual(['audio/mp4; codecs="mp4a.40.1"', 'audio/mp4; codecs="mp4a.40.2"']);
	});

	test('a track with no config gets one from its rate and channels', () => {
		const t = aac(undefined, { sampleRate: 44100, channels: 2 });
		expect(mimeFor(t)?.mime).toBe('audio/mp4; codecs="mp4a.40.2"');
		expect([...t.config!]).toEqual(LC_44K_STEREO);
		const init = createMuxer(t)!.init();
		const esds = findBox(init, 'esds');
		expect(esds).toBeGreaterThan(0);
		// The DecoderSpecificInfo (tag 5) carries the config.
		const body = init.subarray(esds, esds + ((init[esds] << 24) | (init[esds + 1] << 16) | (init[esds + 2] << 8) | init[esds + 3]));
		const tag5 = body.findIndex((b, i) => b === 5 && body[i + 1] === 2 && body[i + 2] === 0x12 && body[i + 3] === 0x10);
		expect(tag5).toBeGreaterThan(0);
	});

	test('a track with no config and ADTS frames takes the config from the first header', () => {
		const t = aac();
		const first: Sample = { track: 2, pts: 0, dts: 0, duration: 0, key: true, data: adtsFrame(3, 6) };
		expect(mimeFor(t, first)?.mime).toBe('audio/mp4; codecs="mp4a.40.2"');
		expect([...t.config!]).toEqual(LC_48K_51);
		expect(t.channels).toBe(6);
		expect(t.sampleRate).toBe(48000);
	});

	test('ADTS that says AAC Main (24 kHz mono, as HE-AAC v2 streams from some muxers) is offered as AAC-LC', () => {
		const t = aac();
		const first: Sample = { track: 2, pts: 0, dts: 0, duration: 0, key: true, data: adtsFrame(6, 1, 16, 0) };
		expect(mimeChoices(t, first)).toEqual(['audio/mp4; codecs="mp4a.40.1"', 'audio/mp4; codecs="mp4a.40.2"']);
		expect([...t.config!]).toEqual([0x0b, 0x08]);
		const add = (mime: string) => {
			if (/40\.1"/.test(mime)) throw new DOMException('not supported', 'NotSupportedError');
			return {} as SourceBuffer;
		};
		const got = audioBuffer(t, first, add, new Set());
		expect(got.muxer?.mime).toBe('audio/mp4; codecs="mp4a.40.2"');
		// The init segment's config says AAC-LC, 24 kHz, mono; the track keeps what the file said.
		const init = got.muxer!.init();
		const esds = findBox(init, 'esds');
		const body = init.subarray(esds);
		expect(body.findIndex((b, i) => b === 5 && body[i + 1] === 2 && body[i + 2] === 0x13 && body[i + 3] === 0x08)).toBeGreaterThan(0);
		expect([...t.config!]).toEqual([0x0b, 0x08]);
	});

	test('ADTS headers are left out of the MP4 samples', () => {
		const t = aac(LC_48K_STEREO, { sampleRate: 48000, channels: 2 });
		const m = createMuxer(t)!;
		const frames = [0, 1, 2].map((i): Sample => ({ track: 2, pts: i * 0.021, dts: i * 0.021, duration: 1024 / 48000, key: true, data: adtsFrame(3, 2) }));
		const seg = m.segment(frames)!;
		const mdat = findBox(seg, 'mdat');
		const size = (seg[mdat] << 24) | (seg[mdat + 1] << 16) | (seg[mdat + 2] << 8) | seg[mdat + 3];
		// Two samples written (the last is held), 16 bytes each without their 7-byte headers.
		expect(size - 8).toBe(32);
		expect(seg[mdat + 8]).toBe(7);
	});

	test('MediaSource is asked for each form in turn, and a refusal of one form is remembered', () => {
		const refused = new Set<string>();
		const asked: string[] = [];
		const add = (mime: string) => {
			asked.push(mime);
			if (/40\.29/.test(mime)) throw new DOMException('not supported', 'NotSupportedError');
			return { mime } as unknown as SourceBuffer;
		};
		const got = audioBuffer(aac(HE_V2), undefined, add, refused);
		expect(got.muxer?.mime).toBe('audio/mp4; codecs="mp4a.40.2"');
		expect(asked).toEqual(['audio/mp4; codecs="mp4a.40.29"', 'audio/mp4; codecs="mp4a.40.2"']);
		expect([...refused]).toEqual(['audio/mp4; codecs="mp4a.40.29"']);
		// The next stream skips the refused form, and AAC-LC streams are untouched by it.
		asked.length = 0;
		audioBuffer(aac(HE_V2), undefined, add, refused);
		expect(asked).toEqual(['audio/mp4; codecs="mp4a.40.2"']);
		asked.length = 0;
		expect(audioBuffer(aac(LC_48K_STEREO), undefined, add, refused).muxer?.mime).toBe('audio/mp4; codecs="mp4a.40.2"');
	});

	test('when every form is refused, the reason names the forms and the config', () => {
		const add = () => {
			throw new DOMException('not supported', 'NotSupportedError');
		};
		const got = audioBuffer(aac(HE_V1), undefined, add, new Set());
		expect(got.sb).toBeNull();
		expect(got.why).toContain('mp4a.40.5');
		expect(got.why).toContain('mp4a.40.2');
		expect(got.why).toContain('NotSupportedError');
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
