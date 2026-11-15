// ISO BMFF (MP4, MOV, fragmented MP4): tracks from the moov, samples from its tables or from
// moof fragments. Used for MP4 files and for HLS fMP4 and DASH segments.

import { Reader, ascii, utf8 } from './bytes';
import type { ByteSource } from './source';
import type { Demuxer, Sample, Track } from './types';

interface Box {
	type: string;
	/** Offset of the box. */
	start: number;
	/** Offset of its body. */
	body: number;
	/** Offset after it; Infinity for a box that runs to the end. */
	end: number;
}

function boxAt(b: Uint8Array, at: number, base = 0): Box | null {
	if (at + 8 > b.length) return null;
	const r = new Reader(b, at);
	let size = r.u32();
	const type = r.ascii(4);
	let head = 8;
	if (size === 1) {
		if (at + 16 > b.length) return null;
		size = r.u64();
		head = 16;
	}
	const start = base + at;
	return { type, start, body: start + head, end: size === 0 ? Infinity : start + size };
}

function* boxes(b: Uint8Array, from = 0, to = b.length): Generator<{ type: string; body: Uint8Array; at: number }> {
	let at = from;
	while (at + 8 <= to) {
		const x = boxAt(b, at);
		if (!x || x.end - x.start < 8) return;
		const end = Math.min(to, x.end);
		yield { type: x.type, body: b.subarray(x.body, end), at };
		at = end;
	}
}

function child(b: Uint8Array, path: string[]): Uint8Array | null {
	let cur: Uint8Array | null = b;
	for (const p of path) {
		let next: Uint8Array | null = null;
		for (const x of boxes(cur!)) if (x.type === p) {
			next = x.body;
			break;
		}
		if (!next) return null;
		cur = next;
	}
	return cur;
}

export interface Mp4Track extends Track {
	timescale: number;
	/** Seconds taken off every time by the edit list. */
	shift: number;
	trex?: { duration: number; size: number; flags: number };
	table?: Table;
}

interface Table {
	offsets: Float64Array;
	sizes: Uint32Array;
	dts: Float64Array; // in timescale units
	cts: Int32Array;
	sync: Uint8Array | null; // null: every sample is a sync sample
}

const AUDIO_OTI: Record<number, string> = { 0x40: 'aac', 0x66: 'aac', 0x67: 'aac', 0x68: 'aac', 0x69: 'mp3', 0x6b: 'mp3', 0xa5: 'ac3', 0xa6: 'eac3', 0xa9: 'dts', 0xdd: 'vorbis', 0xad: 'opus' };

/** Reads an MPEG-4 descriptor's tag and length. */
function descriptor(r: Reader): { tag: number; size: number } {
	const tag = r.u8();
	let size = 0;
	for (let i = 0; i < 4; i++) {
		const b = r.u8();
		size = (size << 7) | (b & 0x7f);
		if (!(b & 0x80)) break;
	}
	return { tag, size };
}

function esds(body: Uint8Array): { oti: number; config?: Uint8Array } {
	const r = new Reader(body, 4);
	let oti = 0;
	let config: Uint8Array | undefined;
	while (r.left > 2) {
		const d = descriptor(r);
		if (d.tag === 3) {
			r.skip(2);
			const flags = r.u8();
			if (flags & 0x80) r.skip(2);
			if (flags & 0x40) r.skip(r.u8());
			if (flags & 0x20) r.skip(2);
		} else if (d.tag === 4) {
			oti = r.u8();
			r.skip(12);
		} else if (d.tag === 5) {
			config = r.bytes(d.size).slice();
			break;
		} else r.skip(d.size);
	}
	return { oti, config };
}

/** OpusHead (what decoders and Matroska use) from MP4's dOps. */
function opusHead(dops: Uint8Array, channels: number): Uint8Array {
	const r = new Reader(dops);
	r.skip(1);
	const ch = r.u8();
	const preSkip = r.u16();
	const rate = r.u32();
	const gain = r.u16();
	const family = r.u8();
	const head = new Uint8Array(19 + (family ? 2 + ch : 0));
	head.set([0x4f, 0x70, 0x75, 0x73, 0x48, 0x65, 0x61, 0x64, 1, ch || channels]);
	const v = new DataView(head.buffer);
	v.setUint16(10, preSkip, true);
	v.setUint32(12, rate, true);
	v.setInt16(16, gain, true);
	head[18] = family;
	if (family) head.set(r.bytes(2 + ch), 19);
	return head;
}

