// Writes one track's samples in a form MediaSource takes: fragmented MP4 for most codecs, WebM for
// VP8 and Vorbis (which MediaSource takes in WebM only). One muxer per SourceBuffer.

import { Writer } from './bytes';
import { ac3Box, codecString } from './codecs';
import type { Sample, Track } from './types';

export interface Muxer {
	/** The MIME type with codecs, for addSourceBuffer. */
	readonly mime: string;
	/** The initialization segment. */
	init(): Uint8Array;
	/** A media segment for these samples (decode order, one track). Durations of 0 are filled in. */
	segment(samples: Sample[]): Uint8Array | null;
	/** After a seek: forget the held sample. */
	reset(): void;
	/** The held last sample, as a segment, at the end of the stream. */
	flush(): Uint8Array | null;
}

/** Which container a track goes to MediaSource in, and its MIME type. Null when there is none. */
export function mimeFor(t: Track, first?: Sample): { container: 'mp4' | 'webm'; mime: string } | null {
	if (t.codec === 'vp8' || t.codec === 'vorbis') {
		return { container: 'webm', mime: `${t.kind}/webm; codecs="${t.codec}"` };
	}
	if (t.kind === 'audio' && (t.codec === 'ac3' || t.codec === 'eac3') && !t.config && first) t.config = ac3Box(first.data, 0);
	if (t.kind === 'video' && t.codec === 'vp9' && first) vp9Config(t, first.data);
	const codec = codecString(t);
	if (!codec) return null;
	return { container: 'mp4', mime: `${t.kind}/mp4; codecs="${codec}"` };
}

export function createMuxer(t: Track, first?: Sample): Muxer | null {
	const m = mimeFor(t, first);
	if (!m) return null;
	return m.container === 'webm' ? new WebmMuxer(t, m.mime) : new Mp4Muxer(t, m.mime);
}

/** Fills a VP9 track's profile and bit depth from a keyframe when Matroska didn't say. */
function vp9Config(t: Track, frame: Uint8Array) {
	if (t.config && t.config[0] === 1 && t.config.length >= 8) return;
	let profile = 0;
	let depth = 8;
	let level = t.height && t.height > 1080 ? 50 : 40;
	if (t.config && t.config.length >= 3) {
		// Matroska's CodecPrivate for VP9: id, length, value features.
		for (let i = 0; i + 2 < t.config.length; i += 2 + t.config[i + 1]) {
			const id = t.config[i];
			const v = t.config[i + 2];
			if (id === 1) profile = v;
			else if (id === 2) level = v;
			else if (id === 3) depth = v;
		}
	} else if (frame.length) {
		const b = frame[0];
		profile = ((b >> 5) & 1) | (((b >> 4) & 1) << 1);
		if (profile >= 2) depth = 10;
	}
	t.config = new Uint8Array([1, 0, 0, 0, profile, level, (depth << 4) | (1 << 1), 1, 1, 1, 0, 0]);
}

// ---------- Fragmented MP4 ----------

function box(w: Writer, type: string, body: (w: Writer) => void) {
	const at = w.mark();
	w.ascii(type);
	body(w);
	w.patch32(at, w.length - at);
}
function full(w: Writer, type: string, version: number, flags: number, body: (w: Writer) => void) {
	box(w, type, (w) => {
		w.u32((version << 24) | flags);
		body(w);
	});
}

function descriptor(w: Writer, tag: number, body: Uint8Array) {
	w.u8(tag);
	let n = body.length;
	const bytes = [];
	do {
		bytes.unshift(n & 0x7f);
		n >>= 7;
	} while (n > 0);
	for (let i = 0; i < bytes.length; i++) w.u8(bytes[i] | (i < bytes.length - 1 ? 0x80 : 0));
	w.bytes(body);
}

function esdsBody(oti: number, config: Uint8Array | undefined): Uint8Array {
	const dcd = new Writer();
	dcd.u8(oti).u8(0x15).u24(0).u32(0).u32(0);
	if (config?.length) descriptor(dcd, 5, config);
	const es = new Writer();
	es.u16(1).u8(0);
	descriptor(es, 4, dcd.done());
	descriptor(es, 6, new Uint8Array([2]));
	const w = new Writer();
	descriptor(w, 3, es.done());
	return w.done();
}

function aacChannels(config?: Uint8Array): number | null {
	if (!config || config.length < 2) return null;
	return (config[1] >> 3) & 15 || null;
}