function sampleEntry(entry: { type: string; body: Uint8Array }, handler: string, t: Mp4Track) {
	const type = entry.type;
	const b = entry.body;
	if (handler === 'vide') {
		const r = new Reader(b, 24);
		t.width = r.u16();
		t.height = r.u16();
		const kids = Array.from(boxes(b, 78));
		const find = (n: string) => kids.find((k) => k.type === n)?.body.slice();
		const base = { avc1: 'h264', avc3: 'h264', hvc1: 'hevc', hev1: 'hevc', dvh1: 'hevc', dvhe: 'hevc', vp09: 'vp9', vp08: 'vp8', av01: 'av1', mp4v: 'mpeg4' }[type];
		t.codec = base ?? type;
		t.config = find('avcC') ?? find('hvcC') ?? find('vpcC') ?? find('av1C');
		if (type === 'mp4v') {
			const e = find('esds');
			if (e && esds(e).oti === 0x61) t.codec = 'mpeg2';
		}
		return;
	}
	if (handler === 'soun') {
		const r = new Reader(b, 8);
		const version = r.u16();
		r.skip(6);
		t.channels = r.u16();
		t.bitDepth = r.u16();
		r.skip(4);
		t.sampleRate = r.u32() / 65536;
		let at = 28;
		if (version === 1) at += 16;
		else if (version === 2) {
			const v2 = new Reader(b, 28 + 4);
			t.sampleRate = new DataView(b.buffer, b.byteOffset + v2.pos).getFloat64(0);
			t.channels = new Reader(b, 28 + 12).u32();
			at += 36;
		}
		const kids = Array.from(boxes(b, at));
		const find = (n: string) => kids.find((k) => k.type === n)?.body;
		const wave = find('wave');
		const es = find('esds') ?? (wave ? child(wave, ['esds']) : null);
		switch (type) {
			case 'mp4a': {
				const e = es ? esds(es) : { oti: 0x40 };
				t.codec = AUDIO_OTI[e.oti] ?? 'aac';
				if (e.config && t.codec === 'aac') t.config = e.config;
				break;
			}
			case '.mp3':
				t.codec = 'mp3';
				break;
			case 'ac-3':
				t.codec = 'ac3';
				break;
			case 'ec-3':
				t.codec = 'eac3';
				break;
			case 'dtsc': case 'dtsh': case 'dtsl': case 'dtse':
				t.codec = 'dts';
				break;
			case 'mlpa':
				t.codec = 'truehd';
				break;
			case 'Opus': {
				t.codec = 'opus';
				const d = find('dOps');
				if (d) t.config = opusHead(d, t.channels ?? 2);
				break;
			}
			case 'fLaC': {
				t.codec = 'flac';
				const d = find('dfLa');
				// dfLa is a full box around FLAC metadata blocks; decoders want "fLaC" + blocks.
				if (d) t.config = new Uint8Array([0x66, 0x4c, 0x61, 0x43, ...d.subarray(4)]);
				break;
			}
			case 'alac': {
				t.codec = 'alac';
				const d = find('alac');
				if (d) t.config = new Uint8Array([0, 0, 0, d.length + 8, 0x61, 0x6c, 0x61, 0x63, ...d]);
				break;
			}
			case 'sowt': t.codec = t.bitDepth === 24 ? 'pcm_s24le' : 'pcm_s16le'; break;
			case 'twos': t.codec = t.bitDepth === 24 ? 'pcm_s24be' : 'pcm_s16be'; break;
			case 'ipcm': case 'lpcm': t.codec = 'pcm_s16le'; break;
			default:
				t.codec = type.trim().toLowerCase();
		}
		return;
	}
	// Text
	if (type === 'tx3g' || type === 'text') t.codec = 'tx3g';
	else if (type === 'wvtt') t.codec = 'webvtt';
	else t.codec = type;
}

/** Tracks from a moov box's body. */
export function parseMoov(moov: Uint8Array): { tracks: Mp4Track[]; duration: number; fragmented: boolean } {
	const tracks: Mp4Track[] = [];
	let duration = NaN;
	const mvhd = child(moov, ['mvhd']);
	if (mvhd) {
		const r = new Reader(mvhd);
		const v = r.u8();
		r.skip(3);
		if (v === 1) {
			r.skip(16);
			const scale = r.u32();
			duration = r.u64() / scale;
		} else {
			r.skip(8);
			const scale = r.u32();
			const d = r.u32();
			duration = d === 0xffffffff ? NaN : d / scale;
		}
	}
	const trex = new Map<number, Mp4Track['trex']>();
	const mvex = child(moov, ['mvex']);
	if (mvex)
		for (const x of boxes(mvex))
			if (x.type === 'trex') {
				const r = new Reader(x.body, 4);
				const id = r.u32();
				r.skip(4);
				trex.set(id, { duration: r.u32(), size: r.u32(), flags: r.u32() });
			}
	for (const trak of boxes(moov)) {
		if (trak.type !== 'trak') continue;
		const tkhd = child(trak.body, ['tkhd']);
		const mdhd = child(trak.body, ['mdia', 'mdhd']);
		const hdlr = child(trak.body, ['mdia', 'hdlr']);
		const stbl = child(trak.body, ['mdia', 'minf', 'stbl']);
		if (!tkhd || !mdhd || !hdlr || !stbl) continue;
		const tk = new Reader(tkhd);
		const tv = tk.u8();
		tk.skip(3 + (tv === 1 ? 16 : 8));
		const id = tk.u32();
		const md = new Reader(mdhd);
		const mv = md.u8();
		md.skip(3 + (mv === 1 ? 16 : 8));
		const timescale = md.u32();
		md.skip(mv === 1 ? 8 : 4);
		const lang = md.u16();
		const language = lang
			? String.fromCharCode(((lang >> 10) & 31) + 0x60, ((lang >> 5) & 31) + 0x60, (lang & 31) + 0x60)
			: undefined;
		const handler = ascii(hdlr, 8, 12);
		const kind = handler === 'vide' ? 'video' : handler === 'soun' ? 'audio' : ['subt', 'text', 'sbtl'].includes(handler) ? 'text' : null;
		if (!kind) continue;
		const name = utf8(hdlr.subarray(24)).replace(/\0+$/, '');
		const t: Mp4Track = {
			id,
			kind,
			codec: '',
			timescale,
			shift: 0,
			language: language === 'und' ? undefined : language,
			name: name && !/Handler$/i.test(name) ? name : undefined,
			trex: trex.get(id),
		};
		const stsd = child(stbl, ['stsd']);
		if (stsd) {
			const first = boxes(stsd, 8).next().value;
			if (first) sampleEntry(first, handler, t);
		}
		if (!t.codec) continue;
		// Edit list: the first non-empty edit's media time is the stream's zero.
		const elst = child(trak.body, ['edts', 'elst']);
		if (elst) {
			const r = new Reader(elst);
			const v = r.u8();
			r.skip(3);
			const n = r.u32();
			for (let i = 0; i < n; i++) {
				if (v === 1) {
					r.skip(8);
					const mt = r.u64();
					r.skip(4);
					if (mt !== 2 ** 64 - 1 && mt < 2 ** 53) {
						t.shift = mt / timescale;
						break;
					}
				} else {
					r.skip(4);
					const mt = r.i32();
					r.skip(4);
					if (mt >= 0) {
						t.shift = mt / timescale;
						break;
					}
				}
			}
		}
		t.table = table(stbl) ?? undefined;
		tracks.push(t);
	}
	return { tracks, duration, fragmented: !!mvex };
}