/** dOps from OpusHead. */
function dOps(head: Uint8Array | undefined, channels: number): Uint8Array {
	const w = new Writer();
	if (head && head.length >= 19) {
		const v = new DataView(head.buffer, head.byteOffset, head.byteLength);
		w.u8(0).u8(head[9]).u16(v.getUint16(10, true)).u32(v.getUint32(12, true)).u16(v.getInt16(16, true) & 0xffff).u8(head[18]);
		if (head[18] && head.length >= 21 + head[9]) w.bytes(head.subarray(19, 21 + head[9]));
	} else w.u8(0).u8(channels).u16(312).u32(48000).u16(0).u8(0);
	return w.done();
}

/** dfLa's metadata blocks from "fLaC" + blocks; only STREAMINFO, marked last. */
function dfLa(config: Uint8Array | undefined): Uint8Array {
	if (!config) return new Uint8Array(0);
	const at = config[0] === 0x66 && config[1] === 0x4c && config[2] === 0x61 && config[3] === 0x43 ? 4 : 0;
	const len = (config[at + 1] << 16) | (config[at + 2] << 8) | config[at + 3];
	const out = config.slice(at, at + 4 + len);
	out[0] = 0x80; // STREAMINFO, last block
	return out;
}

/** STREAMINFO's bits per sample, which the sample entry has to repeat. */
function flacBits(config: Uint8Array | undefined): number {
	const block = dfLa(config);
	if (block.length < 4 + 14) return 16;
	const b = block.subarray(4);
	return (((b[12] & 1) << 4) | (b[13] >> 4)) + 1;
}

export class Mp4Muxer implements Muxer {
	private readonly scale: number;
	private held: Sample | null = null;
	private lastDuration = 0;
	private seq = 1;
	/**
	 * Ticks added to every decode time, so the earliest (below 0 in files with B-frames) starts at
	 * 0. The same for every fragment: a decode time that steps back reads as a break to
	 * MediaSource, which then drops frames up to the next keyframe.
	 */
	private shift = 0;

	constructor(
		private readonly t: Track,
		readonly mime: string,
	) {
		this.scale = t.kind === 'audio' ? Math.round(t.sampleRate || 48000) : 90000;
		if (t.codec === 'opus') this.scale = 48000;
	}

	init(): Uint8Array {
		const t = this.t;
		const w = new Writer();
		box(w, 'ftyp', (w) => w.ascii('isom').u32(1).ascii('isomiso6mp41'));
		box(w, 'moov', (w) => {
			full(w, 'mvhd', 0, 0, (w) => {
				w.u32(0).u32(0).u32(1000).u32(0).u32(0x00010000).u16(0x0100).u16(0).u32(0).u32(0);
				for (const v of [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000]) w.u32(v);
				for (let i = 0; i < 6; i++) w.u32(0);
				w.u32(2);
			});
			box(w, 'trak', (w) => {
				full(w, 'tkhd', 0, 3, (w) => {
					w.u32(0).u32(0).u32(1).u32(0).u32(0).u32(0).u32(0);
					w.u16(0).u16(0).u16(t.kind === 'audio' ? 0x0100 : 0).u16(0);
					for (const v of [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000]) w.u32(v);
					w.u32((t.width ?? 0) * 65536).u32((t.height ?? 0) * 65536);
				});
				box(w, 'mdia', (w) => {
					full(w, 'mdhd', 0, 0, (w) => w.u32(0).u32(0).u32(this.scale).u32(0).u16(0x55c4).u16(0));
					full(w, 'hdlr', 0, 0, (w) => w.u32(0).ascii(t.kind === 'audio' ? 'soun' : 'vide').u32(0).u32(0).u32(0).u8(0));
					box(w, 'minf', (w) => {
						if (t.kind === 'audio') full(w, 'smhd', 0, 0, (w) => w.u32(0));
						else full(w, 'vmhd', 0, 1, (w) => w.u32(0).u32(0));
						box(w, 'dinf', (w) => full(w, 'dref', 0, 0, (w) => {
							w.u32(1);
							full(w, 'url ', 0, 1, () => {});
						}));
						box(w, 'stbl', (w) => {
							full(w, 'stsd', 0, 0, (w) => {
								w.u32(1);
								this.entry(w);
							});
							for (const n of ['stts', 'stsc', 'stco']) full(w, n, 0, 0, (w) => w.u32(0));
							full(w, 'stsz', 0, 0, (w) => w.u32(0).u32(0));
						});
					});
				});
			});
			box(w, 'mvex', (w) => full(w, 'trex', 0, 0, (w) => w.u32(1).u32(1).u32(0).u32(0).u32(0)));
		});
		return w.done();
	}