function table(stbl: Uint8Array): Table | null {
	const get = (n: string) => child(stbl, [n]);
	const stsz = get('stsz');
	const stz2 = get('stz2');
	let sizes: Uint32Array;
	if (stsz) {
		const r = new Reader(stsz, 4);
		const fixed = r.u32();
		const n = r.u32();
		sizes = new Uint32Array(n);
		for (let i = 0; i < n; i++) sizes[i] = fixed || r.u32();
	} else if (stz2) {
		const r = new Reader(stz2, 4);
		r.skip(3);
		const field = r.u8();
		const n = r.u32();
		sizes = new Uint32Array(n);
		for (let i = 0; i < n; i++) {
			if (field === 16) sizes[i] = r.u16();
			else if (field === 8) sizes[i] = r.u8();
			else {
				const b = stz2[r.pos + (i >> 1)];
				sizes[i] = i & 1 ? b & 15 : b >> 4;
			}
		}
	} else return null;
	const n = sizes.length;
	if (!n) return null;

	// Chunk offsets and samples per chunk
	const stco = get('stco');
	const co64 = get('co64');
	const chunks: number[] = [];
	if (stco) {
		const r = new Reader(stco, 4);
		const c = r.u32();
		for (let i = 0; i < c; i++) chunks.push(r.u32());
	} else if (co64) {
		const r = new Reader(co64, 4);
		const c = r.u32();
		for (let i = 0; i < c; i++) chunks.push(r.u64());
	}
	const stsc = get('stsc');
	const runs: { first: number; per: number }[] = [];
	if (stsc) {
		const r = new Reader(stsc, 4);
		const c = r.u32();
		for (let i = 0; i < c; i++) {
			const first = r.u32();
			const per = r.u32();
			r.skip(4);
			runs.push({ first: first - 1, per });
		}
	}
	const offsets = new Float64Array(n);
	let s = 0;
	for (let ri = 0; ri < runs.length && s < n; ri++) {
		const end = ri + 1 < runs.length ? runs[ri + 1].first : chunks.length;
		for (let c = runs[ri].first; c < end && s < n; c++) {
			let off = chunks[c];
			for (let k = 0; k < runs[ri].per && s < n; k++) {
				offsets[s] = off;
				off += sizes[s++];
			}
		}
	}

	const dts = new Float64Array(n);
	const stts = get('stts');
	if (stts) {
		const r = new Reader(stts, 4);
		const c = r.u32();
		let t = 0;
		let i = 0;
		for (let e = 0; e < c && i < n; e++) {
			const count = r.u32();
			const delta = r.u32();
			for (let k = 0; k < count && i < n; k++) {
				dts[i++] = t;
				t += delta;
			}
		}
		while (i < n) dts[i++] = t;
	}
	const cts = new Int32Array(n);
	const ctts = get('ctts');
	if (ctts) {
		const r = new Reader(ctts);
		const v = r.u8();
		r.skip(3);
		const c = r.u32();
		let i = 0;
		for (let e = 0; e < c && i < n; e++) {
			const count = r.u32();
			const off = v === 1 ? r.i32() : r.u32();
			for (let k = 0; k < count && i < n; k++) cts[i++] = off;
		}
	}
	let sync: Uint8Array | null = null;
	const stss = get('stss');
	if (stss) {
		sync = new Uint8Array(n);
		const r = new Reader(stss, 4);
		const c = r.u32();
		for (let i = 0; i < c; i++) {
			const k = r.u32() - 1;
			if (k < n) sync[k] = 1;
		}
	}
	return { offsets, sizes, dts, cts, sync };
}

/** Samples from a moof box and the bytes that follow it. */
export function parseMoof(
	moof: Uint8Array,
	/** The bytes from the moof's first byte to the end of its mdat. */
	all: Uint8Array,
	tracks: Map<number, Mp4Track>,
	out: Sample[],
	/** Seconds added to every time (DASH presentation offset, HLS timeline). */
	offset = 0,
): void {
	for (const traf of boxes(moof)) {
		if (traf.type !== 'traf') continue;
		const tfhd = child(traf.body, ['tfhd']);
		if (!tfhd) continue;
		const h = new Reader(tfhd);
		const tf = h.u32() & 0xffffff;
		const id = h.u32();
		const t = tracks.get(id);
		if (!t) continue;
		let base = 0;
		if (tf & 1) base = h.u64();
		if (tf & 2) h.skip(4);
		let defDur = t.trex?.duration ?? 0;
		let defSize = t.trex?.size ?? 0;
		let defFlags = t.trex?.flags ?? 0;
		if (tf & 8) defDur = h.u32();
		if (tf & 0x10) defSize = h.u32();
		if (tf & 0x20) defFlags = h.u32();
		let time = 0;
		const tfdt = child(traf.body, ['tfdt']);
		if (tfdt) {
			const r = new Reader(tfdt);
			time = r.u8() === 1 ? (r.skip(3), r.u64()) : (r.skip(3), r.u32());
		}
		let dataAt = tf & 1 ? base : 0;
		for (const tr of boxes(traf.body)) {
			if (tr.type !== 'trun') continue;
			const r = new Reader(tr.body);
			const v = r.u8();
			const flags = r.u24();
			const count = r.u32();
			if (flags & 1) dataAt = (tf & 1 ? base : 0) + r.i32();
			const firstFlags = flags & 4 ? r.u32() : null;
			for (let i = 0; i < count; i++) {
				const dur = flags & 0x100 ? r.u32() : defDur;
				const size = flags & 0x200 ? r.u32() : defSize;
				const sf = flags & 0x400 ? r.u32() : i === 0 && firstFlags != null ? firstFlags : defFlags;
				const cto = flags & 0x800 ? (v === 1 ? r.i32() : r.u32()) : 0;
				const key = !(sf & 0x10000) && (t.kind !== 'video' || ((sf >> 24) & 3) !== 1);
				const dts = time / t.timescale - t.shift + offset;
				out.push({
					track: id,
					dts,
					pts: (time + cto) / t.timescale - t.shift + offset,
					duration: dur / t.timescale,
					key,
					data: all.slice(dataAt, dataAt + size),
				});
				time += dur;
				dataAt += size;
			}
		}
	}
}

/** The first `tfdt` time in a moof, in seconds, for bisection. */
function moofTime(moof: Uint8Array, tracks: Map<number, Mp4Track>): number | null {
	const samples: Sample[] = [];
	parseMoof(moof, new Uint8Array(0), tracks, samples);
	const v = samples.find((s) => tracks.get(s.track)?.kind === 'video') ?? samples[0];
	return v ? v.dts : null;
}

/** An MP4 file read over HTTP ranges, progressive or fragmented. */
export class Mp4File implements Demuxer {
	tracks: Mp4Track[] = [];
	duration = NaN;
	private byId = new Map<number, Mp4Track>();
	private fragmented = false;
	private firstFragment = 0;
	private pos = 0;
	/** Progressive: every sample of the wanted tracks in file order. */
	private order: { track: Mp4Track; index: number }[] = [];
	private next = 0;
	private from = new Map<number, number>();
	private wanted = new Set<number>();
	private sidx: { time: number; pos: number }[] | null = null;

	constructor(private readonly src: ByteSource) {}

	async open() {
		let at = 0;
		let moov: Uint8Array | null = null;
		for (let guard = 0; guard < 64; guard++) {
			const head = await this.src.bytes(at, 16);
			const b = boxAt(head, 0, at);
			if (!b) break;
			if (b.type === 'moov') moov = await this.src.bytes(b.body, b.end - b.body);
			else if (b.type === 'sidx' && !this.sidx) this.sidx = this.parseSidx(await this.src.bytes(b.body, b.end - b.body), b.end);
			else if (b.type === 'moof' && !this.firstFragment) this.firstFragment = b.start;
			if (moov && (this.firstFragment || b.type === 'mdat')) break;
			if (!Number.isFinite(b.end)) break;
			at = b.end;
			if (Number.isFinite(this.src.size) && at >= this.src.size) break;
		}
		if (!moov) throw new Error('no moov');
		const m = parseMoov(moov);
		this.tracks = m.tracks;
		this.duration = m.duration;
		for (const t of this.tracks) this.byId.set(t.id, t);
		this.fragmented = !!this.firstFragment || (m.fragmented && !this.tracks.some((t) => t.table?.sizes.length));
		if (this.fragmented && !this.firstFragment) this.firstFragment = at;
		this.wanted = new Set(this.tracks.map((t) => t.id));
		if (!this.fragmented) this.index();
		else if (!(this.duration > 0) && Number.isFinite(this.src.size)) this.duration = await this.lastTime();
		await this.seek(0);
	}