	private entry(w: Writer) {
		const t = this.t;
		if (t.kind === 'video') {
			const type = { h264: 'avc1', hevc: 'hvc1', av1: 'av01', vp9: 'vp09' }[t.codec] ?? t.codec;
			const cfg = { h264: 'avcC', hevc: 'hvcC', av1: 'av1C', vp9: 'vpcC' }[t.codec];
			box(w, type, (w) => {
				w.u32(0).u16(0).u16(1);
				w.u16(0).u16(0).u32(0).u32(0).u32(0);
				w.u16(t.width ?? 0).u16(t.height ?? 0);
				w.u32(0x00480000).u32(0x00480000).u32(0).u16(1);
				for (let i = 0; i < 32; i++) w.u8(0);
				w.u16(0x18).u16(0xffff);
				if (cfg && t.config) {
					if (t.codec === 'vp9') box(w, cfg, (w) => w.bytes(t.config!.length >= 12 ? t.config!.subarray(0, 12) : t.config!));
					else box(w, cfg, (w) => w.bytes(t.config!));
				}
			});
			return;
		}
		const type = { aac: 'mp4a', mp3: 'mp4a', ac3: 'ac-3', eac3: 'ec-3', opus: 'Opus', flac: 'fLaC' }[t.codec] ?? t.codec;
		const channels = (t.codec === 'aac' ? aacChannels(t.config) : null) ?? t.channels ?? 2;
		const rate = t.codec === 'opus' ? 48000 : Math.round(t.sampleRate || 48000);
		box(w, type, (w) => {
			w.u32(0).u16(0).u16(1);
			w.u32(0).u32(0);
			w.u16(Math.min(channels, 8)).u16(t.codec === 'flac' ? flacBits(t.config) : 16).u16(0).u16(0);
			w.u32(rate < 65536 ? rate * 65536 : 0);
			switch (t.codec) {
				case 'aac':
					full(w, 'esds', 0, 0, (w) => w.bytes(esdsBody(0x40, t.config)));
					break;
				case 'mp3':
					full(w, 'esds', 0, 0, (w) => w.bytes(esdsBody(0x6b, undefined)));
					break;
				case 'ac3':
					box(w, 'dac3', (w) => w.bytes(t.config ?? new Uint8Array(3)));
					break;
				case 'eac3':
					box(w, 'dec3', (w) => w.bytes(t.config ?? new Uint8Array(5)));
					break;
				case 'opus':
					box(w, 'dOps', (w) => w.bytes(dOps(t.config, channels)));
					break;
				case 'flac':
					full(w, 'dfLa', 0, 0, (w) => w.bytes(dfLa(t.config)));
					break;
			}
		});
	}

	reset() {
		this.held = null;
	}

	segment(input: Sample[]): Uint8Array | null {
		if (!input.length) return null;
		const all = this.held ? [this.held, ...input] : input.slice();
		this.held = all.pop()!;
		return this.write(all, this.held);
	}

	flush(): Uint8Array | null {
		if (!this.held) return null;
		const last = this.held;
		this.held = null;
		return this.write([last], null);
	}

	private write(samples: Sample[], next: Sample | null): Uint8Array | null {
		if (!samples.length) return null;
		const s = this.scale;
		const ticks = (v: number) => Math.round(v * s);
		const durations = samples.map((x, i) => {
			const after = i + 1 < samples.length ? samples[i + 1] : next;
			let d = after ? ticks(after.dts) - ticks(x.dts) : 0;
			if (d <= 0) d = x.duration > 0 ? ticks(x.duration) : this.lastDuration;
			if (d <= 0) d = this.t.kind === 'audio' ? 1024 : Math.round(s / 24);
			this.lastDuration = d;
			return d;
		});
		const first = ticks(samples[0].dts);
		if (first + this.shift < 0) this.shift = -first;
		const base = first + this.shift;
		let size = 0;
		for (const x of samples) size += x.data.length;
		const w = new Writer();
		const moofAt = w.length;
		let dataOffsetAt = 0;
		box(w, 'moof', (w) => {
			full(w, 'mfhd', 0, 0, (w) => w.u32(this.seq++));
			box(w, 'traf', (w) => {
				full(w, 'tfhd', 0, 0x020000, (w) => w.u32(1));
				full(w, 'tfdt', 1, 0, (w) => w.u64(base));
				full(w, 'trun', 1, 0x000f01, (w) => {
					w.u32(samples.length);
					dataOffsetAt = w.length;
					w.u32(0);
					let dts = base;
					samples.forEach((x, i) => {
						w.u32(durations[i]).u32(x.data.length);
						w.u32(this.t.kind === 'audio' || x.key ? 0x02000000 : 0x01010000);
						w.u32(ticks(x.pts) - dts);
						dts += durations[i];
					});
				});
			});
		});
		const moofSize = w.length - moofAt;
		w.patch32(dataOffsetAt, moofSize + 8);
		w.u32(size + 8).ascii('mdat');
		for (const x of samples) w.bytes(x.data);
		return w.done();
	}
}