	private parseSidx(body: Uint8Array, end: number): { time: number; pos: number }[] {
		const r = new Reader(body);
		const v = r.u8();
		r.skip(3 + 4);
		const scale = r.u32();
		let time = v === 0 ? r.u32() : r.u64();
		const first = v === 0 ? r.u32() : r.u64();
		r.skip(2);
		const n = r.u16();
		let pos = end + first;
		const out = [];
		for (let i = 0; i < n; i++) {
			const size = r.u32() & 0x7fffffff;
			const dur = r.u32();
			r.skip(4);
			out.push({ time: time / scale, pos });
			pos += size;
			time += dur;
		}
		return out;
	}

	private index() {
		this.order = [];
		for (const t of this.tracks) {
			if (!t.table || !this.wanted.has(t.id)) continue;
			for (let i = 0; i < t.table.sizes.length; i++) this.order.push({ track: t, index: i });
		}
		this.order.sort((a, b) => a.track.table!.offsets[a.index] - b.track.table!.offsets[b.index]);
	}

	select(ids: number[]) {
		const same = ids.length === this.wanted.size && ids.every((i) => this.wanted.has(i));
		this.wanted = new Set(ids);
		if (!same && !this.fragmented) {
			const at = this.order[this.next];
			this.index();
			if (at) {
				const off = at.track.table!.offsets[at.index];
				this.next = this.order.findIndex((o) => o.track.table!.offsets[o.index] >= off);
				if (this.next < 0) this.next = this.order.length;
			}
		}
	}

	private time(t: Mp4Track, i: number) {
		const tb = t.table!;
		return { dts: tb.dts[i] / t.timescale - t.shift, pts: (tb.dts[i] + tb.cts[i]) / t.timescale - t.shift };
	}

	async seek(time: number): Promise<number> {
		if (this.fragmented) return this.seekFragment(time);
		const video = this.tracks.find((t) => t.kind === 'video' && t.table) ?? this.tracks.find((t) => t.table);
		if (!video) return 0;
		const tb = video.table!;
		let k = 0;
		for (let i = 0; i < tb.sizes.length; i++) {
			if (this.time(video, i).pts > time + 1e-6 && i > 0) break;
			if (!tb.sync || tb.sync[i]) k = i;
		}
		const start = this.time(video, k).dts;
		// Every other track starts at its sample covering that time.
		this.from.clear();
		let minOffset = tb.offsets[k];
		this.from.set(video.id, k);
		for (const t of this.tracks) {
			if (t === video || !t.table) continue;
			let j = 0;
			while (j + 1 < t.table.sizes.length && this.time(t, j + 1).pts <= start) j++;
			this.from.set(t.id, j);
			if (this.wanted.has(t.id)) minOffset = Math.min(minOffset, t.table.offsets[j]);
		}
		this.next = this.order.findIndex((o) => o.track.table!.offsets[o.index] >= minOffset);
		if (this.next < 0) this.next = this.order.length;
		return Math.max(0, this.time(video, k).pts);
	}

	private async seekFragment(time: number): Promise<number> {
		if (time <= 0 || !this.firstFragment) {
			this.pos = this.firstFragment;
			return 0;
		}
		if (this.sidx?.length) {
			let pick = this.sidx[0];
			for (const s of this.sidx) if (s.time <= time) pick = s;
			this.pos = pick.pos;
			return pick.time;
		}
		// Bisect over moof boxes.
		let lo = this.firstFragment;
		let hi = this.src.size;
		let best = { pos: this.firstFragment, time: 0 };
		for (let i = 0; i < 24 && Number.isFinite(hi) && hi - lo > 1 << 16; i++) {
			const mid = Math.floor((lo + hi) / 2);
			const f = await this.findMoof(mid, hi);
			if (!f) {
				hi = mid;
				continue;
			}
			if (f.time <= time) {
				best = f;
				lo = f.pos + 8;
			} else hi = mid;
		}
		this.pos = best.pos;
		return best.time;
	}

	/** The end of the last fragment, for files whose moov gives no duration. */
	private async lastTime(): Promise<number> {
		const from = Math.max(this.firstFragment, this.src.size - (2 << 20));
		let at = from;
		let end = NaN;
		for (let guard = 0; guard < 4096; guard++) {
			const f = await this.findMoof(at, this.src.size);
			if (!f) break;
			const head = boxAt(await this.src.bytes(f.pos, 16), 0, f.pos);
			if (!head) break;
			const samples: Sample[] = [];
			parseMoof(await this.src.bytes(head.body, head.end - head.body), new Uint8Array(0), this.byId, samples);
			for (const s of samples) end = Math.max(Number.isFinite(end) ? end : 0, s.pts + s.duration);
			at = head.end;
		}
		return end;
	}

	private async findMoof(from: number, to: number): Promise<{ pos: number; time: number } | null> {
		const step = 1 << 16;
		for (let at = from; at < to; at += step - 8) {
			const b = await this.src.bytes(at, step);
			for (let i = 4; i + 4 < b.length; i++) {
				if (b[i] !== 0x6d || b[i + 1] !== 0x6f || b[i + 2] !== 0x6f || b[i + 3] !== 0x66) continue;
				const start = at + i - 4;
				const head = boxAt(await this.src.bytes(start, 16), 0, start);
				if (!head || head.type !== 'moof' || head.end - head.start > 1 << 22) continue;
				const moof = await this.src.bytes(head.body, head.end - head.body);
				const time = moofTime(moof, this.byId);
				if (time != null) return { pos: start, time };
			}
			if (b.length < step) break;
		}
		return null;
	}

	async read(): Promise<Sample[] | null> {
		return this.fragmented ? this.readFragment() : this.readProgressive();
	}

	private async readProgressive(): Promise<Sample[] | null> {
		if (this.next >= this.order.length) return null;
		const out: Sample[] = [];
		let bytes = 0;
		while (this.next < this.order.length && bytes < 1 << 19) {
			const { track, index } = this.order[this.next++];
			if (index < (this.from.get(track.id) ?? 0)) continue;
			const tb = track.table!;
			const data = (await this.src.bytes(tb.offsets[index], tb.sizes[index])).slice();
			bytes += data.length;
			const { dts, pts } = this.time(track, index);
			const nextDts = index + 1 < tb.sizes.length ? tb.dts[index + 1] : tb.dts[index];
			out.push({
				track: track.id,
				dts,
				pts,
				duration: Math.max(0, nextDts - tb.dts[index]) / track.timescale,
				key: !tb.sync || tb.sync[index] === 1,
				data,
			});
		}
		const ahead = this.order[this.next];
		if (ahead) this.src.prefetch(ahead.track.table!.offsets[ahead.index] + (1 << 20));
		return out;
	}

	private async readFragment(): Promise<Sample[] | null> {
		for (;;) {
			if (Number.isFinite(this.src.size) && this.pos >= this.src.size) return null;
			const head = boxAt(await this.src.bytes(this.pos, 16), 0, this.pos);
			if (!head) return null;
			if (head.type !== 'moof') {
				if (!Number.isFinite(head.end)) return null;
				this.pos = head.end;
				continue;
			}
			const moof = await this.src.bytes(head.body, head.end - head.body);
			// The mdat that follows
			const mh = boxAt(await this.src.bytes(head.end, 16), 0, head.end);
			const end = mh && mh.type === 'mdat' && Number.isFinite(mh.end) ? mh.end : head.end;
			this.src.prefetch(end + (1 << 20));
			const all = await this.src.bytes(head.start, end - head.start);
			const out: Sample[] = [];
			parseMoof(moof, all, this.byId, out);
			this.pos = end;
			return out.filter((s) => this.wanted.has(s.track));
		}
	}

	close() {
		this.src.close();
	}
}

/** The moov of an init segment (HLS EXT-X-MAP, DASH initialization). */
export function initSegment(b: Uint8Array) {
	for (const x of boxes(b)) if (x.type === 'moov') return parseMoov(x.body);
	throw new Error('init segment without moov');
}

/** Every moof in a media segment, as samples. */
export function mediaSegment(b: Uint8Array, tracks: Map<number, Mp4Track>, offset = 0): Sample[] {
	const out: Sample[] = [];
	let at = 0;
	while (at + 8 <= b.length) {
		const x = boxAt(b, at);
		if (!x) break;
		const end = Math.min(b.length, x.end);
		if (x.type === 'moof') parseMoof(b.subarray(x.body, end), b.subarray(at), tracks, out, offset);
		at = end;
	}
	return out;
}