// ---------- WebM ----------

function ebmlId(w: Writer, id: number) {
	if (id > 0xffffff) w.u32(id);
	else if (id > 0xffff) w.u24(id);
	else if (id > 0xff) w.u16(id);
	else w.u8(id);
}
/** An element with an 8-byte size, filled in after its body. */
function el(w: Writer, id: number, body: (w: Writer) => void) {
	ebmlId(w, id);
	const sizeAt = w.length;
	w.u32(0).u32(0);
	const start = w.length;
	body(w);
	const n = w.length - start;
	w.set(sizeAt, [0x01, 0, 0, 0, (n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255]);
}
function uintEl(w: Writer, id: number, v: number) {
	el(w, id, (w) => w.u32(v));
}

export class WebmMuxer implements Muxer {
	private held: Sample | null = null;

	constructor(
		private readonly t: Track,
		readonly mime: string,
	) {}

	init(): Uint8Array {
		const t = this.t;
		const w = new Writer();
		el(w, 0x1a45dfa3, (w) => {
			uintEl(w, 0x4286, 1);
			uintEl(w, 0x42f7, 1);
			uintEl(w, 0x42f2, 4);
			uintEl(w, 0x42f3, 8);
			el(w, 0x4282, (w) => w.ascii('webm'));
			uintEl(w, 0x4287, 4);
			uintEl(w, 0x4285, 2);
		});
		// Segment of unknown size
		w.u32(0x18538067).u8(0x01).u8(0xff).u16(0xffff).u32(0xffffffff);
		el(w, 0x1549a966, (w) => {
			uintEl(w, 0x2ad7b1, 1_000_000);
			el(w, 0x4d80, (w) => w.ascii('Kaeri'));
			el(w, 0x5741, (w) => w.ascii('Kaeri'));
		});
		el(w, 0x1654ae6b, (w) =>
			el(w, 0xae, (w) => {
				uintEl(w, 0xd7, 1);
				uintEl(w, 0x73c5, 1);
				uintEl(w, 0x83, t.kind === 'video' ? 1 : 2);
				el(w, 0x86, (w) => w.ascii(t.codec === 'vp8' ? 'V_VP8' : t.codec === 'vorbis' ? 'A_VORBIS' : t.codec === 'vp9' ? 'V_VP9' : 'A_OPUS'));
				if (t.config?.length) el(w, 0x63a2, (w) => w.bytes(t.config!));
				if (t.kind === 'video')
					el(w, 0xe0, (w) => {
						uintEl(w, 0xb0, t.width ?? 0);
						uintEl(w, 0xba, t.height ?? 0);
					});
				else
					el(w, 0xe1, (w) => {
						el(w, 0xb5, (w) => {
							const b = new Uint8Array(8);
							new DataView(b.buffer).setFloat64(0, t.sampleRate || 48000);
							w.bytes(b);
						});
						uintEl(w, 0x9f, t.channels ?? 2);
					});
			}),
		);
		return w.done();
	}

	reset() {
		this.held = null;
	}

	segment(input: Sample[]): Uint8Array | null {
		if (!input.length) return null;
		const all = this.held ? [this.held, ...input] : input.slice();
		this.held = all.pop()!;
		return this.write(all);
	}

	flush(): Uint8Array | null {
		if (!this.held) return null;
		const last = this.held;
		this.held = null;
		return this.write([last]);
	}

	private write(samples: Sample[]): Uint8Array | null {
		if (!samples.length) return null;
		const ms = (v: number) => Math.round(v * 1000);
		const w = new Writer();
		let base = ms(samples[0].pts);
		for (const s of samples) base = Math.min(base, ms(s.pts));
		base = Math.max(0, base);
		el(w, 0x1f43b675, (w) => {
			uintEl(w, 0xe7, base);
			for (const s of samples) {
				const rel = Math.max(-32768, Math.min(32767, ms(s.pts) - base));
				el(w, 0xa3, (w) => {
					w.u8(0x81).u16(rel & 0xffff).u8(s.key || this.t.kind === 'audio' ? 0x80 : 0);
					w.bytes(s.data);
				});
			}
		});
		return w.done();
	}
}
